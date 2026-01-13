//! Configuration management.

mod features;
mod org;

pub use features::FeatureFlags;
pub use org::{ConfigFileOrg, OrgBackendConfig, OrgConfig};

use serde::Deserialize;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// Warns if a config file has world-readable permissions (SEC-M4).
///
/// Config files may contain API keys or other sensitive data. World-readable
/// permissions (mode 0o004 on Unix) pose a security risk in multi-user systems.
///
/// This function logs a warning but does not prevent loading - the user may
/// have intentionally set these permissions.
#[cfg(unix)]
fn warn_if_world_readable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    if let Ok(metadata) = std::fs::metadata(path) {
        let mode = metadata.permissions().mode();
        // Check if "others" have read permission (0o004)
        if mode & 0o004 != 0 {
            tracing::warn!(
                path = %path.display(),
                mode = format!("{mode:04o}"),
                "Config file is world-readable. Consider restricting permissions with: chmod 600 {}",
                path.display()
            );
        }
    }
}

/// No-op on non-Unix platforms.
#[cfg(not(unix))]
fn warn_if_world_readable(_path: &Path) {
    // Windows has a different permission model; skip this check
}

/// Expands environment variable references in a string.
///
/// Supports `${VAR_NAME}` syntax. If the variable is not set, the original
/// reference is preserved (e.g., `${MISSING_VAR}` stays as-is).
///
/// # Performance
///
/// Uses `Cow<str>` to avoid allocation when no expansion is needed.
/// Only allocates when at least one environment variable is found and expanded.
///
/// # Examples
///
/// ```ignore
/// // If OPENAI_API_KEY=sk-xxx in environment
/// expand_env_vars("${OPENAI_API_KEY}") // Returns "sk-xxx"
/// expand_env_vars("prefix-${VAR}-suffix") // Expands VAR in the middle
/// expand_env_vars("no vars here") // Returns unchanged (no allocation)
/// ```
/// Maximum number of environment variable expansions per string (SEC-M5).
/// Prevents `DoS` attacks from strings with many `${VAR}` patterns.
const MAX_ENV_VAR_EXPANSIONS: usize = 100;

fn expand_env_vars(input: &str) -> Cow<'_, str> {
    // Fast path: no ${} pattern at all
    if !input.contains("${") {
        return Cow::Borrowed(input);
    }

    let mut result = input.to_string();
    let mut start = 0;
    let mut expansion_count = 0;

    while let Some(var_start) = result[start..].find("${") {
        // SEC-M5: Limit expansions to prevent DoS from many ${} patterns
        expansion_count += 1;
        if expansion_count > MAX_ENV_VAR_EXPANSIONS {
            tracing::warn!(
                count = expansion_count,
                "Environment variable expansion limit reached"
            );
            break;
        }

        let var_start = start + var_start;
        if let Some(var_end) = result[var_start..].find('}') {
            let var_end = var_start + var_end;
            let var_name = &result[var_start + 2..var_end];
            if let Ok(value) = std::env::var(var_name) {
                result.replace_range(var_start..=var_end, &value);
                // Continue from where we inserted the value
                // Note: We intentionally skip past the inserted value to prevent
                // recursive expansion if the value contains ${} patterns.
                // This is a security feature, not a limitation.
                start = var_start + value.len();
            } else {
                // Skip past this ${...} if var not found
                start = var_end + 1;
            }
        } else {
            // No closing brace, stop processing
            break;
        }
    }

    // We always return owned in the slow path since we've allocated.
    // This is acceptable since we only enter this path if the input
    // contained "${" pattern.
    Cow::Owned(result)
}

fn expand_config_path(input: &str) -> String {
    let expanded = expand_env_vars(input);
    let expanded_ref = expanded.as_ref();
    let is_tilde_home =
        expanded_ref == "~" || expanded_ref.starts_with("~/") || expanded_ref.starts_with("~\\");

    if is_tilde_home && let Some(base_dirs) = directories::BaseDirs::new() {
        let mut path = base_dirs.home_dir().to_path_buf();
        let suffix = expanded_ref
            .strip_prefix("~/")
            .or_else(|| expanded_ref.strip_prefix("~\\"));
        if let Some(suffix) = suffix
            && !suffix.is_empty()
        {
            path.push(suffix);
        }
        return path.to_string_lossy().into_owned();
    }

    expanded.into_owned()
}

fn parse_bool_env(value: &str) -> Option<bool> {
    match value.trim().to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

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
    /// Storage configuration.
    pub storage: StorageConfig,
    /// Consolidation configuration.
    pub consolidation: ConsolidationConfig,
    /// TTL (Time-To-Live) configuration for memory expiration.
    pub ttl: TtlConfig,
    /// Operation timeout configuration (CHAOS-HIGH-005).
    pub timeouts: OperationTimeoutConfig,
    /// Context template configuration.
    pub context_templates: ContextTemplatesConfig,
    /// Organization configuration for shared memory graphs.
    pub org: OrgConfig,
    /// Webhook configuration.
    pub webhooks: WebhooksConfig,
    /// Config files that were loaded (for debugging).
    pub config_sources: Vec<PathBuf>,
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

impl LlmConfig {
    /// Creates LLM config from config file settings.
    ///
    /// ARCH-HIGH-002: Delegated from `SubcogConfig::apply_config_file`.
    #[must_use]
    pub fn from_config_file(file: &ConfigFileLlm) -> Self {
        let mut config = Self::default();

        if let Some(ref provider) = file.provider {
            config.provider = LlmProvider::parse(provider);
        }
        if let Some(ref model) = file.model
            && !model.trim().is_empty()
        {
            config.model = Some(model.clone());
        }
        if let Some(ref api_key) = file.api_key
            && !api_key.trim().is_empty()
        {
            // Expand environment variable references like ${OPENAI_API_KEY}
            config.api_key = Some(expand_env_vars(api_key).into_owned());
        }
        if let Some(ref base_url) = file.base_url
            && !base_url.trim().is_empty()
        {
            config.base_url = Some(base_url.clone());
        }
        config.timeout_ms = file.timeout_ms;
        config.connect_timeout_ms = file.connect_timeout_ms;
        config.max_retries = file.max_retries;
        config.retry_backoff_ms = file.retry_backoff_ms;
        config.breaker_failure_threshold = file.breaker_failure_threshold;
        config.breaker_reset_ms = file.breaker_reset_ms;
        config.breaker_half_open_max_calls = file.breaker_half_open_max_calls;
        config.latency_slo_ms = file.latency_slo_ms;
        config.error_budget_ratio = file.error_budget_ratio;
        config.error_budget_window_secs = file.error_budget_window_secs;

        config
    }

    /// Merges another config into this one.
    ///
    /// Only overrides fields that are set in the source.
    pub fn merge_from(&mut self, file: &ConfigFileLlm) {
        if let Some(ref provider) = file.provider {
            self.provider = LlmProvider::parse(provider);
        }
        if let Some(ref model) = file.model
            && !model.trim().is_empty()
        {
            self.model = Some(model.clone());
        }
        if let Some(ref api_key) = file.api_key
            && !api_key.trim().is_empty()
        {
            self.api_key = Some(expand_env_vars(api_key).into_owned());
        }
        if let Some(ref base_url) = file.base_url
            && !base_url.trim().is_empty()
        {
            self.base_url = Some(base_url.clone());
        }
        // Use file value if present, otherwise keep existing value
        self.timeout_ms = file.timeout_ms.or(self.timeout_ms);
        self.connect_timeout_ms = file.connect_timeout_ms.or(self.connect_timeout_ms);
        self.max_retries = file.max_retries.or(self.max_retries);
        self.retry_backoff_ms = file.retry_backoff_ms.or(self.retry_backoff_ms);
        self.breaker_failure_threshold = file
            .breaker_failure_threshold
            .or(self.breaker_failure_threshold);
        self.breaker_reset_ms = file.breaker_reset_ms.or(self.breaker_reset_ms);
        self.breaker_half_open_max_calls = file
            .breaker_half_open_max_calls
            .or(self.breaker_half_open_max_calls);
        self.latency_slo_ms = file.latency_slo_ms.or(self.latency_slo_ms);
        self.error_budget_ratio = file.error_budget_ratio.or(self.error_budget_ratio);
        self.error_budget_window_secs = file
            .error_budget_window_secs
            .or(self.error_budget_window_secs);
    }
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
    /// Path to log file (logs to stderr if not set).
    pub file: Option<String>,
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
    /// Namespace weights configuration.
    pub weights: NamespaceWeightsConfig,
}

/// Runtime namespace weights configuration.
///
/// Contains weight multipliers for each intent type. Values are
/// stored as `HashMap<String, f32>` where keys are namespace names
/// (lowercase) and values are boost multipliers.
#[derive(Debug, Clone, Default)]
pub struct NamespaceWeightsConfig {
    /// Weights for `HowTo` intent.
    pub howto: std::collections::HashMap<String, f32>,
    /// Weights for `Troubleshoot` intent.
    pub troubleshoot: std::collections::HashMap<String, f32>,
    /// Weights for `Location` intent.
    pub location: std::collections::HashMap<String, f32>,
    /// Weights for `Explanation` intent.
    pub explanation: std::collections::HashMap<String, f32>,
    /// Weights for `Comparison` intent.
    pub comparison: std::collections::HashMap<String, f32>,
    /// Weights for `General` intent.
    pub general: std::collections::HashMap<String, f32>,
}

impl NamespaceWeightsConfig {
    /// Creates a new config with default weights (matches hard-coded behavior).
    #[must_use]
    pub fn with_defaults() -> Self {
        use std::collections::HashMap;

        // Location/Explanation share the same weights
        let location_weights = HashMap::from([
            ("decisions".to_string(), 1.5),
            ("context".to_string(), 1.3),
            ("patterns".to_string(), 1.0),
        ]);

        Self {
            // HowTo: patterns 1.5, learnings 1.3, decisions 1.0
            howto: HashMap::from([
                ("patterns".to_string(), 1.5),
                ("learnings".to_string(), 1.3),
                ("decisions".to_string(), 1.0),
            ]),
            // Troubleshoot: blockers 1.5, learnings 1.3, decisions 1.0
            troubleshoot: HashMap::from([
                ("blockers".to_string(), 1.5),
                ("learnings".to_string(), 1.3),
                ("decisions".to_string(), 1.0),
            ]),
            // Location: decisions 1.5, context 1.3, patterns 1.0
            location: location_weights.clone(),
            // Explanation: decisions 1.5, context 1.3, patterns 1.0
            explanation: location_weights,
            // Comparison: decisions 1.5, patterns 1.3, learnings 1.0
            comparison: HashMap::from([
                ("decisions".to_string(), 1.5),
                ("patterns".to_string(), 1.3),
                ("learnings".to_string(), 1.0),
            ]),
            // General: decisions 1.2, patterns 1.2, learnings 1.0
            general: HashMap::from([
                ("decisions".to_string(), 1.2),
                ("patterns".to_string(), 1.2),
                ("learnings".to_string(), 1.0),
            ]),
        }
    }

    /// Gets the weight for a namespace and intent type.
    ///
    /// Returns 1.0 if no weight is configured.
    #[must_use]
    pub fn get_weight(&self, intent_type: &str, namespace: &str) -> f32 {
        let weights = match intent_type.to_lowercase().as_str() {
            "howto" => &self.howto,
            "troubleshoot" => &self.troubleshoot,
            "location" => &self.location,
            "explanation" => &self.explanation,
            "comparison" => &self.comparison,
            _ => &self.general,
        };
        weights.get(namespace).copied().unwrap_or(1.0)
    }

    /// Gets all namespace weights for a given intent type.
    ///
    /// Returns a vector of (`namespace_name`, weight) pairs.
    #[must_use]
    pub fn get_intent_weights(&self, intent_type: &str) -> Vec<(String, f32)> {
        let weights = match intent_type.to_lowercase().as_str() {
            "howto" => &self.howto,
            "troubleshoot" => &self.troubleshoot,
            "location" => &self.location,
            "explanation" => &self.explanation,
            "comparison" => &self.comparison,
            _ => &self.general,
        };
        weights.iter().map(|(k, v)| (k.clone(), *v)).collect()
    }

    /// Merges config file weights into this config.
    ///
    /// Only overrides values that are explicitly set in the file config.
    pub fn merge_from_file(&mut self, file: &ConfigFileNamespaceWeights) {
        if let Some(ref howto) = file.howto {
            Self::merge_intent_weights(&mut self.howto, howto);
        }
        if let Some(ref troubleshoot) = file.troubleshoot {
            Self::merge_intent_weights(&mut self.troubleshoot, troubleshoot);
        }
        if let Some(ref location) = file.location {
            Self::merge_intent_weights(&mut self.location, location);
        }
        if let Some(ref explanation) = file.explanation {
            Self::merge_intent_weights(&mut self.explanation, explanation);
        }
        if let Some(ref comparison) = file.comparison {
            Self::merge_intent_weights(&mut self.comparison, comparison);
        }
        if let Some(ref general) = file.general {
            Self::merge_intent_weights(&mut self.general, general);
        }
    }

    fn merge_intent_weights(
        target: &mut std::collections::HashMap<String, f32>,
        source: &ConfigFileIntentWeights,
    ) {
        if let Some(v) = source.decisions {
            target.insert("decisions".to_string(), v);
        }
        if let Some(v) = source.patterns {
            target.insert("patterns".to_string(), v);
        }
        if let Some(v) = source.learnings {
            target.insert("learnings".to_string(), v);
        }
        if let Some(v) = source.context {
            target.insert("context".to_string(), v);
        }
        if let Some(v) = source.tech_debt {
            target.insert("tech-debt".to_string(), v);
        }
        if let Some(v) = source.blockers {
            target.insert("blockers".to_string(), v);
        }
        if let Some(v) = source.apis {
            target.insert("apis".to_string(), v);
        }
        if let Some(v) = source.config {
            target.insert("config".to_string(), v);
        }
        if let Some(v) = source.security {
            target.insert("security".to_string(), v);
        }
        if let Some(v) = source.performance {
            target.insert("performance".to_string(), v);
        }
        if let Some(v) = source.testing {
            target.insert("testing".to_string(), v);
        }
    }
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
            weights: NamespaceWeightsConfig::with_defaults(),
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
        if let Some(ms) = std::env::var("SUBCOG_SEARCH_INTENT_LLM_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            self.llm_timeout_ms = ms;
        }
        if let Some(conf) = std::env::var("SUBCOG_SEARCH_INTENT_MIN_CONFIDENCE")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            self.min_confidence = conf.clamp(0.0, 1.0);
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
        if let Some(ref weights) = config.weights {
            settings.weights.merge_from_file(weights);
        }

        settings
    }

    /// Sets whether search intent detection is enabled.
    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
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
    ///
    /// Value is clamped to the range [0.0, 1.0].
    #[must_use]
    pub const fn with_min_confidence(mut self, confidence: f32) -> Self {
        self.min_confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Sets the base memory count for adaptive injection.
    #[must_use]
    pub const fn with_base_count(mut self, count: usize) -> Self {
        self.base_count = count;
        self
    }

    /// Sets the maximum memory count for adaptive injection.
    #[must_use]
    pub const fn with_max_count(mut self, count: usize) -> Self {
        self.max_count = count;
        self
    }

    /// Sets the maximum tokens for injected memories.
    #[must_use]
    pub const fn with_max_tokens(mut self, tokens: usize) -> Self {
        self.max_tokens = tokens;
        self
    }

    /// Sets the namespace weights configuration.
    #[must_use]
    pub fn with_weights(mut self, weights: NamespaceWeightsConfig) -> Self {
        self.weights = weights;
        self
    }

    /// Validates and builds the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `base_count` is greater than `max_count`
    /// - `max_tokens` is zero
    /// - `llm_timeout_ms` is zero when LLM is enabled
    pub fn build(self) -> Result<Self, ConfigValidationError> {
        if self.base_count > self.max_count {
            return Err(ConfigValidationError::InvalidRange {
                field: "base_count/max_count".to_string(),
                message: format!(
                    "base_count ({}) cannot be greater than max_count ({})",
                    self.base_count, self.max_count
                ),
            });
        }

        if self.max_tokens == 0 {
            return Err(ConfigValidationError::InvalidValue {
                field: "max_tokens".to_string(),
                message: "max_tokens must be greater than 0".to_string(),
            });
        }

        if self.use_llm && self.llm_timeout_ms == 0 {
            return Err(ConfigValidationError::InvalidValue {
                field: "llm_timeout_ms".to_string(),
                message: "llm_timeout_ms must be greater than 0 when LLM is enabled".to_string(),
            });
        }

        Ok(self)
    }
}

/// Errors that can occur during configuration validation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigValidationError {
    /// Invalid range between two related fields.
    #[error("Invalid range for {field}: {message}")]
    InvalidRange {
        /// The field name(s) with invalid range.
        field: String,
        /// Description of the issue.
        message: String,
    },
    /// Invalid value for a field.
    #[error("Invalid value for {field}: {message}")]
    InvalidValue {
        /// The field name with invalid value.
        field: String,
        /// Description of the issue.
        message: String,
    },
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
    /// No LLM provider configured (skips LLM-powered features).
    None,
}

impl LlmProvider {
    /// Parses a provider string.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "openai" => Self::OpenAi,
            "ollama" => Self::Ollama,
            "lmstudio" | "lm_studio" | "lm-studio" => Self::LmStudio,
            "none" | "disabled" | "" => Self::None,
            _ => Self::Anthropic,
        }
    }

    /// Returns `true` if this provider is configured (not `None`).
    #[must_use]
    pub const fn is_configured(&self) -> bool {
        !matches!(self, Self::None)
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
    /// Storage configuration.
    pub storage: Option<ConfigFileStorage>,
    /// Consolidation configuration.
    pub consolidation: Option<ConfigFileConsolidation>,
    /// TTL (Time-To-Live) configuration for memory expiration.
    pub ttl: Option<ConfigFileTtl>,
    /// Context template configuration.
    pub context_templates: Option<ConfigFileContextTemplates>,
    /// Organization configuration for shared memory graphs.
    pub org: Option<ConfigFileOrg>,
    /// Webhook configurations.
    #[serde(default)]
    pub webhooks: Vec<ConfigFileWebhook>,
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
    /// Enable org-scope storage.
    pub org_scope_enabled: Option<bool>,
    /// Enable automatic entity extraction during memory capture.
    pub auto_extract_entities: Option<bool>,
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
    /// Namespace weights configuration.
    pub weights: Option<ConfigFileNamespaceWeights>,
}

/// Namespace weights configuration in config file.
///
/// Allows customizing the boost multipliers applied to search results
/// based on intent type and namespace. Higher values prioritize that
/// namespace for the given intent type.
#[derive(Debug, Deserialize, Default, Clone)]
pub struct ConfigFileNamespaceWeights {
    /// Weights for `HowTo` intent (e.g., "how do I implement X?").
    pub howto: Option<ConfigFileIntentWeights>,
    /// Weights for `Troubleshoot` intent (e.g., "why is X failing?").
    pub troubleshoot: Option<ConfigFileIntentWeights>,
    /// Weights for `Location` intent (e.g., "where is X defined?").
    pub location: Option<ConfigFileIntentWeights>,
    /// Weights for `Explanation` intent (e.g., "what is X?").
    pub explanation: Option<ConfigFileIntentWeights>,
    /// Weights for `Comparison` intent (e.g., "X vs Y?").
    pub comparison: Option<ConfigFileIntentWeights>,
    /// Weights for `General` intent (fallback for other queries).
    pub general: Option<ConfigFileIntentWeights>,
}

/// Per-intent namespace weight multipliers.
///
/// Each field is a boost multiplier (default 1.0). Values > 1.0 boost
/// that namespace, values < 1.0 reduce priority.
#[derive(Debug, Deserialize, Default, Clone)]
pub struct ConfigFileIntentWeights {
    /// Weight for decisions namespace.
    pub decisions: Option<f32>,
    /// Weight for patterns namespace.
    pub patterns: Option<f32>,
    /// Weight for learnings namespace.
    pub learnings: Option<f32>,
    /// Weight for context namespace.
    pub context: Option<f32>,
    /// Weight for tech-debt namespace.
    pub tech_debt: Option<f32>,
    /// Weight for blockers namespace.
    pub blockers: Option<f32>,
    /// Weight for apis namespace.
    pub apis: Option<f32>,
    /// Weight for config namespace.
    pub config: Option<f32>,
    /// Weight for security namespace.
    pub security: Option<f32>,
    /// Weight for performance namespace.
    pub performance: Option<f32>,
    /// Weight for testing namespace.
    pub testing: Option<f32>,
}

/// TTL (Time-To-Live) configuration section in config file.
///
/// Supports duration strings: "7d" (days), "30d", "0" (no expiration).
///
/// # Example TOML
///
/// ```toml
/// [memory.ttl]
/// default = "30d"
///
/// [memory.ttl.namespace]
/// decisions = "90d"
/// context = "7d"
/// tech-debt = "0"  # Never expires
///
/// [memory.ttl.scope]
/// project = "30d"
/// user = "90d"
/// org = "365d"
/// ```
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigFileTtl {
    /// Default TTL for all memories (e.g., "30d").
    /// "0" means no expiration.
    pub default: Option<String>,
    /// Per-namespace TTL overrides.
    pub namespace: Option<ConfigFileTtlNamespace>,
    /// Per-scope TTL overrides.
    pub scope: Option<ConfigFileTtlScope>,
}

/// Per-namespace TTL configuration in config file.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigFileTtlNamespace {
    /// TTL for decisions namespace.
    pub decisions: Option<String>,
    /// TTL for patterns namespace.
    pub patterns: Option<String>,
    /// TTL for learnings namespace.
    pub learnings: Option<String>,
    /// TTL for context namespace.
    pub context: Option<String>,
    /// TTL for tech-debt namespace.
    #[serde(alias = "tech-debt")]
    pub tech_debt: Option<String>,
    /// TTL for apis namespace.
    pub apis: Option<String>,
    /// TTL for config namespace.
    pub config: Option<String>,
    /// TTL for security namespace.
    pub security: Option<String>,
    /// TTL for performance namespace.
    pub performance: Option<String>,
    /// TTL for testing namespace.
    pub testing: Option<String>,
}

/// Per-scope TTL configuration in config file.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigFileTtlScope {
    /// TTL for project-scoped memories.
    pub project: Option<String>,
    /// TTL for user-scoped memories.
    pub user: Option<String>,
    /// TTL for org-scoped memories.
    pub org: Option<String>,
}

/// Context template configuration section in config file.
///
/// # Example TOML
///
/// ```toml
/// [context_templates]
/// enabled = true
/// default_format = "markdown"  # markdown, json, xml
///
/// [context_templates.hooks.session_start]
/// template = "session-context"
/// version = 1
/// format = "markdown"
/// ```
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigFileContextTemplates {
    /// Whether context templates feature is enabled.
    pub enabled: Option<bool>,
    /// Default output format: "markdown", "json", or "xml".
    pub default_format: Option<String>,
    /// Per-hook template configuration.
    pub hooks: Option<ConfigFileHookTemplates>,
}

/// Per-hook template configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigFileHookTemplates {
    /// Template for `session_start` hook.
    pub session_start: Option<ConfigFileHookTemplate>,
    /// Template for `user_prompt_submit` hook.
    pub user_prompt_submit: Option<ConfigFileHookTemplate>,
    /// Template for `post_tool_use` hook.
    pub post_tool_use: Option<ConfigFileHookTemplate>,
    /// Template for `pre_compact` hook.
    pub pre_compact: Option<ConfigFileHookTemplate>,
}

/// Configuration for a specific hook's template.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigFileHookTemplate {
    /// Name of the template to use.
    pub template: Option<String>,
    /// Specific version to use (None = latest).
    pub version: Option<u32>,
    /// Output format override: "markdown", "json", or "xml".
    pub format: Option<String>,
}

/// Runtime context template configuration.
///
/// Controls the context templates feature for formatting memories and statistics
/// in hooks and MCP tool responses.
#[derive(Debug, Clone)]
pub struct ContextTemplatesConfig {
    /// Whether context templates feature is enabled.
    pub enabled: bool,
    /// Default output format for templates.
    pub default_format: crate::models::OutputFormat,
    /// Per-hook template configuration.
    pub hooks: HookTemplatesConfig,
}

impl Default for ContextTemplatesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_format: crate::models::OutputFormat::Markdown,
            hooks: HookTemplatesConfig::default(),
        }
    }
}

impl ContextTemplatesConfig {
    /// Creates config from a config file section.
    pub fn from_config_file(file: &ConfigFileContextTemplates) -> Self {
        let default_format = file
            .default_format
            .as_deref()
            .and_then(parse_output_format)
            .unwrap_or(crate::models::OutputFormat::Markdown);

        let hooks = file
            .hooks
            .as_ref()
            .map(HookTemplatesConfig::from_config_file)
            .unwrap_or_default();

        Self {
            enabled: file.enabled.unwrap_or(true),
            default_format,
            hooks,
        }
    }
}

/// Runtime per-hook template configuration.
#[derive(Debug, Clone, Default)]
pub struct HookTemplatesConfig {
    /// Template for `session_start` hook.
    pub session_start: Option<HookTemplateConfig>,
    /// Template for `user_prompt_submit` hook.
    pub user_prompt_submit: Option<HookTemplateConfig>,
    /// Template for `post_tool_use` hook.
    pub post_tool_use: Option<HookTemplateConfig>,
    /// Template for `pre_compact` hook.
    pub pre_compact: Option<HookTemplateConfig>,
}

impl HookTemplatesConfig {
    /// Creates config from a config file section.
    pub fn from_config_file(file: &ConfigFileHookTemplates) -> Self {
        Self {
            session_start: file
                .session_start
                .as_ref()
                .map(HookTemplateConfig::from_config_file),
            user_prompt_submit: file
                .user_prompt_submit
                .as_ref()
                .map(HookTemplateConfig::from_config_file),
            post_tool_use: file
                .post_tool_use
                .as_ref()
                .map(HookTemplateConfig::from_config_file),
            pre_compact: file
                .pre_compact
                .as_ref()
                .map(HookTemplateConfig::from_config_file),
        }
    }
}

/// Runtime configuration for a specific hook's template.
#[derive(Debug, Clone)]
pub struct HookTemplateConfig {
    /// Name of the template to use.
    pub template: String,
    /// Specific version to use (None = latest).
    pub version: Option<u32>,
    /// Output format override.
    pub format: Option<crate::models::OutputFormat>,
}

impl HookTemplateConfig {
    /// Creates config from a config file section.
    pub fn from_config_file(file: &ConfigFileHookTemplate) -> Self {
        Self {
            template: file.template.clone().unwrap_or_default(),
            version: file.version,
            format: file.format.as_deref().and_then(parse_output_format),
        }
    }
}

/// Parses output format from string.
fn parse_output_format(s: &str) -> Option<crate::models::OutputFormat> {
    match s.to_lowercase().as_str() {
        "markdown" | "md" => Some(crate::models::OutputFormat::Markdown),
        "json" => Some(crate::models::OutputFormat::Json),
        "xml" => Some(crate::models::OutputFormat::Xml),
        _ => None,
    }
}

/// Runtime TTL (Time-To-Live) configuration.
///
/// Controls memory expiration with domain-scoped and per-namespace defaults.
/// TTL values are stored in seconds (0 means no expiration).
///
/// # Defaults
///
/// - `default_seconds`: None (no expiration)
/// - All namespace/scope overrides: None (inherit from default)
///
/// # Environment Variables
///
/// | Variable | Description | Example |
/// |----------|-------------|---------|
/// | `SUBCOG_TTL_DEFAULT` | Default TTL | "30d", "0" |
///
/// # Priority Order (highest to lowest)
///
/// 1. Explicit `--ttl` flag on capture command
/// 2. Per-namespace TTL (e.g., `ttl.namespace.context = "7d"`)
/// 3. Per-scope TTL (e.g., `ttl.scope.project = "30d"`)
/// 4. Global default TTL (e.g., `ttl.default = "30d"`)
/// 5. No expiration (if nothing configured)
#[derive(Debug, Clone, Default)]
pub struct TtlConfig {
    /// Default TTL in seconds for all memories (None = no expiration, 0 = no expiration).
    pub default_seconds: Option<u64>,
    /// Per-namespace TTL overrides in seconds.
    pub namespace: TtlNamespaceConfig,
    /// Per-scope TTL overrides in seconds.
    pub scope: TtlScopeConfig,
}

/// Per-namespace TTL configuration (runtime).
#[derive(Debug, Clone, Default)]
pub struct TtlNamespaceConfig {
    /// TTL for decisions namespace in seconds.
    pub decisions: Option<u64>,
    /// TTL for patterns namespace in seconds.
    pub patterns: Option<u64>,
    /// TTL for learnings namespace in seconds.
    pub learnings: Option<u64>,
    /// TTL for context namespace in seconds.
    pub context: Option<u64>,
    /// TTL for tech-debt namespace in seconds.
    pub tech_debt: Option<u64>,
    /// TTL for apis namespace in seconds.
    pub apis: Option<u64>,
    /// TTL for config namespace in seconds.
    pub config: Option<u64>,
    /// TTL for security namespace in seconds.
    pub security: Option<u64>,
    /// TTL for performance namespace in seconds.
    pub performance: Option<u64>,
    /// TTL for testing namespace in seconds.
    pub testing: Option<u64>,
}

/// Per-scope TTL configuration (runtime).
#[derive(Debug, Clone, Default)]
pub struct TtlScopeConfig {
    /// TTL for project-scoped memories in seconds.
    pub project: Option<u64>,
    /// TTL for user-scoped memories in seconds.
    pub user: Option<u64>,
    /// TTL for org-scoped memories in seconds.
    pub org: Option<u64>,
}

impl TtlConfig {
    /// Creates a new TTL configuration with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates configuration from config file settings.
    #[must_use]
    pub fn from_config_file(file: &ConfigFileTtl) -> Self {
        let mut config = Self::default();

        if let Some(ref default) = file.default {
            config.default_seconds = parse_duration_to_seconds(default);
        }

        if let Some(ref ns) = file.namespace {
            config.namespace = TtlNamespaceConfig::from_config_file(ns);
        }

        if let Some(ref scope) = file.scope {
            config.scope = TtlScopeConfig::from_config_file(scope);
        }

        config
    }

    /// Loads configuration from environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        Self::default().with_env_overrides()
    }

    /// Applies environment variable overrides.
    #[must_use]
    pub fn with_env_overrides(mut self) -> Self {
        if let Ok(v) = std::env::var("SUBCOG_TTL_DEFAULT") {
            self.default_seconds = parse_duration_to_seconds(&v);
        }
        self
    }

    /// Gets the effective TTL in seconds for a given namespace and scope.
    ///
    /// Returns `None` if no TTL is configured (memory never expires).
    /// Returns `Some(0)` if explicitly set to never expire.
    ///
    /// Priority order:
    /// 1. Per-namespace TTL
    /// 2. Per-scope TTL
    /// 3. Global default TTL
    #[must_use]
    pub fn get_ttl_seconds(&self, namespace: &str, scope: &str) -> Option<u64> {
        // Check namespace-specific TTL first
        let ns_ttl = match namespace.to_lowercase().as_str() {
            "decisions" => self.namespace.decisions,
            "patterns" => self.namespace.patterns,
            "learnings" => self.namespace.learnings,
            "context" => self.namespace.context,
            "tech-debt" | "tech_debt" => self.namespace.tech_debt,
            "apis" => self.namespace.apis,
            "config" => self.namespace.config,
            "security" => self.namespace.security,
            "performance" => self.namespace.performance,
            "testing" => self.namespace.testing,
            _ => None,
        };

        if ns_ttl.is_some() {
            return ns_ttl;
        }

        // Check scope-specific TTL
        let scope_ttl = match scope.to_lowercase().as_str() {
            "project" => self.scope.project,
            "user" => self.scope.user,
            "org" => self.scope.org,
            _ => None,
        };

        if scope_ttl.is_some() {
            return scope_ttl;
        }

        // Fall back to global default
        self.default_seconds
    }

    /// Sets the default TTL in seconds.
    #[must_use]
    pub const fn with_default_seconds(mut self, seconds: Option<u64>) -> Self {
        self.default_seconds = seconds;
        self
    }
}

impl TtlNamespaceConfig {
    /// Creates configuration from config file settings.
    #[must_use]
    pub fn from_config_file(file: &ConfigFileTtlNamespace) -> Self {
        Self {
            decisions: file
                .decisions
                .as_ref()
                .and_then(|s| parse_duration_to_seconds(s)),
            patterns: file
                .patterns
                .as_ref()
                .and_then(|s| parse_duration_to_seconds(s)),
            learnings: file
                .learnings
                .as_ref()
                .and_then(|s| parse_duration_to_seconds(s)),
            context: file
                .context
                .as_ref()
                .and_then(|s| parse_duration_to_seconds(s)),
            tech_debt: file
                .tech_debt
                .as_ref()
                .and_then(|s| parse_duration_to_seconds(s)),
            apis: file
                .apis
                .as_ref()
                .and_then(|s| parse_duration_to_seconds(s)),
            config: file
                .config
                .as_ref()
                .and_then(|s| parse_duration_to_seconds(s)),
            security: file
                .security
                .as_ref()
                .and_then(|s| parse_duration_to_seconds(s)),
            performance: file
                .performance
                .as_ref()
                .and_then(|s| parse_duration_to_seconds(s)),
            testing: file
                .testing
                .as_ref()
                .and_then(|s| parse_duration_to_seconds(s)),
        }
    }
}

impl TtlScopeConfig {
    /// Creates configuration from config file settings.
    #[must_use]
    pub fn from_config_file(file: &ConfigFileTtlScope) -> Self {
        Self {
            project: file
                .project
                .as_ref()
                .and_then(|s| parse_duration_to_seconds(s)),
            user: file
                .user
                .as_ref()
                .and_then(|s| parse_duration_to_seconds(s)),
            org: file.org.as_ref().and_then(|s| parse_duration_to_seconds(s)),
        }
    }
}

/// Parses a duration string to seconds.
///
/// Supported formats:
/// - "0" or "" - No expiration (returns `Some(0)`)
/// - "30d" - 30 days
/// - "7d" - 7 days
/// - "24h" - 24 hours
/// - "60m" - 60 minutes
/// - "3600s" or "3600" - 3600 seconds
///
/// # Returns
///
/// - `Some(0)` for "0" or empty string (explicitly no expiration)
/// - `Some(seconds)` for valid duration strings
/// - `None` for invalid formats (caller should use default)
#[must_use]
pub fn parse_duration_to_seconds(s: &str) -> Option<u64> {
    let s = s.trim();

    // Empty or "0" means no expiration
    if s.is_empty() || s == "0" {
        return Some(0);
    }

    // Try to parse as pure number (seconds)
    if let Ok(secs) = s.parse::<u64>() {
        return Some(secs);
    }

    // Parse duration with suffix
    let (num_str, multiplier) = if let Some(num) = s.strip_suffix('d') {
        (num, 86400u64) // days -> seconds
    } else if let Some(num) = s.strip_suffix('h') {
        (num, 3600u64) // hours -> seconds
    } else if let Some(num) = s.strip_suffix('m') {
        (num, 60u64) // minutes -> seconds
    } else if let Some(num) = s.strip_suffix('s') {
        (num, 1u64) // seconds
    } else {
        // Unknown format
        tracing::warn!(duration = %s, "Invalid TTL duration format, expected Nd/Nh/Nm/Ns");
        return None;
    };

    num_str.trim().parse::<u64>().map_or_else(
        |_| {
            tracing::warn!(duration = %s, "Invalid TTL duration number");
            None
        },
        |num| Some(num.saturating_mul(multiplier)),
    )
}

/// Consolidation configuration section in config file.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigFileConsolidation {
    /// Whether consolidation is enabled.
    pub enabled: Option<bool>,
    /// Filter to specific namespaces (None = all namespaces).
    pub namespace_filter: Option<Vec<String>>,
    /// Time window in days for consolidation (None = no time limit).
    pub time_window_days: Option<u32>,
    /// Minimum number of memories required to trigger consolidation.
    pub min_memories_to_consolidate: Option<usize>,
    /// Similarity threshold for grouping related memories (0.0-1.0).
    pub similarity_threshold: Option<f32>,
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

/// Storage configuration section in config file.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigFileStorage {
    /// Project-scoped storage configuration.
    pub project: Option<ConfigFileStorageBackend>,
    /// User-scoped storage configuration.
    pub user: Option<ConfigFileStorageBackend>,
    /// Organization-scoped storage configuration.
    pub org: Option<ConfigFileStorageBackend>,
}

/// Storage backend configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigFileStorageBackend {
    /// Backend type: sqlite, filesystem, postgresql, redis.
    pub backend: Option<String>,
    /// Path for file-based backends (sqlite, filesystem).
    pub path: Option<String>,
    /// Connection string for database backends (postgresql).
    pub connection_string: Option<String>,
    /// Redis URL for redis backend.
    pub redis_url: Option<String>,
    /// Enable encryption at rest (COMP-CRIT-002).
    /// Defaults to true when not specified.
    pub encryption_enabled: Option<bool>,
}

/// Runtime storage configuration.
#[derive(Debug, Clone, Default)]
pub struct StorageConfig {
    /// Project storage settings.
    pub project: StorageBackendConfig,
    /// User storage settings.
    pub user: StorageBackendConfig,
    /// Org storage settings.
    pub org: StorageBackendConfig,
}

/// Runtime storage backend configuration.
#[derive(Debug, Clone)]
pub struct StorageBackendConfig {
    /// Backend type.
    pub backend: StorageBackendType,
    /// Path for file-based backends.
    pub path: Option<String>,
    /// Connection string for database backends.
    pub connection_string: Option<String>,
    /// Maximum connection pool size for database backends (PostgreSQL).
    /// Defaults to 20 if not specified.
    pub pool_max_size: Option<usize>,
    /// Enable encryption at rest (COMP-CRIT-002).
    /// Defaults to true for security-by-default.
    pub encryption_enabled: bool,
}

impl Default for StorageBackendConfig {
    fn default() -> Self {
        Self {
            backend: StorageBackendType::default(),
            path: None,
            connection_string: None,
            pool_max_size: None,
            // COMP-CRIT-002: Enable encryption by default for security
            encryption_enabled: true,
        }
    }
}

/// Storage backend types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StorageBackendType {
    /// `SQLite` database (default, authoritative storage).
    #[default]
    Sqlite,
    /// Filesystem (fallback).
    Filesystem,
    /// PostgreSQL.
    PostgreSQL,
    /// Redis.
    Redis,
}

impl StorageBackendType {
    /// Parses a backend type from string.
    ///
    /// Defaults to `Sqlite` for unknown values.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "filesystem" | "fs" | "file" => Self::Filesystem,
            "postgresql" | "postgres" | "pg" => Self::PostgreSQL,
            "redis" => Self::Redis,
            // sqlite is the default for any unrecognized value
            _ => Self::Sqlite,
        }
    }
}

/// Operation-level timeout configuration (CHAOS-HIGH-005).
///
/// Provides configurable timeouts for different operation types to prevent
/// indefinite blocking and ensure predictable latency behavior.
///
/// # Environment Variables
///
/// | Variable | Description | Default |
/// |----------|-------------|---------|
/// | `SUBCOG_TIMEOUT_DEFAULT_MS` | Default timeout for all operations | 30000 |
/// | `SUBCOG_TIMEOUT_CAPTURE_MS` | Capture operation timeout | 30000 |
/// | `SUBCOG_TIMEOUT_RECALL_MS` | Recall/search operation timeout | 30000 |
/// | `SUBCOG_TIMEOUT_SYNC_MS` | Git sync operation timeout | 60000 |
/// | `SUBCOG_TIMEOUT_EMBED_MS` | Embedding generation timeout | 30000 |
/// | `SUBCOG_TIMEOUT_REDIS_MS` | Redis operation timeout | 5000 |
/// | `SUBCOG_TIMEOUT_SQLITE_MS` | `SQLite` operation timeout | 5000 |
/// | `SUBCOG_TIMEOUT_POSTGRES_MS` | PostgreSQL operation timeout | 10000 |
#[derive(Debug, Clone)]
pub struct OperationTimeoutConfig {
    /// Default timeout in milliseconds for all operations.
    pub default_ms: u64,
    /// Timeout for capture operations in milliseconds.
    pub capture_ms: u64,
    /// Timeout for recall/search operations in milliseconds.
    pub recall_ms: u64,
    /// Timeout for sync operations in milliseconds.
    pub sync_ms: u64,
    /// Timeout for embedding operations in milliseconds.
    pub embed_ms: u64,
    /// Timeout for Redis operations in milliseconds.
    pub redis_ms: u64,
    /// Timeout for `SQLite` operations in milliseconds.
    pub sqlite_ms: u64,
    /// Timeout for PostgreSQL operations in milliseconds.
    pub postgres_ms: u64,
}

impl Default for OperationTimeoutConfig {
    fn default() -> Self {
        Self {
            default_ms: 30_000,
            capture_ms: 30_000,
            recall_ms: 30_000,
            sync_ms: 60_000, // Sync can be slower
            embed_ms: 30_000,
            redis_ms: 5_000,
            sqlite_ms: 5_000,
            postgres_ms: 10_000,
        }
    }
}

impl OperationTimeoutConfig {
    /// Creates a new configuration with default values.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            default_ms: 30_000,
            capture_ms: 30_000,
            recall_ms: 30_000,
            sync_ms: 60_000,
            embed_ms: 30_000,
            redis_ms: 5_000,
            sqlite_ms: 5_000,
            postgres_ms: 10_000,
        }
    }

    /// Loads configuration from environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        Self::new().with_env_overrides()
    }

    /// Applies environment variable overrides.
    #[must_use]
    pub fn with_env_overrides(mut self) -> Self {
        if let Ok(v) = std::env::var("SUBCOG_TIMEOUT_DEFAULT_MS")
            && let Ok(parsed) = v.parse::<u64>()
        {
            self.default_ms = parsed.max(100); // Minimum 100ms
        }
        if let Ok(v) = std::env::var("SUBCOG_TIMEOUT_CAPTURE_MS")
            && let Ok(parsed) = v.parse::<u64>()
        {
            self.capture_ms = parsed.max(100);
        }
        if let Ok(v) = std::env::var("SUBCOG_TIMEOUT_RECALL_MS")
            && let Ok(parsed) = v.parse::<u64>()
        {
            self.recall_ms = parsed.max(100);
        }
        if let Ok(v) = std::env::var("SUBCOG_TIMEOUT_SYNC_MS")
            && let Ok(parsed) = v.parse::<u64>()
        {
            self.sync_ms = parsed.max(100);
        }
        if let Ok(v) = std::env::var("SUBCOG_TIMEOUT_EMBED_MS")
            && let Ok(parsed) = v.parse::<u64>()
        {
            self.embed_ms = parsed.max(100);
        }
        if let Ok(v) = std::env::var("SUBCOG_TIMEOUT_REDIS_MS")
            && let Ok(parsed) = v.parse::<u64>()
        {
            self.redis_ms = parsed.max(100);
        }
        if let Ok(v) = std::env::var("SUBCOG_TIMEOUT_SQLITE_MS")
            && let Ok(parsed) = v.parse::<u64>()
        {
            self.sqlite_ms = parsed.max(100);
        }
        if let Ok(v) = std::env::var("SUBCOG_TIMEOUT_POSTGRES_MS")
            && let Ok(parsed) = v.parse::<u64>()
        {
            self.postgres_ms = parsed.max(100);
        }
        self
    }

    /// Gets the timeout for a specific operation type.
    #[must_use]
    pub const fn get(&self, operation: OperationType) -> std::time::Duration {
        let ms = match operation {
            OperationType::Capture => self.capture_ms,
            OperationType::Recall => self.recall_ms,
            OperationType::Sync => self.sync_ms,
            OperationType::Embed => self.embed_ms,
            OperationType::Redis => self.redis_ms,
            OperationType::Sqlite => self.sqlite_ms,
            OperationType::Postgres => self.postgres_ms,
            OperationType::Default => self.default_ms,
        };
        std::time::Duration::from_millis(ms)
    }

    /// Builder method to set default timeout.
    #[must_use]
    pub const fn with_default_ms(mut self, ms: u64) -> Self {
        self.default_ms = ms;
        self
    }

    /// Builder method to set capture timeout.
    #[must_use]
    pub const fn with_capture_ms(mut self, ms: u64) -> Self {
        self.capture_ms = ms;
        self
    }

    /// Builder method to set recall timeout.
    #[must_use]
    pub const fn with_recall_ms(mut self, ms: u64) -> Self {
        self.recall_ms = ms;
        self
    }

    /// Builder method to set sync timeout.
    #[must_use]
    pub const fn with_sync_ms(mut self, ms: u64) -> Self {
        self.sync_ms = ms;
        self
    }

    /// Builder method to set embed timeout.
    #[must_use]
    pub const fn with_embed_ms(mut self, ms: u64) -> Self {
        self.embed_ms = ms;
        self
    }

    /// Builder method to set Redis timeout.
    #[must_use]
    pub const fn with_redis_ms(mut self, ms: u64) -> Self {
        self.redis_ms = ms;
        self
    }

    /// Builder method to set `SQLite` timeout.
    #[must_use]
    pub const fn with_sqlite_ms(mut self, ms: u64) -> Self {
        self.sqlite_ms = ms;
        self
    }

    /// Builder method to set PostgreSQL timeout.
    #[must_use]
    pub const fn with_postgres_ms(mut self, ms: u64) -> Self {
        self.postgres_ms = ms;
        self
    }
}

/// Operation types for timeout configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    /// Memory capture operations.
    Capture,
    /// Memory recall/search operations.
    Recall,
    /// Git sync operations.
    Sync,
    /// Embedding generation.
    Embed,
    /// Redis operations.
    Redis,
    /// `SQLite` operations.
    Sqlite,
    /// PostgreSQL operations.
    Postgres,
    /// Default/fallback timeout.
    Default,
}

impl StorageConfig {
    /// Creates storage config from config file settings.
    #[must_use]
    pub fn from_config_file(file: &ConfigFileStorage) -> Self {
        let mut config = Self::default();

        if let Some(ref project) = file.project {
            if let Some(ref backend) = project.backend {
                config.project.backend = StorageBackendType::parse(backend);
            }
            config.project.path = project.path.as_ref().map(|path| expand_config_path(path));
            config
                .project
                .connection_string
                .clone_from(&project.connection_string);
            // COMP-CRIT-002: Allow explicit override, default is true
            if let Some(encryption) = project.encryption_enabled {
                config.project.encryption_enabled = encryption;
            }
        }

        if let Some(ref user) = file.user {
            if let Some(ref backend) = user.backend {
                config.user.backend = StorageBackendType::parse(backend);
            }
            config.user.path = user.path.as_ref().map(|path| expand_config_path(path));
            config
                .user
                .connection_string
                .clone_from(&user.connection_string);
            // COMP-CRIT-002: Allow explicit override, default is true
            if let Some(encryption) = user.encryption_enabled {
                config.user.encryption_enabled = encryption;
            }
        }

        if let Some(ref org) = file.org {
            if let Some(ref backend) = org.backend {
                config.org.backend = StorageBackendType::parse(backend);
            }
            config.org.path = org.path.as_ref().map(|path| expand_config_path(path));
            config
                .org
                .connection_string
                .clone_from(&org.connection_string);
            // COMP-CRIT-002: Allow explicit override, default is true
            if let Some(encryption) = org.encryption_enabled {
                config.org.encryption_enabled = encryption;
            }
        }

        config
    }
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

/// Runtime consolidation configuration.
///
/// Controls LLM-powered memory consolidation that summarizes related memories
/// while preserving original details.
///
/// # Defaults
///
/// - `enabled`: false (requires LLM provider to be useful)
/// - `namespace_filter`: None (all namespaces)
/// - `time_window_days`: Some(30) (last 30 days)
/// - `min_memories_to_consolidate`: 3 (need at least 3 related memories)
/// - `similarity_threshold`: 0.7 (70% semantic similarity)
///
/// # Environment Variables
///
/// | Variable | Description | Default |
/// |----------|-------------|---------|
/// | `SUBCOG_CONSOLIDATION_ENABLED` | Enable consolidation | false |
/// | `SUBCOG_CONSOLIDATION_TIME_WINDOW_DAYS` | Time window in days | 30 |
/// | `SUBCOG_CONSOLIDATION_MIN_MEMORIES` | Minimum memories to consolidate | 3 |
/// | `SUBCOG_CONSOLIDATION_SIMILARITY_THRESHOLD` | Similarity threshold (0.0-1.0) | 0.7 |
#[derive(Debug, Clone)]
pub struct ConsolidationConfig {
    /// Whether consolidation is enabled.
    pub enabled: bool,
    /// Filter to specific namespaces (None = all namespaces).
    pub namespace_filter: Option<Vec<crate::models::Namespace>>,
    /// Time window in days for consolidation (None = no time limit).
    pub time_window_days: Option<u32>,
    /// Minimum number of memories required to trigger consolidation.
    pub min_memories_to_consolidate: usize,
    /// Similarity threshold for grouping related memories (0.0-1.0).
    pub similarity_threshold: f32,
}

impl Default for ConsolidationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            namespace_filter: None,
            time_window_days: Some(30),
            min_memories_to_consolidate: 3,
            similarity_threshold: 0.7,
        }
    }
}

impl ConsolidationConfig {
    /// Creates a new consolidation configuration with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates configuration from config file settings.
    #[must_use]
    pub fn from_config_file(file: &ConfigFileConsolidation) -> Self {
        let mut config = Self::default();

        if let Some(enabled) = file.enabled {
            config.enabled = enabled;
        }

        if let Some(ref namespace_filter) = file.namespace_filter {
            let namespaces: Vec<crate::models::Namespace> = namespace_filter
                .iter()
                .filter_map(|s| s.parse().ok())
                .collect();
            if !namespaces.is_empty() {
                config.namespace_filter = Some(namespaces);
            }
        }

        if let Some(time_window_days) = file.time_window_days {
            config.time_window_days = Some(time_window_days);
        }

        if let Some(min_memories) = file.min_memories_to_consolidate {
            config.min_memories_to_consolidate = min_memories.max(2); // At least 2 memories
        }

        if let Some(threshold) = file.similarity_threshold {
            config.similarity_threshold = threshold.clamp(0.0, 1.0);
        }

        config
    }

    /// Loads configuration from environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        Self::default().with_env_overrides()
    }

    /// Applies environment variable overrides.
    #[must_use]
    pub fn with_env_overrides(mut self) -> Self {
        if let Ok(v) = std::env::var("SUBCOG_CONSOLIDATION_ENABLED") {
            self.enabled = v.to_lowercase() == "true" || v == "1";
        }

        if let Ok(v) = std::env::var("SUBCOG_CONSOLIDATION_TIME_WINDOW_DAYS")
            && let Ok(days) = v.parse::<u32>()
        {
            self.time_window_days = Some(days);
        }

        if let Ok(v) = std::env::var("SUBCOG_CONSOLIDATION_MIN_MEMORIES")
            && let Ok(min) = v.parse::<usize>()
        {
            self.min_memories_to_consolidate = min.max(2);
        }

        if let Ok(v) = std::env::var("SUBCOG_CONSOLIDATION_SIMILARITY_THRESHOLD")
            && let Ok(threshold) = v.parse::<f32>()
        {
            self.similarity_threshold = threshold.clamp(0.0, 1.0);
        }

        self
    }

    /// Sets whether consolidation is enabled.
    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Sets the namespace filter.
    #[must_use]
    pub fn with_namespace_filter(mut self, filter: Vec<crate::models::Namespace>) -> Self {
        self.namespace_filter = Some(filter);
        self
    }

    /// Sets the time window in days.
    #[must_use]
    pub const fn with_time_window_days(mut self, days: Option<u32>) -> Self {
        self.time_window_days = days;
        self
    }

    /// Sets the minimum number of memories to consolidate.
    #[must_use]
    pub const fn with_min_memories(mut self, min: usize) -> Self {
        self.min_memories_to_consolidate = min;
        self
    }

    /// Sets the similarity threshold.
    ///
    /// Value is clamped to the range [0.0, 1.0].
    #[must_use]
    pub const fn with_similarity_threshold(mut self, threshold: f32) -> Self {
        self.similarity_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Validates the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `min_memories_to_consolidate` is less than 2
    /// - `similarity_threshold` is not in range [0.0, 1.0]
    /// - `time_window_days` is 0 (if set)
    pub fn build(self) -> Result<Self, ConfigValidationError> {
        if self.min_memories_to_consolidate < 2 {
            return Err(ConfigValidationError::InvalidValue {
                field: "min_memories_to_consolidate".to_string(),
                message: "min_memories_to_consolidate must be at least 2".to_string(),
            });
        }

        if !(0.0..=1.0).contains(&self.similarity_threshold) {
            return Err(ConfigValidationError::InvalidValue {
                field: "similarity_threshold".to_string(),
                message: format!(
                    "similarity_threshold must be in range [0.0, 1.0], got {}",
                    self.similarity_threshold
                ),
            });
        }

        if let Some(days) = self.time_window_days
            && days == 0
        {
            return Err(ConfigValidationError::InvalidValue {
                field: "time_window_days".to_string(),
                message: "time_window_days must be greater than 0".to_string(),
            });
        }

        Ok(self)
    }
}

impl PromptConfig {
    /// Creates a new prompt configuration from config file settings.
    #[must_use]
    pub fn from_config_file(file: &ConfigFilePrompt) -> Self {
        Self {
            identity_addendum: file.identity_addendum.clone(),
            additional_guidance: file.additional_guidance.clone(),
            operation_guidance: PromptOperationConfig {
                capture: file
                    .capture
                    .as_ref()
                    .and_then(|c| c.additional_guidance.clone()),
                search: file
                    .search
                    .as_ref()
                    .and_then(|c| c.additional_guidance.clone()),
                enrichment: file
                    .enrichment
                    .as_ref()
                    .and_then(|c| c.additional_guidance.clone()),
                consolidation: file
                    .consolidation
                    .as_ref()
                    .and_then(|c| c.additional_guidance.clone()),
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
            data_dir: directories::BaseDirs::new()
                .map_or_else(|| PathBuf::from("."), |b| b.data_local_dir().join("subcog")),
            features: FeatureFlags::default(),
            max_results: 10,
            default_search_mode: crate::models::SearchMode::Hybrid,
            llm: LlmConfig::default(),
            search_intent: SearchIntentConfig::default(),
            observability: ObservabilitySettings::default(),
            prompt: PromptConfig::default(),
            storage: StorageConfig::default(),
            consolidation: ConsolidationConfig::default(),
            ttl: TtlConfig::default(),
            timeouts: OperationTimeoutConfig::from_env(),
            context_templates: ContextTemplatesConfig::default(),
            org: OrgConfig::default(),
            webhooks: WebhooksConfig::default(),
            config_sources: Vec::new(),
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
        // SEC-M4: Warn if config file is world-readable (may contain API keys)
        warn_if_world_readable(path);

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
        config.config_sources.push(path.to_path_buf());
        config.apply_env_overrides();
        Ok(config)
    }

    /// Loads configuration from the default location.
    ///
    /// Config location: `~/.config/subcog/config.toml`
    /// Data location: `~/.config/subcog/`
    ///
    /// Returns default configuration if no config file is found.
    #[must_use]
    pub fn load_default() -> Self {
        let Some(base_dirs) = directories::BaseDirs::new() else {
            let mut config = Self::default();
            config.apply_env_overrides();
            return config;
        };

        // Single config location: ~/.config/subcog/
        let config_dir = base_dirs.home_dir().join(".config").join("subcog");

        let mut config = Self {
            data_dir: config_dir.clone(),
            ..Self::default()
        };

        let config_path = config_dir.join("config.toml");
        if apply_config_path(&mut config, &config_path) {
            config.config_sources.push(config_path);
        }

        config.apply_env_overrides();
        config
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(value) = std::env::var("SUBCOG_ORG_SCOPE_ENABLED") {
            let Some(enabled) = parse_bool_env(&value) else {
                self.features.org_scope_enabled = false;
                tracing::warn!(
                    value = %value,
                    "Invalid SUBCOG_ORG_SCOPE_ENABLED value, defaulting to false"
                );
                return;
            };
            self.features.org_scope_enabled = enabled;
            if enabled {
                tracing::info!("Org-scope enabled via SUBCOG_ORG_SCOPE_ENABLED");
            }
        }

        // COMP-CRIT-002: Allow env var override for encryption (applies to all scopes)
        if let Ok(value) = std::env::var("SUBCOG_STORAGE_ENCRYPTION_ENABLED") {
            if let Some(enabled) = parse_bool_env(&value) {
                self.storage.project.encryption_enabled = enabled;
                self.storage.user.encryption_enabled = enabled;
                self.storage.org.encryption_enabled = enabled;
                tracing::info!(
                    enabled = enabled,
                    "Storage encryption configured via SUBCOG_STORAGE_ENCRYPTION_ENABLED"
                );
            } else {
                tracing::warn!(
                    value = %value,
                    "Invalid SUBCOG_STORAGE_ENCRYPTION_ENABLED value, keeping default (true)"
                );
            }
        }

        // Auto-extract entities during capture (for graph-augmented retrieval)
        if let Ok(value) = std::env::var("SUBCOG_AUTO_EXTRACT_ENTITIES") {
            if let Some(enabled) = parse_bool_env(&value) {
                self.features.auto_extract_entities = enabled;
                tracing::info!(
                    enabled = enabled,
                    "Auto entity extraction configured via SUBCOG_AUTO_EXTRACT_ENTITIES"
                );
            } else {
                tracing::warn!(
                    value = %value,
                    "Invalid SUBCOG_AUTO_EXTRACT_ENTITIES value, keeping default (false)"
                );
            }
        }

        self.search_intent = self.search_intent.clone().with_env_overrides();
        self.prompt = self.prompt.clone().with_env_overrides();
        self.consolidation = self.consolidation.clone().with_env_overrides();
        self.ttl = self.ttl.clone().with_env_overrides();
    }

    /// Applies a `ConfigFile` to the current configuration.
    ///
    /// ARCH-HIGH-002: Delegates to sub-config `merge_from`/`from_config_file` methods.
    fn apply_config_file(&mut self, file: ConfigFile) {
        // Core settings
        if let Some(repo_path) = file.repo_path {
            self.repo_path = PathBuf::from(expand_config_path(&repo_path));
        }
        if let Some(data_dir) = file.data_dir {
            self.data_dir = PathBuf::from(expand_config_path(&data_dir));
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

        // Delegate to sub-config types (ARCH-HIGH-002)
        if let Some(ref features) = file.features {
            self.features.merge_from(features);
        }
        if let Some(ref llm) = file.llm {
            self.llm.merge_from(llm);
        }
        if let Some(ref search_intent) = file.search_intent {
            self.search_intent = SearchIntentConfig::from_config_file(search_intent);
        }
        if let Some(observability) = file.observability {
            self.observability = observability;
        }
        if let Some(ref prompt) = file.prompt {
            self.prompt = PromptConfig::from_config_file(prompt);
        }
        if let Some(ref storage) = file.storage {
            self.storage = StorageConfig::from_config_file(storage);
        }
        if let Some(ref consolidation) = file.consolidation {
            self.consolidation = ConsolidationConfig::from_config_file(consolidation);
        }
        if let Some(ref ttl) = file.ttl {
            self.ttl = TtlConfig::from_config_file(ttl);
        }
        if let Some(ref context_templates) = file.context_templates {
            self.context_templates = ContextTemplatesConfig::from_config_file(context_templates);
        }
        if let Some(ref org) = file.org {
            self.org = OrgConfig::from_config_file(org, self.features.org_scope_enabled);
        }

        // Webhooks from [[webhooks]] array
        if !file.webhooks.is_empty() {
            self.webhooks = WebhooksConfig::from_config_file(file.webhooks);
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
    match load_config_file(path) {
        Ok(file) => {
            tracing::debug!(path = %path.display(), "Loaded config file");
            config.apply_config_file(file);
            true
        },
        Err(e) => {
            // Log error so config parsing issues are visible
            tracing::warn!(path = %path.display(), error = %e, "Failed to load config file");
            false
        },
    }
}

fn load_config_file(path: &std::path::Path) -> crate::Result<ConfigFile> {
    // SEC-M4: Warn if config file is world-readable (may contain API keys)
    warn_if_world_readable(path);

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
#[allow(clippy::struct_excessive_bools)]
pub struct ServiceFeatures {
    /// Whether to block content with secrets.
    pub block_secrets: bool,
    /// Whether to redact secrets.
    pub redact_secrets: bool,
    /// Whether to enable auto-sync.
    pub auto_sync: bool,
    /// Whether to auto-extract entities during capture.
    pub auto_extract_entities: bool,
}

impl Default for ServiceFeatures {
    fn default() -> Self {
        Self {
            block_secrets: false,
            redact_secrets: true,
            auto_sync: false,
            auto_extract_entities: false,
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
                auto_extract_entities: subcog.features.auto_extract_entities,
            },
        }
    }
}

// =============================================================================
// WEBHOOK CONFIGURATION
// =============================================================================

/// Webhook configuration from config.toml.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigFileWebhook {
    /// Unique name for this webhook.
    pub name: String,
    /// Target URL for webhook delivery.
    pub url: String,
    /// Authentication configuration.
    pub auth: Option<ConfigFileWebhookAuth>,
    /// Event types to subscribe to (empty = all events).
    #[serde(default)]
    pub events: Vec<String>,
    /// Domain scopes to filter (empty = all scopes).
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Whether this webhook is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Retry configuration.
    #[serde(default)]
    pub retry: ConfigFileWebhookRetry,
    /// Payload format (default, slack, discord).
    pub format: Option<String>,
}

/// Webhook authentication from config.toml.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConfigFileWebhookAuth {
    /// Bearer token authentication.
    Bearer {
        /// The bearer token (supports `${ENV_VAR}` expansion).
        token: String,
    },
    /// HMAC-SHA256 signature authentication.
    Hmac {
        /// The shared secret (supports `${ENV_VAR}` expansion).
        secret: String,
    },
    /// Both Bearer token and HMAC signature.
    Both {
        /// The bearer token.
        token: String,
        /// The HMAC secret.
        secret: String,
    },
}

/// Webhook retry configuration from config.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigFileWebhookRetry {
    /// Maximum number of retry attempts (default: 3).
    #[serde(default = "default_webhook_max_retries")]
    pub max_retries: u32,
    /// Base delay in milliseconds for exponential backoff (default: 1000).
    #[serde(default = "default_webhook_base_delay_ms")]
    pub base_delay_ms: u64,
    /// Request timeout in seconds (default: 30).
    #[serde(default = "default_webhook_timeout_secs")]
    pub timeout_secs: u64,
}

impl Default for ConfigFileWebhookRetry {
    fn default() -> Self {
        Self {
            max_retries: default_webhook_max_retries(),
            base_delay_ms: default_webhook_base_delay_ms(),
            timeout_secs: default_webhook_timeout_secs(),
        }
    }
}

const fn default_true() -> bool {
    true
}

const fn default_webhook_max_retries() -> u32 {
    3
}

const fn default_webhook_base_delay_ms() -> u64 {
    1000
}

const fn default_webhook_timeout_secs() -> u64 {
    30
}

/// Runtime webhook configuration.
#[derive(Debug, Clone, Default)]
pub struct WebhooksConfig {
    /// List of configured webhook endpoints.
    pub webhooks: Vec<ConfigFileWebhook>,
}

impl WebhooksConfig {
    /// Creates webhooks config from parsed config file entries.
    #[must_use]
    pub const fn from_config_file(webhooks: Vec<ConfigFileWebhook>) -> Self {
        Self { webhooks }
    }

    /// Returns the number of configured webhooks.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.webhooks.len()
    }

    /// Returns true if no webhooks are configured.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.webhooks.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_env_vars_with_existing_var() {
        // Use HOME which is always set on Unix/macOS
        // On Windows, use USERPROFILE instead
        let var_name = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        if let Ok(expected) = std::env::var(var_name) {
            let input = format!("${{{var_name}}}");
            let result = expand_env_vars(&input);
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_expand_env_vars_with_prefix_suffix() {
        // Use PATH which is always set
        if let Ok(path_value) = std::env::var("PATH") {
            let result = expand_env_vars("prefix-${PATH}-suffix");
            assert_eq!(result, format!("prefix-{path_value}-suffix"));
        }
    }

    #[test]
    fn test_expand_env_vars_no_vars() {
        let result = expand_env_vars("no variables here");
        assert_eq!(result, "no variables here");
    }

    #[test]
    fn test_expand_env_vars_missing_var_preserved() {
        let result = expand_env_vars("${DEFINITELY_NOT_SET_12345_SUBCOG_TEST}");
        assert_eq!(result, "${DEFINITELY_NOT_SET_12345_SUBCOG_TEST}");
    }

    #[test]
    fn test_expand_env_vars_multiple_existing() {
        // Use HOME and PATH which are always set
        let home_var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        if let (Ok(home), Ok(path)) = (std::env::var(home_var), std::env::var("PATH")) {
            let input = format!("${{{home_var}}}:${{PATH}}");
            let result = expand_env_vars(&input);
            assert_eq!(result, format!("{home}:{path}"));
        }
    }

    #[test]
    fn test_expand_env_vars_unclosed_brace() {
        let result = expand_env_vars("${UNCLOSED");
        assert_eq!(result, "${UNCLOSED");
    }

    #[test]
    fn test_expand_env_vars_empty_braces() {
        // Empty var name - should preserve since no var named ""
        let result = expand_env_vars("${}");
        assert_eq!(result, "${}");
    }

    #[test]
    fn test_expand_env_vars_nested_braces() {
        // Nested braces - only outer should be processed
        let result = expand_env_vars("${${INNER}}");
        // First finds ${${INNER} - var name is "${INNER", which won't exist
        assert_eq!(result, "${${INNER}}");
    }

    #[test]
    fn test_expand_config_path_tilde_home() {
        if let Some(base_dirs) = directories::BaseDirs::new() {
            let expected = base_dirs.home_dir().to_path_buf();
            let result = expand_config_path("~");
            assert_eq!(PathBuf::from(result), expected);
        }
    }

    #[test]
    fn test_expand_config_path_tilde_suffix() {
        if let Some(base_dirs) = directories::BaseDirs::new() {
            let expected = base_dirs.home_dir().join(".config/subcog");
            let result = expand_config_path("~/.config/subcog");
            assert_eq!(PathBuf::from(result), expected);
        }
    }

    #[test]
    fn test_expand_config_path_env_var() {
        let var_name = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        if let Ok(home) = std::env::var(var_name) {
            let input = format!("${{{var_name}}}/data");
            let result = expand_config_path(&input);
            assert_eq!(PathBuf::from(result), PathBuf::from(home).join("data"));
        }
    }

    #[test]
    #[cfg(unix)]
    fn test_warn_if_world_readable_does_not_panic() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;

        // Create a temp file with world-readable permissions
        let dir = tempfile::tempdir().ok();
        if let Some(ref dir) = dir {
            let path = dir.path().join("test_config.toml");
            let mut file = std::fs::File::create(&path).ok();
            if let Some(ref mut f) = file {
                let _ = f.write_all(b"[llm]\nprovider = \"anthropic\"\n");
            }

            // Set world-readable permission (0o644)
            if let Ok(metadata) = std::fs::metadata(&path) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o644);
                let _ = std::fs::set_permissions(&path, perms);
            }

            // Function should not panic
            warn_if_world_readable(&path);

            // Also test with restrictive permissions (0o600)
            if let Ok(metadata) = std::fs::metadata(&path) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o600);
                let _ = std::fs::set_permissions(&path, perms);
            }
            warn_if_world_readable(&path);
        }
    }

    #[test]
    fn test_warn_if_world_readable_nonexistent_file() {
        // Should not panic on non-existent file
        let path = Path::new("/nonexistent/path/to/config.toml");
        warn_if_world_readable(path);
    }

    #[test]
    fn test_consolidation_config_defaults() {
        let config = ConsolidationConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.namespace_filter, None);
        assert_eq!(config.time_window_days, Some(30));
        assert_eq!(config.min_memories_to_consolidate, 3);
        assert_eq!(config.similarity_threshold, 0.7);
    }

    #[test]
    fn test_consolidation_config_builder() {
        let config = ConsolidationConfig::new()
            .with_enabled(true)
            .with_time_window_days(Some(60))
            .with_min_memories(5)
            .with_similarity_threshold(0.8);

        assert!(config.enabled);
        assert_eq!(config.time_window_days, Some(60));
        assert_eq!(config.min_memories_to_consolidate, 5);
        assert_eq!(config.similarity_threshold, 0.8);
    }

    #[test]
    fn test_consolidation_config_similarity_threshold_clamping() {
        let config = ConsolidationConfig::new().with_similarity_threshold(1.5); // Above max
        assert_eq!(config.similarity_threshold, 1.0);

        let config = ConsolidationConfig::new().with_similarity_threshold(-0.5); // Below min
        assert_eq!(config.similarity_threshold, 0.0);
    }

    #[test]
    fn test_consolidation_config_validation_min_memories() {
        let config = ConsolidationConfig::new().with_min_memories(1);

        let result = config.build();
        assert!(result.is_err());
        if let Err(ConfigValidationError::InvalidValue { field, .. }) = result {
            assert_eq!(field, "min_memories_to_consolidate");
        }
    }

    #[test]
    fn test_consolidation_config_validation_similarity_threshold() {
        let mut config = ConsolidationConfig::new();
        config.similarity_threshold = 1.5; // Bypass builder clamping

        let result = config.build();
        assert!(result.is_err());
        if let Err(ConfigValidationError::InvalidValue { field, .. }) = result {
            assert_eq!(field, "similarity_threshold");
        }
    }

    #[test]
    fn test_consolidation_config_validation_time_window() {
        let config = ConsolidationConfig::new().with_time_window_days(Some(0));

        let result = config.build();
        assert!(result.is_err());
        if let Err(ConfigValidationError::InvalidValue { field, .. }) = result {
            assert_eq!(field, "time_window_days");
        }
    }

    #[test]
    fn test_consolidation_config_validation_success() {
        let config = ConsolidationConfig::new()
            .with_enabled(true)
            .with_min_memories(3)
            .with_similarity_threshold(0.7);

        let result = config.build();
        assert!(result.is_ok());
    }

    /// Test that `ConsolidationConfig::from_env()` correctly reads environment variables.
    ///
    /// This test is ignored because Rust 2024 edition requires `unsafe` blocks for
    /// `std::env::set_var`/`remove_var`, and this crate forbids unsafe code.
    /// The functionality is still tested via integration tests that can set env vars
    /// before process startup.
    #[test]
    #[ignore = "Rust 2024: set_var/remove_var require unsafe, crate forbids unsafe_code"]
    fn test_consolidation_config_from_env() {
        // This test would verify:
        // - SUBCOG_CONSOLIDATION_ENABLED=true -> config.enabled = true
        // - SUBCOG_CONSOLIDATION_TIME_WINDOW_DAYS=45 -> config.time_window_days = Some(45)
        // - SUBCOG_CONSOLIDATION_MIN_MEMORIES=5 -> config.min_memories_to_consolidate = 5
        // - SUBCOG_CONSOLIDATION_SIMILARITY_THRESHOLD=0.85 -> config.similarity_threshold = 0.85
    }

    #[test]
    fn test_consolidation_config_from_config_file() {
        let file = ConfigFileConsolidation {
            enabled: Some(true),
            namespace_filter: Some(vec!["decisions".to_string(), "patterns".to_string()]),
            time_window_days: Some(60),
            min_memories_to_consolidate: Some(4),
            similarity_threshold: Some(0.8),
        };

        let config = ConsolidationConfig::from_config_file(&file);

        assert!(config.enabled);
        assert_eq!(config.time_window_days, Some(60));
        assert_eq!(config.min_memories_to_consolidate, 4);
        assert_eq!(config.similarity_threshold, 0.8);

        // Check namespace filter was parsed
        assert!(
            config.namespace_filter.is_some(),
            "Expected namespace_filter to be Some"
        );
        let namespaces = config.namespace_filter.as_ref().unwrap();
        assert_eq!(namespaces.len(), 2);
    }

    #[test]
    fn test_consolidation_config_min_memories_enforcement() {
        let file = ConfigFileConsolidation {
            min_memories_to_consolidate: Some(1), // Too low
            ..Default::default()
        };

        let config = ConsolidationConfig::from_config_file(&file);
        // Should be clamped to minimum of 2
        assert_eq!(config.min_memories_to_consolidate, 2);
    }

    #[test]
    fn test_consolidation_config_threshold_clamping_in_from_file() {
        let file = ConfigFileConsolidation {
            similarity_threshold: Some(1.5), // Above max
            ..Default::default()
        };

        let config = ConsolidationConfig::from_config_file(&file);
        assert_eq!(config.similarity_threshold, 1.0);

        let file = ConfigFileConsolidation {
            similarity_threshold: Some(-0.5), // Below min
            ..Default::default()
        };

        let config = ConsolidationConfig::from_config_file(&file);
        assert_eq!(config.similarity_threshold, 0.0);
    }

    // TTL Configuration Tests

    #[test]
    fn test_parse_duration_to_seconds_days() {
        assert_eq!(parse_duration_to_seconds("7d"), Some(7 * 86400));
        assert_eq!(parse_duration_to_seconds("30d"), Some(30 * 86400));
        assert_eq!(parse_duration_to_seconds("365d"), Some(365 * 86400));
    }

    #[test]
    fn test_parse_duration_to_seconds_hours() {
        assert_eq!(parse_duration_to_seconds("24h"), Some(24 * 3600));
        assert_eq!(parse_duration_to_seconds("1h"), Some(3600));
    }

    #[test]
    fn test_parse_duration_to_seconds_minutes() {
        assert_eq!(parse_duration_to_seconds("60m"), Some(3600));
        assert_eq!(parse_duration_to_seconds("5m"), Some(300));
    }

    #[test]
    fn test_parse_duration_to_seconds_seconds() {
        assert_eq!(parse_duration_to_seconds("3600s"), Some(3600));
        assert_eq!(parse_duration_to_seconds("60s"), Some(60));
    }

    #[test]
    fn test_parse_duration_to_seconds_raw_number() {
        assert_eq!(parse_duration_to_seconds("3600"), Some(3600));
        assert_eq!(parse_duration_to_seconds("86400"), Some(86400));
    }

    #[test]
    fn test_parse_duration_to_seconds_zero_and_empty() {
        assert_eq!(parse_duration_to_seconds("0"), Some(0));
        assert_eq!(parse_duration_to_seconds(""), Some(0));
        assert_eq!(parse_duration_to_seconds("  "), Some(0));
    }

    #[test]
    fn test_parse_duration_to_seconds_invalid() {
        assert_eq!(parse_duration_to_seconds("abc"), None);
        assert_eq!(parse_duration_to_seconds("7x"), None);
        assert_eq!(parse_duration_to_seconds("d7"), None);
    }

    #[test]
    fn test_parse_duration_to_seconds_whitespace() {
        assert_eq!(parse_duration_to_seconds(" 7d "), Some(7 * 86400));
        assert_eq!(parse_duration_to_seconds("  30d"), Some(30 * 86400));
    }

    #[test]
    fn test_ttl_config_defaults() {
        let config = TtlConfig::default();
        assert_eq!(config.default_seconds, None);
        assert_eq!(config.namespace.decisions, None);
        assert_eq!(config.scope.project, None);
    }

    #[test]
    fn test_ttl_config_get_ttl_seconds_priority() {
        let config = TtlConfig {
            default_seconds: Some(30 * 86400), // 30 days
            namespace: TtlNamespaceConfig {
                context: Some(7 * 86400), // 7 days for context
                ..Default::default()
            },
            scope: TtlScopeConfig {
                project: Some(14 * 86400), // 14 days for project
                ..Default::default()
            },
        };

        // Namespace-specific TTL takes priority
        assert_eq!(
            config.get_ttl_seconds("context", "project"),
            Some(7 * 86400)
        );

        // Scope-specific TTL used when no namespace TTL
        assert_eq!(
            config.get_ttl_seconds("decisions", "project"),
            Some(14 * 86400)
        );

        // Default TTL used when no namespace or scope TTL
        assert_eq!(
            config.get_ttl_seconds("decisions", "user"),
            Some(30 * 86400)
        );
    }

    #[test]
    fn test_ttl_config_from_config_file() {
        let file = ConfigFileTtl {
            default: Some("30d".to_string()),
            namespace: Some(ConfigFileTtlNamespace {
                decisions: Some("90d".to_string()),
                context: Some("7d".to_string()),
                tech_debt: Some("0".to_string()), // Never expires
                ..Default::default()
            }),
            scope: Some(ConfigFileTtlScope {
                project: Some("30d".to_string()),
                user: Some("90d".to_string()),
                org: Some("365d".to_string()),
            }),
        };

        let config = TtlConfig::from_config_file(&file);

        assert_eq!(config.default_seconds, Some(30 * 86400));
        assert_eq!(config.namespace.decisions, Some(90 * 86400));
        assert_eq!(config.namespace.context, Some(7 * 86400));
        assert_eq!(config.namespace.tech_debt, Some(0)); // 0 = no expiration
        assert_eq!(config.scope.project, Some(30 * 86400));
        assert_eq!(config.scope.user, Some(90 * 86400));
        assert_eq!(config.scope.org, Some(365 * 86400));
    }

    #[test]
    fn test_ttl_config_no_expiration() {
        let config = TtlConfig::default();

        // No TTL configured means None (never expires)
        assert_eq!(config.get_ttl_seconds("decisions", "project"), None);
    }

    #[test]
    fn test_ttl_config_explicit_no_expiration() {
        let config = TtlConfig {
            namespace: TtlNamespaceConfig {
                tech_debt: Some(0), // Explicitly set to never expire
                ..Default::default()
            },
            ..Default::default()
        };

        // Some(0) means explicitly set to never expire
        assert_eq!(config.get_ttl_seconds("tech-debt", "project"), Some(0));
    }
}
