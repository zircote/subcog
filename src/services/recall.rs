//! Memory recall (search) service.
//!
//! Searches for memories using hybrid (vector + BM25) search with RRF fusion.

use crate::current_timestamp;
use crate::embedding::Embedder;
use crate::gc::branch_exists;
use crate::models::{
    Memory, MemoryEvent, MemoryId, MemoryStatus, SearchFilter, SearchHit, SearchMode, SearchResult,
    TombstoneHint,
};
use crate::security::record_event;
use crate::storage::index::SqliteBackend;
use crate::storage::traits::{IndexBackend, VectorBackend};
use crate::{Error, Result};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use tracing::instrument;

/// Threshold for sparse results that triggers tombstone hint lookup.
/// When active results are below this count, we check for tombstoned memories.
const SPARSE_RESULT_THRESHOLD: usize = 3;

/// Maximum number of branches to include in the tombstone hint.
const MAX_HINT_BRANCHES: usize = 5;

/// Service for searching and retrieving memories.
///
/// Supports three search modes:
/// - **Text**: BM25 full-text search via `SQLite` FTS5
/// - **Vector**: Semantic similarity search via embedding + vector backend
/// - **Hybrid**: Combines both using Reciprocal Rank Fusion (RRF)
///
/// # Facet Filtering
///
/// The service supports filtering by project-level facets:
/// - `project_id`: Filter by project identifier
/// - `branch`: Filter by git branch name
/// - `file_path_pattern`: Filter by file path (glob-style patterns)
///
/// # Tombstone Handling
///
/// By default, tombstoned (soft-deleted) memories are excluded from results.
/// Set `include_tombstoned: true` in the filter for audit/recovery purposes.
///
/// When search results are sparse (< 3 active results), the service automatically
/// checks if tombstoned memories exist that match the query and provides a hint
/// to the caller about their existence and associated branches.
///
/// # Graceful Degradation
///
/// If embedder or vector backend is unavailable:
/// - `SearchMode::Vector` falls back to empty results with a warning
/// - `SearchMode::Hybrid` falls back to text-only search
/// - No errors are raised; partial results are returned
pub struct RecallService {
    /// `SQLite` index backend for BM25 text search.
    index: Option<SqliteBackend>,
    /// Embedder for generating query embeddings (optional).
    embedder: Option<Arc<dyn Embedder>>,
    /// Vector backend for similarity search (optional).
    vector: Option<Arc<dyn VectorBackend + Send + Sync>>,
}

impl RecallService {
    /// Creates a new recall service without any backends.
    ///
    /// This is primarily for testing. Use [`with_index`](Self::with_index) or
    /// [`with_backends`](Self::with_backends) for production use.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            index: None,
            embedder: None,
            vector: None,
        }
    }

    /// Creates a recall service with an index backend (text search only).
    ///
    /// Vector search will be disabled; hybrid search falls back to text-only.
    #[must_use]
    pub const fn with_index(index: SqliteBackend) -> Self {
        Self {
            index: Some(index),
            embedder: None,
            vector: None,
        }
    }

    /// Creates a recall service with full hybrid search support.
    ///
    /// # Arguments
    ///
    /// * `index` - `SQLite` index backend for BM25 text search
    /// * `embedder` - Embedder for generating query embeddings
    /// * `vector` - Vector backend for similarity search
    #[must_use]
    pub fn with_backends(
        index: SqliteBackend,
        embedder: Arc<dyn Embedder>,
        vector: Arc<dyn VectorBackend + Send + Sync>,
    ) -> Self {
        Self {
            index: Some(index),
            embedder: Some(embedder),
            vector: Some(vector),
        }
    }

    /// Adds an embedder to an existing recall service.
    #[must_use]
    pub fn with_embedder(mut self, embedder: Arc<dyn Embedder>) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// Adds a vector backend to an existing recall service.
    #[must_use]
    pub fn with_vector(mut self, vector: Arc<dyn VectorBackend + Send + Sync>) -> Self {
        self.vector = Some(vector);
        self
    }

    /// Returns whether vector search is available.
    #[must_use]
    pub fn has_vector_search(&self) -> bool {
        self.embedder.is_some() && self.vector.is_some()
    }

    /// Searches for memories matching a query.
    ///
    /// # Facet Filtering
    ///
    /// The filter supports project-level facets for scoped searches:
    /// - `project_id`: Limit results to a specific project
    /// - `branch`: Limit results to a specific git branch
    /// - `file_path_pattern`: Limit results to files matching a glob pattern
    ///
    /// # Tombstone Handling
    ///
    /// By default, tombstoned memories are excluded. To include them
    /// (e.g., for audit or recovery), set `filter.include_tombstoned = true`.
    ///
    /// When active results are sparse (< 3), the service checks for tombstoned
    /// memories that match the query and populates `tombstone_hint` with their
    /// count and associated branch names.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidInput`] if:
    /// - The query is empty or contains only whitespace
    ///
    /// Returns [`Error::OperationFailed`] if:
    /// - No index backend is configured (for `Text` and `Hybrid` modes)
    /// - The index backend search operation fails
    #[allow(clippy::cast_possible_truncation)]
    #[instrument(
        skip(self, query, filter),
        fields(
            operation = "recall",
            mode = %mode,
            query_length = query.len(),
            limit = limit,
            project_id = filter.project_id.as_deref(),
            branch = filter.branch.as_deref(),
            include_tombstoned = filter.include_tombstoned
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

        tracing::info!(
            mode = %mode_label,
            query_length = query.len(),
            limit = limit,
            project_id = ?filter.project_id,
            branch = ?filter.branch,
            file_path_pattern = ?filter.file_path_pattern,
            include_tombstoned = filter.include_tombstoned,
            "Searching memories with facet filters"
        );

        let result = (|| {
            // Validate query
            if query.trim().is_empty() {
                return Err(Error::InvalidInput("Query cannot be empty".to_string()));
            }

            // Lazy GC: Check if the filtered branch still exists
            // This provides early feedback without slowing down searches
            let branch_stale = if let Some(ref branch) = filter.branch {
                let exists = branch_exists(branch);
                if !exists {
                    tracing::warn!(
                        branch = %branch,
                        "Search on stale branch - branch no longer exists in repository"
                    );
                    metrics::counter!(
                        "memory_search_stale_branch_total",
                        "branch" => branch.clone()
                    )
                    .increment(1);
                }
                !exists
            } else {
                false
            };

            let mut memories = match mode {
                SearchMode::Text => self.text_search(query, filter, limit)?,
                SearchMode::Vector => self.vector_search(query, filter, limit)?,
                SearchMode::Hybrid => self.hybrid_search(query, filter, limit)?,
            };

            // Normalize scores to 0.0-1.0 range for Text and Vector modes
            // (Hybrid mode already normalizes after RRF fusion)
            if mode != SearchMode::Hybrid {
                normalize_scores(&mut memories);
            }

            // Check for tombstoned memories if results are sparse and tombstones weren't requested
            // Also provide a hint if searching on a stale branch
            let tombstone_hint =
                if memories.len() < SPARSE_RESULT_THRESHOLD && !filter.include_tombstoned {
                    self.check_for_tombstones(query, mode, filter, limit)
                } else if branch_stale {
                    // Branch is stale - provide a hint even if we have results
                    // The memories found might be from before the branch was deleted
                    let branch_name = filter.branch.clone().unwrap_or_default();
                    Some(TombstoneHint::new(memories.len(), vec![branch_name]))
                } else {
                    None
                };

            // Safe cast: u128 milliseconds will practically never exceed u64::MAX
            let execution_time_ms = start.elapsed().as_millis() as u64;
            let total_count = memories.len();
            let timestamp = current_timestamp();
            // Use Arc<str> for zero-copy sharing across events (PERF-C1).
            // Arc::clone() is O(1) atomic increment vs O(n) String::clone().
            let query_arc: std::sync::Arc<str> = query.into();
            for hit in &memories {
                record_event(MemoryEvent::Retrieved {
                    memory_id: hit.memory.id.clone(),
                    query: std::sync::Arc::clone(&query_arc),
                    score: hit.score,
                    timestamp,
                });
            }

            Ok(SearchResult {
                memories,
                total_count,
                mode,
                execution_time_ms,
                tombstone_hint,
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

    /// Checks for tombstoned memories that match the query.
    ///
    /// Called when active search results are sparse to provide visibility
    /// into potentially relevant deleted content.
    ///
    /// Returns `None` if:
    /// - No index backend is configured
    /// - No tombstoned memories match the query
    /// - An error occurs during the check (fails silently)
    fn check_for_tombstones(
        &self,
        query: &str,
        mode: SearchMode,
        filter: &SearchFilter,
        limit: usize,
    ) -> Option<TombstoneHint> {
        // Create a filter that includes tombstoned memories
        let tombstone_filter = SearchFilter {
            include_tombstoned: true,
            ..filter.clone()
        };

        // Search with tombstones included
        let tombstone_results = match mode {
            SearchMode::Text => self.text_search(query, &tombstone_filter, limit),
            SearchMode::Vector => self.vector_search(query, &tombstone_filter, limit),
            SearchMode::Hybrid => self.hybrid_search(query, &tombstone_filter, limit),
        };

        let Ok(all_results) = tombstone_results else {
            tracing::debug!("Tombstone check failed, skipping hint");
            return None;
        };

        // Filter to only tombstoned memories (those with tombstoned_at set)
        let tombstoned: Vec<_> = all_results
            .into_iter()
            .filter(|hit| hit.memory.tombstoned_at.is_some())
            .collect();

        if tombstoned.is_empty() {
            return None;
        }

        // Collect unique branch names from tombstoned memories
        let mut branches: HashSet<String> = HashSet::new();
        for hit in &tombstoned {
            if let Some(ref branch) = hit.memory.branch {
                branches.insert(branch.clone());
            }
        }

        // Convert to sorted vec and limit to MAX_HINT_BRANCHES
        let mut branch_list: Vec<String> = branches.into_iter().collect();
        branch_list.sort();
        branch_list.truncate(MAX_HINT_BRANCHES);

        let hint = TombstoneHint::new(tombstoned.len(), branch_list);

        tracing::debug!(
            tombstone_count = hint.count,
            branches = ?hint.branches,
            "Found tombstoned memories matching query"
        );

        metrics::counter!(
            "memory_search_tombstone_hint_total",
            "has_hint" => "true"
        )
        .increment(1);

        Some(hint)
    }

    /// Lists all memories, optionally filtered by namespace and facets.
    ///
    /// Unlike `search`, this doesn't require a query and returns all matching memories.
    /// Returns minimal metadata (id, namespace) without content - details via drill-down.
    ///
    /// # Facet Filtering
    ///
    /// Supports the same facet filters as `search()`:
    /// - `project_id`: Limit results to a specific project
    /// - `branch`: Limit results to a specific git branch
    /// - `file_path_pattern`: Limit results to files matching a glob pattern
    ///
    /// # Tombstone Handling
    ///
    /// By default, tombstoned memories are excluded from listing.
    /// Set `filter.include_tombstoned = true` for audit/recovery operations.
    ///
    /// # Errors
    ///
    /// Returns [`Error::OperationFailed`] if:
    /// - No index backend is configured
    /// - The index backend list operation fails
    /// - Batch memory retrieval fails
    #[allow(clippy::cast_possible_truncation)]
    #[instrument(
        skip(self, filter),
        fields(
            operation = "list_all",
            limit = limit,
            project_id = filter.project_id.as_deref(),
            branch = filter.branch.as_deref(),
            include_tombstoned = filter.include_tombstoned
        )
    )]
    pub fn list_all(&self, filter: &SearchFilter, limit: usize) -> Result<SearchResult> {
        let start = Instant::now();
        let domain_label = domain_label(filter);

        tracing::debug!(
            project_id = ?filter.project_id,
            branch = ?filter.branch,
            file_path_pattern = ?filter.file_path_pattern,
            include_tombstoned = filter.include_tombstoned,
            "Listing memories with facet filters"
        );

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
                            raw_score: score,
                            vector_score: None,
                            bm25_score: None,
                        }
                    })
                })
                .collect();

            let execution_time_ms = start.elapsed().as_millis() as u64;
            let total_count = memories.len();
            let timestamp = current_timestamp();
            // Use Arc<str> for zero-copy sharing (PERF-C1). Static pattern for list_all.
            let query_arc: std::sync::Arc<str> = "*".into();
            for hit in &memories {
                record_event(MemoryEvent::Retrieved {
                    memory_id: hit.memory.id.clone(),
                    query: std::sync::Arc::clone(&query_arc),
                    score: hit.score,
                    timestamp,
                });
            }

            Ok(SearchResult {
                memories,
                total_count,
                mode: SearchMode::Text,
                execution_time_ms,
                tombstone_hint: None,
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

    /// Searches for memories within a specific project scope.
    ///
    /// Convenience method that creates a filter with the `project_id` preset.
    ///
    /// # Errors
    ///
    /// Returns errors as per [`search()`](Self::search).
    pub fn search_in_project(
        &self,
        query: &str,
        project_id: &str,
        mode: SearchMode,
        limit: usize,
    ) -> Result<SearchResult> {
        let filter = SearchFilter::new().with_project_id(project_id);
        self.search(query, mode, &filter, limit)
    }

    /// Searches for memories on a specific git branch.
    ///
    /// Convenience method that creates a filter with the branch preset.
    ///
    /// # Errors
    ///
    /// Returns errors as per [`search()`](Self::search).
    pub fn search_on_branch(
        &self,
        query: &str,
        branch: &str,
        mode: SearchMode,
        limit: usize,
    ) -> Result<SearchResult> {
        let filter = SearchFilter::new().with_branch(branch);
        self.search(query, mode, &filter, limit)
    }

    /// Searches for memories related to files matching a pattern.
    ///
    /// Convenience method that creates a filter with a file path pattern.
    /// Supports glob patterns like `src/**/*.rs` or `tests/*.rs`.
    ///
    /// # Errors
    ///
    /// Returns errors as per [`search()`](Self::search).
    pub fn search_by_file_pattern(
        &self,
        query: &str,
        file_pattern: &str,
        mode: SearchMode,
        limit: usize,
    ) -> Result<SearchResult> {
        let filter = SearchFilter::new().with_file_path_pattern(file_pattern);
        self.search(query, mode, &filter, limit)
    }

    /// Searches including tombstoned (soft-deleted) memories.
    ///
    /// Use for audit trails or recovery operations.
    ///
    /// # Errors
    ///
    /// Returns errors as per [`search()`](Self::search).
    pub fn search_with_tombstoned(
        &self,
        query: &str,
        mode: SearchMode,
        limit: usize,
    ) -> Result<SearchResult> {
        let filter = SearchFilter::new().with_include_tombstoned(true);
        self.search(query, mode, &filter, limit)
    }

    /// Performs BM25 text search.
    ///
    /// Note: Scores are NOT normalized here. Normalization is applied:
    /// - In `hybrid_search` after RRF fusion
    /// - The caller is responsible for normalization in text-only mode
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
                    raw_score: score,
                    vector_score: None,
                    bm25_score: Some(score),
                }
            })
            .collect();

        Ok(hits)
    }

    /// Performs vector similarity search.
    ///
    /// # Graceful Degradation
    ///
    /// Returns empty results (not an error) if:
    /// - Embedder is not configured
    /// - Vector backend is not configured
    /// - Embedding generation fails
    /// - Vector search fails
    ///
    /// This allows hybrid search to fall back to text-only.
    fn vector_search(
        &self,
        query: &str,
        filter: &SearchFilter,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        // Check if we have the required components
        let (embedder, vector) = match (&self.embedder, &self.vector) {
            (Some(e), Some(v)) => (e, v),
            (None, _) => {
                tracing::debug!("Vector search unavailable: no embedder configured");
                return Ok(Vec::new());
            },
            (_, None) => {
                tracing::debug!("Vector search unavailable: no vector backend configured");
                return Ok(Vec::new());
            },
        };

        // Generate query embedding
        let query_embedding = match embedder.embed(query) {
            Ok(emb) => emb,
            Err(e) => {
                tracing::warn!("Failed to embed query for vector search: {e}");
                return Ok(Vec::new());
            },
        };

        // Search vector backend (convert SearchFilter to VectorFilter)
        let vector_filter = crate::storage::traits::VectorFilter::from(filter);
        let results = match vector.search(&query_embedding, &vector_filter, limit) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Vector search failed: {e}");
                return Ok(Vec::new());
            },
        };

        // Get index backend to retrieve full memories
        let index = match &self.index {
            Some(idx) => idx,
            None => {
                // Return results with placeholder memories if no index
                return Ok(results
                    .into_iter()
                    .map(|(id, score)| SearchHit {
                        memory: create_placeholder_memory(id),
                        score,
                        raw_score: score,
                        vector_score: Some(score),
                        bm25_score: None,
                    })
                    .collect());
            },
        };

        // PERF: Batch fetch memories for vector results
        let ids: Vec<_> = results.iter().map(|(id, _)| id.clone()).collect();
        let batch_memories = match index.get_memories_batch(&ids) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("Failed to fetch memories for vector results: {e}");
                // Return with placeholder memories
                return Ok(results
                    .into_iter()
                    .map(|(id, score)| SearchHit {
                        memory: create_placeholder_memory(id),
                        score,
                        raw_score: score,
                        vector_score: Some(score),
                        bm25_score: None,
                    })
                    .collect());
            },
        };

        // Convert to SearchHits
        let hits: Vec<SearchHit> = results
            .into_iter()
            .zip(batch_memories)
            .map(|((id, score), memory_opt)| {
                let memory = memory_opt.unwrap_or_else(|| create_placeholder_memory(id));
                SearchHit {
                    memory,
                    score,
                    raw_score: score,
                    vector_score: Some(score),
                    bm25_score: None,
                }
            })
            .collect();

        Ok(hits)
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
        let mut fused = self.rrf_fusion(&text_results, &vector_results, limit);

        // Normalize scores to 0.0-1.0 range
        normalize_scores(&mut fused);

        Ok(fused)
    }

    /// Applies Reciprocal Rank Fusion (RRF) to combine search results.
    ///
    /// # Algorithm
    ///
    /// RRF is a rank aggregation technique that combines ranked lists from multiple
    /// retrieval systems. For each document `d` appearing in ranking `r`:
    ///
    /// ```text
    /// RRF_score(d) = Σ 1 / (k + rank_r(d))
    /// ```
    ///
    /// Where:
    /// - `k` = 60 (standard constant, prevents division by zero and dampens high ranks)
    /// - `rank_r(d)` = position of document `d` in ranking `r` (1-indexed)
    ///
    /// # Why RRF?
    ///
    /// - **Score normalization**: Raw scores from different retrievers (BM25 vs cosine)
    ///   are not comparable. RRF uses ranks, which are always comparable.
    /// - **Robust fusion**: Documents ranked highly in multiple systems get boosted.
    /// - **Simple and effective**: No hyperparameter tuning needed (k=60 works well).
    ///
    /// # Example
    ///
    /// ```text
    /// BM25 results:  [doc_A@1, doc_B@2, doc_C@3]
    /// Vector results: [doc_B@1, doc_C@2, doc_D@3]
    ///
    /// RRF scores:
    /// - doc_A: 1/(60+1) = 0.0164  (only in BM25)
    /// - doc_B: 1/(60+2) + 1/(60+1) = 0.0161 + 0.0164 = 0.0325  (in both!)
    /// - doc_C: 1/(60+3) + 1/(60+2) = 0.0159 + 0.0161 = 0.0320  (in both)
    /// - doc_D: 1/(60+3) = 0.0159  (only in vector)
    ///
    /// Final ranking: [doc_B, doc_C, doc_A, doc_D]
    /// ```
    ///
    /// # References
    ///
    /// - Cormack, G. V., Clarke, C. L., & Buettcher, S. (2009). "Reciprocal Rank Fusion
    ///   outperforms Condorcet and individual Rank Learning Methods"
    fn rrf_fusion(
        &self,
        text_results: &[SearchHit],
        vector_results: &[SearchHit],
        limit: usize,
    ) -> Vec<SearchHit> {
        const K: f32 = 60.0; // Standard RRF constant

        // Pre-allocate HashMap with expected capacity (PERF-M1)
        // Max unique results = text_results + vector_results (when no overlap)
        let capacity = text_results.len() + vector_results.len();
        let mut scores: HashMap<String, (f32, Option<SearchHit>)> =
            HashMap::with_capacity(capacity);

        // Add text results
        for (rank, hit) in text_results.iter().enumerate() {
            let id = hit.memory.id.to_string();
            let rrf_score = 1.0 / (K + rank as f32 + 1.0);

            scores
                .entry(id)
                .and_modify(|(s, _)| *s += rrf_score)
                .or_insert((rrf_score, Some(hit.clone())));
        }

        // Add vector results
        for (rank, hit) in vector_results.iter().enumerate() {
            let id = hit.memory.id.to_string();
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
    /// Returns [`Error::OperationFailed`] if:
    /// - No index backend is configured
    /// - The index backend get operation fails
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
    /// Returns [`Error::OperationFailed`] if:
    /// - No persistence backend is configured
    /// - The persistence backend retrieval fails
    ///
    /// # Note
    ///
    /// Currently returns empty results as persistence backend integration is pending.
    pub const fn recent(&self, _limit: usize, _filter: &SearchFilter) -> Result<Vec<Memory>> {
        // Would need persistence backend to implement
        Ok(Vec::new())
    }
}

/// Returns a domain label for metrics, avoiding allocations for common cases.
fn domain_label(filter: &SearchFilter) -> Cow<'static, str> {
    match filter.domains.len() {
        0 => Cow::Borrowed("all"),
        1 => Cow::Owned(filter.domains[0].to_string()),
        _ => Cow::Borrowed("multi"),
    }
}

/// Merges a vector score into an existing search hit.
const fn merge_vector_score(existing: &mut Option<SearchHit>, vector_score: Option<f32>) {
    if let Some(e) = existing {
        e.vector_score = vector_score;
    }
}

/// Normalizes search result scores to the 0.0-1.0 range.
///
/// # Algorithm
///
/// The maximum score in the result set becomes 1.0, and all other scores
/// are scaled proportionally. This ensures:
///
/// - Scores are always in [0.0, 1.0] range
/// - The top result always has score 1.0 (if results exist)
/// - Relative score proportions are preserved
///
/// # Arguments
///
/// * `results` - Mutable slice of search hits to normalize
///
/// # Notes
///
/// - Empty results are handled safely (no division by zero)
/// - Sets `raw_score` to preserve the original score before normalization
/// - If all scores are 0, they remain 0 after normalization
///
/// # Example
///
/// ```text
/// Before: [0.033, 0.020, 0.016]  (RRF scores)
/// After:  [1.0,   0.606, 0.485]  (normalized)
/// ```
fn normalize_scores(results: &mut [SearchHit]) {
    if results.is_empty() {
        return;
    }

    // Find the maximum score
    let max_score = results.iter().map(|h| h.score).fold(0.0_f32, f32::max);

    // Avoid division by zero
    if max_score <= f32::EPSILON {
        return;
    }

    // Normalize all scores
    for hit in results {
        // Store raw score before normalization
        hit.raw_score = hit.score;
        // Normalize to 0.0-1.0 range
        hit.score /= max_score;
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
        project_id: None,
        branch: None,
        file_path: None,
        tombstoned_at: None,
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
            project_id: None,
            branch: None,
            file_path: None,
            tombstoned_at: None,
        }
    }

    fn create_test_memory_with_facets(
        id: &str,
        content: &str,
        project_id: Option<&str>,
        branch: Option<&str>,
        file_path: Option<&str>,
    ) -> Memory {
        use crate::models::Domain;

        Memory {
            id: MemoryId::new(id),
            content: content.to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            status: MemoryStatus::Active,
            created_at: 1_000_000,
            updated_at: 1_000_000,
            embedding: None,
            tags: Vec::new(),
            source: None,
            project_id: project_id.map(String::from),
            branch: branch.map(String::from),
            file_path: file_path.map(String::from),
            tombstoned_at: None,
        }
    }

    fn create_tombstoned_memory(id: &str, content: &str) -> Memory {
        use crate::models::Domain;

        Memory {
            id: MemoryId::new(id),
            content: content.to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            status: MemoryStatus::Active,
            created_at: 1_000_000,
            updated_at: 1_000_000,
            embedding: None,
            tags: Vec::new(),
            source: None,
            project_id: None,
            branch: None,
            file_path: None,
            tombstoned_at: Some(2_000_000),
        }
    }

    fn create_tombstoned_memory_with_branch(id: &str, content: &str, branch: &str) -> Memory {
        use crate::models::Domain;

        Memory {
            id: MemoryId::new(id),
            content: content.to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            status: MemoryStatus::Active,
            created_at: 1_000_000,
            updated_at: 1_000_000,
            embedding: None,
            tags: Vec::new(),
            source: None,
            project_id: None,
            branch: Some(branch.to_string()),
            file_path: None,
            tombstoned_at: Some(2_000_000),
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
        let index = SqliteBackend::in_memory().unwrap();

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
                raw_score: 0.9,
                vector_score: None,
                bm25_score: Some(0.9),
            },
            SearchHit {
                memory: create_test_memory("id2", ""),
                score: 0.8,
                raw_score: 0.8,
                vector_score: None,
                bm25_score: Some(0.8),
            },
        ];

        let vector_hits = vec![
            SearchHit {
                memory: create_test_memory("id2", ""),
                score: 0.95,
                raw_score: 0.95,
                vector_score: Some(0.95),
                bm25_score: None,
            },
            SearchHit {
                memory: create_test_memory("id3", ""),
                score: 0.85,
                raw_score: 0.85,
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

    #[test]
    fn test_vector_search_no_embedder() {
        let service = RecallService::default();
        let result = service.vector_search("test query", &SearchFilter::new(), 10);

        // Should return empty, not error (graceful degradation)
        assert!(result.is_ok());
        assert!(result.expect("vector_search failed").is_empty());
    }

    #[test]
    fn test_vector_search_no_vector_backend() {
        use crate::embedding::FastEmbedEmbedder;

        let embedder: Arc<dyn Embedder> = Arc::new(FastEmbedEmbedder::new());
        let service = RecallService::new().with_embedder(embedder);

        let result = service.vector_search("test query", &SearchFilter::new(), 10);

        // Should return empty, not error (graceful degradation)
        assert!(result.is_ok());
        assert!(result.expect("vector_search failed").is_empty());
    }

    #[test]
    fn test_has_vector_search() {
        use crate::embedding::FastEmbedEmbedder;

        let service = RecallService::default();
        assert!(!service.has_vector_search());

        let embedder: Arc<dyn Embedder> = Arc::new(FastEmbedEmbedder::new());
        let service_with_embedder = RecallService::new().with_embedder(embedder);
        assert!(!service_with_embedder.has_vector_search());
    }

    #[test]
    fn test_with_backends_builder() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");
        let service = RecallService::with_index(index);

        // Test that index is configured but not embedder/vector
        assert!(!service.has_vector_search());
    }

    #[test]
    fn test_hybrid_search_fallback_text_only() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");

        // Add test data
        index
            .index(&create_test_memory("id1", "Rust programming language"))
            .expect("index failed");

        // Service with index but no embedder/vector
        let service = RecallService::with_index(index);

        // Hybrid search should fall back to text-only
        let result = service.search("Rust", SearchMode::Hybrid, &SearchFilter::new(), 10);
        assert!(result.is_ok());

        let search_result = result.expect("search failed");
        // Should get text results even though vector is unavailable
        assert!(!search_result.memories.is_empty());
    }

    #[test]
    fn test_vector_search_mode_graceful() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");
        let service = RecallService::with_index(index);

        // Vector search mode without embedder should return empty (graceful)
        let result = service.search("test", SearchMode::Vector, &SearchFilter::new(), 10);
        assert!(result.is_ok());

        let search_result = result.expect("search failed");
        assert!(search_result.memories.is_empty());
    }

    #[test]
    fn test_rrf_with_empty_vector_results() {
        let service = RecallService::default();

        let text_hits = vec![SearchHit {
            memory: create_test_memory("id1", "content"),
            score: 0.9,
            raw_score: 0.9,
            vector_score: None,
            bm25_score: Some(0.9),
        }];
        let vector_hits: Vec<SearchHit> = vec![]; // Empty vector results

        let fused = service.rrf_fusion(&text_hits, &vector_hits, 10);

        // Should still return text results
        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].memory.id.as_str(), "id1");
    }

    #[test]
    fn test_rrf_with_empty_text_results() {
        let service = RecallService::default();

        let text_hits: Vec<SearchHit> = vec![]; // Empty text results
        let vector_hits = vec![SearchHit {
            memory: create_test_memory("id1", "content"),
            score: 0.9,
            raw_score: 0.9,
            vector_score: Some(0.9),
            bm25_score: None,
        }];

        let fused = service.rrf_fusion(&text_hits, &vector_hits, 10);

        // Should still return vector results
        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].memory.id.as_str(), "id1");
    }

    #[test]
    fn test_domain_label() {
        let filter = SearchFilter::new();
        assert_eq!(domain_label(&filter), "all");
    }

    // ========================================================================
    // Facet Filtering Tests (Issue #43 - Storage Simplification)
    // ========================================================================

    #[test]
    fn test_search_filter_by_project_id() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");

        // Create memories in different projects
        let mem1 = create_test_memory_with_facets(
            "proj1-mem1",
            "Rust in project alpha",
            Some("project-alpha"),
            None,
            None,
        );
        let mem2 = create_test_memory_with_facets(
            "proj2-mem1",
            "Rust in project beta",
            Some("project-beta"),
            None,
            None,
        );
        let mem3 =
            create_test_memory_with_facets("no-proj-mem", "Rust without project", None, None, None);

        index.index(&mem1).unwrap();
        index.index(&mem2).unwrap();
        index.index(&mem3).unwrap();

        let service = RecallService::with_index(index);

        // Search within project-alpha
        let filter = SearchFilter::new().with_project_id("project-alpha");
        let result = service
            .search("Rust", SearchMode::Text, &filter, 10)
            .unwrap();

        assert_eq!(result.memories.len(), 1);
        assert_eq!(result.memories[0].memory.id.as_str(), "proj1-mem1");
    }

    #[test]
    fn test_search_filter_by_branch() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");

        // Create memories on different branches
        let mem1 = create_test_memory_with_facets(
            "main-mem",
            "Rust on main branch",
            None,
            Some("main"),
            None,
        );
        let mem2 = create_test_memory_with_facets(
            "feature-mem",
            "Rust on feature branch",
            None,
            Some("feature/auth"),
            None,
        );
        let mem3 = create_test_memory_with_facets(
            "no-branch-mem",
            "Rust without branch",
            None,
            None,
            None,
        );

        index.index(&mem1).unwrap();
        index.index(&mem2).unwrap();
        index.index(&mem3).unwrap();

        let service = RecallService::with_index(index);

        // Search on feature branch
        let filter = SearchFilter::new().with_branch("feature/auth");
        let result = service
            .search("Rust", SearchMode::Text, &filter, 10)
            .unwrap();

        assert_eq!(result.memories.len(), 1);
        assert_eq!(result.memories[0].memory.id.as_str(), "feature-mem");
    }

    #[test]
    fn test_search_filter_by_file_path_pattern() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");

        // Create memories with different file paths
        let mem1 = create_test_memory_with_facets(
            "src-mem",
            "Rust source file",
            None,
            None,
            Some("src/lib.rs"),
        );
        let mem2 = create_test_memory_with_facets(
            "test-mem",
            "Rust test file",
            None,
            None,
            Some("tests/integration.rs"),
        );
        let mem3 = create_test_memory_with_facets(
            "doc-mem",
            "Rust documentation",
            None,
            None,
            Some("docs/README.md"),
        );

        index.index(&mem1).unwrap();
        index.index(&mem2).unwrap();
        index.index(&mem3).unwrap();

        let service = RecallService::with_index(index);

        // Search for .rs files using glob pattern
        let filter = SearchFilter::new().with_file_path_pattern("%.rs");
        let result = service
            .search("Rust", SearchMode::Text, &filter, 10)
            .unwrap();

        assert_eq!(result.memories.len(), 2);
        let ids: Vec<_> = result
            .memories
            .iter()
            .map(|h| h.memory.id.as_str())
            .collect();
        assert!(ids.contains(&"src-mem"));
        assert!(ids.contains(&"test-mem"));
    }

    #[test]
    fn test_search_excludes_tombstoned_by_default() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");

        // Create active and tombstoned memories
        let active = create_test_memory("active-mem", "Rust active memory");
        let tombstoned = create_tombstoned_memory("tombstoned-mem", "Rust tombstoned memory");

        index.index(&active).unwrap();
        index.index(&tombstoned).unwrap();

        let service = RecallService::with_index(index);

        // Default search should exclude tombstoned
        let result = service
            .search("Rust", SearchMode::Text, &SearchFilter::new(), 10)
            .unwrap();

        assert_eq!(result.memories.len(), 1);
        assert_eq!(result.memories[0].memory.id.as_str(), "active-mem");
    }

    #[test]
    fn test_search_includes_tombstoned_when_requested() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");

        // Create active and tombstoned memories
        let active = create_test_memory("active-mem", "Rust active memory");
        let tombstoned = create_tombstoned_memory("tombstoned-mem", "Rust tombstoned memory");

        index.index(&active).unwrap();
        index.index(&tombstoned).unwrap();

        let service = RecallService::with_index(index);

        // Search with include_tombstoned = true
        let filter = SearchFilter::new().with_include_tombstoned(true);
        let result = service
            .search("Rust", SearchMode::Text, &filter, 10)
            .unwrap();

        assert_eq!(result.memories.len(), 2);
        let ids: Vec<_> = result
            .memories
            .iter()
            .map(|h| h.memory.id.as_str())
            .collect();
        assert!(ids.contains(&"active-mem"));
        assert!(ids.contains(&"tombstoned-mem"));
    }

    #[test]
    fn test_list_all_excludes_tombstoned_by_default() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");

        let active = create_test_memory("active-mem", "Active memory");
        let tombstoned = create_tombstoned_memory("tombstoned-mem", "Tombstoned memory");

        index.index(&active).unwrap();
        index.index(&tombstoned).unwrap();

        let service = RecallService::with_index(index);

        // Default list_all should exclude tombstoned
        let result = service.list_all(&SearchFilter::new(), 10).unwrap();

        assert_eq!(result.memories.len(), 1);
        assert_eq!(result.memories[0].memory.id.as_str(), "active-mem");
    }

    #[test]
    fn test_list_all_with_project_filter() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");

        let mem1 = create_test_memory_with_facets(
            "proj1-mem",
            "Memory in project 1",
            Some("project-1"),
            None,
            None,
        );
        let mem2 = create_test_memory_with_facets(
            "proj2-mem",
            "Memory in project 2",
            Some("project-2"),
            None,
            None,
        );

        index.index(&mem1).unwrap();
        index.index(&mem2).unwrap();

        let service = RecallService::with_index(index);

        let filter = SearchFilter::new().with_project_id("project-1");
        let result = service.list_all(&filter, 10).unwrap();

        assert_eq!(result.memories.len(), 1);
        assert_eq!(result.memories[0].memory.id.as_str(), "proj1-mem");
    }

    #[test]
    fn test_search_in_project_convenience_method() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");

        let mem1 = create_test_memory_with_facets(
            "proj-mem",
            "Rust in my project",
            Some("my-project"),
            None,
            None,
        );
        let mem2 = create_test_memory_with_facets(
            "other-mem",
            "Rust elsewhere",
            Some("other-project"),
            None,
            None,
        );

        index.index(&mem1).unwrap();
        index.index(&mem2).unwrap();

        let service = RecallService::with_index(index);

        let result = service
            .search_in_project("Rust", "my-project", SearchMode::Text, 10)
            .unwrap();

        assert_eq!(result.memories.len(), 1);
        assert_eq!(result.memories[0].memory.id.as_str(), "proj-mem");
    }

    #[test]
    fn test_search_on_branch_convenience_method() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");

        let mem1 =
            create_test_memory_with_facets("main-mem", "Rust on main", None, Some("main"), None);
        let mem2 = create_test_memory_with_facets(
            "dev-mem",
            "Rust on develop",
            None,
            Some("develop"),
            None,
        );

        index.index(&mem1).unwrap();
        index.index(&mem2).unwrap();

        let service = RecallService::with_index(index);

        let result = service
            .search_on_branch("Rust", "develop", SearchMode::Text, 10)
            .unwrap();

        assert_eq!(result.memories.len(), 1);
        assert_eq!(result.memories[0].memory.id.as_str(), "dev-mem");
    }

    #[test]
    fn test_search_with_tombstoned_convenience_method() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");

        let active = create_test_memory("active", "Rust active");
        let tombstoned = create_tombstoned_memory("tombstoned", "Rust tombstoned");

        index.index(&active).unwrap();
        index.index(&tombstoned).unwrap();

        let service = RecallService::with_index(index);

        let result = service
            .search_with_tombstoned("Rust", SearchMode::Text, 10)
            .unwrap();

        assert_eq!(result.memories.len(), 2);
    }

    #[test]
    fn test_combined_facet_filters() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");

        // Create memories with various combinations
        let mem1 = create_test_memory_with_facets(
            "target",
            "Rust target memory",
            Some("my-project"),
            Some("main"),
            Some("src/lib.rs"),
        );
        let mem2 = create_test_memory_with_facets(
            "wrong-project",
            "Rust wrong project",
            Some("other-project"),
            Some("main"),
            Some("src/lib.rs"),
        );
        let mem3 = create_test_memory_with_facets(
            "wrong-branch",
            "Rust wrong branch",
            Some("my-project"),
            Some("develop"),
            Some("src/lib.rs"),
        );

        index.index(&mem1).unwrap();
        index.index(&mem2).unwrap();
        index.index(&mem3).unwrap();

        let service = RecallService::with_index(index);

        // Filter by project AND branch
        let filter = SearchFilter::new()
            .with_project_id("my-project")
            .with_branch("main");
        let result = service
            .search("Rust", SearchMode::Text, &filter, 10)
            .unwrap();

        assert_eq!(result.memories.len(), 1);
        assert_eq!(result.memories[0].memory.id.as_str(), "target");
    }

    // ========================================================================
    // Tombstone Hint Tests (Task 3.5 - Storage Simplification)
    // ========================================================================

    #[test]
    fn test_tombstone_hint_when_sparse_results() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");

        // Create only tombstoned memories (no active ones)
        let tombstoned1 =
            create_tombstoned_memory_with_branch("tomb1", "Rust tombstoned 1", "feature/auth");
        let tombstoned2 =
            create_tombstoned_memory_with_branch("tomb2", "Rust tombstoned 2", "feature/payments");

        index.index(&tombstoned1).unwrap();
        index.index(&tombstoned2).unwrap();

        let service = RecallService::with_index(index);

        // Search should return 0 active results but have a tombstone hint
        let result = service
            .search("Rust", SearchMode::Text, &SearchFilter::new(), 10)
            .unwrap();

        assert!(result.memories.is_empty());
        assert!(result.tombstone_hint.is_some());

        let hint = result.tombstone_hint.unwrap();
        assert_eq!(hint.count, 2);
        assert!(hint.branches.contains(&"feature/auth".to_string()));
        assert!(hint.branches.contains(&"feature/payments".to_string()));
    }

    #[test]
    fn test_tombstone_hint_with_few_active_results() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");

        // Create 2 active and 3 tombstoned (sparse threshold is 3)
        let active1 = create_test_memory("active1", "Rust active 1");
        let active2 = create_test_memory("active2", "Rust active 2");
        let tombstoned1 =
            create_tombstoned_memory_with_branch("tomb1", "Rust tombstoned 1", "old-branch");
        let tombstoned2 =
            create_tombstoned_memory_with_branch("tomb2", "Rust tombstoned 2", "old-branch");
        let tombstoned3 =
            create_tombstoned_memory_with_branch("tomb3", "Rust tombstoned 3", "another-branch");

        index.index(&active1).unwrap();
        index.index(&active2).unwrap();
        index.index(&tombstoned1).unwrap();
        index.index(&tombstoned2).unwrap();
        index.index(&tombstoned3).unwrap();

        let service = RecallService::with_index(index);

        let result = service
            .search("Rust", SearchMode::Text, &SearchFilter::new(), 10)
            .unwrap();

        // Should have 2 active results (< 3 threshold)
        assert_eq!(result.memories.len(), 2);

        // Should have tombstone hint
        assert!(result.tombstone_hint.is_some());
        let hint = result.tombstone_hint.unwrap();
        assert_eq!(hint.count, 3);
        assert!(hint.branches.contains(&"old-branch".to_string()));
        assert!(hint.branches.contains(&"another-branch".to_string()));
    }

    #[test]
    fn test_no_tombstone_hint_when_enough_active_results() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");

        // Create 5 active memories (above threshold)
        for i in 0..5 {
            let mem = create_test_memory(&format!("active{i}"), &format!("Rust active {i}"));
            index.index(&mem).unwrap();
        }

        // Create some tombstoned
        let tombstoned = create_tombstoned_memory("tomb1", "Rust tombstoned");
        index.index(&tombstoned).unwrap();

        let service = RecallService::with_index(index);

        let result = service
            .search("Rust", SearchMode::Text, &SearchFilter::new(), 10)
            .unwrap();

        // Should have 5 active results (>= 3 threshold)
        assert_eq!(result.memories.len(), 5);

        // Should NOT have tombstone hint
        assert!(result.tombstone_hint.is_none());
    }

    #[test]
    fn test_no_tombstone_hint_when_include_tombstoned() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");

        // Create only tombstoned memories
        let tombstoned = create_tombstoned_memory_with_branch("tomb1", "Rust tombstoned", "branch");
        index.index(&tombstoned).unwrap();

        let service = RecallService::with_index(index);

        // Search with include_tombstoned = true
        let filter = SearchFilter::new().with_include_tombstoned(true);
        let result = service
            .search("Rust", SearchMode::Text, &filter, 10)
            .unwrap();

        // Should have the tombstoned memory in results
        assert_eq!(result.memories.len(), 1);

        // Should NOT have tombstone hint (already included)
        assert!(result.tombstone_hint.is_none());
    }

    #[test]
    fn test_tombstone_hint_message() {
        let hint = TombstoneHint::new(3, vec!["feature/auth".to_string(), "develop".to_string()]);

        assert!(hint.has_tombstones());
        let message = hint.message().unwrap();
        assert!(message.contains("3 additional memories"));
        assert!(message.contains("feature/auth"));
        assert!(message.contains("develop"));
    }

    #[test]
    fn test_tombstone_hint_message_no_branches() {
        let hint = TombstoneHint::new(2, vec![]);

        assert!(hint.has_tombstones());
        let message = hint.message().unwrap();
        assert!(message.contains("2 additional memories"));
        assert!(!message.contains("branches:"));
    }

    #[test]
    fn test_tombstone_hint_message_empty() {
        let hint = TombstoneHint::new(0, vec![]);

        assert!(!hint.has_tombstones());
        assert!(hint.message().is_none());
    }

    // ========================================================================
    // Score Normalization Tests (Phase 4.6)
    // ========================================================================

    #[test]
    fn test_normalize_scores_max_becomes_one() {
        let mut hits = vec![
            SearchHit {
                memory: create_test_memory("id1", "high score"),
                score: 0.033,
                raw_score: 0.0,
                vector_score: None,
                bm25_score: None,
            },
            SearchHit {
                memory: create_test_memory("id2", "low score"),
                score: 0.020,
                raw_score: 0.0,
                vector_score: None,
                bm25_score: None,
            },
        ];

        normalize_scores(&mut hits);

        // Max score should be 1.0
        assert!(
            (hits[0].score - 1.0).abs() < f32::EPSILON,
            "Max score should be 1.0"
        );
        // raw_score should be preserved
        assert!(
            (hits[0].raw_score - 0.033).abs() < f32::EPSILON,
            "raw_score should be preserved"
        );
    }

    #[test]
    fn test_normalize_scores_all_in_range() {
        let mut hits = vec![
            SearchHit {
                memory: create_test_memory("id1", ""),
                score: 0.033,
                raw_score: 0.0,
                vector_score: None,
                bm25_score: None,
            },
            SearchHit {
                memory: create_test_memory("id2", ""),
                score: 0.020,
                raw_score: 0.0,
                vector_score: None,
                bm25_score: None,
            },
            SearchHit {
                memory: create_test_memory("id3", ""),
                score: 0.016,
                raw_score: 0.0,
                vector_score: None,
                bm25_score: None,
            },
        ];

        normalize_scores(&mut hits);

        for hit in &hits {
            assert!(
                hit.score >= 0.0,
                "Score should be >= 0.0, got {}",
                hit.score
            );
            assert!(
                hit.score <= 1.0,
                "Score should be <= 1.0, got {}",
                hit.score
            );
        }
    }

    #[test]
    fn test_normalize_scores_empty_results() {
        let mut hits: Vec<SearchHit> = vec![];
        normalize_scores(&mut hits);
        // Should not panic
        assert!(hits.is_empty());
    }

    #[test]
    fn test_normalize_scores_single_result() {
        let mut hits = vec![SearchHit {
            memory: create_test_memory("id1", ""),
            score: 0.5,
            raw_score: 0.0,
            vector_score: None,
            bm25_score: None,
        }];

        normalize_scores(&mut hits);

        // Single result should have score 1.0
        assert!(
            (hits[0].score - 1.0).abs() < f32::EPSILON,
            "Single result should have score 1.0"
        );
    }

    #[test]
    fn test_normalize_scores_proportions_preserved() {
        let mut hits = vec![
            SearchHit {
                memory: create_test_memory("id1", ""),
                score: 0.040,
                raw_score: 0.0,
                vector_score: None,
                bm25_score: None,
            },
            SearchHit {
                memory: create_test_memory("id2", ""),
                score: 0.020,
                raw_score: 0.0,
                vector_score: None,
                bm25_score: None,
            },
        ];

        // Before: id1 is 2x id2
        let ratio_before = hits[0].score / hits[1].score;

        normalize_scores(&mut hits);

        // After: ratio should be preserved
        let ratio_after = hits[0].score / hits[1].score;
        assert!(
            (ratio_before - ratio_after).abs() < 0.001,
            "Proportions should be preserved: before={ratio_before}, after={ratio_after}"
        );
    }

    #[test]
    fn test_normalize_scores_ordering_preserved() {
        let mut hits = vec![
            SearchHit {
                memory: create_test_memory("id1", ""),
                score: 0.033,
                raw_score: 0.0,
                vector_score: None,
                bm25_score: None,
            },
            SearchHit {
                memory: create_test_memory("id2", ""),
                score: 0.020,
                raw_score: 0.0,
                vector_score: None,
                bm25_score: None,
            },
            SearchHit {
                memory: create_test_memory("id3", ""),
                score: 0.016,
                raw_score: 0.0,
                vector_score: None,
                bm25_score: None,
            },
        ];

        normalize_scores(&mut hits);

        // Ordering should be preserved
        assert!(hits[0].score > hits[1].score, "id1 > id2");
        assert!(hits[1].score > hits[2].score, "id2 > id3");
    }

    #[test]
    fn test_normalize_scores_zero_scores() {
        let mut hits = vec![
            SearchHit {
                memory: create_test_memory("id1", ""),
                score: 0.0,
                raw_score: 0.0,
                vector_score: None,
                bm25_score: None,
            },
            SearchHit {
                memory: create_test_memory("id2", ""),
                score: 0.0,
                raw_score: 0.0,
                vector_score: None,
                bm25_score: None,
            },
        ];

        normalize_scores(&mut hits);

        // All zero scores should remain zero (no division by zero)
        assert!(
            hits[0].score.abs() < f32::EPSILON,
            "Zero score should remain zero"
        );
        assert!(
            hits[1].score.abs() < f32::EPSILON,
            "Zero score should remain zero"
        );
    }

    #[test]
    fn test_normalize_scores_raw_score_preserved() {
        let mut hits = vec![
            SearchHit {
                memory: create_test_memory("id1", ""),
                score: 0.033,
                raw_score: 0.0,
                vector_score: None,
                bm25_score: None,
            },
            SearchHit {
                memory: create_test_memory("id2", ""),
                score: 0.020,
                raw_score: 0.0,
                vector_score: None,
                bm25_score: None,
            },
        ];

        normalize_scores(&mut hits);

        // raw_score should contain the original score
        assert!(
            (hits[0].raw_score - 0.033).abs() < f32::EPSILON,
            "raw_score should be 0.033"
        );
        assert!(
            (hits[1].raw_score - 0.020).abs() < f32::EPSILON,
            "raw_score should be 0.020"
        );
    }
}

// ============================================================================
// Property-Based Tests (Phase 4.7)
// ============================================================================
#[cfg(test)]
mod proptests {
    use super::*;
    use crate::models::{Domain, Namespace};
    use proptest::prelude::*;

    fn create_test_memory_prop(id: &str) -> Memory {
        Memory {
            id: MemoryId::new(id),
            content: String::new(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            status: MemoryStatus::Active,
            created_at: 0,
            updated_at: 0,
            embedding: None,
            tags: Vec::new(),
            source: None,
            project_id: None,
            branch: None,
            file_path: None,
            tombstoned_at: None,
        }
    }

    // Strategy for generating valid positive scores
    fn score_strategy() -> impl Strategy<Value = f32> {
        // Use positive scores > EPSILON to avoid edge case of all zeros
        (1u32..=1_000_000u32).prop_map(|n| n as f32 / 1_000_000.0)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: All normalized scores are always in [0.0, 1.0] range.
        #[test]
        fn prop_normalized_scores_in_range(
            scores in prop::collection::vec(score_strategy(), 1..20)
        ) {
            let mut hits: Vec<SearchHit> = scores
                .into_iter()
                .enumerate()
                .map(|(i, score)| SearchHit {
                    memory: create_test_memory_prop(&format!("id{i}")),
                    score,
                    raw_score: 0.0,
                    vector_score: None,
                    bm25_score: None,
                })
                .collect();

            normalize_scores(&mut hits);

            for hit in &hits {
                prop_assert!(
                    hit.score >= 0.0,
                    "Score {} should be >= 0.0",
                    hit.score
                );
                prop_assert!(
                    hit.score <= 1.0,
                    "Score {} should be <= 1.0",
                    hit.score
                );
            }
        }

        /// Property: Score ordering is preserved after normalization.
        #[test]
        fn prop_ordering_preserved(
            scores in prop::collection::vec(score_strategy(), 2..20)
        ) {
            let mut hits: Vec<SearchHit> = scores
                .iter()
                .enumerate()
                .map(|(i, &score)| SearchHit {
                    memory: create_test_memory_prop(&format!("id{i}")),
                    score,
                    raw_score: 0.0,
                    vector_score: None,
                    bm25_score: None,
                })
                .collect();

            // Sort by original score descending
            let mut original_order: Vec<_> = scores.iter().enumerate().collect();
            original_order.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
            let original_ids: Vec<_> = original_order.iter().map(|(i, _)| *i).collect();

            normalize_scores(&mut hits);

            // Sort by normalized score descending
            hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
            let normalized_ids: Vec<_> = hits
                .iter()
                .map(|h| h.memory.id.as_str().strip_prefix("id").unwrap().parse::<usize>().unwrap())
                .collect();

            prop_assert_eq!(
                original_ids,
                normalized_ids,
                "Score ordering should be preserved"
            );
        }

        /// Property: Maximum score becomes 1.0 after normalization.
        #[test]
        fn prop_max_score_is_one(
            scores in prop::collection::vec(score_strategy(), 1..20)
        ) {
            let mut hits: Vec<SearchHit> = scores
                .into_iter()
                .enumerate()
                .map(|(i, score)| SearchHit {
                    memory: create_test_memory_prop(&format!("id{i}")),
                    score,
                    raw_score: 0.0,
                    vector_score: None,
                    bm25_score: None,
                })
                .collect();

            normalize_scores(&mut hits);

            let max_score = hits.iter().map(|h| h.score).fold(0.0_f32, f32::max);
            prop_assert!(
                (max_score - 1.0).abs() < f32::EPSILON,
                "Max score should be 1.0, got {}",
                max_score
            );
        }

        /// Property: raw_score is preserved and equals original score.
        #[test]
        fn prop_raw_score_preserved(
            scores in prop::collection::vec(score_strategy(), 1..20)
        ) {
            let original_scores = scores.clone();

            let mut hits: Vec<SearchHit> = scores
                .into_iter()
                .enumerate()
                .map(|(i, score)| SearchHit {
                    memory: create_test_memory_prop(&format!("id{i}")),
                    score,
                    raw_score: 0.0,
                    vector_score: None,
                    bm25_score: None,
                })
                .collect();

            normalize_scores(&mut hits);

            for (hit, original) in hits.iter().zip(original_scores.iter()) {
                prop_assert!(
                    (hit.raw_score - original).abs() < f32::EPSILON,
                    "raw_score {} should equal original {}",
                    hit.raw_score,
                    original
                );
            }
        }
    }
}
