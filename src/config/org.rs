//! Organization configuration for shared memory graphs.
//!
//! Provides configuration for org-scoped memory storage with support for
//! both `SQLite` (shared file) and PostgreSQL backends.
//!
//! # Example TOML
//!
//! ```toml
//! [org]
//! name = "acme-corp"
//! backend = "sqlite"
//! sqlite_path = "/shared/org/acme-corp/index.db"
//!
//! # OR for PostgreSQL:
//! # backend = "postgresql"
//! # postgres_url = "postgresql://user:pass@host:5432/subcog_org"
//! # postgres_max_connections = 10
//! # postgres_timeout_secs = 30
//! ```

use serde::Deserialize;
use std::path::PathBuf;

use super::expand_config_path;

/// Runtime organization configuration.
///
/// Controls org-scoped memory storage for team collaboration.
#[derive(Debug, Clone)]
pub struct OrgConfig {
    /// Organization name/identifier (e.g., "acme-corp").
    ///
    /// Used in URN construction: `subcog://org/{name}/namespace/id`
    pub name: Option<String>,

    /// Backend configuration for org-scoped storage.
    pub backend: OrgBackendConfig,

    /// Whether org scope is enabled (derived from feature flag + config).
    pub enabled: bool,
}

impl Default for OrgConfig {
    fn default() -> Self {
        Self {
            name: None,
            backend: OrgBackendConfig::None,
            enabled: false,
        }
    }
}

impl OrgConfig {
    /// Creates org config from a config file section.
    ///
    /// # Arguments
    ///
    /// * `file` - The parsed config file org section
    /// * `org_scope_enabled` - Whether the `org_scope` feature flag is enabled
    #[must_use]
    pub fn from_config_file(file: &ConfigFileOrg, org_scope_enabled: bool) -> Self {
        let backend = OrgBackendConfig::from_config_file(file);
        let has_backend = !matches!(backend, OrgBackendConfig::None);

        Self {
            name: file.name.clone(),
            backend,
            enabled: org_scope_enabled && has_backend,
        }
    }

    /// Returns true if org scope is properly configured and enabled.
    #[must_use]
    pub const fn is_available(&self) -> bool {
        self.enabled && !matches!(self.backend, OrgBackendConfig::None)
    }

    /// Returns the org name or a default value.
    #[must_use]
    pub fn name_or_default(&self) -> &str {
        self.name.as_deref().unwrap_or("default")
    }
}

/// Backend configuration for org-scoped storage.
///
/// Supports multiple storage backends for different deployment scenarios:
/// - `SqliteShared`: Simple shared file for small teams (NFS, Dropbox, S3)
/// - `Postgresql`: Production database for larger teams with concurrent access
#[derive(Debug, Clone, Default)]
pub enum OrgBackendConfig {
    /// Shared `SQLite` file backend.
    ///
    /// Suitable for small teams sharing a network filesystem.
    /// Requires proper file permissions for concurrent access.
    SqliteShared {
        /// Path to shared `SQLite` file.
        ///
        /// Must be writable by all team members.
        /// Example: `/shared/org/acme-corp/index.db`
        path: PathBuf,
    },

    /// PostgreSQL backend.
    ///
    /// Recommended for production use with multiple concurrent users.
    /// Provides proper transaction isolation and connection pooling.
    Postgresql {
        /// PostgreSQL connection URL.
        ///
        /// Format: `postgresql://user:pass@host:port/database`
        /// Supports environment variable expansion: `${SUBCOG_ORG_DB_URL}`
        connection_url: String,

        /// Maximum connections in the pool.
        ///
        /// Default: 10
        max_connections: u32,

        /// Connection timeout in seconds.
        ///
        /// Default: 30
        timeout_secs: u64,
    },

    /// No backend configured (org scope disabled).
    #[default]
    None,
}

impl OrgBackendConfig {
    /// Creates backend config from a config file section.
    #[must_use]
    pub fn from_config_file(file: &ConfigFileOrg) -> Self {
        match file.backend.as_deref() {
            Some("sqlite" | "sqlite3") => file.sqlite_path.as_ref().map_or_else(
                || {
                    tracing::warn!("org.backend=sqlite but org.sqlite_path not set");
                    Self::None
                },
                |path| Self::SqliteShared {
                    path: PathBuf::from(expand_config_path(path)),
                },
            ),
            Some("postgresql" | "postgres" | "pg") => file.postgres_url.as_ref().map_or_else(
                || {
                    tracing::warn!("org.backend=postgresql but org.postgres_url not set");
                    Self::None
                },
                |url| Self::Postgresql {
                    connection_url: expand_config_path(url),
                    max_connections: file.postgres_max_connections.unwrap_or(10),
                    timeout_secs: file.postgres_timeout_secs.unwrap_or(30),
                },
            ),
            Some("none") | None => Self::None,
            Some(unknown) => {
                tracing::warn!(
                    backend = unknown,
                    "Unknown org backend type, disabling org scope"
                );
                Self::None
            },
        }
    }

    /// Returns true if this is a configured backend (not None).
    #[must_use]
    pub const fn is_configured(&self) -> bool {
        !matches!(self, Self::None)
    }

    /// Returns a display string for the backend type.
    #[must_use]
    pub const fn backend_type(&self) -> &'static str {
        match self {
            Self::SqliteShared { .. } => "sqlite",
            Self::Postgresql { .. } => "postgresql",
            Self::None => "none",
        }
    }
}

/// Organization configuration from config file.
///
/// Parsed from the `[org]` section in `subcog.toml`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigFileOrg {
    /// Organization name/identifier.
    ///
    /// Used for URN construction and display.
    pub name: Option<String>,

    /// Backend type: "sqlite", "postgresql", or "none".
    ///
    /// Default: "none" (org scope disabled)
    pub backend: Option<String>,

    /// Path to shared `SQLite` file (when backend = "sqlite").
    ///
    /// Supports `~` expansion and environment variables: `${VAR}`
    pub sqlite_path: Option<String>,

    /// PostgreSQL connection URL (when backend = "postgresql").
    ///
    /// Format: `postgresql://user:pass@host:port/database`
    /// Supports environment variable expansion.
    pub postgres_url: Option<String>,

    /// PostgreSQL maximum connections (default: 10).
    pub postgres_max_connections: Option<u32>,

    /// PostgreSQL connection timeout in seconds (default: 30).
    pub postgres_timeout_secs: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_org_config_default() {
        let config = OrgConfig::default();
        assert!(config.name.is_none());
        assert!(!config.enabled);
        assert!(!config.is_available());
    }

    #[test]
    fn test_org_backend_sqlite() {
        let file = ConfigFileOrg {
            name: Some("acme-corp".to_string()),
            backend: Some("sqlite".to_string()),
            sqlite_path: Some("/shared/org/index.db".to_string()),
            ..Default::default()
        };

        let backend = OrgBackendConfig::from_config_file(&file);
        assert!(matches!(backend, OrgBackendConfig::SqliteShared { .. }));
        assert!(backend.is_configured());
        assert_eq!(backend.backend_type(), "sqlite");
    }

    #[test]
    fn test_org_backend_postgresql() {
        let file = ConfigFileOrg {
            name: Some("acme-corp".to_string()),
            backend: Some("postgresql".to_string()),
            postgres_url: Some("postgresql://user:pass@localhost:5432/subcog".to_string()),
            postgres_max_connections: Some(20),
            postgres_timeout_secs: Some(60),
            ..Default::default()
        };

        let backend = OrgBackendConfig::from_config_file(&file);
        assert!(matches!(backend, OrgBackendConfig::Postgresql { .. }));
        assert!(backend.is_configured());
        assert_eq!(backend.backend_type(), "postgresql");

        if let OrgBackendConfig::Postgresql {
            max_connections,
            timeout_secs,
            ..
        } = backend
        {
            assert_eq!(max_connections, 20);
            assert_eq!(timeout_secs, 60);
        }
    }

    #[test]
    fn test_org_backend_none() {
        let file = ConfigFileOrg::default();
        let backend = OrgBackendConfig::from_config_file(&file);
        assert!(matches!(backend, OrgBackendConfig::None));
        assert!(!backend.is_configured());
    }

    #[test]
    fn test_org_config_from_file_enabled() {
        let file = ConfigFileOrg {
            name: Some("test-org".to_string()),
            backend: Some("sqlite".to_string()),
            sqlite_path: Some("/tmp/org.db".to_string()),
            ..Default::default()
        };

        // With org_scope_enabled = true
        let config = OrgConfig::from_config_file(&file, true);
        assert_eq!(config.name.as_deref(), Some("test-org"));
        assert!(config.enabled);
        assert!(config.is_available());
    }

    #[test]
    fn test_org_config_from_file_disabled() {
        let file = ConfigFileOrg {
            name: Some("test-org".to_string()),
            backend: Some("sqlite".to_string()),
            sqlite_path: Some("/tmp/org.db".to_string()),
            ..Default::default()
        };

        // With org_scope_enabled = false
        let config = OrgConfig::from_config_file(&file, false);
        assert!(!config.enabled);
        assert!(!config.is_available());
    }

    #[test]
    fn test_org_config_sqlite_missing_path() {
        let file = ConfigFileOrg {
            name: Some("test-org".to_string()),
            backend: Some("sqlite".to_string()),
            sqlite_path: None, // Missing!
            ..Default::default()
        };

        let backend = OrgBackendConfig::from_config_file(&file);
        assert!(matches!(backend, OrgBackendConfig::None));
    }

    #[test]
    fn test_name_or_default() {
        let config = OrgConfig::default();
        assert_eq!(config.name_or_default(), "default");

        let config = OrgConfig {
            name: Some("my-org".to_string()),
            ..Default::default()
        };
        assert_eq!(config.name_or_default(), "my-org");
    }
}
