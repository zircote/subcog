//! Context template storage backends.
//!
//! Provides domain-scoped storage for context templates with versioning support.
//! Each save auto-increments the version number, allowing retrieval of specific
//! versions or the latest version.
//!
//! # URN Scheme
//!
//! `subcog://{domain}/_context_templates/{template-name}@{version}`
//!
//! Examples:
//! - `subcog://project/_context_templates/search-results@1`
//! - `subcog://user/_context_templates/memory-context@latest`
//!
//! # Backend Selection
//!
//! Currently only `SQLite` is supported. The backend stores templates in
//! `~/.config/subcog/memories.db` alongside other subcog data.
//!
//! # Versioning
//!
//! - Each save creates a new version (auto-increment)
//! - Retrieve specific version: `get("name", Some(2))`
//! - Retrieve latest: `get("name", None)`
//! - List available versions: `get_versions("name")`
//! - Delete specific version or all versions

mod sqlite;
mod traits;

pub use sqlite::{ContextTemplateDbStats, SqliteContextTemplateStorage};
pub use traits::ContextTemplateStorage;

use crate::config::SubcogConfig;
use crate::storage::index::DomainScope;
use crate::{Error, Result};
use std::path::PathBuf;
use std::sync::Arc;

/// Backend type for context template storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContextTemplateBackendType {
    /// `SQLite` database (default, authoritative storage).
    #[default]
    Sqlite,
}

/// Factory for creating domain-scoped context template storage.
pub struct ContextTemplateStorageFactory;

impl ContextTemplateStorageFactory {
    /// Creates a context template storage for the given domain scope.
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
    pub fn create_for_scope(
        scope: DomainScope,
        config: &SubcogConfig,
    ) -> Result<Arc<dyn ContextTemplateStorage>> {
        if matches!(scope, DomainScope::Org)
            && !(config.features.org_scope_enabled || cfg!(feature = "org-scope"))
        {
            return Err(Error::FeatureNotEnabled("org-scope".to_string()));
        }

        let path = match scope {
            DomainScope::Project | DomainScope::User => config
                .storage
                .user
                .path
                .as_ref()
                .map(PathBuf::from)
                .or_else(SqliteContextTemplateStorage::default_user_path),
            DomainScope::Org => {
                let org = Self::resolve_org_identifier()?;
                config
                    .storage
                    .org
                    .path
                    .as_ref()
                    .map(PathBuf::from)
                    .or_else(|| SqliteContextTemplateStorage::default_org_path(&org))
            },
        };

        let db_path = path.ok_or_else(|| Error::OperationFailed {
            operation: "create_context_template_storage".to_string(),
            cause: "Could not determine database path".to_string(),
        })?;

        Ok(Arc::new(SqliteContextTemplateStorage::new(db_path)?))
    }

    /// Creates storage with an explicit path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the `SQLite` database file
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be initialized.
    pub fn create_with_path(path: PathBuf) -> Result<Arc<dyn ContextTemplateStorage>> {
        Ok(Arc::new(SqliteContextTemplateStorage::new(path)?))
    }

    /// Creates an in-memory storage (useful for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be initialized.
    pub fn create_in_memory() -> Result<Arc<dyn ContextTemplateStorage>> {
        Ok(Arc::new(SqliteContextTemplateStorage::in_memory()?))
    }

    /// Resolves the organization identifier.
    ///
    /// Priority:
    /// 1. `SUBCOG_ORG` environment variable
    /// 2. Git remote URL (extracts org from `github.com/org/repo`)
    fn resolve_org_identifier() -> Result<String> {
        // 1. Check SUBCOG_ORG environment variable
        if let Ok(org) = std::env::var("SUBCOG_ORG")
            && !org.is_empty()
        {
            return Ok(org);
        }

        // 2. Try current directory
        if let Ok(cwd) = std::env::current_dir()
            && let Some(org) = Self::extract_org_from_repo_path(&cwd)
        {
            return Ok(org);
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
    fn extract_org_from_git_url(url: &str) -> Option<String> {
        // Handle SSH format: git@github.com:org/repo.git
        if let Some((rest, path_start)) = url
            .strip_prefix("git@")
            .and_then(|rest| rest.find(':').map(|path_start| (rest, path_start)))
        {
            let path = &rest[path_start + 1..];
            return path.split('/').next().map(ToString::to_string);
        }

        // Handle HTTPS/SSH URL format
        let path = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .or_else(|| url.strip_prefix("ssh://"))
            .or_else(|| url.strip_prefix("git://"));

        if let Some(org) = path.and_then(|rest| rest.split('/').nth(1)) {
            return Some(org.to_string());
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_template_backend_type_default() {
        let default = ContextTemplateBackendType::default();
        assert_eq!(default, ContextTemplateBackendType::Sqlite);
    }

    #[test]
    fn test_create_in_memory() {
        let storage = ContextTemplateStorageFactory::create_in_memory();
        assert!(storage.is_ok());
    }

    #[test]
    fn test_create_with_path() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("context_templates.db");

        let storage = ContextTemplateStorageFactory::create_with_path(db_path);
        assert!(storage.is_ok());
    }

    #[test]
    fn test_extract_org_from_git_url() {
        // HTTPS format
        assert_eq!(
            ContextTemplateStorageFactory::extract_org_from_git_url(
                "https://github.com/zircote/subcog.git"
            ),
            Some("zircote".to_string())
        );

        // SSH format
        assert_eq!(
            ContextTemplateStorageFactory::extract_org_from_git_url(
                "git@github.com:zircote/subcog.git"
            ),
            Some("zircote".to_string())
        );

        // Invalid URL
        assert_eq!(
            ContextTemplateStorageFactory::extract_org_from_git_url("not-a-url"),
            None
        );
    }
}
