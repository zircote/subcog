//! Filesystem-based persistence backend.
//!
//! A fallback backend that stores memories as individual files.
//! Useful for testing and environments without git.

use crate::models::{Memory, MemoryId};
use crate::storage::traits::PersistenceBackend;
use crate::{Error, Result};

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
    fn store(&mut self, memory: &Memory) -> Result<()> {
        let path = self.memory_path(&memory.id);
        Err(Error::NotImplemented(format!(
            "FilesystemBackend::store to {}",
            path.display()
        )))
    }

    fn get(&self, id: &MemoryId) -> Result<Option<Memory>> {
        let path = self.memory_path(id);
        Err(Error::NotImplemented(format!(
            "FilesystemBackend::get from {}",
            path.display()
        )))
    }

    fn delete(&mut self, id: &MemoryId) -> Result<bool> {
        let path = self.memory_path(id);
        Err(Error::NotImplemented(format!(
            "FilesystemBackend::delete at {}",
            path.display()
        )))
    }

    fn list_ids(&self) -> Result<Vec<MemoryId>> {
        Err(Error::NotImplemented(format!(
            "FilesystemBackend::list_ids in {}",
            self.base_path.display()
        )))
    }
}
