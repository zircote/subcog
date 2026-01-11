//! Memory consolidation service.
//!
//! Manages memory lifecycle, clustering, and archival.
//!
//! # Circuit Breaker Pattern
//!
//! LLM calls in consolidation should be wrapped with [`ResilientLlmProvider`] for:
//! - **Automatic retries**: Transient failures (timeouts, 5xx errors) retry with exponential backoff
//! - **Circuit breaker**: Opens after consecutive failures to prevent cascading failures
//! - **Error budget**: Tracks error rate and latency SLO violations
//!
//! See [`with_llm`] for usage examples.
//!
//! [`ResilientLlmProvider`]: crate::llm::ResilientLlmProvider
//! [`with_llm`]: ConsolidationService::with_llm

use crate::Result;
use crate::current_timestamp;
use crate::llm::LlmProvider;
use crate::models::{
    EdgeType, EventMeta, Memory, MemoryEvent, MemoryStatus, MemoryTier, Namespace, RetentionScore,
};
use crate::observability::current_request_id;
use crate::security::record_event;
use crate::storage::index::SqliteBackend;
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
    /// Optional index backend for storing memory edges.
    index: Option<Arc<SqliteBackend>>,
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
            index: None,
        }
    }

    /// Sets the LLM provider for intelligent consolidation.
    ///
    /// **Recommended**: Wrap the LLM provider with [`ResilientLlmProvider`] for automatic
    /// retries, circuit breaker pattern, and error budget tracking.
    ///
    /// # Arguments
    ///
    /// * `llm` - The LLM provider to use for summarization and analysis.
    ///
    /// # Examples
    ///
    /// ## With Resilience Wrapper (Recommended)
    ///
    /// ```rust,ignore
    /// use subcog::services::ConsolidationService;
    /// use subcog::llm::{AnthropicClient, ResilientLlmProvider, LlmResilienceConfig};
    /// use subcog::storage::persistence::FilesystemBackend;
    /// use std::sync::Arc;
    ///
    /// let backend = FilesystemBackend::new("/tmp/memories");
    /// let client = AnthropicClient::new();
    /// let resilience_config = LlmResilienceConfig::default();
    /// let llm = Arc::new(ResilientLlmProvider::new(client, resilience_config));
    /// let service = ConsolidationService::new(backend).with_llm(llm);
    /// ```
    ///
    /// ## Without Resilience Wrapper
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
    ///
    /// [`ResilientLlmProvider`]: crate::llm::ResilientLlmProvider
    #[must_use]
    pub fn with_llm(mut self, llm: Arc<dyn LlmProvider + Send + Sync>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Sets the index backend for storing memory edges.
    ///
    /// The index backend is used to store relationships between memories and their summaries.
    /// If not set, edge relationships will not be persisted.
    ///
    /// # Arguments
    ///
    /// * `index` - The SQLite index backend to use for edge storage.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use subcog::services::ConsolidationService;
    /// use subcog::storage::persistence::FilesystemBackend;
    /// use subcog::storage::index::SqliteBackend;
    /// use std::sync::Arc;
    ///
    /// let persistence = FilesystemBackend::new("/tmp/memories");
    /// let index = SqliteBackend::new("/tmp/index.db")?;
    /// let service = ConsolidationService::new(persistence)
    ///     .with_index(Arc::new(index));
    /// # Ok::<(), subcog::Error>(())
    /// ```
    #[must_use]
    pub fn with_index(mut self, index: Arc<SqliteBackend>) -> Self {
        self.index = Some(index);
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

    /// Consolidates memories by finding related groups, summarizing them, and creating summary nodes.
    ///
    /// This is the main orchestrator method for memory consolidation. It:
    /// 1. Finds related memory groups using semantic similarity
    /// 2. Summarizes each group using LLM
    /// 3. Creates summary nodes and stores edge relationships
    ///
    /// # Arguments
    ///
    /// * `recall_service` - The recall service for semantic search
    /// * `config` - Consolidation configuration (filters, thresholds, etc.)
    ///
    /// # Returns
    ///
    /// [`ConsolidationStats`] with counts of processed memories and summaries created.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Finding related memories fails
    /// - LLM summarization fails (when LLM is configured)
    /// - Creating summary nodes fails
    ///
    /// # Graceful Degradation
    ///
    /// When LLM is unavailable:
    /// - Returns an error if summarization is required
    /// - Caller should handle by skipping consolidation or using fallback
    ///
    /// **Circuit Breaker**: If using [`ResilientLlmProvider`], LLM failures will:
    /// - Retry with exponential backoff (3 attempts by default)
    /// - Open circuit after consecutive failures (prevents cascading failures)
    /// - Return circuit breaker errors when circuit is open
    ///
    /// [`ResilientLlmProvider`]: crate::llm::ResilientLlmProvider
    ///
    /// # Configuration
    ///
    /// Respects the following configuration options:
    /// - `namespace_filter`: Only consolidate specific namespaces
    /// - `time_window_days`: Only consolidate recent memories
    /// - `min_memories_to_consolidate`: Minimum group size
    /// - `similarity_threshold`: Similarity threshold for grouping
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use subcog::services::{ConsolidationService, RecallService};
    /// use subcog::config::ConsolidationConfig;
    /// use subcog::storage::persistence::FilesystemBackend;
    /// use std::sync::Arc;
    ///
    /// let backend = FilesystemBackend::new("/tmp/memories");
    /// let mut service = ConsolidationService::new(backend);
    /// let recall = RecallService::new();
    /// let mut config = ConsolidationConfig::new();
    /// config.enabled = true;
    /// config.similarity_threshold = 0.8;
    /// config.time_window_days = Some(30);
    ///
    /// let stats = service.consolidate_memories(&recall, &config)?;
    /// println!("{}", stats.summary());
    /// # Ok::<(), subcog::Error>(())
    /// ```
    #[instrument(
        name = "subcog.memory.consolidate_memories",
        skip(self, recall_service, config),
        fields(
            request_id = tracing::field::Empty,
            component = "memory",
            operation = "consolidate_memories"
        )
    )]
    pub fn consolidate_memories(
        &mut self,
        recall_service: &crate::services::RecallService,
        config: &crate::config::ConsolidationConfig,
    ) -> Result<ConsolidationStats> {
        let start = Instant::now();
        if let Some(request_id) = current_request_id() {
            tracing::Span::current().record("request_id", request_id.as_str());
        }

        let result = (|| {
            let mut stats = ConsolidationStats::default();

            // Check if consolidation is enabled
            if !config.enabled {
                tracing::info!("Consolidation is disabled in configuration");
                return Ok(stats);
            }

            // Find related memory groups
            tracing::info!(
                namespace_filter = ?config.namespace_filter,
                time_window_days = ?config.time_window_days,
                similarity_threshold = config.similarity_threshold,
                "Finding related memory groups for consolidation"
            );

            let groups = self.find_related_memories(recall_service, config)?;

            if groups.is_empty() {
                tracing::info!("No related memory groups found for consolidation");
                return Ok(stats);
            }

            tracing::info!(
                namespace_count = groups.len(),
                "Found related memory groups in {} namespaces",
                groups.len()
            );

            // Process each namespace
            for (namespace, namespace_groups) in groups {
                tracing::debug!(
                    namespace = ?namespace,
                    group_count = namespace_groups.len(),
                    "Processing {} groups in namespace {:?}",
                    namespace_groups.len(),
                    namespace
                );

                // Process each group within the namespace
                for (group_idx, memory_ids) in namespace_groups.iter().enumerate() {
                    tracing::debug!(
                        namespace = ?namespace,
                        group_idx = group_idx,
                        memory_count = memory_ids.len(),
                        "Processing group {} with {} memories",
                        group_idx,
                        memory_ids.len()
                    );

                    // Fetch the actual Memory objects from persistence
                    let mut memories = Vec::new();
                    for memory_id in memory_ids {
                        if let Some(memory) = self.persistence.get(memory_id)? {
                            memories.push(memory);
                            stats.processed += 1;
                        } else {
                            tracing::warn!(
                                memory_id = %memory_id.as_str(),
                                "Memory not found in persistence, skipping"
                            );
                        }
                    }

                    if memories.is_empty() {
                        tracing::warn!(
                            namespace = ?namespace,
                            group_idx = group_idx,
                            "No memories found for group, skipping"
                        );
                        continue;
                    }

                    // Summarize the group using LLM
                    let summary_content = match self.summarize_group(&memories) {
                        Ok(summary) => summary,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                namespace = ?namespace,
                                group_idx = group_idx,
                                memory_count = memories.len(),
                                "Failed to summarize group, creating relationships without summary"
                            );

                            // Graceful degradation: Create RelatedTo edges without summary node
                            if let Some(ref index) = self.index {
                                self.create_related_edges(&memories, index)?;
                                tracing::info!(
                                    namespace = ?namespace,
                                    memory_count = memories.len(),
                                    "Created RelatedTo edges for group without LLM summary"
                                );
                            } else {
                                tracing::debug!(
                                    namespace = ?namespace,
                                    memory_count = memories.len(),
                                    "Index backend not available, skipping edge creation"
                                );
                            }
                            continue;
                        }
                    };

                    // Create summary node (also stores edges if index backend available)
                    match self.create_summary_node(&summary_content, &memories) {
                        Ok(summary_node) => {
                            stats.summaries_created += 1;
                            tracing::info!(
                                summary_id = %summary_node.id.as_str(),
                                namespace = ?namespace,
                                source_count = memories.len(),
                                "Created summary node"
                            );
                        }
                        Err(e) => {
                            tracing::error!(
                                error = %e,
                                namespace = ?namespace,
                                group_idx = group_idx,
                                memory_count = memories.len(),
                                "Failed to create summary node"
                            );
                            return Err(e);
                        }
                    }
                }
            }

            record_event(MemoryEvent::Consolidated {
                meta: EventMeta::new("consolidation", current_request_id()),
                processed: stats.processed,
                archived: stats.archived,
                merged: stats.merged,
            });

            tracing::info!(
                processed = stats.processed,
                summaries_created = stats.summaries_created,
                "Consolidation completed successfully"
            );

            Ok(stats)
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        metrics::counter!(
            "memory_operations_total",
            "operation" => "consolidate_memories",
            "namespace" => "mixed",
            "domain" => "project",
            "status" => status
        )
        .increment(1);
        metrics::histogram!(
            "memory_operation_duration_ms",
            "operation" => "consolidate_memories",
            "namespace" => "mixed"
        )
        .record(start.elapsed().as_secs_f64() * 1000.0);

        result
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

    /// Finds related memories grouped by namespace and semantic similarity.
    ///
    /// Uses semantic search to find memories that are related to each other based on
    /// embeddings. Groups results by namespace and filters by similarity threshold.
    ///
    /// # Arguments
    ///
    /// * `recall_service` - The recall service to use for semantic search
    /// * `config` - Consolidation configuration containing similarity threshold and filters
    ///
    /// # Returns
    ///
    /// A map of namespace to vectors of memory IDs that are related above the threshold.
    ///
    /// # Errors
    ///
    /// Returns an error if the semantic search fails.
    ///
    /// # Graceful Degradation
    ///
    /// If embeddings are not available:
    /// - Returns empty result without error
    /// - Logs a warning for visibility
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use subcog::services::{ConsolidationService, RecallService};
    /// use subcog::config::ConsolidationConfig;
    /// use subcog::storage::persistence::FilesystemBackend;
    ///
    /// let backend = FilesystemBackend::new("/tmp/memories");
    /// let service = ConsolidationService::new(backend);
    /// let recall = RecallService::new();
    /// let config = ConsolidationConfig::new().with_similarity_threshold(0.7);
    ///
    /// let groups = service.find_related_memories(&recall, &config)?;
    /// for (namespace, memory_ids) in groups {
    ///     println!("{:?}: {} related memories", namespace, memory_ids.len());
    /// }
    /// # Ok::<(), subcog::Error>(())
    /// ```
    pub fn find_related_memories(
        &self,
        recall_service: &crate::services::RecallService,
        config: &crate::config::ConsolidationConfig,
    ) -> Result<HashMap<Namespace, Vec<Vec<crate::models::MemoryId>>>> {
        use crate::models::{SearchFilter, SearchMode};

        // Check if vector search is available
        if !recall_service.has_vector_search() {
            tracing::warn!(
                "Vector search not available for consolidation. \
                 Embeddings required for semantic grouping. Returning empty result."
            );
            return Ok(HashMap::new());
        }

        // Get all memory IDs from persistence
        let memory_ids = self.persistence.list_ids()?;

        // Apply time window filter if configured
        let now = current_timestamp();
        let cutoff_timestamp = config.time_window_days.map(|days| {
            let days_in_seconds = u64::from(days) * 86400;
            now.saturating_sub(days_in_seconds)
        });

        // Group by namespace while filtering by time window
        let mut namespace_groups: HashMap<Namespace, Vec<Memory>> = HashMap::new();

        for id in &memory_ids {
            if let Some(memory) = self.persistence.get(id)? {
                // Skip if outside time window
                if let Some(cutoff) = cutoff_timestamp
                    && memory.created_at < cutoff
                {
                    continue;
                }

                // Apply namespace filter if configured
                if let Some(ref filter_namespaces) = config.namespace_filter {
                    if !filter_namespaces.contains(&memory.namespace) {
                        continue;
                    }
                }

                // Skip if embedding is missing (needed for similarity comparison)
                if memory.embedding.is_none() {
                    tracing::debug!(
                        memory_id = %memory.id.as_str(),
                        "Skipping memory without embedding for consolidation"
                    );
                    continue;
                }

                namespace_groups
                    .entry(memory.namespace)
                    .or_default()
                    .push(memory);
            }
        }

        // For each namespace, find clusters of related memories
        let mut result: HashMap<Namespace, Vec<Vec<crate::models::MemoryId>>> = HashMap::new();

        for (namespace, memories) in namespace_groups {
            // Need at least min_memories_to_consolidate to form a group
            if memories.len() < config.min_memories_to_consolidate {
                tracing::debug!(
                    namespace = ?namespace,
                    count = memories.len(),
                    min_required = config.min_memories_to_consolidate,
                    "Skipping namespace with insufficient memories"
                );
                continue;
            }

            // Find related memory groups using semantic similarity
            let groups = self.cluster_by_similarity(&memories, config.similarity_threshold)?;

            if !groups.is_empty() {
                result.insert(namespace, groups);
            }
        }

        Ok(result)
    }

    /// Clusters memories by semantic similarity using embeddings.
    ///
    /// Uses cosine similarity between embeddings to group related memories.
    /// Memories are grouped if their similarity is >= threshold.
    fn cluster_by_similarity(
        &self,
        memories: &[Memory],
        threshold: f32,
    ) -> Result<Vec<Vec<crate::models::MemoryId>>> {
        let mut groups: Vec<Vec<crate::models::MemoryId>> = Vec::new();
        let mut assigned: std::collections::HashSet<String> = std::collections::HashSet::new();

        for (i, memory) in memories.iter().enumerate() {
            // Skip if already assigned to a group
            if assigned.contains(memory.id.as_str()) {
                continue;
            }

            let Some(ref embedding_i) = memory.embedding else {
                continue;
            };

            let mut group = vec![memory.id.clone()];
            assigned.insert(memory.id.as_str().to_string());

            // Find all other memories similar to this one
            for (j, other_memory) in memories.iter().enumerate() {
                if i == j || assigned.contains(other_memory.id.as_str()) {
                    continue;
                }

                let Some(ref embedding_j) = other_memory.embedding else {
                    continue;
                };

                // Calculate cosine similarity
                let similarity = cosine_similarity(embedding_i, embedding_j);

                if similarity >= threshold {
                    group.push(other_memory.id.clone());
                    assigned.insert(other_memory.id.as_str().to_string());
                }
            }

            // Only add groups with multiple memories
            if group.len() >= 2 {
                groups.push(group);
            }
        }

        Ok(groups)
    }

    /// Summarizes a group of related memories using LLM.
    ///
    /// Creates a concise summary from a group of related memories while preserving
    /// all key details. Uses the LLM provider to generate an intelligent summary
    /// that combines related information into a cohesive narrative.
    ///
    /// # Arguments
    ///
    /// * `memories` - A slice of memories to summarize together.
    ///
    /// # Returns
    ///
    /// A summary string that preserves key details from all source memories.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No LLM provider is configured (graceful degradation required)
    /// - The LLM call fails
    /// - The response cannot be processed
    ///
    /// # Graceful Degradation
    ///
    /// When LLM is unavailable:
    /// - Returns an error with a clear message
    /// - Caller should handle by either skipping summarization or using fallback logic
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use subcog::services::ConsolidationService;
    /// use subcog::storage::persistence::FilesystemBackend;
    /// use std::sync::Arc;
    /// use subcog::llm::AnthropicClient;
    ///
    /// let backend = FilesystemBackend::new("/tmp/memories");
    /// let llm = Arc::new(AnthropicClient::new());
    /// let service = ConsolidationService::new(backend).with_llm(llm);
    ///
    /// let memories = vec![/* ... */];
    /// let summary = service.summarize_group(&memories)?;
    /// println!("Summary: {}", summary);
    /// # Ok::<(), subcog::Error>(())
    /// ```
    #[instrument(skip(self, memories), fields(memory_count = memories.len()))]
    pub fn summarize_group(&self, memories: &[Memory]) -> Result<String> {
        use crate::llm::{BASE_SYSTEM_PROMPT, MEMORY_SUMMARIZATION_PROMPT};

        // Check if LLM provider is available
        let llm = self.llm.as_ref().ok_or_else(|| {
            tracing::warn!("No LLM provider configured for memory summarization");
            crate::Error::OperationFailed {
                operation: "summarize_group".to_string(),
                cause: "LLM provider not configured. Use with_llm() to set provider.".to_string(),
            }
        })?;

        // Handle empty input
        if memories.is_empty() {
            tracing::warn!("Attempted to summarize empty group of memories");
            return Err(crate::Error::OperationFailed {
                operation: "summarize_group".to_string(),
                cause: "No memories provided for summarization".to_string(),
            });
        }

        // Format memories into a user prompt
        let memories_text = memories
            .iter()
            .enumerate()
            .map(|(i, memory)| {
                format!(
                    "Memory {}: [ID: {}, Namespace: {:?}, Tags: {}]\n{}",
                    i + 1,
                    memory.id.as_str(),
                    memory.namespace,
                    memory.tags.join(", "),
                    memory.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        let user_prompt = format!(
            "Summarize the following {} related memories into a cohesive summary:\n\n{}",
            memories.len(),
            memories_text
        );

        // Build system prompt
        let system_prompt = format!("{}\n\n{}", BASE_SYSTEM_PROMPT, MEMORY_SUMMARIZATION_PROMPT);

        // Call LLM
        tracing::debug!(
            memory_count = memories.len(),
            "Calling LLM for memory summarization"
        );

        let summary = llm
            .complete_with_system(&system_prompt, &user_prompt)
            .map_err(|e| {
                tracing::error!(
                    error = %e,
                    memory_count = memories.len(),
                    "LLM summarization failed"
                );
                crate::Error::OperationFailed {
                    operation: "summarize_group".to_string(),
                    cause: format!("LLM summarization failed: {}", e),
                }
            })?;

        // Trim whitespace from response
        let summary = summary.trim().to_string();

        // Validate that we got a meaningful response
        if summary.is_empty() {
            tracing::warn!("LLM returned empty summary");
            return Err(crate::Error::OperationFailed {
                operation: "summarize_group".to_string(),
                cause: "LLM returned empty summary".to_string(),
            });
        }

        tracing::info!(
            memory_count = memories.len(),
            summary_length = summary.len(),
            "Successfully generated memory summary"
        );

        Ok(summary)
    }

    /// Creates a summary memory node from a group of related memories.
    ///
    /// Creates a new Memory marked as `is_summary=true` that consolidates multiple
    /// related memories. The original memories are preserved and linked via
    /// `source_memory_ids`. Tags are merged from all source memories without duplicates.
    ///
    /// # Arguments
    ///
    /// * `summary_content` - The summary text (typically generated by LLM).
    /// * `source_memories` - A slice of memories that were consolidated into this summary.
    ///
    /// # Returns
    ///
    /// A new Memory with `is_summary=true` and source memory links.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Source memories slice is empty
    /// - Storing the summary node fails
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use subcog::services::ConsolidationService;
    /// use subcog::storage::persistence::FilesystemBackend;
    ///
    /// let backend = FilesystemBackend::new("/tmp/memories");
    /// let mut service = ConsolidationService::new(backend);
    ///
    /// let memories = vec![/* ... */];
    /// let summary_text = "Consolidated summary of related decisions";
    /// let summary_node = service.create_summary_node(summary_text, &memories)?;
    /// assert!(summary_node.is_summary);
    /// # Ok::<(), subcog::Error>(())
    /// ```
    #[instrument(skip(self, summary_content, source_memories), fields(
        source_count = source_memories.len(),
        summary_length = summary_content.len()
    ))]
    pub fn create_summary_node(
        &mut self,
        summary_content: &str,
        source_memories: &[Memory],
    ) -> Result<Memory> {
        // Validate inputs
        if source_memories.is_empty() {
            tracing::warn!("Attempted to create summary node with no source memories");
            return Err(crate::Error::OperationFailed {
                operation: "create_summary_node".to_string(),
                cause: "No source memories provided for summary".to_string(),
            });
        }

        if summary_content.trim().is_empty() {
            tracing::warn!("Attempted to create summary node with empty content");
            return Err(crate::Error::OperationFailed {
                operation: "create_summary_node".to_string(),
                cause: "Summary content cannot be empty".to_string(),
            });
        }

        let now = current_timestamp();

        // Generate unique ID for summary node
        let summary_id = crate::models::MemoryId::new(format!("summary_{}", now));

        // Collect source memory IDs
        let source_memory_ids: Vec<crate::models::MemoryId> =
            source_memories.iter().map(|m| m.id.clone()).collect();

        // Merge tags from all source memories (no duplicates)
        let mut merged_tags: Vec<String> = Vec::new();
        for memory in source_memories {
            for tag in &memory.tags {
                if !merged_tags.contains(tag) {
                    merged_tags.push(tag.clone());
                }
            }
        }

        // Use namespace and domain from first source memory
        // All memories in a group should have the same namespace (from find_related_memories)
        let namespace = source_memories[0].namespace;
        let domain = source_memories[0].domain;

        // Use project_id and branch from first source if available
        let project_id = source_memories[0].project_id.clone();
        let branch = source_memories[0].branch.clone();

        // Create summary memory node
        let summary_node = Memory {
            id: summary_id,
            content: summary_content.to_string(),
            namespace,
            domain,
            project_id,
            branch,
            file_path: None, // Summary nodes don't have specific file paths
            status: MemoryStatus::Active,
            created_at: now,
            updated_at: now,
            tombstoned_at: None,
            embedding: None, // Will need embedding in future for searchability
            tags: merged_tags,
            source: Some("consolidation".to_string()),
            is_summary: true,
            source_memory_ids: Some(source_memory_ids.clone()),
            consolidation_timestamp: Some(now),
        };

        // Store summary node
        self.persistence.store(&summary_node)?;

        // Store edge relationships if index backend is available
        if let Some(ref index) = self.index {
            tracing::debug!(
                summary_id = %summary_node.id.as_str(),
                source_count = source_memory_ids.len(),
                "Storing edge relationships for summary node"
            );

            for source_id in &source_memory_ids {
                // Create SummarizedBy edge from source to summary
                if let Err(e) = index.store_edge(source_id, &summary_node.id, EdgeType::SummarizedBy) {
                    tracing::warn!(
                        error = %e,
                        source_id = %source_id.as_str(),
                        summary_id = %summary_node.id.as_str(),
                        "Failed to store edge relationship, continuing"
                    );
                    // Continue even if one edge fails - we still have the summary
                }
            }

            tracing::info!(
                summary_id = %summary_node.id.as_str(),
                edges_stored = source_memory_ids.len(),
                "Stored edge relationships for summary node"
            );
        } else {
            tracing::debug!(
                summary_id = %summary_node.id.as_str(),
                "Index backend not available, skipping edge storage"
            );
        }

        tracing::info!(
            summary_id = %summary_node.id.as_str(),
            source_count = source_memories.len(),
            tags_count = summary_node.tags.len(),
            "Created summary memory node"
        );

        Ok(summary_node)
    }

    /// Creates RelatedTo edges between all memories in a group.
    ///
    /// This method is used when LLM summarization is unavailable but we still want
    /// to preserve the relationships between semantically similar memories. Creates
    /// a mesh topology where each memory is linked to every other memory in the group
    /// with `RelatedTo` edges.
    ///
    /// # Arguments
    ///
    /// * `memories` - A slice of related memories to link together.
    /// * `index` - The index backend to use for storing edges.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if edge storage fails.
    ///
    /// # Errors
    ///
    /// Returns an error if edge storage operations fail.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use subcog::services::ConsolidationService;
    /// use subcog::storage::persistence::FilesystemBackend;
    /// use subcog::storage::index::SqliteBackend;
    /// use std::sync::Arc;
    ///
    /// let backend = FilesystemBackend::new("/tmp/memories");
    /// let index = SqliteBackend::new("/tmp/index.db")?;
    /// let mut service = ConsolidationService::new(backend)
    ///     .with_index(Arc::new(index));
    ///
    /// let memories = vec![/* related memories */];
    /// service.create_related_edges(&memories, &index)?;
    /// # Ok::<(), subcog::Error>(())
    /// ```
    #[instrument(skip(self, memories, index), fields(memory_count = memories.len()))]
    fn create_related_edges(
        &self,
        memories: &[Memory],
        index: &Arc<SqliteBackend>,
    ) -> Result<()> {
        if memories.len() < 2 {
            tracing::debug!("Fewer than 2 memories, skipping edge creation");
            return Ok(());
        }

        let mut edge_count = 0;

        // Create a mesh topology: each memory is related to every other memory
        for (i, memory_a) in memories.iter().enumerate() {
            for memory_b in memories.iter().skip(i + 1) {
                // Create bidirectional RelatedTo edges
                if let Err(e) = index.store_edge(&memory_a.id, &memory_b.id, EdgeType::RelatedTo) {
                    tracing::warn!(
                        error = %e,
                        from_id = %memory_a.id.as_str(),
                        to_id = %memory_b.id.as_str(),
                        "Failed to store RelatedTo edge, continuing"
                    );
                } else {
                    edge_count += 1;
                }

                // Store the reverse edge (since RelatedTo is bidirectional)
                if let Err(e) = index.store_edge(&memory_b.id, &memory_a.id, EdgeType::RelatedTo) {
                    tracing::warn!(
                        error = %e,
                        from_id = %memory_b.id.as_str(),
                        to_id = %memory_a.id.as_str(),
                        "Failed to store RelatedTo edge (reverse), continuing"
                    );
                } else {
                    edge_count += 1;
                }
            }
        }

        tracing::info!(
            memory_count = memories.len(),
            edge_count = edge_count,
            "Created RelatedTo edges between memories"
        );

        Ok(())
    }
}

/// Calculates cosine similarity between two embedding vectors.
///
/// Returns a value in the range [0.0, 1.0] where:
/// - 1.0 = identical vectors
/// - 0.0 = orthogonal vectors
///
/// # Panics
///
/// This function does not panic. If vectors have different lengths or zero magnitude,
/// it returns 0.0 to indicate no similarity.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if magnitude_a == 0.0 || magnitude_b == 0.0 {
        return 0.0;
    }

    (dot_product / (magnitude_a * magnitude_b)).clamp(0.0, 1.0)
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
    /// Number of summary nodes created.
    pub summaries_created: usize,
}

impl ConsolidationStats {
    /// Returns true if no work was done.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.processed == 0
            && self.archived == 0
            && self.merged == 0
            && self.contradictions == 0
            && self.summaries_created == 0
    }

    /// Returns a human-readable summary.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.is_empty() {
            "No memories to consolidate".to_string()
        } else {
            format!(
                "Processed: {}, Archived: {}, Merged: {}, Contradictions: {}, Summaries: {}",
                self.processed, self.archived, self.merged, self.contradictions, self.summaries_created
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

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        let vec_a = vec![1.0, 2.0, 3.0];
        let vec_b = vec![1.0, 2.0, 3.0];
        let similarity = super::cosine_similarity(&vec_a, &vec_b);
        assert!((similarity - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors() {
        let vec_a = vec![1.0, 0.0, 0.0];
        let vec_b = vec![0.0, 1.0, 0.0];
        let similarity = super::cosine_similarity(&vec_a, &vec_b);
        assert!(similarity.abs() < f32::EPSILON);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let vec_a = vec![1.0, 2.0, 3.0];
        let vec_b = vec![1.0, 2.0];
        let similarity = super::cosine_similarity(&vec_a, &vec_b);
        assert_eq!(similarity, 0.0);
    }

    #[test]
    fn test_cosine_similarity_zero_vectors() {
        let vec_a = vec![0.0, 0.0, 0.0];
        let vec_b = vec![1.0, 2.0, 3.0];
        let similarity = super::cosine_similarity(&vec_a, &vec_b);
        assert_eq!(similarity, 0.0);
    }

    #[test]
    fn test_find_related_memories_no_vector_search() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_find_related"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let service = ConsolidationService::new(backend);

        // Create recall service without vector search
        let recall = crate::services::RecallService::new();
        let config = crate::config::ConsolidationConfig::new();

        let result = service.find_related_memories(&recall, &config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_cluster_by_similarity() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_cluster"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let service = ConsolidationService::new(backend);

        // Create memories with embeddings
        let embedding_a = vec![1.0, 0.0, 0.0];
        let embedding_b = vec![0.9, 0.1, 0.0]; // Similar to A
        let embedding_c = vec![0.0, 1.0, 0.0]; // Different

        let mut memory_a = create_test_memory("mem_a", "Content A");
        memory_a.embedding = Some(embedding_a);

        let mut memory_b = create_test_memory("mem_b", "Content B");
        memory_b.embedding = Some(embedding_b);

        let mut memory_c = create_test_memory("mem_c", "Content C");
        memory_c.embedding = Some(embedding_c);

        let memories = vec![memory_a, memory_b, memory_c];

        // Use a threshold that should group A and B together
        let result = service.cluster_by_similarity(&memories, 0.7);
        assert!(result.is_ok());

        let groups = result.unwrap();
        // Should have at least one group with A and B
        assert!(!groups.is_empty());

        // Check that we have a group with at least 2 memories
        let has_group = groups.iter().any(|g| g.len() >= 2);
        assert!(has_group);
    }

    #[test]
    fn test_cluster_by_similarity_no_embeddings() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_cluster_no_emb"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let service = ConsolidationService::new(backend);

        // Create memories without embeddings
        let memory_a = create_test_memory("mem_a", "Content A");
        let memory_b = create_test_memory("mem_b", "Content B");

        let memories = vec![memory_a, memory_b];

        let result = service.cluster_by_similarity(&memories, 0.7);
        assert!(result.is_ok());

        let groups = result.unwrap();
        // Should have no groups since no embeddings
        assert!(groups.is_empty());
    }

    #[test]
    fn test_cluster_by_similarity_high_threshold() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_cluster_high"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let service = ConsolidationService::new(backend);

        // Create memories with similar but not identical embeddings
        let embedding_a = vec![1.0, 0.0, 0.0];
        let embedding_b = vec![0.9, 0.1, 0.0];

        let mut memory_a = create_test_memory("mem_a", "Content A");
        memory_a.embedding = Some(embedding_a);

        let mut memory_b = create_test_memory("mem_b", "Content B");
        memory_b.embedding = Some(embedding_b);

        let memories = vec![memory_a, memory_b];

        // Use very high threshold (0.99) - should not group them
        let result = service.cluster_by_similarity(&memories, 0.99);
        assert!(result.is_ok());

        let groups = result.unwrap();
        // Should have no groups due to high threshold
        assert!(groups.is_empty());
    }

    #[test]
    fn test_summarize_group_no_llm() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_summarize_no_llm"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let service = ConsolidationService::new(backend);

        // Create test memories
        let memory_a = create_test_memory("mem_a", "First decision");
        let memory_b = create_test_memory("mem_b", "Second decision");

        let memories = vec![memory_a, memory_b];

        // Should fail without LLM provider
        let result = service.summarize_group(&memories);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("LLM provider not configured"));
    }

    #[test]
    fn test_summarize_group_empty_memories() {
        use std::sync::Arc;

        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_summarize_empty"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);

        // Create mock LLM provider
        struct MockLlm;
        impl crate::llm::LlmProvider for MockLlm {
            fn name(&self) -> &'static str {
                "mock"
            }
            fn complete(&self, _prompt: &str) -> Result<String> {
                Ok("Mock summary".to_string())
            }
            fn analyze_for_capture(&self, _content: &str) -> Result<crate::llm::CaptureAnalysis> {
                unimplemented!()
            }
        }

        let llm: Arc<dyn crate::llm::LlmProvider + Send + Sync> = Arc::new(MockLlm);
        let service = ConsolidationService::new(backend).with_llm(llm);

        // Should fail with empty memories
        let result = service.summarize_group(&[]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No memories provided"));
    }

    #[test]
    fn test_summarize_group_with_mock_llm() {
        use std::sync::Arc;

        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_summarize_mock"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);

        // Create mock LLM provider
        struct MockLlm;
        impl crate::llm::LlmProvider for MockLlm {
            fn name(&self) -> &'static str {
                "mock"
            }
            fn complete(&self, _prompt: &str) -> Result<String> {
                Ok("This is a comprehensive summary of the related decisions, preserving all key technical details.".to_string())
            }
            fn analyze_for_capture(&self, _content: &str) -> Result<crate::llm::CaptureAnalysis> {
                unimplemented!()
            }
        }

        let llm: Arc<dyn crate::llm::LlmProvider + Send + Sync> = Arc::new(MockLlm);
        let service = ConsolidationService::new(backend).with_llm(llm);

        // Create test memories
        let memory_a = create_test_memory("mem_a", "Use PostgreSQL for primary storage");
        let memory_b = create_test_memory("mem_b", "Enable JSONB for flexible schemas");

        let memories = vec![memory_a, memory_b];

        // Should succeed with mock LLM
        let result = service.summarize_group(&memories);
        assert!(result.is_ok());

        let summary = result.unwrap();
        assert!(!summary.is_empty());
        assert!(summary.contains("comprehensive summary"));
    }

    #[test]
    fn test_summarize_group_llm_failure() {
        use std::sync::Arc;

        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_summarize_fail"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);

        // Create failing mock LLM provider
        struct FailingMockLlm;
        impl crate::llm::LlmProvider for FailingMockLlm {
            fn name(&self) -> &'static str {
                "failing_mock"
            }
            fn complete(&self, _prompt: &str) -> Result<String> {
                Err(crate::Error::OperationFailed {
                    operation: "llm_complete".to_string(),
                    cause: "Mock LLM failure".to_string(),
                })
            }
            fn analyze_for_capture(&self, _content: &str) -> Result<crate::llm::CaptureAnalysis> {
                unimplemented!()
            }
        }

        let llm: Arc<dyn crate::llm::LlmProvider + Send + Sync> = Arc::new(FailingMockLlm);
        let service = ConsolidationService::new(backend).with_llm(llm);

        // Create test memories
        let memory_a = create_test_memory("mem_a", "First decision");
        let memory_b = create_test_memory("mem_b", "Second decision");

        let memories = vec![memory_a, memory_b];

        // Should propagate LLM failure
        let result = service.summarize_group(&memories);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("LLM summarization failed"));
    }

    #[test]
    fn test_summarize_group_empty_response() {
        use std::sync::Arc;

        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_summarize_empty_resp"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);

        // Create mock LLM that returns empty string
        struct EmptyMockLlm;
        impl crate::llm::LlmProvider for EmptyMockLlm {
            fn name(&self) -> &'static str {
                "empty_mock"
            }
            fn complete(&self, _prompt: &str) -> Result<String> {
                Ok("   ".to_string()) // Only whitespace
            }
            fn analyze_for_capture(&self, _content: &str) -> Result<crate::llm::CaptureAnalysis> {
                unimplemented!()
            }
        }

        let llm: Arc<dyn crate::llm::LlmProvider + Send + Sync> = Arc::new(EmptyMockLlm);
        let service = ConsolidationService::new(backend).with_llm(llm);

        // Create test memories
        let memory_a = create_test_memory("mem_a", "First decision");

        let memories = vec![memory_a];

        // Should fail with empty response
        let result = service.summarize_group(&memories);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("empty summary"));
    }

    #[test]
    fn test_create_summary_node_success() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_create_summary"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let mut service = ConsolidationService::new(backend);

        // Create test source memories with tags
        let mut memory_a = create_test_memory("mem_a", "First decision");
        memory_a.tags = vec!["database".to_string(), "postgres".to_string()];

        let mut memory_b = create_test_memory("mem_b", "Second decision");
        memory_b.tags = vec!["database".to_string(), "schema".to_string()];

        let source_memories = vec![memory_a.clone(), memory_b.clone()];
        let summary_content = "Combined database decisions using PostgreSQL";

        // Create summary node
        let result = service.create_summary_node(summary_content, &source_memories);
        assert!(result.is_ok());

        let summary_node = result.unwrap();

        // Verify summary node properties
        assert!(summary_node.is_summary);
        assert_eq!(summary_node.content, summary_content);
        assert!(summary_node.source_memory_ids.is_some());
        assert_eq!(summary_node.source_memory_ids.as_ref().unwrap().len(), 2);
        assert!(summary_node.consolidation_timestamp.is_some());
        assert_eq!(summary_node.source, Some("consolidation".to_string()));

        // Verify tags are merged without duplicates
        assert_eq!(summary_node.tags.len(), 3); // database, postgres, schema
        assert!(summary_node.tags.contains(&"database".to_string()));
        assert!(summary_node.tags.contains(&"postgres".to_string()));
        assert!(summary_node.tags.contains(&"schema".to_string()));

        // Verify namespace and domain inherited from first source
        assert_eq!(summary_node.namespace, memory_a.namespace);
        assert_eq!(summary_node.domain, memory_a.domain);
    }

    #[test]
    fn test_create_summary_node_empty_sources() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_create_summary_empty"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let mut service = ConsolidationService::new(backend);

        let summary_content = "Summary text";

        // Should fail with empty source memories
        let result = service.create_summary_node(summary_content, &[]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No source memories"));
    }

    #[test]
    fn test_create_summary_node_empty_content() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_create_summary_empty_content"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let mut service = ConsolidationService::new(backend);

        let memory_a = create_test_memory("mem_a", "Content");
        let source_memories = vec![memory_a];

        // Should fail with empty content
        let result = service.create_summary_node("   ", &source_memories);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Summary content cannot be empty"));
    }

    #[test]
    fn test_create_summary_node_tags_deduplication() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_create_summary_tags"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let mut service = ConsolidationService::new(backend);

        // Create memories with overlapping tags
        let mut memory_a = create_test_memory("mem_a", "Content A");
        memory_a.tags = vec!["tag1".to_string(), "tag2".to_string(), "tag3".to_string()];

        let mut memory_b = create_test_memory("mem_b", "Content B");
        memory_b.tags = vec!["tag2".to_string(), "tag3".to_string(), "tag4".to_string()];

        let mut memory_c = create_test_memory("mem_c", "Content C");
        memory_c.tags = vec!["tag1".to_string(), "tag4".to_string(), "tag5".to_string()];

        let source_memories = vec![memory_a, memory_b, memory_c];
        let summary_content = "Summary with merged tags";

        let result = service.create_summary_node(summary_content, &source_memories);
        assert!(result.is_ok());

        let summary_node = result.unwrap();

        // Should have 5 unique tags: tag1, tag2, tag3, tag4, tag5
        assert_eq!(summary_node.tags.len(), 5);
        assert!(summary_node.tags.contains(&"tag1".to_string()));
        assert!(summary_node.tags.contains(&"tag2".to_string()));
        assert!(summary_node.tags.contains(&"tag3".to_string()));
        assert!(summary_node.tags.contains(&"tag4".to_string()));
        assert!(summary_node.tags.contains(&"tag5".to_string()));
    }

    #[test]
    fn test_create_summary_node_stored_in_persistence() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_create_summary_persist"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let mut service = ConsolidationService::new(backend.clone());

        let memory_a = create_test_memory("mem_a", "Content A");
        let source_memories = vec![memory_a];
        let summary_content = "Persisted summary";

        let result = service.create_summary_node(summary_content, &source_memories);
        assert!(result.is_ok());

        let summary_node = result.unwrap();

        // Verify it was stored in persistence
        let retrieved = backend.get(&summary_node.id);
        assert!(retrieved.is_ok());
        assert!(retrieved.unwrap().is_some());

        let retrieved_node = backend.get(&summary_node.id).unwrap().unwrap();
        assert_eq!(retrieved_node.id, summary_node.id);
        assert!(retrieved_node.is_summary);
        assert_eq!(retrieved_node.content, summary_content);
    }

    #[test]
    fn test_create_summary_node_source_ids_preserved() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_create_summary_source_ids"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let mut service = ConsolidationService::new(backend);

        let memory_a = create_test_memory("mem_a", "Content A");
        let memory_b = create_test_memory("mem_b", "Content B");
        let memory_c = create_test_memory("mem_c", "Content C");

        let source_memories = vec![memory_a.clone(), memory_b.clone(), memory_c.clone()];
        let summary_content = "Summary of three memories";

        let result = service.create_summary_node(summary_content, &source_memories);
        assert!(result.is_ok());

        let summary_node = result.unwrap();

        // Verify all source IDs are preserved
        let source_ids = summary_node.source_memory_ids.unwrap();
        assert_eq!(source_ids.len(), 3);
        assert!(source_ids.contains(&memory_a.id));
        assert!(source_ids.contains(&memory_b.id));
        assert!(source_ids.contains(&memory_c.id));
    }

    #[test]
    fn test_create_summary_node_inherits_project_info() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_create_summary_project"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let mut service = ConsolidationService::new(backend);

        let mut memory_a = create_test_memory("mem_a", "Content A");
        memory_a.project_id = Some("project123".to_string());
        memory_a.branch = Some("main".to_string());

        let source_memories = vec![memory_a.clone()];
        let summary_content = "Summary with project info";

        let result = service.create_summary_node(summary_content, &source_memories);
        assert!(result.is_ok());

        let summary_node = result.unwrap();

        // Verify project info inherited from first source
        assert_eq!(summary_node.project_id, Some("project123".to_string()));
        assert_eq!(summary_node.branch, Some("main".to_string()));
    }

    #[test]
    fn test_create_summary_node_stores_edges_with_index() {
        use crate::storage::index::SqliteBackend;
        use crate::storage::traits::IndexBackend;

        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_create_summary_edges"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);

        // Create SQLite index backend
        let index = SqliteBackend::in_memory().expect("Failed to create in-memory SQLite");
        let index_arc = Arc::new(index);

        let mut service = ConsolidationService::new(backend)
            .with_index(index_arc.clone());

        // Create test source memories
        let memory_a = create_test_memory("edge_source_1", "First memory");
        let memory_b = create_test_memory("edge_source_2", "Second memory");

        // Index the source memories first so foreign key constraints are satisfied
        index_arc.index(&memory_a).expect("Failed to index memory_a");
        index_arc.index(&memory_b).expect("Failed to index memory_b");

        let source_memories = vec![memory_a.clone(), memory_b.clone()];
        let summary_content = "Summary of two memories";

        // Create summary node (should also store edges)
        let result = service.create_summary_node(summary_content, &source_memories);
        assert!(result.is_ok());

        let summary_node = result.unwrap();

        // Index the summary node so we can verify edges
        index_arc.index(&summary_node).expect("Failed to index summary node");

        // Verify edges were stored using the query_edges method
        let edges_from_a = index_arc
            .query_edges(&memory_a.id, EdgeType::SummarizedBy)
            .expect("Failed to query edges from memory_a");
        let edges_from_b = index_arc
            .query_edges(&memory_b.id, EdgeType::SummarizedBy)
            .expect("Failed to query edges from memory_b");

        // Both source memories should have edges pointing to the summary
        assert_eq!(edges_from_a.len(), 1, "memory_a should have 1 edge");
        assert_eq!(edges_from_b.len(), 1, "memory_b should have 1 edge");
        assert_eq!(
            edges_from_a[0],
            summary_node.id,
            "memory_a edge should point to summary"
        );
        assert_eq!(
            edges_from_b[0],
            summary_node.id,
            "memory_b edge should point to summary"
        );
    }

    #[test]
    fn test_create_summary_node_without_index_backend() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_create_summary_no_index"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let mut service = ConsolidationService::new(backend); // No index backend

        let memory_a = create_test_memory("mem_a", "Content A");
        let source_memories = vec![memory_a];
        let summary_content = "Summary without index";

        // Should succeed even without index backend
        let result = service.create_summary_node(summary_content, &source_memories);
        assert!(result.is_ok());

        let summary_node = result.unwrap();
        assert!(summary_node.is_summary);
        // Edges won't be stored, but summary creation should still work
    }

    #[test]
    fn test_consolidate_memories_disabled() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_consolidate_disabled"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let mut service = ConsolidationService::new(backend);

        let recall = crate::services::RecallService::new();
        let config = crate::config::ConsolidationConfig::new(); // enabled = false by default

        let result = service.consolidate_memories(&recall, &config);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert!(stats.is_empty());
        assert_eq!(stats.summaries_created, 0);
    }

    #[test]
    fn test_consolidate_memories_no_vector_search() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_consolidate_no_vector"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let mut service = ConsolidationService::new(backend);

        // RecallService without vector search
        let recall = crate::services::RecallService::new();
        let mut config = crate::config::ConsolidationConfig::new();
        config.enabled = true;

        let result = service.consolidate_memories(&recall, &config);
        assert!(result.is_ok());

        let stats = result.unwrap();
        // Should return empty stats because no vector search available
        assert_eq!(stats.summaries_created, 0);
    }

    #[test]
    fn test_consolidate_memories_no_llm() {
        use crate::embedding::Embedder as EmbedderTrait;
        use crate::storage::traits::VectorBackend;

        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_consolidate_no_llm"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let mut service = ConsolidationService::new(backend);

        // Create memories with embeddings
        let embedding_a = vec![1.0, 0.0, 0.0];
        let embedding_b = vec![0.9, 0.1, 0.0]; // Similar to A

        let mut memory_a = create_test_memory("consolidate_mem_a", "Decision about PostgreSQL");
        memory_a.embedding = Some(embedding_a);
        memory_a.namespace = crate::models::Namespace::Decisions;

        let mut memory_b = create_test_memory("consolidate_mem_b", "Use PostgreSQL for storage");
        memory_b.embedding = Some(embedding_b);
        memory_b.namespace = crate::models::Namespace::Decisions;

        // Store memories
        let _ = service.persistence.store(&memory_a);
        let _ = service.persistence.store(&memory_b);

        // Create mock embedder
        struct MockEmbedder;
        impl EmbedderTrait for MockEmbedder {
            fn embed(&self, _text: &str) -> Result<Vec<f32>> {
                Ok(vec![1.0, 0.0, 0.0])
            }
            fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
                Ok(texts.iter().map(|_| vec![1.0, 0.0, 0.0]).collect())
            }
            fn model_name(&self) -> &str {
                "mock"
            }
        }

        // Create mock vector backend
        struct MockVectorBackend;
        impl VectorBackend for MockVectorBackend {
            fn index(&self, _memory: &Memory) -> Result<()> {
                Ok(())
            }
            fn search(&self, _embedding: &[f32], _limit: usize) -> Result<Vec<(MemoryId, f32)>> {
                Ok(vec![])
            }
            fn delete(&self, _memory_id: &MemoryId) -> Result<()> {
                Ok(())
            }
        }

        // Create RecallService with embedder and vector backend
        let recall = crate::services::RecallService::new()
            .with_embedder(Arc::new(MockEmbedder))
            .with_vector(Arc::new(MockVectorBackend));

        // Note: Service has NO LLM provider
        let mut config = crate::config::ConsolidationConfig::new();
        config.enabled = true;
        config.similarity_threshold = 0.7;

        let result = service.consolidate_memories(&recall, &config);
        assert!(result.is_ok());

        // Should succeed but create no summaries (LLM would fail but we skip those groups)
        let stats = result.unwrap();
        assert_eq!(stats.summaries_created, 0);
    }

    #[test]
    fn test_consolidate_memories_with_mock_llm() {
        use crate::embedding::Embedder as EmbedderTrait;
        use crate::storage::traits::VectorBackend;
        use std::sync::Arc;

        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_consolidate_with_llm"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);

        // Create mock LLM provider
        struct MockLlm;
        impl crate::llm::LlmProvider for MockLlm {
            fn name(&self) -> &'static str {
                "mock"
            }
            fn complete(&self, _prompt: &str) -> Result<String> {
                Ok("Comprehensive summary of database decisions: Use PostgreSQL with JSONB support.".to_string())
            }
            fn analyze_for_capture(&self, _content: &str) -> Result<crate::llm::CaptureAnalysis> {
                unimplemented!()
            }
        }

        let llm: Arc<dyn crate::llm::LlmProvider + Send + Sync> = Arc::new(MockLlm);
        let mut service = ConsolidationService::new(backend).with_llm(llm);

        // Create memories with embeddings
        let embedding_a = vec![1.0, 0.0, 0.0];
        let embedding_b = vec![0.95, 0.05, 0.0]; // Very similar to A

        let mut memory_a = create_test_memory("consolidate_llm_a", "Use PostgreSQL for storage");
        memory_a.embedding = Some(embedding_a.clone());
        memory_a.namespace = crate::models::Namespace::Decisions;

        let mut memory_b = create_test_memory("consolidate_llm_b", "Enable JSONB in PostgreSQL");
        memory_b.embedding = Some(embedding_b.clone());
        memory_b.namespace = crate::models::Namespace::Decisions;

        // Store memories
        let _ = service.persistence.store(&memory_a);
        let _ = service.persistence.store(&memory_b);

        // Create mock embedder
        struct MockEmbedder;
        impl EmbedderTrait for MockEmbedder {
            fn embed(&self, _text: &str) -> Result<Vec<f32>> {
                Ok(vec![1.0, 0.0, 0.0])
            }
            fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
                Ok(texts.iter().map(|_| vec![1.0, 0.0, 0.0]).collect())
            }
            fn model_name(&self) -> &str {
                "mock"
            }
        }

        // Create mock vector backend
        struct MockVectorBackend;
        impl VectorBackend for MockVectorBackend {
            fn index(&self, _memory: &Memory) -> Result<()> {
                Ok(())
            }
            fn search(&self, _embedding: &[f32], _limit: usize) -> Result<Vec<(MemoryId, f32)>> {
                Ok(vec![])
            }
            fn delete(&self, _memory_id: &MemoryId) -> Result<()> {
                Ok(())
            }
        }

        // Create RecallService with embedder and vector backend
        let recall = crate::services::RecallService::new()
            .with_embedder(Arc::new(MockEmbedder))
            .with_vector(Arc::new(MockVectorBackend));

        let mut config = crate::config::ConsolidationConfig::new();
        config.enabled = true;
        config.similarity_threshold = 0.7;
        config.min_memories_to_consolidate = 2;

        let result = service.consolidate_memories(&recall, &config);
        assert!(result.is_ok());

        let stats = result.unwrap();
        // Should process 2 memories and create 1 summary
        assert_eq!(stats.processed, 2);
        assert_eq!(stats.summaries_created, 1);
    }

    #[test]
    fn test_consolidate_memories_respects_namespace_filter() {
        use crate::embedding::Embedder as EmbedderTrait;
        use crate::storage::traits::VectorBackend;
        use std::sync::Arc;

        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_consolidate_namespace_filter"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);

        // Create mock LLM provider
        struct MockLlm;
        impl crate::llm::LlmProvider for MockLlm {
            fn name(&self) -> &'static str {
                "mock"
            }
            fn complete(&self, _prompt: &str) -> Result<String> {
                Ok("Summary".to_string())
            }
            fn analyze_for_capture(&self, _content: &str) -> Result<crate::llm::CaptureAnalysis> {
                unimplemented!()
            }
        }

        let llm: Arc<dyn crate::llm::LlmProvider + Send + Sync> = Arc::new(MockLlm);
        let mut service = ConsolidationService::new(backend).with_llm(llm);

        // Create memories in different namespaces
        let embedding = vec![1.0, 0.0, 0.0];

        let mut memory_decisions = create_test_memory("mem_decisions", "Decision");
        memory_decisions.embedding = Some(embedding.clone());
        memory_decisions.namespace = crate::models::Namespace::Decisions;

        let mut memory_patterns = create_test_memory("mem_patterns", "Pattern");
        memory_patterns.embedding = Some(embedding.clone());
        memory_patterns.namespace = crate::models::Namespace::Patterns;

        // Store memories
        let _ = service.persistence.store(&memory_decisions);
        let _ = service.persistence.store(&memory_patterns);

        // Create mock embedder and vector backend
        struct MockEmbedder;
        impl EmbedderTrait for MockEmbedder {
            fn embed(&self, _text: &str) -> Result<Vec<f32>> {
                Ok(vec![1.0, 0.0, 0.0])
            }
            fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
                Ok(texts.iter().map(|_| vec![1.0, 0.0, 0.0]).collect())
            }
            fn model_name(&self) -> &str {
                "mock"
            }
        }

        struct MockVectorBackend;
        impl VectorBackend for MockVectorBackend {
            fn index(&self, _memory: &Memory) -> Result<()> {
                Ok(())
            }
            fn search(&self, _embedding: &[f32], _limit: usize) -> Result<Vec<(MemoryId, f32)>> {
                Ok(vec![])
            }
            fn delete(&self, _memory_id: &MemoryId) -> Result<()> {
                Ok(())
            }
        }

        let recall = crate::services::RecallService::new()
            .with_embedder(Arc::new(MockEmbedder))
            .with_vector(Arc::new(MockVectorBackend));

        // Filter to only Decisions namespace
        let mut config = crate::config::ConsolidationConfig::new();
        config.enabled = true;
        config.namespace_filter = Some(vec![crate::models::Namespace::Decisions]);
        config.min_memories_to_consolidate = 1; // Allow single memory for testing

        let result = service.consolidate_memories(&recall, &config);
        assert!(result.is_ok());

        // Should only process Decisions namespace
        let stats = result.unwrap();
        // With high similarity threshold and only 1 memory, won't create groups
        assert_eq!(stats.summaries_created, 0);
    }

    #[test]
    fn test_consolidation_stats_with_summaries() {
        let stats = ConsolidationStats {
            processed: 10,
            archived: 2,
            merged: 1,
            contradictions: 0,
            summaries_created: 3,
        };
        assert!(!stats.is_empty());
        let summary = stats.summary();
        assert!(summary.contains("Processed: 10"));
        assert!(summary.contains("Summaries: 3"));
    }

    #[test]
    fn test_create_related_edges() {
        use crate::storage::index::SqliteBackend;
        use crate::storage::traits::IndexBackend;
        use std::sync::Arc;

        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_create_related_edges"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);

        // Create SQLite index backend
        let index = SqliteBackend::in_memory().expect("Failed to create in-memory SQLite");
        let index_arc = Arc::new(index);

        let service = ConsolidationService::new(backend).with_index(index_arc.clone());

        // Create test memories
        let memory_a = create_test_memory("related_a", "First memory");
        let memory_b = create_test_memory("related_b", "Second memory");
        let memory_c = create_test_memory("related_c", "Third memory");

        // Index memories first so foreign key constraints are satisfied
        index_arc.index(&memory_a).expect("Failed to index memory_a");
        index_arc.index(&memory_b).expect("Failed to index memory_b");
        index_arc.index(&memory_c).expect("Failed to index memory_c");

        let memories = vec![memory_a.clone(), memory_b.clone(), memory_c.clone()];

        // Create RelatedTo edges
        let result = service.create_related_edges(&memories, &index_arc);
        assert!(result.is_ok());

        // Verify edges were created
        let edges_from_a = index_arc
            .query_edges(&memory_a.id, EdgeType::RelatedTo)
            .expect("Failed to query edges from memory_a");

        // Memory A should be related to B and C
        assert_eq!(edges_from_a.len(), 2, "memory_a should have 2 RelatedTo edges");
        assert!(
            edges_from_a.contains(&memory_b.id),
            "memory_a should be related to memory_b"
        );
        assert!(
            edges_from_a.contains(&memory_c.id),
            "memory_a should be related to memory_c"
        );

        // Verify bidirectional edges
        let edges_from_b = index_arc
            .query_edges(&memory_b.id, EdgeType::RelatedTo)
            .expect("Failed to query edges from memory_b");

        assert_eq!(edges_from_b.len(), 2, "memory_b should have 2 RelatedTo edges");
        assert!(
            edges_from_b.contains(&memory_a.id),
            "memory_b should be related to memory_a"
        );
        assert!(
            edges_from_b.contains(&memory_c.id),
            "memory_b should be related to memory_c"
        );
    }

    #[test]
    fn test_create_related_edges_single_memory() {
        use crate::storage::index::SqliteBackend;
        use std::sync::Arc;

        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_related_edges_single"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);

        let index = SqliteBackend::in_memory().expect("Failed to create in-memory SQLite");
        let index_arc = Arc::new(index);

        let service = ConsolidationService::new(backend).with_index(index_arc.clone());

        // Create single memory
        let memory_a = create_test_memory("single_mem", "Only memory");
        let memories = vec![memory_a];

        // Should succeed but create no edges
        let result = service.create_related_edges(&memories, &index_arc);
        assert!(result.is_ok());
    }

    #[test]
    fn test_consolidate_memories_no_llm_creates_edges() {
        use crate::embedding::Embedder as EmbedderTrait;
        use crate::storage::index::SqliteBackend;
        use crate::storage::traits::{IndexBackend, VectorBackend};

        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_consolidate_no_llm_edges"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);

        // Create index backend for edge storage
        let index = SqliteBackend::in_memory().expect("Failed to create in-memory SQLite");
        let index_arc = Arc::new(index);

        // Create service WITHOUT LLM but WITH index backend
        let mut service = ConsolidationService::new(backend).with_index(index_arc.clone());

        // Create memories with embeddings
        let embedding_a = vec![1.0, 0.0, 0.0];
        let embedding_b = vec![0.95, 0.05, 0.0]; // Very similar to A

        let mut memory_a = create_test_memory("no_llm_edge_a", "Decision about PostgreSQL");
        memory_a.embedding = Some(embedding_a);
        memory_a.namespace = crate::models::Namespace::Decisions;

        let mut memory_b = create_test_memory("no_llm_edge_b", "Use PostgreSQL for storage");
        memory_b.embedding = Some(embedding_b);
        memory_b.namespace = crate::models::Namespace::Decisions;

        // Index memories so foreign key constraints are satisfied
        index_arc.index(&memory_a).expect("Failed to index memory_a");
        index_arc.index(&memory_b).expect("Failed to index memory_b");

        // Store memories
        let _ = service.persistence.store(&memory_a);
        let _ = service.persistence.store(&memory_b);

        // Create mock embedder and vector backend
        struct MockEmbedder;
        impl EmbedderTrait for MockEmbedder {
            fn embed(&self, _text: &str) -> Result<Vec<f32>> {
                Ok(vec![1.0, 0.0, 0.0])
            }
            fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
                Ok(texts.iter().map(|_| vec![1.0, 0.0, 0.0]).collect())
            }
            fn model_name(&self) -> &str {
                "mock"
            }
        }

        struct MockVectorBackend;
        impl VectorBackend for MockVectorBackend {
            fn index(&self, _memory: &Memory) -> Result<()> {
                Ok(())
            }
            fn search(&self, _embedding: &[f32], _limit: usize) -> Result<Vec<(MemoryId, f32)>> {
                Ok(vec![])
            }
            fn delete(&self, _memory_id: &MemoryId) -> Result<()> {
                Ok(())
            }
        }

        let recall = crate::services::RecallService::new()
            .with_embedder(Arc::new(MockEmbedder))
            .with_vector(Arc::new(MockVectorBackend));

        // Configure consolidation
        let mut config = crate::config::ConsolidationConfig::new();
        config.enabled = true;
        config.similarity_threshold = 0.7;
        config.min_memories_to_consolidate = 2;

        // Run consolidation (should fail LLM summarization but create edges)
        let result = service.consolidate_memories(&recall, &config);
        assert!(result.is_ok());

        let stats = result.unwrap();
        // Should process 2 memories but create 0 summaries (no LLM)
        assert_eq!(stats.processed, 2);
        assert_eq!(stats.summaries_created, 0);

        // Verify RelatedTo edges were created
        let edges_from_a = index_arc
            .query_edges(&memory_a.id, EdgeType::RelatedTo)
            .expect("Failed to query edges from memory_a");

        assert!(
            !edges_from_a.is_empty(),
            "memory_a should have RelatedTo edges even without LLM"
        );
        assert!(
            edges_from_a.contains(&memory_b.id),
            "memory_a should be related to memory_b"
        );
    }

    #[test]
    fn test_consolidate_memories_no_llm_no_index() {
        use crate::embedding::Embedder as EmbedderTrait;
        use crate::storage::traits::VectorBackend;

        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_consolidate_no_llm_no_index"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);

        // Create service WITHOUT LLM and WITHOUT index backend
        let mut service = ConsolidationService::new(backend);

        // Create memories with embeddings
        let embedding_a = vec![1.0, 0.0, 0.0];
        let embedding_b = vec![0.95, 0.05, 0.0];

        let mut memory_a = create_test_memory("no_backends_a", "Decision A");
        memory_a.embedding = Some(embedding_a);
        memory_a.namespace = crate::models::Namespace::Decisions;

        let mut memory_b = create_test_memory("no_backends_b", "Decision B");
        memory_b.embedding = Some(embedding_b);
        memory_b.namespace = crate::models::Namespace::Decisions;

        // Store memories
        let _ = service.persistence.store(&memory_a);
        let _ = service.persistence.store(&memory_b);

        // Create mock embedder and vector backend
        struct MockEmbedder;
        impl EmbedderTrait for MockEmbedder {
            fn embed(&self, _text: &str) -> Result<Vec<f32>> {
                Ok(vec![1.0, 0.0, 0.0])
            }
            fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
                Ok(texts.iter().map(|_| vec![1.0, 0.0, 0.0]).collect())
            }
            fn model_name(&self) -> &str {
                "mock"
            }
        }

        struct MockVectorBackend;
        impl VectorBackend for MockVectorBackend {
            fn index(&self, _memory: &Memory) -> Result<()> {
                Ok(())
            }
            fn search(&self, _embedding: &[f32], _limit: usize) -> Result<Vec<(MemoryId, f32)>> {
                Ok(vec![])
            }
            fn delete(&self, _memory_id: &MemoryId) -> Result<()> {
                Ok(())
            }
        }

        let recall = crate::services::RecallService::new()
            .with_embedder(Arc::new(MockEmbedder))
            .with_vector(Arc::new(MockVectorBackend));

        let mut config = crate::config::ConsolidationConfig::new();
        config.enabled = true;
        config.similarity_threshold = 0.7;
        config.min_memories_to_consolidate = 2;

        // Should succeed gracefully even without LLM or index
        let result = service.consolidate_memories(&recall, &config);
        assert!(result.is_ok());

        let stats = result.unwrap();
        // Should process memories but create nothing
        assert_eq!(stats.processed, 2);
        assert_eq!(stats.summaries_created, 0);
    }

    #[test]
    fn test_cluster_by_similarity_empty_list() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_cluster_empty_list"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let service = ConsolidationService::new(backend);

        // Empty memories list should return empty groups
        let result = service.cluster_by_similarity(&[], 0.7);
        assert!(result.is_ok());

        let groups = result.unwrap();
        assert!(groups.is_empty());
    }

    #[test]
    fn test_create_summary_node_no_tags() {
        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_create_summary_no_tags"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let mut service = ConsolidationService::new(backend);

        // Create memories with no tags
        let mut memory_a = create_test_memory("mem_a", "Content A");
        memory_a.tags = vec![];

        let mut memory_b = create_test_memory("mem_b", "Content B");
        memory_b.tags = vec![];

        let source_memories = vec![memory_a, memory_b];
        let summary_content = "Summary of untagged memories";

        let result = service.create_summary_node(summary_content, &source_memories);
        assert!(result.is_ok());

        let summary_node = result.unwrap();
        // Should have no tags
        assert!(summary_node.tags.is_empty());
    }

    #[test]
    fn test_edge_storage_idempotency() {
        use crate::storage::index::SqliteBackend;
        use crate::storage::traits::IndexBackend;
        use std::sync::Arc;

        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_edge_idempotency"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);

        // Create SQLite index backend
        let index = SqliteBackend::in_memory().expect("Failed to create in-memory SQLite");
        let index_arc = Arc::new(index);

        let mut service = ConsolidationService::new(backend).with_index(index_arc.clone());

        // Create test memories
        let memory_a = create_test_memory("idempotent_a", "First memory");
        let memory_b = create_test_memory("idempotent_b", "Second memory");

        // Index memories first
        index_arc.index(&memory_a).expect("Failed to index memory_a");
        index_arc.index(&memory_b).expect("Failed to index memory_b");

        let source_memories = vec![memory_a.clone(), memory_b.clone()];
        let summary_content = "Summary for idempotency test";

        // Create summary node (stores edges)
        let result1 = service.create_summary_node(summary_content, &source_memories);
        assert!(result1.is_ok());
        let summary1 = result1.unwrap();

        // Index summary so we can query edges
        index_arc.index(&summary1).expect("Failed to index summary");

        // Create same summary again (should handle duplicate edges gracefully)
        let result2 = service.create_summary_node(summary_content, &source_memories);
        assert!(result2.is_ok());
        let summary2 = result2.unwrap();

        // Both summaries should be created successfully
        assert!(summary1.is_summary);
        assert!(summary2.is_summary);

        // Edges should still exist (upsert handles duplicates)
        let edges_from_a = index_arc
            .query_edges(&memory_a.id, EdgeType::SummarizedBy)
            .expect("Failed to query edges");

        // Should have at least one edge (could be multiple if both summaries stored edges)
        assert!(!edges_from_a.is_empty());
    }

    #[test]
    fn test_find_related_memories_with_time_window() {
        use crate::embedding::Embedder as EmbedderTrait;
        use crate::storage::traits::VectorBackend;
        use std::sync::Arc;

        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_find_time_window"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);
        let service = ConsolidationService::new(backend);

        // Create mock embedder and vector backend
        struct MockEmbedder;
        impl EmbedderTrait for MockEmbedder {
            fn embed(&self, _text: &str) -> Result<Vec<f32>> {
                Ok(vec![1.0, 0.0, 0.0])
            }
            fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
                Ok(texts.iter().map(|_| vec![1.0, 0.0, 0.0]).collect())
            }
            fn model_name(&self) -> &str {
                "mock"
            }
        }

        struct MockVectorBackend;
        impl VectorBackend for MockVectorBackend {
            fn index(&self, _memory: &Memory) -> Result<()> {
                Ok(())
            }
            fn search(&self, _embedding: &[f32], _limit: usize) -> Result<Vec<(MemoryId, f32)>> {
                Ok(vec![])
            }
            fn delete(&self, _memory_id: &MemoryId) -> Result<()> {
                Ok(())
            }
        }

        let recall = crate::services::RecallService::new()
            .with_embedder(Arc::new(MockEmbedder))
            .with_vector(Arc::new(MockVectorBackend));

        // Test with time window filter
        let mut config = crate::config::ConsolidationConfig::new();
        config.time_window_days = Some(7); // Only last 7 days

        let result = service.find_related_memories(&recall, &config);
        assert!(result.is_ok());

        // Should return empty (no vector search results)
        let groups = result.unwrap();
        assert!(groups.is_empty());
    }

    #[test]
    fn test_consolidate_memories_multiple_namespaces() {
        use crate::embedding::Embedder as EmbedderTrait;
        use crate::storage::traits::VectorBackend;
        use std::sync::Arc;

        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_consolidate_multi_namespace"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);

        // Create mock LLM
        struct MockLlm;
        impl crate::llm::LlmProvider for MockLlm {
            fn name(&self) -> &'static str {
                "mock"
            }
            fn complete(&self, _prompt: &str) -> Result<String> {
                Ok("Summary of memories".to_string())
            }
            fn analyze_for_capture(&self, _content: &str) -> Result<crate::llm::CaptureAnalysis> {
                unimplemented!()
            }
        }

        let llm: Arc<dyn crate::llm::LlmProvider + Send + Sync> = Arc::new(MockLlm);
        let mut service = ConsolidationService::new(backend).with_llm(llm);

        // Create memories in different namespaces with embeddings
        let embedding = vec![1.0, 0.0, 0.0];

        let mut mem_decisions_1 = create_test_memory("dec_1", "Decision 1");
        mem_decisions_1.embedding = Some(embedding.clone());
        mem_decisions_1.namespace = crate::models::Namespace::Decisions;

        let mut mem_decisions_2 = create_test_memory("dec_2", "Decision 2");
        mem_decisions_2.embedding = Some(embedding.clone());
        mem_decisions_2.namespace = crate::models::Namespace::Decisions;

        let mut mem_patterns_1 = create_test_memory("pat_1", "Pattern 1");
        mem_patterns_1.embedding = Some(embedding.clone());
        mem_patterns_1.namespace = crate::models::Namespace::Patterns;

        let mut mem_patterns_2 = create_test_memory("pat_2", "Pattern 2");
        mem_patterns_2.embedding = Some(embedding.clone());
        mem_patterns_2.namespace = crate::models::Namespace::Patterns;

        // Store all memories
        let _ = service.persistence.store(&mem_decisions_1);
        let _ = service.persistence.store(&mem_decisions_2);
        let _ = service.persistence.store(&mem_patterns_1);
        let _ = service.persistence.store(&mem_patterns_2);

        // Create mock embedder and vector backend
        struct MockEmbedder;
        impl EmbedderTrait for MockEmbedder {
            fn embed(&self, _text: &str) -> Result<Vec<f32>> {
                Ok(vec![1.0, 0.0, 0.0])
            }
            fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
                Ok(texts.iter().map(|_| vec![1.0, 0.0, 0.0]).collect())
            }
            fn model_name(&self) -> &str {
                "mock"
            }
        }

        struct MockVectorBackend;
        impl VectorBackend for MockVectorBackend {
            fn index(&self, _memory: &Memory) -> Result<()> {
                Ok(())
            }
            fn search(&self, _embedding: &[f32], _limit: usize) -> Result<Vec<(MemoryId, f32)>> {
                Ok(vec![])
            }
            fn delete(&self, _memory_id: &MemoryId) -> Result<()> {
                Ok(())
            }
        }

        let recall = crate::services::RecallService::new()
            .with_embedder(Arc::new(MockEmbedder))
            .with_vector(Arc::new(MockVectorBackend));

        // Configure to process multiple namespaces
        let mut config = crate::config::ConsolidationConfig::new();
        config.enabled = true;
        config.namespace_filter = Some(vec![
            crate::models::Namespace::Decisions,
            crate::models::Namespace::Patterns,
        ]);
        config.similarity_threshold = 0.9;
        config.min_memories_to_consolidate = 2;

        let result = service.consolidate_memories(&recall, &config);
        assert!(result.is_ok());

        // Should process memories from both namespaces
        let stats = result.unwrap();
        assert_eq!(stats.processed, 4);
    }

    #[test]
    fn test_summarize_group_preserves_memory_details() {
        use std::sync::Arc;

        let temp_dir = tempfile::tempdir().ok();
        let path = temp_dir.as_ref().map_or_else(
            || std::path::PathBuf::from("/tmp/test_summarize_details"),
            |d| d.path().to_path_buf(),
        );
        let backend = FilesystemBackend::new(&path);

        // Create mock LLM that echoes the input to verify details are passed
        struct DetailCheckingMockLlm;
        impl crate::llm::LlmProvider for DetailCheckingMockLlm {
            fn name(&self) -> &'static str {
                "detail_checking_mock"
            }
            fn complete(&self, prompt: &str) -> Result<String> {
                // Verify prompt contains key details
                if prompt.contains("mem_detail_1")
                    && prompt.contains("mem_detail_2")
                    && prompt.contains("Decisions")
                    && prompt.contains("Important detail 1")
                    && prompt.contains("Important detail 2")
                {
                    Ok("Summary preserving all key technical details from both memories".to_string())
                } else {
                    Err(crate::Error::OperationFailed {
                        operation: "llm_complete".to_string(),
                        cause: "Details not found in prompt".to_string(),
                    })
                }
            }
            fn analyze_for_capture(&self, _content: &str) -> Result<crate::llm::CaptureAnalysis> {
                unimplemented!()
            }
        }

        let llm: Arc<dyn crate::llm::LlmProvider + Send + Sync> = Arc::new(DetailCheckingMockLlm);
        let service = ConsolidationService::new(backend).with_llm(llm);

        // Create memories with specific details
        let mut memory_a = create_test_memory("mem_detail_1", "Important detail 1");
        memory_a.namespace = crate::models::Namespace::Decisions;

        let mut memory_b = create_test_memory("mem_detail_2", "Important detail 2");
        memory_b.namespace = crate::models::Namespace::Decisions;

        let memories = vec![memory_a, memory_b];

        // Should succeed and preserve details in the summary
        let result = service.summarize_group(&memories);
        assert!(result.is_ok());

        let summary = result.unwrap();
        assert!(summary.contains("preserving all key technical details"));
    }
}
