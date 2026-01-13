//! Webhook payload types and HMAC signing.
//!
//! This module defines the JSON payload structure for webhook notifications
//! and provides HMAC-SHA256 signing for payload authentication.
//!
//! # Payload Format
//!
//! ```json
//! {
//!   "event_id": "550e8400-e29b-41d4-a716-446655440000",
//!   "event_type": "captured",
//!   "timestamp": "2024-01-15T10:30:00Z",
//!   "domain": "project",
//!   "data": {
//!     "memory_id": "abc123",
//!     "namespace": "decisions",
//!     "content_length": 256
//!   }
//! }
//! ```
//!
//! # HMAC Signing
//!
//! When HMAC authentication is configured, the payload is signed using
//! HMAC-SHA256 and the signature is added to the `X-Subcog-Signature` header
//! in the format `sha256=<hex-encoded-signature>`.

use crate::models::{MemoryEvent, MemoryId};
use hmac::{Hmac, Mac};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

/// Webhook payload sent to webhook endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    /// Unique event ID for idempotency.
    pub event_id: String,

    /// Event type (e.g., "captured", "deleted", "consolidated").
    pub event_type: String,

    /// ISO 8601 timestamp.
    pub timestamp: String,

    /// Domain scope (e.g., "project", "user", "org").
    pub domain: String,

    /// Event-specific data.
    pub data: serde_json::Value,
}

impl WebhookPayload {
    /// Creates a new webhook payload from a memory event.
    #[must_use]
    pub fn from_event(event: &MemoryEvent, domain: &str) -> Self {
        let meta = event.meta();
        let event_type = event.event_type();

        Self {
            event_id: meta.event_id.clone(),
            event_type: event_type.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            domain: domain.to_string(),
            data: Self::event_to_data(event),
        }
    }

    /// Creates a test event payload for webhook testing.
    #[must_use]
    pub fn test_event() -> Self {
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            event_type: "test".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            domain: "test".to_string(),
            data: serde_json::json!({
                "message": "This is a test webhook event",
                "source": "subcog"
            }),
        }
    }

    /// Converts the payload to a JSON string (default format).
    ///
    /// # Panics
    ///
    /// This function will not panic as the payload structure is always
    /// serializable. Any serialization error would indicate a bug.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Converts the payload to format-specific JSON.
    ///
    /// # Arguments
    ///
    /// * `format` - The payload format to use
    #[must_use]
    pub fn to_format_json(&self, format: super::config::PayloadFormat) -> String {
        match format {
            super::config::PayloadFormat::Default => self.to_json(),
            super::config::PayloadFormat::Slack => self.to_slack_json(),
            super::config::PayloadFormat::Discord => self.to_discord_json(),
        }
    }

    /// Converts the payload to Slack-compatible JSON format.
    ///
    /// Slack expects `{"text": "message"}` or Block Kit format.
    #[must_use]
    pub fn to_slack_json(&self) -> String {
        let text = self.format_message();
        serde_json::json!({
            "text": text,
            "blocks": [
                {
                    "type": "header",
                    "text": {
                        "type": "plain_text",
                        "text": format!("Subcog: {}", self.event_type),
                        "emoji": true
                    }
                },
                {
                    "type": "section",
                    "fields": [
                        {
                            "type": "mrkdwn",
                            "text": format!("*Event:*\n{}", self.event_type)
                        },
                        {
                            "type": "mrkdwn",
                            "text": format!("*Domain:*\n{}", self.domain)
                        }
                    ]
                },
                {
                    "type": "section",
                    "text": {
                        "type": "mrkdwn",
                        "text": format!("*Details:*\n```{}```", self.data)
                    }
                },
                {
                    "type": "context",
                    "elements": [
                        {
                            "type": "mrkdwn",
                            "text": format!("Event ID: {} | {}", self.event_id, self.timestamp)
                        }
                    ]
                }
            ]
        })
        .to_string()
    }

    /// Converts the payload to Discord-compatible JSON format.
    ///
    /// Discord expects `{"content": "message"}` or embed format.
    #[must_use]
    pub fn to_discord_json(&self) -> String {
        let text = self.format_message();
        serde_json::json!({
            "content": text,
            "embeds": [
                {
                    "title": format!("Subcog: {}", self.event_type),
                    "color": 5_814_783,
                    "fields": [
                        {
                            "name": "Event",
                            "value": self.event_type,
                            "inline": true
                        },
                        {
                            "name": "Domain",
                            "value": self.domain,
                            "inline": true
                        },
                        {
                            "name": "Details",
                            "value": format!("```json\n{}\n```", self.data)
                        }
                    ],
                    "footer": {
                        "text": format!("Event ID: {}", self.event_id)
                    },
                    "timestamp": self.timestamp
                }
            ]
        })
        .to_string()
    }

    /// Formats a human-readable message for the event.
    fn format_message(&self) -> String {
        match self.event_type.as_str() {
            "captured" => format!(
                "Memory captured in {} domain",
                self.domain
            ),
            "deleted" => format!(
                "Memory deleted in {} domain",
                self.domain
            ),
            "updated" => format!(
                "Memory updated in {} domain",
                self.domain
            ),
            "consolidated" => format!(
                "Memories consolidated in {} domain",
                self.domain
            ),
            "test" => "Subcog webhook test event".to_string(),
            _ => format!(
                "{} event in {} domain",
                self.event_type, self.domain
            ),
        }
    }

    /// Computes HMAC-SHA256 signature for the payload.
    ///
    /// # Arguments
    ///
    /// * `secret` - The shared secret for signing
    ///
    /// # Returns
    ///
    /// The signature in format `sha256=<hex-encoded-signature>`.
    #[must_use]
    pub fn compute_signature(&self, secret: &SecretString) -> String {
        let payload_json = self.to_json();
        compute_hmac_signature(secret.expose_secret(), &payload_json)
    }

    /// Converts a memory event to event-specific data.
    #[allow(clippy::too_many_lines)]
    fn event_to_data(event: &MemoryEvent) -> serde_json::Value {
        match event {
            MemoryEvent::Captured {
                memory_id,
                namespace,
                domain,
                content_length,
                ..
            } => serde_json::json!({
                "memory_id": memory_id.as_str(),
                "namespace": namespace.as_str(),
                "domain": domain.to_string(),
                "content_length": content_length
            }),

            MemoryEvent::Updated {
                memory_id,
                modified_fields,
                ..
            } => serde_json::json!({
                "memory_id": memory_id.as_str(),
                "modified_fields": modified_fields
            }),

            // Deleted and Archived have identical payload structure
            MemoryEvent::Deleted {
                memory_id, reason, ..
            }
            | MemoryEvent::Archived {
                memory_id, reason, ..
            } => serde_json::json!({
                "memory_id": memory_id.as_str(),
                "reason": reason
            }),

            MemoryEvent::Retrieved {
                memory_id,
                query,
                score,
                ..
            } => serde_json::json!({
                "memory_id": memory_id.as_str(),
                "query": query.as_ref(),
                "score": score
            }),

            MemoryEvent::Redacted {
                memory_id,
                redaction_type,
                ..
            } => serde_json::json!({
                "memory_id": memory_id.as_str(),
                "redaction_type": redaction_type
            }),

            MemoryEvent::Synced {
                pushed,
                pulled,
                conflicts,
                ..
            } => serde_json::json!({
                "pushed": pushed,
                "pulled": pulled,
                "conflicts": conflicts
            }),

            MemoryEvent::Consolidated {
                processed,
                archived,
                merged,
                ..
            } => serde_json::json!({
                "processed": processed,
                "archived": archived,
                "merged": merged
            }),

            MemoryEvent::McpStarted {
                transport, port, ..
            } => serde_json::json!({
                "transport": transport,
                "port": port
            }),

            MemoryEvent::McpAuthFailed {
                client_id, reason, ..
            } => serde_json::json!({
                "client_id": client_id,
                "reason": reason
            }),

            MemoryEvent::McpToolExecuted {
                tool_name,
                status,
                duration_ms,
                error,
                ..
            } => serde_json::json!({
                "tool_name": tool_name,
                "status": status,
                "duration_ms": duration_ms,
                "error": error
            }),

            MemoryEvent::McpRequestError {
                operation, error, ..
            } => serde_json::json!({
                "operation": operation,
                "error": error
            }),

            MemoryEvent::HookInvoked { hook, .. } => serde_json::json!({
                "hook": hook
            }),

            MemoryEvent::HookClassified {
                hook,
                classification,
                classifier,
                confidence,
                ..
            } => serde_json::json!({
                "hook": hook,
                "classification": classification,
                "classifier": classifier,
                "confidence": confidence
            }),

            MemoryEvent::HookCaptureDecision {
                hook,
                decision,
                namespace,
                memory_id,
                ..
            } => serde_json::json!({
                "hook": hook,
                "decision": decision,
                "namespace": namespace,
                "memory_id": memory_id.as_ref().map(MemoryId::as_str)
            }),

            MemoryEvent::HookFailed { hook, error, .. } => serde_json::json!({
                "hook": hook,
                "error": error
            }),
        }
    }
}

/// Computes HMAC-SHA256 signature for a payload string.
///
/// # Arguments
///
/// * `secret` - The shared secret
/// * `payload` - The payload string to sign
///
/// # Returns
///
/// The signature in format `sha256=<hex-encoded-signature>`.
///
/// # Panics
///
/// This function will not panic. HMAC-SHA256 accepts keys of any length.
#[must_use]
#[allow(clippy::expect_used)] // HMAC-SHA256 accepts any key size, cannot fail
pub fn compute_hmac_signature(secret: &str, payload: &str) -> String {
    type HmacSha256 = Hmac<Sha256>;

    // SAFETY: HMAC-SHA256 accepts keys of any length, new_from_slice cannot fail
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC-SHA256 accepts any key size");
    mac.update(payload.as_bytes());

    let result = mac.finalize();
    let signature = hex::encode(result.into_bytes());

    format!("sha256={signature}")
}

/// Verifies an HMAC-SHA256 signature.
///
/// This function is provided for webhook receivers to verify incoming
/// webhook signatures. It uses constant-time comparison to prevent
/// timing attacks.
///
/// # Arguments
///
/// * `secret` - The shared secret
/// * `payload` - The payload string that was signed
/// * `signature` - The signature to verify (with or without `sha256=` prefix)
///
/// # Returns
///
/// `true` if the signature is valid, `false` otherwise.
///
/// # Example
///
/// ```rust,ignore
/// use subcog::webhooks::payload::verify_hmac_signature;
///
/// let is_valid = verify_hmac_signature("my-secret", r#"{"event":"test"}"#, "sha256=...");
/// ```
#[must_use]
#[allow(dead_code)] // Provided for external webhook receivers
pub fn verify_hmac_signature(secret: &str, payload: &str, signature: &str) -> bool {
    let expected = compute_hmac_signature(secret, payload);

    // Handle both with and without prefix
    let signature = if signature.starts_with("sha256=") {
        signature.to_string()
    } else {
        format!("sha256={signature}")
    };

    // Use constant-time comparison to prevent timing attacks
    constant_time_eq(expected.as_bytes(), signature.as_bytes())
}

/// Constant-time comparison to prevent timing attacks.
#[allow(dead_code)] // Used by verify_hmac_signature
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, EventMeta};

    #[test]
    fn test_payload_to_json() {
        let payload = WebhookPayload::test_event();
        let json = payload.to_json();

        assert!(json.contains("event_id"));
        assert!(json.contains("event_type"));
        assert!(json.contains("test"));
    }

    #[test]
    fn test_hmac_signature_computation() {
        let secret = "my-secret-key";
        let payload = r#"{"event":"test"}"#;

        let signature = compute_hmac_signature(secret, payload);

        assert!(signature.starts_with("sha256="));
        assert_eq!(signature.len(), 7 + 64); // "sha256=" + 64 hex chars
    }

    #[test]
    fn test_hmac_signature_verification() {
        let secret = "my-secret-key";
        let payload = r#"{"event":"test"}"#;

        let signature = compute_hmac_signature(secret, payload);

        assert!(verify_hmac_signature(secret, payload, &signature));
        assert!(!verify_hmac_signature("wrong-secret", payload, &signature));
        assert!(!verify_hmac_signature(secret, "wrong-payload", &signature));
    }

    #[test]
    fn test_hmac_verification_handles_prefix() {
        let secret = "my-secret-key";
        let payload = r#"{"event":"test"}"#;

        let signature = compute_hmac_signature(secret, payload);
        let without_prefix = signature.strip_prefix("sha256=").expect("prefix");

        // Both should verify correctly
        assert!(verify_hmac_signature(secret, payload, &signature));
        assert!(verify_hmac_signature(secret, payload, without_prefix));
    }

    #[test]
    fn test_payload_from_captured_event() {
        let event = MemoryEvent::Captured {
            meta: EventMeta::new("test", None),
            memory_id: MemoryId::new("test-123"),
            namespace: crate::Namespace::Decisions,
            domain: Domain::new(),
            content_length: 100,
        };

        let payload = WebhookPayload::from_event(&event, "project");

        assert_eq!(payload.event_type, "captured");
        assert_eq!(payload.domain, "project");
        assert!(payload.data.get("memory_id").is_some());
        assert!(payload.data.get("namespace").is_some());
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hell"));
    }
}
