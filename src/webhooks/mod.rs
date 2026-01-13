//! Webhook event notification system.
//!
//! This module provides real-time notifications for memory events through webhooks.
//! It supports configurable event subscriptions, domain scoping, and multiple
//! authentication methods (Bearer token and HMAC signature).
//!
//! # Architecture
//!
//! The webhook system follows a clean architecture with clear separation of concerns:
//!
//! - **Config** (`config.rs`): Configuration types and YAML parsing
//! - **Payload** (`payload.rs`): JSON payload building and HMAC signing
//! - **Delivery** (`delivery.rs`): HTTP delivery trait and implementation
//! - **Audit** (`audit.rs`): GDPR-compliant delivery logging
//! - **Dispatcher** (`dispatcher.rs`): Event bus subscription and routing
//!
//! # Example Configuration
//!
//! Webhooks are configured in `~/.config/subcog/webhooks.yaml`:
//!
//! ```yaml
//! webhooks:
//!   - name: slack-notifications
//!     url: https://hooks.slack.com/services/YOUR/WEBHOOK/URL
//!     auth:
//!       type: bearer
//!       token: xoxb-your-token
//!     events:
//!       - captured
//!       - deleted
//!     scopes:
//!       - project
//!       - user
//!     retry:
//!       max_retries: 3
//!       base_delay_ms: 1000
//!       timeout_secs: 30
//! ```
//!
//! # GDPR Compliance
//!
//! All webhook deliveries are logged to a domain-scoped `SQLite` database.
//! The audit log supports:
//! - Export of all logs for a domain (`export_logs`)
//! - Deletion of all logs for a domain (`delete_logs`)
//!
//! # Event Types
//!
//! The following events can trigger webhooks:
//! - `captured` - A new memory was captured
//! - `updated` - An existing memory was updated
//! - `deleted` - A memory was deleted (tombstoned)
//! - `consolidated` - Memories were consolidated into a summary
//! - `archived` - A memory was archived
//! - `retrieved` - A memory was retrieved (search)
//! - `synced` - Memories were synced with remote
//!
//! Use `*` to subscribe to all events.

mod audit;
mod config;
mod delivery;
mod dispatcher;
mod payload;

pub use audit::{DeliveryRecord, DeliveryStatus, WebhookAuditBackend, WebhookAuditLogger};
pub use config::{
    EventFilter, PayloadFormat, RetryConfig, WebhookAuth, WebhookConfig, WebhookEndpoint,
};
pub use delivery::{DeliveryResult, HttpDeliveryBackend, WebhookDelivery};
pub use dispatcher::WebhookDispatcher;
pub use payload::WebhookPayload;

use crate::observability::global_event_bus;
use crate::storage::index::DomainScope;
use crate::{Error, Result};
use std::path::Path;
use std::sync::Arc;

/// Webhook service that manages configuration, delivery, and audit logging.
///
/// # Example
///
/// ```rust,ignore
/// use subcog::webhooks::WebhookService;
///
/// let service = WebhookService::from_config_file()?;
/// if let Some(service) = service {
///     service.start().await?;
/// }
/// ```
pub struct WebhookService {
    /// Webhook configuration.
    config: WebhookConfig,
    /// HTTP delivery backend.
    delivery: Arc<dyn WebhookDelivery>,
    /// Audit logger.
    audit: Arc<WebhookAuditLogger>,
    /// Domain scope for this service instance.
    scope: DomainScope,
}

impl WebhookService {
    /// Creates a new webhook service with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Webhook configuration
    /// * `scope` - Domain scope (project/user/org)
    /// * `audit_db_path` - Path to the audit database
    ///
    /// # Errors
    ///
    /// Returns an error if the audit database cannot be created.
    pub fn new(config: WebhookConfig, scope: DomainScope, audit_db_path: &Path) -> Result<Self> {
        let delivery = Arc::new(HttpDeliveryBackend::new());
        let audit = Arc::new(WebhookAuditLogger::new(audit_db_path)?);

        Ok(Self {
            config,
            delivery,
            audit,
            scope,
        })
    }

    /// Creates a webhook service with custom delivery and audit backends.
    ///
    /// This is primarily useful for testing with mock implementations.
    #[must_use]
    pub fn with_backends(
        config: WebhookConfig,
        scope: DomainScope,
        delivery: Arc<dyn WebhookDelivery>,
        audit: Arc<WebhookAuditLogger>,
    ) -> Self {
        Self {
            config,
            delivery,
            audit,
            scope,
        }
    }

    /// Loads webhook service from the default configuration file.
    ///
    /// Returns `Ok(None)` if no webhooks are configured.
    ///
    /// # Arguments
    ///
    /// * `scope` - Domain scope (project/user/org)
    /// * `data_dir` - Base data directory for the audit database
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration file exists but is invalid.
    pub fn from_config_file(scope: DomainScope, data_dir: &Path) -> Result<Option<Self>> {
        let config = WebhookConfig::load_default();

        if config.webhooks.is_empty() {
            return Ok(None);
        }

        let audit_db_path = data_dir.join("webhook_audit.db");
        Self::new(config, scope, &audit_db_path).map(Some)
    }

    /// Returns the number of configured webhooks.
    #[must_use]
    pub const fn webhook_count(&self) -> usize {
        self.config.webhooks.len()
    }

    /// Returns the number of enabled webhooks.
    #[must_use]
    pub fn enabled_webhook_count(&self) -> usize {
        self.config.webhooks.iter().filter(|w| w.enabled).count()
    }

    /// Creates and starts a webhook dispatcher.
    ///
    /// The dispatcher subscribes to the global event bus and delivers
    /// webhooks for matching events.
    ///
    /// # Returns
    ///
    /// A `WebhookDispatcher` that can be spawned as a background task.
    #[must_use]
    pub fn create_dispatcher(&self) -> WebhookDispatcher {
        WebhookDispatcher::new(
            self.config.webhooks.clone(),
            Arc::clone(&self.delivery),
            Arc::clone(&self.audit),
            self.scope,
        )
    }

    /// Starts the webhook dispatcher as a background task.
    ///
    /// This spawns a tokio task that listens to the event bus and
    /// dispatches webhooks for matching events.
    ///
    /// # Returns
    ///
    /// A join handle for the background task.
    #[must_use]
    pub fn start(&self) -> tokio::task::JoinHandle<()> {
        let dispatcher = self.create_dispatcher();
        let event_bus = global_event_bus();

        tokio::spawn(async move {
            dispatcher.run(event_bus).await;
        })
    }

    /// Returns the audit logger for querying delivery history.
    #[must_use]
    pub fn audit_logger(&self) -> &WebhookAuditLogger {
        &self.audit
    }

    /// Tests a webhook by sending a test event.
    ///
    /// # Arguments
    ///
    /// * `webhook_name` - Name of the webhook to test
    ///
    /// # Errors
    ///
    /// Returns an error if the webhook is not found or delivery fails.
    pub fn test_webhook(&self, webhook_name: &str) -> Result<DeliveryResult> {
        let webhook = self
            .config
            .webhooks
            .iter()
            .find(|w| w.name == webhook_name)
            .ok_or_else(|| Error::InvalidInput(format!("Webhook not found: {webhook_name}")))?;

        let test_payload = WebhookPayload::test_event();
        self.delivery.deliver(webhook, &test_payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config() -> WebhookConfig {
        WebhookConfig {
            webhooks: vec![WebhookEndpoint {
                name: "test-webhook".to_string(),
                url: "https://example.com/webhook".to_string(),
                auth: WebhookAuth::None,
                events: vec!["captured".to_string()],
                scopes: vec!["project".to_string()],
                enabled: true,
                retry: RetryConfig::default(),
                format: config::PayloadFormat::Default,
            }],
        }
    }

    #[test]
    fn test_webhook_service_creation() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = test_config();
        let audit_db_path = temp_dir.path().join("webhook_audit.db");
        let service = WebhookService::new(config, DomainScope::Project, &audit_db_path)
            .expect("create service");

        assert_eq!(service.webhook_count(), 1);
        assert_eq!(service.enabled_webhook_count(), 1);
    }

}
