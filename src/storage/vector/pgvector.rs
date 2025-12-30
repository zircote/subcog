//! pgvector-based vector backend.
//!
//! Provides vector similarity search using PostgreSQL with pgvector extension.

use crate::models::{MemoryId, SearchFilter};
use crate::storage::traits::VectorBackend;
use crate::{Error, Result};

/// pgvector-based vector backend.
pub struct PgvectorBackend {
    /// PostgreSQL connection URL.
    connection_url: String,
    /// Table name for vectors.
    table_name: String,
    /// Embedding dimensions.
    dimensions: usize,
}

impl PgvectorBackend {
    /// Creates a new pgvector backend.
    #[must_use]
    pub fn new(
        connection_url: impl Into<String>,
        table_name: impl Into<String>,
        dimensions: usize,
    ) -> Self {
        Self {
            connection_url: connection_url.into(),
            table_name: table_name.into(),
            dimensions,
        }
    }

    /// Creates a backend with default settings.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new("postgresql://localhost/subcog", "memory_vectors", 384)
    }

    /// Default embedding dimensions for all-MiniLM-L6-v2.
    pub const DEFAULT_DIMENSIONS: usize = 384;
}

impl VectorBackend for PgvectorBackend {
    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn upsert(&mut self, _id: &MemoryId, _embedding: &[f32]) -> Result<()> {
        Err(Error::NotImplemented(format!(
            "PgvectorBackend::upsert for {} on {}",
            self.table_name, self.connection_url
        )))
    }

    fn remove(&mut self, _id: &MemoryId) -> Result<bool> {
        Err(Error::NotImplemented(format!(
            "PgvectorBackend::remove for {} on {}",
            self.table_name, self.connection_url
        )))
    }

    fn search(
        &self,
        _query_embedding: &[f32],
        _filter: &SearchFilter,
        _limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>> {
        Err(Error::NotImplemented(format!(
            "PgvectorBackend::search for {} on {}",
            self.table_name, self.connection_url
        )))
    }

    fn count(&self) -> Result<usize> {
        Err(Error::NotImplemented(format!(
            "PgvectorBackend::count for {} on {}",
            self.table_name, self.connection_url
        )))
    }

    fn clear(&mut self) -> Result<()> {
        Err(Error::NotImplemented(format!(
            "PgvectorBackend::clear for {} on {}",
            self.table_name, self.connection_url
        )))
    }
}
