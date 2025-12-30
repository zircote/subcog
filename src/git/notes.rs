//! Git notes CRUD operations.

use crate::{Error, Result};
use std::path::Path;

/// Manages git notes operations.
pub struct NotesManager {
    /// Path to the repository.
    repo_path: std::path::PathBuf,
    /// Notes ref to use.
    notes_ref: String,
}

impl NotesManager {
    /// Creates a new notes manager.
    #[must_use]
    pub fn new(repo_path: impl AsRef<Path>) -> Self {
        Self {
            repo_path: repo_path.as_ref().to_path_buf(),
            notes_ref: "refs/notes/subcog".to_string(),
        }
    }

    /// Sets a custom notes ref.
    #[must_use]
    pub fn with_notes_ref(mut self, notes_ref: impl Into<String>) -> Self {
        self.notes_ref = notes_ref.into();
        self
    }

    /// Adds a note to a commit.
    ///
    /// # Errors
    ///
    /// Returns an error if the note cannot be added.
    pub fn add(&self, _commit: &str, _content: &str) -> Result<()> {
        Err(Error::NotImplemented(format!(
            "NotesManager::add to {} in {}",
            self.notes_ref,
            self.repo_path.display()
        )))
    }

    /// Gets a note from a commit.
    ///
    /// # Errors
    ///
    /// Returns an error if the note cannot be retrieved.
    pub fn get(&self, _commit: &str) -> Result<Option<String>> {
        Err(Error::NotImplemented(format!(
            "NotesManager::get from {} in {}",
            self.notes_ref,
            self.repo_path.display()
        )))
    }

    /// Removes a note from a commit.
    ///
    /// # Errors
    ///
    /// Returns an error if the note cannot be removed.
    pub fn remove(&self, _commit: &str) -> Result<bool> {
        Err(Error::NotImplemented(format!(
            "NotesManager::remove from {} in {}",
            self.notes_ref,
            self.repo_path.display()
        )))
    }

    /// Lists all notes.
    ///
    /// # Errors
    ///
    /// Returns an error if notes cannot be listed.
    pub fn list(&self) -> Result<Vec<(String, String)>> {
        Err(Error::NotImplemented(format!(
            "NotesManager::list from {} in {}",
            self.notes_ref,
            self.repo_path.display()
        )))
    }
}
