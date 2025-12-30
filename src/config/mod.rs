//! Configuration management.

mod features;

pub use features::FeatureFlags;

use serde::Deserialize;
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
    /// LLM provider configuration.
    pub llm: LlmConfig,
}

/// LLM provider configuration.
#[derive(Debug, Clone, Default)]
pub struct LlmConfig {
    /// Provider name: "anthropic", "openai", "ollama", "lmstudio".
    pub provider: LlmProvider,
    /// Model name.
    pub model: Option<String>,
    /// API key (can be environment variable reference like `${OPENAI_API_KEY}`).
    pub api_key: Option<String>,
    /// Base URL for the provider (for self-hosted).
    pub base_url: Option<String>,
}

/// Available LLM providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LlmProvider {
    /// Anthropic Claude.
    #[default]
    Anthropic,
    /// `OpenAI` GPT.
    OpenAi,
    /// Ollama (local).
    Ollama,
    /// LM Studio (local).
    LmStudio,
}

impl LlmProvider {
    /// Parses a provider string.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "openai" => Self::OpenAi,
            "ollama" => Self::Ollama,
            "lmstudio" | "lm_studio" | "lm-studio" => Self::LmStudio,
            _ => Self::Anthropic,
        }
    }
}

/// Configuration file structure (for TOML parsing).
#[derive(Debug, Deserialize, Default)]
pub struct ConfigFile {
    /// Repository path.
    pub repo_path: Option<String>,
    /// Data directory.
    pub data_dir: Option<String>,
    /// Max results.
    pub max_results: Option<usize>,
    /// Default search mode.
    pub default_search_mode: Option<String>,
    /// Feature flags.
    pub features: Option<ConfigFileFeatures>,
    /// LLM configuration.
    pub llm: Option<ConfigFileLlm>,
}

/// Features section in config file.
#[derive(Debug, Deserialize, Default)]
pub struct ConfigFileFeatures {
    /// Secrets filter.
    pub secrets_filter: Option<bool>,
    /// PII filter.
    pub pii_filter: Option<bool>,
    /// Audit log.
    pub audit_log: Option<bool>,
}

/// LLM section in config file.
#[derive(Debug, Deserialize, Default)]
pub struct ConfigFileLlm {
    /// Provider name.
    pub provider: Option<String>,
    /// Model name.
    pub model: Option<String>,
    /// API key.
    pub api_key: Option<String>,
    /// Base URL.
    pub base_url: Option<String>,
}

impl Default for SubcogConfig {
    fn default() -> Self {
        Self {
            repo_path: PathBuf::from("."),
            data_dir: PathBuf::from(".subcog"),
            features: FeatureFlags::default(),
            max_results: 10,
            default_search_mode: crate::models::SearchMode::Hybrid,
            llm: LlmConfig::default(),
        }
    }
}

impl SubcogConfig {
    /// Creates a new configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads configuration from a file path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn load_from_file(path: &std::path::Path) -> crate::Result<Self> {
        let contents =
            std::fs::read_to_string(path).map_err(|e| crate::Error::OperationFailed {
                operation: "read_config_file".to_string(),
                cause: e.to_string(),
            })?;

        let file: ConfigFile =
            toml::from_str(&contents).map_err(|e| crate::Error::OperationFailed {
                operation: "parse_config_file".to_string(),
                cause: e.to_string(),
            })?;

        Ok(Self::from_config_file(file))
    }

    /// Loads configuration from the default location.
    ///
    /// Checks the following paths in order:
    /// 1. Platform-specific config dir (`~/Library/Application Support/subcog/` on macOS)
    /// 2. XDG config dir (`~/.config/subcog/` for Unix compatibility)
    ///
    /// Returns default configuration if no config file is found.
    #[must_use]
    pub fn load_default() -> Self {
        let Some(base_dirs) = directories::BaseDirs::new() else {
            return Self::default();
        };

        // Check platform-specific config dir first
        let platform_config = base_dirs.config_dir().join("subcog").join("config.toml");
        if platform_config.exists() {
            if let Ok(config) = Self::load_from_file(&platform_config) {
                return config;
            }
        }

        // Fall back to XDG-style ~/.config/subcog/ for Unix compatibility
        let xdg_config = base_dirs
            .home_dir()
            .join(".config")
            .join("subcog")
            .join("config.toml");
        if xdg_config.exists() {
            if let Ok(config) = Self::load_from_file(&xdg_config) {
                return config;
            }
        }

        Self::default()
    }

    /// Converts a `ConfigFile` to `SubcogConfig`.
    fn from_config_file(file: ConfigFile) -> Self {
        let mut config = Self::default();

        if let Some(repo_path) = file.repo_path {
            config.repo_path = PathBuf::from(repo_path);
        }
        if let Some(data_dir) = file.data_dir {
            config.data_dir = PathBuf::from(data_dir);
        }
        if let Some(max_results) = file.max_results {
            config.max_results = max_results;
        }
        if let Some(mode) = file.default_search_mode {
            config.default_search_mode = match mode.to_lowercase().as_str() {
                "text" => crate::models::SearchMode::Text,
                "vector" => crate::models::SearchMode::Vector,
                _ => crate::models::SearchMode::Hybrid,
            };
        }
        if let Some(features) = file.features {
            if let Some(v) = features.secrets_filter {
                config.features.secrets_filter = v;
            }
            if let Some(v) = features.pii_filter {
                config.features.pii_filter = v;
            }
            if let Some(v) = features.audit_log {
                config.features.audit_log = v;
            }
        }
        if let Some(llm) = file.llm {
            if let Some(provider) = llm.provider {
                config.llm.provider = LlmProvider::parse(&provider);
            }
            config.llm.model = llm.model;
            config.llm.api_key = llm.api_key;
            config.llm.base_url = llm.base_url;
        }

        config
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
