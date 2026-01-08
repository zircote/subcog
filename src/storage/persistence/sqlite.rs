//! `SQLite`-based persistence backend.
//!
//! Provides durable storage using `SQLite` as the authoritative source of truth.
//! This backend implements ONLY the persistence operations (store, get, delete, list).
//! For full-text search capabilities, use `SqliteBackend` from the index module.

use crate::models::{Memory, MemoryId};
use crate::storage::sqlite::{
    MemoryRow, acquire_lock, build_memory_from_row, configure_connection, record_operation_metrics,
};
use crate::storage::traits::PersistenceBackend;
use crate::{Error, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;
use tracing::instrument;

/// `SQLite`-based persistence backend.
///
/// This backend handles durable storage of memories using `SQLite`. It does NOT
/// implement full-text search - that functionality is in `SqliteBackend` (index module).
///
/// # Concurrency Model
///
/// Uses a `Mutex<Connection>` for thread-safe access. `SQLite`'s WAL mode and
/// `busy_timeout` pragma mitigate contention:
///
/// - **WAL mode**: Allows concurrent readers with a single writer
/// - **`busy_timeout`**: Waits up to 5 seconds for locks instead of failing immediately
/// - **NORMAL synchronous**: Balances durability with performance
///
/// # Schema
///
/// The persistence backend creates and maintains the `memories` table with:
/// - Core fields: id, namespace, domain, status, `created_at`, `tombstoned_at`
/// - Content: Stored here (not in FTS table)
/// - Facet fields: `project_id`, branch, `file_path`
/// - Metadata: tags, source
///
/// Note: The FTS5 table is managed by the index backend.
pub struct SqlitePersistenceBackend {
    /// Connection to the `SQLite` database.
    ///
    /// Protected by Mutex because `rusqlite::Connection` is not `Sync`.
    /// WAL mode and `busy_timeout` handle concurrent access gracefully.
    conn: Mutex<Connection>,
    /// Path to the `SQLite` database (None for in-memory).
    db_path: Option<PathBuf>,
}

impl SqlitePersistenceBackend {
    /// Creates a new `SQLite` persistence backend.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or initialized.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use subcog::storage::persistence::SqlitePersistenceBackend;
    ///
    /// let backend = SqlitePersistenceBackend::new("./memories.db")?;
    /// # Ok::<(), subcog::Error>(())
    /// ```
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        let conn = Connection::open(&db_path).map_err(|e| Error::OperationFailed {
            operation: "open_sqlite".to_string(),
            cause: e.to_string(),
        })?;

        let backend = Self {
            conn: Mutex::new(conn),
            db_path: Some(db_path),
        };

        backend.initialize()?;
        Ok(backend)
    }

    /// Creates an in-memory `SQLite` persistence backend (useful for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be initialized.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use subcog::storage::persistence::SqlitePersistenceBackend;
    ///
    /// let backend = SqlitePersistenceBackend::in_memory()?;
    /// # Ok::<(), subcog::Error>(())
    /// ```
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| Error::OperationFailed {
            operation: "open_sqlite_in_memory".to_string(),
            cause: e.to_string(),
        })?;

        let backend = Self {
            conn: Mutex::new(conn),
            db_path: None,
        };

        backend.initialize()?;
        Ok(backend)
    }

    /// Returns the database path (None for in-memory).
    #[must_use]
    pub const fn db_path(&self) -> Option<&PathBuf> {
        self.db_path.as_ref()
    }

    /// Initializes the database schema.
    ///
    /// Creates the `memories` table with proper indexes. Does NOT create
    /// the FTS5 table - that's handled by the index backend.
    ///
    /// # Errors
    ///
    /// Returns an error if schema initialization fails.
    fn initialize(&self) -> Result<()> {
        let conn = acquire_lock(&self.conn);

        // Configure connection for optimal performance
        configure_connection(&conn)?;

        // Create the main table for memory storage
        // Note: We store content here in the persistence layer
        conn.execute(
            "CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                namespace TEXT NOT NULL,
                domain TEXT,
                project_id TEXT,
                branch TEXT,
                file_path TEXT,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                tombstoned_at INTEGER,
                tags TEXT,
                source TEXT,
                content TEXT NOT NULL
            )",
            [],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "create_memories_table".to_string(),
            cause: e.to_string(),
        })?;

        // Create indexes for common query patterns
        Self::create_indexes(&conn);

        Ok(())
    }

    /// Creates indexes for optimized queries.
    fn create_indexes(conn: &Connection) {
        // Index on namespace for filtered searches
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_namespace ON memories(namespace)",
            [],
        );

        // Index on domain for domain-scoped searches
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_domain ON memories(domain)",
            [],
        );

        // Index on status for filtered searches
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_status ON memories(status)",
            [],
        );

        // Index on created_at for time-based queries
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_created_at ON memories(created_at DESC)",
            [],
        );

        // Composite index for common filter patterns
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_namespace_status ON memories(namespace, status)",
            [],
        );

        // Compound index for time-filtered namespace queries
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_namespace_created ON memories(namespace, created_at DESC)",
            [],
        );

        // Compound index for source filtering with status
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_source_status ON memories(source, status)",
            [],
        );

        // Facet indexes (ADR-0049)
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_project_id ON memories(project_id)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_project_branch ON memories(project_id, branch)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_file_path ON memories(file_path)",
            [],
        );

        // Partial index for tombstoned memories (ADR-0053)
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_tombstoned ON memories(tombstoned_at) WHERE tombstoned_at IS NOT NULL",
            [],
        );
    }
}

impl PersistenceBackend for SqlitePersistenceBackend {
    #[instrument(skip(self, memory), fields(operation = "store", backend = "sqlite_persistence", memory.id = %memory.id.as_str()))]
    fn store(&self, memory: &Memory) -> Result<()> {
        let start = Instant::now();
        let result = (|| {
            let conn = acquire_lock(&self.conn);

            let tags_str = memory.tags.join(",");
            let domain_str = memory.domain.to_string();

            // Use transaction for atomicity (DB-H2)
            conn.execute("BEGIN IMMEDIATE", [])
                .map_err(|e| Error::OperationFailed {
                    operation: "begin_transaction".to_string(),
                    cause: e.to_string(),
                })?;

            let result = (|| {
                // Insert or replace in main table
                // Note: Cast u64 to i64 for SQLite compatibility (rusqlite doesn't impl ToSql for u64)
                #[allow(clippy::cast_possible_wrap)]
                let created_at_i64 = memory.created_at as i64;
                let tombstoned_at_i64 = memory.tombstoned_at.map(|t| t.timestamp());

                conn.execute(
                    "INSERT OR REPLACE INTO memories (id, namespace, domain, project_id, branch, file_path, status, created_at, tombstoned_at, tags, source, content)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        memory.id.as_str(),
                        memory.namespace.as_str(),
                        domain_str,
                        memory.project_id.as_deref(),
                        memory.branch.as_deref(),
                        memory.file_path.as_deref(),
                        memory.status.as_str(),
                        created_at_i64,
                        tombstoned_at_i64,
                        tags_str,
                        memory.source.as_deref(),
                        memory.content
                    ],
                )
                .map_err(|e| Error::OperationFailed {
                    operation: "insert_memory".to_string(),
                    cause: e.to_string(),
                })?;

                Ok(())
            })();

            if result.is_ok() {
                conn.execute("COMMIT", [])
                    .map_err(|e| Error::OperationFailed {
                        operation: "commit_transaction".to_string(),
                        cause: e.to_string(),
                    })?;
            } else {
                let _ = conn.execute("ROLLBACK", []);
            }

            result
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        record_operation_metrics("sqlite_persistence", "store", start, status);
        result
    }

    #[instrument(skip(self), fields(operation = "get", backend = "sqlite_persistence", memory.id = %id.as_str()))]
    fn get(&self, id: &MemoryId) -> Result<Option<Memory>> {
        let start = Instant::now();
        let result = (|| {
            let conn = acquire_lock(&self.conn);

            // Use fetch_memory_row but without JOIN to FTS table
            // We need to implement a simpler version here
            let row: Option<MemoryRow> = conn
                .query_row(
                    "SELECT id, namespace, domain, project_id, branch, file_path, status, created_at,
                            tombstoned_at, tags, source, content
                     FROM memories
                     WHERE id = ?1",
                    params![id.as_str()],
                    |row| {
                        Ok(MemoryRow {
                            id: row.get(0)?,
                            namespace: row.get(1)?,
                            domain: row.get(2)?,
                            project_id: row.get(3)?,
                            branch: row.get(4)?,
                            file_path: row.get(5)?,
                            status: row.get(6)?,
                            created_at: row.get(7)?,
                            tombstoned_at: row.get(8)?,
                            tags: row.get(9)?,
                            source: row.get(10)?,
                            content: row.get(11)?,
                        })
                    },
                )
                .optional()
                .map_err(|e| Error::OperationFailed {
                    operation: "get_memory".to_string(),
                    cause: e.to_string(),
                })?;

            Ok(row.map(build_memory_from_row))
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        record_operation_metrics("sqlite_persistence", "get", start, status);
        result
    }

    #[instrument(skip(self), fields(operation = "delete", backend = "sqlite_persistence", memory.id = %id.as_str()))]
    fn delete(&self, id: &MemoryId) -> Result<bool> {
        let start = Instant::now();
        let result = (|| {
            let conn = acquire_lock(&self.conn);

            // Use transaction for atomicity (DB-H2)
            conn.execute("BEGIN IMMEDIATE", [])
                .map_err(|e| Error::OperationFailed {
                    operation: "begin_transaction".to_string(),
                    cause: e.to_string(),
                })?;

            let result = (|| {
                // Delete from main table
                let deleted = conn
                    .execute("DELETE FROM memories WHERE id = ?1", params![id.as_str()])
                    .map_err(|e| Error::OperationFailed {
                        operation: "delete_memory".to_string(),
                        cause: e.to_string(),
                    })?;

                Ok(deleted > 0)
            })();

            if result.is_ok() {
                conn.execute("COMMIT", [])
                    .map_err(|e| Error::OperationFailed {
                        operation: "commit_transaction".to_string(),
                        cause: e.to_string(),
                    })?;
            } else {
                let _ = conn.execute("ROLLBACK", []);
            }

            result
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        record_operation_metrics("sqlite_persistence", "delete", start, status);
        result
    }

    #[instrument(
        skip(self),
        fields(operation = "list_ids", backend = "sqlite_persistence")
    )]
    fn list_ids(&self) -> Result<Vec<MemoryId>> {
        let start = Instant::now();
        let result = (|| {
            let conn = acquire_lock(&self.conn);

            let mut stmt =
                conn.prepare("SELECT id FROM memories")
                    .map_err(|e| Error::OperationFailed {
                        operation: "prepare_list_ids".to_string(),
                        cause: e.to_string(),
                    })?;

            let ids: Vec<MemoryId> = stmt
                .query_map([], |row| {
                    let id: String = row.get(0)?;
                    Ok(MemoryId::new(&id))
                })
                .map_err(|e| Error::OperationFailed {
                    operation: "list_ids".to_string(),
                    cause: e.to_string(),
                })?
                .filter_map(std::result::Result::ok)
                .collect();

            Ok(ids)
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        record_operation_metrics("sqlite_persistence", "list_ids", start, status);
        result
    }

    #[instrument(skip(self, ids), fields(operation = "get_batch", backend = "sqlite_persistence", count = ids.len()))]
    fn get_batch(&self, ids: &[MemoryId]) -> Result<Vec<Memory>> {
        let start = Instant::now();

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let result = (|| {
            let conn = acquire_lock(&self.conn);

            // Build placeholders for IN clause
            let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{i}")).collect();

            let sql = format!(
                "SELECT id, namespace, domain, project_id, branch, file_path, status, created_at,
                        tombstoned_at, tags, source, content
                 FROM memories
                 WHERE id IN ({})",
                placeholders.join(", ")
            );

            let mut stmt = conn.prepare(&sql).map_err(|e| Error::OperationFailed {
                operation: "prepare_get_batch".to_string(),
                cause: e.to_string(),
            })?;

            // Collect results into a HashMap for O(1) lookup
            let id_strs: Vec<&str> = ids.iter().map(MemoryId::as_str).collect();
            let mut memory_map: HashMap<String, Memory> = HashMap::with_capacity(ids.len());

            let rows = stmt
                .query_map(rusqlite::params_from_iter(id_strs.iter()), |row| {
                    Ok(MemoryRow {
                        id: row.get(0)?,
                        namespace: row.get(1)?,
                        domain: row.get(2)?,
                        project_id: row.get(3)?,
                        branch: row.get(4)?,
                        file_path: row.get(5)?,
                        status: row.get(6)?,
                        created_at: row.get(7)?,
                        tombstoned_at: row.get(8)?,
                        tags: row.get(9)?,
                        source: row.get(10)?,
                        content: row.get(11)?,
                    })
                })
                .map_err(|e| Error::OperationFailed {
                    operation: "get_batch".to_string(),
                    cause: e.to_string(),
                })?;

            for row_result in rows {
                let row = row_result.map_err(|e| Error::OperationFailed {
                    operation: "get_batch_row".to_string(),
                    cause: e.to_string(),
                })?;
                let memory = build_memory_from_row(row);
                memory_map.insert(memory.id.as_str().to_string(), memory);
            }

            // Return memories in the same order as requested IDs
            let memories: Vec<Memory> = ids
                .iter()
                .filter_map(|id| memory_map.remove(id.as_str()))
                .collect();

            Ok(memories)
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        record_operation_metrics("sqlite_persistence", "get_batch", start, status);
        result
    }

    #[instrument(skip(self), fields(operation = "exists", backend = "sqlite_persistence", memory.id = %id.as_str()))]
    fn exists(&self, id: &MemoryId) -> Result<bool> {
        let start = Instant::now();
        let result = (|| {
            let conn = acquire_lock(&self.conn);

            let exists: bool = conn
                .query_row(
                    "SELECT 1 FROM memories WHERE id = ?1",
                    params![id.as_str()],
                    |_| Ok(true),
                )
                .optional()
                .map_err(|e| Error::OperationFailed {
                    operation: "exists".to_string(),
                    cause: e.to_string(),
                })?
                .unwrap_or(false);

            Ok(exists)
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        record_operation_metrics("sqlite_persistence", "exists", start, status);
        result
    }

    #[instrument(
        skip(self),
        fields(operation = "count", backend = "sqlite_persistence")
    )]
    fn count(&self) -> Result<usize> {
        let start = Instant::now();
        let result = (|| {
            let conn = acquire_lock(&self.conn);

            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0))
                .map_err(|e| Error::OperationFailed {
                    operation: "count".to_string(),
                    cause: e.to_string(),
                })?;

            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            Ok(count as usize)
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        record_operation_metrics("sqlite_persistence", "count", start, status);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, MemoryStatus, Namespace};
    use chrono::{TimeZone, Utc};

    fn create_test_memory(id: &str, content: &str, namespace: Namespace) -> Memory {
        Memory {
            id: MemoryId::new(id),
            content: content.to_string(),
            namespace,
            domain: Domain::new(),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: 1_234_567_890,
            updated_at: 1_234_567_890,
            tombstoned_at: None,
            embedding: None,
            tags: vec!["test".to_string()],
            source: None,
        }
    }

    #[test]
    fn test_store_and_get() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let memory = create_test_memory("id1", "Test content", Namespace::Decisions);
        backend.store(&memory).unwrap();

        let retrieved = backend.get(&memory.id).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, memory.id);
        assert_eq!(retrieved.content, memory.content);
        assert_eq!(retrieved.namespace, memory.namespace);
    }

    #[test]
    fn test_get_nonexistent() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let result = backend.get(&MemoryId::new("nonexistent")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_delete() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let memory = create_test_memory("id1", "Test content", Namespace::Decisions);
        backend.store(&memory).unwrap();

        // Verify it exists
        assert!(backend.get(&memory.id).unwrap().is_some());

        // Delete it
        let deleted = backend.delete(&memory.id).unwrap();
        assert!(deleted);

        // Verify it's gone
        assert!(backend.get(&memory.id).unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let deleted = backend.delete(&MemoryId::new("nonexistent")).unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_list_ids() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let memory1 = create_test_memory("id1", "Content 1", Namespace::Decisions);
        let memory2 = create_test_memory("id2", "Content 2", Namespace::Learnings);
        let memory3 = create_test_memory("id3", "Content 3", Namespace::Patterns);

        backend.store(&memory1).unwrap();
        backend.store(&memory2).unwrap();
        backend.store(&memory3).unwrap();

        let ids = backend.list_ids().unwrap();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&memory1.id));
        assert!(ids.contains(&memory2.id));
        assert!(ids.contains(&memory3.id));
    }

    #[test]
    fn test_get_batch() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let memory1 = create_test_memory("id1", "Content 1", Namespace::Decisions);
        let memory2 = create_test_memory("id2", "Content 2", Namespace::Learnings);
        let memory3 = create_test_memory("id3", "Content 3", Namespace::Patterns);

        backend.store(&memory1).unwrap();
        backend.store(&memory2).unwrap();
        backend.store(&memory3).unwrap();

        let ids = vec![memory1.id.clone(), memory2.id.clone()];
        let memories = backend.get_batch(&ids).unwrap();

        assert_eq!(memories.len(), 2);
        let retrieved_ids: Vec<_> = memories.iter().map(|m| &m.id).collect();
        assert!(retrieved_ids.contains(&&memory1.id));
        assert!(retrieved_ids.contains(&&memory2.id));
    }

    #[test]
    fn test_get_batch_empty() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let memories = backend.get_batch(&[]).unwrap();
        assert!(memories.is_empty());
    }

    #[test]
    fn test_exists() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let memory = create_test_memory("id1", "Test content", Namespace::Decisions);
        backend.store(&memory).unwrap();

        assert!(backend.exists(&memory.id).unwrap());
        assert!(!backend.exists(&MemoryId::new("nonexistent")).unwrap());
    }

    #[test]
    fn test_count() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        assert_eq!(backend.count().unwrap(), 0);

        let memory1 = create_test_memory("id1", "Content 1", Namespace::Decisions);
        let memory2 = create_test_memory("id2", "Content 2", Namespace::Learnings);

        backend.store(&memory1).unwrap();
        assert_eq!(backend.count().unwrap(), 1);

        backend.store(&memory2).unwrap();
        assert_eq!(backend.count().unwrap(), 2);

        backend.delete(&memory1.id).unwrap();
        assert_eq!(backend.count().unwrap(), 1);
    }

    #[test]
    fn test_store_update() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let mut memory = create_test_memory("id1", "Original content", Namespace::Decisions);
        backend.store(&memory).unwrap();

        // Update content
        memory.content = "Updated content".to_string();
        backend.store(&memory).unwrap();

        let retrieved = backend.get(&memory.id).unwrap().unwrap();
        assert_eq!(retrieved.content, "Updated content");
    }

    #[test]
    fn test_db_path() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();
        assert!(backend.db_path().is_none());

        // Would test with file path too, but that requires temp file management
    }

    #[test]
    fn test_store_and_retrieve_facet_fields() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let mut memory = create_test_memory("facet_test", "Test facets", Namespace::Decisions);
        memory.project_id = Some("github.com/org/repo".to_string());
        memory.branch = Some("main".to_string());
        memory.file_path = Some("src/lib.rs".to_string());

        backend.store(&memory).unwrap();

        let retrieved = backend.get(&memory.id).unwrap().unwrap();
        assert_eq!(
            retrieved.project_id,
            Some("github.com/org/repo".to_string())
        );
        assert_eq!(retrieved.branch, Some("main".to_string()));
        assert_eq!(retrieved.file_path, Some("src/lib.rs".to_string()));
    }

    #[test]
    fn test_store_and_retrieve_domain() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        // Test org-only domain
        let mut memory1 = create_test_memory("domain1", "Test org domain", Namespace::Decisions);
        memory1.domain = Domain {
            organization: Some("myorg".to_string()),
            ..Default::default()
        };
        backend.store(&memory1).unwrap();

        let retrieved1 = backend.get(&memory1.id).unwrap().unwrap();
        assert_eq!(retrieved1.domain.organization.as_deref(), Some("myorg"));
        assert_eq!(retrieved1.domain.repository.as_deref(), None);

        // Test org/repo domain
        let mut memory2 =
            create_test_memory("domain2", "Test org/repo domain", Namespace::Decisions);
        memory2.domain = Domain::for_repository("myorg", "myrepo");
        backend.store(&memory2).unwrap();

        let retrieved2 = backend.get(&memory2.id).unwrap().unwrap();
        assert_eq!(retrieved2.domain.organization.as_deref(), Some("myorg"));
        assert_eq!(retrieved2.domain.repository.as_deref(), Some("myrepo"));
    }

    #[test]
    fn test_store_and_retrieve_multiple_tags() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let mut memory = create_test_memory("tags_test", "Test tags", Namespace::Decisions);
        memory.tags = vec![
            "rust".to_string(),
            "architecture".to_string(),
            "performance".to_string(),
        ];

        backend.store(&memory).unwrap();

        let retrieved = backend.get(&memory.id).unwrap().unwrap();
        assert_eq!(retrieved.tags.len(), 3);
        assert!(retrieved.tags.contains(&"rust".to_string()));
        assert!(retrieved.tags.contains(&"architecture".to_string()));
        assert!(retrieved.tags.contains(&"performance".to_string()));
    }

    #[test]
    fn test_store_and_retrieve_different_statuses() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let statuses = [
            MemoryStatus::Active,
            MemoryStatus::Archived,
            MemoryStatus::Superseded,
            MemoryStatus::Pending,
            MemoryStatus::Deleted,
            MemoryStatus::Tombstoned,
        ];

        for (i, status) in statuses.iter().enumerate() {
            let mut memory =
                create_test_memory(&format!("status{i}"), "Test status", Namespace::Decisions);
            memory.status = *status;
            backend.store(&memory).unwrap();

            let retrieved = backend.get(&memory.id).unwrap().unwrap();
            assert_eq!(retrieved.status, *status);
        }
    }

    #[test]
    fn test_store_and_retrieve_tombstoned_memory() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let mut memory =
            create_test_memory("tombstone_test", "Test tombstone", Namespace::Decisions);
        memory.status = MemoryStatus::Tombstoned;
        memory.tombstoned_at = Utc.timestamp_opt(1_234_567_890, 0).single();

        backend.store(&memory).unwrap();

        let retrieved = backend.get(&memory.id).unwrap().unwrap();
        assert_eq!(retrieved.status, MemoryStatus::Tombstoned);
        assert!(retrieved.tombstoned_at.is_some());
        assert_eq!(retrieved.tombstoned_at.unwrap().timestamp(), 1_234_567_890);
    }

    #[test]
    fn test_store_and_retrieve_source() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let mut memory = create_test_memory("source_test", "Test source", Namespace::Decisions);
        memory.source = Some("src/main.rs:42".to_string());

        backend.store(&memory).unwrap();

        let retrieved = backend.get(&memory.id).unwrap().unwrap();
        assert_eq!(retrieved.source, Some("src/main.rs:42".to_string()));
    }

    #[test]
    fn test_store_content_with_special_characters() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let special_content = r#"Content with "quotes", 'apostrophes', and
newlines, plus special chars: \n\t\r and SQL chars: '; DROP TABLE memories; --"#;

        let memory = create_test_memory("special_chars", special_content, Namespace::Decisions);
        backend.store(&memory).unwrap();

        let retrieved = backend.get(&memory.id).unwrap().unwrap();
        assert_eq!(retrieved.content, special_content);
    }

    #[test]
    fn test_get_batch_with_partial_results() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let memory1 = create_test_memory("exists1", "Memory 1", Namespace::Decisions);
        let memory2 = create_test_memory("exists2", "Memory 2", Namespace::Learnings);

        backend.store(&memory1).unwrap();
        backend.store(&memory2).unwrap();

        // Request 4 IDs, but only 2 exist
        let ids = vec![
            memory1.id,
            MemoryId::new("nonexistent1"),
            memory2.id,
            MemoryId::new("nonexistent2"),
        ];

        let memories = backend.get_batch(&ids).unwrap();

        // Should only return the 2 that exist (PersistenceBackend::get_batch skips missing)
        assert_eq!(memories.len(), 2);
        let retrieved_ids: Vec<_> = memories.iter().map(|m| m.id.as_str()).collect();
        assert!(retrieved_ids.contains(&"exists1"));
        assert!(retrieved_ids.contains(&"exists2"));
    }

    #[test]
    fn test_store_empty_tags() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let mut memory = create_test_memory("empty_tags", "No tags", Namespace::Decisions);
        memory.tags = vec![];

        backend.store(&memory).unwrap();

        let retrieved = backend.get(&memory.id).unwrap().unwrap();
        assert!(retrieved.tags.is_empty());
    }

    #[test]
    fn test_store_tags_with_special_characters() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let mut memory =
            create_test_memory("special_tags", "Test special tags", Namespace::Decisions);
        memory.tags = vec![
            "100%".to_string(),
            "user_name".to_string(),
            "path\\file".to_string(),
        ];

        backend.store(&memory).unwrap();

        let retrieved = backend.get(&memory.id).unwrap().unwrap();
        assert_eq!(retrieved.tags.len(), 3);
        assert!(retrieved.tags.contains(&"100%".to_string()));
        assert!(retrieved.tags.contains(&"user_name".to_string()));
        assert!(retrieved.tags.contains(&"path\\file".to_string()));
    }

    #[test]
    fn test_store_all_namespaces() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        // Test all valid namespaces
        let namespaces = Namespace::user_namespaces();

        for (i, namespace) in namespaces.iter().enumerate() {
            let memory = create_test_memory(&format!("ns{i}"), "Test namespace", *namespace);
            backend.store(&memory).unwrap();

            let retrieved = backend.get(&memory.id).unwrap().unwrap();
            assert_eq!(retrieved.namespace, *namespace);
        }
    }

    #[test]
    fn test_update_all_fields() {
        let backend = SqlitePersistenceBackend::in_memory().unwrap();

        let mut memory = create_test_memory("update_all", "Original", Namespace::Decisions);
        backend.store(&memory).unwrap();

        // Update all mutable fields
        memory.content = "Updated content".to_string();
        memory.namespace = Namespace::Learnings;
        memory.domain = Domain::for_repository("neworg", "newrepo");
        memory.project_id = Some("new_project".to_string());
        memory.branch = Some("develop".to_string());
        memory.file_path = Some("src/new.rs".to_string());
        memory.status = MemoryStatus::Archived;
        memory.tags = vec!["updated".to_string(), "modified".to_string()];
        memory.source = Some("updated_source".to_string());

        backend.store(&memory).unwrap();

        let retrieved = backend.get(&memory.id).unwrap().unwrap();
        assert_eq!(retrieved.content, "Updated content");
        assert_eq!(retrieved.namespace, Namespace::Learnings);
        assert_eq!(retrieved.domain.organization.as_deref(), Some("neworg"));
        assert_eq!(retrieved.domain.repository.as_deref(), Some("newrepo"));
        assert_eq!(retrieved.project_id, Some("new_project".to_string()));
        assert_eq!(retrieved.branch, Some("develop".to_string()));
        assert_eq!(retrieved.file_path, Some("src/new.rs".to_string()));
        assert_eq!(retrieved.status, MemoryStatus::Archived);
        assert_eq!(retrieved.tags.len(), 2);
        assert_eq!(retrieved.source, Some("updated_source".to_string()));
    }
}
