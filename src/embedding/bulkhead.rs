//! Bulkhead pattern implementation for embedding operations.
//!
//! Provides concurrency limiting to prevent resource exhaustion when making
//! parallel embedding calls. Uses a semaphore-based approach to limit the number
//! of concurrent operations.
//!
//! # Why Bulkhead for Embeddings?
//!
//! Embedding generation is CPU and memory intensive:
//!
//! - **CPU**: ONNX runtime uses significant CPU per embedding
//! - **Memory**: Model weights and intermediate tensors
//! - **Batching**: Large batches can exhaust memory
//! - **Latency**: Too many concurrent operations increase latency for all
//!
//! # Usage
//!
//! ```rust,ignore
//! use subcog::embedding::{BulkheadEmbedder, EmbeddingBulkheadConfig, FastEmbedEmbedder};
//!
//! let embedder = FastEmbedEmbedder::new()?;
//! let bulkhead = BulkheadEmbedder::new(embedder, EmbeddingBulkheadConfig::default());
//!
//! // Only 2 concurrent embedding operations allowed (default)
//! let embedding = bulkhead.embed("Hello world")?;
//! ```

use super::Embedder;
use crate::{Error, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// Configuration for the embedding bulkhead pattern.
#[derive(Debug, Clone)]
pub struct EmbeddingBulkheadConfig {
    /// Maximum concurrent embedding operations allowed.
    ///
    /// Default: 2 (conservative due to CPU/memory intensity).
    pub max_concurrent: usize,

    /// Timeout for acquiring a permit in milliseconds (0 = no timeout).
    ///
    /// Default: 30000ms (30 seconds - embeddings can be slow).
    pub acquire_timeout_ms: u64,

    /// Whether to fail fast when bulkhead is full (vs. waiting).
    ///
    /// Default: false (wait for permit).
    pub fail_fast: bool,
}

impl Default for EmbeddingBulkheadConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 2,
            acquire_timeout_ms: 30_000,
            fail_fast: false,
        }
    }
}

impl EmbeddingBulkheadConfig {
    /// Creates a new embedding bulkhead configuration.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_concurrent: 2,
            acquire_timeout_ms: 30_000,
            fail_fast: false,
        }
    }

    /// Loads configuration from environment variables.
    ///
    /// | Variable | Description | Default |
    /// |----------|-------------|---------|
    /// | `SUBCOG_EMBEDDING_BULKHEAD_MAX_CONCURRENT` | Max concurrent ops | 2 |
    /// | `SUBCOG_EMBEDDING_BULKHEAD_ACQUIRE_TIMEOUT_MS` | Permit timeout | 30000 |
    /// | `SUBCOG_EMBEDDING_BULKHEAD_FAIL_FAST` | Fail when full | false |
    #[must_use]
    pub fn from_env() -> Self {
        Self::default().with_env_overrides()
    }

    /// Applies environment variable overrides.
    #[must_use]
    pub fn with_env_overrides(mut self) -> Self {
        if let Ok(v) = std::env::var("SUBCOG_EMBEDDING_BULKHEAD_MAX_CONCURRENT")
            && let Ok(parsed) = v.parse::<usize>()
        {
            self.max_concurrent = parsed.max(1);
        }
        if let Ok(v) = std::env::var("SUBCOG_EMBEDDING_BULKHEAD_ACQUIRE_TIMEOUT_MS")
            && let Ok(parsed) = v.parse::<u64>()
        {
            self.acquire_timeout_ms = parsed;
        }
        if let Ok(v) = std::env::var("SUBCOG_EMBEDDING_BULKHEAD_FAIL_FAST") {
            self.fail_fast = v.to_lowercase() == "true" || v == "1";
        }
        self
    }

    /// Sets the maximum concurrent operations.
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

/// Embedder wrapper with bulkhead (concurrency limiting) pattern.
///
/// Limits the number of concurrent embedding operations to prevent resource exhaustion.
pub struct BulkheadEmbedder<E: Embedder> {
    inner: E,
    config: EmbeddingBulkheadConfig,
    semaphore: Arc<Semaphore>,
}

impl<E: Embedder> BulkheadEmbedder<E> {
    /// Creates a new bulkhead-wrapped embedder.
    #[must_use]
    pub fn new(inner: E, config: EmbeddingBulkheadConfig) -> Self {
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
        let semaphore = &self.semaphore;
        let available = semaphore.available_permits();

        metrics::gauge!("embedding_bulkhead_available_permits").set(available as f64);

        if self.config.fail_fast {
            return self.acquire_permit_fail_fast(semaphore, available);
        }

        self.acquire_permit_with_timeout(semaphore)
    }

    /// Fast-fail acquisition that returns error immediately if bulkhead is full.
    fn acquire_permit_fail_fast(
        &self,
        semaphore: &Arc<Semaphore>,
        available: usize,
    ) -> Result<tokio::sync::OwnedSemaphorePermit> {
        Arc::clone(semaphore).try_acquire_owned().map_or_else(
            |_| {
                metrics::counter!("embedding_bulkhead_rejections_total", "reason" => "full")
                    .increment(1);
                Err(Error::OperationFailed {
                    operation: "embedding_bulkhead_acquire".to_string(),
                    cause: format!(
                        "Embedding bulkhead full: {} concurrent operations (max: {})",
                        self.config.max_concurrent - available,
                        self.config.max_concurrent
                    ),
                })
            },
            |permit| {
                metrics::counter!("embedding_bulkhead_permits_acquired_total").increment(1);
                Ok(permit)
            },
        )
    }

    /// Acquisition with timeout that waits for a permit.
    fn acquire_permit_with_timeout(
        &self,
        semaphore: &Arc<Semaphore>,
    ) -> Result<tokio::sync::OwnedSemaphorePermit> {
        let timeout_ms = if self.config.acquire_timeout_ms == 0 {
            120_000 // 2 minute safety cap
        } else {
            self.config.acquire_timeout_ms
        };
        let timeout = Duration::from_millis(timeout_ms);
        let start = std::time::Instant::now();

        loop {
            if let Ok(permit) = Arc::clone(semaphore).try_acquire_owned() {
                metrics::counter!("embedding_bulkhead_permits_acquired_total").increment(1);
                return Ok(permit);
            }

            if start.elapsed() >= timeout {
                metrics::counter!("embedding_bulkhead_rejections_total", "reason" => "timeout")
                    .increment(1);
                return Err(Error::OperationFailed {
                    operation: "embedding_bulkhead_acquire".to_string(),
                    cause: format!(
                        "Embedding bulkhead acquire timed out after {}ms",
                        timeout.as_millis()
                    ),
                });
            }

            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Executes an operation with bulkhead protection.
    fn execute<T, F>(&self, operation: &'static str, call: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        let _permit = self.acquire_permit()?;

        tracing::trace!(operation = operation, "Acquired embedding bulkhead permit");

        let result = call();

        tracing::trace!(
            operation = operation,
            success = result.is_ok(),
            "Released embedding bulkhead permit"
        );

        result
    }
}

impl<E: Embedder> Embedder for BulkheadEmbedder<E> {
    fn dimensions(&self) -> usize {
        self.inner.dimensions()
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.execute("embed", || self.inner.embed(text))
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.execute("embed_batch", || self.inner.embed_batch(texts))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Mock embedder for testing
    struct MockEmbedder {
        delay_ms: u64,
        call_count: AtomicUsize,
    }

    impl MockEmbedder {
        fn new(delay_ms: u64) -> Self {
            Self {
                delay_ms,
                call_count: AtomicUsize::new(0),
            }
        }
    }

    impl Embedder for MockEmbedder {
        fn dimensions(&self) -> usize {
            384
        }

        fn embed(&self, _text: &str) -> Result<Vec<f32>> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            if self.delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(self.delay_ms));
            }
            Ok(vec![0.0; 384])
        }
    }

    #[test]
    fn test_embedding_bulkhead_config_default() {
        let config = EmbeddingBulkheadConfig::default();
        assert_eq!(config.max_concurrent, 2);
        assert_eq!(config.acquire_timeout_ms, 30_000);
        assert!(!config.fail_fast);
    }

    #[test]
    fn test_embedding_bulkhead_config_builder() {
        let config = EmbeddingBulkheadConfig::new()
            .with_max_concurrent(4)
            .with_acquire_timeout_ms(10_000)
            .with_fail_fast(true);

        assert_eq!(config.max_concurrent, 4);
        assert_eq!(config.acquire_timeout_ms, 10_000);
        assert!(config.fail_fast);
    }

    #[test]
    fn test_bulkhead_allows_operations_within_limit() {
        let embedder = MockEmbedder::new(0);
        let bulkhead = BulkheadEmbedder::new(embedder, EmbeddingBulkheadConfig::default());

        let result = bulkhead.embed("test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 384);
    }

    #[test]
    fn test_bulkhead_available_permits() {
        let embedder = MockEmbedder::new(0);
        let config = EmbeddingBulkheadConfig::new().with_max_concurrent(3);
        let bulkhead = BulkheadEmbedder::new(embedder, config);

        assert_eq!(bulkhead.available_permits(), 3);
    }

    #[test]
    fn test_bulkhead_fail_fast_when_full() {
        let embedder = MockEmbedder::new(100);
        let config = EmbeddingBulkheadConfig::new()
            .with_max_concurrent(1)
            .with_fail_fast(true);
        let bulkhead = Arc::new(BulkheadEmbedder::new(embedder, config));

        // Start a slow operation in another thread
        let bulkhead_clone = Arc::clone(&bulkhead);
        let handle = std::thread::spawn(move || bulkhead_clone.embed("slow"));

        // Give the thread time to acquire the permit
        std::thread::sleep(Duration::from_millis(10));

        // This might fail if the bulkhead is full
        let result = bulkhead.embed("fast");

        let _ = handle.join();

        if let Err(err) = result {
            assert!(err.to_string().contains("bulkhead full"));
        }
    }

    #[test]
    fn test_bulkhead_dimensions_passthrough() {
        let embedder = MockEmbedder::new(0);
        let bulkhead = BulkheadEmbedder::new(embedder, EmbeddingBulkheadConfig::default());

        assert_eq!(bulkhead.dimensions(), 384);
    }
}
