//! Webhook configuration types.
//!
//! Configuration is stored in `~/.config/subcog/config.toml` under `[[webhooks]]`:
//!
//! ```toml
//! [[webhooks]]
//! name = "analytics"
//! url = "https://analytics.example.com/events"
//! events = ["captured", "consolidated"]
//! scopes = ["project"]
//!
//! [webhooks.auth]
//! type = "hmac"
//! secret = "${WEBHOOK_SECRET}"
//!
//! [webhooks.retry]
//! max_retries = 3
//! base_delay_ms = 1000
//! timeout_secs = 30
//! ```

use crate::config::{ConfigFileWebhook, ConfigFileWebhookAuth, SubcogConfig};
use crate::{Error, Result};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Payload format for webhook delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PayloadFormat {
    /// Default Subcog JSON format with `event_id`, `event_type`, timestamp, domain, data.
    #[default]
    Default,
    /// Slack-compatible format with text field and optional blocks.
    Slack,
    /// Discord-compatible format with content field.
    Discord,
}

impl FromStr for PayloadFormat {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "slack" => Ok(Self::Slack),
            "discord" => Ok(Self::Discord),
            _ => Ok(Self::Default),
        }
    }
}

/// Root configuration for webhooks.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebhookConfig {
    /// List of configured webhook endpoints.
    #[serde(default)]
    pub webhooks: Vec<WebhookEndpoint>,
}

impl WebhookConfig {
    /// Loads webhook configuration from the global `SubcogConfig`.
    ///
    /// This reads from `~/.config/subcog/config.toml`.
    #[must_use]
    pub fn load_default() -> Self {
        let config = SubcogConfig::load_default();
        Self::from_subcog_config(&config)
    }

    /// Creates webhook config from `SubcogConfig`.
    #[must_use]
    pub fn from_subcog_config(config: &SubcogConfig) -> Self {
        let webhooks = config
            .webhooks
            .webhooks
            .iter()
            .map(WebhookEndpoint::from_config_file)
            .collect();

        let result = Self { webhooks };

        // Log validation errors but don't fail - allow partial config
        if let Err(e) = result.validate() {
            tracing::warn!(error = %e, "Webhook configuration validation failed");
        }

        result
    }

    /// Validates the webhook configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any webhook URL is invalid
    /// - Duplicate webhook names exist
    /// - Required authentication fields are missing
    fn validate(&self) -> Result<()> {
        let mut names = std::collections::HashSet::new();

        for webhook in &self.webhooks {
            // Check for duplicate names
            if !names.insert(&webhook.name) {
                return Err(Error::InvalidInput(format!(
                    "Duplicate webhook name: {}",
                    webhook.name
                )));
            }

            // Validate URL
            if !webhook.url.starts_with("https://") && !webhook.url.starts_with("http://localhost")
            {
                return Err(Error::InvalidInput(format!(
                    "Webhook URL must use HTTPS (except localhost): {}",
                    webhook.url
                )));
            }

            // Validate authentication
            webhook.auth.validate(&webhook.name)?;
        }

        Ok(())
    }

    /// Returns webhooks that match the given event type and scope.
    pub fn matching_webhooks<'a>(
        &'a self,
        event_type: &str,
        scope: &str,
    ) -> impl Iterator<Item = &'a WebhookEndpoint> {
        self.webhooks
            .iter()
            .filter(move |w| w.enabled && w.matches_event(event_type) && w.matches_scope(scope))
    }
}

/// Configuration for a single webhook endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    /// Unique name for this webhook.
    pub name: String,

    /// Target URL for webhook delivery.
    pub url: String,

    /// Authentication configuration.
    #[serde(default)]
    pub auth: WebhookAuth,

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
    pub retry: RetryConfig,

    /// Payload format (default, slack, discord).
    #[serde(default)]
    pub format: PayloadFormat,
}

impl WebhookEndpoint {
    /// Creates a webhook endpoint from config file entry.
    #[must_use]
    pub fn from_config_file(config: &ConfigFileWebhook) -> Self {
        let auth = config
            .auth
            .as_ref()
            .map(WebhookAuth::from_config_file)
            .unwrap_or_default();

        let retry = RetryConfig {
            max_retries: config.retry.max_retries,
            base_delay_ms: config.retry.base_delay_ms,
            timeout_secs: config.retry.timeout_secs,
        };

        let format = config
            .format
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or_default();

        Self {
            name: config.name.clone(),
            url: config.url.clone(),
            auth,
            events: config.events.clone(),
            scopes: config.scopes.clone(),
            enabled: config.enabled,
            retry,
            format,
        }
    }

    /// Checks if this webhook should receive the given event type.
    #[must_use]
    pub fn matches_event(&self, event_type: &str) -> bool {
        if self.events.is_empty() {
            return true; // Empty = all events
        }
        self.events.iter().any(|e| e == "*" || e == event_type)
    }

    /// Checks if this webhook should receive events from the given scope.
    #[must_use]
    pub fn matches_scope(&self, scope: &str) -> bool {
        if self.scopes.is_empty() {
            return true; // Empty = all scopes
        }
        self.scopes.iter().any(|s| s == "*" || s == scope)
    }
}

/// Authentication configuration for a webhook.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebhookAuth {
    /// Bearer token authentication.
    ///
    /// Adds `Authorization: Bearer <token>` header.
    Bearer {
        /// The bearer token.
        #[serde(with = "secret_string_serde")]
        token: SecretString,
    },

    /// HMAC-SHA256 signature authentication.
    ///
    /// Adds `X-Subcog-Signature: sha256=<signature>` header.
    Hmac {
        /// The shared secret for HMAC signing.
        #[serde(with = "secret_string_serde")]
        secret: SecretString,
    },

    /// Both Bearer token and HMAC signature.
    ///
    /// Most secure option - adds both headers.
    Both {
        /// The bearer token.
        #[serde(with = "secret_string_serde")]
        token: SecretString,
        /// The HMAC secret.
        #[serde(with = "secret_string_serde")]
        secret: SecretString,
    },

    /// No authentication.
    ///
    /// Only use for testing or trusted internal endpoints.
    #[default]
    None,
}

impl WebhookAuth {
    /// Creates webhook auth from config file entry.
    #[must_use]
    pub fn from_config_file(config: &ConfigFileWebhookAuth) -> Self {
        match config {
            ConfigFileWebhookAuth::Bearer { token } => Self::Bearer {
                token: SecretString::from(token.clone()),
            },
            ConfigFileWebhookAuth::Hmac { secret } => Self::Hmac {
                secret: SecretString::from(secret.clone()),
            },
            ConfigFileWebhookAuth::Both { token, secret } => Self::Both {
                token: SecretString::from(token.clone()),
                secret: SecretString::from(secret.clone()),
            },
        }
    }

    /// Returns the bearer token if configured.
    #[must_use]
    pub const fn bearer_token(&self) -> Option<&SecretString> {
        match self {
            Self::Bearer { token } | Self::Both { token, .. } => Some(token),
            _ => None,
        }
    }

    /// Returns the HMAC secret if configured.
    #[must_use]
    pub const fn hmac_secret(&self) -> Option<&SecretString> {
        match self {
            Self::Hmac { secret } | Self::Both { secret, .. } => Some(secret),
            _ => None,
        }
    }

    /// Validates the authentication configuration.
    fn validate(&self, webhook_name: &str) -> Result<()> {
        match self {
            Self::Bearer { token } if token.expose_secret().is_empty() => Err(Error::InvalidInput(
                format!("Webhook '{webhook_name}': Bearer token cannot be empty"),
            )),
            Self::Hmac { secret } if secret.expose_secret().is_empty() => Err(Error::InvalidInput(
                format!("Webhook '{webhook_name}': HMAC secret cannot be empty"),
            )),
            Self::Both { token, secret }
                if token.expose_secret().is_empty() || secret.expose_secret().is_empty() =>
            {
                Err(Error::InvalidInput(format!(
                    "Webhook '{webhook_name}': Bearer token and HMAC secret cannot be empty"
                )))
            },
            _ => Ok(()),
        }
    }
}

/// Retry configuration for webhook delivery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (default: 3).
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Base delay in milliseconds for exponential backoff (default: 1000).
    #[serde(default = "default_base_delay_ms")]
    pub base_delay_ms: u64,

    /// Request timeout in seconds (default: 30).
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            base_delay_ms: default_base_delay_ms(),
            timeout_secs: default_timeout_secs(),
        }
    }
}

impl RetryConfig {
    /// Calculates the delay for a given attempt using exponential backoff.
    ///
    /// Formula: `base_delay_ms` * 2^(attempt - 1)
    ///
    /// # Arguments
    ///
    /// * `attempt` - The attempt number (1-based)
    ///
    /// # Returns
    ///
    /// The delay in milliseconds.
    #[must_use]
    pub fn delay_for_attempt(&self, attempt: u32) -> u64 {
        if attempt == 0 {
            return 0;
        }
        self.base_delay_ms
            .saturating_mul(1 << (attempt - 1).min(10))
    }
}

/// Event filter configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventFilter {
    /// Event types to include (empty = all).
    #[serde(default)]
    pub event_types: Vec<String>,

    /// Namespaces to include (empty = all).
    #[serde(default)]
    pub namespaces: Option<Vec<String>>,

    /// Tags to filter by (OR logic, empty = all).
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

impl EventFilter {
    /// Checks if an event type matches this filter.
    #[must_use]
    pub fn matches_event_type(&self, event_type: &str) -> bool {
        if self.event_types.is_empty() {
            return true;
        }
        self.event_types.iter().any(|t| t == "*" || t == event_type)
    }

    /// Checks if a namespace matches this filter.
    #[must_use]
    pub fn matches_namespace(&self, namespace: &str) -> bool {
        match &self.namespaces {
            None => true,
            Some(ns) if ns.is_empty() => true,
            Some(ns) => ns.iter().any(|n| n == "*" || n == namespace),
        }
    }
}

// Default value functions for serde
const fn default_true() -> bool {
    true
}

const fn default_max_retries() -> u32 {
    3
}

const fn default_base_delay_ms() -> u64 {
    1000
}

const fn default_timeout_secs() -> u64 {
    30
}

/// Serde module for `SecretString` serialization.
mod secret_string_serde {
    use secrecy::SecretString;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(_secret: &SecretString, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Never serialize the actual secret - use placeholder
        serializer.serialize_str("***REDACTED***")
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SecretString, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(SecretString::from(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_delay_calculation() {
        let config = RetryConfig::default();

        assert_eq!(config.delay_for_attempt(1), 1000); // 1s
        assert_eq!(config.delay_for_attempt(2), 2000); // 2s
        assert_eq!(config.delay_for_attempt(3), 4000); // 4s
        assert_eq!(config.delay_for_attempt(0), 0); // No delay for attempt 0
    }

    #[test]
    fn test_webhook_endpoint_matches_event() {
        let webhook = WebhookEndpoint {
            name: "test".to_string(),
            url: "https://example.com".to_string(),
            auth: WebhookAuth::None,
            events: vec!["captured".to_string(), "deleted".to_string()],
            scopes: vec![],
            enabled: true,
            retry: RetryConfig::default(),
            format: PayloadFormat::Default,
        };

        assert!(webhook.matches_event("captured"));
        assert!(webhook.matches_event("deleted"));
        assert!(!webhook.matches_event("updated"));
    }

    #[test]
    fn test_webhook_endpoint_matches_all_events() {
        let webhook = WebhookEndpoint {
            name: "test".to_string(),
            url: "https://example.com".to_string(),
            auth: WebhookAuth::None,
            events: vec![], // Empty = all events
            scopes: vec![],
            enabled: true,
            retry: RetryConfig::default(),
            format: PayloadFormat::Default,
        };

        assert!(webhook.matches_event("captured"));
        assert!(webhook.matches_event("deleted"));
        assert!(webhook.matches_event("any_event"));
    }

    #[test]
    fn test_webhook_endpoint_matches_wildcard() {
        let webhook = WebhookEndpoint {
            name: "test".to_string(),
            url: "https://example.com".to_string(),
            auth: WebhookAuth::None,
            events: vec!["*".to_string()],
            scopes: vec!["*".to_string()],
            enabled: true,
            retry: RetryConfig::default(),
            format: PayloadFormat::Default,
        };

        assert!(webhook.matches_event("any_event"));
        assert!(webhook.matches_scope("any_scope"));
    }

    #[test]
    fn test_config_validation_rejects_http() {
        let config = WebhookConfig {
            webhooks: vec![WebhookEndpoint {
                name: "test".to_string(),
                url: "http://external.com/webhook".to_string(),
                auth: WebhookAuth::None,
                events: vec![],
                scopes: vec![],
                enabled: true,
                retry: RetryConfig::default(),
                format: PayloadFormat::Default,
            }],
        };

        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_config_validation_allows_localhost_http() {
        let config = WebhookConfig {
            webhooks: vec![WebhookEndpoint {
                name: "test".to_string(),
                url: "http://localhost:8080/webhook".to_string(),
                auth: WebhookAuth::None,
                events: vec![],
                scopes: vec![],
                enabled: true,
                retry: RetryConfig::default(),
                format: PayloadFormat::Default,
            }],
        };

        let result = config.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_validation_rejects_duplicate_names() {
        let config = WebhookConfig {
            webhooks: vec![
                WebhookEndpoint {
                    name: "duplicate".to_string(),
                    url: "https://example1.com".to_string(),
                    auth: WebhookAuth::None,
                    events: vec![],
                    scopes: vec![],
                    enabled: true,
                    retry: RetryConfig::default(),
                    format: PayloadFormat::Default,
                },
                WebhookEndpoint {
                    name: "duplicate".to_string(),
                    url: "https://example2.com".to_string(),
                    auth: WebhookAuth::None,
                    events: vec![],
                    scopes: vec![],
                    enabled: true,
                    retry: RetryConfig::default(),
                    format: PayloadFormat::Default,
                },
            ],
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(
            result
                .expect_err("should error")
                .to_string()
                .contains("Duplicate")
        );
    }
}
