//! SQLite + FTS5 index backend.
//!
//! Provides full-text search using SQLite's FTS5 extension.

use crate::models::{Memory, MemoryId, SearchFilter};
use crate::storage::traits::IndexBackend;
use crate::{Error, Result};

/// SQLite-based index backend with FTS5.
pub struct SqliteBackend {
    /// Path to the SQLite database.
    db_path: std::path::PathBuf,
}

impl SqliteBackend {
    /// Creates a new SQLite backend.
    #[must_use]
    pub fn new(db_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            db_path: db_path.into(),
        }
    }

    /// Creates an in-memory SQLite backend (useful for testing).
    #[must_use]
    pub fn in_memory() -> Self {
        Self {
            db_path: std::path::PathBuf::from(":memory:"),
        }
    }
}

impl IndexBackend for SqliteBackend {
    fn index(&mut self, _memory: &Memory) -> Result<()> {
        Err(Error::NotImplemented(format!(
            "SqliteBackend::index for {}",
            self.db_path.display()
        )))
    }

    fn remove(&mut self, _id: &MemoryId) -> Result<bool> {
        Err(Error::NotImplemented(format!(
            "SqliteBackend::remove for {}",
            self.db_path.display()
        )))
    }

    fn search(
        &self,
        _query: &str,
        _filter: &SearchFilter,
        _limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>> {
        Err(Error::NotImplemented(format!(
            "SqliteBackend::search for {}",
            self.db_path.display()
        )))
    }

    fn clear(&mut self) -> Result<()> {
        Err(Error::NotImplemented(format!(
            "SqliteBackend::clear for {}",
            self.db_path.display()
        )))
    }
}
