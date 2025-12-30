//! Memory enrichment service.
//!
//! Enriches memories with tags, structure, and context using LLM.

use crate::git::{NotesManager, YamlFrontMatterParser};
use crate::llm::LlmProvider;
use crate::{Error, Result};
use std::path::Path;

/// Service for enriching memories with LLM-generated tags and metadata.
pub struct EnrichmentService<P: LlmProvider> {
    /// LLM provider for generating enrichments.
    llm: P,
    /// Repository path for git notes access.
    repo_path: std::path::PathBuf,
}

impl<P: LlmProvider> EnrichmentService<P> {
    /// Creates a new enrichment service.
    #[must_use]
    pub fn new(llm: P, repo_path: impl AsRef<Path>) -> Self {
        Self {
            llm,
            repo_path: repo_path.as_ref().to_path_buf(),
        }
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
    /// Returns an error if enrichment fails.
    pub fn enrich_all(&self, dry_run: bool, update_all: bool) -> Result<EnrichmentStats> {
        let notes = NotesManager::new(&self.repo_path);
        let all_notes = notes.list()?;

        let mut stats = EnrichmentStats {
            total: all_notes.len(),
            ..Default::default()
        };

        for (commit_id, content) in &all_notes {
            self.process_note(commit_id, content, dry_run, update_all, &mut stats);
        }

        Ok(stats)
    }

    /// Processes a single note for enrichment.
    fn process_note(
        &self,
        commit_id: &str,
        content: &str,
        dry_run: bool,
        update_all: bool,
        stats: &mut EnrichmentStats,
    ) {
        let (metadata, body) = match YamlFrontMatterParser::parse(content) {
            Ok(result) => result,
            Err(e) => {
                tracing::warn!("Failed to parse note {commit_id}: {e}");
                stats.failed += 1;
                return;
            },
        };

        // Check if tags exist
        let has_tags = metadata
            .get("tags")
            .and_then(|v| v.as_array())
            .is_some_and(|arr| !arr.is_empty());

        // Skip if has tags and not updating all
        if has_tags && !update_all {
            stats.skipped += 1;
            return;
        }

        let memory_id = metadata
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or(commit_id);

        let namespace = metadata
            .get("namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("decisions");

        let new_tags = match self.generate_tags(&body, namespace) {
            Ok(tags) => tags,
            Err(e) => {
                tracing::warn!("Failed to generate tags for {memory_id}: {e}");
                stats.failed += 1;
                return;
            },
        };

        let action = if has_tags { "update" } else { "enrich" };

        if dry_run {
            tracing::info!("Would {action} {memory_id} with tags: {new_tags:?}");
            if has_tags {
                stats.would_update += 1;
            } else {
                stats.would_enrich += 1;
            }
            return;
        }

        match self.update_note_tags(commit_id, content, &new_tags) {
            Ok(()) => {
                tracing::info!("{action}ed {memory_id} with tags: {new_tags:?}");
                if has_tags {
                    stats.updated += 1;
                } else {
                    stats.enriched += 1;
                }
            },
            Err(e) => {
                tracing::warn!("Failed to update note {memory_id}: {e}");
                stats.failed += 1;
            },
        }
    }

    /// Enriches a specific memory by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the memory is not found or enrichment fails.
    pub fn enrich_one(&self, memory_id: &str, dry_run: bool) -> Result<EnrichmentResult> {
        let notes = NotesManager::new(&self.repo_path);
        let all_notes = notes.list()?;

        // Find the note with matching ID
        for (commit_id, content) in &all_notes {
            let (metadata, body) = match YamlFrontMatterParser::parse(content) {
                Ok(result) => result,
                Err(_) => continue,
            };

            let id = metadata
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or(commit_id);

            if id != memory_id {
                continue;
            }

            let namespace = metadata
                .get("namespace")
                .and_then(|v| v.as_str())
                .unwrap_or("decisions");

            // Generate tags
            let new_tags = self.generate_tags(&body, namespace)?;

            if dry_run {
                return Ok(EnrichmentResult {
                    memory_id: memory_id.to_string(),
                    new_tags,
                    applied: false,
                });
            }

            // Update the note
            self.update_note_tags(commit_id, content, &new_tags)?;

            return Ok(EnrichmentResult {
                memory_id: memory_id.to_string(),
                new_tags,
                applied: true,
            });
        }

        Err(Error::OperationFailed {
            operation: "enrich_one".to_string(),
            cause: format!("Memory not found: {memory_id}"),
        })
    }

    /// Generates tags for content using LLM.
    fn generate_tags(&self, content: &str, namespace: &str) -> Result<Vec<String>> {
        let prompt = format!(
            r#"Generate 3-5 relevant tags for this memory. Tags should be lowercase, hyphenated, and descriptive.

Namespace: {namespace}
Content: {content}

Respond with ONLY a JSON array of strings, no other text. Example: ["rust", "error-handling", "async"]"#
        );

        let response = self.llm.complete(&prompt)?;

        // Parse the JSON response
        let tags: Vec<String> =
            serde_json::from_str(&response).map_err(|e| Error::OperationFailed {
                operation: "parse_tags".to_string(),
                cause: format!("Failed to parse LLM response: {e}. Response was: {response}"),
            })?;

        Ok(tags)
    }

    /// Updates a note with new tags.
    fn update_note_tags(
        &self,
        commit_id: &str,
        original_content: &str,
        new_tags: &[String],
    ) -> Result<()> {
        let (mut metadata, body) = YamlFrontMatterParser::parse(original_content)?;

        // Update tags in metadata - metadata is a serde_json::Value (Object)
        if let Some(obj) = metadata.as_object_mut() {
            obj.insert(
                "tags".to_string(),
                serde_json::Value::Array(
                    new_tags
                        .iter()
                        .map(|t| serde_json::Value::String(t.clone()))
                        .collect(),
                ),
            );
        }

        // Use the parser's serialize method
        let updated_content = YamlFrontMatterParser::serialize(&metadata, &body)?;

        // Write the updated note
        let notes = NotesManager::new(&self.repo_path);
        notes.add(commit_id, &updated_content)?;

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
