//! Persistence backend trait.

use crate::models::{Memory, MemoryId};
use crate::Result;

/// Trait for persistence layer backends.
///
/// Persistence backends are the authoritative source of truth for memories.
/// They handle long-term storage and retrieval.
pub trait PersistenceBackend: Send + Sync {
    /// Stores a memory.
    fn store(&mut self, memory: &Memory) -> Result<()>;

    /// Retrieves a memory by ID.
    fn get(&self, id: &MemoryId) -> Result<Option<Memory>>;

    /// Deletes a memory by ID.
    fn delete(&mut self, id: &MemoryId) -> Result<bool>;

    /// Lists all memory IDs.
    fn list_ids(&self) -> Result<Vec<MemoryId>>;

    /// Checks if a memory exists.
    fn exists(&self, id: &MemoryId) -> Result<bool> {
        Ok(self.get(id)?.is_some())
    }

    /// Returns the total count of memories.
    fn count(&self) -> Result<usize> {
        Ok(self.list_ids()?.len())
    }
}
