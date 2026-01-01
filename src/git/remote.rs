//! Git remote operations.
//!
//! Handles fetching and pushing notes to/from remote repositories.

use crate::{Error, Result};
use git2::{FetchOptions, PushOptions, RemoteCallbacks, Repository};
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Default timeout for git remote operations (30 seconds).
const DEFAULT_REMOTE_TIMEOUT: Duration = Duration::from_secs(30);

/// Manages git remote operations for notes.
pub struct RemoteManager {
    /// Path to the repository.
    repo_path: std::path::PathBuf,
    /// The notes ref to sync.
    notes_ref: String,
    /// Timeout for remote operations.
    timeout: Duration,
}

impl RemoteManager {
    /// Default notes ref for subcog.
    pub const DEFAULT_NOTES_REF: &'static str = "refs/notes/subcog";

    /// Creates a new remote manager.
    #[must_use]
    pub fn new(repo_path: impl AsRef<Path>) -> Self {
        Self {
            repo_path: repo_path.as_ref().to_path_buf(),
            notes_ref: Self::DEFAULT_NOTES_REF.to_string(),
            timeout: DEFAULT_REMOTE_TIMEOUT,
        }
    }

    /// Sets a custom notes ref.
    #[must_use]
    pub fn with_notes_ref(mut self, notes_ref: impl Into<String>) -> Self {
        self.notes_ref = notes_ref.into();
        self
    }

    /// Sets a custom timeout for remote operations.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Opens the git repository.
    fn open_repo(&self) -> Result<Repository> {
        Repository::open(&self.repo_path).map_err(|e| Error::OperationFailed {
            operation: "open_repository".to_string(),
            cause: e.to_string(),
        })
    }

    /// Creates default remote callbacks for credential handling.
    fn create_callbacks<'a>() -> RemoteCallbacks<'a> {
        let mut callbacks = RemoteCallbacks::new();

        // Try to use SSH agent for authentication
        callbacks.credentials(Self::handle_credentials);

        callbacks
    }

    /// Handles credential requests for git operations.
    fn handle_credentials(
        _url: &str,
        username_from_url: Option<&str>,
        allowed_types: git2::CredentialType,
    ) -> std::result::Result<git2::Cred, git2::Error> {
        // Try SSH agent with username if available
        if allowed_types.contains(git2::CredentialType::SSH_KEY) {
            if let Some(username) = username_from_url {
                return git2::Cred::ssh_key_from_agent(username);
            }
        }

        if allowed_types.contains(git2::CredentialType::DEFAULT) {
            return git2::Cred::default();
        }

        Err(git2::Error::from_str("No suitable credentials found"))
    }

    /// Fetches notes from a remote with timeout protection.
    ///
    /// # Errors
    ///
    /// Returns an error if the fetch fails or times out.
    pub fn fetch(&self, remote_name: &str) -> Result<usize> {
        let repo_path = self.repo_path.clone();
        let notes_ref = self.notes_ref.clone();
        let remote_name = remote_name.to_string();
        let timeout = self.timeout;

        // Run fetch in a separate thread with timeout
        let (tx, rx) = mpsc::channel();

        let handle = thread::spawn(move || {
            let result = Self::fetch_inner(&repo_path, &remote_name, &notes_ref);
            let _ = tx.send(result);
        });

        match rx.recv_timeout(timeout) {
            Ok(result) => {
                // Wait for thread to finish (should be immediate)
                let _ = handle.join();
                result
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Thread is still running but we timed out
                // Note: We can't forcibly kill the thread, but we return an error
                tracing::warn!(
                    "Git fetch timed out after {:?}, operation may still be running",
                    timeout
                );
                Err(Error::OperationFailed {
                    operation: "fetch_notes".to_string(),
                    cause: format!("Operation timed out after {timeout:?}"),
                })
            },
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(Error::OperationFailed {
                operation: "fetch_notes".to_string(),
                cause: "Fetch thread panicked".to_string(),
            }),
        }
    }

    /// Inner fetch implementation (runs in separate thread).
    fn fetch_inner(
        repo_path: &std::path::Path,
        remote_name: &str,
        notes_ref: &str,
    ) -> Result<usize> {
        let repo = Repository::open(repo_path).map_err(|e| Error::OperationFailed {
            operation: "open_repository".to_string(),
            cause: e.to_string(),
        })?;

        let mut remote = repo
            .find_remote(remote_name)
            .map_err(|e| Error::OperationFailed {
                operation: "find_remote".to_string(),
                cause: e.to_string(),
            })?;

        let callbacks = Self::create_callbacks();
        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        // Fetch the notes ref
        let refspec = format!("+{notes_ref}:{notes_ref}");
        remote
            .fetch(&[&refspec], Some(&mut fetch_options), None)
            .map_err(|e| Error::OperationFailed {
                operation: "fetch_notes".to_string(),
                cause: e.to_string(),
            })?;

        // Get fetch stats
        let stats = remote.stats();
        Ok(stats.received_objects())
    }

    /// Pushes notes to a remote with timeout protection.
    ///
    /// # Errors
    ///
    /// Returns an error if the push fails or times out.
    pub fn push(&self, remote_name: &str) -> Result<usize> {
        let repo_path = self.repo_path.clone();
        let notes_ref = self.notes_ref.clone();
        let remote_name = remote_name.to_string();
        let timeout = self.timeout;

        // Run push in a separate thread with timeout
        let (tx, rx) = mpsc::channel();

        let handle = thread::spawn(move || {
            let result = Self::push_inner(&repo_path, &remote_name, &notes_ref);
            let _ = tx.send(result);
        });

        match rx.recv_timeout(timeout) {
            Ok(result) => {
                // Wait for thread to finish (should be immediate)
                let _ = handle.join();
                result
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Thread is still running but we timed out
                tracing::warn!(
                    "Git push timed out after {:?}, operation may still be running",
                    timeout
                );
                Err(Error::OperationFailed {
                    operation: "push_notes".to_string(),
                    cause: format!("Operation timed out after {timeout:?}"),
                })
            },
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(Error::OperationFailed {
                operation: "push_notes".to_string(),
                cause: "Push thread panicked".to_string(),
            }),
        }
    }

    /// Inner push implementation (runs in separate thread).
    fn push_inner(
        repo_path: &std::path::Path,
        remote_name: &str,
        notes_ref: &str,
    ) -> Result<usize> {
        let repo = Repository::open(repo_path).map_err(|e| Error::OperationFailed {
            operation: "open_repository".to_string(),
            cause: e.to_string(),
        })?;

        let mut remote = repo
            .find_remote(remote_name)
            .map_err(|e| Error::OperationFailed {
                operation: "find_remote".to_string(),
                cause: e.to_string(),
            })?;

        let callbacks = Self::create_callbacks();
        let mut push_options = PushOptions::new();
        push_options.remote_callbacks(callbacks);

        // Push the notes ref
        let refspec = format!("{notes_ref}:{notes_ref}");
        remote
            .push(&[&refspec], Some(&mut push_options))
            .map_err(|e| Error::OperationFailed {
                operation: "push_notes".to_string(),
                cause: e.to_string(),
            })?;

        // Return approximate count (we can't easily get exact push stats)
        Ok(1)
    }

    /// Lists available remotes.
    ///
    /// # Errors
    ///
    /// Returns an error if listing fails.
    pub fn list_remotes(&self) -> Result<Vec<String>> {
        let repo = self.open_repo()?;

        let remotes = repo.remotes().map_err(|e| Error::OperationFailed {
            operation: "list_remotes".to_string(),
            cause: e.to_string(),
        })?;

        Ok(remotes.iter().filter_map(|r| r.map(String::from)).collect())
    }

    /// Gets the URL for a remote.
    ///
    /// # Errors
    ///
    /// Returns an error if the remote doesn't exist.
    pub fn get_remote_url(&self, remote_name: &str) -> Result<Option<String>> {
        let repo = self.open_repo()?;

        match repo.find_remote(remote_name) {
            Ok(remote) => Ok(remote.url().map(String::from)),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(None),
            Err(e) => Err(Error::OperationFailed {
                operation: "get_remote_url".to_string(),
                cause: e.to_string(),
            }),
        }
    }

    /// Checks if a remote exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the check fails.
    pub fn remote_exists(&self, remote_name: &str) -> Result<bool> {
        self.get_remote_url(remote_name).map(|url| url.is_some())
    }

    /// Gets the default remote name (usually "origin").
    ///
    /// # Errors
    ///
    /// Returns an error if no remotes exist.
    pub fn default_remote(&self) -> Result<Option<String>> {
        let remotes = self.list_remotes()?;

        // Prefer "origin" if it exists
        if remotes.contains(&"origin".to_string()) {
            return Ok(Some("origin".to_string()));
        }

        // Otherwise return the first remote
        Ok(remotes.into_iter().next())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, Repository) {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        // Create an initial commit in a separate scope so tree is dropped before returning
        {
            let sig = Signature::now("test", "test@test.com").unwrap();
            let tree_id = repo.index().unwrap().write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                .unwrap();
        }

        (dir, repo)
    }

    #[test]
    fn test_remote_manager_creation() {
        let manager = RemoteManager::new("/tmp/test");
        assert_eq!(manager.notes_ref, RemoteManager::DEFAULT_NOTES_REF);

        let custom = RemoteManager::new("/tmp/test").with_notes_ref("refs/notes/custom");
        assert_eq!(custom.notes_ref, "refs/notes/custom");
    }

    #[test]
    fn test_list_remotes_empty() {
        let (dir, _repo) = create_test_repo();
        let manager = RemoteManager::new(dir.path());

        let remotes = manager.list_remotes().unwrap();
        assert!(remotes.is_empty());
    }

    #[test]
    fn test_list_remotes_with_origin() {
        let (dir, repo) = create_test_repo();

        // Add a remote
        repo.remote("origin", "https://github.com/test/test.git")
            .unwrap();

        let manager = RemoteManager::new(dir.path());
        let remotes = manager.list_remotes().unwrap();

        assert_eq!(remotes.len(), 1);
        assert_eq!(remotes[0], "origin");
    }

    #[test]
    fn test_get_remote_url() {
        let (dir, repo) = create_test_repo();
        let test_url = "https://github.com/test/test.git";

        repo.remote("origin", test_url).unwrap();

        let manager = RemoteManager::new(dir.path());
        let url = manager.get_remote_url("origin").unwrap();

        assert_eq!(url, Some(test_url.to_string()));
    }

    #[test]
    fn test_get_nonexistent_remote_url() {
        let (dir, _repo) = create_test_repo();
        let manager = RemoteManager::new(dir.path());

        let url = manager.get_remote_url("nonexistent").unwrap();
        assert!(url.is_none());
    }

    #[test]
    fn test_remote_exists() {
        let (dir, repo) = create_test_repo();
        repo.remote("origin", "https://github.com/test/test.git")
            .unwrap();

        let manager = RemoteManager::new(dir.path());

        assert!(manager.remote_exists("origin").unwrap());
        assert!(!manager.remote_exists("nonexistent").unwrap());
    }

    #[test]
    fn test_default_remote() {
        let (dir, repo) = create_test_repo();

        let manager = RemoteManager::new(dir.path());

        // No remotes
        assert!(manager.default_remote().unwrap().is_none());

        // Add origin
        repo.remote("origin", "https://github.com/test/test.git")
            .unwrap();

        assert_eq!(
            manager.default_remote().unwrap(),
            Some("origin".to_string())
        );
    }

    #[test]
    fn test_default_remote_prefers_origin() {
        let (dir, repo) = create_test_repo();

        // Add remotes in non-origin order
        repo.remote("upstream", "https://github.com/upstream/test.git")
            .unwrap();
        repo.remote("origin", "https://github.com/test/test.git")
            .unwrap();

        let manager = RemoteManager::new(dir.path());

        // Should prefer origin
        assert_eq!(
            manager.default_remote().unwrap(),
            Some("origin".to_string())
        );
    }
}
