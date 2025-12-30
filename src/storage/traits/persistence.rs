//! Persistence backend trait.

use crate::Result;
use crate::models::{Memory, MemoryId};

/// Trait for persistence layer backends.
///
/// Persistence backends are the authoritative source of truth for memories.
/// They handle long-term storage and retrieval.
pub trait PersistenceBackend: Send + Sync {
    /// Stores a memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage operation fails.
    fn store(&mut self, memory: &Memory) -> Result<()>;

    /// Retrieves a memory by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the retrieval operation fails.
    fn get(&self, id: &MemoryId) -> Result<Option<Memory>>;

    /// Deletes a memory by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion operation fails.
    fn delete(&mut self, id: &MemoryId) -> Result<bool>;

    /// Lists all memory IDs.
    ///
    /// # Errors
    ///
    /// Returns an error if the list operation fails.
    fn list_ids(&self) -> Result<Vec<MemoryId>>;

    /// Checks if a memory exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the existence check fails.
    fn exists(&self, id: &MemoryId) -> Result<bool> {
        Ok(self.get(id)?.is_some())
    }

    /// Returns the total count of memories.
    ///
    /// # Errors
    ///
    /// Returns an error if the count operation fails.
    fn count(&self) -> Result<usize> {
        Ok(self.list_ids()?.len())
    }
}
