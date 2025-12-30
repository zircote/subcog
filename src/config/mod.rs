//! Configuration management.

mod features;

pub use features::FeatureFlags;

use std::path::PathBuf;

/// Main configuration for subcog.
#[derive(Debug, Clone)]
pub struct SubcogConfig {
    /// Path to the git repository.
    pub repo_path: PathBuf,
    /// Path to the data directory.
    pub data_dir: PathBuf,
    /// Feature flags.
    pub features: FeatureFlags,
    /// Maximum number of search results.
    pub max_results: usize,
    /// Default search mode.
    pub default_search_mode: crate::models::SearchMode,
}

impl Default for SubcogConfig {
    fn default() -> Self {
        Self {
            repo_path: PathBuf::from("."),
            data_dir: PathBuf::from(".subcog"),
            features: FeatureFlags::default(),
            max_results: 10,
            default_search_mode: crate::models::SearchMode::Hybrid,
        }
    }
}

impl SubcogConfig {
    /// Creates a new configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the repository path.
    #[must_use]
    pub fn with_repo_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.repo_path = path.into();
        self
    }

    /// Sets the data directory.
    #[must_use]
    pub fn with_data_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.data_dir = path.into();
        self
    }
}

/// Service configuration (for backwards compatibility).
///
/// Used by services for runtime configuration.
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Path to the git repository.
    pub repo_path: Option<PathBuf>,
    /// Path to the data directory.
    pub data_dir: Option<PathBuf>,
    /// Feature configuration.
    pub features: ServiceFeatures,
}

/// Feature configuration for services.
#[derive(Debug, Clone)]
pub struct ServiceFeatures {
    /// Whether to block content with secrets.
    pub block_secrets: bool,
    /// Whether to redact secrets.
    pub redact_secrets: bool,
    /// Whether to enable auto-sync.
    pub auto_sync: bool,
}

impl Default for ServiceFeatures {
    fn default() -> Self {
        Self {
            block_secrets: false,
            redact_secrets: true,
            auto_sync: false,
        }
    }
}

impl Config {
    /// Creates a new config with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates config with a repository path.
    #[must_use]
    pub fn with_repo_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.repo_path = Some(path.into());
        self
    }

    /// Creates config with a data directory.
    #[must_use]
    pub fn with_data_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.data_dir = Some(path.into());
        self
    }
}

impl From<SubcogConfig> for Config {
    fn from(subcog: SubcogConfig) -> Self {
        Self {
            repo_path: Some(subcog.repo_path),
            data_dir: Some(subcog.data_dir),
            features: ServiceFeatures {
                block_secrets: false,
                redact_secrets: subcog.features.secrets_filter,
                auto_sync: false,
            },
        }
    }
}
