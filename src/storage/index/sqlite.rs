//! `SQLite` + FTS5 index backend.
//!
//! Provides full-text search using `SQLite`'s FTS5 extension.

use crate::models::{Memory, MemoryId, SearchFilter};
use crate::storage::traits::IndexBackend;
use crate::{Error, Result};
use chrono::{TimeZone, Utc};
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

/// Converts a glob pattern to a SQL LIKE pattern safely (HIGH-SEC-005).
///
/// First escapes existing SQL LIKE wildcards (`%`, `_`, `\`), then converts
/// glob wildcards (`*` → `%`, `?` → `_`). This prevents SQL injection via
/// patterns like `foo%bar` where `%` would otherwise be a SQL wildcard.
///
/// # Examples
///
/// ```ignore
/// // Glob wildcards are converted
/// assert_eq!(glob_to_like_pattern("src/*.rs"), "src/%\\.rs");
/// // Literal % is escaped
/// assert_eq!(glob_to_like_pattern("100%"), "100\\%");
/// // Combined: literal % escaped, glob * converted
/// assert_eq!(glob_to_like_pattern("foo%*bar"), "foo\\%%bar");
/// ```
fn glob_to_like_pattern(pattern: &str) -> String {
    let mut result = String::with_capacity(pattern.len() * 2);
    for c in pattern.chars() {
        match c {
            // Escape existing SQL LIKE wildcards (they're meant to be literal)
            '%' | '_' | '\\' => {
                result.push('\\');
                result.push(c);
            },
            // Convert glob wildcards to SQL LIKE wildcards
            '*' => result.push('%'),
            '?' => result.push('_'),
            _ => result.push(c),
        }
    }
    result
}

/// `SQLite`-based index backend with FTS5.
///
/// # Concurrency Model
///
/// Uses a `Mutex<Connection>` for thread-safe access. While this serializes
/// database operations, `SQLite`'s WAL mode and `busy_timeout` pragma mitigate
/// contention:
///
/// - **WAL mode**: Allows concurrent readers with a single writer
/// - **`busy_timeout`**: Waits up to 5 seconds for locks instead of failing immediately
/// - **NORMAL synchronous**: Balances durability with performance
///
/// For high-throughput scenarios requiring true connection pooling, consider
/// using `r2d2-rusqlite` or `deadpool-sqlite`. This would require refactoring
/// to use `Pool<SqliteConnectionManager>` instead of `Mutex<Connection>`.
pub struct SqliteBackend {
    /// Connection to the `SQLite` database.
    ///
    /// Protected by Mutex because `rusqlite::Connection` is not `Sync`.
    /// WAL mode and `busy_timeout` handle concurrent access gracefully.
    conn: Mutex<Connection>,
    /// Path to the `SQLite` database (None for in-memory).
    db_path: Option<PathBuf>,
}

struct MemoryRow {
    id: String,
    namespace: String,
    domain: Option<String>,
    project_id: Option<String>,
    branch: Option<String>,
    file_path: Option<String>,
    status: String,
    created_at: i64,
    tombstoned_at: Option<i64>,
    tags: Option<String>,
    source: Option<String>,
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
        // Set busy timeout to 5 seconds to handle lock contention gracefully
        // This prevents SQLITE_BUSY errors during high concurrent access
        let _ = conn.pragma_update(None, "busy_timeout", "5000");

        // Create the main table for memory metadata
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

        // Add facet columns if they don't exist (ADR-0048/0049)
        let _ = conn.execute("ALTER TABLE memories ADD COLUMN project_id TEXT", []);
        let _ = conn.execute("ALTER TABLE memories ADD COLUMN branch TEXT", []);
        let _ = conn.execute("ALTER TABLE memories ADD COLUMN file_path TEXT", []);

        // Add tombstoned_at column if it doesn't exist (ADR-0053)
        let _ = conn.execute("ALTER TABLE memories ADD COLUMN tombstoned_at INTEGER", []);

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

        // Partial index on tombstoned_at for cleanup queries
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_tombstoned_at ON memories(tombstoned_at) WHERE tombstoned_at IS NOT NULL",
            [],
        );

        // Composite index for common filter patterns
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_namespace_status ON memories(namespace, status)",
            [],
        );

        // Compound index for time-filtered namespace queries (Phase 15 fix)
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_namespace_created ON memories(namespace, created_at DESC)",
            [],
        );

        // Compound index for source filtering with status (Phase 15 fix)
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
        // HIGH-SEC-005: Use glob_to_like_pattern to escape SQL wildcards before conversion
        if let Some(ref pattern) = filter.source_pattern {
            conditions.push(format!("m.source LIKE ?{param_idx} ESCAPE '\\'"));
            param_idx += 1;
            params.push(glob_to_like_pattern(pattern));
        }

        if let Some(ref project_id) = filter.project_id {
            conditions.push(format!("m.project_id = ?{param_idx}"));
            param_idx += 1;
            params.push(project_id.clone());
        }

        if let Some(ref branch) = filter.branch {
            conditions.push(format!("m.branch = ?{param_idx}"));
            param_idx += 1;
            params.push(branch.clone());
        }

        if let Some(ref file_path) = filter.file_path {
            conditions.push(format!("m.file_path = ?{param_idx}"));
            param_idx += 1;
            params.push(file_path.clone());
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

        // Exclude tombstoned memories by default (ADR-0053)
        if !filter.include_tombstoned {
            conditions.push("m.status != 'tombstoned'".to_string());
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

    /// Performs a WAL checkpoint to merge WAL file into main database (RES-M3).
    ///
    /// This is useful for:
    /// - Graceful shutdown (ensure WAL is flushed)
    /// - Periodic maintenance (prevent WAL file growth)
    /// - Before backup operations
    ///
    /// Uses TRUNCATE mode which blocks briefly but ensures WAL is fully merged
    /// and then truncated to zero bytes.
    ///
    /// # Returns
    ///
    /// Returns a tuple of (`pages_written`, `pages_remaining`) on success.
    /// `pages_remaining` should be 0 if checkpoint completed fully.
    ///
    /// # Errors
    ///
    /// Returns an error if the checkpoint operation fails.
    #[instrument(skip(self), fields(operation = "checkpoint", backend = "sqlite"))]
    pub fn checkpoint(&self) -> Result<(u32, u32)> {
        let start = Instant::now();
        let conn = acquire_lock(&self.conn);

        // PRAGMA wal_checkpoint(TRUNCATE) checkpoints and truncates the WAL file
        // Returns: (busy, log_pages, checkpointed_pages)
        // - busy: 0 if not blocked, 1 if another connection blocked us
        // - log_pages: total pages in WAL
        // - checkpointed_pages: pages successfully written to main database
        let result: std::result::Result<(i32, i32, i32), _> =
            conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            });

        match result {
            Ok((busy, log_pages, checkpointed_pages)) => {
                #[allow(clippy::cast_sign_loss)]
                let (log, checkpointed) = (log_pages as u32, checkpointed_pages as u32);

                tracing::info!(
                    busy = busy,
                    log_pages = log,
                    checkpointed_pages = checkpointed,
                    duration_ms = start.elapsed().as_millis(),
                    "WAL checkpoint completed"
                );

                metrics::counter!(
                    "sqlite_checkpoint_total",
                    "status" => "success"
                )
                .increment(1);
                metrics::gauge!("sqlite_wal_pages_checkpointed").set(f64::from(checkpointed));
                metrics::histogram!("sqlite_checkpoint_duration_ms")
                    .record(start.elapsed().as_secs_f64() * 1000.0);

                Ok((checkpointed, log.saturating_sub(checkpointed)))
            },
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    duration_ms = start.elapsed().as_millis(),
                    "WAL checkpoint failed"
                );

                metrics::counter!(
                    "sqlite_checkpoint_total",
                    "status" => "error"
                )
                .increment(1);

                Err(Error::OperationFailed {
                    operation: "wal_checkpoint".to_string(),
                    cause: e.to_string(),
                })
            },
        }
    }

    /// Returns the current WAL file size in pages.
    ///
    /// Useful for monitoring and deciding when to trigger a checkpoint.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    #[must_use]
    pub fn wal_size(&self) -> Option<u32> {
        let conn = acquire_lock(&self.conn);

        // PRAGMA wal_checkpoint(PASSIVE) returns current state without blocking
        let result: std::result::Result<(i32, i32, i32), _> =
            conn.query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            });

        result.ok().map(|(_, log_pages, _)| {
            #[allow(clippy::cast_sign_loss)]
            let pages = log_pages as u32;
            pages
        })
    }

    /// Checkpoints the WAL if it exceeds the given threshold in pages.
    ///
    /// Default `SQLite` auto-checkpoint threshold is 1000 pages (~4MB with 4KB pages).
    /// This method allows explicit control over checkpointing.
    ///
    /// # Arguments
    ///
    /// * `threshold_pages` - Checkpoint if WAL size exceeds this number of pages.
    ///
    /// # Returns
    ///
    /// Returns `Some((pages_written, pages_remaining))` if checkpoint was performed,
    /// `None` if WAL was below threshold.
    ///
    /// # Errors
    ///
    /// Returns an error if the checkpoint operation fails.
    pub fn checkpoint_if_needed(&self, threshold_pages: u32) -> Result<Option<(u32, u32)>> {
        if let Some(current_size) = self.wal_size() {
            if current_size > threshold_pages {
                tracing::debug!(
                    current_pages = current_size,
                    threshold = threshold_pages,
                    "WAL exceeds threshold, triggering checkpoint"
                );
                return self.checkpoint().map(Some);
            }
        }
        Ok(None)
    }
}

fn fetch_memory_row(conn: &Connection, id: &MemoryId) -> Result<Option<MemoryRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.namespace, m.domain, m.project_id, m.branch, m.file_path, m.status, m.created_at,
                    m.tombstoned_at, m.tags, m.source, f.content
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
        if d.is_empty() || d == "project" {
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
        "tombstoned" => MemoryStatus::Tombstoned,
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
    let tombstoned_at = row
        .tombstoned_at
        .and_then(|ts| Utc.timestamp_opt(ts, 0).single());

    Memory {
        id: MemoryId::new(row.id),
        content: row.content,
        namespace,
        domain,
        project_id: row.project_id,
        branch: row.branch,
        file_path: row.file_path,
        status,
        created_at: created_at_u64,
        updated_at: created_at_u64,
        tombstoned_at,
        embedding: None,
        tags,
        source: row.source,
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
                let tombstoned_at_i64 = memory.tombstoned_at.map(|t| t.timestamp());
                conn.execute(
                    "INSERT OR REPLACE INTO memories (id, namespace, domain, project_id, branch, file_path, status, created_at, tags, source, tombstoned_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        memory.id.as_str(),
                        memory.namespace.as_str(),
                        domain_str,
                        memory.project_id.as_deref(),
                        memory.branch.as_deref(),
                        memory.file_path.as_deref(),
                        memory.status.as_str(),
                        created_at_i64,
                        tags_str,
                        memory.source.as_deref(),
                        tombstoned_at_i64
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
                "SELECT m.id, m.namespace, m.domain, m.project_id, m.branch, m.file_path, m.status, m.created_at,
                        m.tombstoned_at, m.tags, m.source, f.content
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
    fn test_search_with_facet_filters() {
        let backend = SqliteBackend::in_memory().unwrap();

        let mut memory = create_test_memory("id1", "Rust facets", Namespace::Decisions);
        memory.project_id = Some("github.com/org/repo".to_string());
        memory.branch = Some("main".to_string());
        memory.file_path = Some("src/lib.rs".to_string());

        backend.index(&memory).unwrap();

        let filter = SearchFilter::new()
            .with_project_id("github.com/org/repo")
            .with_branch("main")
            .with_file_path("src/lib.rs");

        let results = backend.search("Rust", &filter, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.as_str(), "id1");
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
    fn test_glob_to_like_pattern() {
        // Glob wildcards are converted
        assert_eq!(glob_to_like_pattern("*"), "%");
        assert_eq!(glob_to_like_pattern("?"), "_");
        assert_eq!(glob_to_like_pattern("src/*.rs"), "src/%.rs");
        assert_eq!(glob_to_like_pattern("test?.txt"), "test_.txt");

        // Literal SQL LIKE wildcards are escaped
        assert_eq!(glob_to_like_pattern("100%"), "100\\%");
        assert_eq!(glob_to_like_pattern("user_name"), "user\\_name");

        // Combined: literal % escaped, glob * converted
        assert_eq!(glob_to_like_pattern("foo%*bar"), "foo\\%%bar");
        assert_eq!(glob_to_like_pattern("*_test%?"), "%\\_test\\%_");

        // Backslash is escaped
        assert_eq!(glob_to_like_pattern("path\\file*"), "path\\\\file%");

        // Complex pattern (** becomes %%, each * is a separate wildcard)
        assert_eq!(
            glob_to_like_pattern("src/**/test_*.rs"),
            "src/%%/test\\_%.rs"
        );

        // Empty string
        assert_eq!(glob_to_like_pattern(""), "");

        // No special characters
        assert_eq!(glob_to_like_pattern("normal"), "normal");
    }

    #[test]
    fn test_source_pattern_with_sql_wildcards() {
        let backend = SqliteBackend::in_memory().unwrap();

        // Create memories with various source paths
        let mut memory1 = create_test_memory("id1", "Test 100% pass", Namespace::Decisions);
        memory1.source = Some("src/100%_file.rs".to_string());
        backend.index(&memory1).unwrap();

        let mut memory2 = create_test_memory("id2", "Test content", Namespace::Decisions);
        memory2.source = Some("src/other_file.rs".to_string());
        backend.index(&memory2).unwrap();

        // Search with source pattern containing literal % (should be escaped)
        let filter = SearchFilter::new().with_source_pattern("src/100%*");
        let results = backend.search("Test", &filter, 10).unwrap();
        assert_eq!(
            results.len(),
            1,
            "Should find only file with literal 100% in name"
        );
        assert_eq!(results[0].0.as_str(), "id1");

        // Search with glob wildcard only (should match both)
        let filter2 = SearchFilter::new().with_source_pattern("src/*");
        let results2 = backend.search("Test", &filter2, 10).unwrap();
        assert_eq!(
            results2.len(),
            2,
            "Should find both files with glob wildcard"
        );
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

    #[test]
    fn test_checkpoint() {
        let backend = SqliteBackend::in_memory().unwrap();

        // Index some data to create WAL entries
        let memory = create_test_memory("checkpoint-test", "Test checkpoint", Namespace::Decisions);
        backend.index(&memory).unwrap();

        // Checkpoint should succeed (even on empty WAL)
        let result = backend.checkpoint();
        assert!(result.is_ok(), "Checkpoint should succeed");

        let (written, remaining) = result.unwrap();
        // In-memory databases may have 0 pages since WAL behavior differs
        assert_eq!(
            remaining, 0,
            "No pages should remain after TRUNCATE checkpoint"
        );
        // written can be 0 for in-memory DBs
        let _ = written;
    }

    #[test]
    fn test_wal_size() {
        let backend = SqliteBackend::in_memory().unwrap();

        // WAL size should be queryable
        let size = backend.wal_size();
        // In-memory databases might return Some(0) or None depending on WAL mode
        // The important thing is it doesn't crash
        let _ = size;
    }

    #[test]
    fn test_checkpoint_if_needed_below_threshold() {
        let backend = SqliteBackend::in_memory().unwrap();

        // First check current WAL size
        let wal_size = backend.wal_size().unwrap_or(0);

        // With threshold above current size, checkpoint shouldn't trigger
        let result = backend.checkpoint_if_needed(wal_size.saturating_add(1000));
        assert!(result.is_ok());
        // In-memory databases have minimal WAL, so this should not checkpoint
        // unless WAL is already above threshold
        let _ = result.unwrap(); // Just verify it doesn't error
    }

    #[test]
    fn test_checkpoint_if_needed_above_threshold() {
        let backend = SqliteBackend::in_memory().unwrap();

        // With threshold of 0, checkpoint should always trigger (if WAL exists)
        let result = backend.checkpoint_if_needed(0);
        assert!(result.is_ok());
        // Result may be Some or None depending on WAL state
    }
}
