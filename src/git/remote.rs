//! Git remote operations.
//!
//! Provides git context detection for repository, branch, and remote information.

use crate::{Error, Result};
use git2::Repository;
use std::path::Path;

/// Manages git remote operations for context detection.
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

    /// Opens the git repository.
    fn open_repo(&self) -> Result<Repository> {
        Repository::open(&self.repo_path).map_err(|e| Error::OperationFailed {
            operation: "open_repository".to_string(),
            cause: e.to_string(),
        })
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

    /// Gets the current branch name.
    ///
    /// # Errors
    ///
    /// Returns an error if the branch cannot be determined.
    pub fn current_branch(&self) -> Result<Option<String>> {
        let repo = self.open_repo()?;

        let head = match repo.head() {
            Ok(h) => h,
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => return Ok(None),
            Err(e) => {
                return Err(Error::OperationFailed {
                    operation: "get_head".to_string(),
                    cause: e.to_string(),
                });
            },
        };

        if head.is_branch() {
            Ok(head.shorthand().map(String::from))
        } else {
            Ok(None)
        }
    }

    /// Gets the repository root path.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository cannot be opened.
    pub fn repo_root(&self) -> Result<std::path::PathBuf> {
        let repo = self.open_repo()?;
        repo.workdir()
            .map(std::path::Path::to_path_buf)
            .ok_or_else(|| Error::OperationFailed {
                operation: "get_workdir".to_string(),
                cause: "Repository has no working directory (bare repo)".to_string(),
            })
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
        let _manager = RemoteManager::new("/tmp/test");
        // Just verifies creation works
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

    #[test]
    fn test_current_branch() {
        let (dir, _repo) = create_test_repo();
        let manager = RemoteManager::new(dir.path());

        // Default branch after init is usually "master" or "main"
        let branch = manager.current_branch().unwrap();
        assert!(branch.is_some());
    }

    #[test]
    fn test_repo_root() {
        let (dir, _repo) = create_test_repo();
        let manager = RemoteManager::new(dir.path());

        let root = manager.repo_root().unwrap();
        // Canonicalize both paths to handle symlinks (e.g., /var -> /private/var on macOS)
        let expected = dir.path().canonicalize().unwrap();
        let actual = root.canonicalize().unwrap();
        assert_eq!(actual, expected);
    }
}
