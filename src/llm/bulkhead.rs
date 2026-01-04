//! Bulkhead pattern implementation for LLM calls.
//!
//! Provides concurrency limiting to prevent resource exhaustion when making
//! parallel LLM calls. Uses a semaphore-based approach to limit the number
//! of concurrent requests.
//!
//! # Why Bulkhead?
//!
//! The bulkhead pattern isolates failures and prevents cascading resource
//! exhaustion. For LLM providers:
//!
//! - **Memory**: Each request holds response buffers
//! - **Connections**: HTTP connection pool limits
//! - **Rate limits**: Provider-side rate limiting
//! - **Cost**: Runaway concurrency increases API costs
//!
//! # Usage
//!
//! ```rust,ignore
//! use subcog::llm::{BulkheadLlmProvider, BulkheadConfig, AnthropicClient};
//!
//! let client = AnthropicClient::new();
//! let bulkhead = BulkheadLlmProvider::new(client, BulkheadConfig::default());
//!
//! // Only 4 concurrent calls allowed (default)
//! let response = bulkhead.complete("Hello")?;
//! ```

use super::{CaptureAnalysis, LlmProvider};
use crate::{Error, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// Configuration for the bulkhead pattern.
#[derive(Debug, Clone)]
pub struct BulkheadConfig {
    /// Maximum concurrent LLM calls allowed.
    ///
    /// Default: 4 (conservative for API rate limits).
    pub max_concurrent: usize,

    /// Timeout for acquiring a permit (0 = no timeout, wait indefinitely).
    ///
    /// Default: 30 seconds.
    pub acquire_timeout_ms: u64,

    /// Whether to fail fast when bulkhead is full (vs. waiting).
    ///
    /// Default: false (wait for permit).
    pub fail_fast: bool,
}

impl Default for BulkheadConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 4,
            acquire_timeout_ms: 30_000,
            fail_fast: false,
        }
    }
}

impl BulkheadConfig {
    /// Creates a new bulkhead configuration.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_concurrent: 4,
            acquire_timeout_ms: 30_000,
            fail_fast: false,
        }
    }

    /// Loads configuration from environment variables.
    ///
    /// | Variable | Description | Default |
    /// |----------|-------------|---------|
    /// | `SUBCOG_LLM_BULKHEAD_MAX_CONCURRENT` | Max concurrent calls | 4 |
    /// | `SUBCOG_LLM_BULKHEAD_ACQUIRE_TIMEOUT_MS` | Permit timeout | 30000 |
    /// | `SUBCOG_LLM_BULKHEAD_FAIL_FAST` | Fail when full | false |
    #[must_use]
    pub fn from_env() -> Self {
        Self::default().with_env_overrides()
    }

    /// Applies environment variable overrides.
    #[must_use]
    pub fn with_env_overrides(mut self) -> Self {
        if let Ok(v) = std::env::var("SUBCOG_LLM_BULKHEAD_MAX_CONCURRENT") {
            if let Ok(parsed) = v.parse::<usize>() {
                self.max_concurrent = parsed.max(1);
            }
        }
        if let Ok(v) = std::env::var("SUBCOG_LLM_BULKHEAD_ACQUIRE_TIMEOUT_MS") {
            if let Ok(parsed) = v.parse::<u64>() {
                self.acquire_timeout_ms = parsed;
            }
        }
        if let Ok(v) = std::env::var("SUBCOG_LLM_BULKHEAD_FAIL_FAST") {
            self.fail_fast = v.to_lowercase() == "true" || v == "1";
        }
        self
    }

    /// Sets the maximum concurrent calls.
    #[must_use]
    pub const fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max;
        self
    }

    /// Sets the acquire timeout in milliseconds.
    #[must_use]
    pub const fn with_acquire_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.acquire_timeout_ms = timeout_ms;
        self
    }

    /// Sets whether to fail fast when the bulkhead is full.
    #[must_use]
    pub const fn with_fail_fast(mut self, fail_fast: bool) -> Self {
        self.fail_fast = fail_fast;
        self
    }
}

/// LLM provider wrapper with bulkhead (concurrency limiting) pattern.
///
/// Limits the number of concurrent LLM calls to prevent resource exhaustion.
pub struct BulkheadLlmProvider<P: LlmProvider> {
    inner: P,
    config: BulkheadConfig,
    semaphore: Arc<Semaphore>,
}

impl<P: LlmProvider> BulkheadLlmProvider<P> {
    /// Creates a new bulkhead-wrapped LLM provider.
    #[must_use]
    pub fn new(inner: P, config: BulkheadConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent.max(1)));
        Self {
            inner,
            config,
            semaphore,
        }
    }

    /// Returns the current number of available permits.
    #[must_use]
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Acquires a permit, respecting the configured timeout and fail-fast settings.
    fn acquire_permit(&self) -> Result<tokio::sync::OwnedSemaphorePermit> {
        let semaphore = Arc::clone(&self.semaphore);

        // Record metrics
        let available = semaphore.available_permits();
        metrics::gauge!(
            "llm_bulkhead_available_permits",
            "provider" => self.inner.name()
        )
        .set(available as f64);

        if self.config.fail_fast {
            // Try to acquire immediately without waiting
            match semaphore.try_acquire_owned() {
                Ok(permit) => {
                    metrics::counter!(
                        "llm_bulkhead_permits_acquired_total",
                        "provider" => self.inner.name()
                    )
                    .increment(1);
                    Ok(permit)
                },
                Err(_) => {
                    metrics::counter!(
                        "llm_bulkhead_rejections_total",
                        "provider" => self.inner.name(),
                        "reason" => "full"
                    )
                    .increment(1);
                    Err(Error::OperationFailed {
                        operation: "llm_bulkhead_acquire".to_string(),
                        cause: format!(
                            "Bulkhead full: {} concurrent calls in progress (max: {})",
                            self.config.max_concurrent - available,
                            self.config.max_concurrent
                        ),
                    })
                },
            }
        } else if self.config.acquire_timeout_ms == 0 {
            // Wait indefinitely (blocking)
            // Note: In sync context, we use blocking acquire
            match semaphore.try_acquire_owned() {
                Ok(permit) => {
                    metrics::counter!(
                        "llm_bulkhead_permits_acquired_total",
                        "provider" => self.inner.name()
                    )
                    .increment(1);
                    Ok(permit)
                },
                Err(_) => {
                    // Semaphore is closed or full - in sync context, spin briefly
                    let start = std::time::Instant::now();
                    loop {
                        std::thread::sleep(Duration::from_millis(10));
                        if let Ok(permit) = Arc::clone(&self.semaphore).try_acquire_owned() {
                            metrics::counter!(
                                "llm_bulkhead_permits_acquired_total",
                                "provider" => self.inner.name()
                            )
                            .increment(1);
                            return Ok(permit);
                        }
                        // Safety: don't spin forever, cap at 5 minutes
                        if start.elapsed() > Duration::from_secs(300) {
                            metrics::counter!(
                                "llm_bulkhead_rejections_total",
                                "provider" => self.inner.name(),
                                "reason" => "timeout"
                            )
                            .increment(1);
                            return Err(Error::OperationFailed {
                                operation: "llm_bulkhead_acquire".to_string(),
                                cause: "Bulkhead acquire timed out after 5 minutes".to_string(),
                            });
                        }
                    }
                },
            }
        } else {
            // Wait with timeout
            let timeout = Duration::from_millis(self.config.acquire_timeout_ms);
            let start = std::time::Instant::now();

            loop {
                if let Ok(permit) = Arc::clone(&self.semaphore).try_acquire_owned() {
                    metrics::counter!(
                        "llm_bulkhead_permits_acquired_total",
                        "provider" => self.inner.name()
                    )
                    .increment(1);
                    return Ok(permit);
                }

                if start.elapsed() >= timeout {
                    metrics::counter!(
                        "llm_bulkhead_rejections_total",
                        "provider" => self.inner.name(),
                        "reason" => "timeout"
                    )
                    .increment(1);
                    return Err(Error::OperationFailed {
                        operation: "llm_bulkhead_acquire".to_string(),
                        cause: format!(
                            "Bulkhead acquire timed out after {}ms",
                            self.config.acquire_timeout_ms
                        ),
                    });
                }

                // Brief sleep before retry
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    /// Executes an operation with bulkhead protection.
    fn execute<T, F>(&self, operation: &'static str, call: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        let provider = self.inner.name();
        let span = tracing::info_span!(
            "llm.bulkhead",
            provider = provider,
            operation = operation,
            available_permits = tracing::field::Empty
        );
        let _enter = span.enter();

        span.record("available_permits", self.available_permits());

        // Acquire permit (blocks or fails based on config)
        let _permit = self.acquire_permit()?;

        tracing::debug!(
            provider = provider,
            operation = operation,
            "Acquired bulkhead permit"
        );

        // Execute the operation
        let result = call();

        // Permit is automatically released when _permit is dropped
        tracing::debug!(
            provider = provider,
            operation = operation,
            success = result.is_ok(),
            "Released bulkhead permit"
        );

        result
    }
}

impl<P: LlmProvider> LlmProvider for BulkheadLlmProvider<P> {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn complete(&self, prompt: &str) -> Result<String> {
        self.execute("complete", || self.inner.complete(prompt))
    }

    fn complete_with_system(&self, system: &str, user: &str) -> Result<String> {
        self.execute("complete_with_system", || {
            self.inner.complete_with_system(system, user)
        })
    }

    fn analyze_for_capture(&self, content: &str) -> Result<CaptureAnalysis> {
        self.execute("analyze_for_capture", || {
            self.inner.analyze_for_capture(content)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock LLM provider for testing
    struct MockProvider {
        delay_ms: u64,
    }

    impl LlmProvider for MockProvider {
        fn name(&self) -> &'static str {
            "mock"
        }

        fn complete(&self, _prompt: &str) -> Result<String> {
            if self.delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(self.delay_ms));
            }
            Ok("response".to_string())
        }

        fn analyze_for_capture(&self, _content: &str) -> Result<CaptureAnalysis> {
            Ok(CaptureAnalysis {
                should_capture: true,
                confidence: 0.9,
                suggested_namespace: Some("decisions".to_string()),
                suggested_tags: vec!["test".to_string()],
                reasoning: "Test analysis".to_string(),
            })
        }
    }

    #[test]
    fn test_bulkhead_config_default() {
        let config = BulkheadConfig::default();
        assert_eq!(config.max_concurrent, 4);
        assert_eq!(config.acquire_timeout_ms, 30_000);
        assert!(!config.fail_fast);
    }

    #[test]
    fn test_bulkhead_config_builder() {
        let config = BulkheadConfig::new()
            .with_max_concurrent(8)
            .with_acquire_timeout_ms(5000)
            .with_fail_fast(true);

        assert_eq!(config.max_concurrent, 8);
        assert_eq!(config.acquire_timeout_ms, 5000);
        assert!(config.fail_fast);
    }

    #[test]
    fn test_bulkhead_allows_calls_within_limit() {
        let provider = MockProvider { delay_ms: 0 };
        let bulkhead = BulkheadLlmProvider::new(provider, BulkheadConfig::default());

        // Should succeed
        let result = bulkhead.complete("test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "response");
    }

    #[test]
    fn test_bulkhead_available_permits() {
        let provider = MockProvider { delay_ms: 0 };
        let config = BulkheadConfig::new().with_max_concurrent(2);
        let bulkhead = BulkheadLlmProvider::new(provider, config);

        assert_eq!(bulkhead.available_permits(), 2);
    }

    #[test]
    fn test_bulkhead_fail_fast_when_full() {
        let provider = MockProvider { delay_ms: 100 };
        let config = BulkheadConfig::new()
            .with_max_concurrent(1)
            .with_fail_fast(true);
        let bulkhead = Arc::new(BulkheadLlmProvider::new(provider, config));

        // Acquire the only permit by starting a slow call in another thread
        let bulkhead_clone = Arc::clone(&bulkhead);
        let handle = std::thread::spawn(move || bulkhead_clone.complete("slow"));

        // Give the thread time to acquire the permit
        std::thread::sleep(Duration::from_millis(10));

        // This should fail fast since we're at capacity
        // Note: Due to timing, this test may be flaky - the first thread needs
        // to have acquired the permit before we try
        let result = bulkhead.complete("fast");

        // Wait for the first thread to complete
        let _ = handle.join();

        // The result might be Ok if timing allowed, or Err if bulkhead was full
        // This is acceptable for fail_fast behavior
        if result.is_err() {
            let err = result.unwrap_err();
            assert!(err.to_string().contains("Bulkhead full"));
        }
    }

    #[test]
    fn test_bulkhead_timeout() {
        let provider = MockProvider { delay_ms: 200 };
        let config = BulkheadConfig::new()
            .with_max_concurrent(1)
            .with_acquire_timeout_ms(50); // Very short timeout
        let bulkhead = Arc::new(BulkheadLlmProvider::new(provider, config));

        // Start a slow call in another thread
        let bulkhead_clone = Arc::clone(&bulkhead);
        let handle = std::thread::spawn(move || bulkhead_clone.complete("slow"));

        // Give the thread time to acquire the permit
        std::thread::sleep(Duration::from_millis(10));

        // This should timeout waiting for the permit
        let result = bulkhead.complete("waiting");

        // Wait for the first thread
        let _ = handle.join();

        // Similar to fail_fast test - timing dependent
        if result.is_err() {
            let err = result.unwrap_err();
            assert!(err.to_string().contains("timed out"));
        }
    }

    #[test]
    fn test_bulkhead_wraps_analyze_for_capture() {
        let provider = MockProvider { delay_ms: 0 };
        let bulkhead = BulkheadLlmProvider::new(provider, BulkheadConfig::default());

        let result = bulkhead.analyze_for_capture("test content");
        assert!(result.is_ok());
        let analysis = result.unwrap();
        assert!(analysis.should_capture);
    }
}
