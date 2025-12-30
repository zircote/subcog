//! Redis-based index backend using `RediSearch`.
//!
//! Provides full-text search using Redis with the `RediSearch` module.

#[cfg(feature = "redis")]
mod implementation {
    use crate::models::{Memory, MemoryId, Namespace, SearchFilter};
    use crate::storage::traits::IndexBackend;
    use crate::{Error, Result};
    use redis::{Client, Commands, Connection};

    /// Redis-based index backend using `RediSearch`.
    pub struct RedisBackend {
        /// Redis client.
        client: Client,
        /// Index name in Redis.
        index_name: String,
    }

    impl RedisBackend {
        /// Creates a new Redis backend.
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

        /// Gets a connection from the client.
        fn get_connection(&self) -> Result<Connection> {
            self.client
                .get_connection()
                .map_err(|e| Error::OperationFailed {
                    operation: "redis_get_connection".to_string(),
                    cause: e.to_string(),
                })
        }

        /// Ensures the `RediSearch` index exists.
        fn ensure_index(&self) -> Result<()> {
            let mut conn = self.get_connection()?;

            // Check if index exists
            if self.index_exists(&mut conn) {
                return Ok(());
            }

            // Create the index with schema
            self.create_index(&mut conn)
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
        fn index(&mut self, memory: &Memory) -> Result<()> {
            let mut conn = self.get_connection()?;
            let key = format!("mem:{}", memory.id.as_str());

            // Store as hash
            let tags_str = memory.tags.join(",");
            let domain_str = memory.domain.to_string();
            let status_str = memory.status.as_str();
            let namespace_str = memory.namespace.as_str();

            let _: () = conn
                .hset_multiple(
                    &key,
                    &[
                        ("content", memory.content.as_str()),
                        ("namespace", namespace_str),
                        ("domain", &domain_str),
                        ("status", status_str),
                        ("tags", &tags_str),
                    ],
                )
                .map_err(|e| Error::OperationFailed {
                    operation: "redis_index".to_string(),
                    cause: e.to_string(),
                })?;

            // Set numeric fields separately
            let _: () = conn
                .hset(&key, "created_at", memory.created_at)
                .map_err(|e| Error::OperationFailed {
                    operation: "redis_index_created".to_string(),
                    cause: e.to_string(),
                })?;

            let _: () = conn
                .hset(&key, "updated_at", memory.updated_at)
                .map_err(|e| Error::OperationFailed {
                    operation: "redis_index_updated".to_string(),
                    cause: e.to_string(),
                })?;

            Ok(())
        }

        fn remove(&mut self, id: &MemoryId) -> Result<bool> {
            let mut conn = self.get_connection()?;
            let key = format!("mem:{}", id.as_str());

            let deleted: i32 = conn.del(&key).map_err(|e| Error::OperationFailed {
                operation: "redis_remove".to_string(),
                cause: e.to_string(),
            })?;

            Ok(deleted > 0)
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

            match result {
                Ok(values) => Ok(parse_search_results(&values)),
                Err(e) => Err(Error::OperationFailed {
                    operation: "redis_search".to_string(),
                    cause: e.to_string(),
                }),
            }
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

            match result {
                Ok(values) => Ok(parse_list_results(&values)),
                Err(e) => Err(Error::OperationFailed {
                    operation: "redis_list_all".to_string(),
                    cause: e.to_string(),
                }),
            }
        }

        fn clear(&mut self) -> Result<()> {
            let mut conn = self.get_connection()?;

            // Drop and recreate the index
            let _: redis::RedisResult<String> = redis::cmd("FT.DROPINDEX")
                .arg(&self.index_name)
                .arg("DD")
                .query(&mut conn);

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
        fn index(&mut self, _memory: &Memory) -> Result<()> {
            Err(Error::FeatureNotEnabled("redis".to_string()))
        }

        fn remove(&mut self, _id: &MemoryId) -> Result<bool> {
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

        fn clear(&mut self) -> Result<()> {
            Err(Error::FeatureNotEnabled("redis".to_string()))
        }
    }
}

#[cfg(not(feature = "redis"))]
pub use stub::RedisBackend;
