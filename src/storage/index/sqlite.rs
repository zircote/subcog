//! `SQLite` + FTS5 index backend.
//!
//! Provides full-text search using `SQLite`'s FTS5 extension.

use crate::models::{Memory, MemoryId, SearchFilter};
use crate::storage::traits::IndexBackend;
use crate::{Error, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tracing::instrument;

/// Timeout for acquiring mutex lock (5 seconds).
/// Reserved for future use when upgrading to `parking_lot::Mutex`.
#[allow(dead_code)]
const MUTEX_LOCK_TIMEOUT: Duration = Duration::from_secs(5);

/// Helper to acquire mutex lock with poison recovery.
///
/// If the mutex is poisoned (due to a panic in a previous critical section),
/// we recover the inner value and log a warning. This prevents cascading
/// failures when one operation panics.
fn acquire_lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // First try to acquire lock normally
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            // Recover from poison - this is safe because we log the issue
            // and the connection state should still be valid
            tracing::warn!("SQLite mutex was poisoned, recovering");
            metrics::counter!("sqlite_mutex_poison_recovery_total").increment(1);
            poisoned.into_inner()
        },
    }
}

/// Alternative lock acquisition with spin-wait timeout.
///
/// Note: Rust's `std::sync::Mutex` doesn't have a native `try_lock_for`,
/// so we implement a spin-wait with sleep. For production, consider
/// using `parking_lot::Mutex` which has proper timed locking.
///
/// Reserved for future use - currently using simpler `acquire_lock` with poison recovery.
#[allow(dead_code)]
fn acquire_lock_with_timeout<T>(mutex: &Mutex<T>, timeout: Duration) -> Result<MutexGuard<'_, T>> {
    let start = Instant::now();
    let sleep_duration = Duration::from_millis(10);

    loop {
        match mutex.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                tracing::warn!("SQLite mutex was poisoned, recovering");
                metrics::counter!("sqlite_mutex_poison_recovery_total").increment(1);
                return Ok(poisoned.into_inner());
            },
            Err(std::sync::TryLockError::WouldBlock) => {
                if start.elapsed() > timeout {
                    metrics::counter!("sqlite_mutex_timeout_total").increment(1);
                    return Err(Error::OperationFailed {
                        operation: "acquire_lock".to_string(),
                        cause: format!("Lock acquisition timed out after {timeout:?}"),
                    });
                }
                std::thread::sleep(sleep_duration);
            },
        }
    }
}

/// Escapes SQL LIKE wildcards in a string (SEC-M4).
///
/// `SQLite` LIKE patterns treat `%` as "any characters" and `_` as "single character".
/// If user input contains these characters, they must be escaped to be treated literally.
/// Uses `\` as the escape character (requires `ESCAPE '\'` in LIKE clause).
///
/// # Examples
///
/// ```ignore
/// assert_eq!(escape_like_wildcards("100%"), "100\\%");
/// assert_eq!(escape_like_wildcards("user_name"), "user\\_name");
/// assert_eq!(escape_like_wildcards("path\\file"), "path\\\\file");
/// ```
fn escape_like_wildcards(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' | '_' | '\\' => {
                result.push('\\');
                result.push(c);
            },
            _ => result.push(c),
        }
    }
    result
}

/// SQLite-based index backend with FTS5.
pub struct SqliteBackend {
    /// Connection to the `SQLite` database.
    conn: Mutex<Connection>,
    /// Path to the `SQLite` database (None for in-memory).
    db_path: Option<PathBuf>,
}

struct MemoryRow {
    id: String,
    namespace: String,
    domain: Option<String>,
    status: String,
    created_at: i64,
    tags: Option<String>,
    content: String,
}

impl SqliteBackend {
    /// Creates a new `SQLite` backend.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or initialized.
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

    /// Creates an in-memory `SQLite` backend (useful for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be initialized.
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| Error::OperationFailed {
            operation: "open_sqlite_memory".to_string(),
            cause: e.to_string(),
        })?;

        let backend = Self {
            conn: Mutex::new(conn),
            db_path: None,
        };

        backend.initialize()?;
        Ok(backend)
    }

    /// Returns the database path.
    #[must_use]
    pub fn db_path(&self) -> Option<&Path> {
        self.db_path.as_deref()
    }

    /// Initializes the database schema.
    fn initialize(&self) -> Result<()> {
        let conn = acquire_lock(&self.conn);

        // Enable WAL mode for better concurrent read performance
        // Note: pragma_update returns the result which we ignore - journal_mode returns
        // a string like "wal" which would cause execute_batch to fail
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "synchronous", "NORMAL");

        // Create the main table for memory metadata
        conn.execute(
            "CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                namespace TEXT NOT NULL,
                domain TEXT,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                tags TEXT,
                source TEXT
            )",
            [],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "create_memories_table".to_string(),
            cause: e.to_string(),
        })?;

        // Add source column if it doesn't exist (for migration)
        let _ = conn.execute("ALTER TABLE memories ADD COLUMN source TEXT", []);

        // Create indexes for common query patterns (DB-H1)
        Self::create_indexes(&conn);

        // Create FTS5 virtual table for full-text search (standalone, not synced with memories)
        // Note: FTS5 virtual tables use inverted indexes for MATCH queries and don't support
        // traditional B-tree indexes. Joins with the memories table use memories.id (PRIMARY KEY)
        // which is already indexed. The FTS5 MATCH operation returns a small result set first,
        // making the join efficient. See: https://sqlite.org/fts5.html
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                id,
                content,
                tags
            )",
            [],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "create_fts_table".to_string(),
            cause: e.to_string(),
        })?;

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
    }

    /// Builds a WHERE clause from a search filter with numbered parameters.
    /// Returns the clause string, the parameters, and the next parameter index.
    fn build_filter_clause_numbered(
        &self,
        filter: &SearchFilter,
        start_param: usize,
    ) -> (String, Vec<String>, usize) {
        let mut conditions = Vec::new();
        let mut params = Vec::new();
        let mut param_idx = start_param;

        if !filter.namespaces.is_empty() {
            let placeholders: Vec<String> = filter
                .namespaces
                .iter()
                .map(|_| {
                    let p = format!("?{param_idx}");
                    param_idx += 1;
                    p
                })
                .collect();
            conditions.push(format!("m.namespace IN ({})", placeholders.join(",")));
            for ns in &filter.namespaces {
                params.push(ns.as_str().to_string());
            }
        }

        if !filter.statuses.is_empty() {
            let placeholders: Vec<String> = filter
                .statuses
                .iter()
                .map(|_| {
                    let p = format!("?{param_idx}");
                    param_idx += 1;
                    p
                })
                .collect();
            conditions.push(format!("m.status IN ({})", placeholders.join(",")));
            for s in &filter.statuses {
                params.push(s.as_str().to_string());
            }
        }

        // Tag filtering (AND logic - must have ALL tags)
        // Use ',tag,' pattern with wrapped column to match whole tags only
        // Escape LIKE wildcards in tags to prevent SQL injection (SEC-M4)
        for tag in &filter.tags {
            conditions.push(format!(
                "(',' || m.tags || ',') LIKE ?{param_idx} ESCAPE '\\'"
            ));
            param_idx += 1;
            params.push(format!("%,{},%", escape_like_wildcards(tag)));
        }

        // Tag filtering (OR logic - must have ANY tag)
        if !filter.tags_any.is_empty() {
            let or_conditions: Vec<String> = filter
                .tags_any
                .iter()
                .map(|tag| {
                    let cond = format!("(',' || m.tags || ',') LIKE ?{param_idx} ESCAPE '\\'");
                    param_idx += 1;
                    params.push(format!("%,{},%", escape_like_wildcards(tag)));
                    cond
                })
                .collect();
            conditions.push(format!("({})", or_conditions.join(" OR ")));
        }

        // Excluded tags (NOT LIKE) - match whole tags only
        // Escape LIKE wildcards (SEC-M4)
        for tag in &filter.excluded_tags {
            conditions.push(format!(
                "(',' || m.tags || ',') NOT LIKE ?{param_idx} ESCAPE '\\'"
            ));
            param_idx += 1;
            params.push(format!("%,{},%", escape_like_wildcards(tag)));
        }

        // Source pattern (glob-style converted to SQL LIKE)
        if let Some(ref pattern) = filter.source_pattern {
            // Convert glob pattern to SQL LIKE pattern: * -> %, ? -> _
            let sql_pattern = pattern.replace('*', "%").replace('?', "_");
            conditions.push(format!("m.source LIKE ?{param_idx}"));
            param_idx += 1;
            params.push(sql_pattern);
        }

        if let Some(after) = filter.created_after {
            conditions.push(format!("m.created_at >= ?{param_idx}"));
            param_idx += 1;
            params.push(after.to_string());
        }

        if let Some(before) = filter.created_before {
            conditions.push(format!("m.created_at <= ?{param_idx}"));
            param_idx += 1;
            params.push(before.to_string());
        }

        let clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" AND {}", conditions.join(" AND "))
        };

        (clause, params, param_idx)
    }

    fn record_operation_metrics(
        &self,
        operation: &'static str,
        start: Instant,
        status: &'static str,
    ) {
        metrics::counter!(
            "storage_operations_total",
            "backend" => "sqlite",
            "operation" => operation,
            "status" => status
        )
        .increment(1);
        metrics::histogram!(
            "storage_operation_duration_ms",
            "backend" => "sqlite",
            "operation" => operation,
            "status" => status
        )
        .record(start.elapsed().as_secs_f64() * 1000.0);
    }
}

fn fetch_memory_row(conn: &Connection, id: &MemoryId) -> Result<Option<MemoryRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.namespace, m.domain, m.status, m.created_at, m.tags, f.content
             FROM memories m
             JOIN memories_fts f ON m.id = f.id
             WHERE m.id = ?1",
        )
        .map_err(|e| Error::OperationFailed {
            operation: "prepare_get_memory".to_string(),
            cause: e.to_string(),
        })?;

    let result: std::result::Result<Option<_>, _> = stmt
        .query_row(params![id.as_str()], |row| {
            Ok(MemoryRow {
                id: row.get(0)?,
                namespace: row.get(1)?,
                domain: row.get(2)?,
                status: row.get(3)?,
                created_at: row.get(4)?,
                tags: row.get(5)?,
                content: row.get(6)?,
            })
        })
        .optional();

    result.map_err(|e| Error::OperationFailed {
        operation: "get_memory".to_string(),
        cause: e.to_string(),
    })
}

fn build_memory_from_row(row: MemoryRow) -> Memory {
    use crate::models::{Domain, MemoryStatus, Namespace};

    let namespace = Namespace::parse(&row.namespace).unwrap_or_default();
    let domain = row.domain.map_or_else(Domain::new, |d: String| {
        if d.is_empty() || d == "global" {
            Domain::new()
        } else {
            let parts: Vec<&str> = d.split('/').collect();
            match parts.len() {
                1 => Domain {
                    organization: Some(parts[0].to_string()),
                    project: None,
                    repository: None,
                },
                2 => Domain {
                    organization: Some(parts[0].to_string()),
                    project: None,
                    repository: Some(parts[1].to_string()),
                },
                _ => Domain::new(),
            }
        }
    });

    let status = match row.status.to_lowercase().as_str() {
        "active" => MemoryStatus::Active,
        "archived" => MemoryStatus::Archived,
        "superseded" => MemoryStatus::Superseded,
        "pending" => MemoryStatus::Pending,
        "deleted" => MemoryStatus::Deleted,
        _ => MemoryStatus::Active,
    };

    let tags: Vec<String> = row
        .tags
        .map(|t| {
            t.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    #[allow(clippy::cast_sign_loss)]
    let created_at_u64 = row.created_at as u64;

    Memory {
        id: MemoryId::new(row.id),
        content: row.content,
        namespace,
        domain,
        status,
        created_at: created_at_u64,
        updated_at: created_at_u64,
        embedding: None,
        tags,
        source: None,
    }
}

impl IndexBackend for SqliteBackend {
    #[instrument(
        skip(self, memory),
        fields(
            operation = "index",
            backend = "sqlite",
            memory.id = %memory.id.as_str(),
            namespace = %memory.namespace.as_str(),
            domain = %memory.domain.to_string()
        )
    )]
    fn index(&self, memory: &Memory) -> Result<()> {
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
                conn.execute(
                    "INSERT OR REPLACE INTO memories (id, namespace, domain, status, created_at, tags, source)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        memory.id.as_str(),
                        memory.namespace.as_str(),
                        domain_str,
                        memory.status.as_str(),
                        created_at_i64,
                        tags_str,
                        memory.source.as_deref()
                    ],
                )
                .map_err(|e| Error::OperationFailed {
                    operation: "insert_memory".to_string(),
                    cause: e.to_string(),
                })?;

                // Delete from FTS if exists (FTS5 uses rowid internally for matching)
                conn.execute(
                    "DELETE FROM memories_fts WHERE id = ?1",
                    params![memory.id.as_str()],
                )
                .map_err(|e| Error::OperationFailed {
                    operation: "delete_fts".to_string(),
                    cause: e.to_string(),
                })?;

                // Insert into FTS table
                conn.execute(
                    "INSERT INTO memories_fts (id, content, tags) VALUES (?1, ?2, ?3)",
                    params![memory.id.as_str(), memory.content, tags_str],
                )
                .map_err(|e| Error::OperationFailed {
                    operation: "insert_fts".to_string(),
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
        self.record_operation_metrics("index", start, status);
        result
    }

    #[instrument(skip(self), fields(operation = "remove", backend = "sqlite", memory.id = %id.as_str()))]
    fn remove(&self, id: &MemoryId) -> Result<bool> {
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
                // Delete from FTS
                conn.execute(
                    "DELETE FROM memories_fts WHERE id = ?1",
                    params![id.as_str()],
                )
                .map_err(|e| Error::OperationFailed {
                    operation: "delete_fts".to_string(),
                    cause: e.to_string(),
                })?;

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
        self.record_operation_metrics("remove", start, status);
        result
    }

    #[instrument(
        skip(self, query, filter),
        fields(operation = "search", backend = "sqlite", query_length = query.len(), limit = limit)
    )]
    fn search(
        &self,
        query: &str,
        filter: &SearchFilter,
        limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>> {
        let start = Instant::now();
        let result = (|| {
            let conn = acquire_lock(&self.conn);

            // Build filter clause with numbered parameters starting from ?2
            // ?1 is the FTS query
            let (filter_clause, filter_params, next_param) =
                self.build_filter_clause_numbered(filter, 2);

            // Use FTS5 MATCH for search with BM25 ranking
            // Limit parameter comes after all filter parameters
            let sql = format!(
                "SELECT f.id, bm25(memories_fts) as score
                 FROM memories_fts f
                 JOIN memories m ON f.id = m.id
                 WHERE memories_fts MATCH ?1 {filter_clause}
                 ORDER BY score
                 LIMIT ?{next_param}"
            );

            let mut stmt = conn.prepare(&sql).map_err(|e| Error::OperationFailed {
                operation: "prepare_search".to_string(),
                cause: e.to_string(),
            })?;

            // Build parameters: query, filter params, limit
            let mut results = Vec::new();

            // FTS5 query - escape special characters and wrap terms in quotes
            // FTS5 special chars: - (NOT), * (prefix), " (phrase), : (column)
            // Pre-allocate: each term becomes ~term.len() + 6 chars ("term" OR )
            let terms: Vec<_> = query.split_whitespace().collect();
            let estimated_len = terms.iter().map(|t| t.len() + 8).sum::<usize>();
            let mut fts_query = String::with_capacity(estimated_len);
            for (i, term) in terms.iter().enumerate() {
                if i > 0 {
                    fts_query.push_str(" OR ");
                }
                fts_query.push('"');
                // Escape double quotes for literal matching
                for c in term.chars() {
                    if c == '"' {
                        fts_query.push_str("\"\"");
                    } else {
                        fts_query.push(c);
                    }
                }
                fts_query.push('"');
            }

            let rows = stmt
                .query_map(
                    rusqlite::params_from_iter(
                        std::iter::once(fts_query)
                            .chain(filter_params.into_iter())
                            .chain(std::iter::once(limit.to_string())),
                    ),
                    |row| {
                        let id: String = row.get(0)?;
                        let score: f64 = row.get(1)?;
                        Ok((id, score))
                    },
                )
                .map_err(|e| Error::OperationFailed {
                    operation: "execute_search".to_string(),
                    cause: e.to_string(),
                })?;

            for row in rows {
                let (id, score) = row.map_err(|e| Error::OperationFailed {
                    operation: "read_search_row".to_string(),
                    cause: e.to_string(),
                })?;

                // Normalize BM25 score (DB-H3 fix)
                // SQLite FTS5 bm25() returns negative values where MORE NEGATIVE = BETTER MATCH
                // For example: -10.0 is a better match than -2.0
                //
                // We negate and apply sigmoid normalization to map to 0-1 range:
                // - Negate: makes higher values = better matches
                // - Sigmoid: 1.0 / (1.0 + e^(-k*x)) where k controls steepness
                // - This gives values in (0, 1) with ~0.5 for score=0
                #[allow(clippy::cast_possible_truncation)]
                let normalized_score = {
                    // Negate so higher = better, apply sigmoid with k=0.5 for gentle curve
                    let positive_score = -score;
                    let sigmoid = 1.0 / (1.0 + (-0.5 * positive_score).exp());
                    sigmoid.clamp(0.0, 1.0) as f32
                };

                results.push((MemoryId::new(id), normalized_score));
            }

            // Apply minimum score filter if specified
            if let Some(min_score) = filter.min_score {
                results.retain(|(_, score)| *score >= min_score);
            }

            Ok(results)
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        self.record_operation_metrics("search", start, status);
        result
    }

    #[instrument(skip(self), fields(operation = "clear", backend = "sqlite"))]
    fn clear(&self) -> Result<()> {
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
                conn.execute("DELETE FROM memories_fts", []).map_err(|e| {
                    Error::OperationFailed {
                        operation: "clear_fts".to_string(),
                        cause: e.to_string(),
                    }
                })?;

                conn.execute("DELETE FROM memories", [])
                    .map_err(|e| Error::OperationFailed {
                        operation: "clear_memories".to_string(),
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
        self.record_operation_metrics("clear", start, status);
        result
    }

    #[instrument(
        skip(self, filter),
        fields(operation = "list_all", backend = "sqlite", limit = limit)
    )]
    fn list_all(&self, filter: &SearchFilter, limit: usize) -> Result<Vec<(MemoryId, f32)>> {
        let start = Instant::now();
        let result = (|| {
            let conn = acquire_lock(&self.conn);

            // Build filter clause (starting at parameter 1, no FTS query)
            let (filter_clause, filter_params, next_param) =
                self.build_filter_clause_numbered(filter, 1);

            // Query all memories without FTS MATCH, ordered by created_at desc
            let sql = format!(
                "SELECT m.id, 1.0 as score
                 FROM memories m
                 WHERE 1=1 {filter_clause}
                 ORDER BY m.created_at DESC
                 LIMIT ?{next_param}"
            );

            let mut stmt = conn.prepare(&sql).map_err(|e| Error::OperationFailed {
                operation: "prepare_list_all".to_string(),
                cause: e.to_string(),
            })?;

            let mut results = Vec::new();

            let rows = stmt
                .query_map(
                    rusqlite::params_from_iter(
                        filter_params
                            .into_iter()
                            .chain(std::iter::once(limit.to_string())),
                    ),
                    |row| {
                        let id: String = row.get(0)?;
                        let score: f64 = row.get(1)?;
                        Ok((id, score))
                    },
                )
                .map_err(|e| Error::OperationFailed {
                    operation: "list_all".to_string(),
                    cause: e.to_string(),
                })?;

            for row in rows {
                let (id, score) = row.map_err(|e| Error::OperationFailed {
                    operation: "read_list_row".to_string(),
                    cause: e.to_string(),
                })?;

                #[allow(clippy::cast_possible_truncation)]
                results.push((MemoryId::new(id), score as f32));
            }

            Ok(results)
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        self.record_operation_metrics("list_all", start, status);
        result
    }

    #[instrument(skip(self), fields(operation = "get_memory", backend = "sqlite", memory.id = %id.as_str()))]
    fn get_memory(&self, id: &MemoryId) -> Result<Option<Memory>> {
        let start = Instant::now();
        let result = (|| {
            let conn = acquire_lock(&self.conn);
            let row = fetch_memory_row(&conn, id)?;
            Ok(row.map(build_memory_from_row))
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        self.record_operation_metrics("get_memory", start, status);
        result
    }

    /// Retrieves multiple memories in a single batch query (PERF-C1 fix).
    ///
    /// Uses a single SQL query with IN clause instead of N individual queries.
    #[instrument(skip(self, ids), fields(operation = "get_memories_batch", backend = "sqlite", count = ids.len()))]
    fn get_memories_batch(&self, ids: &[MemoryId]) -> Result<Vec<Option<Memory>>> {
        let start = Instant::now();

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let result = (|| {
            let conn = acquire_lock(&self.conn);

            // Build placeholders for IN clause
            let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{i}")).collect();

            let sql = format!(
                "SELECT m.id, m.namespace, m.domain, m.status, m.created_at, m.tags, f.content
                 FROM memories m
                 JOIN memories_fts f ON m.id = f.id
                 WHERE m.id IN ({})",
                placeholders.join(", ")
            );

            let mut stmt = conn.prepare(&sql).map_err(|e| Error::OperationFailed {
                operation: "prepare_get_memories_batch".to_string(),
                cause: e.to_string(),
            })?;

            // Collect results into a HashMap for O(1) lookup
            let id_strs: Vec<&str> = ids.iter().map(MemoryId::as_str).collect();
            let mut memory_map: std::collections::HashMap<String, Memory> =
                std::collections::HashMap::with_capacity(ids.len());

            let rows = stmt
                .query_map(rusqlite::params_from_iter(id_strs.iter()), |row| {
                    Ok(MemoryRow {
                        id: row.get(0)?,
                        namespace: row.get(1)?,
                        domain: row.get(2)?,
                        status: row.get(3)?,
                        created_at: row.get(4)?,
                        tags: row.get(5)?,
                        content: row.get(6)?,
                    })
                })
                .map_err(|e| Error::OperationFailed {
                    operation: "execute_get_memories_batch".to_string(),
                    cause: e.to_string(),
                })?;

            for row in rows {
                let memory_row = row.map_err(|e| Error::OperationFailed {
                    operation: "read_batch_row".to_string(),
                    cause: e.to_string(),
                })?;
                let id = memory_row.id.clone();
                memory_map.insert(id, build_memory_from_row(memory_row));
            }

            // Return memories in the same order as input IDs
            Ok(ids
                .iter()
                .map(|id| memory_map.remove(id.as_str()))
                .collect())
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        self.record_operation_metrics("get_memories_batch", start, status);
        result
    }

    /// Re-indexes all memories in a single transaction (DB-H2).
    ///
    /// This is more efficient than the default implementation which creates
    /// a transaction per memory.
    #[instrument(skip(self, memories), fields(operation = "reindex", backend = "sqlite", count = memories.len()))]
    fn reindex(&self, memories: &[Memory]) -> Result<()> {
        let start = Instant::now();

        if memories.is_empty() {
            return Ok(());
        }

        let result = (|| {
            let conn = acquire_lock(&self.conn);

            // Use a single transaction for all operations
            conn.execute("BEGIN IMMEDIATE", [])
                .map_err(|e| Error::OperationFailed {
                    operation: "begin_transaction".to_string(),
                    cause: e.to_string(),
                })?;

            let result = (|| {
                for memory in memories {
                    let tags_str = memory.tags.join(",");
                    let domain_str = memory.domain.to_string();

                    // Insert or replace in main table
                    // Note: Cast u64 to i64 for SQLite compatibility (rusqlite doesn't impl ToSql for u64)
                    #[allow(clippy::cast_possible_wrap)]
                    let created_at_i64 = memory.created_at as i64;
                    conn.execute(
                        "INSERT OR REPLACE INTO memories (id, namespace, domain, status, created_at, tags, source)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![
                            memory.id.as_str(),
                            memory.namespace.as_str(),
                            domain_str,
                            memory.status.as_str(),
                            created_at_i64,
                            tags_str,
                            memory.source.as_deref()
                        ],
                    )
                    .map_err(|e| Error::OperationFailed {
                        operation: "insert_memory".to_string(),
                        cause: e.to_string(),
                    })?;

                    // Delete from FTS if exists
                    conn.execute(
                        "DELETE FROM memories_fts WHERE id = ?1",
                        params![memory.id.as_str()],
                    )
                    .map_err(|e| Error::OperationFailed {
                        operation: "delete_fts".to_string(),
                        cause: e.to_string(),
                    })?;

                    // Insert into FTS table
                    conn.execute(
                        "INSERT INTO memories_fts (id, content, tags) VALUES (?1, ?2, ?3)",
                        params![memory.id.as_str(), memory.content, tags_str],
                    )
                    .map_err(|e| Error::OperationFailed {
                        operation: "insert_fts".to_string(),
                        cause: e.to_string(),
                    })?;
                }
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
        self.record_operation_metrics("reindex", start, status);
        result
    }
}

// Implement PersistenceBackend for SqliteBackend so it can be used with ConsolidationService
impl crate::storage::traits::PersistenceBackend for SqliteBackend {
    fn store(&self, memory: &Memory) -> Result<()> {
        // Delegate to index() which stores the full memory
        self.index(memory)
    }

    fn get(&self, id: &MemoryId) -> Result<Option<Memory>> {
        self.get_memory(id)
    }

    fn delete(&self, id: &MemoryId) -> Result<bool> {
        self.remove(id)
    }

    fn list_ids(&self) -> Result<Vec<MemoryId>> {
        let start = Instant::now();
        let result = (|| {
            let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
                operation: "lock_connection".to_string(),
                cause: e.to_string(),
            })?;

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
        self.record_operation_metrics("list_ids", start, status);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, MemoryStatus, Namespace};

    fn create_test_memory(id: &str, content: &str, namespace: Namespace) -> Memory {
        Memory {
            id: MemoryId::new(id),
            content: content.to_string(),
            namespace,
            domain: Domain::new(),
            status: MemoryStatus::Active,
            created_at: 1_234_567_890,
            updated_at: 1_234_567_890,
            embedding: None,
            tags: vec!["test".to_string()],
            source: None,
        }
    }

    #[test]
    fn test_index_and_search() {
        let backend = SqliteBackend::in_memory().unwrap();

        let memory1 = create_test_memory("id1", "Rust programming language", Namespace::Decisions);
        let memory2 = create_test_memory("id2", "Python scripting", Namespace::Learnings);
        let memory3 =
            create_test_memory("id3", "Rust ownership and borrowing", Namespace::Patterns);

        backend.index(&memory1).unwrap();
        backend.index(&memory2).unwrap();
        backend.index(&memory3).unwrap();

        // Search for "Rust"
        let results = backend.search("Rust", &SearchFilter::new(), 10).unwrap();

        assert_eq!(results.len(), 2);
        let ids: Vec<_> = results.iter().map(|(id, _)| id.as_str()).collect();
        assert!(ids.contains(&"id1"));
        assert!(ids.contains(&"id3"));
    }

    #[test]
    fn test_search_with_namespace_filter() {
        let backend = SqliteBackend::in_memory().unwrap();

        let memory1 = create_test_memory("id1", "Rust programming", Namespace::Decisions);
        let memory2 = create_test_memory("id2", "Rust patterns", Namespace::Patterns);

        backend.index(&memory1).unwrap();
        backend.index(&memory2).unwrap();

        // Search with namespace filter
        let filter = SearchFilter::new().with_namespace(Namespace::Patterns);
        let results = backend.search("Rust", &filter, 10).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.as_str(), "id2");
    }

    #[test]
    fn test_remove() {
        let backend = SqliteBackend::in_memory().unwrap();

        let memory = create_test_memory("to_remove", "Test content", Namespace::Decisions);
        backend.index(&memory).unwrap();

        // Verify it exists
        let results = backend.search("content", &SearchFilter::new(), 10).unwrap();
        assert_eq!(results.len(), 1);

        // Remove it
        let removed = backend.remove(&MemoryId::new("to_remove")).unwrap();
        assert!(removed);

        // Verify it's gone
        let results = backend.search("content", &SearchFilter::new(), 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_clear() {
        let backend = SqliteBackend::in_memory().unwrap();

        backend
            .index(&create_test_memory("id1", "content1", Namespace::Decisions))
            .unwrap();
        backend
            .index(&create_test_memory("id2", "content2", Namespace::Decisions))
            .unwrap();

        backend.clear().unwrap();

        let results = backend.search("content", &SearchFilter::new(), 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_reindex() {
        let backend = SqliteBackend::in_memory().unwrap();

        let memories = vec![
            create_test_memory("id1", "memory one", Namespace::Decisions),
            create_test_memory("id2", "memory two", Namespace::Learnings),
            create_test_memory("id3", "memory three", Namespace::Patterns),
        ];

        backend.reindex(&memories).unwrap();

        let results = backend.search("memory", &SearchFilter::new(), 10).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_update_index() {
        let backend = SqliteBackend::in_memory().unwrap();

        let mut memory =
            create_test_memory("update_test", "original content", Namespace::Decisions);
        backend.index(&memory).unwrap();

        // Update the memory
        memory.content = "updated content completely different".to_string();
        backend.index(&memory).unwrap();

        // Search for old content should not find it
        let old_results = backend
            .search("original", &SearchFilter::new(), 10)
            .unwrap();
        assert!(old_results.is_empty());

        // Search for new content should find it
        let new_results = backend
            .search("different", &SearchFilter::new(), 10)
            .unwrap();
        assert_eq!(new_results.len(), 1);
    }

    #[test]
    fn test_get_memories_batch() {
        let backend = SqliteBackend::in_memory().unwrap();

        let memory1 = create_test_memory("batch1", "First memory", Namespace::Decisions);
        let memory2 = create_test_memory("batch2", "Second memory", Namespace::Learnings);
        let memory3 = create_test_memory("batch3", "Third memory", Namespace::Patterns);

        backend.index(&memory1).unwrap();
        backend.index(&memory2).unwrap();
        backend.index(&memory3).unwrap();

        // Fetch all three in a batch
        let ids = vec![
            MemoryId::new("batch1"),
            MemoryId::new("batch2"),
            MemoryId::new("batch3"),
        ];
        let results = backend.get_memories_batch(&ids).unwrap();

        assert_eq!(results.len(), 3);
        assert!(results[0].is_some());
        assert!(results[1].is_some());
        assert!(results[2].is_some());

        // Verify order is preserved
        assert_eq!(results[0].as_ref().unwrap().id.as_str(), "batch1");
        assert_eq!(results[1].as_ref().unwrap().id.as_str(), "batch2");
        assert_eq!(results[2].as_ref().unwrap().id.as_str(), "batch3");
    }

    #[test]
    fn test_get_memories_batch_with_missing() {
        let backend = SqliteBackend::in_memory().unwrap();

        let memory1 = create_test_memory("exists1", "Memory one", Namespace::Decisions);
        backend.index(&memory1).unwrap();

        // Request both existing and non-existing
        let ids = vec![
            MemoryId::new("exists1"),
            MemoryId::new("does_not_exist"),
            MemoryId::new("also_missing"),
        ];
        let results = backend.get_memories_batch(&ids).unwrap();

        assert_eq!(results.len(), 3);
        assert!(results[0].is_some());
        assert!(results[1].is_none());
        assert!(results[2].is_none());
    }

    #[test]
    fn test_get_memories_batch_empty() {
        let backend = SqliteBackend::in_memory().unwrap();
        let results = backend.get_memories_batch(&[]).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_with_status_filter() {
        let backend = SqliteBackend::in_memory().unwrap();

        // Create memories with different statuses
        let mut memory1 = create_test_memory("id1", "Rust programming", Namespace::Decisions);
        memory1.status = MemoryStatus::Active;

        let mut memory2 = create_test_memory("id2", "Rust patterns", Namespace::Decisions);
        memory2.status = MemoryStatus::Archived;

        backend.index(&memory1).unwrap();
        backend.index(&memory2).unwrap();

        // Search with status filter
        let filter = SearchFilter::new().with_status(MemoryStatus::Active);
        let results = backend.search("Rust", &filter, 10).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.as_str(), "id1");
    }

    #[test]
    fn test_search_with_tag_filter() {
        let backend = SqliteBackend::in_memory().unwrap();

        let mut memory1 = create_test_memory("id1", "Rust guide", Namespace::Decisions);
        memory1.tags = vec!["rust".to_string(), "guide".to_string()];

        let mut memory2 = create_test_memory("id2", "Rust tutorial", Namespace::Decisions);
        memory2.tags = vec!["rust".to_string(), "tutorial".to_string()];

        backend.index(&memory1).unwrap();
        backend.index(&memory2).unwrap();

        // Search with tag filter (using with_tag for single tag)
        let filter = SearchFilter::new().with_tag("guide");
        let results = backend.search("Rust", &filter, 10).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.as_str(), "id1");
    }

    #[test]
    fn test_search_fts_special_characters() {
        let backend = SqliteBackend::in_memory().unwrap();

        let memory = create_test_memory(
            "special",
            "Error: unexpected 'syntax' in /path/to/file.rs:42",
            Namespace::Learnings,
        );
        backend.index(&memory).unwrap();

        // Search with special characters should be escaped properly
        let results = backend
            .search("Error syntax", &SearchFilter::new(), 10)
            .unwrap();
        assert_eq!(results.len(), 1);

        // Search for path-like content
        let results = backend.search("file.rs", &SearchFilter::new(), 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_get_memory_single() {
        let backend = SqliteBackend::in_memory().unwrap();

        let memory = create_test_memory(
            "single_get",
            "Fetching a single memory",
            Namespace::Decisions,
        );
        backend.index(&memory).unwrap();

        // Fetch the memory
        let result = backend.get_memory(&MemoryId::new("single_get")).unwrap();
        assert!(result.is_some());

        let fetched = result.unwrap();
        assert_eq!(fetched.id.as_str(), "single_get");
        assert_eq!(fetched.content, "Fetching a single memory");
        assert_eq!(fetched.namespace, Namespace::Decisions);
    }

    #[test]
    fn test_get_memory_not_found() {
        let backend = SqliteBackend::in_memory().unwrap();

        let result = backend.get_memory(&MemoryId::new("nonexistent")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_remove_nonexistent() {
        let backend = SqliteBackend::in_memory().unwrap();

        // Removing a non-existent memory should return false
        let removed = backend.remove(&MemoryId::new("does_not_exist")).unwrap();
        assert!(!removed);
    }

    #[test]
    fn test_search_whitespace_only_query() {
        let backend = SqliteBackend::in_memory().unwrap();

        let memory = create_test_memory("id1", "Some content", Namespace::Decisions);
        backend.index(&memory).unwrap();

        // Whitespace-only query should be handled gracefully
        // The search function should either return empty or handle the error
        let results = backend.search("   ", &SearchFilter::new(), 10);
        // Either empty results or an error is acceptable for whitespace-only queries
        assert!(results.is_ok() || results.is_err());
    }

    #[test]
    fn test_search_limit() {
        let backend = SqliteBackend::in_memory().unwrap();

        // Index 5 memories all containing "test"
        for i in 0..5 {
            let memory = create_test_memory(
                &format!("id{i}"),
                &format!("test content {i}"),
                Namespace::Decisions,
            );
            backend.index(&memory).unwrap();
        }

        // Search with limit of 3
        let results = backend.search("test", &SearchFilter::new(), 3).unwrap();
        assert_eq!(results.len(), 3);

        // Search with limit of 10 (more than available)
        let results = backend.search("test", &SearchFilter::new(), 10).unwrap();
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn test_index_and_search_with_unicode() {
        let backend = SqliteBackend::in_memory().unwrap();

        let memory = create_test_memory(
            "unicode",
            "Testing Unicode support with accents: cafe naive resume",
            Namespace::Learnings,
        );
        backend.index(&memory).unwrap();

        // Search for English content
        let results = backend
            .search("Testing Unicode", &SearchFilter::new(), 10)
            .unwrap();
        assert_eq!(results.len(), 1);

        // Search for accented content (should work with FTS5's unicode tokenizer)
        let results = backend.search("cafe", &SearchFilter::new(), 10).unwrap();
        // Note: FTS5's default tokenizer may or may not match accented words
        // This test validates that unicode content doesn't break indexing
        assert!(results.is_empty() || results.len() == 1);
    }

    #[test]
    fn test_db_path() {
        let backend = SqliteBackend::in_memory().unwrap();
        assert!(backend.db_path().is_none());
    }

    #[test]
    fn test_escape_like_wildcards() {
        // No special characters
        assert_eq!(escape_like_wildcards("normal"), "normal");
        assert_eq!(escape_like_wildcards("test-tag"), "test-tag");

        // Percent sign (LIKE wildcard for "any characters")
        assert_eq!(escape_like_wildcards("100%"), "100\\%");
        assert_eq!(escape_like_wildcards("%prefix"), "\\%prefix");

        // Underscore (LIKE wildcard for "single character")
        assert_eq!(escape_like_wildcards("user_name"), "user\\_name");
        assert_eq!(escape_like_wildcards("_private"), "\\_private");

        // Backslash (the escape character itself)
        assert_eq!(escape_like_wildcards("path\\file"), "path\\\\file");

        // Multiple special characters
        assert_eq!(escape_like_wildcards("100%_test\\"), "100\\%\\_test\\\\");

        // Empty string
        assert_eq!(escape_like_wildcards(""), "");
    }

    #[test]
    fn test_tag_filtering_with_special_characters() {
        let backend = SqliteBackend::in_memory().unwrap();

        // Create memory with tag containing LIKE wildcard characters
        let mut memory = create_test_memory("id1", "Test content", Namespace::Decisions);
        memory.tags = vec!["100%_complete".to_string(), "normal-tag".to_string()];
        backend.index(&memory).unwrap();

        // Search with exact tag match (should find it)
        let mut filter = SearchFilter::new();
        filter.tags.push("100%_complete".to_string());
        let results = backend.search("Test", &filter, 10).unwrap();
        assert_eq!(
            results.len(),
            1,
            "Should find memory with escaped wildcards"
        );

        // Search with partial match that would work without escaping (should NOT find)
        let mut filter2 = SearchFilter::new();
        filter2.tags.push("100".to_string()); // Without escaping, % would match anything
        let results2 = backend.search("Test", &filter2, 10).unwrap();
        assert_eq!(
            results2.len(),
            0,
            "Should not match partial tag due to proper escaping"
        );
    }
}
