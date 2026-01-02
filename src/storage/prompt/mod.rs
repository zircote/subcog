//! Prompt storage backends.
//!
//! Provides domain-scoped storage for prompt templates with pluggable backends:
//!
//! - **Project scope**: Git notes (`refs/notes/_prompts`)
//! - **User scope**: `SQLite`, PostgreSQL, Redis, or Filesystem
//! - **Org scope**: Deferred (not yet implemented)
//!
//! # URN Scheme
//!
//! `subcog://{domain}/_prompts/{prompt-name}`
//!
//! Examples:
//! - `subcog://project/_prompts/code-review`
//! - `subcog://user/_prompts/api-design`
//!
//! # Domain Routing
//!
//! Each domain scope maps to an appropriate storage backend:
//!
//! | Domain | Backend | Location |
//! |--------|---------|----------|
//! | Project | Git Notes | `.git/refs/notes/_prompts` |
//! | User | SQLite | `~/.config/subcog/memories.db` |
//! | User | PostgreSQL | Configured connection |
//! | User | Redis | Configured connection |
//! | User | Filesystem | `~/.config/subcog/prompts/` |
//! | Org | Deferred | Not yet implemented |

mod filesystem;
mod git_notes;
mod postgresql;
mod redis;
mod sqlite;
mod traits;

pub use filesystem::FilesystemPromptStorage;
pub use git_notes::GitNotesPromptStorage;
pub use postgresql::PostgresPromptStorage;
pub use redis::RedisPromptStorage;
pub use sqlite::SqlitePromptStorage;
pub use traits::PromptStorage;

use crate::config::Config;
use crate::storage::index::DomainScope;
use crate::{Error, Result};
use std::path::PathBuf;
use std::sync::Arc;

/// Backend type for prompt storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PromptBackendType {
    /// Git notes (project scope only).
    GitNotes,
    /// `SQLite` database.
    #[default]
    Sqlite,
    /// PostgreSQL database.
    PostgreSQL,
    /// Redis with `RediSearch`.
    Redis,
    /// Filesystem fallback.
    Filesystem,
}

/// Factory for creating domain-scoped prompt storage.
pub struct PromptStorageFactory;

impl PromptStorageFactory {
    /// Creates a prompt storage for the given domain scope.
    ///
    /// # Domain Routing
    ///
    /// - **Project**: Git notes in the repository
    /// - **User**: `SQLite` at `~/.config/subcog/memories.db` (default)
    /// - **Org**: Returns an error (deferred)
    ///
    /// # Arguments
    ///
    /// * `scope` - The domain scope
    /// * `config` - Application configuration
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Org scope is requested (not implemented)
    /// - Storage backend cannot be initialized
    pub fn create_for_scope(scope: DomainScope, config: &Config) -> Result<Arc<dyn PromptStorage>> {
        match scope {
            DomainScope::Project => Self::create_project_storage(config),
            DomainScope::User => Self::create_user_storage(config),
            DomainScope::Org => Err(Error::NotImplemented(
                "Organization-scope prompt storage is not yet implemented. \
                 Use project or user scope for now."
                    .to_string(),
            )),
        }
    }

    /// Creates storage for the given scope using full subcog configuration.
    ///
    /// Uses the storage settings from the config file when available.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The storage backend cannot be initialized
    /// - Required paths or connection strings are missing
    /// - The Redis feature is not enabled for Redis backend
    pub fn create_for_scope_with_subcog_config(
        scope: DomainScope,
        config: &crate::config::SubcogConfig,
    ) -> Result<Arc<dyn PromptStorage>> {
        use crate::config::StorageBackendType;

        let storage_config = match scope {
            DomainScope::Project => &config.storage.project,
            DomainScope::User => &config.storage.user,
            DomainScope::Org => &config.storage.org,
        };

        let backend = match storage_config.backend {
            StorageBackendType::GitNotes => PromptBackendType::GitNotes,
            StorageBackendType::Sqlite => PromptBackendType::Sqlite,
            StorageBackendType::Filesystem => PromptBackendType::Filesystem,
            StorageBackendType::PostgreSQL => PromptBackendType::PostgreSQL,
            StorageBackendType::Redis => PromptBackendType::Redis,
        };

        let path = storage_config.path.as_ref().map(PathBuf::from);
        let connection_url = storage_config.connection_string.clone();

        Self::create_with_backend(backend, path, connection_url)
    }

    /// Creates project-scoped storage (git notes).
    fn create_project_storage(config: &Config) -> Result<Arc<dyn PromptStorage>> {
        let repo_path = config
            .repo_path
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .ok_or_else(|| {
                Error::InvalidInput(
                    "Repository path not configured and current directory unavailable".to_string(),
                )
            })?;

        Ok(Arc::new(GitNotesPromptStorage::new(repo_path)))
    }

    /// Creates user-scoped storage based on configuration.
    ///
    /// Priority order:
    /// 1. `SQLite` (default)
    /// 2. Filesystem (fallback if `SQLite` fails)
    ///
    /// Note: PostgreSQL and Redis support requires explicit backend selection
    /// via `create_with_backend()`.
    fn create_user_storage(_config: &Config) -> Result<Arc<dyn PromptStorage>> {
        // Try SQLite (default)
        if let Some(db_path) = SqlitePromptStorage::default_user_path() {
            match SqlitePromptStorage::new(&db_path) {
                Ok(storage) => return Ok(Arc::new(storage)),
                Err(e) => {
                    tracing::warn!("Failed to create SQLite prompt storage: {e}");
                    // Fall through to filesystem
                },
            }
        }

        // Fallback to filesystem
        let fs_path =
            FilesystemPromptStorage::default_user_path().ok_or_else(|| Error::OperationFailed {
                operation: "create_user_storage".to_string(),
                cause: "Could not determine user config directory".to_string(),
            })?;

        Ok(Arc::new(FilesystemPromptStorage::new(fs_path)?))
    }

    /// Creates storage with an explicit backend type.
    ///
    /// # Arguments
    ///
    /// * `backend` - The backend type to use
    /// * `path` - Path for file-based backends (repo path for git, db path for `SQLite`, dir for filesystem)
    /// * `connection_url` - Connection URL for network backends (PostgreSQL, Redis)
    ///
    /// # Errors
    ///
    /// Returns an error if the backend cannot be initialized.
    pub fn create_with_backend(
        backend: PromptBackendType,
        path: Option<PathBuf>,
        connection_url: Option<String>,
    ) -> Result<Arc<dyn PromptStorage>> {
        match backend {
            PromptBackendType::GitNotes => {
                let repo_path = path
                    .or_else(|| std::env::current_dir().ok())
                    .ok_or_else(|| {
                        Error::InvalidInput(
                            "Repository path required for git notes backend".to_string(),
                        )
                    })?;
                Ok(Arc::new(GitNotesPromptStorage::new(repo_path)))
            },
            PromptBackendType::Sqlite => {
                let db_path = path
                    .or_else(SqlitePromptStorage::default_user_path)
                    .ok_or_else(|| {
                        Error::InvalidInput("Database path required for SQLite backend".to_string())
                    })?;
                Ok(Arc::new(SqlitePromptStorage::new(db_path)?))
            },
            PromptBackendType::PostgreSQL => {
                let url = connection_url.ok_or_else(|| {
                    Error::InvalidInput(
                        "Connection URL required for PostgreSQL backend".to_string(),
                    )
                })?;
                Ok(Arc::new(PostgresPromptStorage::new(&url, "prompts")?))
            },
            PromptBackendType::Redis => {
                let url = connection_url.ok_or_else(|| {
                    Error::InvalidInput("Connection URL required for Redis backend".to_string())
                })?;
                #[cfg(feature = "redis")]
                {
                    Ok(Arc::new(RedisPromptStorage::new(&url, "subcog_prompts")?))
                }
                #[cfg(not(feature = "redis"))]
                {
                    let _ = url;
                    Err(Error::FeatureNotEnabled("redis".to_string()))
                }
            },
            PromptBackendType::Filesystem => {
                let dir_path = path
                    .or_else(FilesystemPromptStorage::default_user_path)
                    .ok_or_else(|| {
                        Error::InvalidInput(
                            "Directory path required for filesystem backend".to_string(),
                        )
                    })?;
                Ok(Arc::new(FilesystemPromptStorage::new(dir_path)?))
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_prompt_backend_type_default() {
        let default = PromptBackendType::default();
        assert_eq!(default, PromptBackendType::Sqlite);
    }

    #[test]
    fn test_create_with_git_notes_backend() {
        let dir = TempDir::new().unwrap();

        // Initialize git repo
        git2::Repository::init(dir.path()).unwrap();
        {
            let repo = git2::Repository::open(dir.path()).unwrap();
            let sig = git2::Signature::now("test", "test@test.com").unwrap();
            let tree_id = repo.index().unwrap().write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                .unwrap();
        }

        let storage = PromptStorageFactory::create_with_backend(
            PromptBackendType::GitNotes,
            Some(dir.path().to_path_buf()),
            None,
        );

        assert!(storage.is_ok());
    }

    #[test]
    fn test_create_with_sqlite_backend() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("prompts.db");

        let storage = PromptStorageFactory::create_with_backend(
            PromptBackendType::Sqlite,
            Some(db_path),
            None,
        );

        assert!(storage.is_ok());
    }

    #[test]
    fn test_create_with_filesystem_backend() {
        let dir = TempDir::new().unwrap();

        let storage = PromptStorageFactory::create_with_backend(
            PromptBackendType::Filesystem,
            Some(dir.path().to_path_buf()),
            None,
        );

        assert!(storage.is_ok());
    }

    #[test]
    fn test_create_org_scope_returns_error() {
        let config = Config::default();
        let result = PromptStorageFactory::create_for_scope(DomainScope::Org, &config);

        assert!(result.is_err(), "Expected Err, got Ok");
        // Use if let to check the error without requiring Debug on Ok type
        let Err(err) = result else { return };
        assert!(
            matches!(err, Error::NotImplemented(ref msg) if msg.contains("Organization-scope")),
            "Expected NotImplemented error with Organization-scope message"
        );
    }

    #[cfg(not(feature = "postgres"))]
    #[test]
    fn test_create_postgresql_backend_stub() {
        let storage = PromptStorageFactory::create_with_backend(
            PromptBackendType::PostgreSQL,
            None,
            Some("postgresql://localhost/test".to_string()),
        );

        // Should create successfully (stub returns Ok)
        assert!(storage.is_ok());
    }

    #[cfg(not(feature = "redis"))]
    #[test]
    fn test_create_redis_backend_without_feature() {
        let result = PromptStorageFactory::create_with_backend(
            PromptBackendType::Redis,
            None,
            Some("redis://localhost".to_string()),
        );

        assert!(matches!(result, Err(Error::FeatureNotEnabled(_))));
    }
}
