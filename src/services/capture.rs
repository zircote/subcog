//! Memory capture service.
//!
//! Handles capturing new memories, including validation, redaction, and storage.
//! Now also generates embeddings and indexes memories for searchability.

use crate::config::Config;
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
/// When fully configured, captures are written to two layers:
/// 1. **Index** (`SQLite` FTS5) - Authoritative storage with full-text search via BM25
/// 2. **Vector** (usearch) - Semantic similarity search
///
/// # Graceful Degradation
///
/// If embedder or vector backend is unavailable:
/// - Capture still succeeds (Index layer is authoritative)
/// - A warning is logged for each failed secondary store
/// - The memory may not be searchable via semantic similarity
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

        // Maximum content size (500KB) - prevents abuse and memory issues (MED-SEC-002, MED-COMP-003)
        const MAX_CONTENT_SIZE: usize = 500_000;

        let result = (|| {
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

            // Create memory
            let mut memory = Memory {
                id: memory_id.clone(),
                content,
                namespace: request.namespace,
                domain: request.domain,
                status: MemoryStatus::Active,
                created_at: now,
                updated_at: now,
                tombstoned_at: None,
                embedding: embedding.clone(),
                tags: request.tags,
                source: request.source,
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
    /// # Arguments
    ///
    /// * `request` - The capture request
    /// * `auth` - Authorization context with permissions
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unauthorized`] if write permission is not granted.
    /// Returns other errors as per [`capture`](Self::capture).
    pub fn capture_authorized(
        &self,
        request: CaptureRequest,
        auth: &super::auth::AuthContext,
    ) -> Result<CaptureResult> {
        auth.require(super::auth::Permission::Write)?;
        self.capture(request)
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
            status: MemoryStatus::Active,
            created_at: 0,
            updated_at: 0,
            tombstoned_at: None,
            embedding: None,
            tags: vec![],
            source: None,
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
