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
    /// Search intent configuration.
    pub search_intent: SearchIntentConfig,
    /// Observability settings.
    pub observability: ObservabilitySettings,
    /// Prompt customization settings.
    pub prompt: PromptConfig,
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
    /// Request timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Connect timeout in milliseconds.
    pub connect_timeout_ms: Option<u64>,
    /// Maximum retries for LLM calls.
    pub max_retries: Option<u32>,
    /// Retry backoff in milliseconds.
    pub retry_backoff_ms: Option<u64>,
    /// Consecutive failures before opening circuit breaker.
    pub breaker_failure_threshold: Option<u32>,
    /// Circuit breaker reset timeout in milliseconds.
    pub breaker_reset_ms: Option<u64>,
    /// Half-open trial requests.
    pub breaker_half_open_max_calls: Option<u32>,
    /// Latency budget in milliseconds.
    pub latency_slo_ms: Option<u64>,
    /// Error budget ratio threshold.
    pub error_budget_ratio: Option<f64>,
    /// Error budget window in seconds.
    pub error_budget_window_secs: Option<u64>,
}

/// Observability configuration settings.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ObservabilitySettings {
    /// Logging settings.
    pub logging: Option<LoggingSettings>,
    /// Tracing settings.
    pub tracing: Option<TracingSettings>,
    /// Metrics settings.
    pub metrics: Option<MetricsSettings>,
}

/// Logging configuration settings.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LoggingSettings {
    /// Log format ("json" or "pretty").
    pub format: Option<String>,
    /// Log level (e.g. "info").
    pub level: Option<String>,
    /// Full filter override (e.g. "subcog=debug,hyper=info").
    pub filter: Option<String>,
}

/// Tracing configuration settings.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TracingSettings {
    /// Whether tracing is enabled.
    pub enabled: Option<bool>,
    /// OTLP exporter settings.
    pub otlp: Option<OtlpSettings>,
    /// Sample ratio for traces.
    pub sample_ratio: Option<f64>,
    /// Service name override.
    pub service_name: Option<String>,
    /// Resource attributes (key=value entries).
    pub resource_attributes: Option<Vec<String>>,
}

/// OTLP exporter settings.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OtlpSettings {
    /// Collector endpoint URL.
    pub endpoint: Option<String>,
    /// Transport protocol ("grpc" or "http").
    pub protocol: Option<String>,
}

/// Metrics configuration settings.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MetricsSettings {
    /// Whether metrics are enabled.
    pub enabled: Option<bool>,
    /// Prometheus exporter port.
    pub port: Option<u16>,
    /// Push gateway settings for short-lived processes.
    pub push_gateway: Option<MetricsPushGatewaySettings>,
}

/// Prometheus push gateway configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MetricsPushGatewaySettings {
    /// Push gateway endpoint URI.
    pub endpoint: Option<String>,
    /// Optional username for basic auth.
    pub username: Option<String>,
    /// Optional password for basic auth.
    pub password: Option<String>,
    /// Use HTTP POST instead of PUT.
    pub use_http_post: Option<bool>,
}

/// Configuration for search intent detection.
#[derive(Debug, Clone)]
pub struct SearchIntentConfig {
    /// Whether search intent detection is enabled.
    pub enabled: bool,
    /// Whether to use LLM for intent classification.
    pub use_llm: bool,
    /// Timeout for LLM classification in milliseconds.
    pub llm_timeout_ms: u64,
    /// Minimum confidence threshold for memory injection.
    pub min_confidence: f32,
    /// Base memory count for adaptive injection.
    pub base_count: usize,
    /// Maximum memory count for adaptive injection.
    pub max_count: usize,
    /// Maximum tokens for injected memories.
    pub max_tokens: usize,
}

impl Default for SearchIntentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            use_llm: true,
            llm_timeout_ms: 200,
            min_confidence: 0.5,
            base_count: 5,
            max_count: 15,
            max_tokens: 4000,
        }
    }
}

impl SearchIntentConfig {
    /// Creates a new configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads configuration from environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        Self::default().with_env_overrides()
    }

    /// Applies environment overrides.
    #[must_use]
    pub fn with_env_overrides(mut self) -> Self {
        if let Ok(v) = std::env::var("SUBCOG_SEARCH_INTENT_ENABLED") {
            self.enabled = v.to_lowercase() == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("SUBCOG_SEARCH_INTENT_USE_LLM") {
            self.use_llm = v.to_lowercase() == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("SUBCOG_SEARCH_INTENT_LLM_TIMEOUT_MS") {
            if let Ok(ms) = v.parse::<u64>() {
                self.llm_timeout_ms = ms;
            }
        }
        if let Ok(v) = std::env::var("SUBCOG_SEARCH_INTENT_MIN_CONFIDENCE") {
            if let Ok(conf) = v.parse::<f32>() {
                self.min_confidence = conf.clamp(0.0, 1.0);
            }
        }

        self
    }

    /// Builds configuration from config file settings.
    #[must_use]
    pub fn from_config_file(config: &ConfigFileSearchIntent) -> Self {
        let mut settings = Self::default();

        if let Some(enabled) = config.enabled {
            settings.enabled = enabled;
        }
        if let Some(use_llm) = config.use_llm {
            settings.use_llm = use_llm;
        }
        if let Some(llm_timeout_ms) = config.llm_timeout_ms {
            settings.llm_timeout_ms = llm_timeout_ms;
        }
        if let Some(min_confidence) = config.min_confidence {
            settings.min_confidence = min_confidence.clamp(0.0, 1.0);
        }
        if let Some(base_count) = config.base_count {
            settings.base_count = base_count;
        }
        if let Some(max_count) = config.max_count {
            settings.max_count = max_count;
        }
        if let Some(max_tokens) = config.max_tokens {
            settings.max_tokens = max_tokens;
        }

        settings
    }

    /// Sets whether LLM is enabled.
    #[must_use]
    pub const fn with_use_llm(mut self, use_llm: bool) -> Self {
        self.use_llm = use_llm;
        self
    }

    /// Sets the LLM timeout in milliseconds.
    #[must_use]
    pub const fn with_llm_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.llm_timeout_ms = timeout_ms;
        self
    }

    /// Sets the minimum confidence threshold.
    #[must_use]
    pub const fn with_min_confidence(mut self, confidence: f32) -> Self {
        self.min_confidence = confidence;
        self
    }
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
    /// Search intent configuration.
    pub search_intent: Option<ConfigFileSearchIntent>,
    /// Observability configuration.
    pub observability: Option<ObservabilitySettings>,
    /// Prompt customization.
    pub prompt: Option<ConfigFilePrompt>,
}

/// Features section in config file.
#[derive(Debug, Deserialize, Default)]
pub struct ConfigFileFeatures {
    /// Secrets filter.
    pub secrets_filter: Option<bool>,
    /// PII filter.
    pub pii_filter: Option<bool>,
    /// Multi-domain support.
    pub multi_domain: Option<bool>,
    /// Audit log.
    pub audit_log: Option<bool>,
    /// LLM-powered features.
    pub llm_features: Option<bool>,
    /// Auto-capture feature.
    pub auto_capture: Option<bool>,
    /// Consolidation feature.
    pub consolidation: Option<bool>,
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
    /// Request timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Connect timeout in milliseconds.
    pub connect_timeout_ms: Option<u64>,
    /// Maximum retries for LLM calls.
    pub max_retries: Option<u32>,
    /// Retry backoff in milliseconds.
    pub retry_backoff_ms: Option<u64>,
    /// Circuit breaker failure threshold.
    pub breaker_failure_threshold: Option<u32>,
    /// Circuit breaker reset timeout in milliseconds.
    pub breaker_reset_ms: Option<u64>,
    /// Circuit breaker half-open max calls.
    pub breaker_half_open_max_calls: Option<u32>,
    /// Latency budget in milliseconds.
    pub latency_slo_ms: Option<u64>,
    /// Error budget ratio threshold.
    pub error_budget_ratio: Option<f64>,
    /// Error budget window in seconds.
    pub error_budget_window_secs: Option<u64>,
}

/// Search intent section in config file.
#[derive(Debug, Deserialize, Default)]
pub struct ConfigFileSearchIntent {
    /// Whether search intent detection is enabled.
    pub enabled: Option<bool>,
    /// Whether to use LLM for intent classification.
    pub use_llm: Option<bool>,
    /// Timeout for LLM classification in milliseconds.
    pub llm_timeout_ms: Option<u64>,
    /// Minimum confidence threshold.
    pub min_confidence: Option<f32>,
    /// Base memory count for adaptive injection.
    pub base_count: Option<usize>,
    /// Maximum memory count for adaptive injection.
    pub max_count: Option<usize>,
    /// Maximum tokens for injected memories.
    pub max_tokens: Option<usize>,
}

/// Prompt customization section in config file.
///
/// Allows users to add custom guidance to the LLM system prompts.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigFilePrompt {
    /// Additional identity context (who subcog is in your environment).
    /// Appended to the identity section of the base prompt.
    pub identity_addendum: Option<String>,

    /// Additional global guidance (applies to all operations).
    /// Appended after the base prompt.
    pub additional_guidance: Option<String>,

    /// Per-operation customizations.
    pub capture: Option<ConfigFilePromptOperation>,
    /// Search operation customizations.
    pub search: Option<ConfigFilePromptOperation>,
    /// Enrichment operation customizations.
    pub enrichment: Option<ConfigFilePromptOperation>,
    /// Consolidation operation customizations.
    pub consolidation: Option<ConfigFilePromptOperation>,
}

/// Per-operation prompt customization.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigFilePromptOperation {
    /// Additional guidance for this specific operation.
    pub additional_guidance: Option<String>,
}

/// Runtime prompt configuration.
#[derive(Debug, Clone, Default)]
pub struct PromptConfig {
    /// Additional identity context (who subcog is in your environment).
    pub identity_addendum: Option<String>,
    /// Additional global guidance (applies to all operations).
    pub additional_guidance: Option<String>,
    /// Per-operation guidance.
    pub operation_guidance: PromptOperationConfig,
}

/// Per-operation prompt guidance.
#[derive(Debug, Clone, Default)]
pub struct PromptOperationConfig {
    /// Additional guidance for capture analysis.
    pub capture: Option<String>,
    /// Additional guidance for search intent.
    pub search: Option<String>,
    /// Additional guidance for enrichment.
    pub enrichment: Option<String>,
    /// Additional guidance for consolidation.
    pub consolidation: Option<String>,
}

impl PromptConfig {
    /// Creates a new prompt configuration from config file settings.
    #[must_use]
    pub fn from_config_file(file: &ConfigFilePrompt) -> Self {
        Self {
            identity_addendum: file.identity_addendum.clone(),
            additional_guidance: file.additional_guidance.clone(),
            operation_guidance: PromptOperationConfig {
                capture: file.capture.as_ref().and_then(|c| c.additional_guidance.clone()),
                search: file.search.as_ref().and_then(|c| c.additional_guidance.clone()),
                enrichment: file.enrichment.as_ref().and_then(|c| c.additional_guidance.clone()),
                consolidation: file.consolidation.as_ref().and_then(|c| c.additional_guidance.clone()),
            },
        }
    }

    /// Gets the operation-specific guidance for a given operation mode.
    #[must_use]
    pub fn get_operation_guidance(&self, operation: &str) -> Option<&str> {
        match operation {
            "capture_analysis" => self.operation_guidance.capture.as_deref(),
            "search_intent" => self.operation_guidance.search.as_deref(),
            "enrichment" => self.operation_guidance.enrichment.as_deref(),
            "consolidation" => self.operation_guidance.consolidation.as_deref(),
            _ => None,
        }
    }

    /// Applies environment variable overrides.
    #[must_use]
    pub fn with_env_overrides(mut self) -> Self {
        if let Ok(v) = std::env::var("SUBCOG_PROMPT_IDENTITY_ADDENDUM") {
            self.identity_addendum = Some(v);
        }
        if let Ok(v) = std::env::var("SUBCOG_PROMPT_ADDITIONAL_GUIDANCE") {
            self.additional_guidance = Some(v);
        }
        self
    }
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
            search_intent: SearchIntentConfig::default(),
            observability: ObservabilitySettings::default(),
            prompt: PromptConfig::default(),
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

        let mut config = Self::default();
        config.apply_config_file(file);
        config.apply_env_overrides();
        Ok(config)
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
            let mut config = Self::default();
            config.apply_env_overrides();
            return config;
        };

        let mut config = Self::default();

        let system_config = std::path::Path::new("/etc")
            .join("subcog")
            .join("config.toml");
        let _ = apply_config_path(&mut config, &system_config);

        let xdg_config = base_dirs
            .home_dir()
            .join(".config")
            .join("subcog")
            .join("config.toml");
        let _ = apply_config_path(&mut config, &xdg_config);

        let platform_config = base_dirs.config_dir().join("subcog").join("config.toml");
        let _ = apply_config_path(&mut config, &platform_config);

        if let Ok(cwd) = std::env::current_dir() {
            let project_config = cwd.join(".subcog").join("config.toml");
            let applied_project = apply_config_path(&mut config, &project_config);
            let repo_config = (!applied_project)
                .then(|| crate::storage::index::find_repo_root(&cwd).ok())
                .flatten()
                .map(|root| root.join(".subcog").join("config.toml"));
            let _ = repo_config
                .as_ref()
                .map(|path| apply_config_path(&mut config, path));
        }

        config.apply_env_overrides();
        config
    }

    fn apply_env_overrides(&mut self) {
        self.search_intent = self.search_intent.clone().with_env_overrides();
        self.prompt = self.prompt.clone().with_env_overrides();
    }

    /// Applies a `ConfigFile` to the current configuration.
    fn apply_config_file(&mut self, file: ConfigFile) {
        if let Some(repo_path) = file.repo_path {
            self.repo_path = PathBuf::from(repo_path);
        }
        if let Some(data_dir) = file.data_dir {
            self.data_dir = PathBuf::from(data_dir);
        }
        if let Some(max_results) = file.max_results {
            self.max_results = max_results;
        }
        if let Some(mode) = file.default_search_mode {
            self.default_search_mode = match mode.to_lowercase().as_str() {
                "text" => crate::models::SearchMode::Text,
                "vector" => crate::models::SearchMode::Vector,
                _ => crate::models::SearchMode::Hybrid,
            };
        }
        if let Some(features) = file.features {
            if let Some(v) = features.secrets_filter {
                self.features.secrets_filter = v;
            }
            if let Some(v) = features.pii_filter {
                self.features.pii_filter = v;
            }
            if let Some(v) = features.multi_domain {
                self.features.multi_domain = v;
            }
            if let Some(v) = features.audit_log {
                self.features.audit_log = v;
            }
            if let Some(v) = features.llm_features {
                self.features.llm_features = v;
            }
            if let Some(v) = features.auto_capture {
                self.features.auto_capture = v;
            }
            if let Some(v) = features.consolidation {
                self.features.consolidation = v;
            }
        }
        if let Some(llm) = file.llm {
            if let Some(provider) = llm.provider {
                self.llm.provider = LlmProvider::parse(&provider);
            }
            if let Some(model) = llm.model.filter(|value| !value.trim().is_empty()) {
                self.llm.model = Some(model);
            }
            if let Some(api_key) = llm.api_key.filter(|value| !value.trim().is_empty()) {
                self.llm.api_key = Some(api_key);
            }
            if let Some(base_url) = llm.base_url.filter(|value| !value.trim().is_empty()) {
                self.llm.base_url = Some(base_url);
            }
            if llm.timeout_ms.is_some() {
                self.llm.timeout_ms = llm.timeout_ms;
            }
            if llm.connect_timeout_ms.is_some() {
                self.llm.connect_timeout_ms = llm.connect_timeout_ms;
            }
            if llm.max_retries.is_some() {
                self.llm.max_retries = llm.max_retries;
            }
            if llm.retry_backoff_ms.is_some() {
                self.llm.retry_backoff_ms = llm.retry_backoff_ms;
            }
            if llm.breaker_failure_threshold.is_some() {
                self.llm.breaker_failure_threshold = llm.breaker_failure_threshold;
            }
            if llm.breaker_reset_ms.is_some() {
                self.llm.breaker_reset_ms = llm.breaker_reset_ms;
            }
            if llm.breaker_half_open_max_calls.is_some() {
                self.llm.breaker_half_open_max_calls = llm.breaker_half_open_max_calls;
            }
            if llm.latency_slo_ms.is_some() {
                self.llm.latency_slo_ms = llm.latency_slo_ms;
            }
            if llm.error_budget_ratio.is_some() {
                self.llm.error_budget_ratio = llm.error_budget_ratio;
            }
            if llm.error_budget_window_secs.is_some() {
                self.llm.error_budget_window_secs = llm.error_budget_window_secs;
            }
        }
        if let Some(search_intent) = file.search_intent {
            self.search_intent = SearchIntentConfig::from_config_file(&search_intent);
        }
        if let Some(observability) = file.observability {
            self.observability = observability;
        }
        if let Some(prompt) = file.prompt {
            self.prompt = PromptConfig::from_config_file(&prompt);
        }
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

fn apply_config_path(config: &mut SubcogConfig, path: &std::path::Path) -> bool {
    if let Ok(file) = load_config_file(path) {
        config.apply_config_file(file);
        return true;
    }
    false
}

fn load_config_file(path: &std::path::Path) -> crate::Result<ConfigFile> {
    let contents = std::fs::read_to_string(path).map_err(|e| crate::Error::OperationFailed {
        operation: "read_config_file".to_string(),
        cause: e.to_string(),
    })?;

    toml::from_str(&contents).map_err(|e| crate::Error::OperationFailed {
        operation: "parse_config_file".to_string(),
        cause: e.to_string(),
    })
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
