//! PostgreSQL-based persistence backend.
//!
//! Provides reliable persistence using PostgreSQL as the storage backend.

use crate::models::{Memory, MemoryId};
use crate::storage::traits::PersistenceBackend;
use crate::{Error, Result};

/// PostgreSQL-based persistence backend.
pub struct PostgresBackend {
    /// PostgreSQL connection URL.
    connection_url: String,
    /// Table name for memories.
    table_name: String,
}

impl PostgresBackend {
    /// Creates a new PostgreSQL backend.
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

impl PersistenceBackend for PostgresBackend {
    fn store(&mut self, _memory: &Memory) -> Result<()> {
        Err(Error::NotImplemented(format!(
            "PostgresBackend::store to {} on {}",
            self.table_name, self.connection_url
        )))
    }

    fn get(&self, _id: &MemoryId) -> Result<Option<Memory>> {
        Err(Error::NotImplemented(format!(
            "PostgresBackend::get from {} on {}",
            self.table_name, self.connection_url
        )))
    }

    fn delete(&mut self, _id: &MemoryId) -> Result<bool> {
        Err(Error::NotImplemented(format!(
            "PostgresBackend::delete from {} on {}",
            self.table_name, self.connection_url
        )))
    }

    fn list_ids(&self) -> Result<Vec<MemoryId>> {
        Err(Error::NotImplemented(format!(
            "PostgresBackend::list_ids from {} on {}",
            self.table_name, self.connection_url
        )))
    }
}
