//! Vector backend trait.

use crate::Result;
use crate::models::{MemoryId, SearchFilter};

/// Trait for vector layer backends.
///
/// Vector backends provide similarity search using embedding vectors.
pub trait VectorBackend: Send + Sync {
    /// The dimensionality of embedding vectors.
    fn dimensions(&self) -> usize;

    /// Inserts or updates an embedding for a memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the upsert operation fails.
    fn upsert(&mut self, id: &MemoryId, embedding: &[f32]) -> Result<()>;

    /// Removes an embedding by memory ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the removal operation fails.
    fn remove(&mut self, id: &MemoryId) -> Result<bool>;

    /// Searches for similar embeddings.
    ///
    /// Returns memory IDs with their cosine similarity scores (0.0 to 1.0),
    /// ordered by descending similarity.
    ///
    /// # Errors
    ///
    /// Returns an error if the search operation fails.
    fn search(
        &self,
        query_embedding: &[f32],
        filter: &SearchFilter,
        limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>>;

    /// Returns the total count of indexed embeddings.
    ///
    /// # Errors
    ///
    /// Returns an error if the count operation fails.
    fn count(&self) -> Result<usize>;

    /// Clears all embeddings.
    ///
    /// # Errors
    ///
    /// Returns an error if the clear operation fails.
    fn clear(&mut self) -> Result<()>;
}
