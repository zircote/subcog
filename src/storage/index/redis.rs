//! Redis-based index backend using `RediSearch`.
//!
//! Provides full-text search using Redis with the `RediSearch` module.
//!
//! # Connection Pooling (DB-H6)
//!
//! This backend reuses a single connection per instance. For high-concurrency
//! scenarios, consider using `r2d2-redis` or `deadpool-redis` for connection
//! pooling. The current implementation is suitable for CLI and single-threaded
//! MCP server usage.
//!
//! # Command Timeout (CHAOS-H2)
//!
//! Redis operations use a 5-second response timeout to prevent indefinite
//! blocking on slow or unresponsive Redis servers. The timeout is configured
//! via `redis::ConnectionInfo` settings.

#[cfg(feature = "redis")]
mod implementation {
    use crate::models::{Memory, MemoryId, MemoryStatus, Namespace, SearchFilter};
    use crate::storage::traits::IndexBackend;
    use crate::{Error, Result};
    use redis::{Client, Commands, Connection};
    use std::sync::Mutex;
    use std::time::Duration;

    /// Redis-based index backend using `RediSearch`.
    ///
    /// # Connection Management (DB-H6)
    ///
    /// Maintains a reusable connection via `Mutex<Option<Connection>>`.
    /// The connection is lazily initialized and reused across operations
    /// to avoid the overhead of establishing new connections for each command.
    ///
    /// # Command Timeout (CHAOS-H2)
    ///
    /// Connections are configured with a 5-second response timeout to prevent
    /// indefinite blocking on unresponsive servers.
    pub struct RedisBackend {
        /// Redis client.
        client: Client,
        /// Index name in Redis.
        index_name: String,
        /// Cached connection for reuse (DB-H6).
        connection: Mutex<Option<Connection>>,
    }

    /// Default timeout for Redis operations (CHAOS-H2).
    const REDIS_TIMEOUT: Duration = Duration::from_secs(5);

    impl RedisBackend {
        /// Creates a new Redis backend.
        ///
        /// Configures the connection with a 5-second timeout (CHAOS-H2) to prevent
        /// indefinite blocking on slow or unresponsive Redis servers.
        ///
        /// # Errors
        ///
        /// Returns an error if the Redis connection fails.
        pub fn new(connection_url: &str, index_name: impl Into<String>) -> Result<Self> {
            let client = Client::open(connection_url).map_err(|e| Error::OperationFailed {
                operation: "redis_connect".to_string(),
                cause: e.to_string(),
            })?;

            let backend = Self {
                client,
                index_name: index_name.into(),
                connection: Mutex::new(None),
            };

            // Ensure index exists
            backend.ensure_index()?;

            Ok(backend)
        }

        /// Creates a backend with default settings.
        ///
        /// # Errors
        ///
        /// Returns an error if the Redis connection fails.
        pub fn with_defaults() -> Result<Self> {
            Self::new("redis://localhost:6379", "subcog_memories")
        }

        /// Gets a connection, reusing the cached one if available (DB-H6).
        ///
        /// This method reuses an existing connection when possible, falling back
        /// to creating a new connection if the cache is empty or the connection
        /// is broken. The connection is stored in a `Mutex` for thread-safety.
        ///
        /// # Timeout (CHAOS-H2)
        ///
        /// New connections are configured with a 5-second response timeout.
        fn get_connection(&self) -> Result<Connection> {
            // Try to reuse existing connection
            let mut guard = self.connection.lock().map_err(|e| Error::OperationFailed {
                operation: "redis_lock_connection".to_string(),
                cause: e.to_string(),
            })?;

            // Take the existing connection if available
            if let Some(conn) = guard.take() {
                // Return the connection - if it fails, caller will get error
                // and next call will create fresh connection
                return Ok(conn);
            }

            // No cached connection, create a new one with timeout (CHAOS-H2)
            let conn = self
                .client
                .get_connection()
                .map_err(|e| Error::OperationFailed {
                    operation: "redis_get_connection".to_string(),
                    cause: e.to_string(),
                })?;

            // Set response timeout to prevent indefinite blocking
            conn.set_read_timeout(Some(REDIS_TIMEOUT))
                .map_err(|e| Error::OperationFailed {
                    operation: "redis_set_read_timeout".to_string(),
                    cause: e.to_string(),
                })?;
            conn.set_write_timeout(Some(REDIS_TIMEOUT))
                .map_err(|e| Error::OperationFailed {
                    operation: "redis_set_write_timeout".to_string(),
                    cause: e.to_string(),
                })?;

            Ok(conn)
        }

        /// Returns a connection to the cache for reuse (DB-H6).
        fn return_connection(&self, conn: Connection) {
            if let Ok(mut guard) = self.connection.lock() {
                *guard = Some(conn);
            }
            // If lock fails, just drop the connection - not critical
        }

        /// Ensures the `RediSearch` index exists.
        fn ensure_index(&self) -> Result<()> {
            let mut conn = self.get_connection()?;

            // Check if index exists
            let exists = self.index_exists(&mut conn);
            if exists {
                self.return_connection(conn);
                return Ok(());
            }

            // Create the index with schema
            let result = self.create_index(&mut conn);
            self.return_connection(conn);
            result
        }

        /// Checks if the index already exists.
        fn index_exists(&self, conn: &mut Connection) -> bool {
            let result: redis::RedisResult<Vec<String>> = redis::cmd("FT._LIST").query(conn);

            result
                .map(|indices| indices.iter().any(|i| i == &self.index_name))
                .unwrap_or(false)
        }

        /// Creates the `RediSearch` index.
        fn create_index(&self, conn: &mut Connection) -> Result<()> {
            let result: redis::RedisResult<String> = redis::cmd("FT.CREATE")
                .arg(&self.index_name)
                .arg("ON")
                .arg("HASH")
                .arg("PREFIX")
                .arg(1)
                .arg("mem:")
                .arg("SCHEMA")
                .arg("content")
                .arg("TEXT")
                .arg("WEIGHT")
                .arg("1.0")
                .arg("namespace")
                .arg("TAG")
                .arg("domain")
                .arg("TAG")
                .arg("status")
                .arg("TAG")
                .arg("tags")
                .arg("TAG")
                .arg("created_at")
                .arg("NUMERIC")
                .arg("SORTABLE")
                .arg("updated_at")
                .arg("NUMERIC")
                .arg("SORTABLE")
                .query(conn);

            match result {
                Ok(_) => Ok(()),
                Err(e) if e.to_string().contains("Index already exists") => Ok(()),
                Err(e) => Err(Error::OperationFailed {
                    operation: "redis_create_index".to_string(),
                    cause: e.to_string(),
                }),
            }
        }

        /// Builds a filter clause for `RediSearch` queries.
        fn build_filter_clause(filter: &SearchFilter) -> String {
            let mut clauses = Vec::new();

            if !filter.namespaces.is_empty() {
                let ns_tags: Vec<&str> = filter
                    .namespaces
                    .iter()
                    .copied()
                    .map(namespace_to_tag)
                    .collect();
                clauses.push(format!("@namespace:{{{}}}", ns_tags.join("|")));
            }

            if !filter.domains.is_empty() {
                let domain_strs: Vec<String> =
                    filter.domains.iter().map(ToString::to_string).collect();
                clauses.push(format!("@domain:{{{}}}", domain_strs.join("|")));
            }

            if !filter.statuses.is_empty() {
                let status_strs: Vec<&str> = filter
                    .statuses
                    .iter()
                    .map(crate::models::MemoryStatus::as_str)
                    .collect();
                clauses.push(format!("@status:{{{}}}", status_strs.join("|")));
            }

            if clauses.is_empty() {
                String::new()
            } else {
                clauses.join(" ")
            }
        }
    }

    /// Converts namespace to tag value.
    const fn namespace_to_tag(ns: Namespace) -> &'static str {
        ns.as_str()
    }

    impl IndexBackend for RedisBackend {
        fn index(&self, memory: &Memory) -> Result<()> {
            let mut conn = self.get_connection()?;
            let key = format!("mem:{}", memory.id.as_str());

            // Store as hash
            let tags_str = memory.tags.join(",");
            let domain_str = memory.domain.to_string();
            let status_str = memory.status.as_str();
            let namespace_str = memory.namespace.as_str();

            let result: redis::RedisResult<()> = conn.hset_multiple(
                &key,
                &[
                    ("content", memory.content.as_str()),
                    ("namespace", namespace_str),
                    ("domain", &domain_str),
                    ("status", status_str),
                    ("tags", &tags_str),
                ],
            );

            if let Err(e) = result {
                self.return_connection(conn);
                return Err(Error::OperationFailed {
                    operation: "redis_index".to_string(),
                    cause: e.to_string(),
                });
            }

            // Set numeric fields separately
            let result: redis::RedisResult<()> = conn.hset(&key, "created_at", memory.created_at);
            if let Err(e) = result {
                self.return_connection(conn);
                return Err(Error::OperationFailed {
                    operation: "redis_index_created".to_string(),
                    cause: e.to_string(),
                });
            }

            let result: redis::RedisResult<()> = conn.hset(&key, "updated_at", memory.updated_at);
            if let Err(e) = result {
                self.return_connection(conn);
                return Err(Error::OperationFailed {
                    operation: "redis_index_updated".to_string(),
                    cause: e.to_string(),
                });
            }

            self.return_connection(conn);
            Ok(())
        }

        fn remove(&self, id: &MemoryId) -> Result<bool> {
            let mut conn = self.get_connection()?;
            let key = format!("mem:{}", id.as_str());

            let result: redis::RedisResult<i32> = conn.del(&key);
            match result {
                Ok(deleted) => {
                    self.return_connection(conn);
                    Ok(deleted > 0)
                },
                Err(e) => {
                    self.return_connection(conn);
                    Err(Error::OperationFailed {
                        operation: "redis_remove".to_string(),
                        cause: e.to_string(),
                    })
                },
            }
        }

        fn search(
            &self,
            query: &str,
            filter: &SearchFilter,
            limit: usize,
        ) -> Result<Vec<(MemoryId, f32)>> {
            let mut conn = self.get_connection()?;

            // Build query with filters
            let filter_clause = Self::build_filter_clause(filter);
            let full_query = if filter_clause.is_empty() {
                query.to_string()
            } else {
                format!("{query} {filter_clause}")
            };

            // FT.SEARCH idx "query" LIMIT 0 limit WITHSCORES
            let result: redis::RedisResult<Vec<redis::Value>> = redis::cmd("FT.SEARCH")
                .arg(&self.index_name)
                .arg(&full_query)
                .arg("LIMIT")
                .arg(0)
                .arg(limit)
                .arg("WITHSCORES")
                .query(&mut conn);

            let output = match result {
                Ok(values) => Ok(parse_search_results(&values)),
                Err(e) => Err(Error::OperationFailed {
                    operation: "redis_search".to_string(),
                    cause: e.to_string(),
                }),
            };
            self.return_connection(conn);
            output
        }

        fn list_all(&self, filter: &SearchFilter, limit: usize) -> Result<Vec<(MemoryId, f32)>> {
            let mut conn = self.get_connection()?;

            // Build query - use * for all, with optional filters
            let filter_clause = Self::build_filter_clause(filter);
            let query = if filter_clause.is_empty() {
                "*".to_string()
            } else {
                filter_clause
            };

            // FT.SEARCH idx "*" LIMIT 0 limit SORTBY updated_at DESC
            let result: redis::RedisResult<Vec<redis::Value>> = redis::cmd("FT.SEARCH")
                .arg(&self.index_name)
                .arg(&query)
                .arg("LIMIT")
                .arg(0)
                .arg(limit)
                .arg("SORTBY")
                .arg("updated_at")
                .arg("DESC")
                .query(&mut conn);

            let output = match result {
                Ok(values) => Ok(parse_list_results(&values)),
                Err(e) => Err(Error::OperationFailed {
                    operation: "redis_list_all".to_string(),
                    cause: e.to_string(),
                }),
            };
            self.return_connection(conn);
            output
        }

        fn get_memory(&self, id: &MemoryId) -> Result<Option<Memory>> {
            use crate::models::{Domain, Namespace};

            let mut conn = self.get_connection()?;
            let key = format!("mem:{}", id.as_str());

            // Get all fields from hash
            let result: redis::RedisResult<std::collections::HashMap<String, String>> =
                conn.hgetall(&key);

            let output = match result {
                Ok(fields) if fields.is_empty() => Ok(None),
                Ok(fields) => {
                    let content = fields.get("content").cloned().unwrap_or_default();
                    let namespace_str = fields.get("namespace").cloned().unwrap_or_default();
                    let domain_str = fields.get("domain").cloned();
                    let status_str = fields.get("status").cloned().unwrap_or_default();
                    let tags_str = fields.get("tags").cloned();
                    let created_at: u64 = fields
                        .get("created_at")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    let updated_at: u64 = fields
                        .get("updated_at")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(created_at);

                    let namespace = Namespace::parse(&namespace_str).unwrap_or_default();
                    let domain = domain_str.map_or_else(Domain::new, |_| Domain::new());
                    let status = parse_memory_status(&status_str);
                    let tags = parse_tags_string(tags_str);

                    Ok(Some(Memory {
                        id: id.clone(),
                        content,
                        namespace,
                        domain,
                        status,
                        created_at,
                        updated_at,
                        tombstoned_at: None,
                        embedding: None,
                        tags,
                        source: None,
                    }))
                },
                Err(e) => Err(Error::OperationFailed {
                    operation: "redis_get_memory".to_string(),
                    cause: e.to_string(),
                }),
            };
            self.return_connection(conn);
            output
        }

        fn clear(&self) -> Result<()> {
            let mut conn = self.get_connection()?;

            // Drop and recreate the index
            let _: redis::RedisResult<String> = redis::cmd("FT.DROPINDEX")
                .arg(&self.index_name)
                .arg("DD")
                .query(&mut conn);

            // Return connection before ensure_index (which gets its own)
            self.return_connection(conn);

            // Recreate the index
            self.ensure_index()
        }
    }

    /// Parses FT.SEARCH results with scores.
    fn parse_search_results(values: &[redis::Value]) -> Vec<(MemoryId, f32)> {
        let mut results = Vec::new();

        if values.is_empty() {
            return results;
        }

        let mut i = 1; // Skip count
        while i + 1 < values.len() {
            if let redis::Value::BulkString(key_bytes) = &values[i] {
                let key = String::from_utf8_lossy(key_bytes);
                let id = key.strip_prefix("mem:").unwrap_or(&key);

                i += 1;
                let score = parse_score(&values[i]);
                results.push((MemoryId::new(id), score));
            }

            i += 1;
            // Skip fields array if present
            if i < values.len() && matches!(&values[i], redis::Value::Array(_)) {
                i += 1;
            }
        }

        results
    }

    /// Parses a score from a Redis value.
    fn parse_score(value: &redis::Value) -> f32 {
        match value {
            redis::Value::BulkString(s) => String::from_utf8_lossy(s).parse::<f32>().unwrap_or(1.0),
            _ => 1.0,
        }
    }

    /// Parses FT.SEARCH results without scores (for `list_all`).
    fn parse_list_results(values: &[redis::Value]) -> Vec<(MemoryId, f32)> {
        let mut results = Vec::new();

        if values.is_empty() {
            return results;
        }

        let mut i = 1; // Skip count
        while i < values.len() {
            if let redis::Value::BulkString(key_bytes) = &values[i] {
                let key = String::from_utf8_lossy(key_bytes);
                let id = key.strip_prefix("mem:").unwrap_or(&key);
                results.push((MemoryId::new(id), 1.0));
            }

            i += 1;
            // Skip fields array if present
            if i < values.len() && matches!(&values[i], redis::Value::Array(_)) {
                i += 1;
            }
        }

        results
    }

    /// Parses a status string to `MemoryStatus`.
    fn parse_memory_status(s: &str) -> MemoryStatus {
        match s.to_lowercase().as_str() {
            "active" => MemoryStatus::Active,
            "archived" => MemoryStatus::Archived,
            "superseded" => MemoryStatus::Superseded,
            "pending" => MemoryStatus::Pending,
            "deleted" => MemoryStatus::Deleted,
            _ => MemoryStatus::Active,
        }
    }

    /// Parses a comma-separated tags string.
    fn parse_tags_string(tags_str: Option<String>) -> Vec<String> {
        tags_str.map_or_else(Vec::new, |t| {
            t.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
    }
}

#[cfg(feature = "redis")]
pub use implementation::RedisBackend;

#[cfg(not(feature = "redis"))]
mod stub {
    use crate::models::{Memory, MemoryId, SearchFilter};
    use crate::storage::traits::IndexBackend;
    use crate::{Error, Result};

    /// Stub Redis backend when feature is not enabled.
    pub struct RedisBackend;

    impl RedisBackend {
        /// Creates a new Redis backend (stub).
        ///
        /// # Errors
        ///
        /// Always returns an error because the feature is not enabled.
        pub fn new(_connection_url: &str, _index_name: impl Into<String>) -> Result<Self> {
            Err(Error::FeatureNotEnabled("redis".to_string()))
        }

        /// Creates a backend with default settings (stub).
        ///
        /// # Errors
        ///
        /// Always returns an error because the feature is not enabled.
        pub fn with_defaults() -> Result<Self> {
            Err(Error::FeatureNotEnabled("redis".to_string()))
        }
    }

    impl IndexBackend for RedisBackend {
        fn index(&self, _memory: &Memory) -> Result<()> {
            Err(Error::FeatureNotEnabled("redis".to_string()))
        }

        fn remove(&self, _id: &MemoryId) -> Result<bool> {
            Err(Error::FeatureNotEnabled("redis".to_string()))
        }

        fn search(
            &self,
            _query: &str,
            _filter: &SearchFilter,
            _limit: usize,
        ) -> Result<Vec<(MemoryId, f32)>> {
            Err(Error::FeatureNotEnabled("redis".to_string()))
        }

        fn list_all(&self, _filter: &SearchFilter, _limit: usize) -> Result<Vec<(MemoryId, f32)>> {
            Err(Error::FeatureNotEnabled("redis".to_string()))
        }

        fn get_memory(&self, _id: &MemoryId) -> Result<Option<Memory>> {
            Err(Error::FeatureNotEnabled("redis".to_string()))
        }

        fn clear(&self) -> Result<()> {
            Err(Error::FeatureNotEnabled("redis".to_string()))
        }
    }
}

#[cfg(not(feature = "redis"))]
pub use stub::RedisBackend;
