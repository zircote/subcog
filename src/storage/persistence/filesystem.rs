//! Filesystem-based persistence backend.
//!
//! A fallback backend that stores memories as individual files.
//! Useful for testing and environments without git.

use crate::models::{Memory, MemoryId};
use crate::storage::traits::PersistenceBackend;
use crate::Result;

/// Filesystem-based persistence backend.
pub struct FilesystemBackend {
    /// Base directory for storage.
    base_path: std::path::PathBuf,
}

impl FilesystemBackend {
    /// Creates a new filesystem backend.
    #[must_use]
    pub fn new(base_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// Returns the path for a memory file.
    fn memory_path(&self, id: &MemoryId) -> std::path::PathBuf {
        self.base_path.join(format!("{}.json", id.as_str()))
    }
}

impl PersistenceBackend for FilesystemBackend {
    fn store(&mut self, _memory: &Memory) -> Result<()> {
        // TODO: Implement filesystem storage
        todo!("FilesystemBackend::store not yet implemented")
    }

    fn get(&self, _id: &MemoryId) -> Result<Option<Memory>> {
        // TODO: Implement filesystem retrieval
        todo!("FilesystemBackend::get not yet implemented")
    }

    fn delete(&mut self, _id: &MemoryId) -> Result<bool> {
        // TODO: Implement filesystem deletion
        todo!("FilesystemBackend::delete not yet implemented")
    }

    fn list_ids(&self) -> Result<Vec<MemoryId>> {
        // TODO: Implement filesystem listing
        todo!("FilesystemBackend::list_ids not yet implemented")
    }
}
