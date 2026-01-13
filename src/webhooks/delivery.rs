//! Webhook delivery backend trait and HTTP implementation.
//!
//! This module defines the `WebhookDelivery` trait for abstracting webhook
//! delivery, and provides an HTTP implementation using `reqwest`.
//!
//! # Retry Strategy
//!
//! The HTTP delivery backend implements exponential backoff with the following
//! default configuration:
//! - Max retries: 3
//! - Base delay: 1 second
//! - Delays: 1s, 2s, 4s (exponential backoff)
//!
//! # Error Handling
//!
//! - Network errors: Retry with backoff
//! - 4xx client errors: No retry, log failure
//! - 5xx server errors: Retry with backoff
//! - Timeout: Retry with backoff

use super::config::{WebhookAuth, WebhookEndpoint};
use super::payload::WebhookPayload;
use crate::Result;
use secrecy::ExposeSecret;
use std::time::Duration;

/// Result of a webhook delivery attempt.
#[derive(Debug, Clone)]
pub struct DeliveryResult {
    /// Whether the delivery was successful.
    pub success: bool,

    /// HTTP status code (if available).
    pub status_code: Option<u16>,

    /// Number of attempts made.
    pub attempts: u32,

    /// Total duration in milliseconds.
    pub duration_ms: u64,

    /// Error message (if failed).
    pub error: Option<String>,
}

impl DeliveryResult {
    /// Creates a successful delivery result.
    #[must_use]
    pub const fn success(status_code: u16, attempts: u32, duration_ms: u64) -> Self {
        Self {
            success: true,
            status_code: Some(status_code),
            attempts,
            duration_ms,
            error: None,
        }
    }

    /// Creates a failed delivery result.
    #[must_use]
    pub const fn failure(error: String, attempts: u32, duration_ms: u64) -> Self {
        Self {
            success: false,
            status_code: None,
            attempts,
            duration_ms,
            error: Some(error),
        }
    }

    /// Creates a failed delivery result with status code.
    #[must_use]
    pub const fn failure_with_status(
        status_code: u16,
        error: String,
        attempts: u32,
        duration_ms: u64,
    ) -> Self {
        Self {
            success: false,
            status_code: Some(status_code),
            attempts,
            duration_ms,
            error: Some(error),
        }
    }
}

/// Trait for webhook delivery backends.
///
/// This trait allows for different delivery implementations (HTTP, mock for testing).
pub trait WebhookDelivery: Send + Sync {
    /// Delivers a webhook payload to the configured endpoint.
    ///
    /// # Arguments
    ///
    /// * `endpoint` - The webhook endpoint configuration
    /// * `payload` - The payload to deliver
    ///
    /// # Returns
    ///
    /// A `DeliveryResult` containing success/failure status and metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if delivery fails after all retries.
    fn deliver(
        &self,
        endpoint: &WebhookEndpoint,
        payload: &WebhookPayload,
    ) -> Result<DeliveryResult>;
}

/// HTTP webhook delivery backend using reqwest.
pub struct HttpDeliveryBackend {
    /// HTTP client with connection pooling.
    client: reqwest::blocking::Client,
}

impl HttpDeliveryBackend {
    /// Creates a new HTTP delivery backend.
    #[must_use]
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent(format!("Subcog/{}", env!("CARGO_PKG_VERSION")))
            .pool_max_idle_per_host(10)
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        Self { client }
    }

    /// Creates a new HTTP delivery backend with custom timeout.
    #[must_use]
    pub fn with_timeout(timeout: Duration) -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent(format!("Subcog/{}", env!("CARGO_PKG_VERSION")))
            .timeout(timeout)
            .pool_max_idle_per_host(10)
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        Self { client }
    }

    /// Attempts a single delivery without retries.
    fn attempt_delivery(
        &self,
        endpoint: &WebhookEndpoint,
        payload: &WebhookPayload,
    ) -> std::result::Result<u16, String> {
        // Use format-specific JSON based on endpoint configuration
        let payload_json = payload.to_format_json(endpoint.format);

        let mut request = self
            .client
            .post(&endpoint.url)
            .header("Content-Type", "application/json")
            .header("X-Subcog-Event", &payload.event_type)
            .header("X-Subcog-Delivery-Id", &payload.event_id)
            .timeout(Duration::from_secs(endpoint.retry.timeout_secs));

        // Add authentication headers (use default format for HMAC signature)
        let signature_json = payload.to_json();
        request = Self::add_auth_headers(request, &endpoint.auth, &signature_json);

        // Send request
        let response = request
            .body(payload_json)
            .send()
            .map_err(|e| format!("HTTP request failed: {e}"))?;

        let status = response.status().as_u16();

        if response.status().is_success() {
            Ok(status)
        } else {
            Err(format!("HTTP {status} response"))
        }
    }

    /// Adds authentication headers to a request.
    fn add_auth_headers(
        mut request: reqwest::blocking::RequestBuilder,
        auth: &WebhookAuth,
        payload_json: &str,
    ) -> reqwest::blocking::RequestBuilder {
        // Add Bearer token if configured
        if let Some(token) = auth.bearer_token() {
            request = request.header("Authorization", format!("Bearer {}", token.expose_secret()));
        }

        // Add HMAC signature if configured
        if let Some(secret) = auth.hmac_secret() {
            let signature =
                super::payload::compute_hmac_signature(secret.expose_secret(), payload_json);
            request = request.header("X-Subcog-Signature", signature);
        }

        request
    }

    /// Delivers with retry logic.
    fn deliver_with_retry(
        &self,
        endpoint: &WebhookEndpoint,
        payload: &WebhookPayload,
    ) -> DeliveryResult {
        let start = std::time::Instant::now();
        let retry_config = &endpoint.retry;
        let max_attempts = retry_config.max_retries + 1;

        for attempt in 1..=max_attempts {
            if let Some(result) = self.try_single_delivery(
                endpoint,
                payload,
                attempt,
                max_attempts,
                start,
                retry_config,
            ) {
                return result;
            }
        }

        // Should not reach here
        let duration_ms = Self::elapsed_ms(start);
        DeliveryResult::failure(
            "Max retries exceeded".to_string(),
            max_attempts,
            duration_ms,
        )
    }

    /// Attempts a single delivery, returning Some if done (success or final failure).
    fn try_single_delivery(
        &self,
        endpoint: &WebhookEndpoint,
        payload: &WebhookPayload,
        attempt: u32,
        max_attempts: u32,
        start: std::time::Instant,
        retry_config: &super::config::RetryConfig,
    ) -> Option<DeliveryResult> {
        match self.attempt_delivery(endpoint, payload) {
            Ok(status_code) => {
                let duration_ms = Self::elapsed_ms(start);
                Some(DeliveryResult::success(status_code, attempt, duration_ms))
            },
            Err(ref error) => {
                Self::handle_delivery_error(error, attempt, max_attempts, start, retry_config)
            },
        }
    }

    /// Gets elapsed milliseconds, saturating at `u64::MAX`.
    #[allow(clippy::cast_possible_truncation)]
    fn elapsed_ms(start: std::time::Instant) -> u64 {
        // Duration in ms will not realistically exceed u64::MAX
        start.elapsed().as_millis() as u64
    }

    /// Handles delivery errors, returning Some if we should stop retrying.
    fn handle_delivery_error(
        error: &str,
        attempt: u32,
        max_attempts: u32,
        start: std::time::Instant,
        retry_config: &super::config::RetryConfig,
    ) -> Option<DeliveryResult> {
        // Check if this is a client error (4xx) - don't retry
        if error.contains("HTTP 4") {
            let duration_ms = Self::elapsed_ms(start);
            let status_code = Self::extract_status_code(error);
            return Some(DeliveryResult::failure_with_status(
                status_code.unwrap_or(400),
                error.to_string(),
                attempt,
                duration_ms,
            ));
        }

        // Last attempt - return failure
        if attempt >= max_attempts {
            let duration_ms = Self::elapsed_ms(start);
            return Some(DeliveryResult::failure(
                error.to_string(),
                attempt,
                duration_ms,
            ));
        }

        // Sleep before retry
        let delay_ms = retry_config.delay_for_attempt(attempt);
        std::thread::sleep(Duration::from_millis(delay_ms));

        None
    }

    /// Extracts status code from error message.
    fn extract_status_code(error: &str) -> Option<u16> {
        // Pattern: "HTTP 4xx" or "HTTP 5xx"
        if let Some(start) = error.find("HTTP ") {
            let code_str = &error[start + 5..];
            if code_str.len() >= 3 {
                return code_str[..3].parse().ok();
            }
        }
        None
    }
}

impl Default for HttpDeliveryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl WebhookDelivery for HttpDeliveryBackend {
    fn deliver(
        &self,
        endpoint: &WebhookEndpoint,
        payload: &WebhookPayload,
    ) -> Result<DeliveryResult> {
        Ok(self.deliver_with_retry(endpoint, payload))
    }
}

/// Mock delivery backend for testing.
#[cfg(test)]
pub struct MockDeliveryBackend {
    /// Responses to return for each delivery.
    responses: std::sync::Mutex<Vec<Result<DeliveryResult>>>,
    /// Payloads that were delivered.
    pub delivered: std::sync::Mutex<Vec<(String, WebhookPayload)>>,
}

#[cfg(test)]
impl MockDeliveryBackend {
    /// Creates a new mock backend.
    pub fn new() -> Self {
        Self {
            responses: std::sync::Mutex::new(Vec::new()),
            delivered: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Queues a response for the next delivery.
    pub fn queue_response(&self, result: Result<DeliveryResult>) {
        self.responses.lock().expect("lock").push(result);
    }

    /// Returns the number of deliveries made.
    pub fn delivery_count(&self) -> usize {
        self.delivered.lock().expect("lock").len()
    }
}

#[cfg(test)]
impl WebhookDelivery for MockDeliveryBackend {
    fn deliver(
        &self,
        endpoint: &WebhookEndpoint,
        payload: &WebhookPayload,
    ) -> Result<DeliveryResult> {
        self.delivered
            .lock()
            .expect("lock")
            .push((endpoint.name.clone(), payload.clone()));

        self.responses
            .lock()
            .expect("lock")
            .pop()
            .unwrap_or_else(|| Ok(DeliveryResult::success(200, 1, 10)))
    }
}

#[cfg(test)]
mod tests {
    use super::super::config::{PayloadFormat, RetryConfig};
    use super::*;

    #[test]
    fn test_delivery_result_success() {
        let result = DeliveryResult::success(200, 1, 100);

        assert!(result.success);
        assert_eq!(result.status_code, Some(200));
        assert_eq!(result.attempts, 1);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_delivery_result_failure() {
        let result = DeliveryResult::failure("Connection refused".to_string(), 3, 5000);

        assert!(!result.success);
        assert!(result.status_code.is_none());
        assert_eq!(result.attempts, 3);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_extract_status_code() {
        assert_eq!(
            HttpDeliveryBackend::extract_status_code("HTTP 404 response"),
            Some(404)
        );
        assert_eq!(
            HttpDeliveryBackend::extract_status_code("HTTP 500 response"),
            Some(500)
        );
        assert_eq!(
            HttpDeliveryBackend::extract_status_code("Connection refused"),
            None
        );
    }

    #[test]
    fn test_mock_delivery_backend() {
        let mock = MockDeliveryBackend::new();
        mock.queue_response(Ok(DeliveryResult::success(201, 1, 50)));

        let endpoint = WebhookEndpoint {
            name: "test".to_string(),
            url: "https://example.com".to_string(),
            auth: WebhookAuth::None,
            events: vec![],
            scopes: vec![],
            enabled: true,
            retry: RetryConfig::default(),
            format: PayloadFormat::Default,
        };

        let payload = WebhookPayload::test_event();
        let result = mock.deliver(&endpoint, &payload).expect("delivery");

        assert!(result.success);
        assert_eq!(result.status_code, Some(201));
        assert_eq!(mock.delivery_count(), 1);
    }
}
