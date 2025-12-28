//! Vector backend trait.

use crate::models::{MemoryId, SearchFilter};
use crate::Result;

/// Trait for vector layer backends.
///
/// Vector backends provide similarity search using embedding vectors.
pub trait VectorBackend: Send + Sync {
    /// The dimensionality of embedding vectors.
    fn dimensions(&self) -> usize;

    /// Inserts or updates an embedding for a memory.
    fn upsert(&mut self, id: &MemoryId, embedding: &[f32]) -> Result<()>;

    /// Removes an embedding by memory ID.
    fn remove(&mut self, id: &MemoryId) -> Result<bool>;

    /// Searches for similar embeddings.
    ///
    /// Returns memory IDs with their cosine similarity scores (0.0 to 1.0),
    /// ordered by descending similarity.
    fn search(
        &self,
        query_embedding: &[f32],
        filter: &SearchFilter,
        limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>>;

    /// Returns the total count of indexed embeddings.
    fn count(&self) -> Result<usize>;

    /// Clears all embeddings.
    fn clear(&mut self) -> Result<()>;
}
