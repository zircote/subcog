//! PostgreSQL-based index backend.
//!
//! Provides full-text search using PostgreSQL's built-in tsvector/tsquery.

use crate::models::{Memory, MemoryId, SearchFilter};
use crate::storage::traits::IndexBackend;
use crate::{Error, Result};

/// PostgreSQL-based index backend.
pub struct PostgresIndexBackend {
    /// PostgreSQL connection URL.
    connection_url: String,
    /// Table name for memories.
    table_name: String,
}

impl PostgresIndexBackend {
    /// Creates a new PostgreSQL index backend.
    #[must_use]
    pub fn new(connection_url: impl Into<String>, table_name: impl Into<String>) -> Self {
        Self {
            connection_url: connection_url.into(),
            table_name: table_name.into(),
        }
    }

    /// Creates a backend with default settings.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new("postgresql://localhost/subcog", "memories")
    }
}

impl IndexBackend for PostgresIndexBackend {
    fn index(&mut self, _memory: &Memory) -> Result<()> {
        Err(Error::NotImplemented(format!(
            "PostgresIndexBackend::index for {} on {}",
            self.table_name, self.connection_url
        )))
    }

    fn remove(&mut self, _id: &MemoryId) -> Result<bool> {
        Err(Error::NotImplemented(format!(
            "PostgresIndexBackend::remove for {} on {}",
            self.table_name, self.connection_url
        )))
    }

    fn search(
        &self,
        _query: &str,
        _filter: &SearchFilter,
        _limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>> {
        Err(Error::NotImplemented(format!(
            "PostgresIndexBackend::search for {} on {}",
            self.table_name, self.connection_url
        )))
    }

    fn clear(&mut self) -> Result<()> {
        Err(Error::NotImplemented(format!(
            "PostgresIndexBackend::clear for {} on {}",
            self.table_name, self.connection_url
        )))
    }
}
