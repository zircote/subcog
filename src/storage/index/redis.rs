//! Redis-based index backend using `RediSearch`.
//!
//! Provides full-text search using Redis with the `RediSearch` module.

use crate::models::{Memory, MemoryId, SearchFilter};
use crate::storage::traits::IndexBackend;
use crate::{Error, Result};

/// Redis-based index backend using `RediSearch`.
pub struct RedisBackend {
    /// Redis connection URL.
    connection_url: String,
    /// Index name in Redis.
    index_name: String,
}

impl RedisBackend {
    /// Creates a new Redis backend.
    #[must_use]
    pub fn new(connection_url: impl Into<String>, index_name: impl Into<String>) -> Self {
        Self {
            connection_url: connection_url.into(),
            index_name: index_name.into(),
        }
    }

    /// Creates a backend with default settings.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new("redis://localhost:6379", "subcog_memories")
    }
}

impl IndexBackend for RedisBackend {
    fn index(&mut self, _memory: &Memory) -> Result<()> {
        Err(Error::NotImplemented(format!(
            "RedisBackend::index for {} on {}",
            self.index_name, self.connection_url
        )))
    }

    fn remove(&mut self, _id: &MemoryId) -> Result<bool> {
        Err(Error::NotImplemented(format!(
            "RedisBackend::remove for {} on {}",
            self.index_name, self.connection_url
        )))
    }

    fn search(
        &self,
        _query: &str,
        _filter: &SearchFilter,
        _limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>> {
        Err(Error::NotImplemented(format!(
            "RedisBackend::search for {} on {}",
            self.index_name, self.connection_url
        )))
    }

    fn clear(&mut self) -> Result<()> {
        Err(Error::NotImplemented(format!(
            "RedisBackend::clear for {} on {}",
            self.index_name, self.connection_url
        )))
    }
}
