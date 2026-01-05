//! Prompt storage backends.
//!
//! Provides domain-scoped storage for prompt templates with pluggable backends:
//!
//! - **Project scope**: `SQLite` (faceted by repo/branch)
//! - **User scope**: `SQLite`, PostgreSQL, Redis, or Filesystem
//! - **Org scope**: `SQLite` or Filesystem (org-isolated)
//!
//! # URN Scheme
//!
//! `subcog://{domain}/_prompts/{prompt-name}`
//!
//! Examples:
//! - `subcog://project/_prompts/code-review`
//! - `subcog://user/_prompts/api-design`
//! - `subcog://org/_prompts/team-review`
//!
//! # Backend Selection Logic
//!
//! The backend is selected based on configuration and domain:
//!
//! ```text
//! 1. Check explicit backend type in config/env
//!     ├─► PostgreSQL if SUBCOG_POSTGRES_URL set + domain supports it
//!     ├─► Redis if SUBCOG_REDIS_URL set + domain supports it
//!     └─► Continue to step 2
//!
//! 2. Check domain scope
//!     ├─► Project → `SQLite` (faceted by repo/branch)
//!     ├─► User → `SQLite` (local, performant, no server required)
//!     └─► Org → `SQLite` with org-prefixed path
//!
//! 3. Fallback
//!     └─► Filesystem (always available, human-readable YAML files)
//! ```
//!
//! ## Selection Priority by Domain
//!
//! | Domain | Priority Order | Rationale |
//! |--------|----------------|-----------|
//! | Project | `SQLite` → Filesystem | Faceted by repo/branch |
//! | User | `PostgreSQL` → `Redis` → `SQLite` → Filesystem | Configured external, then local |
//! | Org | `PostgreSQL` → `Redis` → `SQLite` → Filesystem | Shared org database preferred |
//!
//! ## Backend Capabilities
//!
//! | Backend | ACID | Shared | Versioned | Query | Setup |
//! |---------|------|--------|-----------|-------|-------|
//! | `SQLite` | Yes | No | No | Full SQL | None |
//! | PostgreSQL | Yes | Yes | No | Full SQL | Server |
//! | Redis | No | Yes | No | Pattern | Server |
//! | Filesystem | No | Via sync | No | Glob only | None |
//!
//! # Domain Routing
//!
//! Each domain scope maps to an appropriate storage backend:
//!
//! | Domain | Backend | Location |
//! |--------|---------|----------|
//! | Project | `SQLite` | `~/.config/subcog/memories.db` (with repo/branch facets) |
//! | User | `SQLite` | `~/.config/subcog/memories.db` |
//! | User | PostgreSQL | Configured connection |
//! | User | Redis | Configured connection |
//! | User | Filesystem | `~/.config/subcog/prompts/` |
//! | Org | `SQLite` | `~/.config/subcog/orgs/{org}/memories.db` |
//! | Org | Filesystem | `~/.config/subcog/orgs/{org}/prompts/` |
//!
//! # Org Identifier Resolution
//!
//! The org identifier is resolved from:
//! 1. `SUBCOG_ORG` environment variable (highest priority)
//! 2. Git remote URL (extracts org from `github.com/org/repo`)
//! 3. Returns error if no identifier can be resolved

mod filesystem;
mod postgresql;
mod redis;
mod sqlite;
mod traits;

pub use filesystem::FilesystemPromptStorage;
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
    /// `SQLite` database (default, authoritative storage).
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
    /// - **Project**: `SQLite` at `~/.config/subcog/memories.db` (with repo/branch facets)
    /// - **User**: `SQLite` at `~/.config/subcog/memories.db` (default)
    /// - **Org**: `SQLite` at `~/.config/subcog/orgs/{org}/memories.db`
    ///
    /// # Arguments
    ///
    /// * `scope` - The domain scope
    /// * `config` - Application configuration
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Org scope is requested but no org identifier can be resolved
    /// - Storage backend cannot be initialized
    pub fn create_for_scope(scope: DomainScope, config: &Config) -> Result<Arc<dyn PromptStorage>> {
        match scope {
            DomainScope::Project => Self::create_project_storage(config),
            DomainScope::User => Self::create_user_storage(config),
            DomainScope::Org => Self::create_org_storage(config),
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

        if matches!(scope, DomainScope::Org)
            && !(config.features.org_scope_enabled || cfg!(feature = "org-scope"))
        {
            return Err(Error::FeatureNotEnabled("org-scope".to_string()));
        }

        let storage_config = match scope {
            DomainScope::Project => &config.storage.project,
            DomainScope::User => &config.storage.user,
            DomainScope::Org => &config.storage.org,
        };

        let backend = match storage_config.backend {
            StorageBackendType::Sqlite => PromptBackendType::Sqlite,
            StorageBackendType::Filesystem => PromptBackendType::Filesystem,
            StorageBackendType::PostgreSQL => PromptBackendType::PostgreSQL,
            StorageBackendType::Redis => PromptBackendType::Redis,
        };

        let path = storage_config.path.as_ref().map(PathBuf::from);
        let connection_url = storage_config.connection_string.clone();

        Self::create_with_backend(backend, path, connection_url)
    }

    /// Creates project-scoped storage (`SQLite` in project directory).
    fn create_project_storage(_config: &Config) -> Result<Arc<dyn PromptStorage>> {
        // Project scope now uses SQLite (same as user scope for consistency)
        if let Some(db_path) = SqlitePromptStorage::default_user_path() {
            match SqlitePromptStorage::new(&db_path) {
                Ok(storage) => return Ok(Arc::new(storage)),
                Err(e) => {
                    tracing::warn!("Failed to create SQLite prompt storage: {e}");
                },
            }
        }

        // Fallback to filesystem
        let fs_path =
            FilesystemPromptStorage::default_user_path().ok_or_else(|| Error::OperationFailed {
                operation: "create_project_storage".to_string(),
                cause: "Could not determine user config directory".to_string(),
            })?;

        Ok(Arc::new(FilesystemPromptStorage::new(fs_path)?))
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

    /// Creates org-scoped storage based on configuration.
    ///
    /// Resolves org identifier from:
    /// 1. `SUBCOG_ORG` environment variable
    /// 2. Git remote URL (extracts org from `github.com/org/repo`)
    /// 3. Falls back with error if no org can be resolved
    ///
    /// Priority order:
    /// 1. `SQLite` (default)
    /// 2. Filesystem (fallback if `SQLite` fails)
    fn create_org_storage(config: &Config) -> Result<Arc<dyn PromptStorage>> {
        let org = Self::resolve_org_identifier(config)?;

        // Try SQLite (default)
        if let Some(db_path) = SqlitePromptStorage::default_org_path(&org) {
            match SqlitePromptStorage::new(&db_path) {
                Ok(storage) => return Ok(Arc::new(storage)),
                Err(e) => {
                    tracing::warn!("Failed to create SQLite org prompt storage: {e}");
                    // Fall through to filesystem
                },
            }
        }

        // Fallback to filesystem
        let fs_path = FilesystemPromptStorage::default_org_path(&org).ok_or_else(|| {
            Error::OperationFailed {
                operation: "create_org_storage".to_string(),
                cause: "Could not determine org config directory".to_string(),
            }
        })?;

        Ok(Arc::new(FilesystemPromptStorage::new(fs_path)?))
    }

    /// Resolves the organization identifier.
    ///
    /// Priority:
    /// 1. `SUBCOG_ORG` environment variable
    /// 2. Git remote URL (extracts org from `github.com/org/repo`)
    fn resolve_org_identifier(config: &Config) -> Result<String> {
        // 1. Check SUBCOG_ORG environment variable
        if let Ok(org) = std::env::var("SUBCOG_ORG") {
            if !org.is_empty() {
                return Ok(org);
            }
        }

        // 2. Try to extract from git remote in config repo path
        if let Some(ref repo_path) = config.repo_path {
            if let Some(org) = Self::extract_org_from_repo_path(repo_path) {
                return Ok(org);
            }
        }

        // 3. Try current directory as fallback
        if let Ok(cwd) = std::env::current_dir() {
            if let Some(org) = Self::extract_org_from_repo_path(&cwd) {
                return Ok(org);
            }
        }

        Err(Error::InvalidInput(
            "Could not resolve organization identifier. \
             Set SUBCOG_ORG environment variable or ensure git remote is configured."
                .to_string(),
        ))
    }

    /// Extracts organization from a git repository's origin remote.
    fn extract_org_from_repo_path(path: &std::path::Path) -> Option<String> {
        let repo = git2::Repository::open(path).ok()?;
        let remote = repo.find_remote("origin").ok()?;
        let url = remote.url()?;
        Self::extract_org_from_git_url(url)
    }

    /// Extracts organization name from a git URL.
    ///
    /// Supports:
    /// - `https://github.com/org/repo.git`
    /// - `git@github.com:org/repo.git`
    /// - `ssh://git@github.com/org/repo.git`
    fn extract_org_from_git_url(url: &str) -> Option<String> {
        // Handle SSH format: git@github.com:org/repo.git
        if let Some(rest) = url.strip_prefix("git@") {
            if let Some(path_start) = rest.find(':') {
                let path = &rest[path_start + 1..];
                return path.split('/').next().map(ToString::to_string);
            }
        }

        // Handle HTTPS/SSH URL format
        // Examples:
        // - https://github.com/org/repo.git
        // - ssh://git@github.com/org/repo.git
        let path = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .or_else(|| url.strip_prefix("ssh://"))
            .or_else(|| url.strip_prefix("git://"));

        if let Some(rest) = path {
            // Skip host (github.com, gitlab.com, etc.)
            let parts: Vec<&str> = rest.split('/').collect();
            if parts.len() >= 2 {
                // parts[0] = host (github.com) or user@host
                // parts[1] = org
                return Some(parts[1].to_string());
            }
        }

        None
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
    fn test_create_org_scope_with_git_remote() {
        // This test verifies that org scope works when in a git repo with origin remote.
        // If SUBCOG_ORG is set or we're in a repo with origin, this should succeed.
        let config = Config::default();
        let result = PromptStorageFactory::create_for_scope(DomainScope::Org, &config);

        // In most test environments (including CI), we're in a git repo with origin.
        // The result depends on whether an org identifier can be resolved.
        // If SUBCOG_ORG is set OR we have a git origin, it should succeed.
        // If neither, it fails with InvalidInput.
        if result.is_ok() {
            // Org was resolved from env or git remote - success path
            assert!(result.is_ok());
        } else {
            // No org could be resolved - error path
            let Err(err) = result else { return };
            assert!(
                matches!(err, Error::InvalidInput(ref msg) if msg.contains("organization identifier")),
                "Expected InvalidInput error about org identifier"
            );
        }
    }

    #[test]
    fn test_resolve_org_from_git_url_in_config() {
        // Test org resolution with a repo path pointing to this repo
        let cwd = std::env::current_dir().ok();
        let config = Config {
            repo_path: cwd,
            ..Config::default()
        };

        // If we're in a git repo with origin, this should succeed
        let result = PromptStorageFactory::resolve_org_identifier(&config);

        // Result depends on environment - in most cases we're in a git repo
        if let Ok(org) = result {
            assert!(!org.is_empty(), "Org should not be empty");
        }
    }

    #[test]
    fn test_extract_org_from_git_url() {
        // HTTPS format
        assert_eq!(
            PromptStorageFactory::extract_org_from_git_url("https://github.com/zircote/subcog.git"),
            Some("zircote".to_string())
        );

        // SSH format
        assert_eq!(
            PromptStorageFactory::extract_org_from_git_url("git@github.com:zircote/subcog.git"),
            Some("zircote".to_string())
        );

        // SSH URL format
        assert_eq!(
            PromptStorageFactory::extract_org_from_git_url(
                "ssh://git@github.com/zircote/subcog.git"
            ),
            Some("zircote".to_string())
        );

        // GitLab
        assert_eq!(
            PromptStorageFactory::extract_org_from_git_url("https://gitlab.com/myorg/myrepo.git"),
            Some("myorg".to_string())
        );

        // Invalid URL
        assert_eq!(
            PromptStorageFactory::extract_org_from_git_url("not-a-url"),
            None
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
