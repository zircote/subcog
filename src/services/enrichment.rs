//! Memory enrichment service.
//!
//! Enriches memories with tags, structure, and context using LLM.

use crate::llm::{
    LlmProvider, OperationMode, build_system_prompt, sanitize_llm_response_for_error,
};
use crate::models::{Memory, MemoryId, SearchFilter};
use crate::storage::traits::IndexBackend;
use crate::{Error, Result};
use std::sync::Arc;
use std::time::Instant;
use tracing::instrument;

/// Service for enriching memories with LLM-generated tags and metadata.
pub struct EnrichmentService<P: LlmProvider> {
    /// LLM provider for generating enrichments.
    llm: P,
    /// Index backend for memory access.
    index: Arc<dyn IndexBackend>,
}

impl<P: LlmProvider> EnrichmentService<P> {
    /// Creates a new enrichment service.
    #[must_use]
    pub fn new(llm: P, index: Arc<dyn IndexBackend>) -> Self {
        Self { llm, index }
    }

    /// Enriches all memories that have empty tags.
    ///
    /// # Arguments
    ///
    /// * `dry_run` - If true, shows what would be changed without applying
    /// * `update_all` - If true, updates all memories even if they have tags
    ///
    /// # Errors
    ///
    /// Returns [`Error::OperationFailed`] if:
    /// - Memory listing fails (database access error)
    /// - LLM enrichment fails for all memories
    #[instrument(skip(self), fields(operation = "enrich_all", dry_run = dry_run, update_all = update_all))]
    pub fn enrich_all(&self, dry_run: bool, update_all: bool) -> Result<EnrichmentStats> {
        let start = Instant::now();
        let result = (|| {
            // Get all memory IDs from SQLite
            let filter = SearchFilter::default();
            let all_ids = self.index.list_all(&filter, usize::MAX)?;

            let mut stats = EnrichmentStats {
                total: all_ids.len(),
                ..Default::default()
            };

            for (memory_id, _score) in &all_ids {
                if let Some(memory) = self.index.get_memory(memory_id)? {
                    self.process_memory(&memory, dry_run, update_all, &mut stats);
                }
            }

            Ok(stats)
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        metrics::counter!(
            "memory_operations_total",
            "operation" => "enrich",
            "namespace" => "mixed",
            "domain" => "project",
            "status" => status
        )
        .increment(1);
        metrics::histogram!(
            "memory_operation_duration_ms",
            "operation" => "enrich",
            "namespace" => "mixed"
        )
        .record(start.elapsed().as_secs_f64() * 1000.0);

        result
    }

    /// Processes a single memory for enrichment.
    fn process_memory(
        &self,
        memory: &Memory,
        dry_run: bool,
        update_all: bool,
        stats: &mut EnrichmentStats,
    ) {
        // Check if tags exist
        let has_tags = !memory.tags.is_empty();

        // Skip if has tags and not updating all
        if has_tags && !update_all {
            stats.skipped += 1;
            return;
        }

        let namespace = memory.namespace.as_str();

        let new_tags = match self.generate_tags(&memory.content, namespace) {
            Ok(tags) => tags,
            Err(e) => {
                tracing::warn!("Failed to generate tags for {}: {e}", memory.id.as_str());
                stats.failed += 1;
                return;
            },
        };

        let action = if has_tags { "update" } else { "enrich" };

        if dry_run {
            tracing::info!(
                "Would {action} {} with tags: {new_tags:?}",
                memory.id.as_str()
            );
            if has_tags {
                stats.would_update += 1;
            } else {
                stats.would_enrich += 1;
            }
            return;
        }

        match self.update_memory_tags(memory, &new_tags) {
            Ok(()) => {
                tracing::info!("{action}ed {} with tags: {new_tags:?}", memory.id.as_str());
                if has_tags {
                    stats.updated += 1;
                } else {
                    stats.enriched += 1;
                }
            },
            Err(e) => {
                tracing::warn!("Failed to update memory {}: {e}", memory.id.as_str());
                stats.failed += 1;
            },
        }
    }

    /// Enriches a specific memory by ID.
    ///
    /// # Errors
    ///
    /// Returns [`Error::OperationFailed`] if:
    /// - The memory with the given ID is not found
    /// - LLM tag generation fails (provider error or invalid response)
    /// - Memory update fails (database error)
    #[instrument(skip(self), fields(operation = "enrich_one", dry_run = dry_run, memory_id = memory_id))]
    pub fn enrich_one(&self, memory_id: &str, dry_run: bool) -> Result<EnrichmentResult> {
        let start = Instant::now();
        let result = (|| {
            let id = MemoryId::new(memory_id);
            let memory = self
                .index
                .get_memory(&id)?
                .ok_or_else(|| Error::OperationFailed {
                    operation: "enrich_one".to_string(),
                    cause: format!("Memory not found: {memory_id}"),
                })?;

            let namespace = memory.namespace.as_str();

            // Generate tags
            let new_tags = self.generate_tags(&memory.content, namespace)?;

            if dry_run {
                return Ok(EnrichmentResult {
                    memory_id: memory_id.to_string(),
                    new_tags,
                    applied: false,
                });
            }

            // Update the memory
            self.update_memory_tags(&memory, &new_tags)?;

            Ok(EnrichmentResult {
                memory_id: memory_id.to_string(),
                new_tags,
                applied: true,
            })
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        metrics::counter!(
            "memory_operations_total",
            "operation" => "enrich",
            "namespace" => "mixed",
            "domain" => "project",
            "status" => status
        )
        .increment(1);
        metrics::histogram!(
            "memory_operation_duration_ms",
            "operation" => "enrich",
            "namespace" => "mixed"
        )
        .record(start.elapsed().as_secs_f64() * 1000.0);

        result
    }

    /// Generates tags for content using LLM.
    fn generate_tags(&self, content: &str, namespace: &str) -> Result<Vec<String>> {
        let system = build_system_prompt(OperationMode::Enrichment, None);
        let user_prompt = format!(
            "Generate tags for this memory.\n\nNamespace: {namespace}\nContent: {content}\n\nReturn ONLY a JSON array of strings."
        );
        let response = self.llm.complete_with_system(&system, &user_prompt)?;

        // Parse the JSON response
        let sanitized = sanitize_llm_response_for_error(&response);
        let tags: Vec<String> =
            serde_json::from_str(&response).map_err(|e| Error::OperationFailed {
                operation: "parse_tags".to_string(),
                cause: format!("Failed to parse LLM response: {e}. Response was: {sanitized}"),
            })?;

        Ok(tags)
    }

    /// Updates a memory with new tags.
    fn update_memory_tags(&self, memory: &Memory, new_tags: &[String]) -> Result<()> {
        // Create updated memory with new tags
        let updated_memory = Memory {
            id: memory.id.clone(),
            content: memory.content.clone(),
            namespace: memory.namespace,
            domain: memory.domain.clone(),
            project_id: memory.project_id.clone(),
            branch: memory.branch.clone(),
            file_path: memory.file_path.clone(),
            status: memory.status,
            created_at: memory.created_at,
            updated_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(memory.updated_at),
            tombstoned_at: memory.tombstoned_at,
            expires_at: memory.expires_at,
            embedding: memory.embedding.clone(),
            tags: new_tags.to_vec(),
            #[cfg(feature = "group-scope")]
            group_id: memory.group_id.clone(),
            source: memory.source.clone(),
            is_summary: memory.is_summary,
            source_memory_ids: memory.source_memory_ids.clone(),
            consolidation_timestamp: memory.consolidation_timestamp,
        };

        // Re-index the updated memory
        self.index.index(&updated_memory)?;

        Ok(())
    }
}

/// Statistics from a batch enrichment operation.
#[derive(Debug, Clone, Default)]
pub struct EnrichmentStats {
    /// Total memories scanned.
    pub total: usize,
    /// Memories newly enriched (had no tags).
    pub enriched: usize,
    /// Memories updated (had existing tags).
    pub updated: usize,
    /// Memories skipped (already have tags, not in update mode).
    pub skipped: usize,
    /// Memories that would be enriched (dry run).
    pub would_enrich: usize,
    /// Memories that would be updated (dry run).
    pub would_update: usize,
    /// Memories that failed to enrich.
    pub failed: usize,
}

impl EnrichmentStats {
    /// Returns a human-readable summary.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.would_enrich > 0 || self.would_update > 0 {
            format!(
                "Dry run: {} would be enriched, {} would be updated, {} skipped, {} failed (of {} total)",
                self.would_enrich, self.would_update, self.skipped, self.failed, self.total
            )
        } else {
            format!(
                "Enriched: {}, Updated: {}, Skipped: {}, Failed: {} (of {} total)",
                self.enriched, self.updated, self.skipped, self.failed, self.total
            )
        }
    }
}

/// Result of enriching a single memory.
#[derive(Debug, Clone)]
pub struct EnrichmentResult {
    /// Memory ID that was enriched.
    pub memory_id: String,
    /// New tags that were generated.
    pub new_tags: Vec<String>,
    /// Whether the changes were applied.
    pub applied: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enrichment_stats_summary() {
        let stats = EnrichmentStats {
            total: 10,
            enriched: 5,
            updated: 2,
            skipped: 1,
            would_enrich: 0,
            would_update: 0,
            failed: 2,
        };
        let summary = stats.summary();
        assert!(summary.contains("Enriched: 5"));
        assert!(summary.contains("Updated: 2"));
        assert!(summary.contains("Skipped: 1"));
        assert!(summary.contains("Failed: 2"));
    }

    #[test]
    fn test_enrichment_stats_dry_run_summary() {
        let stats = EnrichmentStats {
            total: 10,
            enriched: 0,
            updated: 0,
            skipped: 1,
            would_enrich: 5,
            would_update: 2,
            failed: 2,
        };
        let summary = stats.summary();
        assert!(summary.contains("Dry run"));
        assert!(summary.contains("5 would be enriched"));
        assert!(summary.contains("2 would be updated"));
    }

    #[test]
    fn test_enrichment_stats_default() {
        let stats = EnrichmentStats::default();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.enriched, 0);
        assert_eq!(stats.updated, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.failed, 0);
    }

    #[test]
    fn test_enrichment_result() {
        let result = EnrichmentResult {
            memory_id: "test-id".to_string(),
            new_tags: vec!["rust".to_string(), "memory".to_string()],
            applied: true,
        };
        assert_eq!(result.memory_id, "test-id");
        assert_eq!(result.new_tags.len(), 2);
        assert!(result.applied);
    }
}
