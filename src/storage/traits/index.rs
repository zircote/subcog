//! Index backend trait.

use crate::models::{Memory, MemoryId, SearchFilter};
use crate::Result;

/// Trait for index layer backends.
///
/// Index backends provide full-text search capabilities using BM25 or similar algorithms.
pub trait IndexBackend: Send + Sync {
    /// Indexes a memory for full-text search.
    fn index(&mut self, memory: &Memory) -> Result<()>;

    /// Removes a memory from the index.
    fn remove(&mut self, id: &MemoryId) -> Result<bool>;

    /// Searches for memories matching a text query.
    ///
    /// Returns memory IDs with their BM25 scores, ordered by relevance.
    fn search(&self, query: &str, filter: &SearchFilter, limit: usize)
        -> Result<Vec<(MemoryId, f32)>>;

    /// Re-indexes all memories.
    fn reindex(&mut self, memories: &[Memory]) -> Result<()> {
        for memory in memories {
            self.index(memory)?;
        }
        Ok(())
    }

    /// Clears the entire index.
    fn clear(&mut self) -> Result<()>;
}
