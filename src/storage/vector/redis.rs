//! Redis-based vector backend.
//!
//! Provides vector similarity search using Redis with vector search module.

use crate::models::{MemoryId, SearchFilter};
use crate::storage::traits::VectorBackend;
use crate::{Error, Result};

/// Redis-based vector backend.
pub struct RedisVectorBackend {
    /// Redis connection URL.
    connection_url: String,
    /// Index name in Redis.
    index_name: String,
    /// Embedding dimensions.
    dimensions: usize,
}

impl RedisVectorBackend {
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
        }
    }

    /// Creates a backend with default settings.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new("redis://localhost:6379", "subcog_vectors", 384)
    }

    /// Default embedding dimensions for all-MiniLM-L6-v2.
    pub const DEFAULT_DIMENSIONS: usize = 384;
}

impl VectorBackend for RedisVectorBackend {
    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn upsert(&mut self, _id: &MemoryId, _embedding: &[f32]) -> Result<()> {
        Err(Error::NotImplemented(format!(
            "RedisVectorBackend::upsert for {} on {}",
            self.index_name, self.connection_url
        )))
    }

    fn remove(&mut self, _id: &MemoryId) -> Result<bool> {
        Err(Error::NotImplemented(format!(
            "RedisVectorBackend::remove for {} on {}",
            self.index_name, self.connection_url
        )))
    }

    fn search(
        &self,
        _query_embedding: &[f32],
        _filter: &SearchFilter,
        _limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>> {
        Err(Error::NotImplemented(format!(
            "RedisVectorBackend::search for {} on {}",
            self.index_name, self.connection_url
        )))
    }

    fn count(&self) -> Result<usize> {
        Err(Error::NotImplemented(format!(
            "RedisVectorBackend::count for {} on {}",
            self.index_name, self.connection_url
        )))
    }

    fn clear(&mut self) -> Result<()> {
        Err(Error::NotImplemented(format!(
            "RedisVectorBackend::clear for {} on {}",
            self.index_name, self.connection_url
        )))
    }
}
