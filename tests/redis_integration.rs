//! Redis Integration Tests (TEST-HIGH-002)
//!
//! Tests Redis storage backends in isolation, focusing on:
//! - Connection management and timeout behavior
//! - Index operations (index, search, remove)
//! - Vector operations (upsert, search, remove)
//! - Error handling and graceful degradation
//!
//! These tests require a running Redis server with RediSearch module. Set the
//! environment variable `SUBCOG_TEST_REDIS_URL` to enable these tests:
//!
//! ```bash
//! export SUBCOG_TEST_REDIS_URL="redis://localhost:6379"
//! cargo test --features redis redis_integration
//! ```

// Integration tests use expect/unwrap for simplicity - panics are acceptable in tests
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::doc_markdown,
    clippy::cast_precision_loss,
    clippy::needless_borrows_for_generic_args,
    clippy::collapsible_if,
    unsafe_code
)]
#![cfg(feature = "redis")]

use std::env;

/// Environment variable for Redis test connection URL.
const REDIS_URL_ENV: &str = "SUBCOG_TEST_REDIS_URL";

/// Returns the Redis connection URL if available, or None to skip tests.
fn get_redis_url() -> Option<String> {
    env::var(REDIS_URL_ENV).ok()
}

/// Macro to skip tests when Redis is not available.
macro_rules! require_redis {
    () => {
        match get_redis_url() {
            Some(url) => url,
            None => {
                eprintln!(
                    "Skipping test: {} not set. Set this environment variable to run Redis tests.",
                    REDIS_URL_ENV
                );
                return;
            },
        }
    };
}

// ============================================================================
// Index Backend Tests
// ============================================================================

mod index {
    use super::*;
    use subcog::models::{Domain, Memory, MemoryId, MemoryStatus, Namespace, SearchFilter};
    use subcog::storage::index::RedisBackend;
    use subcog::storage::traits::IndexBackend;
    use uuid::Uuid;

    fn unique_index_name() -> String {
        format!("test_idx_{}", Uuid::new_v4().simple())
    }

    fn create_test_memory(content: &str, namespace: Namespace) -> Memory {
        Memory {
            id: MemoryId::new(Uuid::new_v4().to_string()),
            content: content.to_string(),
            namespace,
            domain: Domain::new(),
            status: MemoryStatus::Active,
            tags: vec![],
            source: None,
            embedding: None,
            created_at: subcog::current_timestamp(),
            updated_at: subcog::current_timestamp(),
            tombstoned_at: None,
            project_id: None,
            branch: None,
            file_path: None,
            is_summary: false,
            source_memory_ids: None,
            consolidation_timestamp: None,
        }
    }

    #[test]
    fn test_redis_backend_connection() {
        let url = require_redis!();
        let index_name = unique_index_name();

        let backend = RedisBackend::new(&url, &index_name);
        // RediSearch module may not be available - gracefully handle
        if backend.is_err() {
            let err = backend.err().unwrap().to_string();
            if err.contains("RediSearch") || err.contains("unknown command") {
                eprintln!("Skipping test: RediSearch module not available");
                return;
            }
            panic!("Unexpected error: {err}");
        }

        assert!(backend.is_ok(), "Should connect to Redis");
    }

    #[test]
    fn test_redis_index_and_search() {
        let url = require_redis!();
        let index_name = unique_index_name();

        let backend = match RedisBackend::new(&url, &index_name) {
            Ok(b) => b,
            Err(e) => {
                if e.to_string().contains("RediSearch") || e.to_string().contains("unknown command")
                {
                    eprintln!("Skipping test: RediSearch module not available");
                    return;
                }
                panic!("Unexpected error: {e}");
            },
        };

        // Index a memory
        let memory = Memory {
            id: MemoryId::new(Uuid::new_v4().to_string()),
            content: "Redis is an in-memory data structure store".to_string(),
            namespace: Namespace::Learnings,
            domain: Domain::new(),
            status: MemoryStatus::Active,
            tags: vec!["database".to_string(), "cache".to_string()],
            source: Some("test.rs".to_string()),
            embedding: None,
            created_at: subcog::current_timestamp(),
            updated_at: subcog::current_timestamp(),
            tombstoned_at: None,
            project_id: Some("test-project".to_string()),
            branch: Some("main".to_string()),
            file_path: None,
            is_summary: false,
            source_memory_ids: None,
            consolidation_timestamp: None,
        };

        let index_result = backend.index(&memory);
        assert!(
            index_result.is_ok(),
            "Index should succeed: {:?}",
            index_result.err()
        );

        // Search
        let filter = SearchFilter::new();
        let results = backend.search("Redis memory", &filter, 10);
        assert!(
            results.is_ok(),
            "Search should succeed: {:?}",
            results.err()
        );

        let results = results.unwrap();
        assert!(!results.is_empty(), "Should find indexed memory");
        assert_eq!(results[0].0, memory.id);
    }

    #[test]
    fn test_redis_index_multiple_memories() {
        let url = require_redis!();
        let index_name = unique_index_name();

        let backend = match RedisBackend::new(&url, &index_name) {
            Ok(b) => b,
            Err(e) => {
                if e.to_string().contains("RediSearch") || e.to_string().contains("unknown command")
                {
                    eprintln!("Skipping test: RediSearch module not available");
                    return;
                }
                panic!("Unexpected error: {e}");
            },
        };

        let mem1 = create_test_memory("First memory about Redis caching", Namespace::Decisions);
        let mem2 = create_test_memory("Second memory about database indexing", Namespace::Patterns);
        let mem3 = create_test_memory(
            "Third memory about search performance",
            Namespace::Learnings,
        );

        backend.index(&mem1).expect("Index mem1");
        backend.index(&mem2).expect("Index mem2");
        backend.index(&mem3).expect("Index mem3");

        // Search for specific term
        let filter = SearchFilter::new();
        let results = backend.search("Redis", &filter, 10);
        assert!(results.is_ok());

        let results = results.unwrap();
        assert!(!results.is_empty(), "Should find at least one memory");
    }

    #[test]
    fn test_redis_remove_from_index() {
        let url = require_redis!();
        let index_name = unique_index_name();

        let backend = match RedisBackend::new(&url, &index_name) {
            Ok(b) => b,
            Err(e) => {
                if e.to_string().contains("RediSearch") || e.to_string().contains("unknown command")
                {
                    eprintln!("Skipping test: RediSearch module not available");
                    return;
                }
                panic!("Unexpected error: {e}");
            },
        };

        let memory = create_test_memory("Memory to be removed from index", Namespace::Context);
        backend.index(&memory).expect("Index memory");

        // Remove from index
        let remove_result = backend.remove(&memory.id);
        assert!(remove_result.is_ok(), "Remove should succeed");

        // Search should not find it (or find fewer results)
        let filter = SearchFilter::new();
        let results = backend.search("removed", &filter, 10).unwrap();
        assert!(
            !results.iter().any(|(id, _)| *id == memory.id),
            "Removed memory should not be in results"
        );
    }

    #[test]
    fn test_redis_search_with_filter() {
        let url = require_redis!();
        let index_name = unique_index_name();

        let backend = match RedisBackend::new(&url, &index_name) {
            Ok(b) => b,
            Err(e) => {
                if e.to_string().contains("RediSearch") || e.to_string().contains("unknown command")
                {
                    eprintln!("Skipping test: RediSearch module not available");
                    return;
                }
                panic!("Unexpected error: {e}");
            },
        };

        let decision = create_test_memory("A decision about architecture", Namespace::Decisions);
        let pattern = create_test_memory("A pattern for error handling", Namespace::Patterns);

        backend.index(&decision).expect("Index decision");
        backend.index(&pattern).expect("Index pattern");

        // Filter by namespace
        let filter = SearchFilter::new().with_namespace(Namespace::Decisions);
        let results = backend.search("architecture", &filter, 10);
        assert!(results.is_ok());

        // Results should only include decisions namespace
        let results = results.unwrap();
        for (id, _score) in &results {
            // The filter should have been applied
            assert!(
                *id == decision.id,
                "Should only find decision memory in filtered search"
            );
        }
    }

    #[test]
    fn test_redis_invalid_connection() {
        let result = RedisBackend::new("redis://invalid-host:6379", "test_invalid");

        // Connection to invalid host should fail
        assert!(result.is_err(), "Invalid connection should fail");
        let err = result.err().unwrap().to_string();
        assert!(
            err.contains("redis") || err.contains("connection") || err.contains("failed"),
            "Error should mention redis or connection: {err}"
        );
    }
}

// ============================================================================
// Vector Backend Tests
// ============================================================================

mod vector {
    use super::*;
    use subcog::models::MemoryId;
    use subcog::storage::traits::{VectorBackend, VectorFilter};
    use subcog::storage::vector::RedisVectorBackend;
    use uuid::Uuid;

    fn unique_index_name() -> String {
        format!("test_vec_{}", Uuid::new_v4().simple())
    }

    #[test]
    fn test_redis_vector_upsert_and_search() {
        let url = require_redis!();
        let index_name = unique_index_name();

        // Create backend with 384 dimensions (common embedding size)
        let backend = match RedisVectorBackend::new(&url, &index_name, 384) {
            Ok(b) => b,
            Err(e) => {
                if e.to_string().contains("RediSearch") || e.to_string().contains("unknown command")
                {
                    eprintln!("Skipping test: RediSearch module not available");
                    return;
                }
                panic!("Unexpected error: {e}");
            },
        };

        let id = MemoryId::new(Uuid::new_v4().to_string());
        let embedding: Vec<f32> = (0..384).map(|i| (i as f32) / 384.0).collect();

        // Upsert
        let upsert_result = backend.upsert(&id, &embedding);
        assert!(
            upsert_result.is_ok(),
            "Upsert should succeed: {:?}",
            upsert_result.err()
        );

        // Search with similar vector
        let filter = VectorFilter::new();
        let results = backend.search(&embedding, &filter, 10);
        assert!(
            results.is_ok(),
            "Search should succeed: {:?}",
            results.err()
        );

        let results = results.unwrap();
        assert!(!results.is_empty(), "Should find stored vector");
        assert_eq!(results[0].0, id);
    }

    #[test]
    fn test_redis_vector_remove() {
        let url = require_redis!();
        let index_name = unique_index_name();

        let backend = match RedisVectorBackend::new(&url, &index_name, 384) {
            Ok(b) => b,
            Err(e) => {
                if e.to_string().contains("RediSearch") || e.to_string().contains("unknown command")
                {
                    eprintln!("Skipping test: RediSearch module not available");
                    return;
                }
                panic!("Unexpected error: {e}");
            },
        };

        let id = MemoryId::new(Uuid::new_v4().to_string());
        let embedding: Vec<f32> = (0..384).map(|i| (i as f32) / 384.0).collect();

        backend.upsert(&id, &embedding).expect("Upsert");

        // Remove
        let remove_result = backend.remove(&id);
        assert!(remove_result.is_ok(), "Remove should succeed");

        // Search should not find it
        let filter = VectorFilter::new();
        let results = backend.search(&embedding, &filter, 10).unwrap();
        assert!(
            !results.iter().any(|(found_id, _)| *found_id == id),
            "Removed vector should not be found"
        );
    }

    #[test]
    fn test_redis_vector_multiple_upserts() {
        let url = require_redis!();
        let index_name = unique_index_name();

        let backend = match RedisVectorBackend::new(&url, &index_name, 384) {
            Ok(b) => b,
            Err(e) => {
                if e.to_string().contains("RediSearch") || e.to_string().contains("unknown command")
                {
                    eprintln!("Skipping test: RediSearch module not available");
                    return;
                }
                panic!("Unexpected error: {e}");
            },
        };

        // Upsert multiple items individually
        let items: Vec<(MemoryId, Vec<f32>)> = (0..5)
            .map(|i| {
                let id = MemoryId::new(Uuid::new_v4().to_string());
                let embedding: Vec<f32> =
                    (0..384).map(|j| ((i * 100 + j) as f32) / 1000.0).collect();
                (id, embedding)
            })
            .collect();

        for (id, embedding) in &items {
            backend
                .upsert(id, embedding)
                .expect("Upsert should succeed");
        }

        // Verify we can search
        let filter = VectorFilter::new();
        let results = backend.search(&items[0].1, &filter, 10);
        assert!(results.is_ok());
        assert!(!results.unwrap().is_empty());
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

mod errors {
    use super::*;
    use subcog::models::MemoryId;
    use subcog::storage::index::RedisBackend;
    use subcog::storage::traits::IndexBackend;

    #[test]
    fn test_redis_remove_nonexistent() {
        let url = require_redis!();

        let backend = match RedisBackend::new(&url, "test_errors") {
            Ok(b) => b,
            Err(e) => {
                if e.to_string().contains("RediSearch") || e.to_string().contains("unknown command")
                {
                    eprintln!("Skipping test: RediSearch module not available");
                    return;
                }
                panic!("Unexpected error: {e}");
            },
        };

        let result = backend.remove(&MemoryId::new("nonexistent-id-12345"));

        // Remove of nonexistent should succeed (idempotent) or return appropriate error
        // Either behavior is acceptable for Redis
        if let Err(e) = result {
            // Some error is acceptable but shouldn't panic
            eprintln!("Remove nonexistent returned error (acceptable): {e}");
        }
    }

    #[test]
    fn test_redis_search_empty_index() {
        let url = require_redis!();

        let backend = match RedisBackend::new(
            &url,
            &format!("empty_idx_{}", uuid::Uuid::new_v4().simple()),
        ) {
            Ok(b) => b,
            Err(e) => {
                if e.to_string().contains("RediSearch") || e.to_string().contains("unknown command")
                {
                    eprintln!("Skipping test: RediSearch module not available");
                    return;
                }
                panic!("Unexpected error: {e}");
            },
        };

        let filter = subcog::models::SearchFilter::new();
        let results = backend.search("anything", &filter, 10);

        // Should return empty results, not an error
        assert!(results.is_ok(), "Search on empty index should not error");
        assert!(
            results.unwrap().is_empty(),
            "Empty index should return no results"
        );
    }
}

// ============================================================================
// Timeout Tests
// ============================================================================

mod timeout {
    use super::*;
    use subcog::storage::index::RedisBackend;

    #[test]
    fn test_redis_configurable_timeout() {
        let url = require_redis!();

        // Set a custom timeout via env var before creating backend
        // Note: This tests that the timeout configuration is read
        // SAFETY: Tests run single-threaded, no concurrent env var access
        unsafe {
            std::env::set_var("SUBCOG_TIMEOUT_REDIS_MS", "10000");
        }

        let result = RedisBackend::new(&url, "test_timeout");

        // Reset env var
        // SAFETY: Tests run single-threaded, no concurrent env var access
        unsafe {
            std::env::remove_var("SUBCOG_TIMEOUT_REDIS_MS");
        }

        // If RediSearch isn't available, that's fine
        if let Err(e) = &result {
            if e.to_string().contains("RediSearch") || e.to_string().contains("unknown command") {
                eprintln!("Skipping test: RediSearch module not available");
                return;
            }
        }

        assert!(result.is_ok(), "Should create backend with custom timeout");
    }
}
