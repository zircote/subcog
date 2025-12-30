//! usearch HNSW vector backend.
//!
//! Provides high-performance approximate nearest neighbor search.

use crate::models::{MemoryId, SearchFilter};
use crate::storage::traits::VectorBackend;
use crate::{Error, Result};

/// usearch-based vector backend.
pub struct UsearchBackend {
    /// Path to the index file.
    index_path: std::path::PathBuf,
    /// Embedding dimensions.
    dimensions: usize,
}

impl UsearchBackend {
    /// Creates a new usearch backend.
    #[must_use]
    pub fn new(index_path: impl Into<std::path::PathBuf>, dimensions: usize) -> Self {
        Self {
            index_path: index_path.into(),
            dimensions,
        }
    }

    /// Default embedding dimensions for all-MiniLM-L6-v2.
    pub const DEFAULT_DIMENSIONS: usize = 384;
}

impl VectorBackend for UsearchBackend {
    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn upsert(&mut self, _id: &MemoryId, _embedding: &[f32]) -> Result<()> {
        Err(Error::NotImplemented(format!(
            "UsearchBackend::upsert for {}",
            self.index_path.display()
        )))
    }

    fn remove(&mut self, _id: &MemoryId) -> Result<bool> {
        Err(Error::NotImplemented(format!(
            "UsearchBackend::remove for {}",
            self.index_path.display()
        )))
    }

    fn search(
        &self,
        _query_embedding: &[f32],
        _filter: &SearchFilter,
        _limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>> {
        Err(Error::NotImplemented(format!(
            "UsearchBackend::search for {}",
            self.index_path.display()
        )))
    }

    fn count(&self) -> Result<usize> {
        Err(Error::NotImplemented(format!(
            "UsearchBackend::count for {}",
            self.index_path.display()
        )))
    }

    fn clear(&mut self) -> Result<()> {
        Err(Error::NotImplemented(format!(
            "UsearchBackend::clear for {}",
            self.index_path.display()
        )))
    }
}
