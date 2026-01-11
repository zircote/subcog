//! Reciprocal Rank Fusion (RRF) algorithm for hybrid search.
//!
//! RRF is a rank aggregation technique that combines ranked lists from multiple
//! retrieval systems without requiring score normalization.
//!
//! # Algorithm
//!
//! For each document `d` appearing in ranking `r`:
//!
//! ```text
//! RRF_score(d) = sum(1 / (k + rank_r(d)))
//! ```
//!
//! Where:
//! - `k` = 60 (standard constant, prevents division by zero and dampens high ranks)
//! - `rank_r(d)` = position of document `d` in ranking `r` (1-indexed)
//!
//! # References
//!
//! - Cormack, G. V., Clarke, C. L., & Buettcher, S. (2009). "Reciprocal Rank Fusion
//!   outperforms Condorcet and individual Rank Learning Methods"

use crate::models::SearchHit;
use std::collections::HashMap;

/// Configuration for RRF fusion.
#[derive(Debug, Clone)]
pub struct RrfConfig {
    /// The k constant for RRF scoring (default: 60).
    ///
    /// Higher values dampen the contribution of top-ranked documents.
    /// The standard value of 60 works well for most use cases.
    pub k: f32,

    /// Maximum number of results to return.
    pub limit: usize,
}

impl Default for RrfConfig {
    fn default() -> Self {
        Self { k: 60.0, limit: 10 }
    }
}

impl RrfConfig {
    /// Creates a new RRF configuration with the specified limit.
    #[must_use]
    pub const fn with_limit(limit: usize) -> Self {
        Self { k: 60.0, limit }
    }

    /// Sets the k constant.
    #[must_use]
    pub const fn with_k(mut self, k: f32) -> Self {
        self.k = k;
        self
    }
}

/// Reciprocal Rank Fusion combiner for hybrid search results.
///
/// Combines results from multiple ranking sources (e.g., BM25 text search
/// and vector similarity search) into a single ranked list.
///
/// # Example
///
/// ```ignore
/// use subcog::services::RrfFusion;
///
/// let fusion = RrfFusion::new();
/// let combined = fusion.fuse(&text_results, &vector_results, 10);
/// ```
#[derive(Debug, Clone, Default)]
pub struct RrfFusion {
    config: RrfConfig,
}

impl RrfFusion {
    /// Creates a new RRF fusion combiner with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new RRF fusion combiner with the specified configuration.
    #[must_use]
    pub const fn with_config(config: RrfConfig) -> Self {
        Self { config }
    }

    /// Creates a new RRF fusion combiner with the specified limit.
    #[must_use]
    pub const fn with_limit(limit: usize) -> Self {
        Self {
            config: RrfConfig::with_limit(limit),
        }
    }

    /// Fuses two ranked lists using Reciprocal Rank Fusion.
    ///
    /// # Arguments
    ///
    /// * `text_results` - Results from BM25/text search (ranked by BM25 score)
    /// * `vector_results` - Results from vector/semantic search (ranked by similarity)
    /// * `limit` - Maximum number of results to return (overrides config)
    ///
    /// # Returns
    ///
    /// Combined results ranked by RRF score, with the specified limit applied.
    ///
    /// # Algorithm
    ///
    /// ```text
    /// BM25 results:  [doc_A@1, doc_B@2, doc_C@3]
    /// Vector results: [doc_B@1, doc_C@2, doc_D@3]
    ///
    /// RRF scores (k=60):
    /// - doc_A: 1/(60+1) = 0.0164  (only in BM25)
    /// - doc_B: 1/(60+2) + 1/(60+1) = 0.0161 + 0.0164 = 0.0325  (in both!)
    /// - doc_C: 1/(60+3) + 1/(60+2) = 0.0159 + 0.0161 = 0.0320  (in both)
    /// - doc_D: 1/(60+3) = 0.0159  (only in vector)
    ///
    /// Final ranking: [doc_B, doc_C, doc_A, doc_D]
    /// ```
    #[must_use]
    pub fn fuse(
        &self,
        text_results: &[SearchHit],
        vector_results: &[SearchHit],
        limit: usize,
    ) -> Vec<SearchHit> {
        let k = self.config.k;

        // Pre-allocate HashMap with expected capacity
        let capacity = text_results.len() + vector_results.len();
        let mut scores: HashMap<&str, (f32, Option<SearchHit>)> = HashMap::with_capacity(capacity);

        // Add text results with RRF scores
        for (rank, hit) in text_results.iter().enumerate() {
            let id = hit.memory.id.as_str();
            #[allow(clippy::cast_precision_loss)]
            let rrf_score = 1.0 / (k + rank as f32 + 1.0);

            scores
                .entry(id)
                .and_modify(|(s, _)| *s += rrf_score)
                .or_insert((rrf_score, Some(hit.clone())));
        }

        // Add vector results with RRF scores
        for (rank, hit) in vector_results.iter().enumerate() {
            let id = hit.memory.id.as_str();
            #[allow(clippy::cast_precision_loss)]
            let rrf_score = 1.0 / (k + rank as f32 + 1.0);

            scores
                .entry(id)
                .and_modify(|(s, existing)| {
                    *s += rrf_score;
                    // Merge vector score into existing hit
                    if let Some(e) = existing {
                        e.vector_score = hit.vector_score;
                    }
                })
                .or_insert((rrf_score, Some(hit.clone())));
        }

        // Collect and sort by RRF score (descending)
        let mut results: Vec<_> = scores
            .into_iter()
            .filter_map(|(_, (score, hit))| {
                hit.map(|mut h| {
                    h.score = score;
                    h
                })
            })
            .collect();

        // Use unstable sort for better performance (stability not needed)
        results.sort_unstable_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        results
    }

    /// Fuses multiple ranked lists using Reciprocal Rank Fusion.
    ///
    /// This is a more general version that can combine any number of result lists.
    ///
    /// # Arguments
    ///
    /// * `result_lists` - Vector of result lists, each ranked by their respective scores
    ///
    /// # Returns
    ///
    /// Combined results ranked by RRF score.
    #[must_use]
    pub fn fuse_multiple(&self, result_lists: &[&[SearchHit]]) -> Vec<SearchHit> {
        let k = self.config.k;

        // Calculate total capacity
        let capacity: usize = result_lists.iter().map(|l| l.len()).sum();
        let mut scores: HashMap<&str, (f32, Option<SearchHit>)> = HashMap::with_capacity(capacity);

        // Process each result list
        for results in result_lists {
            for (rank, hit) in results.iter().enumerate() {
                let id = hit.memory.id.as_str();
                #[allow(clippy::cast_precision_loss)]
                let rrf_score = 1.0 / (k + rank as f32 + 1.0);

                scores
                    .entry(id)
                    .and_modify(|(s, _)| *s += rrf_score)
                    .or_insert((rrf_score, Some(hit.clone())));
            }
        }

        // Collect and sort
        let mut results: Vec<_> = scores
            .into_iter()
            .filter_map(|(_, (score, hit))| {
                hit.map(|mut h| {
                    h.score = score;
                    h
                })
            })
            .collect();

        results.sort_unstable_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(self.config.limit);

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, Memory, MemoryId, MemoryStatus, Namespace};

    fn create_test_memory(id: &str) -> Memory {
        Memory {
            id: MemoryId::new(id),
            content: String::new(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: 0,
            updated_at: 0,
            tombstoned_at: None,
            embedding: None,
            tags: Vec::new(),
            source: None,
            is_summary: false,
            source_memory_ids: None,
            consolidation_timestamp: None,
        }
    }

    fn create_hit(id: &str, score: f32) -> SearchHit {
        SearchHit {
            memory: create_test_memory(id),
            score,
            raw_score: score,
            vector_score: None,
            bm25_score: Some(score),
        }
    }

    fn create_vector_hit(id: &str, score: f32) -> SearchHit {
        SearchHit {
            memory: create_test_memory(id),
            score,
            raw_score: score,
            vector_score: Some(score),
            bm25_score: None,
        }
    }

    #[test]
    fn test_rrf_fusion_basic() {
        let fusion = RrfFusion::new();

        let text_hits = vec![create_hit("id1", 0.9), create_hit("id2", 0.8)];

        let vector_hits = vec![
            create_vector_hit("id2", 0.95),
            create_vector_hit("id3", 0.85),
        ];

        let fused = fusion.fuse(&text_hits, &vector_hits, 10);

        // id2 should be ranked highest (appears in both)
        assert!(!fused.is_empty());
        let id2_idx = fused
            .iter()
            .position(|h| h.memory.id.as_str() == "id2")
            .unwrap();
        let id1_idx = fused
            .iter()
            .position(|h| h.memory.id.as_str() == "id1")
            .unwrap();
        assert!(id2_idx < id1_idx, "id2 should rank higher than id1");
    }

    #[test]
    fn test_rrf_fusion_empty_lists() {
        let fusion = RrfFusion::new();

        let text_hits: Vec<SearchHit> = vec![];
        let vector_hits: Vec<SearchHit> = vec![];

        let fused = fusion.fuse(&text_hits, &vector_hits, 10);
        assert!(fused.is_empty());
    }

    #[test]
    fn test_rrf_fusion_single_list() {
        let fusion = RrfFusion::new();

        let text_hits = vec![create_hit("id1", 0.9), create_hit("id2", 0.8)];
        let vector_hits: Vec<SearchHit> = vec![];

        let fused = fusion.fuse(&text_hits, &vector_hits, 10);
        assert_eq!(fused.len(), 2);
    }

    #[test]
    fn test_rrf_fusion_limit() {
        let fusion = RrfFusion::new();

        let text_hits: Vec<SearchHit> =
            (0..10).map(|i| create_hit(&format!("t{i}"), 0.9)).collect();
        let vector_hits: Vec<SearchHit> = (0..10)
            .map(|i| create_vector_hit(&format!("v{i}"), 0.9))
            .collect();

        let fused = fusion.fuse(&text_hits, &vector_hits, 5);
        assert_eq!(fused.len(), 5);
    }

    #[test]
    fn test_rrf_config() {
        let config = RrfConfig::with_limit(20).with_k(30.0);
        assert_eq!(config.limit, 20);
        assert!((config.k - 30.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_fuse_multiple() {
        let fusion = RrfFusion::with_limit(10);

        let list1 = vec![create_hit("a", 0.9), create_hit("b", 0.8)];
        let list2 = vec![create_hit("b", 0.95), create_hit("c", 0.85)];
        let list3 = vec![create_hit("c", 0.92), create_hit("d", 0.82)];

        let fused = fusion.fuse_multiple(&[&list1, &list2, &list3]);

        // b and c appear in multiple lists, should rank high
        assert!(!fused.is_empty());
        // b appears in 2 lists, c appears in 2 lists
        let b_score = fused
            .iter()
            .find(|h| h.memory.id.as_str() == "b")
            .map(|h| h.score)
            .unwrap();
        let a_score = fused
            .iter()
            .find(|h| h.memory.id.as_str() == "a")
            .map(|h| h.score)
            .unwrap();
        assert!(
            b_score > a_score,
            "b (in 2 lists) should score higher than a (in 1 list)"
        );
    }

    #[test]
    fn test_vector_score_preserved() {
        let fusion = RrfFusion::new();

        let text_hits = vec![create_hit("id1", 0.9)];
        let vector_hits = vec![create_vector_hit("id1", 0.95)];

        let fused = fusion.fuse(&text_hits, &vector_hits, 10);
        assert_eq!(fused.len(), 1);
        // Vector score should be merged in
        assert!(fused[0].vector_score.is_some());
    }
}
