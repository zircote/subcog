//! Redis-based vector backend.
//!
//! Provides vector similarity search using Redis Stack's Vector Similarity Search (VSS)
//! feature. Requires Redis Stack 7.2+ or `RediSearch` 2.4+ module.
//!
//! # Redis Commands Used
//!
//! | Operation | Redis Command | Description |
//! |-----------|---------------|-------------|
//! | Create Index | `FT.CREATE` | Creates vector index with HNSW algorithm |
//! | Upsert | `HSET` | Stores vector as binary blob in hash |
//! | Search | `FT.SEARCH ... KNN` | K-nearest neighbor search |
//! | Remove | `DEL` | Deletes hash key |
//! | Count | `FT.INFO` | Gets index statistics |
//! | Clear | `FT.DROPINDEX` + recreate | Drops and recreates index |
//!
//! # Configuration
//!
//! ```toml
//! [storage.vector]
//! backend = "redis"
//! redis_url = "redis://localhost:6379"
//! index_name = "subcog_vectors"
//! dimensions = 384
//! ```
//!
//! # Index Schema
//!
//! The index uses the following schema:
//! - Key pattern: `{index_name}:{memory_id}`
//! - Fields:
//!   - `embedding`: VECTOR field with HNSW algorithm
//!   - `memory_id`: TAG field for exact match filtering
//!
//! # HNSW Parameters
//!
//! | Parameter | Default | Description |
//! |-----------|---------|-------------|
//! | `M` | 16 | Max outgoing edges per node |
//! | `EF_CONSTRUCTION` | 200 | Size of dynamic candidate list |
//! | `EF_RUNTIME` | 10 | Search-time candidate list size |
//! | `DISTANCE_METRIC` | COSINE | Similarity measure |
//!
//! # Thread Safety
//!
//! This backend uses interior mutability via `Mutex` to enable sharing
//! via `Arc<dyn VectorBackend>`. The connection is cached and reused
//! across operations for efficiency.

use crate::models::MemoryId;
use crate::storage::traits::{VectorBackend, VectorFilter};
use crate::{Error, Result};

#[cfg(feature = "redis")]
use crate::storage::resilience::{StorageResilienceConfig, retry_connection};
#[cfg(feature = "redis")]
use redis::{Client, Commands, Connection, RedisResult};
#[cfg(feature = "redis")]
use std::sync::Mutex;
#[cfg(feature = "redis")]
use std::time::Duration;

/// Default timeout for Redis operations (CHAOS-HIGH-005).
///
/// Configurable via `SUBCOG_TIMEOUT_REDIS_MS` environment variable.
#[cfg(feature = "redis")]
fn default_redis_timeout() -> Duration {
    crate::config::OperationTimeoutConfig::from_env().get(crate::config::OperationType::Redis)
}

/// Redis-based vector backend using `RediSearch` Vector Similarity Search.
///
/// This backend requires Redis Stack or Redis with the `RediSearch` 2.4+ module.
/// Vectors are stored as binary blobs in Redis hashes and indexed using HNSW.
///
/// # Thread Safety
///
/// Uses interior mutability via `Mutex` for mutable state, enabling the backend
/// to be shared via `Arc<dyn VectorBackend>` across threads.
///
/// # Timeout Configuration (CHAOS-HIGH-005)
///
/// Redis operation timeouts are configurable via `SUBCOG_TIMEOUT_REDIS_MS`.
/// Default: 5000ms.
pub struct RedisVectorBackend {
    /// Redis connection URL.
    connection_url: String,
    /// Index name in Redis.
    index_name: String,
    /// Embedding dimensions.
    dimensions: usize,
    /// Redis client.
    #[cfg(feature = "redis")]
    client: Client,
    /// Cached connection for reuse (DB-H6).
    #[cfg(feature = "redis")]
    connection: Mutex<Option<Connection>>,
    /// Whether the index has been created (interior mutability).
    #[cfg(feature = "redis")]
    index_created: Mutex<bool>,
    /// Operation timeout (CHAOS-HIGH-005).
    #[cfg(feature = "redis")]
    timeout: Duration,
}

impl RedisVectorBackend {
    /// Default embedding dimensions for all-MiniLM-L6-v2.
    ///
    /// Re-exported from `crate::embedding::DEFAULT_DIMENSIONS` for convenience.
    pub const DEFAULT_DIMENSIONS: usize = crate::embedding::DEFAULT_DIMENSIONS;

    /// Creates a new Redis vector backend.
    ///
    /// # Errors
    ///
    /// Returns an error if the Redis client cannot be created.
    #[cfg(feature = "redis")]
    pub fn new(
        connection_url: impl Into<String>,
        index_name: impl Into<String>,
        dimensions: usize,
    ) -> Result<Self> {
        let connection_url = connection_url.into();
        let client = Client::open(connection_url.as_str()).map_err(|e| Error::OperationFailed {
            operation: "redis_connect".to_string(),
            cause: e.to_string(),
        })?;

        Ok(Self {
            connection_url,
            index_name: index_name.into(),
            dimensions,
            client,
            connection: Mutex::new(None),
            index_created: Mutex::new(false),
            timeout: default_redis_timeout(),
        })
    }

    /// Creates a new Redis vector backend (stub when feature disabled).
    #[cfg(not(feature = "redis"))]
    #[must_use]
    pub fn new(
        connection_url: impl Into<String>,
        index_name: impl Into<String>,
        dimensions: usize,
    ) -> Self {
        Self {
            connection_url: connection_url.into(),
            index_name: index_name.into(),
            dimensions,
        }
    }

    /// Creates a backend with default settings.
    ///
    /// # Errors
    ///
    /// Returns an error if the Redis client cannot be created.
    #[cfg(feature = "redis")]
    pub fn with_defaults() -> Result<Self> {
        Self::new(
            "redis://localhost:6379",
            "subcog_vectors",
            Self::DEFAULT_DIMENSIONS,
        )
    }

    /// Creates a backend with default settings (stub when feature disabled).
    #[cfg(not(feature = "redis"))]
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(
            "redis://localhost:6379",
            "subcog_vectors",
            Self::DEFAULT_DIMENSIONS,
        )
    }

    /// Returns the connection URL.
    #[must_use]
    pub fn connection_url(&self) -> &str {
        &self.connection_url
    }

    /// Returns the index name.
    #[must_use]
    pub fn index_name(&self) -> &str {
        &self.index_name
    }

    /// Checks if the Redis connection is healthy (DB-HIGH-002).
    ///
    /// Performs a PING command to verify connectivity and responsiveness.
    ///
    /// # Returns
    ///
    /// `Ok(true)` if the connection is healthy, `Ok(false)` if unhealthy,
    /// or `Err` if the health check itself fails unexpectedly.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let backend = RedisVectorBackend::new("redis://localhost:6379", "subcog", 384)?;
    /// if backend.health_check()? {
    ///     println!("Redis vector backend is healthy");
    /// } else {
    ///     eprintln!("Redis vector backend is not responding");
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be established.
    #[cfg(feature = "redis")]
    pub fn health_check(&self) -> Result<bool> {
        let mut conn = match self.get_connection() {
            Ok(c) => c,
            Err(_) => return Ok(false),
        };

        let result: redis::RedisResult<String> = redis::cmd("PING").query(&mut conn);

        let healthy = result.is_ok_and(|response| response == "PONG");

        self.return_connection(conn);
        Ok(healthy)
    }

    /// Health check stub (DB-HIGH-002).
    ///
    /// # Errors
    ///
    /// Always returns an error because the feature is not enabled.
    #[cfg(not(feature = "redis"))]
    pub fn health_check(&self) -> Result<bool> {
        Err(Error::FeatureNotEnabled("redis".to_string()))
    }
}

#[cfg(feature = "redis")]
impl RedisVectorBackend {
    /// Returns the key prefix for memory vectors.
    fn key_prefix(&self) -> String {
        format!("{}:", self.index_name)
    }

    /// Returns the Redis key for a memory ID.
    fn memory_key(&self, id: &MemoryId) -> String {
        format!("{}:{}", self.index_name, id.as_str())
    }

    /// Validates embedding dimensions.
    fn validate_embedding(&self, embedding: &[f32]) -> Result<()> {
        if embedding.len() != self.dimensions {
            return Err(Error::InvalidInput(format!(
                "Embedding dimension mismatch: expected {}, got {}",
                self.dimensions,
                embedding.len()
            )));
        }
        Ok(())
    }

    /// Converts f32 vector to bytes for Redis storage.
    fn vector_to_bytes(embedding: &[f32]) -> Vec<u8> {
        embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
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
    ///
    /// # Connection Retry (CHAOS-HIGH-003)
    ///
    /// New connection attempts use exponential backoff with jitter for transient
    /// failures (Redis starting up, network issues, etc.).
    fn get_connection(&self) -> Result<Connection> {
        // Try to reuse existing connection
        let mut guard = self.connection.lock().map_err(|e| Error::OperationFailed {
            operation: "redis_lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        // Take the existing connection if available
        if let Some(conn) = guard.take() {
            return Ok(conn);
        }
        drop(guard); // Release lock before potentially slow retry loop

        // No cached connection, create a new one with retry (CHAOS-HIGH-003)
        let resilience_config = StorageResilienceConfig::from_env();
        let timeout = self.timeout;

        retry_connection(&resilience_config, "redis_vector", "get_connection", || {
            let conn = self
                .client
                .get_connection()
                .map_err(|e| Error::OperationFailed {
                    operation: "redis_get_connection".to_string(),
                    cause: e.to_string(),
                })?;

            // Set response timeout to prevent indefinite blocking (CHAOS-HIGH-005)
            conn.set_read_timeout(Some(timeout))
                .map_err(|e| Error::OperationFailed {
                    operation: "redis_set_read_timeout".to_string(),
                    cause: e.to_string(),
                })?;
            conn.set_write_timeout(Some(timeout))
                .map_err(|e| Error::OperationFailed {
                    operation: "redis_set_write_timeout".to_string(),
                    cause: e.to_string(),
                })?;

            Ok(conn)
        })
    }

    /// Returns a connection to the cache for reuse (DB-H6).
    fn return_connection(&self, conn: Connection) {
        if let Ok(mut guard) = self.connection.lock() {
            *guard = Some(conn);
        }
        // If lock fails, just drop the connection - not critical
    }

    /// Ensures the vector index exists in Redis.
    fn ensure_index(&self, conn: &mut Connection) -> Result<()> {
        // Check if already created (fast path)
        {
            let guard = self
                .index_created
                .lock()
                .map_err(|e| Error::OperationFailed {
                    operation: "redis_lock_index_created".to_string(),
                    cause: e.to_string(),
                })?;
            if *guard {
                return Ok(());
            }
        }

        // Check if index exists in Redis
        let info_result: RedisResult<redis::Value> =
            redis::cmd("FT.INFO").arg(&self.index_name).query(conn);

        if info_result.is_ok() {
            let mut guard = self
                .index_created
                .lock()
                .map_err(|e| Error::OperationFailed {
                    operation: "redis_lock_index_created".to_string(),
                    cause: e.to_string(),
                })?;
            *guard = true;
            return Ok(());
        }

        // Create the index with HNSW vector field
        let create_result: RedisResult<()> = redis::cmd("FT.CREATE")
            .arg(&self.index_name)
            .arg("ON")
            .arg("HASH")
            .arg("PREFIX")
            .arg("1")
            .arg(self.key_prefix())
            .arg("SCHEMA")
            .arg("embedding")
            .arg("VECTOR")
            .arg("HNSW")
            .arg("6")
            .arg("TYPE")
            .arg("FLOAT32")
            .arg("DIM")
            .arg(self.dimensions)
            .arg("DISTANCE_METRIC")
            .arg("COSINE")
            .arg("memory_id")
            .arg("TAG")
            .query(conn);

        match create_result {
            Ok(()) => {
                let mut guard = self
                    .index_created
                    .lock()
                    .map_err(|e| Error::OperationFailed {
                        operation: "redis_lock_index_created".to_string(),
                        cause: e.to_string(),
                    })?;
                *guard = true;
                Ok(())
            },
            Err(e) => {
                if e.to_string().contains("Index already exists") {
                    let mut guard =
                        self.index_created
                            .lock()
                            .map_err(|e| Error::OperationFailed {
                                operation: "redis_lock_index_created".to_string(),
                                cause: e.to_string(),
                            })?;
                    *guard = true;
                    Ok(())
                } else {
                    Err(Error::OperationFailed {
                        operation: "create_index".to_string(),
                        cause: e.to_string(),
                    })
                }
            },
        }
    }

    /// Parses FT.SEARCH results into memory IDs with scores.
    fn parse_search_results(value: &redis::Value) -> Vec<(MemoryId, f32)> {
        use redis::Value;

        let Value::Array(arr) = value else {
            return Vec::new();
        };

        if arr.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        let mut i = 1;

        while i + 1 < arr.len() {
            let Some(key) = Self::value_to_string(&arr[i]) else {
                i += 2;
                continue;
            };

            let memory_id = key.split(':').next_back().unwrap_or(&key);
            let score = Self::extract_score_from_fields(&arr[i + 1]);
            results.push((MemoryId::new(memory_id), score));
            i += 2;
        }

        results
    }

    /// Extracts the embedding score from a fields array.
    fn extract_score_from_fields(value: &redis::Value) -> f32 {
        use redis::Value;

        let Value::Array(fields) = value else {
            return 0.0;
        };

        let mut j = 0;
        while j + 1 < fields.len() {
            let field_name = Self::value_to_string(&fields[j]).unwrap_or_default();
            if field_name != "__embedding_score" {
                j += 2;
                continue;
            }
            let Some(s) = Self::value_to_string(&fields[j + 1]) else {
                j += 2;
                continue;
            };
            let Ok(distance) = s.parse::<f32>() else {
                j += 2;
                continue;
            };
            return 1.0 - distance.clamp(0.0, 2.0) / 2.0;
        }
        0.0
    }

    /// Parses FT.INFO response to extract `num_docs`.
    fn parse_info_num_docs(value: &redis::Value) -> usize {
        use redis::Value;

        let Value::Array(arr) = value else {
            return 0;
        };

        let mut i = 0;
        while i + 1 < arr.len() {
            let key = Self::value_to_string(&arr[i]).unwrap_or_default();
            if key != "num_docs" {
                i += 2;
                continue;
            }
            let Some(s) = Self::value_to_string(&arr[i + 1]) else {
                i += 2;
                continue;
            };
            return s.parse().unwrap_or(0);
        }
        0
    }

    /// Converts a Redis value to a string.
    fn value_to_string(value: &redis::Value) -> Option<String> {
        use redis::Value;

        match value {
            Value::BulkString(s) => Some(String::from_utf8_lossy(s).to_string()),
            Value::SimpleString(s) => Some(s.clone()),
            Value::Int(i) => Some(i.to_string()),
            _ => None,
        }
    }
}

#[cfg(feature = "redis")]
impl VectorBackend for RedisVectorBackend {
    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn upsert(&self, id: &MemoryId, embedding: &[f32]) -> Result<()> {
        self.validate_embedding(embedding)?;

        let mut conn = self.get_connection()?;

        let result = self.ensure_index(&mut conn);
        if let Err(e) = result {
            self.return_connection(conn);
            return Err(e);
        }

        let key = self.memory_key(id);
        let vector_bytes = Self::vector_to_bytes(embedding);

        let result: RedisResult<()> = conn.hset_multiple(
            &key,
            &[
                ("embedding", vector_bytes.as_slice()),
                ("memory_id", id.as_str().as_bytes()),
            ],
        );

        match result {
            Ok(()) => {
                self.return_connection(conn);
                Ok(())
            },
            Err(e) => {
                self.return_connection(conn);
                Err(Error::OperationFailed {
                    operation: "upsert".to_string(),
                    cause: e.to_string(),
                })
            },
        }
    }

    fn remove(&self, id: &MemoryId) -> Result<bool> {
        let mut conn = self.get_connection()?;
        let key = self.memory_key(id);

        let result: RedisResult<i32> = conn.del(&key);

        match result {
            Ok(deleted) => {
                self.return_connection(conn);
                Ok(deleted > 0)
            },
            Err(e) => {
                self.return_connection(conn);
                Err(Error::OperationFailed {
                    operation: "remove".to_string(),
                    cause: e.to_string(),
                })
            },
        }
    }

    fn search(
        &self,
        query_embedding: &[f32],
        _filter: &VectorFilter,
        limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>> {
        self.validate_embedding(query_embedding)?;

        let mut conn = self.get_connection()?;

        let vector_bytes = Self::vector_to_bytes(query_embedding);
        let query = format!("*=>[KNN {limit} @embedding $BLOB]");

        let result: RedisResult<redis::Value> = redis::cmd("FT.SEARCH")
            .arg(&self.index_name)
            .arg(&query)
            .arg("PARAMS")
            .arg("2")
            .arg("BLOB")
            .arg(vector_bytes.as_slice())
            .arg("RETURN")
            .arg("2")
            .arg("memory_id")
            .arg("__embedding_score")
            .arg("DIALECT")
            .arg("2")
            .query(&mut conn);

        match result {
            Ok(value) => {
                self.return_connection(conn);
                Ok(Self::parse_search_results(&value))
            },
            Err(e) => {
                self.return_connection(conn);
                Err(Error::OperationFailed {
                    operation: "search".to_string(),
                    cause: e.to_string(),
                })
            },
        }
    }

    fn count(&self) -> Result<usize> {
        let mut conn = self.get_connection()?;

        let result: RedisResult<redis::Value> =
            redis::cmd("FT.INFO").arg(&self.index_name).query(&mut conn);

        match result {
            Ok(info) => {
                self.return_connection(conn);
                Ok(Self::parse_info_num_docs(&info))
            },
            Err(e) => {
                self.return_connection(conn);
                if e.to_string().contains("Unknown index name") {
                    return Err(Error::OperationFailed {
                        operation: "count".to_string(),
                        cause: "index_not_found".to_string(),
                    });
                }
                Err(Error::OperationFailed {
                    operation: "count".to_string(),
                    cause: e.to_string(),
                })
            },
        }
    }

    fn clear(&self) -> Result<()> {
        let mut conn = self.get_connection()?;

        let _: RedisResult<()> = redis::cmd("FT.DROPINDEX")
            .arg(&self.index_name)
            .arg("DD")
            .query(&mut conn);

        // Reset index_created flag
        {
            let mut guard = self
                .index_created
                .lock()
                .map_err(|e| Error::OperationFailed {
                    operation: "redis_lock_index_created".to_string(),
                    cause: e.to_string(),
                })?;
            *guard = false;
        }

        self.return_connection(conn);
        Ok(())
    }
}

#[cfg(not(feature = "redis"))]
impl VectorBackend for RedisVectorBackend {
    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn upsert(&self, _id: &MemoryId, _embedding: &[f32]) -> Result<()> {
        Err(Error::NotImplemented(
            "Redis vector backend requires 'redis' feature".to_string(),
        ))
    }

    fn remove(&self, _id: &MemoryId) -> Result<bool> {
        Err(Error::NotImplemented(
            "Redis vector backend requires 'redis' feature".to_string(),
        ))
    }

    fn search(
        &self,
        _query_embedding: &[f32],
        _filter: &VectorFilter,
        _limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>> {
        Err(Error::NotImplemented(
            "Redis vector backend requires 'redis' feature".to_string(),
        ))
    }

    fn count(&self) -> Result<usize> {
        Err(Error::NotImplemented(
            "Redis vector backend requires 'redis' feature".to_string(),
        ))
    }

    fn clear(&self) -> Result<()> {
        Err(Error::NotImplemented(
            "Redis vector backend requires 'redis' feature".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(feature = "redis"))]
    #[test]
    fn test_redis_backend_creation() {
        let backend = RedisVectorBackend::new("redis://localhost:6379", "test_idx", 384);
        assert_eq!(backend.dimensions(), 384);
        assert_eq!(backend.connection_url(), "redis://localhost:6379");
        assert_eq!(backend.index_name(), "test_idx");
    }

    #[cfg(not(feature = "redis"))]
    #[test]
    fn test_redis_backend_defaults() {
        let backend = RedisVectorBackend::with_defaults();
        assert_eq!(backend.dimensions(), RedisVectorBackend::DEFAULT_DIMENSIONS);
        assert_eq!(backend.connection_url(), "redis://localhost:6379");
        assert_eq!(backend.index_name(), "subcog_vectors");
    }

    #[cfg(feature = "redis")]
    #[test]
    fn test_redis_backend_creation() {
        // This test requires a Redis server, so we only test construction
        // which can fail if Redis is not available
        let result = RedisVectorBackend::new("redis://localhost:6379", "test_idx", 384);
        // Don't assert success since Redis may not be running
        if let Ok(backend) = result {
            assert_eq!(backend.dimensions(), 384);
            assert_eq!(backend.connection_url(), "redis://localhost:6379");
            assert_eq!(backend.index_name(), "test_idx");
        }
    }

    #[cfg(feature = "redis")]
    #[test]
    fn test_key_generation() {
        if let Ok(backend) = RedisVectorBackend::new("redis://localhost", "idx", 384) {
            assert_eq!(backend.key_prefix(), "idx:");
            assert_eq!(backend.memory_key(&MemoryId::new("mem-001")), "idx:mem-001");
        }
    }

    #[cfg(feature = "redis")]
    #[test]
    fn test_validate_embedding() {
        if let Ok(backend) = RedisVectorBackend::new("redis://localhost", "idx", 384) {
            let valid: Vec<f32> = vec![0.0; 384];
            assert!(backend.validate_embedding(&valid).is_ok());

            let invalid: Vec<f32> = vec![0.0; 256];
            assert!(backend.validate_embedding(&invalid).is_err());
        }
    }

    #[cfg(not(feature = "redis"))]
    #[test]
    fn test_stub_returns_not_implemented() {
        let backend = RedisVectorBackend::with_defaults();
        let embedding: Vec<f32> = vec![0.0; 384];
        let id = MemoryId::new("test");

        assert!(backend.upsert(&id, &embedding).is_err());
        assert!(backend.remove(&id).is_err());
        assert!(
            backend
                .search(&embedding, &VectorFilter::new(), 10)
                .is_err()
        );
        assert!(backend.count().is_err());
        assert!(backend.clear().is_err());
    }
}
