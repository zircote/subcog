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
//! | M | 16 | Max outgoing edges per node |
//! | EF_CONSTRUCTION | 200 | Size of dynamic candidate list |
//! | EF_RUNTIME | 10 | Search-time candidate list size |
//! | DISTANCE_METRIC | COSINE | Similarity measure |

use crate::models::{MemoryId, SearchFilter};
use crate::storage::traits::VectorBackend;
use crate::{Error, Result};

#[cfg(feature = "redis")]
use redis::{Client, Commands, Connection, RedisResult};

/// Redis-based vector backend using `RediSearch` Vector Similarity Search.
///
/// This backend requires Redis Stack or Redis with the `RediSearch` 2.4+ module.
/// Vectors are stored as binary blobs in Redis hashes and indexed using HNSW.
pub struct RedisVectorBackend {
    /// Redis connection URL.
    connection_url: String,
    /// Index name in Redis.
    index_name: String,
    /// Embedding dimensions.
    dimensions: usize,
    /// Redis client (lazy initialized).
    #[cfg(feature = "redis")]
    client: Option<Client>,
    /// Whether the index has been created.
    #[cfg(feature = "redis")]
    index_created: bool,
}

impl RedisVectorBackend {
    /// Default embedding dimensions for all-MiniLM-L6-v2.
    pub const DEFAULT_DIMENSIONS: usize = 384;

    /// Creates a new Redis vector backend.
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
            #[cfg(feature = "redis")]
            client: None,
            #[cfg(feature = "redis")]
            index_created: false,
        }
    }

    /// Creates a backend with default settings.
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

    /// Gets or creates a Redis connection.
    fn get_connection(&mut self) -> Result<Connection> {
        if self.client.is_none() {
            let client =
                Client::open(self.connection_url.as_str()).map_err(|e| Error::OperationFailed {
                    operation: "redis_connect".to_string(),
                    cause: e.to_string(),
                })?;
            self.client = Some(client);
        }

        let client = self.client.as_ref().ok_or_else(|| Error::OperationFailed {
            operation: "redis_connect".to_string(),
            cause: "Failed to get Redis client".to_string(),
        })?;

        client.get_connection().map_err(|e| Error::OperationFailed {
            operation: "redis_connection".to_string(),
            cause: e.to_string(),
        })
    }

    /// Ensures the vector index exists in Redis.
    fn ensure_index(&mut self, conn: &mut Connection) -> Result<()> {
        if self.index_created {
            return Ok(());
        }

        // Check if index exists
        let info_result: RedisResult<redis::Value> =
            redis::cmd("FT.INFO").arg(&self.index_name).query(conn);

        if info_result.is_ok() {
            self.index_created = true;
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
                self.index_created = true;
                Ok(())
            },
            Err(e) => {
                if e.to_string().contains("Index already exists") {
                    self.index_created = true;
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

    fn upsert(&mut self, id: &MemoryId, embedding: &[f32]) -> Result<()> {
        self.validate_embedding(embedding)?;

        let mut conn = self.get_connection()?;
        self.ensure_index(&mut conn)?;

        let key = self.memory_key(id);
        let vector_bytes = Self::vector_to_bytes(embedding);

        conn.hset_multiple::<_, _, _, ()>(
            &key,
            &[
                ("embedding", vector_bytes.as_slice()),
                ("memory_id", id.as_str().as_bytes()),
            ],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "upsert".to_string(),
            cause: e.to_string(),
        })?;

        Ok(())
    }

    fn remove(&mut self, id: &MemoryId) -> Result<bool> {
        let mut conn = self.get_connection()?;
        let key = self.memory_key(id);

        let deleted: i32 = conn.del(&key).map_err(|e| Error::OperationFailed {
            operation: "remove".to_string(),
            cause: e.to_string(),
        })?;

        Ok(deleted > 0)
    }

    fn search(
        &self,
        query_embedding: &[f32],
        _filter: &SearchFilter,
        limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>> {
        self.validate_embedding(query_embedding)?;

        let client =
            Client::open(self.connection_url.as_str()).map_err(|e| Error::OperationFailed {
                operation: "redis_connect".to_string(),
                cause: e.to_string(),
            })?;

        let mut conn = client
            .get_connection()
            .map_err(|e| Error::OperationFailed {
                operation: "redis_connection".to_string(),
                cause: e.to_string(),
            })?;

        let vector_bytes = Self::vector_to_bytes(query_embedding);
        let query = format!("*=>[KNN {limit} @embedding $BLOB]");

        let result: redis::Value = redis::cmd("FT.SEARCH")
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
            .query(&mut conn)
            .map_err(|e| Error::OperationFailed {
                operation: "search".to_string(),
                cause: e.to_string(),
            })?;

        Ok(Self::parse_search_results(&result))
    }

    fn count(&self) -> Result<usize> {
        let client =
            Client::open(self.connection_url.as_str()).map_err(|e| Error::OperationFailed {
                operation: "redis_connect".to_string(),
                cause: e.to_string(),
            })?;

        let mut conn = client
            .get_connection()
            .map_err(|e| Error::OperationFailed {
                operation: "redis_connection".to_string(),
                cause: e.to_string(),
            })?;

        let info: redis::Value = redis::cmd("FT.INFO")
            .arg(&self.index_name)
            .query(&mut conn)
            .map_err(|e| {
                if e.to_string().contains("Unknown index name") {
                    return Error::OperationFailed {
                        operation: "count".to_string(),
                        cause: "index_not_found".to_string(),
                    };
                }
                Error::OperationFailed {
                    operation: "count".to_string(),
                    cause: e.to_string(),
                }
            })?;

        Ok(Self::parse_info_num_docs(&info))
    }

    fn clear(&mut self) -> Result<()> {
        let mut conn = self.get_connection()?;

        let _: RedisResult<()> = redis::cmd("FT.DROPINDEX")
            .arg(&self.index_name)
            .arg("DD")
            .query(&mut conn);

        self.index_created = false;
        Ok(())
    }
}

#[cfg(not(feature = "redis"))]
impl VectorBackend for RedisVectorBackend {
    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn upsert(&mut self, _id: &MemoryId, _embedding: &[f32]) -> Result<()> {
        Err(Error::NotImplemented(
            "Redis vector backend requires 'redis' feature".to_string(),
        ))
    }

    fn remove(&mut self, _id: &MemoryId) -> Result<bool> {
        Err(Error::NotImplemented(
            "Redis vector backend requires 'redis' feature".to_string(),
        ))
    }

    fn search(
        &self,
        _query_embedding: &[f32],
        _filter: &SearchFilter,
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

    fn clear(&mut self) -> Result<()> {
        Err(Error::NotImplemented(
            "Redis vector backend requires 'redis' feature".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_backend_creation() {
        let backend = RedisVectorBackend::new("redis://localhost:6379", "test_idx", 384);
        assert_eq!(backend.dimensions(), 384);
        assert_eq!(backend.connection_url(), "redis://localhost:6379");
        assert_eq!(backend.index_name(), "test_idx");
    }

    #[test]
    fn test_redis_backend_defaults() {
        let backend = RedisVectorBackend::with_defaults();
        assert_eq!(backend.dimensions(), RedisVectorBackend::DEFAULT_DIMENSIONS);
        assert_eq!(backend.connection_url(), "redis://localhost:6379");
        assert_eq!(backend.index_name(), "subcog_vectors");
    }

    #[cfg(feature = "redis")]
    #[test]
    fn test_key_generation() {
        let backend = RedisVectorBackend::new("redis://localhost", "idx", 384);
        assert_eq!(backend.key_prefix(), "idx:");
        assert_eq!(backend.memory_key(&MemoryId::new("mem-001")), "idx:mem-001");
    }

    #[cfg(feature = "redis")]
    #[test]
    fn test_validate_embedding() {
        let backend = RedisVectorBackend::new("redis://localhost", "idx", 384);

        let valid: Vec<f32> = vec![0.0; 384];
        assert!(backend.validate_embedding(&valid).is_ok());

        let invalid: Vec<f32> = vec![0.0; 256];
        assert!(backend.validate_embedding(&invalid).is_err());
    }

    #[cfg(not(feature = "redis"))]
    #[test]
    fn test_stub_returns_not_implemented() {
        let mut backend = RedisVectorBackend::with_defaults();
        let embedding: Vec<f32> = vec![0.0; 384];
        let id = MemoryId::new("test");

        assert!(backend.upsert(&id, &embedding).is_err());
        assert!(backend.remove(&id).is_err());
        assert!(
            backend
                .search(&embedding, &SearchFilter::new(), 10)
                .is_err()
        );
        assert!(backend.count().is_err());
        assert!(backend.clear().is_err());
    }
}
