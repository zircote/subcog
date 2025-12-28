//! Git remote operations.

use crate::Result;
use std::path::Path;

/// Manages git remote operations for notes.
pub struct RemoteManager {
    /// Path to the repository.
    repo_path: std::path::PathBuf,
}

impl RemoteManager {
    /// Creates a new remote manager.
    #[must_use]
    pub fn new(repo_path: impl AsRef<Path>) -> Self {
        Self {
            repo_path: repo_path.as_ref().to_path_buf(),
        }
    }

    /// Fetches notes from a remote.
    ///
    /// # Errors
    ///
    /// Returns an error if the fetch fails.
    pub fn fetch(&self, _remote: &str) -> Result<usize> {
        // TODO: Implement git fetch for notes
        todo!("RemoteManager::fetch not yet implemented")
    }

    /// Pushes notes to a remote.
    ///
    /// # Errors
    ///
    /// Returns an error if the push fails.
    pub fn push(&self, _remote: &str) -> Result<usize> {
        // TODO: Implement git push for notes
        todo!("RemoteManager::push not yet implemented")
    }
}
