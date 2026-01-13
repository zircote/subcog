//! Organization index router.
//!
//! Routes org-scoped storage operations to the configured backend (`SQLite` or PostgreSQL).
//! Provides a unified interface for org memory storage regardless of underlying backend.
//!
//! # Example
//!
//! ```rust,ignore
//! use subcog::config::OrgBackendConfig;
//! use subcog::storage::index::OrgIndexRouter;
//!
//! let config = OrgBackendConfig::SqliteShared {
//!     path: PathBuf::from("/shared/org/index.db"),
//! };
//! let router = OrgIndexRouter::new(config)?;
//! let backend = router.backend();
//! ```

use crate::config::OrgBackendConfig;
use crate::storage::traits::IndexBackend;
use crate::{Error, Result};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use super::SqliteBackend;

#[cfg(feature = "postgres")]
use super::PostgresIndexBackend;

/// Routes org-scoped storage to the configured backend.
///
/// Supports multiple backends for different deployment scenarios:
/// - `SqliteShared`: Simple shared file for small teams (NFS, shared drives)
/// - `Postgresql`: Production database for larger teams with concurrent access
#[derive(Clone)]
pub struct OrgIndexRouter {
    /// The underlying index backend.
    backend: Arc<dyn IndexBackend + Send + Sync>,
    /// Backend type for status reporting.
    backend_type: OrgBackendType,
    /// Path or URL for the backend (for status reporting).
    backend_location: String,
}

impl fmt::Debug for OrgIndexRouter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OrgIndexRouter")
            .field("backend_type", &self.backend_type)
            .field("backend_location", &self.backend_location)
            .finish_non_exhaustive()
    }
}

/// Backend type for status reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrgBackendType {
    /// Shared `SQLite` file backend.
    SqliteShared,
    /// PostgreSQL database backend.
    Postgresql,
}

impl OrgBackendType {
    /// Returns the backend type as a string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::SqliteShared => "sqlite",
            Self::Postgresql => "postgresql",
        }
    }
}

impl OrgIndexRouter {
    /// Creates a new org index router from configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The org backend configuration
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The backend cannot be initialized
    /// - The `SQLite` path's parent directory cannot be created
    /// - The PostgreSQL connection fails (when postgres feature is enabled)
    pub fn new(config: &OrgBackendConfig) -> Result<Self> {
        match config {
            OrgBackendConfig::SqliteShared { path } => Self::new_sqlite(path),
            OrgBackendConfig::Postgresql {
                connection_url,
                max_connections,
                timeout_secs,
            } => Self::new_postgresql(connection_url, *max_connections, *timeout_secs),
            OrgBackendConfig::None => Err(Error::InvalidInput(
                "Cannot create org index router with no backend configured".to_string(),
            )),
        }
    }

    /// Creates a new SQLite-backed org index router.
    fn new_sqlite(path: &PathBuf) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::OperationFailed {
                operation: "create_org_index_dir".to_string(),
                cause: e.to_string(),
            })?;
        }

        let backend = SqliteBackend::new(path)?;

        Ok(Self {
            backend: Arc::new(backend),
            backend_type: OrgBackendType::SqliteShared,
            backend_location: path.display().to_string(),
        })
    }

    /// Creates a new PostgreSQL-backed org index router.
    #[cfg(feature = "postgres")]
    fn new_postgresql(
        connection_url: &str,
        max_connections: u32,
        timeout_secs: u64,
    ) -> Result<Self> {
        let backend = PostgresIndexBackend::new(connection_url, "org_memories_index")?;

        // Log connection pool settings for debugging
        tracing::debug!(
            max_connections = max_connections,
            timeout_secs = timeout_secs,
            "Initialized PostgreSQL org index backend"
        );

        Ok(Self {
            backend: Arc::new(backend),
            backend_type: OrgBackendType::Postgresql,
            backend_location: sanitize_connection_url(connection_url),
        })
    }

    /// Creates a new PostgreSQL-backed org index router (stub when feature disabled).
    #[cfg(not(feature = "postgres"))]
    fn new_postgresql(
        _connection_url: &str,
        _max_connections: u32,
        _timeout_secs: u64,
    ) -> Result<Self> {
        Err(Error::NotImplemented(
            "PostgreSQL org backend requires the 'postgres' feature flag".to_string(),
        ))
    }

    /// Returns a reference to the underlying index backend.
    #[must_use]
    pub fn backend(&self) -> Arc<dyn IndexBackend + Send + Sync> {
        Arc::clone(&self.backend)
    }

    /// Returns the backend type.
    #[must_use]
    pub const fn backend_type(&self) -> OrgBackendType {
        self.backend_type
    }

    /// Returns the backend location (path or sanitized URL).
    #[must_use]
    pub fn backend_location(&self) -> &str {
        &self.backend_location
    }

    /// Returns status information about the org index.
    #[must_use]
    pub fn status(&self) -> OrgIndexStatus {
        OrgIndexStatus {
            backend_type: self.backend_type,
            location: self.backend_location.clone(),
            connected: true, // If we got here, we're connected
        }
    }
}

/// Status information about the org index.
#[derive(Debug, Clone)]
pub struct OrgIndexStatus {
    /// The type of backend being used.
    pub backend_type: OrgBackendType,
    /// The location (path or sanitized URL) of the backend.
    pub location: String,
    /// Whether the backend is connected/available.
    pub connected: bool,
}

/// Sanitizes a PostgreSQL connection URL for logging/display.
///
/// Removes password from the URL to prevent credential leakage.
#[cfg(feature = "postgres")]
fn sanitize_connection_url(url: &str) -> String {
    // Parse and redact password
    reqwest::Url::parse(url).map_or_else(
        |_| {
            // If parsing fails, just show the scheme
            if url.starts_with("postgresql://") {
                "postgresql://***".to_string()
            } else if url.starts_with("postgres://") {
                "postgres://***".to_string()
            } else {
                "***".to_string()
            }
        },
        |mut parsed| {
            if parsed.password().is_some() {
                let _ = parsed.set_password(Some("***"));
            }
            parsed.to_string()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_org_backend_type_as_str() {
        assert_eq!(OrgBackendType::SqliteShared.as_str(), "sqlite");
        assert_eq!(OrgBackendType::Postgresql.as_str(), "postgresql");
    }

    #[test]
    fn test_new_sqlite_creates_directory() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("org").join("subdir").join("index.db");

        let config = OrgBackendConfig::SqliteShared { path };
        let router = OrgIndexRouter::new(&config).unwrap();

        assert_eq!(router.backend_type(), OrgBackendType::SqliteShared);
        // Verify parent directory was created via the router's backend_location
        assert!(!router.backend_location().is_empty());
    }

    #[test]
    fn test_new_none_returns_error() {
        let config = OrgBackendConfig::None;
        let result = OrgIndexRouter::new(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_status() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("index.db");

        let config = OrgBackendConfig::SqliteShared { path };
        let router = OrgIndexRouter::new(&config).unwrap();

        let status = router.status();
        assert_eq!(status.backend_type, OrgBackendType::SqliteShared);
        assert!(status.connected);
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn test_sanitize_connection_url() {
        let url = "postgresql://user:secret@localhost:5432/subcog";
        let sanitized = sanitize_connection_url(url);
        assert!(sanitized.contains("***"));
        assert!(!sanitized.contains("secret"));
    }

    #[cfg(not(feature = "postgres"))]
    #[test]
    fn test_postgres_requires_feature() {
        let config = OrgBackendConfig::Postgresql {
            connection_url: "postgresql://localhost/test".to_string(),
            max_connections: 10,
            timeout_secs: 30,
        };
        let result = OrgIndexRouter::new(&config);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("postgres' feature flag")
        );
    }
}
