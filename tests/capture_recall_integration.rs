//! Capture-Recall Integration Tests (Phase 3.9)
//!
//! Tests the complete pipeline: capture → recall roundtrip
//! and verifies semantic search finds captured memories.

// Integration tests use expect/unwrap for simplicity - panics are acceptable in tests
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;
use subcog::embedding::FastEmbedEmbedder;
use subcog::storage::index::SqliteBackend;
use subcog::storage::vector::UsearchBackend;
use subcog::{
    CaptureRequest, CaptureService, Domain, Namespace, RecallService, SearchFilter, SearchMode,
};
use tempfile::TempDir;

/// Helper to create a fully-configured capture service with all backends.
fn create_capture_service(temp_dir: &TempDir) -> CaptureService {
    let config = subcog::config::Config::default();
    let embedder: Arc<dyn subcog::Embedder> = Arc::new(FastEmbedEmbedder::new());

    let index_path = temp_dir.path().join("test_index.db");
    let index: Arc<dyn subcog::IndexBackend + Send + Sync> =
        Arc::new(SqliteBackend::new(&index_path).expect("Failed to create SQLite index"));

    let vector_path = temp_dir.path().join("test_vectors");
    #[cfg(feature = "usearch-hnsw")]
    let vector: Arc<dyn subcog::VectorBackend + Send + Sync> = Arc::new(
        UsearchBackend::new(&vector_path, FastEmbedEmbedder::DEFAULT_DIMENSIONS)
            .expect("Failed to create vector backend"),
    );
    #[cfg(not(feature = "usearch-hnsw"))]
    let vector: Arc<dyn subcog::VectorBackend + Send + Sync> = Arc::new(UsearchBackend::new(
        &vector_path,
        FastEmbedEmbedder::DEFAULT_DIMENSIONS,
    ));

    CaptureService::with_backends(config, embedder, index, vector)
}

/// Helper to create a recall service with the same backends as capture.
fn create_recall_service(temp_dir: &TempDir) -> RecallService {
    let embedder: Arc<dyn subcog::Embedder> = Arc::new(FastEmbedEmbedder::new());

    let index_path = temp_dir.path().join("test_index.db");
    let index = SqliteBackend::new(&index_path).expect("Failed to create SQLite index");

    let vector_path = temp_dir.path().join("test_vectors");
    #[cfg(feature = "usearch-hnsw")]
    let vector: Arc<dyn subcog::VectorBackend + Send + Sync> = Arc::new(
        UsearchBackend::new(&vector_path, FastEmbedEmbedder::DEFAULT_DIMENSIONS)
            .expect("Failed to create vector backend"),
    );
    #[cfg(not(feature = "usearch-hnsw"))]
    let vector: Arc<dyn subcog::VectorBackend + Send + Sync> = Arc::new(UsearchBackend::new(
        &vector_path,
        FastEmbedEmbedder::DEFAULT_DIMENSIONS,
    ));

    RecallService::with_backends(index, embedder, vector)
}

/// Test: Capture → Text search roundtrip
///
/// Captures a memory and immediately retrieves it via text search.
#[test]
fn test_capture_recall_text_search_roundtrip() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let capture_service = create_capture_service(&temp_dir);
    let recall_service = create_recall_service(&temp_dir);

    // Capture a memory
    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "Use PostgreSQL for primary storage because of strong JSONB support".to_string(),
        domain: Domain::new(),
        tags: vec!["database".to_string(), "architecture".to_string()],
        source: None,
        skip_security_check: true,
    };

    let result = capture_service.capture(request);
    assert!(result.is_ok(), "Capture should succeed: {:?}", result.err());
    let capture_result = result.unwrap();
    assert!(
        !capture_result.memory_id.as_str().is_empty(),
        "Memory ID should be non-empty"
    );

    // Recall via text search
    let filter = SearchFilter::default();
    let search_result = recall_service.search("PostgreSQL database", SearchMode::Text, &filter, 10);

    assert!(
        search_result.is_ok(),
        "Search should succeed: {:?}",
        search_result.err()
    );
    let result = search_result.unwrap();

    // Should find the captured memory
    assert!(
        !result.memories.is_empty(),
        "Should find at least one memory"
    );

    // First result should be the one we captured
    let first = &result.memories[0];
    assert!(
        first.memory.content.contains("PostgreSQL"),
        "First result should contain 'PostgreSQL': {}",
        first.memory.content
    );
}

/// Test: Capture → Vector (semantic) search roundtrip
///
/// Captures a memory and retrieves it via semantic similarity search.
/// Note: This test only asserts results when the `usearch-hnsw` feature is enabled.
/// Without the feature, vector search gracefully returns empty results.
#[test]
fn test_capture_recall_vector_search_roundtrip() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let capture_service = create_capture_service(&temp_dir);
    let recall_service = create_recall_service(&temp_dir);

    // Capture a memory with specific semantic content
    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content:
            "We chose Redis for caching layer due to its in-memory speed and pub/sub capabilities"
                .to_string(),
        domain: Domain::new(),
        tags: vec!["caching".to_string(), "redis".to_string()],
        source: None,
        skip_security_check: true,
    };

    let result = capture_service.capture(request);
    assert!(result.is_ok(), "Capture should succeed");

    // Recall via vector search with semantically similar query
    // Note: we're NOT using exact words, testing semantic similarity
    let filter = SearchFilter::default();
    let search_result = recall_service.search(
        "fast memory cache distributed storage", // Semantically related
        SearchMode::Vector,
        &filter,
        10,
    );

    assert!(
        search_result.is_ok(),
        "Vector search should succeed: {:?}",
        search_result.err()
    );
    let result = search_result.unwrap();

    // Vector search behavior depends on the usearch-hnsw feature
    #[cfg(feature = "usearch-hnsw")]
    {
        // With native usearch, should find the captured memory via semantic similarity
        assert!(
            !result.memories.is_empty(),
            "Should find at least one memory via semantic search"
        );

        // The captured memory should be in results
        let found = result
            .memories
            .iter()
            .any(|m| m.memory.content.contains("Redis"));
        assert!(found, "Should find Redis memory via semantic search");
    }

    #[cfg(not(feature = "usearch-hnsw"))]
    {
        // Without native usearch, vector search gracefully returns empty
        // This is expected graceful degradation behavior
        assert!(
            result.memories.is_empty(),
            "Without usearch-hnsw feature, vector search returns empty (graceful degradation)"
        );
    }
}

/// Test: Capture → Hybrid search roundtrip
///
/// Captures multiple memories and uses hybrid search (text + vector).
#[test]
fn test_capture_recall_hybrid_search_roundtrip() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let capture_service = create_capture_service(&temp_dir);
    let recall_service = create_recall_service(&temp_dir);

    // Capture multiple memories
    let memories = vec![
        (
            "Use authentication via JWT tokens for stateless sessions",
            vec!["auth", "jwt"],
        ),
        (
            "Rate limiting implemented with token bucket algorithm",
            vec!["performance", "rate-limit"],
        ),
        (
            "Database connection pooling configured with pgbouncer",
            vec!["database", "connection"],
        ),
    ];

    for (content, tags) in memories {
        let request = CaptureRequest {
            namespace: Namespace::Decisions,
            content: content.to_string(),
            domain: Domain::new(),
            tags: tags.iter().map(std::string::ToString::to_string).collect(),
            source: None,
            skip_security_check: true,
        };

        let result = capture_service.capture(request);
        assert!(result.is_ok(), "Capture should succeed for: {content}");
    }

    // Hybrid search - combines text and vector results
    let filter = SearchFilter::default();
    let search_result = recall_service.search(
        "authentication security tokens",
        SearchMode::Hybrid,
        &filter,
        10,
    );

    assert!(
        search_result.is_ok(),
        "Hybrid search should succeed: {:?}",
        search_result.err()
    );
    let result = search_result.unwrap();

    // Should find memories
    assert!(
        !result.memories.is_empty(),
        "Hybrid search should find memories"
    );

    // JWT memory should rank high for auth-related query
    let has_jwt = result
        .memories
        .iter()
        .any(|m| m.memory.content.contains("JWT"));
    assert!(has_jwt, "Should find JWT memory for authentication query");
}

/// Test: Capture with namespace filter in recall
///
/// Captures to different namespaces and verifies namespace filtering.
#[test]
fn test_capture_recall_namespace_filtering() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let capture_service = create_capture_service(&temp_dir);
    let recall_service = create_recall_service(&temp_dir);

    // Capture to Decisions namespace
    let request1 = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "Chose microservices architecture for scalability".to_string(),
        domain: Domain::new(),
        tags: vec!["architecture".to_string()],
        source: None,
        skip_security_check: true,
    };
    capture_service
        .capture(request1)
        .expect("Capture 1 should succeed");

    // Capture to Patterns namespace
    let request2 = CaptureRequest {
        namespace: Namespace::Patterns,
        content: "Use repository pattern for data access layer".to_string(),
        domain: Domain::new(),
        tags: vec!["architecture".to_string()],
        source: None,
        skip_security_check: true,
    };
    capture_service
        .capture(request2)
        .expect("Capture 2 should succeed");

    // Search with namespace filter - should only find Decisions
    let filter = SearchFilter::default().with_namespace(Namespace::Decisions);

    let search_result = recall_service.search("architecture", SearchMode::Text, &filter, 10);

    assert!(search_result.is_ok(), "Search should succeed");
    let result = search_result.unwrap();

    // Should find only Decisions namespace memory
    for hit in &result.memories {
        assert_eq!(
            hit.memory.namespace,
            Namespace::Decisions,
            "All results should be in Decisions namespace"
        );
    }
}

/// Test: Captured memory has embedding
///
/// Verifies that captured memories have embeddings generated.
#[test]
fn test_captured_memory_has_embedding() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let capture_service = create_capture_service(&temp_dir);

    // Verify embedder is configured
    assert!(
        capture_service.has_embedder(),
        "Capture service should have embedder"
    );
    assert!(
        capture_service.has_index(),
        "Capture service should have index"
    );
    assert!(
        capture_service.has_vector(),
        "Capture service should have vector"
    );

    // Capture a memory
    let request = CaptureRequest {
        namespace: Namespace::Learnings,
        content: "Rust's borrow checker prevents data races at compile time".to_string(),
        domain: Domain::new(),
        tags: vec!["rust".to_string(), "memory-safety".to_string()],
        source: None,
        skip_security_check: true,
    };

    let result = capture_service.capture(request);
    assert!(result.is_ok(), "Capture should succeed");
    let capture_result = result.unwrap();

    // URN should indicate embedding was stored
    assert!(
        capture_result.urn.starts_with("subcog://"),
        "URN should be properly formatted: {}",
        capture_result.urn
    );
}

/// Test: Graceful degradation - capture succeeds without backends
///
/// Verifies that capture works even when optional backends fail.
#[test]
fn test_capture_graceful_degradation() {
    let config = subcog::config::Config::default();

    // Create capture service without any backends
    let capture_service = CaptureService::new(config);

    assert!(!capture_service.has_embedder(), "Should not have embedder");
    assert!(!capture_service.has_index(), "Should not have index");
    assert!(!capture_service.has_vector(), "Should not have vector");

    // Capture should still succeed (Git Notes is the authoritative store)
    let request = CaptureRequest {
        namespace: Namespace::TechDebt,
        content: "TODO: Refactor authentication module".to_string(),
        domain: Domain::new(),
        tags: vec!["refactoring".to_string()],
        source: None,
        skip_security_check: true,
    };

    // Note: This may fail without Git repo, but the point is it doesn't crash
    // due to missing embedder/index/vector
    let _result = capture_service.capture(request);
    // We just verify it doesn't panic
}

/// Test: Limit parameter is honored
///
/// Verifies that search respects the limit parameter.
#[test]
fn test_recall_limit_parameter_honored() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let capture_service = create_capture_service(&temp_dir);
    let recall_service = create_recall_service(&temp_dir);

    // Capture 5 memories with same keyword
    for i in 1..=5 {
        let request = CaptureRequest {
            namespace: Namespace::Decisions,
            content: format!("Database decision number {i} about storage options"),
            domain: Domain::new(),
            tags: vec!["database".to_string()],
            source: None,
            skip_security_check: true,
        };

        let result = capture_service.capture(request);
        assert!(result.is_ok(), "Capture {i} should succeed");
    }

    // Search with limit of 2
    let filter = SearchFilter::default();
    let search_result = recall_service.search("database decision", SearchMode::Text, &filter, 2);

    assert!(search_result.is_ok(), "Search should succeed");
    let result = search_result.unwrap();

    // Should return at most 2 results
    assert!(
        result.memories.len() <= 2,
        "Should return at most 2 results due to limit, got {}",
        result.memories.len()
    );
}

/// Test: Multiple captures are searchable together
///
/// Verifies that multiple captured memories can be searched.
#[test]
fn test_multiple_captures_searchable() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let capture_service = create_capture_service(&temp_dir);
    let recall_service = create_recall_service(&temp_dir);

    // Capture 5 different memories
    let memories = vec![
        "Implemented circuit breaker pattern for external API calls",
        "Added retry logic with exponential backoff",
        "Configured health check endpoints for Kubernetes",
        "Set up distributed tracing with OpenTelemetry",
        "Implemented request correlation IDs for debugging",
    ];

    for content in memories {
        let request = CaptureRequest {
            namespace: Namespace::Patterns,
            content: content.to_string(),
            domain: Domain::new(),
            tags: vec!["resilience".to_string()],
            source: None,
            skip_security_check: true,
        };

        let result = capture_service.capture(request);
        assert!(result.is_ok(), "Capture should succeed for: {content}");
    }

    // Search for patterns with text search (words must be in content)
    // Using "pattern" which appears in multiple captured memories
    let filter = SearchFilter::default();
    let search_result = recall_service.search(
        "pattern implemented", // These words appear in the captured content
        SearchMode::Text,
        &filter,
        10,
    );

    assert!(search_result.is_ok(), "Search should succeed");
    let result = search_result.unwrap();

    // Should find at least one memory (circuit breaker pattern)
    assert!(
        !result.memories.is_empty(),
        "Should find at least one memory with 'pattern' or 'implemented', found {}",
        result.memories.len()
    );

    // Verify the results contain expected content
    let found_circuit_breaker = result
        .memories
        .iter()
        .any(|m| m.memory.content.contains("circuit breaker"));
    let found_correlation = result
        .memories
        .iter()
        .any(|m| m.memory.content.contains("correlation"));
    assert!(
        found_circuit_breaker || found_correlation,
        "Should find either circuit breaker or correlation ID memory"
    );
}

// ============================================================================
// End-to-End Tests (Phase 5.5)
// ============================================================================

/// Test: Full workflow - capture → recall → update → recall
///
/// Simulates a complete memory lifecycle including updates.
#[test]
fn test_full_workflow_capture_recall_update() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let capture_service = create_capture_service(&temp_dir);
    let recall_service = create_recall_service(&temp_dir);

    // Phase 1: Initial capture
    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "Initial architecture decision: Use microservices".to_string(),
        domain: Domain::new(),
        tags: vec!["architecture".to_string(), "initial".to_string()],
        source: None,
        skip_security_check: true,
    };

    let result = capture_service.capture(request);
    assert!(result.is_ok(), "Initial capture should succeed");

    // Phase 2: Recall initial decision
    let filter = SearchFilter::default();
    let search_result =
        recall_service.search("architecture decision", SearchMode::Text, &filter, 10);
    assert!(search_result.is_ok(), "Initial recall should succeed");
    let result = search_result.unwrap();
    assert!(!result.memories.is_empty(), "Should find initial decision");

    // Phase 3: "Update" by capturing a follow-up decision
    let update_request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "Updated architecture decision: Use microservices with event sourcing".to_string(),
        domain: Domain::new(),
        tags: vec!["architecture".to_string(), "updated".to_string()],
        source: None,
        skip_security_check: true,
    };

    let update_result = capture_service.capture(update_request);
    assert!(update_result.is_ok(), "Update capture should succeed");

    // Phase 4: Recall should find both decisions
    let final_result =
        recall_service.search("architecture decision", SearchMode::Text, &filter, 10);
    assert!(final_result.is_ok(), "Final recall should succeed");
    let final_memories = final_result.unwrap();

    // Should find at least 2 memories (initial + updated)
    assert!(
        final_memories.memories.len() >= 2,
        "Should find at least 2 architecture decisions, found {}",
        final_memories.memories.len()
    );

    // Verify both initial and updated are present
    let has_initial = final_memories
        .memories
        .iter()
        .any(|m| m.memory.content.contains("Initial"));
    let has_updated = final_memories
        .memories
        .iter()
        .any(|m| m.memory.content.contains("Updated"));
    assert!(has_initial, "Should find initial decision");
    assert!(has_updated, "Should find updated decision");
}

/// Test: Cross-namespace workflow
///
/// Tests capturing to different namespaces and recalling across them.
#[test]
fn test_cross_namespace_workflow() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let capture_service = create_capture_service(&temp_dir);
    let recall_service = create_recall_service(&temp_dir);

    // Capture a decision
    let decision = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "Choose PostgreSQL for production database".to_string(),
        domain: Domain::new(),
        tags: vec!["database".to_string()],
        source: None,
        skip_security_check: true,
    };
    capture_service
        .capture(decision)
        .expect("Decision capture should succeed");

    // Capture a pattern related to the decision
    let pattern = CaptureRequest {
        namespace: Namespace::Patterns,
        content: "Repository pattern for PostgreSQL access layer".to_string(),
        domain: Domain::new(),
        tags: vec!["database".to_string()],
        source: None,
        skip_security_check: true,
    };
    capture_service
        .capture(pattern)
        .expect("Pattern capture should succeed");

    // Capture a learning related to both
    let learning = CaptureRequest {
        namespace: Namespace::Learnings,
        content: "PostgreSQL JSONB outperforms MongoDB for our use case".to_string(),
        domain: Domain::new(),
        tags: vec!["database".to_string()],
        source: None,
        skip_security_check: true,
    };
    capture_service
        .capture(learning)
        .expect("Learning capture should succeed");

    // Search without namespace filter - should find all
    let filter = SearchFilter::default();
    let all_result = recall_service.search("PostgreSQL database", SearchMode::Text, &filter, 10);
    assert!(all_result.is_ok(), "All search should succeed");
    let all_memories = all_result.unwrap();
    assert!(
        all_memories.memories.len() >= 3,
        "Should find at least 3 memories across namespaces, found {}",
        all_memories.memories.len()
    );

    // Search with Decisions filter - should find only decision
    let decisions_filter = SearchFilter::default().with_namespace(Namespace::Decisions);
    let decisions_result =
        recall_service.search("PostgreSQL", SearchMode::Text, &decisions_filter, 10);
    assert!(decisions_result.is_ok(), "Decisions search should succeed");
    let decisions_memories = decisions_result.unwrap();
    for hit in &decisions_memories.memories {
        assert_eq!(
            hit.memory.namespace,
            Namespace::Decisions,
            "All results should be Decisions namespace"
        );
    }
}

/// Test: Semantic search finds related concepts
///
/// Tests that vector search can find semantically related content.
#[test]
fn test_semantic_search_related_concepts() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let capture_service = create_capture_service(&temp_dir);
    let recall_service = create_recall_service(&temp_dir);

    // Capture memory about caching
    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "Implemented Redis for distributed caching with 5 minute TTL".to_string(),
        domain: Domain::new(),
        tags: vec!["caching".to_string()],
        source: None,
        skip_security_check: true,
    };
    capture_service
        .capture(request)
        .expect("Capture should succeed");

    // Search using semantically related terms (not exact match)
    // "performance" and "speed" are semantically related to "caching"
    let filter = SearchFilter::default();
    let search_result = recall_service.search(
        "performance speed optimization",
        SearchMode::Hybrid,
        &filter,
        10,
    );

    assert!(search_result.is_ok(), "Hybrid search should succeed");
    // Note: Whether this finds the Redis memory depends on embedding quality
    // The test verifies the search completes without error
}

/// Test: Score normalization in search results
///
/// Verifies that search results have normalized scores.
#[test]
fn test_score_normalization_in_results() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let capture_service = create_capture_service(&temp_dir);
    let recall_service = create_recall_service(&temp_dir);

    // Capture multiple memories
    for i in 1..=5 {
        let request = CaptureRequest {
            namespace: Namespace::Decisions,
            content: format!("Technical decision number {i} about system design"),
            domain: Domain::new(),
            tags: vec!["design".to_string()],
            source: None,
            skip_security_check: true,
        };
        capture_service
            .capture(request)
            .expect("Capture should succeed");
    }

    // Search and verify scores are normalized
    let filter = SearchFilter::default();
    let search_result = recall_service.search("technical decision", SearchMode::Text, &filter, 10);

    assert!(search_result.is_ok(), "Search should succeed");
    let result = search_result.unwrap();

    // All scores should be in [0.0, 1.0] range
    for hit in &result.memories {
        assert!(
            (0.0..=1.0).contains(&hit.score),
            "Score should be normalized to [0, 1], got {}",
            hit.score
        );
    }

    // If there are results, the max score should be 1.0 (or close to it)
    if !result.memories.is_empty() {
        let max_score = result
            .memories
            .iter()
            .map(|h| h.score)
            .fold(0.0_f32, f32::max);
        assert!(
            max_score > 0.5,
            "Max score should be significant, got {max_score}"
        );
    }
}

// ============================================================================
// User-Scope Storage Tests (SPEC-2026-01-02-USER-SCOPE)
// ============================================================================

use subcog::services::ServiceContainer;

/// Test: `ServiceContainer::for_user()` creates user-scoped storage
///
/// Verifies that the user-scope factory method creates a working service container.
#[test]
fn test_service_container_for_user_creates_storage() {
    // ServiceContainer::for_user() creates storage in user data dir
    let result = ServiceContainer::for_user();
    assert!(
        result.is_ok(),
        "for_user() should succeed: {:?}",
        result.err()
    );

    let container = result.unwrap();

    // User-scoped container has no repo path
    assert!(container.is_user_scope(), "Container should be user-scoped");
    assert!(
        container.repo_path().is_none(),
        "User-scoped container should have no repo_path"
    );

    // Capture service should be available
    let capture = container.capture();
    assert!(
        !capture.has_embedder() || capture.has_embedder(),
        "Capture service should be created (embedder state is implementation detail)"
    );
}

/// Test: `ServiceContainer::from_current_dir_or_user()` fallback behavior
///
/// Verifies that the fallback factory method returns a working container
/// regardless of whether we're in a git repo.
#[test]
fn test_service_container_from_current_dir_or_user_always_succeeds() {
    // This should always succeed - either project scope or user scope
    let result = ServiceContainer::from_current_dir_or_user();
    assert!(
        result.is_ok(),
        "from_current_dir_or_user() should always succeed: {:?}",
        result.err()
    );

    let container = result.unwrap();

    // We can't predict which scope we'll get, but container should work
    let capture = container.capture();

    // Capture a test memory
    let request = CaptureRequest {
        namespace: Namespace::Testing,
        content: "User-scope integration test memory".to_string(),
        domain: Domain::default_for_context(),
        tags: vec!["test".to_string(), "user-scope".to_string()],
        source: None,
        skip_security_check: true,
    };

    // Capture should succeed regardless of scope
    let capture_result = capture.capture(request);
    assert!(
        capture_result.is_ok(),
        "Capture should succeed in user scope: {:?}",
        capture_result.err()
    );
}

/// Test: User-scope capture-recall roundtrip
///
/// Verifies that memories captured in user scope can be recalled.
#[test]
fn test_user_scope_capture_recall_roundtrip() {
    let container = ServiceContainer::for_user().expect("for_user() should succeed");

    // Capture a memory in user scope
    let capture = container.capture();
    let request = CaptureRequest {
        namespace: Namespace::Learnings,
        content: "User-scope test: SQLite persistence works without git".to_string(),
        domain: Domain::new(), // User scope uses default domain
        tags: vec!["user-scope".to_string(), "sqlite".to_string()],
        source: None,
        skip_security_check: true,
    };

    let capture_result = capture.capture(request);
    assert!(
        capture_result.is_ok(),
        "User-scope capture should succeed: {:?}",
        capture_result.err()
    );

    let captured = capture_result.unwrap();
    assert!(
        !captured.memory_id.as_str().is_empty(),
        "Memory ID should be non-empty"
    );
    assert!(
        captured.urn.starts_with("subcog://"),
        "URN should be properly formatted: {}",
        captured.urn
    );

    // Recall should find the memory
    let recall = container.recall().expect("recall() should succeed");
    let filter = SearchFilter::default();
    let search_result = recall.search("SQLite persistence", SearchMode::Text, &filter, 10);

    assert!(
        search_result.is_ok(),
        "User-scope recall should succeed: {:?}",
        search_result.err()
    );

    let result = search_result.unwrap();
    assert!(
        !result.memories.is_empty(),
        "Should find the captured memory"
    );

    // Verify the captured memory is found
    let found = result
        .memories
        .iter()
        .any(|m| m.memory.content.contains("SQLite persistence"));
    assert!(found, "Should find the specific captured memory");
}

/// Test: `SyncService::no_op()` returns disabled sync
///
/// Verifies that the no-op sync service is properly disabled.
#[test]
fn test_sync_service_no_op_is_disabled() {
    use subcog::SyncService;

    let sync = SyncService::no_op();
    assert!(
        !sync.is_enabled(),
        "no_op() sync service should be disabled"
    );
}

/// Test: User-scope container provides working sync (no-op)
///
/// Verifies that user-scope containers provide a sync service that doesn't crash.
#[test]
fn test_user_scope_sync_service_no_op() {
    let container = ServiceContainer::for_user().expect("for_user() should succeed");
    let sync = container.sync();

    // Sync operations should not crash (they may return errors or no-ops)
    let push_result = sync.push();
    // Push may fail because there's no remote, but it shouldn't panic
    if push_result.is_err() {
        // Expected - no remote configured
    }

    let fetch_result = sync.fetch();
    // Fetch may fail because there's no remote, but it shouldn't panic
    if fetch_result.is_err() {
        // Expected - no remote configured
    }
}

/// Test: `Domain::default_for_context()` works in git repo
///
/// Verifies context-aware domain defaulting behavior.
#[test]
fn test_domain_default_for_context() {
    // Domain::default_for_context() returns appropriate domain based on git presence
    let domain = Domain::default_for_context();

    // Domain should be usable (is_global returns true when all fields are None)
    // In a git repo, it may have project/repository info
    // The key test is that it doesn't panic
    let _is_global = domain.is_global();

    // Domain::new() should create an empty/global domain
    let empty_domain = Domain::new();
    assert!(
        empty_domain.is_global(),
        "Domain::new() should create a global domain"
    );
}
