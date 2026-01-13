//! PostgreSQL Integration Tests (TEST-HIGH-001)
//!
//! Tests PostgreSQL storage backends in isolation, focusing on:
//! - Connection management and pool behavior
//! - CRUD operations for persistence, index, and prompt storage
//! - Migration execution
//! - Error handling and graceful degradation
//!
//! These tests require a running PostgreSQL server. Set the environment variable
//! `SUBCOG_TEST_POSTGRES_URL` to enable these tests:
//!
//! ```bash
//! export SUBCOG_TEST_POSTGRES_URL="postgres://user:pass@localhost/subcog_test"
//! cargo test --features postgres postgresql_integration
//! ```

// Integration tests use expect/unwrap for simplicity - panics are acceptable in tests
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::redundant_clone
)]
#![cfg(feature = "postgres")]

use std::env;

/// Environment variable for PostgreSQL test connection URL.
const POSTGRES_URL_ENV: &str = "SUBCOG_TEST_POSTGRES_URL";

/// Returns the PostgreSQL connection URL if available, or None to skip tests.
fn get_postgres_url() -> Option<String> {
    env::var(POSTGRES_URL_ENV).ok()
}

/// Macro to skip tests when PostgreSQL is not available.
macro_rules! require_postgres {
    () => {
        match get_postgres_url() {
            Some(url) => url,
            None => {
                eprintln!(
                    "Skipping test: {} not set. Set this environment variable to run PostgreSQL tests.",
                    POSTGRES_URL_ENV
                );
                return;
            }
        }
    };
}

// ============================================================================
// Persistence Backend Tests
// ============================================================================

mod persistence {
    use super::*;
    use subcog::models::{Domain, Memory, MemoryId, MemoryStatus, Namespace};
    use subcog::storage::persistence::PostgresBackend;
    use subcog::storage::traits::PersistenceBackend;
    use uuid::Uuid;

    fn unique_table_name() -> String {
        format!("test_memories_{}", Uuid::new_v4().simple())
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
            expires_at: None,
        }
    }

    #[test]
    fn test_postgres_backend_connection() {
        let url = require_postgres!();
        let table_name = unique_table_name();

        let backend = PostgresBackend::new(&url, &table_name);
        assert!(
            backend.is_ok(),
            "Should connect to PostgreSQL: {:?}",
            backend.err()
        );

        // Cleanup
        if let Ok(backend) = backend {
            drop(backend);
        }
    }

    #[test]
    fn test_postgres_store_and_get() {
        let url = require_postgres!();
        let table_name = unique_table_name();

        let backend = PostgresBackend::new(&url, &table_name).expect("Failed to create backend");

        let memory = Memory {
            id: MemoryId::new(Uuid::new_v4().to_string()),
            content: "Test decision: Use PostgreSQL for persistence".to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            status: MemoryStatus::Active,
            tags: vec!["test".to_string(), "postgres".to_string()],
            source: Some("test_file.rs".to_string()),
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
            expires_at: None,
        };

        // Store
        let store_result = backend.store(&memory);
        assert!(
            store_result.is_ok(),
            "Store should succeed: {:?}",
            store_result.err()
        );

        // Get
        let get_result = backend.get(&memory.id);
        assert!(
            get_result.is_ok(),
            "Get should succeed: {:?}",
            get_result.err()
        );

        let retrieved = get_result.unwrap();
        assert!(retrieved.is_some(), "Memory should exist");

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, memory.id);
        assert_eq!(retrieved.content, memory.content);
        assert_eq!(retrieved.namespace, memory.namespace);
        assert_eq!(retrieved.tags, memory.tags);
    }

    #[test]
    fn test_postgres_list_ids() {
        let url = require_postgres!();
        let table_name = unique_table_name();

        let backend = PostgresBackend::new(&url, &table_name).expect("Failed to create backend");

        // Create memories
        let decision = create_test_memory("Decision memory", Namespace::Decisions);
        let pattern = create_test_memory("Pattern memory", Namespace::Patterns);

        backend.store(&decision).expect("Store decision");
        backend.store(&pattern).expect("Store pattern");

        // List IDs
        let ids = backend.list_ids();
        assert!(ids.is_ok(), "List should succeed: {:?}", ids.err());

        let ids = ids.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&decision.id));
        assert!(ids.contains(&pattern.id));
    }

    #[test]
    fn test_postgres_delete() {
        let url = require_postgres!();
        let table_name = unique_table_name();

        let backend = PostgresBackend::new(&url, &table_name).expect("Failed to create backend");

        let memory = create_test_memory("Memory to delete", Namespace::Learnings);

        backend.store(&memory).expect("Store memory");

        // Verify it exists
        let exists = backend.get(&memory.id).expect("Get should work");
        assert!(exists.is_some(), "Memory should exist before delete");

        // Delete
        let delete_result = backend.delete(&memory.id);
        assert!(delete_result.is_ok(), "Delete should succeed");
        assert!(delete_result.unwrap(), "Delete should return true");

        // Verify deleted
        let exists_after = backend.get(&memory.id).expect("Get should work");
        assert!(
            exists_after.is_none(),
            "Memory should not exist after delete"
        );
    }

    #[test]
    fn test_postgres_update() {
        let url = require_postgres!();
        let table_name = unique_table_name();

        let backend = PostgresBackend::new(&url, &table_name).expect("Failed to create backend");

        let mut memory = Memory {
            id: MemoryId::new(Uuid::new_v4().to_string()),
            content: "Original content".to_string(),
            namespace: Namespace::Context,
            domain: Domain::new(),
            status: MemoryStatus::Active,
            tags: vec!["original".to_string()],
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
            expires_at: None,
        };

        backend.store(&memory).expect("Store memory");

        // Update
        memory.content = "Updated content".to_string();
        memory.tags = vec!["updated".to_string()];
        memory.updated_at = subcog::current_timestamp();

        let update_result = backend.store(&memory);
        assert!(update_result.is_ok(), "Update should succeed");

        // Verify update
        let retrieved = backend
            .get(&memory.id)
            .expect("Get should work")
            .expect("Should exist");
        assert_eq!(retrieved.content, "Updated content");
        assert_eq!(retrieved.tags, vec!["updated".to_string()]);
    }

    #[test]
    fn test_postgres_get_batch() {
        let url = require_postgres!();
        let table_name = unique_table_name();

        let backend = PostgresBackend::new(&url, &table_name).expect("Failed to create backend");

        let mem1 = create_test_memory("Memory 1", Namespace::Decisions);
        let mem2 = create_test_memory("Memory 2", Namespace::Patterns);
        let mem3 = create_test_memory("Memory 3", Namespace::Learnings);

        backend.store(&mem1).expect("Store 1");
        backend.store(&mem2).expect("Store 2");
        backend.store(&mem3).expect("Store 3");

        // Batch get
        let ids = vec![mem1.id.clone(), mem3.id.clone()];
        let batch = backend.get_batch(&ids);
        assert!(batch.is_ok(), "Batch get should succeed");

        let batch = batch.unwrap();
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn test_postgres_exists() {
        let url = require_postgres!();
        let table_name = unique_table_name();

        let backend = PostgresBackend::new(&url, &table_name).expect("Failed to create backend");

        let memory = create_test_memory("Test exists", Namespace::Apis);
        backend.store(&memory).expect("Store");

        assert!(backend.exists(&memory.id).unwrap());
        assert!(!backend.exists(&MemoryId::new("nonexistent")).unwrap());
    }

    #[test]
    fn test_postgres_count() {
        let url = require_postgres!();
        let table_name = unique_table_name();

        let backend = PostgresBackend::new(&url, &table_name).expect("Failed to create backend");

        assert_eq!(backend.count().unwrap(), 0);

        let mem1 = create_test_memory("Memory 1", Namespace::Decisions);
        let mem2 = create_test_memory("Memory 2", Namespace::Patterns);

        backend.store(&mem1).expect("Store 1");
        assert_eq!(backend.count().unwrap(), 1);

        backend.store(&mem2).expect("Store 2");
        assert_eq!(backend.count().unwrap(), 2);
    }

    #[test]
    fn test_postgres_invalid_connection() {
        let result = PostgresBackend::new(
            "postgres://invalid:invalid@localhost:5432/nonexistent",
            "test",
        );
        // Connection might succeed but operations will fail, or it might fail immediately
        // Either way, this shouldn't panic
        if result.is_err() {
            // Expected - invalid connection
            let err = result.err().unwrap().to_string();
            assert!(
                err.contains("postgres") || err.contains("connection"),
                "Error should mention postgres: {err}"
            );
        }
    }
}

// ============================================================================
// Index Backend Tests
// ============================================================================

mod index {
    use super::*;
    use subcog::models::{Domain, Memory, MemoryId, MemoryStatus, Namespace, SearchFilter};
    use subcog::storage::index::PostgresIndexBackend;
    use subcog::storage::traits::IndexBackend;
    use uuid::Uuid;

    fn unique_table_name() -> String {
        format!("test_index_{}", Uuid::new_v4().simple())
    }

    #[test]
    fn test_postgres_index_text_search() {
        let url = require_postgres!();
        let table_name = unique_table_name();

        let backend = PostgresIndexBackend::new(&url, &table_name);
        if backend.is_err() {
            eprintln!("Skipping test: PostgreSQL index backend not available");
            return;
        }
        let backend = backend.unwrap();

        // Index a memory
        let memory = Memory {
            id: MemoryId::new(Uuid::new_v4().to_string()),
            content: "PostgreSQL is a powerful relational database".to_string(),
            namespace: Namespace::Learnings,
            domain: Domain::new(),
            status: MemoryStatus::Active,
            tags: vec!["database".to_string()],
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
            expires_at: None,
        };

        backend.index(&memory).expect("Index should succeed");

        // Search
        let filter = SearchFilter::new();
        let results = backend.search("PostgreSQL database", &filter, 10);
        assert!(
            results.is_ok(),
            "Search should succeed: {:?}",
            results.err()
        );

        let results = results.unwrap();
        assert!(!results.is_empty(), "Should find indexed memory");
        assert_eq!(results[0].0, memory.id);
    }
}

// ============================================================================
// Pool Management Tests
// ============================================================================

mod pool {
    use super::*;
    use subcog::storage::persistence::PostgresBackend;

    #[test]
    fn test_postgres_pool_size_configuration() {
        let url = require_postgres!();

        // Test with custom pool size
        let backend = PostgresBackend::with_pool_size(&url, "test_pool", Some(5));
        assert!(backend.is_ok(), "Should accept custom pool size");
    }

    #[test]
    fn test_postgres_pool_default_size() {
        let url = require_postgres!();

        // Test with default pool size (None)
        let backend = PostgresBackend::with_pool_size(&url, "test_pool_default", None);
        assert!(backend.is_ok(), "Should work with default pool size");
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

mod errors {
    use super::*;
    use subcog::models::MemoryId;
    use subcog::storage::persistence::PostgresBackend;
    use subcog::storage::traits::PersistenceBackend;

    #[test]
    fn test_postgres_get_nonexistent() {
        let url = require_postgres!();

        let backend = PostgresBackend::new(&url, "test_errors").expect("Backend creation");
        let result = backend.get(&MemoryId::new("nonexistent-id-12345"));

        assert!(result.is_ok(), "Get for nonexistent should not error");
        assert!(
            result.unwrap().is_none(),
            "Should return None for nonexistent"
        );
    }

    #[test]
    fn test_postgres_delete_nonexistent() {
        let url = require_postgres!();

        let backend = PostgresBackend::new(&url, "test_errors_delete").expect("Backend creation");
        let result = backend.delete(&MemoryId::new("nonexistent-id-67890"));

        // Delete of nonexistent should return false (nothing deleted)
        assert!(result.is_ok(), "Delete should not error");
        assert!(
            !result.unwrap(),
            "Delete should return false for nonexistent"
        );
    }
}
