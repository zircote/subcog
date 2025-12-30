//! Index backend trait.

use crate::Result;
use crate::models::{Memory, MemoryId, SearchFilter};

/// Trait for index layer backends.
///
/// Index backends provide full-text search capabilities using BM25 or similar algorithms.
pub trait IndexBackend: Send + Sync {
    /// Indexes a memory for full-text search.
    ///
    /// # Errors
    ///
    /// Returns an error if the indexing operation fails.
    fn index(&mut self, memory: &Memory) -> Result<()>;

    /// Removes a memory from the index.
    ///
    /// # Errors
    ///
    /// Returns an error if the removal operation fails.
    fn remove(&mut self, id: &MemoryId) -> Result<bool>;

    /// Searches for memories matching a text query.
    ///
    /// Returns memory IDs with their BM25 scores, ordered by relevance.
    ///
    /// # Errors
    ///
    /// Returns an error if the search operation fails.
    fn search(
        &self,
        query: &str,
        filter: &SearchFilter,
        limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>>;

    /// Re-indexes all memories.
    ///
    /// # Errors
    ///
    /// Returns an error if any memory fails to index.
    fn reindex(&mut self, memories: &[Memory]) -> Result<()> {
        for memory in memories {
            self.index(memory)?;
        }
        Ok(())
    }

    /// Clears the entire index.
    ///
    /// # Errors
    ///
    /// Returns an error if the clear operation fails.
    fn clear(&mut self) -> Result<()>;
}
