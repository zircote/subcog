//! Git notes persistence backend.
//!
//! This is the primary persistence backend for subcog.
//! Memories are stored as git notes attached to a dedicated ref.

use crate::models::{Memory, MemoryId};
use crate::storage::traits::PersistenceBackend;
use crate::{Error, Result};

/// Git notes-based persistence backend.
pub struct GitNotesBackend {
    /// Path to the git repository.
    repo_path: std::path::PathBuf,
    /// Git notes ref (e.g., "refs/notes/subcog").
    notes_ref: String,
}

impl GitNotesBackend {
    /// Creates a new git notes backend.
    #[must_use]
    pub fn new(repo_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
            notes_ref: "refs/notes/subcog".to_string(),
        }
    }

    /// Sets a custom notes ref.
    #[must_use]
    pub fn with_notes_ref(mut self, notes_ref: impl Into<String>) -> Self {
        self.notes_ref = notes_ref.into();
        self
    }
}

impl PersistenceBackend for GitNotesBackend {
    fn store(&mut self, _memory: &Memory) -> Result<()> {
        Err(Error::NotImplemented(format!(
            "GitNotesBackend::store to {} in {}",
            self.notes_ref,
            self.repo_path.display()
        )))
    }

    fn get(&self, _id: &MemoryId) -> Result<Option<Memory>> {
        Err(Error::NotImplemented(format!(
            "GitNotesBackend::get from {} in {}",
            self.notes_ref,
            self.repo_path.display()
        )))
    }

    fn delete(&mut self, _id: &MemoryId) -> Result<bool> {
        Err(Error::NotImplemented(format!(
            "GitNotesBackend::delete from {} in {}",
            self.notes_ref,
            self.repo_path.display()
        )))
    }

    fn list_ids(&self) -> Result<Vec<MemoryId>> {
        Err(Error::NotImplemented(format!(
            "GitNotesBackend::list_ids from {} in {}",
            self.notes_ref,
            self.repo_path.display()
        )))
    }
}
