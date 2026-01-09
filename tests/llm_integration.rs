//! LLM Client Integration Tests (TEST-HIGH-003)
//!
//! Tests LLM provider implementations and resilience layer in integration scenarios:
//! - Provider configuration and validation
//! - Resilient provider wrapping with circuit breaker and retry logic
//! - Error handling and categorization across providers
//! - Provider fallback behavior
//!
//! These tests do NOT require actual API keys and use mock endpoints
//! to test error handling behavior.

// Integration tests use expect/unwrap for simplicity - panics are acceptable in tests
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::excessive_nesting,
    dead_code
)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use subcog::Error;
use subcog::llm::{
    AnthropicClient, LlmHttpConfig, LlmProvider, LlmResilienceConfig, LmStudioClient, OllamaClient,
    OpenAiClient, ResilientLlmProvider,
};

// ============================================================================
// Provider Configuration Tests
// ============================================================================

mod provider_config {
    use super::*;

    #[test]
    fn test_anthropic_client_builder() {
        let client = AnthropicClient::new()
            .with_api_key("sk-ant-api03-test-key-for-testing-purposes-only1234")
            .with_endpoint("https://test.anthropic.com/v1")
            .with_model("claude-3-opus-20240229");

        assert_eq!(client.name(), "anthropic");
    }

    #[test]
    fn test_openai_client_builder() {
        let client = OpenAiClient::new()
            .with_api_key("sk-proj-test-key-for-testing-only1234567890")
            .with_endpoint("https://test.openai.com/v1")
            .with_model("gpt-4-turbo");

        assert_eq!(client.name(), "openai");
    }

    #[test]
    fn test_ollama_client_builder() {
        let client = OllamaClient::new()
            .with_endpoint("http://localhost:11434")
            .with_model("llama2");

        assert_eq!(client.name(), "ollama");
    }

    #[test]
    fn test_lmstudio_client_builder() {
        let client = LmStudioClient::new()
            .with_endpoint("http://localhost:1234/v1")
            .with_model("local-model");

        assert_eq!(client.name(), "lmstudio");
    }

    #[test]
    fn test_http_config_builder() {
        let config = LlmHttpConfig {
            timeout_ms: 30_000,
            connect_timeout_ms: 5_000,
        };

        let client = OpenAiClient::new()
            .with_api_key("sk-proj-test-key-for-testing-only1234567890")
            .with_http_config(config);

        assert_eq!(client.name(), "openai");
    }

    #[test]
    fn test_http_config_from_env() {
        // Default config should have reasonable values
        let config = LlmHttpConfig::from_env();
        assert!(config.timeout_ms > 0);
        assert!(config.connect_timeout_ms > 0);
    }
}

// ============================================================================
// Provider Validation Tests
// ============================================================================

mod provider_validation {
    use super::*;

    #[test]
    fn test_anthropic_rejects_missing_api_key() {
        // Create client without API key
        let client = AnthropicClient::new().with_endpoint("https://api.anthropic.com/v1");

        let result = client.complete("test prompt");
        assert!(result.is_err());

        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("not set") || err_str.contains("not configured"),
            "Expected API key error, got: {err_str}"
        );
    }

    #[test]
    fn test_anthropic_rejects_invalid_api_key_format() {
        let client = AnthropicClient::new()
            .with_api_key("invalid-key-format")
            .with_endpoint("https://api.anthropic.com/v1");

        let result = client.complete("test prompt");
        assert!(result.is_err());

        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("Invalid API key") || err_str.contains("format"),
            "Expected format error, got: {err_str}"
        );
    }

    #[test]
    fn test_openai_rejects_missing_api_key() {
        // Use without_api_key() to clear any env-provided key
        let client = OpenAiClient::new()
            .without_api_key()
            .with_endpoint("https://api.openai.com/v1");

        let result = client.complete("test prompt");
        assert!(result.is_err());

        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("not set") || err_str.contains("not configured"),
            "Expected API key error, got: {err_str}"
        );
    }

    #[test]
    fn test_openai_rejects_invalid_api_key_format() {
        let client = OpenAiClient::new()
            .with_api_key("invalid-key-without-prefix")
            .with_endpoint("https://api.openai.com/v1");

        let result = client.complete("test prompt");
        assert!(result.is_err());
    }
}

// ============================================================================
// Connection Error Tests
// ============================================================================

mod connection_errors {
    use super::*;

    #[test]
    fn test_anthropic_connection_refused() {
        let client = AnthropicClient::new()
            .with_api_key("sk-ant-api03-test-key-for-testing-purposes-only1234")
            .with_endpoint("http://127.0.0.1:59998");

        let result = client.complete("test prompt");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, Error::OperationFailed { .. }));
    }

    #[test]
    fn test_openai_connection_refused() {
        let client = OpenAiClient::new()
            .with_api_key("sk-proj-test-key-for-testing-only1234567890")
            .with_endpoint("http://127.0.0.1:59997");

        let result = client.complete("test prompt");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, Error::OperationFailed { .. }));
    }

    #[test]
    fn test_ollama_connection_refused() {
        let client = OllamaClient::new().with_endpoint("http://127.0.0.1:59996");

        let result = client.complete("test prompt");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, Error::OperationFailed { .. }));
    }

    #[test]
    fn test_lmstudio_connection_refused() {
        let client = LmStudioClient::new().with_endpoint("http://127.0.0.1:59995");

        let result = client.complete("test prompt");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, Error::OperationFailed { .. }));
    }

    #[test]
    fn test_timeout_with_non_routable_address() {
        let config = LlmHttpConfig {
            timeout_ms: 100,        // Very short timeout
            connect_timeout_ms: 50, // Very short connect timeout
        };

        let client = OpenAiClient::new()
            .with_api_key("sk-proj-test-key-for-testing-only1234567890")
            .with_endpoint("http://10.255.255.1") // Non-routable
            .with_http_config(config);

        let result = client.complete("test prompt");
        assert!(result.is_err());

        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("timeout") || err_str.contains("connect"),
            "Expected timeout/connect error, got: {err_str}"
        );
    }

    #[test]
    fn test_dns_resolution_failure() {
        let client = OpenAiClient::new()
            .with_api_key("sk-proj-test-key-for-testing-only1234567890")
            .with_endpoint("http://this.domain.definitely.does.not.exist.test");

        let result = client.complete("test prompt");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, Error::OperationFailed { .. }));
    }
}

// ============================================================================
// Resilient Provider Tests
// ============================================================================

mod resilient_provider {
    use super::*;

    /// Mock LLM provider for testing resilience behavior.
    struct MockProvider {
        name: &'static str,
        call_count: Arc<AtomicU32>,
        fail_until: u32,
        failure_type: MockFailureType,
    }

    #[derive(Clone, Copy)]
    enum MockFailureType {
        Timeout,
        ConnectionRefused,
        RateLimit,
        BadRequest,
    }

    impl MockProvider {
        fn new(name: &'static str, fail_until: u32, failure_type: MockFailureType) -> Self {
            Self {
                name,
                call_count: Arc::new(AtomicU32::new(0)),
                fail_until,
                failure_type,
            }
        }

        fn calls(&self) -> u32 {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    impl LlmProvider for MockProvider {
        fn name(&self) -> &'static str {
            self.name
        }

        fn complete(&self, _prompt: &str) -> subcog::Result<String> {
            let count = self.call_count.fetch_add(1, Ordering::SeqCst) + 1;

            if count <= self.fail_until {
                let cause = match self.failure_type {
                    MockFailureType::Timeout => "Request timeout after 30s".to_string(),
                    MockFailureType::ConnectionRefused => {
                        "connect error: connection refused".to_string()
                    },
                    MockFailureType::RateLimit => "429 Too many requests".to_string(),
                    MockFailureType::BadRequest => "400 Bad Request".to_string(),
                };
                return Err(Error::OperationFailed {
                    operation: "mock_complete".to_string(),
                    cause,
                });
            }

            Ok("Success response".to_string())
        }

        fn analyze_for_capture(
            &self,
            _content: &str,
        ) -> subcog::Result<subcog::llm::CaptureAnalysis> {
            Ok(subcog::llm::CaptureAnalysis {
                should_capture: false,
                confidence: 0.0,
                suggested_namespace: None,
                suggested_tags: vec![],
                reasoning: "Mock analysis".to_string(),
            })
        }
    }

    #[test]
    fn test_resilient_provider_succeeds_on_first_try() {
        let mock = MockProvider::new("test", 0, MockFailureType::Timeout);
        let config = LlmResilienceConfig {
            max_retries: 3,
            retry_backoff_ms: 1, // Minimal delay for tests
            ..Default::default()
        };

        let resilient = ResilientLlmProvider::new(mock, config);
        let result = resilient.complete("test");

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Success response");
    }

    #[test]
    fn test_resilient_provider_retries_on_timeout() {
        let mock = MockProvider::new("test", 2, MockFailureType::Timeout);
        let call_count = mock.call_count.clone();

        let config = LlmResilienceConfig {
            max_retries: 3,
            retry_backoff_ms: 1, // Minimal delay for tests
            ..Default::default()
        };

        let resilient = ResilientLlmProvider::new(mock, config);
        let result = resilient.complete("test");

        // Should succeed on third attempt
        assert!(result.is_ok());
        // Should have made 3 calls (2 failures + 1 success)
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_resilient_provider_retries_on_connection_error() {
        let mock = MockProvider::new("test", 1, MockFailureType::ConnectionRefused);
        let call_count = mock.call_count.clone();

        let config = LlmResilienceConfig {
            max_retries: 3,
            retry_backoff_ms: 1,
            ..Default::default()
        };

        let resilient = ResilientLlmProvider::new(mock, config);
        let result = resilient.complete("test");

        assert!(result.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_resilient_provider_retries_on_rate_limit() {
        let mock = MockProvider::new("test", 2, MockFailureType::RateLimit);
        let call_count = mock.call_count.clone();

        let config = LlmResilienceConfig {
            max_retries: 3,
            retry_backoff_ms: 1,
            ..Default::default()
        };

        let resilient = ResilientLlmProvider::new(mock, config);
        let result = resilient.complete("test");

        assert!(result.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_resilient_provider_does_not_retry_bad_request() {
        let mock = MockProvider::new("test", 10, MockFailureType::BadRequest);
        let call_count = mock.call_count.clone();

        let config = LlmResilienceConfig {
            max_retries: 3,
            retry_backoff_ms: 1,
            ..Default::default()
        };

        let resilient = ResilientLlmProvider::new(mock, config);
        let result = resilient.complete("test");

        // Should fail without retries (400 is not retryable)
        assert!(result.is_err());
        // Should have made only 1 call
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_resilient_provider_exhausts_retries() {
        let mock = MockProvider::new("test", 10, MockFailureType::Timeout);
        let call_count = mock.call_count.clone();

        let config = LlmResilienceConfig {
            max_retries: 3,
            retry_backoff_ms: 1,
            ..Default::default()
        };

        let resilient = ResilientLlmProvider::new(mock, config);
        let result = resilient.complete("test");

        // Should fail after exhausting retries
        assert!(result.is_err());
        // Should have made max_retries + 1 calls (initial + retries)
        assert_eq!(call_count.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn test_resilient_provider_preserves_name() {
        let mock = MockProvider::new("custom_provider", 0, MockFailureType::Timeout);
        let config = LlmResilienceConfig::default();

        let resilient = ResilientLlmProvider::new(mock, config);
        assert_eq!(resilient.name(), "custom_provider");
    }
}

// ============================================================================
// Circuit Breaker Integration Tests
// ============================================================================

mod circuit_breaker_integration {
    use super::*;

    /// Provider that always fails with the specified error.
    struct AlwaysFailProvider {
        name: &'static str,
        call_count: Arc<AtomicU32>,
        error_message: String,
    }

    impl AlwaysFailProvider {
        fn new(name: &'static str, error_message: &str) -> Self {
            Self {
                name,
                call_count: Arc::new(AtomicU32::new(0)),
                error_message: error_message.to_string(),
            }
        }

        fn calls(&self) -> u32 {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    impl LlmProvider for AlwaysFailProvider {
        fn name(&self) -> &'static str {
            self.name
        }

        fn complete(&self, _prompt: &str) -> subcog::Result<String> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Err(Error::OperationFailed {
                operation: "mock_complete".to_string(),
                cause: self.error_message.clone(),
            })
        }

        fn analyze_for_capture(
            &self,
            _content: &str,
        ) -> subcog::Result<subcog::llm::CaptureAnalysis> {
            Err(Error::OperationFailed {
                operation: "mock_analyze".to_string(),
                cause: self.error_message.clone(),
            })
        }
    }

    #[test]
    fn test_circuit_breaker_opens_after_failures() {
        // Use a non-retryable error so we don't have retries inflating call count
        let provider = AlwaysFailProvider::new("test", "400 Bad Request");
        let call_count = provider.call_count.clone();

        let config = LlmResilienceConfig {
            max_retries: 0,               // No retries
            breaker_failure_threshold: 3, // Open after 3 failures
            breaker_reset_timeout_ms: 10_000,
            ..Default::default()
        };

        let resilient = ResilientLlmProvider::new(provider, config);

        // First 3 calls should fail and trip the breaker
        for _ in 0..3 {
            let _ = resilient.complete("test");
        }
        assert_eq!(call_count.load(Ordering::SeqCst), 3);

        // Next calls should be blocked by circuit breaker (no underlying calls)
        let result = resilient.complete("test");
        assert!(result.is_err());

        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("circuit breaker") || err_str.contains("open"),
            "Expected circuit breaker error, got: {err_str}"
        );

        // Call count should not increase (blocked by breaker)
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_circuit_breaker_transitions_to_half_open() {
        let provider = AlwaysFailProvider::new("test", "400 Bad Request");
        let call_count = provider.call_count.clone();

        let config = LlmResilienceConfig {
            max_retries: 0,
            breaker_failure_threshold: 1, // Open immediately
            breaker_reset_timeout_ms: 10, // Very short reset for testing
            breaker_half_open_max_calls: 1,
            ..Default::default()
        };

        let resilient = ResilientLlmProvider::new(provider, config);

        // Trip the breaker
        let _ = resilient.complete("test");
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Wait for reset timeout
        std::thread::sleep(Duration::from_millis(20));

        // Next call should be allowed (half-open state)
        let _ = resilient.complete("test");
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }
}

// ============================================================================
// Configuration Integration Tests
// ============================================================================

mod config_integration {
    use super::*;

    #[test]
    fn test_resilience_config_default() {
        let config = LlmResilienceConfig::default();

        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_backoff_ms, 100);
        assert_eq!(config.breaker_failure_threshold, 3);
        assert_eq!(config.breaker_reset_timeout_ms, 30_000);
        assert!((config.error_budget_ratio - 0.05).abs() < 0.001);
    }

    #[test]
    fn test_resilience_config_from_env() {
        // This tests that from_env() doesn't panic and returns valid config
        let config = LlmResilienceConfig::from_env();

        // Should have reasonable defaults
        assert!(config.max_retries > 0);
        assert!(config.retry_backoff_ms > 0);
        assert!(config.breaker_failure_threshold > 0);
    }

    #[test]
    fn test_resilience_config_from_llm_config() {
        let llm_config = subcog::config::LlmConfig {
            max_retries: Some(5),
            retry_backoff_ms: Some(200),
            breaker_failure_threshold: Some(10),
            breaker_reset_ms: Some(60_000),
            breaker_half_open_max_calls: Some(2),
            latency_slo_ms: Some(5_000),
            error_budget_ratio: Some(0.10),
            error_budget_window_secs: Some(600),
            ..Default::default()
        };

        let config = LlmResilienceConfig::from_config(&llm_config);

        assert_eq!(config.max_retries, 5);
        assert_eq!(config.retry_backoff_ms, 200);
        assert_eq!(config.breaker_failure_threshold, 10);
        assert_eq!(config.breaker_reset_timeout_ms, 60_000);
        assert_eq!(config.breaker_half_open_max_calls, 2);
        assert_eq!(config.latency_slo_ms, 5_000);
        assert!((config.error_budget_ratio - 0.10).abs() < 0.001);
        assert_eq!(config.error_budget_window_secs, 600);
    }
}

// ============================================================================
// Provider-Specific Error Message Tests
// ============================================================================

mod error_messages {
    use super::*;

    #[test]
    fn test_anthropic_error_includes_provider_name() {
        let client = AnthropicClient::new().with_endpoint("http://127.0.0.1:59994");

        let result = client.complete("test");
        assert!(result.is_err());

        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("anthropic") || err_str.contains("connect"),
            "Error should identify provider or connection issue: {err_str}"
        );
    }

    #[test]
    fn test_openai_error_includes_provider_name() {
        let client = OpenAiClient::new()
            .with_api_key("sk-proj-test-key-for-testing-only1234567890")
            .with_endpoint("http://127.0.0.1:59993");

        let result = client.complete("test");
        assert!(result.is_err());

        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("openai") || err_str.contains("connect"),
            "Error should identify provider or connection issue: {err_str}"
        );
    }
}
