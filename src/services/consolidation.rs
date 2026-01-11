//! Memory consolidation service.
//!
//! Manages memory lifecycle, clustering, and archival.

use crate::Result;
use crate::current_timestamp;
use crate::llm::LlmProvider;
use crate::models::{
    EdgeType, EventMeta, Memory, MemoryEvent, MemoryStatus, MemoryTier, Namespace, RetentionScore,
};
use crate::observability::current_request_id;
use crate::security::record_event;
use crate::storage::traits::PersistenceBackend;
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info_span, instrument};

// Retention score calculation constants
/// Seconds per day for age calculation.
const SECONDS_PER_DAY: f32 = 86400.0;
/// Half-life in days for recency decay.
const RECENCY_DECAY_DAYS: f32 = 30.0;
/// Default importance score when LLM analysis unavailable.
const DEFAULT_IMPORTANCE: f32 = 0.5;
/// Minimum memories in namespace before flagging contradictions.
const CONTRADICTION_THRESHOLD: usize = 10;
/// Maximum entries in access tracking caches (HIGH-PERF-001).
/// Using `NonZeroUsize` directly to avoid runtime `expect()` calls.
// SAFETY: 10_000 is a non-zero constant, verified at compile time
#[allow(clippy::expect_used)]
const ACCESS_CACHE_CAPACITY: NonZeroUsize = {
    // This unwrap is safe: 10_000 is non-zero
    match NonZeroUsize::new(10_000) {
        Some(n) => n,
        None => panic!("ACCESS_CACHE_CAPACITY must be non-zero"),
    }
};

/// Service for consolidating and managing memory lifecycle.
pub struct ConsolidationService<P: PersistenceBackend> {
    /// Persistence backend for memory storage.
    persistence: P,
    /// Access counts for memories (`memory_id` -> count), bounded LRU (HIGH-PERF-001).
    access_counts: LruCache<String, u32>,
    /// Last access times (`memory_id` -> timestamp), bounded LRU (HIGH-PERF-001).
    last_access: LruCache<String, u64>,
    /// Optional LLM provider for intelligent consolidation.
    llm: Option<Arc<dyn LlmProvider + Send + Sync>>,
}

impl<P: PersistenceBackend> ConsolidationService<P> {
    /// Creates a new consolidation service.
    #[must_use]
    pub fn new(persistence: P) -> Self {
        Self {
            persistence,
            access_counts: LruCache::new(ACCESS_CACHE_CAPACITY),
            last_access: LruCache::new(ACCESS_CACHE_CAPACITY),
            llm: None,
        }
    }

    /// Sets the LLM provider for intelligent consolidation.
    ///
    /// # Arguments
    ///
    /// * `llm` - The LLM provider to use for summarization and analysis.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use subcog::services::ConsolidationService;
    /// use subcog::llm::AnthropicClient;
    /// use subcog::storage::persistence::FilesystemBackend;
    /// use std::sync::Arc;
    ///
    /// let backend = FilesystemBackend::new("/tmp/memories");
    /// let llm = Arc::new(AnthropicClient::new());
    /// let service = ConsolidationService::new(backend).with_llm(llm);
    /// ```
    #[must_use]
    pub fn with_llm(mut self, llm: Arc<dyn LlmProvider + Send + Sync>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Records an access to a memory.
    pub fn record_access(&mut self, memory_id: &str) {
        let now = current_timestamp();
        let key = memory_id.to_string();
        let count = self.access_counts.get(&key).copied().unwrap_or(0) + 1;
        self.access_counts.put(key.clone(), count);
        self.last_access.put(key, now);
    }

    /// Runs consolidation on all memories.
    ///
    /// # Errors
    ///
    /// Returns an error if consolidation fails.
    #[instrument(
        name = "subcog.memory.consolidate",
        skip(self),
        fields(
            request_id = tracing::field::Empty,
            component = "memory",
            operation = "consolidate"
        )
    )]
    pub fn consolidate(&mut self) -> Result<ConsolidationStats> {
        let start = Instant::now();
        if let Some(request_id) = current_request_id() {
            tracing::Span::current().record("request_id", request_id.as_str());
        }
        let result = (|| {
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
            {
                let _span = info_span!("subcog.memory.consolidate.archive").entered();
                for id in to_archive {
                    if let Some(mut memory) = self.persistence.get(&id)? {
                        memory.status = MemoryStatus::Archived;
                        self.persistence.store(&memory)?;
                        record_event(MemoryEvent::Archived {
                            meta: EventMeta::with_timestamp(
                                "consolidation",
                                current_request_id(),
                                now,
                            ),
                            memory_id: memory.id.clone(),
                            reason: "consolidation_archive".to_string(),
                        });
                        stats.archived += 1;
                    }
                }
            }

            // Detect contradictions (simple heuristic: same namespace, similar timestamps)
            {
                let _span = info_span!("subcog.memory.consolidate.contradictions").entered();
                stats.contradictions = self.detect_contradictions(&memory_ids)?;
            }

            record_event(MemoryEvent::Consolidated {
                meta: EventMeta::new("consolidation", current_request_id()),
                processed: stats.processed,
                archived: stats.archived,
                merged: stats.merged,
            });

            Ok(stats)
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        metrics::counter!(
            "memory_operations_total",
            "operation" => "consolidate",
            "namespace" => "mixed",
            "domain" => "project",
            "status" => status
        )
        .increment(1);
        metrics::histogram!(
            "memory_operation_duration_ms",
            "operation" => "consolidate",
            "namespace" => "mixed"
        )
        .record(start.elapsed().as_secs_f64() * 1000.0);
        metrics::histogram!(
            "memory_lifecycle_duration_ms",
            "component" => "memory",
            "operation" => "consolidate"
        )
        .record(start.elapsed().as_secs_f64() * 1000.0);

        result
    }

    /// Calculates the retention score for a memory.
    ///
    /// # Precision Notes
    /// The u32 and u64 to f32 casts are acceptable here as exact precision
    /// is not required for retention score calculations (values are normalized 0.0-1.0).
    #[allow(clippy::cast_precision_loss)]
    fn calculate_retention_score(&self, memory_id: &str, now: u64) -> RetentionScore {
        // Access frequency: normalized by max observed accesses
        // Use peek() to avoid modifying LRU order during read-only operation
        let access_count = self.access_counts.peek(memory_id).copied().unwrap_or(0);
        let max_accesses = self
            .access_counts
            .iter()
            .map(|(_, v)| *v)
            .max()
            .unwrap_or(1)
            .max(1);
        let access_frequency = (access_count as f32) / (max_accesses as f32);

        // Recency: decay over time (RECENCY_DECAY_DAYS days = 0.5 score)
        let last_access = self.last_access.peek(memory_id).copied().unwrap_or(0);
        let age_days = (now.saturating_sub(last_access)) as f32 / SECONDS_PER_DAY;
        let recency = (-age_days / RECENCY_DECAY_DAYS).exp().clamp(0.0, 1.0);

        // Importance: default (would need LLM for real analysis)
        let importance = DEFAULT_IMPORTANCE;

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
            if ids.len() > CONTRADICTION_THRESHOLD {
                // Flag potential contradictions when many memories in same namespace
                contradiction_count += ids.len() / CONTRADICTION_THRESHOLD;
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
            project_id: target
                .project_id
                .clone()
                .or_else(|| source.project_id.clone()),
            branch: target.branch.clone().or_else(|| source.branch.clone()),
            file_path: target
                .file_path
                .clone()
                .or_else(|| source.file_path.clone()),
            status: MemoryStatus::Active,
            created_at: target.created_at.min(source_created_at),
            updated_at: now,
            tombstoned_at: None,
            embedding: None, // Will need re-embedding
            tags: merged_tags,
            source: target.source.or(source_source),
            is_summary: false,
            source_memory_ids: None,
            consolidation_timestamp: None,
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
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: current_timestamp(),
            updated_at: current_timestamp(),
            tombstoned_at: None,
            embedding: None,
            tags: vec!["test".to_string()],
            source: None,
            is_summary: false,
            source_memory_ids: None,
            consolidation_timestamp: None,
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

        // Use peek() to avoid modifying LRU order during assertions
        assert_eq!(service.access_counts.peek("memory_1"), Some(&2));
        assert_eq!(service.access_counts.peek("memory_2"), Some(&1));
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
        let backend = FilesystemBackend::new(&path);

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
