//! Group storage backends.
//!
//! Provides organization-scoped storage for groups, members, and invites.
//! Groups enable team collaboration through shared memory graphs within an organization.
//!
//! # Features
//!
//! - **Group Management**: Create, list, and delete groups within an organization
//! - **Member Management**: Add/remove members with role-based permissions
//! - **Invite System**: Token-based invites with expiration and usage limits
//!
//! # URN Scheme
//!
//! - Groups: `subcog://org:{org_id}/groups/{group_id}`
//! - Members: `subcog://org:{org_id}/groups/{group_id}/members/{email}`
//!
//! # Backend Selection
//!
//! Currently only `SQLite` is supported. The backend stores groups in
//! `~/.config/subcog/orgs/{org}/memories.db` alongside other org-scoped data.
//!
//! # Roles
//!
//! - **Admin**: Full control (manage members, create invites, delete group)
//! - **Write**: Can capture memories to the group
//! - **Read**: Can only recall memories from the group

mod sqlite;
mod traits;

pub use sqlite::SqliteGroupBackend;
pub use traits::GroupBackend;

use crate::config::SubcogConfig;
use crate::{Error, Result};
use std::path::PathBuf;
use std::sync::Arc;

/// Backend type for group storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupBackendType {
    /// `SQLite` database (default, authoritative storage).
    #[default]
    Sqlite,
}

/// Factory for creating organization-scoped group storage.
pub struct GroupStorageFactory;

impl GroupStorageFactory {
    /// Creates a group storage for the given organization.
    ///
    /// # Arguments
    ///
    /// * `org_id` - The organization identifier
    /// * `config` - Application configuration
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The group-scope feature is not enabled
    /// - Storage backend cannot be initialized
    pub fn create_for_org(org_id: &str, config: &SubcogConfig) -> Result<Arc<dyn GroupBackend>> {
        if !(config.features.org_scope_enabled || cfg!(feature = "org-scope")) {
            return Err(Error::FeatureNotEnabled("org-scope".to_string()));
        }

        if !cfg!(feature = "group-scope") {
            return Err(Error::FeatureNotEnabled("group-scope".to_string()));
        }

        let path = config
            .storage
            .org
            .path
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| SqliteGroupBackend::default_org_path(org_id));

        let db_path = path.ok_or_else(|| Error::OperationFailed {
            operation: "create_group_storage".to_string(),
            cause: "Could not determine database path".to_string(),
        })?;

        Ok(Arc::new(SqliteGroupBackend::new(db_path)?))
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
    pub fn create_with_path(path: PathBuf) -> Result<Arc<dyn GroupBackend>> {
        Ok(Arc::new(SqliteGroupBackend::new(path)?))
    }

    /// Creates an in-memory storage (useful for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be initialized.
    pub fn create_in_memory() -> Result<Arc<dyn GroupBackend>> {
        Ok(Arc::new(SqliteGroupBackend::in_memory()?))
    }

    /// Resolves the organization identifier from environment or git remote.
    ///
    /// Priority:
    /// 1. `SUBCOG_ORG` environment variable
    /// 2. Git remote URL (extracts org from `github.com/org/repo`)
    ///
    /// # Errors
    ///
    /// Returns an error if no organization identifier can be resolved.
    pub fn resolve_org_identifier() -> Result<String> {
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
    fn test_group_backend_type_default() {
        let default = GroupBackendType::default();
        assert_eq!(default, GroupBackendType::Sqlite);
    }

    #[test]
    fn test_create_in_memory() {
        let storage = GroupStorageFactory::create_in_memory();
        assert!(storage.is_ok());
    }

    #[test]
    fn test_create_with_path() {
        let dir = tempfile::TempDir::new().expect("failed to create temp dir");
        let db_path = dir.path().join("groups.db");

        let storage = GroupStorageFactory::create_with_path(db_path);
        assert!(storage.is_ok());
    }

    #[test]
    fn test_extract_org_from_git_url() {
        // HTTPS format
        assert_eq!(
            GroupStorageFactory::extract_org_from_git_url("https://github.com/zircote/subcog.git"),
            Some("zircote".to_string())
        );

        // SSH format
        assert_eq!(
            GroupStorageFactory::extract_org_from_git_url("git@github.com:zircote/subcog.git"),
            Some("zircote".to_string())
        );

        // Invalid URL
        assert_eq!(
            GroupStorageFactory::extract_org_from_git_url("not-a-url"),
            None
        );
    }
}
