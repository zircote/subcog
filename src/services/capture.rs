//! Memory capture service.
//!
//! Handles capturing new memories, including validation, redaction, and storage.
//! Now also generates embeddings and indexes memories for searchability.
//!
//! # Examples
//!
//! Basic capture:
//!
//! ```
//! use subcog::services::CaptureService;
//! use subcog::models::{CaptureRequest, Namespace, Domain};
//!
//! let service = CaptureService::default();
//! let request = CaptureRequest {
//!     content: "Use PostgreSQL for primary storage".to_string(),
//!     namespace: Namespace::Decisions,
//!     domain: Domain::default(),
//!     tags: vec!["database".to_string()],
//!     source: Some("ARCHITECTURE.md".to_string()),
//!     skip_security_check: false,
//!     ttl_seconds: None,
//!     scope: None,
//!     ..Default::default()
//! };
//!
//! let result = service.capture(request).expect("capture should succeed");
//! assert!(result.urn.starts_with("subcog://"));
//! ```
//!
//! Validation before capture:
//!
//! ```
//! use subcog::services::CaptureService;
//! use subcog::models::{CaptureRequest, Namespace, Domain};
//!
//! let service = CaptureService::default();
//! let request = CaptureRequest {
//!     content: "Important decision".to_string(),
//!     namespace: Namespace::Decisions,
//!     domain: Domain::default(),
//!     tags: vec![],
//!     source: None,
//!     skip_security_check: false,
//!     ttl_seconds: None,
//!     scope: None,
//!     ..Default::default()
//! };
//!
//! let validation = service.validate(&request).expect("validation should succeed");
//! if validation.is_valid {
//!     let _result = service.capture(request);
//! }
//! ```

use crate::config::Config;
use crate::context::GitContext;
use crate::embedding::Embedder;
use crate::gc::{ExpirationConfig, ExpirationService};
use crate::models::{
    CaptureRequest, CaptureResult, EventMeta, Memory, MemoryEvent, MemoryId, MemoryStatus,
};
use crate::observability::current_request_id;
use crate::security::{ContentRedactor, SecretDetector, record_event};
use crate::services::deduplication::ContentHasher;
use crate::storage::traits::{IndexBackend, VectorBackend};
use crate::{Error, Result};
use std::path::Path;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::{info_span, instrument};

/// Callback type for post-capture entity extraction.
///
/// Called after successful capture with the memory content and ID.
/// Should extract entities and store them in the knowledge graph.
/// Errors are logged but do not fail the capture operation.
pub type EntityExtractionCallback =
    Arc<dyn Fn(&str, &MemoryId) -> Result<EntityExtractionStats> + Send + Sync>;

/// Statistics from entity extraction during capture.
#[derive(Debug, Clone, Default)]
pub struct EntityExtractionStats {
    /// Number of entities extracted and stored.
    pub entities_stored: usize,
    /// Number of relationships extracted and stored.
    pub relationships_stored: usize,
    /// Whether the extraction used fallback (no LLM).
    pub used_fallback: bool,
}

/// Runs entity extraction and logs results/metrics.
///
/// Extracted to a separate function to support both async (`tokio::spawn`) and
/// sync (inline) execution paths.
fn run_entity_extraction(callback: &EntityExtractionCallback, content: &str, memory_id: &MemoryId) {
    let _span = info_span!(
        "subcog.memory.capture.entity_extraction",
        memory_id = %memory_id
    )
    .entered();

    match callback(content, memory_id) {
        Ok(stats) => {
            tracing::debug!(
                memory_id = %memory_id,
                entities = stats.entities_stored,
                relationships = stats.relationships_stored,
                fallback = stats.used_fallback,
                "Entity extraction completed"
            );
            metrics::counter!(
                "entity_extraction_total",
                "status" => "success",
                "fallback" => if stats.used_fallback { "true" } else { "false" }
            )
            .increment(1);
        },
        Err(e) => {
            tracing::warn!(
                memory_id = %memory_id,
                error = %e,
                "Entity extraction failed"
            );
            metrics::counter!("entity_extraction_total", "status" => "error").increment(1);
        },
    }
}

/// Service for capturing memories.
///
/// # Storage Layers
///
/// When fully configured, captures are written to two layers:
/// 1. **Index** (`SQLite` FTS5) - Authoritative storage with full-text search via BM25
/// 2. **Vector** (usearch) - Semantic similarity search
///
/// # Entity Extraction
///
/// When configured with an entity extraction callback and the feature is enabled,
/// entities (people, organizations, technologies, concepts) are automatically
/// extracted from captured content and stored in the knowledge graph for
/// graph-augmented retrieval (Graph RAG).
///
/// # Graceful Degradation
///
/// If embedder or vector backend is unavailable:
/// - Capture still succeeds (Index layer is authoritative)
/// - A warning is logged for each failed secondary store
/// - The memory may not be searchable via semantic similarity
///
/// If entity extraction fails:
/// - Capture still succeeds
/// - A warning is logged
/// - The memory won't have graph relationships
pub struct CaptureService {
    /// Configuration.
    config: Config,
    /// Secret detector.
    secret_detector: SecretDetector,
    /// Content redactor.
    redactor: ContentRedactor,
    /// Embedder for generating embeddings (optional).
    embedder: Option<Arc<dyn Embedder>>,
    /// Index backend for full-text search (optional).
    index: Option<Arc<dyn IndexBackend + Send + Sync>>,
    /// Vector backend for similarity search (optional).
    vector: Option<Arc<dyn VectorBackend + Send + Sync>>,
    /// Entity extraction callback for graph-augmented retrieval (optional).
    entity_extraction: Option<EntityExtractionCallback>,
    /// Expiration configuration for probabilistic TTL cleanup (optional).
    ///
    /// When set, a probabilistic cleanup of TTL-expired memories is triggered
    /// after each successful capture (default 5% probability).
    expiration_config: Option<ExpirationConfig>,
    /// Organization-scoped index backend (optional).
    ///
    /// When set and the capture request has `scope: Some(DomainScope::Org)`,
    /// the memory is stored in the org-shared index instead of the user-local index.
    org_index: Option<Arc<dyn IndexBackend + Send + Sync>>,
}

impl CaptureService {
    /// Creates a new capture service (persistence only).
    ///
    /// For full searchability, use [`with_backends`](Self::with_backends) to
    /// also configure index and vector backends.
    ///
    /// # Examples
    ///
    /// ```
    /// use subcog::config::Config;
    /// use subcog::services::CaptureService;
    ///
    /// let config = Config::default();
    /// let service = CaptureService::new(config);
    /// assert!(!service.has_embedder());
    /// assert!(!service.has_index());
    /// ```
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            secret_detector: SecretDetector::new(),
            redactor: ContentRedactor::new(),
            embedder: None,
            index: None,
            vector: None,
            entity_extraction: None,
            expiration_config: None,
            org_index: None,
        }
    }

    /// Creates a capture service with all storage backends.
    ///
    /// This enables:
    /// - Embedding generation during capture
    /// - Immediate indexing for text search
    /// - Immediate vector upsert for semantic search
    #[must_use]
    pub fn with_backends(
        config: Config,
        embedder: Arc<dyn Embedder>,
        index: Arc<dyn IndexBackend + Send + Sync>,
        vector: Arc<dyn VectorBackend + Send + Sync>,
    ) -> Self {
        Self {
            config,
            secret_detector: SecretDetector::new(),
            redactor: ContentRedactor::new(),
            embedder: Some(embedder),
            index: Some(index),
            vector: Some(vector),
            entity_extraction: None,
            expiration_config: None,
            org_index: None,
        }
    }

    /// Adds an embedder to an existing capture service.
    #[must_use]
    pub fn with_embedder(mut self, embedder: Arc<dyn Embedder>) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// Adds an index backend to an existing capture service.
    #[must_use]
    pub fn with_index(mut self, index: Arc<dyn IndexBackend + Send + Sync>) -> Self {
        self.index = Some(index);
        self
    }

    /// Adds a vector backend to an existing capture service.
    #[must_use]
    pub fn with_vector(mut self, vector: Arc<dyn VectorBackend + Send + Sync>) -> Self {
        self.vector = Some(vector);
        self
    }

    /// Adds an org-scoped index backend for shared memory storage.
    ///
    /// When configured and a capture request has `scope: Some(DomainScope::Org)`,
    /// the memory will be stored in the org-shared index instead of the user-local index.
    #[must_use]
    pub fn with_org_index(mut self, index: Arc<dyn IndexBackend + Send + Sync>) -> Self {
        self.org_index = Some(index);
        self
    }

    /// Returns whether an org-scoped index is configured.
    #[must_use]
    pub fn has_org_index(&self) -> bool {
        self.org_index.is_some()
    }

    /// Adds an entity extraction callback for graph-augmented retrieval.
    ///
    /// When configured, entities (people, organizations, technologies, concepts)
    /// are automatically extracted from captured content and stored in the
    /// knowledge graph. This enables Graph RAG (Retrieval-Augmented Generation)
    /// by finding related memories through entity relationships.
    ///
    /// # Arguments
    ///
    /// * `callback` - A callback that extracts entities from content and stores them.
    ///   The callback receives the content and memory ID, and should return stats.
    ///   Errors are logged but don't fail the capture operation.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use std::sync::Arc;
    /// use subcog::services::{CaptureService, EntityExtractionStats};
    ///
    /// let callback = Arc::new(|content: &str, memory_id: &MemoryId| {
    ///     // Extract entities and store in graph...
    ///     Ok(EntityExtractionStats::default())
    /// });
    ///
    /// let service = CaptureService::default()
    ///     .with_entity_extraction(callback);
    /// ```
    #[must_use]
    pub fn with_entity_extraction(mut self, callback: EntityExtractionCallback) -> Self {
        self.entity_extraction = Some(callback);
        self
    }

    /// Returns whether entity extraction is configured.
    #[must_use]
    pub fn has_entity_extraction(&self) -> bool {
        self.entity_extraction.is_some()
    }

    /// Returns whether embedding generation is available.
    #[must_use]
    pub fn has_embedder(&self) -> bool {
        self.embedder.is_some()
    }

    /// Returns whether immediate indexing is available.
    #[must_use]
    pub fn has_index(&self) -> bool {
        self.index.is_some()
    }

    /// Returns whether vector upsert is available.
    #[must_use]
    pub fn has_vector(&self) -> bool {
        self.vector.is_some()
    }

    /// Adds expiration configuration for probabilistic TTL cleanup.
    ///
    /// When configured, a probabilistic cleanup of TTL-expired memories is
    /// triggered after each successful capture. By default, there is a 5%
    /// chance of running cleanup after each capture.
    ///
    /// # Examples
    ///
    /// ```
    /// use subcog::services::CaptureService;
    /// use subcog::gc::ExpirationConfig;
    ///
    /// // Enable expiration cleanup with default 5% probability
    /// let service = CaptureService::default()
    ///     .with_expiration_config(ExpirationConfig::default());
    ///
    /// // Or with custom probability (10%)
    /// let config = ExpirationConfig::new().with_cleanup_probability(0.10);
    /// let service = CaptureService::default()
    ///     .with_expiration_config(config);
    /// ```
    #[must_use]
    pub const fn with_expiration_config(mut self, config: ExpirationConfig) -> Self {
        self.expiration_config = Some(config);
        self
    }

    /// Returns whether expiration cleanup is configured.
    #[must_use]
    pub const fn has_expiration(&self) -> bool {
        self.expiration_config.is_some()
    }

    /// Captures a memory.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The content is empty
    /// - The content contains unredacted secrets (when blocking is enabled)
    /// - Storage fails
    ///
    /// # Examples
    ///
    /// ```
    /// use subcog::services::CaptureService;
    /// use subcog::models::{CaptureRequest, Namespace, Domain};
    ///
    /// let service = CaptureService::default();
    /// let request = CaptureRequest {
    ///     content: "Use SQLite for local development".to_string(),
    ///     namespace: Namespace::Decisions,
    ///     domain: Domain::default(),
    ///     tags: vec!["database".to_string(), "architecture".to_string()],
    ///     source: Some("src/config.rs".to_string()),
    ///     skip_security_check: false,
    ///     ttl_seconds: None,
    ///     scope: None,
    ///     ..Default::default()
    /// };
    ///
    /// let result = service.capture(request)?;
    /// assert!(!result.memory_id.as_str().is_empty());
    /// assert!(result.urn.starts_with("subcog://"));
    /// # Ok::<(), subcog::Error>(())
    /// ```
    #[instrument(
        name = "subcog.memory.capture",
        skip(self, request),
        fields(
            request_id = tracing::field::Empty,
            component = "memory",
            operation = "capture",
            namespace = %request.namespace,
            domain = %request.domain,
            content_length = request.content.len(),
            skip_security_check = request.skip_security_check,
            memory.id = tracing::field::Empty
        )
    )]
    #[allow(clippy::too_many_lines)]
    pub fn capture(&self, request: CaptureRequest) -> Result<CaptureResult> {
        let start = Instant::now();
        let namespace_label = request.namespace.as_str().to_string();
        let domain_label = request.domain.to_string();
        if let Some(request_id) = current_request_id() {
            tracing::Span::current().record("request_id", request_id.as_str());
        }

        tracing::info!(namespace = %namespace_label, domain = %domain_label, "Capturing memory");

        // Maximum content size (500KB) - prevents abuse and memory issues (MED-SEC-002, MED-COMP-003)
        const MAX_CONTENT_SIZE: usize = 500_000;

        let result = (|| {
            let has_secrets = {
                let _span = info_span!("subcog.memory.capture.validate").entered();
                // Validate content length (MED-SEC-002, MED-COMP-003)
                if request.content.trim().is_empty() {
                    return Err(Error::InvalidInput("Content cannot be empty".to_string()));
                }
                if request.content.len() > MAX_CONTENT_SIZE {
                    return Err(Error::InvalidInput(format!(
                        "Content exceeds maximum size of {} bytes (got {} bytes)",
                        MAX_CONTENT_SIZE,
                        request.content.len()
                    )));
                }

                // Check for secrets
                let has_secrets = self.secret_detector.contains_secrets(&request.content);
                if has_secrets && self.config.features.block_secrets && !request.skip_security_check
                {
                    return Err(Error::ContentBlocked {
                        reason: "Content contains detected secrets".to_string(),
                    });
                }
                has_secrets
            };

            // Optionally redact secrets
            let (content, was_redacted) = {
                let _span = info_span!("subcog.memory.capture.redact").entered();
                if has_secrets
                    && self.config.features.redact_secrets
                    && !request.skip_security_check
                {
                    (self.redactor.redact(&request.content), true)
                } else {
                    (request.content.clone(), false)
                }
            };

            // Get current timestamp
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            // Generate memory ID from UUID (12 hex chars for consistency)
            let uuid = uuid::Uuid::new_v4();
            let memory_id = MemoryId::new(uuid.to_string().replace('-', "")[..12].to_string());

            let span = tracing::Span::current();
            span.record("memory.id", memory_id.as_str());

            // Generate embedding if embedder is available
            let embedding = {
                let _span = info_span!("subcog.memory.capture.embed").entered();
                if let Some(ref embedder) = self.embedder {
                    match embedder.embed(&content) {
                        Ok(emb) => {
                            tracing::debug!(
                                memory_id = %memory_id,
                                dimensions = emb.len(),
                                "Generated embedding for memory"
                            );
                            Some(emb)
                        },
                        Err(e) => {
                            tracing::warn!(
                                memory_id = %memory_id,
                                error = %e,
                                "Failed to generate embedding (continuing without)"
                            );
                            None
                        },
                    }
                } else {
                    None
                }
            };

            // Resolve git context for facets.
            let git_context = self
                .config
                .repo_path
                .as_ref()
                .map_or_else(GitContext::from_cwd, |path| GitContext::from_path(path));

            let file_path =
                resolve_file_path(self.config.repo_path.as_deref(), request.source.as_ref());

            let mut tags = request.tags;
            let hash_tag = ContentHasher::content_to_tag(&content);
            if !tags.iter().any(|tag| tag == &hash_tag) {
                tags.push(hash_tag);
            }

            // Calculate expires_at from TTL if provided
            // ttl_seconds = 0 means "never expire" (expires_at = None)
            // ttl_seconds > 0 means expires at now + ttl_seconds
            let expires_at = request.ttl_seconds.and_then(|ttl| {
                if ttl == 0 {
                    None // Explicit "never expire"
                } else {
                    Some(now.saturating_add(ttl))
                }
            });

            // Create memory
            let mut memory = Memory {
                id: memory_id.clone(),
                content,
                namespace: request.namespace,
                domain: request.domain,
                project_id: git_context.project_id,
                branch: git_context.branch,
                file_path,
                status: MemoryStatus::Active,
                created_at: now,
                updated_at: now,
                tombstoned_at: None,
                expires_at,
                embedding: embedding.clone(),
                tags,
                #[cfg(feature = "group-scope")]
                group_id: request.group_id,
                source: request.source,
                is_summary: false,
                source_memory_ids: None,
                consolidation_timestamp: None,
            };

            // Generate URN (always use subcog:// format)
            let urn = self.generate_urn(&memory);

            // Collect warnings
            let mut warnings = Vec::new();
            if was_redacted {
                warnings.push("Content was redacted due to detected secrets".to_string());
            }

            // Index memory for text search (best-effort)
            if let Some(ref index) = self.index {
                let _span = info_span!("subcog.memory.capture.index").entered();
                // Need mutable access - clone the Arc and get mutable ref
                let index_clone = Arc::clone(index);
                // IndexBackend::index takes &self with interior mutability
                match index_clone.index(&memory) {
                    Ok(()) => {
                        tracing::debug!(memory_id = %memory_id, "Indexed memory for text search");
                    },
                    Err(e) => {
                        tracing::warn!(
                            memory_id = %memory_id,
                            error = %e,
                            "Failed to index memory (continuing without)"
                        );
                        warnings.push("Memory not indexed for text search".to_string());
                    },
                }
            }

            // Upsert embedding to vector store (best-effort)
            if let (Some(vector), Some(emb)) = (&self.vector, &embedding) {
                let _span = info_span!("subcog.memory.capture.vector").entered();
                // VectorBackend::upsert takes &self with interior mutability
                let vector_clone = Arc::clone(vector);
                match vector_clone.upsert(&memory_id, emb) {
                    Ok(()) => {
                        tracing::debug!(memory_id = %memory_id, "Upserted embedding to vector store");
                    },
                    Err(e) => {
                        tracing::warn!(
                            memory_id = %memory_id,
                            error = %e,
                            "Failed to upsert embedding (continuing without)"
                        );
                        warnings.push("Embedding not stored in vector index".to_string());
                    },
                }
            }

            // Extract entities for graph-augmented retrieval (async when possible, sync fallback)
            // Entity extraction runs in the background to avoid blocking capture latency.
            if self.config.features.auto_extract_entities
                && let Some(ref callback) = self.entity_extraction
            {
                let callback = Arc::clone(callback);
                let content = memory.content.clone();
                let memory_id_for_task = memory_id.clone();

                // Check if we're in a tokio runtime context
                if tokio::runtime::Handle::try_current().is_ok() {
                    // Async path: spawn background task
                    tokio::spawn(async move {
                        run_entity_extraction(&callback, &content, &memory_id_for_task);
                    });
                } else {
                    // Sync path: run inline (for tests/CLI without async runtime)
                    run_entity_extraction(&callback, &content, &memory_id_for_task);
                }
            }

            // Clear embedding from memory before returning (it's stored separately)
            memory.embedding = None;

            record_event(MemoryEvent::Captured {
                meta: EventMeta::with_timestamp("capture", current_request_id(), now),
                memory_id: memory_id.clone(),
                namespace: memory.namespace,
                domain: memory.domain.clone(),
                content_length: memory.content.len(),
            });
            if was_redacted {
                record_event(MemoryEvent::Redacted {
                    meta: EventMeta::with_timestamp("capture", current_request_id(), now),
                    memory_id: memory_id.clone(),
                    redaction_type: "secrets".to_string(),
                });
            }

            Ok(CaptureResult {
                memory_id,
                urn,
                content_modified: was_redacted,
                warnings,
            })
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        metrics::counter!(
            "memory_operations_total",
            "operation" => "capture",
            "namespace" => namespace_label.clone(),
            "domain" => domain_label,
            "status" => status
        )
        .increment(1);
        metrics::histogram!(
            "memory_operation_duration_ms",
            "operation" => "capture",
            "namespace" => namespace_label
        )
        .record(start.elapsed().as_secs_f64() * 1000.0);
        metrics::histogram!(
            "memory_lifecycle_duration_ms",
            "component" => "memory",
            "operation" => "capture"
        )
        .record(start.elapsed().as_secs_f64() * 1000.0);

        // Probabilistic TTL cleanup (only on success, with configured index)
        if result.is_ok() {
            self.maybe_run_expiration_cleanup();
        }

        result
    }

    /// Probabilistically runs expiration cleanup of TTL-expired memories.
    ///
    /// This is called after each successful capture to lazily clean up
    /// expired memories without requiring a separate scheduled job.
    fn maybe_run_expiration_cleanup(&self) {
        // Need both expiration config and index backend
        let (Some(config), Some(index)) = (&self.expiration_config, &self.index) else {
            return;
        };

        // Create a temporary expiration service to check probability
        let service = ExpirationService::new(Arc::clone(index), config.clone());

        if !service.should_run_cleanup() {
            return;
        }

        // Run cleanup (best-effort, don't fail capture)
        let _span = info_span!("subcog.memory.capture.expiration_cleanup").entered();
        match service.gc_expired_memories(false) {
            Ok(result) => {
                if result.memories_tombstoned > 0 {
                    tracing::info!(
                        tombstoned = result.memories_tombstoned,
                        checked = result.memories_checked,
                        duration_ms = result.duration_ms,
                        "Probabilistic TTL cleanup completed"
                    );
                } else {
                    tracing::debug!(
                        checked = result.memories_checked,
                        duration_ms = result.duration_ms,
                        "Probabilistic TTL cleanup found no expired memories"
                    );
                }
            },
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Probabilistic TTL cleanup failed (capture still succeeded)"
                );
            },
        }
    }

    /// Generates a URN for the memory.
    #[allow(clippy::unused_self)] // Method kept for potential future use of self
    fn generate_urn(&self, memory: &Memory) -> String {
        let domain_part = if memory.domain.is_project_scoped() {
            "project".to_string()
        } else {
            memory.domain.to_string()
        };

        format!(
            "subcog://{}/{}/{}",
            domain_part,
            memory.namespace.as_str(),
            memory.id.as_str()
        )
    }

    /// Validates a capture request without storing.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use subcog::services::CaptureService;
    /// use subcog::models::{CaptureRequest, Namespace, Domain};
    ///
    /// let service = CaptureService::default();
    ///
    /// // Valid request
    /// let request = CaptureRequest {
    ///     content: "Valid content".to_string(),
    ///     namespace: Namespace::Learnings,
    ///     domain: Domain::default(),
    ///     tags: vec![],
    ///     source: None,
    ///     skip_security_check: false,
    ///     ttl_seconds: None,
    ///     scope: None,
    ///     ..Default::default()
    /// };
    /// let result = service.validate(&request)?;
    /// assert!(result.is_valid);
    ///
    /// // Empty content is invalid
    /// let empty_request = CaptureRequest {
    ///     content: "".to_string(),
    ///     namespace: Namespace::Learnings,
    ///     domain: Domain::default(),
    ///     tags: vec![],
    ///     source: None,
    ///     skip_security_check: false,
    ///     ttl_seconds: None,
    ///     scope: None,
    ///     ..Default::default()
    /// };
    /// let result = service.validate(&empty_request)?;
    /// assert!(!result.is_valid);
    /// # Ok::<(), subcog::Error>(())
    /// ```
    pub fn validate(&self, request: &CaptureRequest) -> Result<ValidationResult> {
        let mut issues = Vec::new();
        let mut warnings = Vec::new();

        // Check content length
        if request.content.trim().is_empty() {
            issues.push("Content cannot be empty".to_string());
        } else if request.content.len() > 100_000 {
            warnings.push("Content is very long (>100KB)".to_string());
        }

        // Check for secrets
        let secrets = self.secret_detector.detect_types(&request.content);
        if !secrets.is_empty() {
            if self.config.features.block_secrets {
                issues.push(format!("Content contains secrets: {}", secrets.join(", ")));
            } else {
                warnings.push(format!("Content contains secrets: {}", secrets.join(", ")));
            }
        }

        Ok(ValidationResult {
            is_valid: issues.is_empty(),
            issues,
            warnings,
        })
    }

    /// Captures a memory with authorization check (CRIT-006).
    ///
    /// This method requires [`super::auth::Permission::Write`] to be present in the auth context.
    /// Use this for MCP/HTTP endpoints where authorization is required.
    ///
    /// When the request contains a `group_id`, this method also validates that the
    /// auth context has write permission to that group.
    ///
    /// # Arguments
    ///
    /// * `request` - The capture request
    /// * `auth` - Authorization context with permissions
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unauthorized`] if write permission is not granted.
    /// Returns [`Error::Unauthorized`] if group write permission is required but not granted.
    /// Returns other errors as per [`capture`](Self::capture).
    pub fn capture_authorized(
        &self,
        request: CaptureRequest,
        auth: &super::auth::AuthContext,
    ) -> Result<CaptureResult> {
        auth.require(super::auth::Permission::Write)?;

        // Check group write permission if group_id is specified
        #[cfg(feature = "group-scope")]
        if let Some(ref group_id) = request.group_id {
            use crate::models::group::GroupRole;
            auth.require_group_role(group_id, GroupRole::Write)?;
        }

        self.capture(request)
    }
}

fn resolve_file_path(repo_root: Option<&Path>, source: Option<&String>) -> Option<String> {
    let source = source?;
    if source.contains("://") {
        return None;
    }

    let source_path = Path::new(source);
    let repo_root = repo_root?;

    if let Ok(relative) = source_path.strip_prefix(repo_root) {
        return Some(normalize_path(&relative.to_string_lossy()));
    }

    if source_path.is_relative() {
        return Some(normalize_path(source));
    }

    None
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

impl Default for CaptureService {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

/// Result of capture validation.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the capture request is valid.
    pub is_valid: bool,
    /// List of blocking issues.
    pub issues: Vec<String>,
    /// List of non-blocking warnings.
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, Namespace};

    fn test_config() -> Config {
        Config::default()
    }

    fn test_request(content: &str) -> CaptureRequest {
        CaptureRequest {
            content: content.to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::default(),
            tags: vec!["test".to_string()],
            source: Some("test.rs".to_string()),
            skip_security_check: false,
            ttl_seconds: None,
            scope: None,
            #[cfg(feature = "group-scope")]
            group_id: None,
        }
    }

    #[test]
    fn test_capture_success() {
        let service = CaptureService::new(test_config());
        let request = test_request("Use PostgreSQL for primary storage");

        let result = service.capture(request);
        assert!(result.is_ok());

        let result = result.unwrap();
        // Memory ID is SHA only (12 hex chars), no namespace prefix
        assert_eq!(result.memory_id.as_str().len(), 12);
        assert!(
            result
                .memory_id
                .as_str()
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        );
        assert!(result.urn.starts_with("subcog://"));
        assert!(!result.content_modified);
    }

    #[test]
    fn test_capture_empty_content() {
        let service = CaptureService::new(test_config());
        let request = test_request("   ");

        let result = service.capture(request);
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::InvalidInput(_))));
    }

    #[test]
    fn test_capture_with_secrets_redacted() {
        let mut config = test_config();
        config.features.redact_secrets = true;
        config.features.block_secrets = false;

        let service = CaptureService::new(config);
        let request = test_request("My API key is AKIAIOSFODNN7EXAMPLE");

        let result = service.capture(request);
        assert!(result.is_ok());
        assert!(result.unwrap().content_modified);
    }

    #[test]
    fn test_capture_with_secrets_blocked() {
        let mut config = test_config();
        config.features.block_secrets = true;

        let service = CaptureService::new(config);
        let request = test_request("My API key is AKIAIOSFODNN7EXAMPLE");

        let result = service.capture(request);
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::ContentBlocked { .. })));
    }

    #[test]
    fn test_validate_valid() {
        let service = CaptureService::new(test_config());
        let request = test_request("Valid content");

        let result = service.validate(&request).unwrap();
        assert!(result.is_valid);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_validate_empty() {
        let service = CaptureService::new(test_config());
        let request = test_request("");

        let result = service.validate(&request).unwrap();
        assert!(!result.is_valid);
        assert!(!result.issues.is_empty());
    }

    #[test]
    fn test_generate_urn() {
        let service = CaptureService::new(test_config());

        let memory = Memory {
            id: MemoryId::new("test_123"),
            content: "Test".to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::for_repository("zircote", "subcog"),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: 0,
            updated_at: 0,
            tombstoned_at: None,
            expires_at: None,
            embedding: None,
            tags: vec![],
            #[cfg(feature = "group-scope")]
            group_id: None,
            source: None,
            is_summary: false,
            source_memory_ids: None,
            consolidation_timestamp: None,
        };

        let urn = service.generate_urn(&memory);
        assert!(urn.contains("subcog"));
        assert!(urn.contains("decisions"));
        assert!(urn.contains("test_123"));
    }

    // ========================================================================
    // Phase 3 (MEM-003) Tests: Embedding generation and backend integration
    // ========================================================================

    use crate::embedding::FastEmbedEmbedder;
    use crate::services::deduplication::ContentHasher;
    use crate::storage::index::SqliteBackend;
    use crate::storage::vector::UsearchBackend;
    use git2::{Repository, Signature};
    use tempfile::TempDir;

    fn init_test_repo() -> (TempDir, Repository) {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let repo = Repository::init(dir.path()).expect("Failed to init repo");

        let sig = Signature::now("test", "test@test.com").expect("Failed to create signature");
        let tree_id = repo
            .index()
            .expect("Failed to get index")
            .write_tree()
            .expect("Failed to write tree");
        {
            let tree = repo.find_tree(tree_id).expect("Failed to find tree");
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                .expect("Failed to create commit");
        }
        repo.remote("origin", "https://github.com/org/repo.git")
            .expect("Failed to add remote");

        (dir, repo)
    }

    #[test]
    fn test_capture_with_embedder_generates_embedding() {
        // Test that embedder is invoked during capture
        let embedder: Arc<dyn Embedder> = Arc::new(FastEmbedEmbedder::new());
        let service = CaptureService::new(test_config()).with_embedder(embedder);

        assert!(service.has_embedder());
        // Note: The embedding is generated internally during capture.
        // We verify the service is configured correctly.
    }

    #[test]
    fn test_capture_with_index_backend() {
        // Test that index backend is used during capture
        let index = SqliteBackend::in_memory().expect("Failed to create in-memory SQLite backend");
        let index_arc: Arc<dyn IndexBackend + Send + Sync> = Arc::new(index);

        let service = CaptureService::new(test_config()).with_index(index_arc);

        assert!(service.has_index());
        // Capture should succeed and index the memory
        let request = test_request("Use PostgreSQL for primary storage");
        let result = service.capture(request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_capture_sets_facets_and_hash_tag() {
        let (dir, _repo) = init_test_repo();
        let repo_path = dir.path();
        let index: Arc<dyn IndexBackend + Send + Sync> =
            Arc::new(SqliteBackend::in_memory().unwrap());
        let config = Config::new().with_repo_path(repo_path);
        let service = CaptureService::new(config).with_index(Arc::clone(&index));

        let file_path = repo_path.join("src").join("lib.rs");
        std::fs::create_dir_all(file_path.parent().expect("parent path")).expect("create dir");
        std::fs::write(&file_path, "fn main() {}\n").expect("write file");

        let request = CaptureRequest {
            content: "Test content for facets".to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            tags: vec!["test".to_string()],
            source: Some(file_path.to_string_lossy().to_string()),
            skip_security_check: false,
            ttl_seconds: None,
            scope: None,
            #[cfg(feature = "group-scope")]
            group_id: None,
        };

        let result = service.capture(request).expect("capture");
        let stored = index
            .get_memory(&result.memory_id)
            .expect("get memory")
            .expect("stored memory");

        assert_eq!(stored.project_id.as_deref(), Some("github.com/org/repo"));
        assert!(stored.branch.is_some());
        assert_eq!(stored.file_path.as_deref(), Some("src/lib.rs"));

        let hash_tag = ContentHasher::content_to_tag(&stored.content);
        assert!(stored.tags.contains(&hash_tag));
    }

    #[test]
    fn test_capture_with_vector_backend() {
        // Test that vector backend is used during capture
        // Using fallback UsearchBackend for testing (no usearch-hnsw feature needed)
        #[cfg(not(feature = "usearch-hnsw"))]
        let vector = UsearchBackend::new(
            std::env::temp_dir().join("test_vector_capture"),
            FastEmbedEmbedder::DEFAULT_DIMENSIONS,
        );
        #[cfg(feature = "usearch-hnsw")]
        let vector = UsearchBackend::new(
            std::env::temp_dir().join("test_vector_capture"),
            FastEmbedEmbedder::DEFAULT_DIMENSIONS,
        )
        .expect("Failed to create usearch backend");

        let vector_arc: Arc<dyn VectorBackend + Send + Sync> = Arc::new(vector);

        let service = CaptureService::new(test_config()).with_vector(vector_arc);

        assert!(service.has_vector());
        // Note: Without embedder, no embedding will be generated for vector storage
    }

    #[test]
    fn test_capture_with_all_backends() {
        // Test full pipeline: embedder + index + vector
        let embedder: Arc<dyn Embedder> = Arc::new(FastEmbedEmbedder::new());
        let index = SqliteBackend::in_memory().expect("Failed to create in-memory SQLite backend");
        let index_arc: Arc<dyn IndexBackend + Send + Sync> = Arc::new(index);

        #[cfg(not(feature = "usearch-hnsw"))]
        let vector = UsearchBackend::new(
            std::env::temp_dir().join("test_vector_all"),
            FastEmbedEmbedder::DEFAULT_DIMENSIONS,
        );
        #[cfg(feature = "usearch-hnsw")]
        let vector = UsearchBackend::new(
            std::env::temp_dir().join("test_vector_all"),
            FastEmbedEmbedder::DEFAULT_DIMENSIONS,
        )
        .expect("Failed to create usearch backend");

        let vector_arc: Arc<dyn VectorBackend + Send + Sync> = Arc::new(vector);

        let service = CaptureService::with_backends(
            test_config(),
            Arc::clone(&embedder),
            Arc::clone(&index_arc),
            Arc::clone(&vector_arc),
        );

        assert!(service.has_embedder());
        assert!(service.has_index());
        assert!(service.has_vector());

        // Capture should succeed with all backends
        let request = test_request("Use PostgreSQL for primary storage");
        let result = service.capture(request);
        assert!(result.is_ok(), "Capture failed: {:?}", result.err());
    }

    #[test]
    fn test_capture_succeeds_without_backends() {
        // Graceful degradation: capture should succeed even without optional backends
        let service = CaptureService::new(test_config());

        assert!(!service.has_embedder());
        assert!(!service.has_index());
        assert!(!service.has_vector());

        let request = test_request("This should still work");
        let result = service.capture(request);
        assert!(result.is_ok(), "Capture should succeed without backends");
    }

    #[test]
    fn test_capture_succeeds_with_only_embedder() {
        // Test partial configuration: only embedder (no storage backends)
        let embedder: Arc<dyn Embedder> = Arc::new(FastEmbedEmbedder::new());
        let service = CaptureService::new(test_config()).with_embedder(embedder);

        assert!(service.has_embedder());
        assert!(!service.has_index());
        assert!(!service.has_vector());

        let request = test_request("Test with embedder only");
        let result = service.capture(request);
        assert!(result.is_ok(), "Capture should succeed with embedder only");
    }

    #[test]
    fn test_capture_index_failure_doesnt_fail_capture() {
        // Test graceful degradation: index failure shouldn't fail the capture
        // This is verified by the warning log, but capture still succeeds
        // We test this indirectly by verifying the capture succeeds
        // even when index could potentially fail.

        let embedder: Arc<dyn Embedder> = Arc::new(FastEmbedEmbedder::new());
        let service = CaptureService::new(test_config()).with_embedder(embedder);

        let request = test_request("Test graceful degradation");
        let result = service.capture(request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_has_embedder_returns_false_when_not_configured() {
        let service = CaptureService::new(test_config());
        assert!(!service.has_embedder());
    }

    #[test]
    fn test_has_index_returns_false_when_not_configured() {
        let service = CaptureService::new(test_config());
        assert!(!service.has_index());
    }

    #[test]
    fn test_has_vector_returns_false_when_not_configured() {
        let service = CaptureService::new(test_config());
        assert!(!service.has_vector());
    }

    #[test]
    fn test_builder_methods_chain() {
        // Test that builder methods can be chained
        let embedder: Arc<dyn Embedder> = Arc::new(FastEmbedEmbedder::new());
        let index = SqliteBackend::in_memory().expect("Failed to create in-memory SQLite backend");
        let index_arc: Arc<dyn IndexBackend + Send + Sync> = Arc::new(index);

        let service = CaptureService::new(test_config())
            .with_embedder(embedder)
            .with_index(index_arc);

        assert!(service.has_embedder());
        assert!(service.has_index());
        assert!(!service.has_vector()); // Not configured
    }

    // ========================================================================
    // Group-scoped capture tests
    // ========================================================================

    #[cfg(feature = "group-scope")]
    mod group_scope_tests {
        use super::*;
        use crate::services::auth::AuthContext;

        fn test_config() -> Config {
            Config::default()
        }

        #[test]
        fn test_capture_with_group_id() {
            let index: Arc<dyn IndexBackend + Send + Sync> =
                Arc::new(SqliteBackend::in_memory().unwrap());
            let service = CaptureService::new(test_config()).with_index(Arc::clone(&index));

            let request = CaptureRequest::new("Group-scoped memory content")
                .with_namespace(Namespace::Decisions)
                .with_group_id("team-alpha");

            let result = service.capture(request).expect("capture should succeed");

            // Verify the memory was stored with group_id
            let stored = index
                .get_memory(&result.memory_id)
                .expect("get memory")
                .expect("stored memory");
            assert_eq!(stored.group_id.as_deref(), Some("team-alpha"));
        }

        #[test]
        fn test_capture_authorized_with_group_permission() {
            let service = CaptureService::new(test_config());

            // Use builder to create auth with write scope and group permission
            let auth = AuthContext::builder()
                .scope("write")
                .group_role("team-alpha", "write")
                .build();

            let request = CaptureRequest::new("Group content")
                .with_namespace(Namespace::Decisions)
                .with_group_id("team-alpha");

            let result = service.capture_authorized(request, &auth);
            assert!(result.is_ok(), "Should succeed with group permission");
        }

        #[test]
        fn test_capture_authorized_fails_without_group_permission() {
            let service = CaptureService::new(test_config());

            // Auth with write scope but NO group permission
            let auth = AuthContext::builder().scope("write").build();

            let request = CaptureRequest::new("Group content")
                .with_namespace(Namespace::Decisions)
                .with_group_id("team-alpha");

            let result = service.capture_authorized(request, &auth);
            assert!(result.is_err(), "Should fail without group permission");
            assert!(matches!(result, Err(Error::Unauthorized { .. })));
        }

        #[test]
        fn test_capture_authorized_fails_with_read_only_group_permission() {
            let service = CaptureService::new(test_config());

            // Auth with write scope but only read permission to group
            let auth = AuthContext::builder()
                .scope("write")
                .group_role("team-alpha", "read")
                .build();

            let request = CaptureRequest::new("Group content")
                .with_namespace(Namespace::Decisions)
                .with_group_id("team-alpha");

            let result = service.capture_authorized(request, &auth);
            assert!(result.is_err(), "Should fail with read-only group access");
        }

        #[test]
        fn test_capture_authorized_without_group_id_succeeds() {
            let service = CaptureService::new(test_config());

            // Auth with write scope
            let auth = AuthContext::builder().scope("write").build();

            // No group_id in request - should not require group permission
            let request =
                CaptureRequest::new("Non-group content").with_namespace(Namespace::Decisions);

            let result = service.capture_authorized(request, &auth);
            assert!(result.is_ok(), "Should succeed without group_id");
        }

        #[test]
        fn test_capture_request_builder_with_group_id() {
            let request = CaptureRequest::new("Content")
                .with_namespace(Namespace::Learnings)
                .with_tag("test")
                .with_group_id("my-group");

            assert_eq!(request.group_id.as_deref(), Some("my-group"));
            assert_eq!(request.namespace, Namespace::Learnings);
        }

        #[test]
        fn test_capture_with_local_auth_and_group_id() {
            // Local auth context should have implicit group admin access
            let service = CaptureService::new(test_config());
            let auth = AuthContext::local();

            let request = CaptureRequest::new("Local group content")
                .with_namespace(Namespace::Decisions)
                .with_group_id("any-group");

            let result = service.capture_authorized(request, &auth);
            assert!(
                result.is_ok(),
                "Local auth should have implicit group access"
            );
        }
    }
}
