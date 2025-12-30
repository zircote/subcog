//! Domain-scoped index management.
//!
//! Manages separate indices for different domain scopes:
//! - **Project**: `<repo>/.subcog/index.db` - project-specific memories
//! - **User**: `~/Library/.../subcog/repos/<hash>/index.db` - personal memories per repo
//! - **Org**: Configured path or database URL - team/enterprise memories

use crate::storage::index::SqliteBackend;
use crate::{Error, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Domain scope for index isolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DomainScope {
    /// Project-local index in `.subcog/` within the repository.
    Project,
    /// User-level index, hashed by repository path.
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
}

/// Configuration for domain-scoped indices.
#[derive(Debug, Clone, Default)]
pub struct DomainIndexConfig {
    /// Path to the git repository root (for project scope).
    pub repo_path: Option<PathBuf>,
    /// Organization index configuration.
    pub org_config: Option<OrgIndexConfig>,
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
        let user_data_dir = get_user_data_dir()?;

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
            DomainScope::Project => self.get_project_index_path(),
            DomainScope::User => self.get_user_index_path(),
            DomainScope::Org => self.get_org_index_path(),
        }
    }

    /// Gets the project-scoped index path: `<repo>/.subcog/index.db`
    fn get_project_index_path(&self) -> Result<PathBuf> {
        let repo_path = self.config.repo_path.as_ref().ok_or_else(|| {
            Error::InvalidInput("Repository path not configured for project scope".to_string())
        })?;

        Ok(repo_path.join(".subcog").join("index.db"))
    }

    /// Gets the user-scoped index path: `<user_data>/repos/<hash>/index.db`
    fn get_user_index_path(&self) -> Result<PathBuf> {
        let repo_path = self.config.repo_path.as_ref().ok_or_else(|| {
            Error::InvalidInput("Repository path not configured for user scope".to_string())
        })?;

        let hash = hash_path(repo_path);
        Ok(self.user_data_dir.join("repos").join(hash).join("index.db"))
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
}

/// Gets the user data directory for subcog.
fn get_user_data_dir() -> Result<PathBuf> {
    directories::BaseDirs::new()
        .map(|b| b.data_local_dir().join("subcog"))
        .ok_or_else(|| Error::OperationFailed {
            operation: "get_user_data_dir".to_string(),
            cause: "Could not determine user data directory".to_string(),
        })
}

/// Hashes a path for use as a directory name.
///
/// Uses a short hash to avoid overly long paths while maintaining uniqueness.
fn hash_path(path: &Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    let hash = hasher.finish();

    // Use first 16 hex chars (64 bits) - collision-resistant enough
    format!("{hash:016x}")
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
    fn test_hash_path_consistency() {
        let path = PathBuf::from("/tmp/test/repo");
        let hash1 = hash_path(&path);
        let hash2 = hash_path(&path);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_path_uniqueness() {
        let path1 = PathBuf::from("/tmp/test/repo1");
        let path2 = PathBuf::from("/tmp/test/repo2");
        let hash1 = hash_path(&path1);
        let hash2 = hash_path(&path2);
        assert_ne!(hash1, hash2);
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
        };
        let manager = DomainIndexManager::new(config).unwrap();

        let path = manager.get_index_path(DomainScope::Project).unwrap();
        assert_eq!(path, PathBuf::from("/path/to/repo/.subcog/index.db"));
    }

    #[test]
    fn test_user_index_path() {
        let config = DomainIndexConfig {
            repo_path: Some(PathBuf::from("/path/to/repo")),
            org_config: None,
        };
        let manager = DomainIndexManager::new(config).unwrap();

        let path = manager.get_index_path(DomainScope::User).unwrap();
        assert!(path.to_string_lossy().contains("repos"));
        assert!(path.to_string_lossy().ends_with("index.db"));
    }

    #[test]
    fn test_org_index_path_configured() {
        let config = DomainIndexConfig {
            repo_path: Some(PathBuf::from("/path/to/repo")),
            org_config: Some(OrgIndexConfig::SqlitePath(PathBuf::from(
                "/shared/org/index.db",
            ))),
        };
        let manager = DomainIndexManager::new(config).unwrap();

        let path = manager.get_index_path(DomainScope::Org).unwrap();
        assert_eq!(path, PathBuf::from("/shared/org/index.db"));
    }
}
