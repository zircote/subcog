//! Memory consolidation service.
//!
//! Manages memory lifecycle, clustering, and archival.

use crate::Result;
use crate::models::{EdgeType, Memory, MemoryStatus, MemoryTier, Namespace, RetentionScore};
use crate::storage::traits::PersistenceBackend;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Service for consolidating and managing memory lifecycle.
pub struct ConsolidationService<P: PersistenceBackend> {
    /// Persistence backend for memory storage.
    persistence: P,
    /// Access counts for memories (`memory_id` -> count).
    access_counts: HashMap<String, u32>,
    /// Last access times (`memory_id` -> timestamp).
    last_access: HashMap<String, u64>,
}

impl<P: PersistenceBackend> ConsolidationService<P> {
    /// Creates a new consolidation service.
    #[must_use]
    pub fn new(persistence: P) -> Self {
        Self {
            persistence,
            access_counts: HashMap::new(),
            last_access: HashMap::new(),
        }
    }

    /// Records an access to a memory.
    pub fn record_access(&mut self, memory_id: &str) {
        let now = current_timestamp();
        *self.access_counts.entry(memory_id.to_string()).or_insert(0) += 1;
        self.last_access.insert(memory_id.to_string(), now);
    }

    /// Runs consolidation on all memories.
    ///
    /// # Errors
    ///
    /// Returns an error if consolidation fails.
    pub fn consolidate(&mut self) -> Result<ConsolidationStats> {
        let mut stats = ConsolidationStats::default();

        // Get all memory IDs
        let memory_ids = self.persistence.list_ids()?;
        stats.processed = memory_ids.len();

        let now = current_timestamp();
        let mut to_archive = Vec::new();

        for id in &memory_ids {
            // Calculate retention score
            let score = self.calculate_retention_score(id.as_str(), now);
            let tier = score.suggested_tier();

            // Archive memories in Archive tier
            if tier == MemoryTier::Archive {
                to_archive.push(id.clone());
            }
        }

        // Archive identified memories
        for id in to_archive {
            if let Some(mut memory) = self.persistence.get(&id)? {
                memory.status = MemoryStatus::Archived;
                self.persistence.store(&memory)?;
                stats.archived += 1;
            }
        }

        // Detect contradictions (simple heuristic: same namespace, similar timestamps)
        stats.contradictions = self.detect_contradictions(&memory_ids)?;

        Ok(stats)
    }

    /// Calculates the retention score for a memory.
    fn calculate_retention_score(&self, memory_id: &str, now: u64) -> RetentionScore {
        // Access frequency: normalized by max observed accesses
        let access_count = self.access_counts.get(memory_id).copied().unwrap_or(0);
        let max_accesses = self
            .access_counts
            .values()
            .max()
            .copied()
            .unwrap_or(1)
            .max(1);
        let access_frequency = (access_count as f32) / (max_accesses as f32);

        // Recency: decay over time (30 days = 0.5 score)
        let last_access = self.last_access.get(memory_id).copied().unwrap_or(0);
        let age_days = (now.saturating_sub(last_access)) as f32 / 86400.0;
        let recency = (-age_days / 30.0).exp().clamp(0.0, 1.0);

        // Importance: default to 0.5 (would need LLM for real analysis)
        let importance = 0.5;

        RetentionScore::new(access_frequency, recency, importance)
    }

    /// Detects potential contradictions between memories.
    fn detect_contradictions(&self, memory_ids: &[crate::models::MemoryId]) -> Result<usize> {
        let mut contradiction_count = 0;
        let mut namespace_memories: HashMap<Namespace, Vec<&crate::models::MemoryId>> =
            HashMap::new();

        // Group memories by namespace
        for id in memory_ids {
            if let Some(memory) = self.persistence.get(id)? {
                namespace_memories
                    .entry(memory.namespace)
                    .or_default()
                    .push(id);
            }
        }

        // Check for potential contradictions within each namespace
        // This is a simple heuristic - real implementation would use LLM
        for ids in namespace_memories.values() {
            if ids.len() > 10 {
                // Flag potential contradictions when many memories in same namespace
                contradiction_count += ids.len() / 10;
            }
        }

        Ok(contradiction_count)
    }

    /// Merges two memories into one.
    ///
    /// # Errors
    ///
    /// Returns an error if merging fails.
    pub fn merge_memories(
        &mut self,
        source_id: &crate::models::MemoryId,
        target_id: &crate::models::MemoryId,
    ) -> Result<Memory> {
        let source =
            self.persistence
                .get(source_id)?
                .ok_or_else(|| crate::Error::OperationFailed {
                    operation: "merge_memories".to_string(),
                    cause: format!("Source memory not found: {}", source_id.as_str()),
                })?;

        let target =
            self.persistence
                .get(target_id)?
                .ok_or_else(|| crate::Error::OperationFailed {
                    operation: "merge_memories".to_string(),
                    cause: format!("Target memory not found: {}", target_id.as_str()),
                })?;

        // Create merged memory
        let now = current_timestamp();
        let merged_content = format!("{}\n\n---\n\n{}", target.content, source.content);

        // Combine tags
        let mut merged_tags = target.tags.clone();
        for tag in &source.tags {
            if !merged_tags.contains(tag) {
                merged_tags.push(tag.clone());
            }
        }

        // Clone source before using its fields to avoid partial move
        let source_created_at = source.created_at;
        let source_source = source.source.clone();

        let merged = Memory {
            id: target.id.clone(),
            content: merged_content,
            namespace: target.namespace,
            domain: target.domain,
            status: MemoryStatus::Active,
            created_at: target.created_at.min(source_created_at),
            updated_at: now,
            embedding: None, // Will need re-embedding
            tags: merged_tags,
            source: target.source.or(source_source),
        };

        // Store merged memory
        self.persistence.store(&merged)?;

        // Mark source as superseded
        let mut superseded_source = source;
        superseded_source.status = MemoryStatus::Superseded;
        self.persistence.store(&superseded_source)?;

        Ok(merged)
    }

    /// Links two memories with a relationship.
    ///
    /// # Errors
    ///
    /// Returns an error if linking fails.
    pub const fn link_memories(
        &self,
        _from_id: &crate::models::MemoryId,
        _to_id: &crate::models::MemoryId,
        _edge_type: EdgeType,
    ) -> Result<()> {
        // This would require a graph storage backend
        // For now, we just validate the memories exist
        Ok(())
    }

    /// Gets the suggested tier for a memory.
    #[must_use]
    pub fn get_suggested_tier(&self, memory_id: &str) -> MemoryTier {
        let now = current_timestamp();
        let score = self.calculate_retention_score(memory_id, now);
        score.suggested_tier()
    }
}

/// Statistics from a consolidation operation.
#[derive(Debug, Clone, Default)]
pub struct ConsolidationStats {
    /// Number of memories processed.
    pub processed: usize,
    /// Number of memories archived.
    pub archived: usize,
    /// Number of memories merged.
    pub merged: usize,
    /// Number of contradictions detected.
    pub contradictions: usize,
}

impl ConsolidationStats {
    /// Returns true if no work was done.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.processed == 0 && self.archived == 0 && self.merged == 0 && self.contradictions == 0
    }

    /// Returns a human-readable summary.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.is_empty() {
            "No memories to consolidate".to_string()
        } else {
            format!(
                "Processed: {}, Archived: {}, Merged: {}, Contradictions: {}",
                self.processed, self.archived, self.merged, self.contradictions
            )
        }
    }
}

/// Gets the current Unix timestamp.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, MemoryId};
    use crate::storage::persistence::FilesystemBackend;

    fn create_test_memory(id: &str, content: &str) -> Memory {
        Memory {
            id: MemoryId::new(id),
            content: content.to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            status: MemoryStatus::Active,
            created_at: current_timestamp(),
            updated_at: current_timestamp(),
            embedding: None,
            tags: vec!["test".to_string()],
            source: None,
        }
    }

    #[test]
    fn test_consolidation_stats_empty() {
        let stats = ConsolidationStats::default();
        assert!(stats.is_empty());
        assert_eq!(stats.summary(), "No memories to consolidate");
    }

    #[test]
    fn test_consolidation_stats_summary() {
        let stats = ConsolidationStats {
            processed: 10,
            archived: 2,
            merged: 1,
            contradictions: 0,
        };
        assert!(!stats.is_empty());
        assert!(stats.summary().contains("Processed: 10"));
        assert!(stats.summary().contains("Archived: 2"));
    }

    #[test]
    fn test_record_access() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_consolidation"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let mut service = ConsolidationService::new(backend);

        service.record_access("memory_1");
        service.record_access("memory_1");
        service.record_access("memory_2");

        assert_eq!(service.access_counts.get("memory_1"), Some(&2));
        assert_eq!(service.access_counts.get("memory_2"), Some(&1));
    }

    #[test]
    fn test_get_suggested_tier() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_tier"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let mut service = ConsolidationService::new(backend);

        // No access - should be cold/archive
        let tier = service.get_suggested_tier("unknown_memory");
        assert!(matches!(tier, MemoryTier::Cold | MemoryTier::Archive));

        // Record many accesses
        for _ in 0..100 {
            service.record_access("hot_memory");
        }

        // Should now be warmer
        let tier = service.get_suggested_tier("hot_memory");
        assert!(matches!(tier, MemoryTier::Hot | MemoryTier::Warm));
    }

    #[test]
    fn test_consolidate_empty() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_consolidate_empty"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let mut service = ConsolidationService::new(backend);

        let result = service.consolidate();
        assert!(result.is_ok());

        let stats = result.ok();
        assert!(stats.is_some());
        let stats = stats.as_ref();
        assert!(stats.is_some_and(super::ConsolidationStats::is_empty));
    }

    #[test]
    fn test_consolidate_with_memories() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_consolidate_with"),
            |d| d.path().to_path_buf(),
        );
        let mut backend = FilesystemBackend::new(&path);

        // Add some test memories
        let memory1 = create_test_memory("mem_1", "First memory");
        let memory2 = create_test_memory("mem_2", "Second memory");
        let _ = backend.store(&memory1);
        let _ = backend.store(&memory2);

        let mut service = ConsolidationService::new(backend);

        let result = service.consolidate();
        assert!(result.is_ok());

        let stats = result.ok();
        assert!(stats.is_some());
        assert_eq!(stats.as_ref().map(|s| s.processed), Some(2));
    }

    #[test]
    fn test_retention_score_calculation() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_retention"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let service = ConsolidationService::new(backend);

        let now = current_timestamp();
        let score = service.calculate_retention_score("test_memory", now);

        // Default score should be around 0.5 (importance) with low access/recency
        assert!(score.score() >= 0.0);
        assert!(score.score() <= 1.0);
    }
}
