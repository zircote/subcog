//! Centralized path management for subcog storage locations.
//!
//! This module provides a unified interface for constructing and managing
//! paths used by subcog's storage backends. It centralizes:
//!
//! - Path constants (directory names, file names)
//! - Path construction methods for different storage types
//! - Directory creation with proper error handling
//!
//! # Examples
//!
//! ```rust,ignore
//! use subcog::services::PathManager;
//! use std::path::Path;
//!
//! // For project-scoped storage (user-level data dir with project facets)
//! let manager = PathManager::for_repo(Path::new("/path/to/repo"));
//! let index_path = manager.index_path();
//! let vector_path = manager.vector_path();
//!
//! // Ensure directories exist before creating backends
//! manager.ensure_subcog_dir()?;
//! ```

use crate::storage::get_user_data_dir;
use crate::{Error, Result};
use std::path::{Path, PathBuf};

/// Legacy name for the repo-local subcog directory (project storage no longer uses it).
pub const SUBCOG_DIR_NAME: &str = ".subcog";

/// Name of the `SQLite` index database file.
pub const INDEX_DB_NAME: &str = "index.db";

/// Name of the vector index file.
pub const VECTOR_INDEX_NAME: &str = "vectors.idx";

/// Manages storage paths for subcog backends.
///
/// `PathManager` provides a centralized way to construct paths for:
/// - `SQLite` index databases
/// - Vector similarity indices
/// - The user-level data directory
///
/// Project scope uses the user-level data directory with project facets.
#[derive(Debug, Clone)]
pub struct PathManager {
    /// Base directory for storage (user data dir).
    base_dir: PathBuf,
    /// The subcog data directory (same as base dir).
    subcog_dir: PathBuf,
}

impl PathManager {
    /// Creates a `PathManager` for repository-scoped storage.
    ///
    /// Storage paths will be within the user data directory.
    /// Falls back to a temporary user-level directory if the user data dir cannot be resolved.
    ///
    /// # Arguments
    ///
    /// * `repo_root` - Path to the git repository root
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let manager = PathManager::for_repo(Path::new("/home/user/project"));
    /// // Uses user data directory, not repo-local storage
    /// ```
    #[must_use]
    pub fn for_repo(_repo_root: impl AsRef<Path>) -> Self {
        let base_dir = get_user_data_dir().unwrap_or_else(|err| {
            tracing::warn!(
                error = %err,
                "Failed to resolve user data dir; falling back to temp dir"
            );
            std::env::temp_dir().join("subcog")
        });
        let subcog_dir = base_dir.clone();
        Self {
            base_dir,
            subcog_dir,
        }
    }

    /// Creates a `PathManager` for user-scoped storage.
    ///
    /// Storage paths will be directly within the user data directory
    /// (no `.subcog` subdirectory).
    ///
    /// # Arguments
    ///
    /// * `user_data_dir` - Platform-specific user data directory
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let manager = PathManager::for_user(Path::new("/home/user/.local/share/subcog"));
    /// assert_eq!(manager.subcog_dir(), Path::new("/home/user/.local/share/subcog"));
    /// ```
    #[must_use]
    pub fn for_user(user_data_dir: impl AsRef<Path>) -> Self {
        let base_dir = user_data_dir.as_ref().to_path_buf();
        // For user scope, the base dir IS the subcog dir (no .subcog subdirectory)
        let subcog_dir = base_dir.clone();
        Self {
            base_dir,
            subcog_dir,
        }
    }

    /// Returns the base directory (user data dir).
    #[must_use]
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Returns the subcog data directory.
    ///
    /// For project/user scope: `{user_data_dir}` (same as base)
    #[must_use]
    pub fn subcog_dir(&self) -> &Path {
        &self.subcog_dir
    }

    /// Returns the path to the `SQLite` index database.
    ///
    /// # Returns
    ///
    /// `{subcog_dir}/index.db`
    #[must_use]
    pub fn index_path(&self) -> PathBuf {
        self.subcog_dir.join(INDEX_DB_NAME)
    }

    /// Returns the path to the vector similarity index.
    ///
    /// # Returns
    ///
    /// `{subcog_dir}/vectors.idx`
    #[must_use]
    pub fn vector_path(&self) -> PathBuf {
        self.subcog_dir.join(VECTOR_INDEX_NAME)
    }

    /// Ensures the subcog directory exists.
    ///
    /// Creates the directory and any necessary parent directories.
    ///
    /// # Errors
    ///
    /// Returns an error if directory creation fails due to permissions
    /// or other filesystem issues.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let manager = PathManager::for_repo(Path::new("/path/to/repo"));
    /// manager.ensure_subcog_dir()?;
    /// // Now safe to create backends at manager.index_path() and manager.vector_path()
    /// ```
    pub fn ensure_subcog_dir(&self) -> Result<()> {
        std::fs::create_dir_all(&self.subcog_dir).map_err(|e| Error::OperationFailed {
            operation: "create_subcog_dir".to_string(),
            cause: format!(
                "Cannot create {}: {}. Please create manually with: mkdir -p {}",
                self.subcog_dir.display(),
                e,
                self.subcog_dir.display()
            ),
        })
    }

    /// Ensures the parent directory of a path exists.
    ///
    /// Useful for ensuring index or vector file parents exist before
    /// creating backends.
    ///
    /// # Arguments
    ///
    /// * `path` - The path whose parent should be created
    ///
    /// # Errors
    ///
    /// Returns an error if directory creation fails.
    pub fn ensure_parent_dir(path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::OperationFailed {
                operation: "create_index_dir".to_string(),
                cause: e.to_string(),
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn test_for_repo_paths() {
        let manager = PathManager::for_repo("/home/user/project");
        let expected_base =
            get_user_data_dir().unwrap_or_else(|_| PathBuf::from("/home/user/project"));

        assert_eq!(manager.base_dir(), expected_base.as_path());
        assert_eq!(manager.subcog_dir(), expected_base.as_path());
        assert_eq!(manager.index_path(), expected_base.join("index.db"));
        assert_eq!(manager.vector_path(), expected_base.join("vectors.idx"));
    }

    #[test]
    fn test_for_user_paths() {
        let manager = PathManager::for_user("/home/user/.local/share/subcog");

        assert_eq!(
            manager.base_dir(),
            Path::new("/home/user/.local/share/subcog")
        );
        // For user scope, subcog_dir equals base_dir
        assert_eq!(
            manager.subcog_dir(),
            Path::new("/home/user/.local/share/subcog")
        );
        assert_eq!(
            manager.index_path(),
            Path::new("/home/user/.local/share/subcog/index.db")
        );
        assert_eq!(
            manager.vector_path(),
            Path::new("/home/user/.local/share/subcog/vectors.idx")
        );
    }

    #[test]
    fn test_constants() {
        assert_eq!(SUBCOG_DIR_NAME, ".subcog");
        assert_eq!(INDEX_DB_NAME, "index.db");
        assert_eq!(VECTOR_INDEX_NAME, "vectors.idx");
    }

    #[test]
    fn test_ensure_subcog_dir() {
        let temp_dir = std::env::temp_dir().join("subcog_path_manager_test");
        let _ = std::fs::remove_dir_all(&temp_dir); // Clean up from previous runs

        let manager = PathManager::for_user(&temp_dir);

        // Directory should not exist yet
        assert!(!manager.subcog_dir().exists());

        // Create it
        manager
            .ensure_subcog_dir()
            .expect("Failed to create subcog dir");

        // Now it should exist
        assert!(manager.subcog_dir().exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_ensure_parent_dir() {
        let temp_dir = std::env::temp_dir().join("subcog_path_manager_parent_test");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let nested_path = temp_dir.join("deeply").join("nested").join("file.db");

        // Parent should not exist
        assert!(!nested_path.parent().unwrap().exists());

        // Ensure parent exists
        PathManager::ensure_parent_dir(&nested_path).expect("Failed to create parent");

        // Now parent should exist
        assert!(nested_path.parent().unwrap().exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
