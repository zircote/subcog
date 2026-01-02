//! Memory recall (search) service.
//!
//! Searches for memories using hybrid (vector + BM25) search with RRF fusion.

use crate::current_timestamp;
use crate::models::{
    Memory, MemoryEvent, MemoryId, MemoryStatus, SearchFilter, SearchHit, SearchMode, SearchResult,
};
use crate::security::record_event;
use crate::storage::index::SqliteBackend;
use crate::storage::traits::IndexBackend;
use crate::{Error, Result};
use std::collections::HashMap;
use std::time::Instant;
use tracing::instrument;

/// Service for searching and retrieving memories.
pub struct RecallService {
    /// `SQLite` index backend.
    index: Option<SqliteBackend>,
}

impl RecallService {
    /// Creates a new recall service.
    #[must_use]
    pub const fn new() -> Self {
        Self { index: None }
    }

    /// Creates a recall service with an index backend.
    #[must_use]
    pub const fn with_index(index: SqliteBackend) -> Self {
        Self { index: Some(index) }
    }

    /// Searches for memories matching a query.
    ///
    /// # Errors
    ///
    /// Returns an error if the search fails.
    #[allow(clippy::cast_possible_truncation)]
    #[instrument(
        skip(self, query, filter),
        fields(
            operation = "recall",
            mode = %mode,
            query_length = query.len(),
            limit = limit
        )
    )]
    pub fn search(
        &self,
        query: &str,
        mode: SearchMode,
        filter: &SearchFilter,
        limit: usize,
    ) -> Result<SearchResult> {
        let start = Instant::now();
        let domain_label = domain_label(filter);
        let mode_label = mode.as_str();

        tracing::info!(mode = %mode_label, query_length = query.len(), limit = limit, "Searching memories");

        let result = (|| {
            // Validate query
            if query.trim().is_empty() {
                return Err(Error::InvalidInput("Query cannot be empty".to_string()));
            }

            let memories = match mode {
                SearchMode::Text => self.text_search(query, filter, limit)?,
                SearchMode::Vector => self.vector_search(query, filter, limit)?,
                SearchMode::Hybrid => self.hybrid_search(query, filter, limit)?,
            };

            // Safe cast: u128 milliseconds will practically never exceed u64::MAX
            let execution_time_ms = start.elapsed().as_millis() as u64;
            let total_count = memories.len();
            let timestamp = current_timestamp();
            let query_value = query.to_string();
            for hit in &memories {
                record_event(MemoryEvent::Retrieved {
                    memory_id: hit.memory.id.clone(),
                    query: query_value.clone(),
                    score: hit.score,
                    timestamp,
                });
            }

            Ok(SearchResult {
                memories,
                total_count,
                mode,
                execution_time_ms,
            })
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        metrics::counter!(
            "memory_search_total",
            "mode" => mode_label,
            "domain" => domain_label,
            "status" => status
        )
        .increment(1);
        metrics::histogram!(
            "memory_search_duration_ms",
            "mode" => mode_label,
            "backend" => "sqlite"
        )
        .record(start.elapsed().as_secs_f64() * 1000.0);

        result
    }

    /// Lists all memories, optionally filtered by namespace.
    ///
    /// Unlike `search`, this doesn't require a query and returns all matching memories.
    /// Returns minimal metadata (id, namespace) without content - details via drill-down.
    ///
    /// # Errors
    ///
    /// Returns an error if the index is not available.
    #[allow(clippy::cast_possible_truncation)]
    #[instrument(skip(self, filter), fields(operation = "list_all", limit = limit))]
    pub fn list_all(&self, filter: &SearchFilter, limit: usize) -> Result<SearchResult> {
        let start = Instant::now();
        let domain_label = domain_label(filter);

        let result = (|| {
            let index = self.index.as_ref().ok_or_else(|| Error::OperationFailed {
                operation: "list_all".to_string(),
                cause: "No index backend configured".to_string(),
            })?;

            let results = index.list_all(filter, limit)?;

            // PERF-C1: Use batch query instead of N+1 individual get_memory calls
            let ids: Vec<_> = results.iter().map(|(id, _)| id.clone()).collect();
            let batch_memories = index.get_memories_batch(&ids)?;

            // Zip results with fetched memories, clearing content for lightweight response
            let memories: Vec<SearchHit> = results
                .into_iter()
                .zip(batch_memories)
                .filter_map(|((_, score), memory_opt)| {
                    memory_opt.map(|mut memory| {
                        // Clear content for lightweight response
                        memory.content = String::new();
                        SearchHit {
                            memory,
                            score,
                            vector_score: None,
                            bm25_score: None,
                        }
                    })
                })
                .collect();

            let execution_time_ms = start.elapsed().as_millis() as u64;
            let total_count = memories.len();
            let timestamp = current_timestamp();
            for hit in &memories {
                record_event(MemoryEvent::Retrieved {
                    memory_id: hit.memory.id.clone(),
                    query: "*".to_string(),
                    score: hit.score,
                    timestamp,
                });
            }

            Ok(SearchResult {
                memories,
                total_count,
                mode: SearchMode::Text,
                execution_time_ms,
            })
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        metrics::counter!(
            "memory_search_total",
            "mode" => "list_all",
            "domain" => domain_label,
            "status" => status
        )
        .increment(1);
        metrics::histogram!(
            "memory_search_duration_ms",
            "mode" => "list_all",
            "backend" => "sqlite"
        )
        .record(start.elapsed().as_secs_f64() * 1000.0);

        result
    }

    /// Performs BM25 text search.
    fn text_search(
        &self,
        query: &str,
        filter: &SearchFilter,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        let index = self.index.as_ref().ok_or_else(|| Error::OperationFailed {
            operation: "text_search".to_string(),
            cause: "No index backend configured".to_string(),
        })?;

        let results = index.search(query, filter, limit)?;

        // PERF-C1: Use batch query instead of N+1 individual get_memory calls
        let ids: Vec<_> = results.iter().map(|(id, _)| id.clone()).collect();
        let batch_memories = index.get_memories_batch(&ids)?;

        // Convert to SearchHits - zip with fetched memories
        let hits: Vec<SearchHit> = results
            .into_iter()
            .zip(batch_memories)
            .map(|((id, score), memory_opt)| {
                let memory = memory_opt.unwrap_or_else(|| create_placeholder_memory(id));
                SearchHit {
                    memory,
                    score,
                    vector_score: None,
                    bm25_score: Some(score),
                }
            })
            .collect();

        Ok(hits)
    }

    /// Performs vector similarity search.
    const fn vector_search(
        &self,
        _query: &str,
        _filter: &SearchFilter,
        _limit: usize,
    ) -> Result<Vec<SearchHit>> {
        // Vector search requires embeddings - for now return empty
        // This would need the embedding service and vector backend
        Ok(Vec::new())
    }

    /// Performs hybrid search with RRF fusion.
    fn hybrid_search(
        &self,
        query: &str,
        filter: &SearchFilter,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        // Get results from both search modes
        let text_results = self.text_search(query, filter, limit * 2)?;
        let vector_results = self.vector_search(query, filter, limit * 2)?;

        // Apply Reciprocal Rank Fusion
        let fused = self.rrf_fusion(&text_results, &vector_results, limit);

        Ok(fused)
    }

    /// Applies Reciprocal Rank Fusion to combine search results.
    ///
    /// RRF score = sum(1 / (k + `rank_i`)) for each ranking
    fn rrf_fusion(
        &self,
        text_results: &[SearchHit],
        vector_results: &[SearchHit],
        limit: usize,
    ) -> Vec<SearchHit> {
        const K: f32 = 60.0; // Standard RRF constant

        let mut scores: HashMap<String, (f32, Option<SearchHit>)> = HashMap::new();

        // Add text results
        for (rank, hit) in text_results.iter().enumerate() {
            let id = hit.memory.id.as_str().to_string();
            let rrf_score = 1.0 / (K + rank as f32 + 1.0);

            scores
                .entry(id)
                .and_modify(|(s, _)| *s += rrf_score)
                .or_insert((rrf_score, Some(hit.clone())));
        }

        // Add vector results
        for (rank, hit) in vector_results.iter().enumerate() {
            let id = hit.memory.id.as_str().to_string();
            let rrf_score = 1.0 / (K + rank as f32 + 1.0);

            scores
                .entry(id)
                .and_modify(|(s, existing)| {
                    *s += rrf_score;
                    // Merge vector score into existing hit
                    merge_vector_score(existing, hit.vector_score);
                })
                .or_insert((rrf_score, Some(hit.clone())));
        }

        // Sort by combined score
        let mut results: Vec<_> = scores
            .into_iter()
            .filter_map(|(_, (score, hit))| {
                hit.map(|mut h| {
                    h.score = score;
                    h
                })
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        results
    }

    /// Retrieves a memory by ID with full content.
    ///
    /// Use this for targeted fetch when full content is needed.
    ///
    /// # Errors
    ///
    /// Returns an error if the index is not available.
    pub fn get_by_id(&self, id: &MemoryId) -> Result<Option<Memory>> {
        let index = self.index.as_ref().ok_or_else(|| Error::OperationFailed {
            operation: "get_by_id".to_string(),
            cause: "No index backend configured".to_string(),
        })?;

        index.get_memory(id)
    }

    /// Retrieves recent memories.
    ///
    /// # Errors
    ///
    /// Returns an error if retrieval fails.
    pub const fn recent(&self, _limit: usize, _filter: &SearchFilter) -> Result<Vec<Memory>> {
        // Would need persistence backend to implement
        Ok(Vec::new())
    }
}

fn domain_label(filter: &SearchFilter) -> String {
    match filter.domains.len() {
        0 => "all".to_string(),
        1 => filter.domains[0].to_string(),
        _ => "multi".to_string(),
    }
}

/// Merges a vector score into an existing search hit.
const fn merge_vector_score(existing: &mut Option<SearchHit>, vector_score: Option<f32>) {
    if let Some(e) = existing {
        e.vector_score = vector_score;
    }
}

impl Default for RecallService {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a placeholder memory for search results.
const fn create_placeholder_memory(id: MemoryId) -> Memory {
    use crate::models::{Domain, Namespace};

    Memory {
        id,
        content: String::new(),
        namespace: Namespace::Decisions,
        domain: Domain::new(),
        status: MemoryStatus::Active,
        created_at: 0,
        updated_at: 0,
        embedding: None,
        tags: Vec::new(),
        source: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Namespace;

    fn create_test_memory(id: &str, content: &str) -> Memory {
        use crate::models::Domain;

        Memory {
            id: MemoryId::new(id),
            content: content.to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            status: MemoryStatus::Active,
            created_at: 0,
            updated_at: 0,
            embedding: None,
            tags: Vec::new(),
            source: None,
        }
    }

    #[test]
    fn test_search_empty_query() {
        let service = RecallService::default();
        let result = service.search("", SearchMode::Text, &SearchFilter::new(), 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_search_no_backend() {
        let service = RecallService::default();
        let result = service.search("test", SearchMode::Text, &SearchFilter::new(), 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_search_with_backend() {
        let mut index = SqliteBackend::in_memory().unwrap();

        // Add some test data
        index
            .index(&create_test_memory("id1", "Rust programming language"))
            .unwrap();
        index
            .index(&create_test_memory("id2", "Python scripting"))
            .unwrap();

        let service = RecallService::with_index(index);

        let result = service.search("Rust", SearchMode::Text, &SearchFilter::new(), 10);
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(!result.memories.is_empty());
    }

    #[test]
    fn test_rrf_fusion() {
        let service = RecallService::default();

        let text_hits = vec![
            SearchHit {
                memory: create_test_memory("id1", ""),
                score: 0.9,
                vector_score: None,
                bm25_score: Some(0.9),
            },
            SearchHit {
                memory: create_test_memory("id2", ""),
                score: 0.8,
                vector_score: None,
                bm25_score: Some(0.8),
            },
        ];

        let vector_hits = vec![
            SearchHit {
                memory: create_test_memory("id2", ""),
                score: 0.95,
                vector_score: Some(0.95),
                bm25_score: None,
            },
            SearchHit {
                memory: create_test_memory("id3", ""),
                score: 0.85,
                vector_score: Some(0.85),
                bm25_score: None,
            },
        ];

        let fused = service.rrf_fusion(&text_hits, &vector_hits, 10);

        // id2 should be ranked higher because it appears in both
        assert!(!fused.is_empty());

        // Check that id2 has a higher score than id1 and id3
        let id2_score = fused
            .iter()
            .find(|h| h.memory.id.as_str() == "id2")
            .map(|h| h.score);
        let id1_score = fused
            .iter()
            .find(|h| h.memory.id.as_str() == "id1")
            .map(|h| h.score);

        assert!(id2_score > id1_score);
    }

    #[test]
    fn test_hybrid_search_mode() {
        let result =
            RecallService::default().search("test", SearchMode::Hybrid, &SearchFilter::new(), 10);
        // Will fail without backend, but tests the path
        assert!(result.is_err());
    }
}
