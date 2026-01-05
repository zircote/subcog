//! Memory recall (search) service.
//!
//! Searches for memories using hybrid (vector + BM25) search with RRF fusion.

use crate::context::GitContext;
use crate::current_timestamp;
use crate::embedding::Embedder;
use crate::gc::branch_exists;
use crate::models::{
    EventMeta, Memory, MemoryEvent, MemoryId, MemoryStatus, SearchFilter, SearchHit, SearchMode,
    SearchResult,
};
use crate::observability::current_request_id;
use crate::security::record_event;
use crate::storage::index::SqliteBackend;
use crate::storage::traits::{IndexBackend, VectorBackend};
use crate::{Error, Result};
use chrono::{TimeZone, Utc};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::{instrument, warn};

/// RRF fusion entry storing indices instead of cloning [`SearchHit`].
type RrfEntry = (f32, Option<usize>, Option<usize>, Option<f32>);

/// Default search timeout in milliseconds (5 seconds).
pub const DEFAULT_SEARCH_TIMEOUT_MS: u64 = 5_000;

/// Service for searching and retrieving memories.
///
/// Supports three search modes:
/// - **Text**: BM25 full-text search via `SQLite` FTS5
/// - **Vector**: Semantic similarity search via embedding + vector backend
/// - **Hybrid**: Combines both using Reciprocal Rank Fusion (RRF)
///
/// # Graceful Degradation
///
/// If embedder or vector backend is unavailable:
/// - `SearchMode::Vector` falls back to empty results with a warning
/// - `SearchMode::Hybrid` falls back to text-only search
/// - No errors are raised; partial results are returned
///
/// # Timeout Enforcement (RES-M5)
///
/// Search operations respect a configurable timeout (default 5 seconds).
/// If the deadline is exceeded, the search returns partial results or an error.
pub struct RecallService {
    /// `SQLite` index backend for BM25 text search.
    index: Option<SqliteBackend>,
    /// Embedder for generating query embeddings (optional).
    embedder: Option<Arc<dyn Embedder>>,
    /// Vector backend for similarity search (optional).
    vector: Option<Arc<dyn VectorBackend + Send + Sync>>,
    /// Search timeout in milliseconds (RES-M5).
    timeout_ms: u64,
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
            timeout_ms: DEFAULT_SEARCH_TIMEOUT_MS,
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
            timeout_ms: DEFAULT_SEARCH_TIMEOUT_MS,
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
            timeout_ms: DEFAULT_SEARCH_TIMEOUT_MS,
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

    /// Sets the search timeout in milliseconds (RES-M5).
    ///
    /// Default: 5000ms (5 seconds).
    ///
    /// # Arguments
    ///
    /// * `timeout_ms` - Timeout in milliseconds. Use 0 for no timeout.
    #[must_use]
    pub const fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Returns the configured search timeout in milliseconds.
    #[must_use]
    pub const fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Returns whether vector search is available.
    #[must_use]
    pub fn has_vector_search(&self) -> bool {
        self.embedder.is_some() && self.vector.is_some()
    }

    /// Searches for memories matching a query.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidInput`] if:
    /// - The query is empty or contains only whitespace
    ///
    /// Returns [`Error::OperationFailed`] if:
    /// - No index backend is configured (for `Text` and `Hybrid` modes)
    /// - The index backend search operation fails
    /// - The search timeout is exceeded (RES-M5)
    #[allow(clippy::cast_possible_truncation)]
    #[instrument(
        skip(self, query, filter),
        fields(
            operation = "recall",
            mode = %mode,
            query_length = query.len(),
            limit = limit,
            timeout_ms = self.timeout_ms
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

        tracing::info!(mode = %mode_label, query_length = query.len(), limit = limit, timeout_ms = self.timeout_ms, "Searching memories");

        // Maximum query size (10KB) - prevents abuse and ensures reasonable embedding times (MED-RES-005)
        const MAX_QUERY_SIZE: usize = 10_000;

        // Deadline for timeout enforcement (RES-M5)
        let deadline_ms = self.timeout_ms;

        let result = (|| {
            // Validate query length (MED-RES-005)
            if query.trim().is_empty() {
                return Err(Error::InvalidInput("Query cannot be empty".to_string()));
            }
            if query.len() > MAX_QUERY_SIZE {
                return Err(Error::InvalidInput(format!(
                    "Query exceeds maximum size of {} bytes (got {} bytes)",
                    MAX_QUERY_SIZE,
                    query.len()
                )));
            }

            // Check timeout before search (RES-M5)
            if deadline_ms > 0 && start.elapsed().as_millis() as u64 >= deadline_ms {
                tracing::warn!(
                    elapsed_ms = start.elapsed().as_millis(),
                    timeout_ms = deadline_ms,
                    "Search timeout before execution"
                );
                metrics::counter!("memory_search_timeouts_total", "mode" => mode_label, "phase" => "pre_search").increment(1);
                return Err(Error::OperationFailed {
                    operation: "search".to_string(),
                    cause: format!("Search timeout exceeded ({deadline_ms}ms)"),
                });
            }

            let mut memories = match mode {
                SearchMode::Text => self.text_search(query, filter, limit)?,
                SearchMode::Vector => self.vector_search(query, filter, limit)?,
                SearchMode::Hybrid => self.hybrid_search(query, filter, limit)?,
            };

            // Check timeout after search (RES-M5)
            if deadline_ms > 0 && start.elapsed().as_millis() as u64 >= deadline_ms {
                tracing::warn!(
                    elapsed_ms = start.elapsed().as_millis(),
                    timeout_ms = deadline_ms,
                    results_found = memories.len(),
                    "Search timeout after execution, returning partial results"
                );
                metrics::counter!("memory_search_timeouts_total", "mode" => mode_label, "phase" => "post_search").increment(1);
                // Return partial results instead of error - graceful degradation
            }

            // Normalize scores to 0.0-1.0 range for Text and Vector modes
            // (Hybrid mode already normalizes after RRF fusion)
            if mode != SearchMode::Hybrid {
                normalize_scores(&mut memories);
            }

            self.lazy_tombstone_stale_branches(&mut memories, filter);

            // Safe cast: u128 milliseconds will practically never exceed u64::MAX
            let execution_time_ms = start.elapsed().as_millis() as u64;
            let total_count = memories.len();
            let timestamp = current_timestamp();
            // Use Arc<str> for zero-copy sharing across events (PERF-C1).
            // Arc::clone() is O(1) atomic increment vs O(n) String::clone().
            let query_arc: std::sync::Arc<str> = query.into();
            for hit in &memories {
                record_event(MemoryEvent::Retrieved {
                    meta: EventMeta::with_timestamp("recall", current_request_id(), timestamp),
                    memory_id: hit.memory.id.clone(),
                    query: std::sync::Arc::clone(&query_arc),
                    score: hit.score,
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

    fn lazy_tombstone_stale_branches(&self, hits: &mut Vec<SearchHit>, filter: &SearchFilter) {
        let Some(index) = self.index.as_ref() else {
            return;
        };

        let ctx = GitContext::from_cwd();
        let Some(project_id) = ctx.project_id else {
            return;
        };

        let now = current_timestamp();
        let now_i64 = i64::try_from(now).unwrap_or(i64::MAX);
        let now_dt = Utc
            .timestamp_opt(now_i64, 0)
            .single()
            .unwrap_or_else(Utc::now);

        for hit in hits.iter_mut() {
            let Some(branch) = hit.memory.branch.as_deref() else {
                continue;
            };

            if hit.memory.status == MemoryStatus::Tombstoned {
                continue;
            }

            if hit.memory.project_id.as_deref() != Some(&project_id) {
                continue;
            }

            if branch_exists(branch) {
                continue;
            }

            let mut updated = hit.memory.clone();
            updated.status = MemoryStatus::Tombstoned;
            updated.tombstoned_at = Some(now_dt);

            if let Err(err) = index.index(&updated) {
                warn!(
                    error = %err,
                    memory_id = %updated.id.as_str(),
                    "Failed to tombstone stale branch memory during recall"
                );
                continue;
            }

            hit.memory.status = MemoryStatus::Tombstoned;
            hit.memory.tombstoned_at = Some(now_dt);
        }

        if !filter.include_tombstoned {
            hits.retain(|hit| hit.memory.status != MemoryStatus::Tombstoned);
        }
    }

    /// Lists all memories, optionally filtered by namespace.
    ///
    /// Unlike `search`, this doesn't require a query and returns all matching memories.
    /// Returns minimal metadata (id, namespace) without content - details via drill-down.
    ///
    /// # Errors
    ///
    /// Returns [`Error::OperationFailed`] if:
    /// - No index backend is configured
    /// - The index backend list operation fails
    /// - Batch memory retrieval fails
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
                    meta: EventMeta::with_timestamp("recall", current_request_id(), timestamp),
                    memory_id: hit.memory.id.clone(),
                    query: std::sync::Arc::clone(&query_arc),
                    score: hit.score,
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

        // Use indices instead of cloning SearchHits (PERF-C2)
        // Store: (rrf_score, text_index, vector_index, vector_score)
        // - text_index: Some if hit came from text search
        // - vector_index: Some if hit also/only came from vector search
        // - vector_score: Optional vector score to merge
        let capacity = text_results.len() + vector_results.len();
        let mut scores: HashMap<String, RrfEntry> = HashMap::with_capacity(capacity);

        // Add text results - store indices instead of cloning (PERF-C2)
        for (rank, hit) in text_results.iter().enumerate() {
            let id = hit.memory.id.to_string();
            let rrf_score = 1.0 / (K + rank as f32 + 1.0);

            scores
                .entry(id)
                .and_modify(|(s, _, _, _)| *s += rrf_score)
                .or_insert((rrf_score, Some(rank), None, None));
        }

        // Add vector results - merge with existing or insert index (PERF-C2)
        for (rank, hit) in vector_results.iter().enumerate() {
            let id = hit.memory.id.to_string();
            let rrf_score = 1.0 / (K + rank as f32 + 1.0);

            scores
                .entry(id)
                .and_modify(|(s, _, vec_idx, vec_score)| {
                    *s += rrf_score;
                    // Store vector index and score for merging later
                    *vec_idx = Some(rank);
                    *vec_score = hit.vector_score;
                })
                .or_insert((rrf_score, None, Some(rank), hit.vector_score));
        }

        // Reconstruct results from indices - only clone at final step (PERF-C2)
        let mut results: Vec<_> = scores
            .into_iter()
            .filter_map(|(_, (score, text_idx, vec_idx, vec_score))| {
                // Prefer text hit (has BM25 score), fall back to vector hit
                let mut hit = if let Some(idx) = text_idx {
                    text_results.get(idx).cloned()
                } else {
                    vec_idx.and_then(|idx| vector_results.get(idx).cloned())
                }?;

                // Merge vector score if we have one from vector search
                if vec_score.is_some() {
                    hit.vector_score = vec_score;
                }

                hit.score = score;
                Some(hit)
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

    /// Searches for memories with authorization check (CRIT-006).
    ///
    /// This method requires [`super::auth::Permission::Read`] to be present in the auth context.
    /// Use this for MCP/HTTP endpoints where authorization is required.
    ///
    /// # Arguments
    ///
    /// * `query` - The search query
    /// * `mode` - Search mode (text, vector, or hybrid)
    /// * `filter` - Optional filters for namespace, domain, etc.
    /// * `limit` - Maximum number of results to return
    /// * `auth` - Authorization context with permissions
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unauthorized`] if read permission is not granted.
    /// Returns other errors as per [`search`](Self::search).
    pub fn search_authorized(
        &self,
        query: &str,
        mode: SearchMode,
        filter: &SearchFilter,
        limit: usize,
        auth: &super::auth::AuthContext,
    ) -> Result<SearchResult> {
        auth.require(super::auth::Permission::Read)?;
        self.search(query, mode, filter, limit)
    }

    /// Retrieves a memory by ID with authorization check (CRIT-006).
    ///
    /// This method requires [`super::auth::Permission::Read`] to be present in the auth context.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unauthorized`] if read permission is not granted.
    pub fn get_by_id_authorized(
        &self,
        id: &MemoryId,
        auth: &super::auth::AuthContext,
    ) -> Result<Option<Memory>> {
        auth.require(super::auth::Permission::Read)?;
        self.get_by_id(id)
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
    // Search Timeout Tests (RES-M5)
    // ========================================================================

    #[test]
    fn test_default_timeout() {
        let service = RecallService::new();
        assert_eq!(service.timeout_ms(), DEFAULT_SEARCH_TIMEOUT_MS);
        assert_eq!(service.timeout_ms(), 5_000);
    }

    #[test]
    fn test_with_timeout_ms() {
        let service = RecallService::new().with_timeout_ms(1_000);
        assert_eq!(service.timeout_ms(), 1_000);
    }

    #[test]
    fn test_timeout_zero_disables_check() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");
        index
            .index(&create_test_memory("id1", "Rust programming"))
            .expect("index failed");

        // timeout_ms = 0 should disable timeout checking
        let service = RecallService::with_index(index).with_timeout_ms(0);

        let result = service.search("Rust", SearchMode::Text, &SearchFilter::new(), 10);
        assert!(
            result.is_ok(),
            "Search should succeed with timeout disabled"
        );
    }

    #[test]
    fn test_timeout_with_index_builder() {
        let index = SqliteBackend::in_memory().expect("in_memory failed");
        let service = RecallService::with_index(index);

        // Default timeout should be applied
        assert_eq!(service.timeout_ms(), DEFAULT_SEARCH_TIMEOUT_MS);
    }

    #[test]
    fn test_timeout_builder_chaining() {
        let service = RecallService::new().with_timeout_ms(2_500);

        assert_eq!(service.timeout_ms(), 2_500);
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
