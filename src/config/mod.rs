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
