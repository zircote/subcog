//! usearch HNSW vector backend.
//!
//! Provides high-performance approximate nearest neighbor search.

use crate::models::{MemoryId, SearchFilter};
use crate::storage::traits::VectorBackend;
use crate::Result;

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
        // TODO: Implement usearch upsert
        todo!("UsearchBackend::upsert not yet implemented")
    }

    fn remove(&mut self, _id: &MemoryId) -> Result<bool> {
        // TODO: Implement usearch removal
        todo!("UsearchBackend::remove not yet implemented")
    }

    fn search(
        &self,
        _query_embedding: &[f32],
        _filter: &SearchFilter,
        _limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>> {
        // TODO: Implement usearch search
        todo!("UsearchBackend::search not yet implemented")
    }

    fn count(&self) -> Result<usize> {
        // TODO: Implement usearch count
        todo!("UsearchBackend::count not yet implemented")
    }

    fn clear(&mut self) -> Result<()> {
        // TODO: Implement usearch clear
        todo!("UsearchBackend::clear not yet implemented")
    }
}
