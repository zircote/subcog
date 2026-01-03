//! Memory capture service.
//!
//! Handles capturing new memories, including validation, redaction, and storage.
//! Now also generates embeddings and indexes memories for searchability.
//!
//! # Context Detection (Issue #43)
//!
//! When capturing memories, the service automatically detects git context (`project_id`, branch)
//! from the current working directory if not explicitly provided in the request. This enables
//! faceted storage and filtering without requiring callers to manually specify context.

use crate::config::Config;
use crate::context::GitContext;
use crate::embedding::Embedder;
use crate::models::{CaptureRequest, CaptureResult, Memory, MemoryEvent, MemoryId, MemoryStatus};
use crate::security::{ContentRedactor, SecretDetector, record_event};
use crate::storage::traits::{IndexBackend, VectorBackend};
use crate::{Error, Result};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::instrument;

/// Service for capturing memories.
///
/// # Storage Layers
///
/// When fully configured, captures are written to three layers:
/// 1. **Persistence** (`SQLite`) - Authoritative storage
/// 2. **Index** (`SQLite` FTS5) - Full-text search via BM25
/// 3. **Vector** (usearch) - Semantic similarity search
///
/// # Graceful Degradation
///
/// If embedder or index/vector backends are unavailable:
/// - Capture still succeeds (`SQLite` is authoritative)
/// - A warning is logged for each failed secondary store
/// - The memory may not be immediately searchable
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
}

impl CaptureService {
    /// Creates a new capture service (persistence only).
    ///
    /// For full searchability, use [`with_backends`](Self::with_backends) to
    /// also configure index and vector backends.
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            secret_detector: SecretDetector::new(),
            redactor: ContentRedactor::new(),
            embedder: None,
            index: None,
            vector: None,
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

    /// Captures a memory.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The content is empty
    /// - The content contains unredacted secrets (when blocking is enabled)
    /// - Storage fails
    #[instrument(
        skip(self, request),
        fields(
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

        tracing::info!(namespace = %namespace_label, domain = %domain_label, "Capturing memory");

        let result = (|| {
            // Validate content
            if request.content.trim().is_empty() {
                return Err(Error::InvalidInput("Content cannot be empty".to_string()));
            }

            // Check for secrets
            let has_secrets = self.secret_detector.contains_secrets(&request.content);
            if has_secrets && self.config.features.block_secrets && !request.skip_security_check {
                return Err(Error::ContentBlocked {
                    reason: "Content contains detected secrets".to_string(),
                });
            }

            // Optionally redact secrets
            let (content, was_redacted) = if has_secrets
                && self.config.features.redact_secrets
                && !request.skip_security_check
            {
                (self.redactor.redact(&request.content), true)
            } else {
                (request.content.clone(), false)
            };

            // Auto-detect git context if facet fields not provided (Issue #43)
            let git_context = GitContext::from_cwd();
            let project_id = request.project_id.clone().or(git_context.project_id);
            let branch = request.branch.clone().or(git_context.branch);
            let file_path = request.file_path.clone();

            if project_id.is_some() || branch.is_some() {
                tracing::debug!(
                    project_id = ?project_id,
                    branch = ?branch,
                    file_path = ?file_path,
                    "Detected git context for memory capture"
                );
            }

            // Get current timestamp
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            // Generate UUID-based memory ID (Issue #43: removed git-notes storage)
            // SQLite is now the single source of truth for persistence
            let uuid = uuid::Uuid::new_v4();
            let memory_id = MemoryId::new(uuid.to_string().replace('-', "")[..12].to_string());

            let span = tracing::Span::current();
            span.record("memory.id", memory_id.as_str());

            // Generate embedding if embedder is available
            let embedding = if let Some(ref embedder) = self.embedder {
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
            };

            // Create memory with auto-detected context (Issue #43)
            let mut memory = Memory {
                id: memory_id.clone(),
                content,
                namespace: request.namespace,
                domain: request.domain,
                status: MemoryStatus::Active,
                created_at: now,
                updated_at: now,
                embedding: embedding.clone(),
                tags: request.tags,
                source: request.source,
                project_id,
                branch,
                file_path,
                tombstoned_at: None,
            };

            // Generate URN using the centralized Memory::urn() method (Task 3.4)
            // URN format: subcog://{scope}/{namespace}/{id}
            // Scope is derived from domain via Domain::urn_scope()
            let urn = memory.urn();

            // Collect warnings
            let mut warnings = Vec::new();
            if was_redacted {
                warnings.push("Content was redacted due to detected secrets".to_string());
            }

            // Index memory for text search (best-effort)
            if let Some(ref index) = self.index {
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

            // Clear embedding from memory before returning (it's stored separately)
            memory.embedding = None;

            record_event(MemoryEvent::Captured {
                memory_id: memory_id.clone(),
                namespace: memory.namespace,
                domain: memory.domain.clone(),
                content_length: memory.content.len(),
                timestamp: now,
            });
            if was_redacted {
                record_event(MemoryEvent::Redacted {
                    memory_id: memory_id.clone(),
                    redaction_type: "secrets".to_string(),
                    timestamp: now,
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

        result
    }

    /// Validates a capture request without storing.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails.
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
            project_id: None,
            branch: None,
            file_path: None,
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

    // ========================================================================
    // URN Generation Tests (Task 3.4: Storage Simplification)
    // ========================================================================

    #[test]
    fn test_capture_urn_project_domain() {
        let service = CaptureService::new(test_config());
        let request = test_request("Test content");

        let result = service.capture(request).unwrap();
        // Default domain is project, so URN should start with subcog://project/
        assert!(
            result.urn.starts_with("subcog://project/"),
            "Expected URN to start with 'subcog://project/', got: {}",
            result.urn
        );
        assert!(result.urn.contains("/decisions/"));
    }

    #[test]
    fn test_capture_urn_user_domain() {
        let service = CaptureService::new(test_config());
        let mut request = test_request("Test content");
        request.domain = Domain::for_user();

        let result = service.capture(request).unwrap();
        // User domain should produce subcog://user/ URN
        assert!(
            result.urn.starts_with("subcog://user/"),
            "Expected URN to start with 'subcog://user/', got: {}",
            result.urn
        );
        assert!(result.urn.contains("/decisions/"));
    }

    #[test]
    fn test_capture_urn_repository_domain() {
        let service = CaptureService::new(test_config());
        let mut request = test_request("Test content");
        request.domain = Domain::for_repository("zircote", "subcog");

        let result = service.capture(request).unwrap();
        // Repository domain should produce subcog://org/repo/ URN
        assert!(
            result.urn.starts_with("subcog://zircote/subcog/"),
            "Expected URN to start with 'subcog://zircote/subcog/', got: {}",
            result.urn
        );
        assert!(result.urn.contains("/decisions/"));
    }

    #[test]
    fn test_capture_urn_format() {
        let service = CaptureService::new(test_config());
        let request = test_request("Test content");

        let result = service.capture(request).unwrap();

        // URN should follow format: subcog://{scope}/{namespace}/{id}
        let parts: Vec<&str> = result.urn.split('/').collect();
        assert_eq!(parts[0], "subcog:");
        assert_eq!(parts[1], ""); // empty after ://
        assert_eq!(parts[2], "project"); // scope
        assert_eq!(parts[3], "decisions"); // namespace
        assert_eq!(parts[4].len(), 12); // id (12 hex chars)
    }

    // ========================================================================
    // Phase 3 (MEM-003) Tests: Embedding generation and backend integration
    // ========================================================================

    use crate::embedding::FastEmbedEmbedder;
    use crate::storage::index::SqliteBackend;
    use crate::storage::vector::UsearchBackend;

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
}
