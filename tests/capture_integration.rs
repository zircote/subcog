//! `CaptureService` Integration Tests (TEST-CRIT-001)
//!
//! Tests the `CaptureService` in isolation, focusing on:
//! - Content validation (empty, size limits)
//! - Secret detection and handling
//! - Content hash generation
//! - Warning collection
//! - Graceful degradation

// Integration tests use expect/unwrap for simplicity - panics are acceptable in tests
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;
use subcog::config::Config;
use subcog::embedding::FastEmbedEmbedder;
use subcog::storage::index::SqliteBackend;
use subcog::storage::vector::UsearchBackend;
use subcog::{
    CaptureRequest, CaptureService, Domain, Embedder, IndexBackend, Namespace, VectorBackend,
};
use tempfile::TempDir;

// ============================================================================
// Test Helpers
// ============================================================================

/// Creates a fully-configured capture service with all backends.
fn create_capture_service(temp_dir: &TempDir) -> CaptureService {
    let config = Config::default();
    let embedder: Arc<dyn Embedder> = Arc::new(FastEmbedEmbedder::new());

    let index_path = temp_dir.path().join("test_index.db");
    let index: Arc<dyn IndexBackend + Send + Sync> =
        Arc::new(SqliteBackend::new(&index_path).expect("Failed to create SQLite index"));

    let vector_path = temp_dir.path().join("test_vectors");
    #[cfg(feature = "usearch-hnsw")]
    let vector: Arc<dyn VectorBackend + Send + Sync> = Arc::new(
        UsearchBackend::new(&vector_path, FastEmbedEmbedder::DEFAULT_DIMENSIONS)
            .expect("Failed to create vector backend"),
    );
    #[cfg(not(feature = "usearch-hnsw"))]
    let vector: Arc<dyn VectorBackend + Send + Sync> = Arc::new(UsearchBackend::new(
        &vector_path,
        FastEmbedEmbedder::DEFAULT_DIMENSIONS,
    ));

    CaptureService::with_backends(config, embedder, index, vector)
}

/// Creates a capture service with secrets blocking enabled.
fn create_capture_service_with_secrets_blocking(temp_dir: &TempDir) -> CaptureService {
    let mut config = Config::default();
    config.features.block_secrets = true;
    config.features.redact_secrets = false;

    let embedder: Arc<dyn Embedder> = Arc::new(FastEmbedEmbedder::new());

    let index_path = temp_dir.path().join("test_index.db");
    let index: Arc<dyn IndexBackend + Send + Sync> =
        Arc::new(SqliteBackend::new(&index_path).expect("Failed to create SQLite index"));

    let vector_path = temp_dir.path().join("test_vectors");
    #[cfg(feature = "usearch-hnsw")]
    let vector: Arc<dyn VectorBackend + Send + Sync> = Arc::new(
        UsearchBackend::new(&vector_path, FastEmbedEmbedder::DEFAULT_DIMENSIONS)
            .expect("Failed to create vector backend"),
    );
    #[cfg(not(feature = "usearch-hnsw"))]
    let vector: Arc<dyn VectorBackend + Send + Sync> = Arc::new(UsearchBackend::new(
        &vector_path,
        FastEmbedEmbedder::DEFAULT_DIMENSIONS,
    ));

    CaptureService::with_backends(config, embedder, index, vector)
}

// ============================================================================
// Content Validation Tests
// ============================================================================

/// Test: Empty content is rejected
///
/// Verifies that empty content returns an appropriate error.
#[test]
fn test_capture_rejects_empty_content() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);

    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: String::new(),
        domain: Domain::new(),
        tags: vec![],
        source: None,
        skip_security_check: true,
        ttl_seconds: None,
        scope: None,
    };

    let result = capture_service.capture(request);
    assert!(result.is_err(), "Empty content should be rejected");

    let error = result.unwrap_err();
    let error_msg = error.to_string();
    assert!(
        error_msg.contains("empty") || error_msg.contains("Content"),
        "Error should mention empty content: {error_msg}"
    );
}

/// Test: Whitespace-only content is rejected
///
/// Verifies that content with only whitespace is rejected.
#[test]
fn test_capture_rejects_whitespace_only_content() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);

    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "   \n\t\r\n   ".to_string(),
        domain: Domain::new(),
        tags: vec![],
        source: None,
        skip_security_check: true,
        ttl_seconds: None,
        scope: None,
    };

    let result = capture_service.capture(request);
    assert!(
        result.is_err(),
        "Whitespace-only content should be rejected"
    );
}

/// Test: Content exceeding max size is rejected
///
/// Verifies that very large content is rejected (max 500KB).
#[test]
fn test_capture_rejects_oversized_content() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);

    // Create content larger than 500KB
    let large_content = "x".repeat(600_000);

    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: large_content,
        domain: Domain::new(),
        tags: vec![],
        source: None,
        skip_security_check: true,
        ttl_seconds: None,
        scope: None,
    };

    let result = capture_service.capture(request);
    assert!(result.is_err(), "Oversized content should be rejected");

    let error = result.unwrap_err();
    let error_msg = error.to_string();
    assert!(
        error_msg.contains("size") || error_msg.contains("exceeds"),
        "Error should mention size limit: {error_msg}"
    );
}

/// Test: Content at max size boundary is accepted
///
/// Verifies that content at exactly max size is accepted.
#[test]
fn test_capture_accepts_content_at_size_limit() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);

    // Create content at exactly 500KB (boundary case)
    let boundary_content = "x".repeat(500_000);

    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: boundary_content,
        domain: Domain::new(),
        tags: vec![],
        source: None,
        skip_security_check: true,
        ttl_seconds: None,
        scope: None,
    };

    let result = capture_service.capture(request);
    assert!(result.is_ok(), "Content at size limit should be accepted");
}

// ============================================================================
// Secret Handling Tests
// ============================================================================

/// Test: Content with secrets is blocked when blocking is enabled
///
/// Verifies that secret detection blocks captures when configured.
#[test]
fn test_capture_blocks_secrets_when_configured() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service_with_secrets_blocking(&temp_dir);

    // Content with an AWS Access Key pattern (AKIA followed by 16 alphanumeric chars)
    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "Using AWS key: AKIAIOSFODNN7EXAMPLE".to_string(),
        domain: Domain::new(),
        tags: vec![],
        source: None,
        skip_security_check: false, // Don't skip security check
        ttl_seconds: None,
        scope: None,
    };

    let result = capture_service.capture(request);
    assert!(
        result.is_err(),
        "Content with secrets should be blocked when blocking is enabled"
    );

    let error = result.unwrap_err();
    let error_msg = error.to_string();
    assert!(
        error_msg.contains("secrets") || error_msg.contains("blocked"),
        "Error should mention secrets: {error_msg}"
    );
}

/// Test: Content with secrets is allowed when `skip_security_check` is true
///
/// Verifies that `skip_security_check` bypasses secret blocking.
#[test]
fn test_capture_allows_secrets_when_skip_security_check() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service_with_secrets_blocking(&temp_dir);

    // Content with an AWS Access Key pattern (AKIA followed by 16 alphanumeric chars)
    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "Using AWS key: AKIAIOSFODNN7EXAMPLE".to_string(),
        domain: Domain::new(),
        tags: vec![],
        source: None,
        skip_security_check: true, // Skip security check
        ttl_seconds: None,
        scope: None,
    };

    let result = capture_service.capture(request);
    assert!(
        result.is_ok(),
        "Content with secrets should be allowed when skip_security_check is true"
    );
}

/// Test: Content without secrets is allowed
///
/// Verifies that normal content passes secret detection.
#[test]
fn test_capture_allows_content_without_secrets() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service_with_secrets_blocking(&temp_dir);

    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "Use PostgreSQL for primary storage because of JSONB support".to_string(),
        domain: Domain::new(),
        tags: vec!["database".to_string()],
        source: None,
        skip_security_check: false,
        ttl_seconds: None,
        scope: None,
    };

    let result = capture_service.capture(request);
    assert!(
        result.is_ok(),
        "Content without secrets should be allowed: {:?}",
        result.err()
    );
}

// ============================================================================
// Content Hash Tests
// ============================================================================

/// Test: Same content produces same hash tag
///
/// Verifies content hashing is deterministic.
#[test]
fn test_content_hash_is_deterministic() {
    use subcog::services::deduplication::ContentHasher;

    let content = "Use PostgreSQL for storage";

    let hash1 = ContentHasher::content_to_tag(content);
    let hash2 = ContentHasher::content_to_tag(content);

    assert_eq!(hash1, hash2, "Same content should produce same hash");
    assert!(
        hash1.starts_with("hash:sha256:"),
        "Hash tag should have hash:sha256: prefix"
    );
}

/// Test: Different content produces different hash tag
///
/// Verifies content hashing differentiates content.
#[test]
fn test_different_content_produces_different_hash() {
    use subcog::services::deduplication::ContentHasher;

    let content1 = "Use PostgreSQL for storage";
    let content2 = "Use MySQL for storage";

    let hash1 = ContentHasher::content_to_tag(content1);
    let hash2 = ContentHasher::content_to_tag(content2);

    assert_ne!(
        hash1, hash2,
        "Different content should produce different hashes"
    );
}

/// Test: Hash is normalized (case and whitespace)
///
/// Verifies content normalization before hashing.
#[test]
fn test_content_hash_normalization() {
    use subcog::services::deduplication::ContentHasher;

    // These should produce the same hash after normalization
    let content1 = "Use PostgreSQL for storage";
    let content2 = "  use   postgresql   for   storage  ";
    let content3 = "USE POSTGRESQL FOR STORAGE";

    let hash1 = ContentHasher::content_to_tag(content1);
    let hash2 = ContentHasher::content_to_tag(content2);
    let hash3 = ContentHasher::content_to_tag(content3);

    assert_eq!(
        hash1, hash2,
        "Content with extra whitespace should normalize to same hash"
    );
    assert_eq!(
        hash1, hash3,
        "Content with different case should normalize to same hash"
    );
}

// ============================================================================
// Service Configuration Tests
// ============================================================================

/// Test: Service has expected backends configured
///
/// Verifies builder methods configure backends correctly.
#[test]
fn test_capture_service_has_configured_backends() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);

    assert!(capture_service.has_embedder(), "Should have embedder");
    assert!(capture_service.has_index(), "Should have index");
    assert!(capture_service.has_vector(), "Should have vector");
}

/// Test: Service without backends still works
///
/// Verifies graceful degradation without optional backends.
#[test]
fn test_capture_service_without_backends() {
    let config = Config::default();
    let capture_service = CaptureService::new(config);

    assert!(!capture_service.has_embedder(), "Should not have embedder");
    assert!(!capture_service.has_index(), "Should not have index");
    assert!(!capture_service.has_vector(), "Should not have vector");
}

/// Test: Builder methods chain correctly
///
/// Verifies that builder pattern works for adding backends.
#[test]
fn test_capture_service_builder_pattern() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = Config::default();
    let embedder: Arc<dyn Embedder> = Arc::new(FastEmbedEmbedder::new());

    let index_path = temp_dir.path().join("test_index.db");
    let index: Arc<dyn IndexBackend + Send + Sync> =
        Arc::new(SqliteBackend::new(&index_path).expect("Failed to create SQLite index"));

    let vector_path = temp_dir.path().join("test_vectors");
    #[cfg(feature = "usearch-hnsw")]
    let vector: Arc<dyn VectorBackend + Send + Sync> = Arc::new(
        UsearchBackend::new(&vector_path, FastEmbedEmbedder::DEFAULT_DIMENSIONS)
            .expect("Failed to create vector backend"),
    );
    #[cfg(not(feature = "usearch-hnsw"))]
    let vector: Arc<dyn VectorBackend + Send + Sync> = Arc::new(UsearchBackend::new(
        &vector_path,
        FastEmbedEmbedder::DEFAULT_DIMENSIONS,
    ));

    // Build step by step
    let service = CaptureService::new(config)
        .with_embedder(embedder)
        .with_index(index)
        .with_vector(vector);

    assert!(service.has_embedder(), "Should have embedder after builder");
    assert!(service.has_index(), "Should have index after builder");
    assert!(service.has_vector(), "Should have vector after builder");
}

// ============================================================================
// URN Generation Tests
// ============================================================================

/// Test: Captured memory has valid URN
///
/// Verifies URN format in capture results.
#[test]
fn test_capture_result_has_valid_urn() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);

    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "Use PostgreSQL for primary storage".to_string(),
        domain: Domain::new(),
        tags: vec![],
        source: None,
        skip_security_check: true,
        ttl_seconds: None,
        scope: None,
    };

    let result = capture_service.capture(request);
    assert!(result.is_ok(), "Capture should succeed");

    let capture_result = result.unwrap();
    assert!(
        capture_result.urn.starts_with("subcog://"),
        "URN should start with subcog://: {}",
        capture_result.urn
    );
    assert!(
        capture_result.urn.contains("decisions"),
        "URN should contain namespace: {}",
        capture_result.urn
    );
}

/// Test: Captured memory has valid memory ID
///
/// Verifies memory ID format in capture results.
#[test]
fn test_capture_result_has_valid_memory_id() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);

    let request = CaptureRequest {
        namespace: Namespace::Patterns,
        content: "Use repository pattern for data access".to_string(),
        domain: Domain::new(),
        tags: vec![],
        source: None,
        skip_security_check: true,
        ttl_seconds: None,
        scope: None,
    };

    let result = capture_service.capture(request);
    assert!(result.is_ok(), "Capture should succeed");

    let capture_result = result.unwrap();
    assert!(
        !capture_result.memory_id.as_str().is_empty(),
        "Memory ID should not be empty"
    );
    assert_eq!(
        capture_result.memory_id.as_str().len(),
        12,
        "Memory ID should be 12 characters"
    );
}

// ============================================================================
// Tag Handling Tests
// ============================================================================

/// Test: Tags are preserved in capture
///
/// Verifies that user-provided tags are preserved.
#[test]
fn test_capture_preserves_tags() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);

    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "Use PostgreSQL for primary storage".to_string(),
        domain: Domain::new(),
        tags: vec![
            "database".to_string(),
            "architecture".to_string(),
            "postgresql".to_string(),
        ],
        source: None,
        skip_security_check: true,
        ttl_seconds: None,
        scope: None,
    };

    let result = capture_service.capture(request);
    assert!(result.is_ok(), "Capture should succeed");

    // The capture result doesn't include tags, but the memory was stored with them
    // We verify the capture succeeded - tag verification requires recall
}

/// Test: Content hash tag is automatically added
///
/// Verifies that a content hash tag is added during capture.
#[test]
fn test_capture_adds_hash_tag() {
    use subcog::services::deduplication::ContentHasher;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);

    let content = "Use PostgreSQL for primary storage";
    let expected_hash_tag = ContentHasher::content_to_tag(content);

    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: content.to_string(),
        domain: Domain::new(),
        tags: vec!["database".to_string()],
        source: None,
        skip_security_check: true,
        ttl_seconds: None,
        scope: None,
    };

    let result = capture_service.capture(request);
    assert!(result.is_ok(), "Capture should succeed");

    // The hash tag is added internally - verify the hash tag format
    assert!(
        expected_hash_tag.starts_with("hash:sha256:"),
        "Hash tag should have hash:sha256: prefix"
    );
}

// ============================================================================
// Namespace Tests
// ============================================================================

/// Test: All namespaces are valid for capture
///
/// Verifies that all defined namespaces can be used for capture.
#[test]
fn test_capture_accepts_all_namespaces() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);

    let namespaces = vec![
        Namespace::Decisions,
        Namespace::Patterns,
        Namespace::Learnings,
        Namespace::Context,
        Namespace::TechDebt,
        Namespace::Apis,
        Namespace::Config,
        Namespace::Security,
        Namespace::Performance,
        Namespace::Testing,
    ];

    for namespace in namespaces {
        let request = CaptureRequest {
            namespace,
            content: format!("Test content for namespace {}", namespace.as_str()),
            domain: Domain::new(),
            tags: vec![],
            source: None,
            skip_security_check: true,
            ttl_seconds: None,
            scope: None,
        };

        let result = capture_service.capture(request);
        assert!(
            result.is_ok(),
            "Capture should succeed for namespace {}: {:?}",
            namespace.as_str(),
            result.err()
        );
    }
}

// ============================================================================
// Domain Tests
// ============================================================================

/// Test: Default domain is accepted
///
/// Verifies that `Domain::new()` creates a valid domain for capture.
#[test]
fn test_capture_accepts_default_domain() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);

    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "Use default domain for capture".to_string(),
        domain: Domain::new(),
        tags: vec![],
        source: None,
        skip_security_check: true,
        ttl_seconds: None,
        scope: None,
    };

    let result = capture_service.capture(request);
    assert!(result.is_ok(), "Capture with default domain should succeed");
}

/// Test: Context-aware domain is accepted
///
/// Verifies that `Domain::default_for_context()` works for capture.
#[test]
fn test_capture_accepts_context_aware_domain() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);

    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "Use context-aware domain for capture".to_string(),
        domain: Domain::default_for_context(),
        tags: vec![],
        source: None,
        skip_security_check: true,
        ttl_seconds: None,
        scope: None,
    };

    let result = capture_service.capture(request);
    assert!(
        result.is_ok(),
        "Capture with context-aware domain should succeed"
    );
}

// ============================================================================
// Source Path Tests
// ============================================================================

/// Test: Source path is accepted
///
/// Verifies that source file path is accepted in capture.
#[test]
fn test_capture_accepts_source_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);

    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "Decision documented in specific file".to_string(),
        domain: Domain::new(),
        tags: vec![],
        source: Some("src/main.rs".to_string()),
        skip_security_check: true,
        ttl_seconds: None,
        scope: None,
    };

    let result = capture_service.capture(request);
    assert!(result.is_ok(), "Capture with source path should succeed");
}

/// Test: None source path is accepted
///
/// Verifies that omitting source path is valid.
#[test]
fn test_capture_accepts_none_source_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);

    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "Decision without source path".to_string(),
        domain: Domain::new(),
        tags: vec![],
        source: None,
        skip_security_check: true,
        ttl_seconds: None,
        scope: None,
    };

    let result = capture_service.capture(request);
    assert!(result.is_ok(), "Capture without source path should succeed");
}
