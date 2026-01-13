//! Event dispatcher for routing memory events to webhooks.
//!
//! The dispatcher subscribes to the global event bus and routes matching
//! events to configured webhook endpoints, handling delivery and audit logging.
//!
//! # Architecture
//!
//! ```text
//! EventBus --[subscribe]--> Dispatcher --[filter]--> Webhook Endpoints
//!                                |                         |
//!                                v                         v
//!                          Domain Filter            Delivery Backend
//!                                                         |
//!                                                         v
//!                                                   Audit Logger
//! ```
//!
//! # Event Flow
//!
//! 1. Dispatcher subscribes to the global event bus
//! 2. Events are filtered by domain scope
//! 3. Matching webhooks are identified by event type and scope
//! 4. Payloads are built and delivered
//! 5. Results are logged to the audit database

use super::audit::{WebhookAuditBackend, WebhookAuditLogger};
use super::config::WebhookEndpoint;
use super::delivery::{DeliveryResult, WebhookDelivery};
use super::payload::WebhookPayload;
use crate::models::MemoryEvent;
use crate::observability::EventBus;
use crate::storage::index::DomainScope;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Event dispatcher that routes memory events to matching webhooks.
pub struct WebhookDispatcher {
    /// Configured webhook endpoints.
    webhooks: Vec<WebhookEndpoint>,
    /// Delivery backend for sending webhooks.
    delivery: Arc<dyn WebhookDelivery>,
    /// Audit logger for recording delivery attempts.
    audit: Arc<WebhookAuditLogger>,
    /// Domain scope for filtering events.
    scope: DomainScope,
}

impl WebhookDispatcher {
    /// Creates a new webhook dispatcher.
    ///
    /// # Arguments
    ///
    /// * `webhooks` - List of webhook endpoint configurations
    /// * `delivery` - Delivery backend for HTTP requests
    /// * `audit` - Audit logger for GDPR-compliant logging
    /// * `scope` - Domain scope for event filtering
    #[must_use]
    pub fn new(
        webhooks: Vec<WebhookEndpoint>,
        delivery: Arc<dyn WebhookDelivery>,
        audit: Arc<WebhookAuditLogger>,
        scope: DomainScope,
    ) -> Self {
        Self {
            webhooks,
            delivery,
            audit,
            scope,
        }
    }

    /// Runs the dispatcher, listening for events from the event bus.
    ///
    /// This is a long-running async task that should be spawned as a
    /// background task. It will run until the event bus is closed.
    ///
    /// # Arguments
    ///
    /// * `event_bus` - The event bus to subscribe to
    pub async fn run(&self, event_bus: &EventBus) {
        let mut receiver = event_bus.subscribe();

        loop {
            match receiver.recv().await {
                Ok(event) => {
                    self.handle_event(&event);
                },
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    metrics::counter!("webhook_events_lagged_total").increment(skipped);
                    tracing::warn!(
                        skipped = skipped,
                        "Webhook dispatcher lagged behind event bus"
                    );
                },
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::info!("Event bus closed, webhook dispatcher shutting down");
                    break;
                },
            }
        }
    }

    /// Handles a single event by dispatching to matching webhooks.
    fn handle_event(&self, event: &MemoryEvent) {
        let event_type = event.event_type();
        let domain_str = self.scope_to_string();

        metrics::counter!("webhook_events_received_total", "event_type" => event_type.to_string())
            .increment(1);

        // Find matching webhooks
        let matching: Vec<&WebhookEndpoint> = self
            .webhooks
            .iter()
            .filter(|w| w.enabled && w.matches_event(event_type) && w.matches_scope(&domain_str))
            .collect();

        if matching.is_empty() {
            return;
        }

        // Build payload
        let payload = WebhookPayload::from_event(event, &domain_str);

        // Dispatch to each matching webhook
        for webhook in matching {
            self.dispatch_to_webhook(webhook, &payload);
        }
    }

    /// Dispatches a payload to a single webhook endpoint.
    fn dispatch_to_webhook(&self, webhook: &WebhookEndpoint, payload: &WebhookPayload) {
        let start = std::time::Instant::now();

        metrics::counter!(
            "webhook_deliveries_total",
            "webhook" => webhook.name.clone(),
            "event_type" => payload.event_type.clone()
        )
        .increment(1);

        // Attempt delivery
        let result = self.delivery.deliver(webhook, payload);

        // Record metrics
        let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
        metrics::histogram!(
            "webhook_delivery_duration_ms",
            "webhook" => webhook.name.clone()
        )
        .record(duration_ms);

        // Log to audit database
        self.log_delivery(webhook, payload, &result);

        // Log result
        match &result {
            Ok(r) if r.success => {
                metrics::counter!(
                    "webhook_deliveries_success_total",
                    "webhook" => webhook.name.clone()
                )
                .increment(1);

                tracing::debug!(
                    webhook = %webhook.name,
                    event_type = %payload.event_type,
                    event_id = %payload.event_id,
                    status_code = ?r.status_code,
                    attempts = r.attempts,
                    duration_ms = r.duration_ms,
                    "Webhook delivered successfully"
                );
            },
            Ok(r) => {
                metrics::counter!(
                    "webhook_deliveries_failed_total",
                    "webhook" => webhook.name.clone()
                )
                .increment(1);

                tracing::warn!(
                    webhook = %webhook.name,
                    event_type = %payload.event_type,
                    event_id = %payload.event_id,
                    error = ?r.error,
                    attempts = r.attempts,
                    "Webhook delivery failed"
                );
            },
            Err(e) => {
                metrics::counter!(
                    "webhook_deliveries_error_total",
                    "webhook" => webhook.name.clone()
                )
                .increment(1);

                tracing::error!(
                    webhook = %webhook.name,
                    event_type = %payload.event_type,
                    event_id = %payload.event_id,
                    error = %e,
                    "Webhook delivery error"
                );
            },
        }
    }

    /// Logs a delivery attempt to the audit database.
    fn log_delivery(
        &self,
        webhook: &WebhookEndpoint,
        payload: &WebhookPayload,
        result: &crate::Result<DeliveryResult>,
    ) {
        use super::audit::{DeliveryRecord, DeliveryStatus};

        let record = match result {
            Ok(r) if r.success => DeliveryRecord {
                id: uuid::Uuid::new_v4().to_string(),
                webhook_name: webhook.name.clone(),
                event_type: payload.event_type.clone(),
                event_id: payload.event_id.clone(),
                domain: payload.domain.clone(),
                url: webhook.url.clone(),
                status: DeliveryStatus::Success,
                status_code: r.status_code.map(i32::from),
                attempts: i32::try_from(r.attempts).unwrap_or(i32::MAX),
                duration_ms: i64::try_from(r.duration_ms).unwrap_or(i64::MAX),
                error: None,
                timestamp: chrono::Utc::now().timestamp(),
            },
            Ok(r) => DeliveryRecord {
                id: uuid::Uuid::new_v4().to_string(),
                webhook_name: webhook.name.clone(),
                event_type: payload.event_type.clone(),
                event_id: payload.event_id.clone(),
                domain: payload.domain.clone(),
                url: webhook.url.clone(),
                status: DeliveryStatus::Failed,
                status_code: r.status_code.map(i32::from),
                attempts: i32::try_from(r.attempts).unwrap_or(i32::MAX),
                duration_ms: i64::try_from(r.duration_ms).unwrap_or(i64::MAX),
                error: r.error.clone(),
                timestamp: chrono::Utc::now().timestamp(),
            },
            Err(e) => DeliveryRecord {
                id: uuid::Uuid::new_v4().to_string(),
                webhook_name: webhook.name.clone(),
                event_type: payload.event_type.clone(),
                event_id: payload.event_id.clone(),
                domain: payload.domain.clone(),
                url: webhook.url.clone(),
                status: DeliveryStatus::Failed,
                status_code: None,
                attempts: 0,
                duration_ms: 0,
                error: Some(e.to_string()),
                timestamp: chrono::Utc::now().timestamp(),
            },
        };

        if let Err(e) = self.audit.store(&record) {
            tracing::error!(
                webhook = %webhook.name,
                error = %e,
                "Failed to log webhook delivery to audit database"
            );
        }
    }

    /// Converts the domain scope to a string for matching.
    fn scope_to_string(&self) -> String {
        match self.scope {
            DomainScope::Project => "project".to_string(),
            DomainScope::User => "user".to_string(),
            DomainScope::Org => "org".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, EventMeta, MemoryId, Namespace};
    use crate::webhooks::config::{PayloadFormat, RetryConfig, WebhookAuth};
    use crate::webhooks::delivery::MockDeliveryBackend;
    use tempfile::TempDir;

    fn test_webhook(name: &str, events: Vec<&str>, scopes: Vec<&str>) -> WebhookEndpoint {
        WebhookEndpoint {
            name: name.to_string(),
            url: "https://example.com/webhook".to_string(),
            auth: WebhookAuth::None,
            events: events.into_iter().map(String::from).collect(),
            scopes: scopes.into_iter().map(String::from).collect(),
            enabled: true,
            retry: RetryConfig::default(),
            format: PayloadFormat::Default,
        }
    }

    #[test]
    fn test_dispatcher_routes_matching_event() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let audit_path = temp_dir.path().join("audit.db");

        let webhooks = vec![
            test_webhook("captured-only", vec!["captured"], vec!["project"]),
            test_webhook("all-events", vec![], vec![]),
        ];

        let delivery = Arc::new(MockDeliveryBackend::new());
        let audit = Arc::new(WebhookAuditLogger::new(&audit_path).expect("create audit"));

        let dispatcher = WebhookDispatcher::new(
            webhooks,
            Arc::clone(&delivery) as Arc<dyn WebhookDelivery>,
            audit,
            DomainScope::Project,
        );

        let event = MemoryEvent::Captured {
            meta: EventMeta::new("test", None),
            memory_id: MemoryId::new("test-123"),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            content_length: 100,
        };

        dispatcher.handle_event(&event);

        // Both webhooks should have received the event
        assert_eq!(delivery.delivery_count(), 2);
    }

    #[test]
    fn test_dispatcher_filters_by_event_type() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let audit_path = temp_dir.path().join("audit.db");

        let webhooks = vec![test_webhook("deleted-only", vec!["deleted"], vec![])];

        let delivery = Arc::new(MockDeliveryBackend::new());
        let audit = Arc::new(WebhookAuditLogger::new(&audit_path).expect("create audit"));

        let dispatcher = WebhookDispatcher::new(
            webhooks,
            Arc::clone(&delivery) as Arc<dyn WebhookDelivery>,
            audit,
            DomainScope::Project,
        );

        // Send a captured event (should not match)
        let event = MemoryEvent::Captured {
            meta: EventMeta::new("test", None),
            memory_id: MemoryId::new("test-123"),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            content_length: 100,
        };

        dispatcher.handle_event(&event);

        // No deliveries should have been made
        assert_eq!(delivery.delivery_count(), 0);
    }

    #[test]
    fn test_dispatcher_filters_by_scope() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let audit_path = temp_dir.path().join("audit.db");

        let webhooks = vec![test_webhook("user-only", vec![], vec!["user"])];

        let delivery = Arc::new(MockDeliveryBackend::new());
        let audit = Arc::new(WebhookAuditLogger::new(&audit_path).expect("create audit"));

        // Dispatcher is project-scoped
        let dispatcher = WebhookDispatcher::new(
            webhooks,
            Arc::clone(&delivery) as Arc<dyn WebhookDelivery>,
            audit,
            DomainScope::Project,
        );

        let event = MemoryEvent::Captured {
            meta: EventMeta::new("test", None),
            memory_id: MemoryId::new("test-123"),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            content_length: 100,
        };

        dispatcher.handle_event(&event);

        // No deliveries (scope mismatch)
        assert_eq!(delivery.delivery_count(), 0);
    }

    #[test]
    fn test_dispatcher_skips_disabled_webhooks() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let audit_path = temp_dir.path().join("audit.db");

        let mut webhook = test_webhook("disabled", vec![], vec![]);
        webhook.enabled = false;

        let webhooks = vec![webhook];

        let delivery = Arc::new(MockDeliveryBackend::new());
        let audit = Arc::new(WebhookAuditLogger::new(&audit_path).expect("create audit"));

        let dispatcher = WebhookDispatcher::new(
            webhooks,
            Arc::clone(&delivery) as Arc<dyn WebhookDelivery>,
            audit,
            DomainScope::Project,
        );

        let event = MemoryEvent::Captured {
            meta: EventMeta::new("test", None),
            memory_id: MemoryId::new("test-123"),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            content_length: 100,
        };

        dispatcher.handle_event(&event);

        // No deliveries (webhook disabled)
        assert_eq!(delivery.delivery_count(), 0);
    }
}
