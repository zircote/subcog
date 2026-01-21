//! Domain-scoped index management.
//!
//! Manages separate indices for different domain scopes:
//! - **Project**: `<user-data>/index.db` - project memories with project/branch/path facets
//! - **User**: `<user-data>/index.db` - user-wide memories
//! - **Org**: Configured path or database URL - team/enterprise memories

use crate::storage::index::SqliteBackend;
use crate::{Error, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Domain scope for index isolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DomainScope {
    /// Project scope stored in user-level index with project faceting.
    Project,
    /// User-level index stored in the user data directory.
    User,
    /// Organization-level index, configured externally.
    Org,
}

impl DomainScope {
    /// Returns the scope as a string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::User => "user",
            Self::Org => "org",
        }
    }

    /// Returns the appropriate default domain scope based on current context.
    ///
    /// - If in a git repository (`.git` folder exists): returns `Project`
    /// - If NOT in a git repository: returns `User`
    ///
    /// Storage for both scopes is user-level `SQLite` with facets for project/branch/path.
    #[must_use]
    pub fn default_for_context() -> Self {
        if is_in_git_repo() {
            Self::Project
        } else {
            Self::User
        }
    }

    /// Returns the appropriate default domain scope for a specific path.
    ///
    /// - If path is in a git repository: returns `Project`
    /// - If path is NOT in a git repository: returns `User`
    #[must_use]
    pub fn default_for_path(path: &Path) -> Self {
        if is_path_in_git_repo(path) {
            Self::Project
        } else {
            Self::User
        }
    }
}

/// Configuration for domain-scoped indices.
#[derive(Debug, Clone, Default)]
pub struct DomainIndexConfig {
    /// Path to the git repository root (for project context/faceting).
    pub repo_path: Option<PathBuf>,
    /// Organization index configuration.
    pub org_config: Option<OrgIndexConfig>,
    /// User data directory (from config). If `None`, uses platform default.
    ///
    /// This should be set from `SubcogConfig.data_dir` to ensure all components
    /// use the same path, respecting user configuration.
    pub user_data_dir: Option<PathBuf>,
}

/// Organization index configuration.
#[derive(Debug, Clone)]
pub enum OrgIndexConfig {
    /// `SQLite` file at a shared path.
    SqlitePath(PathBuf),
    /// PostgreSQL connection URL (future).
    PostgresUrl(String),
    /// Redis connection URL (future).
    RedisUrl(String),
}

/// Manages indices for different domain scopes.
pub struct DomainIndexManager {
    /// Indices by domain scope.
    indices: HashMap<DomainScope, Mutex<SqliteBackend>>,
    /// Configuration.
    config: DomainIndexConfig,
    /// User data directory base path.
    user_data_dir: PathBuf,
}

impl DomainIndexManager {
    /// Creates a new domain index manager.
    ///
    /// # Arguments
    ///
    /// * `config` - Domain index configuration
    ///
    /// # Errors
    ///
    /// Returns an error if required paths cannot be determined.
    pub fn new(config: DomainIndexConfig) -> Result<Self> {
        // Use config's user_data_dir if provided, otherwise fall back to platform default.
        // This ensures all components use the same path, respecting user configuration.
        let user_data_dir = config
            .user_data_dir
            .clone()
            .map_or_else(get_user_data_dir, Ok)?;

        Ok(Self {
            indices: HashMap::new(),
            config,
            user_data_dir,
        })
    }

    /// Gets or creates the index for a domain scope.
    ///
    /// # Errors
    ///
    /// Returns an error if the index cannot be initialized.
    pub fn get_or_create(&mut self, scope: DomainScope) -> Result<&Mutex<SqliteBackend>> {
        if !self.indices.contains_key(&scope) {
            let backend = self.create_index(scope)?;
            self.indices.insert(scope, Mutex::new(backend));
        }

        self.indices
            .get(&scope)
            .ok_or_else(|| Error::OperationFailed {
                operation: "get_index".to_string(),
                cause: format!("Index for scope {scope:?} not found"),
            })
    }

    /// Creates an index for the specified scope.
    fn create_index(&self, scope: DomainScope) -> Result<SqliteBackend> {
        let db_path = self.get_index_path(scope)?;

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::OperationFailed {
                operation: "create_index_dir".to_string(),
                cause: e.to_string(),
            })?;
        }

        SqliteBackend::new(&db_path)
    }

    /// Gets the index path for a domain scope.
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be determined.
    pub fn get_index_path(&self, scope: DomainScope) -> Result<PathBuf> {
        match scope {
            DomainScope::Project => Ok(self.get_project_index_path()),
            DomainScope::User => Ok(self.get_user_index_path()),
            DomainScope::Org => self.get_org_index_path(),
        }
    }

    /// Gets the project-scoped index path: `<user-data>/index.db`
    fn get_project_index_path(&self) -> PathBuf {
        self.user_data_dir.join("index.db")
    }

    /// Gets the user-scoped index path.
    ///
    /// `<user_data>/index.db`
    fn get_user_index_path(&self) -> PathBuf {
        self.user_data_dir.join("index.db")
    }

    /// Gets the org-scoped index path from configuration.
    fn get_org_index_path(&self) -> Result<PathBuf> {
        match &self.config.org_config {
            Some(OrgIndexConfig::SqlitePath(path)) => Ok(path.clone()),
            Some(OrgIndexConfig::PostgresUrl(_) | OrgIndexConfig::RedisUrl(_)) => {
                Err(Error::NotImplemented(
                    "PostgreSQL and Redis org indices not yet implemented".to_string(),
                ))
            },
            None => {
                // Default to user data dir org folder
                Ok(self.user_data_dir.join("org").join("index.db"))
            },
        }
    }

    /// Returns all available scopes that have been initialized.
    #[must_use]
    pub fn available_scopes(&self) -> Vec<DomainScope> {
        self.indices.keys().copied().collect()
    }

    /// Checks if a scope has an initialized index.
    #[must_use]
    pub fn has_scope(&self, scope: DomainScope) -> bool {
        self.indices.contains_key(&scope)
    }

    /// Creates a new `SQLite` backend for the specified scope.
    ///
    /// This is a high-level method that:
    /// 1. Determines the correct index path for the scope
    /// 2. Ensures the parent directory exists
    /// 3. Creates and returns a new `SqliteBackend`
    ///
    /// Unlike `get_or_create`, this always creates a fresh backend instance
    /// rather than returning a cached one. This is useful when the caller
    /// needs to manage the backend's lifecycle independently.
    ///
    /// # Arguments
    ///
    /// * `scope` - The domain scope for the index
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The index path cannot be determined (e.g., missing repo path for project scope)
    /// - The parent directory cannot be created
    /// - The `SQLite` backend fails to initialize
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let manager = DomainIndexManager::new(config)?;
    /// let backend = manager.create_backend(DomainScope::Project)?;
    /// // backend is ready for use
    /// ```
    pub fn create_backend(&self, scope: DomainScope) -> Result<SqliteBackend> {
        self.create_index(scope)
    }

    /// Creates a new `SQLite` backend with full path resolution.
    ///
    /// Returns both the created backend and the path it was created at.
    /// This is useful when the caller needs to know the path for logging
    /// or other purposes.
    ///
    /// # Arguments
    ///
    /// * `scope` - The domain scope for the index
    ///
    /// # Errors
    ///
    /// Returns an error if the backend cannot be created.
    pub fn create_backend_with_path(&self, scope: DomainScope) -> Result<(SqliteBackend, PathBuf)> {
        let path = self.get_index_path(scope)?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::OperationFailed {
                operation: "create_index_dir".to_string(),
                cause: e.to_string(),
            })?;
        }

        let backend = SqliteBackend::new(&path)?;
        Ok((backend, path))
    }
}

/// Gets the user data directory for subcog.
///
/// Returns the platform-specific user data directory:
/// - macOS: `~/Library/Application Support/subcog/`
/// - Linux: `~/.local/share/subcog/`
/// - Windows: `C:\Users\<User>\AppData\Local\subcog\`
///
/// # Errors
///
/// Returns an error if the user data directory cannot be determined.
pub fn get_user_data_dir() -> Result<PathBuf> {
    directories::BaseDirs::new()
        .map(|b| b.data_local_dir().join("subcog"))
        .ok_or_else(|| Error::OperationFailed {
            operation: "get_user_data_dir".to_string(),
            cause: "Could not determine user data directory".to_string(),
        })
}

/// Checks if the current working directory is inside a git repository.
///
/// Returns `true` if a `.git` directory exists in the current directory
/// or any parent directory.
#[must_use]
pub fn is_in_git_repo() -> bool {
    std::env::current_dir()
        .map(|cwd| is_path_in_git_repo(&cwd))
        .unwrap_or(false)
}

/// Checks if a given path is inside a git repository.
///
/// Returns `true` if a `.git` directory exists at or above the given path.
#[must_use]
pub fn is_path_in_git_repo(path: &Path) -> bool {
    find_repo_root(path).is_ok()
}

/// Resolves the repository root from a given path.
///
/// Walks up the directory tree looking for a `.git` directory.
///
/// # Errors
///
/// Returns an error if no git repository is found.
pub fn find_repo_root(start: &Path) -> Result<PathBuf> {
    let mut current = start.to_path_buf();

    // Canonicalize to resolve symlinks
    if let Ok(canonical) = current.canonicalize() {
        current = canonical;
    }

    loop {
        if current.join(".git").exists() {
            return Ok(current);
        }

        if !current.pop() {
            break;
        }
    }

    Err(Error::InvalidInput(format!(
        "No git repository found starting from: {}",
        start.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_domain_scope_as_str() {
        assert_eq!(DomainScope::Project.as_str(), "project");
        assert_eq!(DomainScope::User.as_str(), "user");
        assert_eq!(DomainScope::Org.as_str(), "org");
    }

    #[test]
    fn test_find_repo_root() {
        let dir = TempDir::new().unwrap();
        let repo_root = dir.path();

        // Create .git directory
        std::fs::create_dir(repo_root.join(".git")).unwrap();

        // Create nested directory
        let nested = repo_root.join("src").join("lib");
        std::fs::create_dir_all(&nested).unwrap();

        // Should find repo root from nested path
        let found = find_repo_root(&nested).unwrap();
        assert_eq!(found, repo_root.canonicalize().unwrap());
    }

    #[test]
    fn test_find_repo_root_not_found() {
        let dir = TempDir::new().unwrap();
        // No .git directory
        let result = find_repo_root(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_project_index_path() {
        let config = DomainIndexConfig {
            repo_path: Some(PathBuf::from("/path/to/repo")),
            org_config: None,
            user_data_dir: None,
        };
        let manager = DomainIndexManager::new(config).unwrap();

        let path = manager.get_index_path(DomainScope::Project).unwrap();
        let expected = get_user_data_dir().unwrap().join("index.db");
        assert_eq!(path, expected);
    }

    #[test]
    fn test_user_index_path() {
        let config = DomainIndexConfig {
            repo_path: Some(PathBuf::from("/path/to/repo")),
            org_config: None,
            user_data_dir: None,
        };
        let manager = DomainIndexManager::new(config).unwrap();

        let path = manager.get_index_path(DomainScope::User).unwrap();
        let expected = get_user_data_dir().unwrap().join("index.db");
        assert_eq!(path, expected);
    }

    #[test]
    fn test_org_index_path_configured() {
        let config = DomainIndexConfig {
            repo_path: Some(PathBuf::from("/path/to/repo")),
            org_config: Some(OrgIndexConfig::SqlitePath(PathBuf::from(
                "/shared/org/index.db",
            ))),
            user_data_dir: None,
        };
        let manager = DomainIndexManager::new(config).unwrap();

        let path = manager.get_index_path(DomainScope::Org).unwrap();
        assert_eq!(path, PathBuf::from("/shared/org/index.db"));
    }

    #[test]
    fn test_is_path_in_git_repo_with_git() {
        let dir = TempDir::new().unwrap();
        let repo_root = dir.path();

        // Create .git directory
        std::fs::create_dir(repo_root.join(".git")).unwrap();

        // Path with .git should return true
        assert!(is_path_in_git_repo(repo_root));

        // Nested path should also return true
        let nested = repo_root.join("src");
        std::fs::create_dir_all(&nested).unwrap();
        assert!(is_path_in_git_repo(&nested));
    }

    #[test]
    fn test_is_path_in_git_repo_without_git() {
        let dir = TempDir::new().unwrap();
        // No .git directory - should return false
        assert!(!is_path_in_git_repo(dir.path()));
    }

    #[test]
    fn test_default_for_path_in_git_repo() {
        let dir = TempDir::new().unwrap();
        let repo_root = dir.path();

        // Create .git directory
        std::fs::create_dir(repo_root.join(".git")).unwrap();

        // Should default to Project when in git repo
        assert_eq!(
            DomainScope::default_for_path(repo_root),
            DomainScope::Project
        );
    }

    #[test]
    fn test_default_for_path_not_in_git_repo() {
        let dir = TempDir::new().unwrap();
        // No .git directory - should default to User
        assert_eq!(DomainScope::default_for_path(dir.path()), DomainScope::User);
    }
}
