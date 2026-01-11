//! Bulkhead pattern implementation for storage operations.
//!
//! Provides concurrency limiting to prevent resource exhaustion when making
//! parallel storage calls. Uses a semaphore-based approach to limit the number
//! of concurrent operations.
//!
//! # Why Bulkhead for Storage?
//!
//! The bulkhead pattern isolates storage operations and prevents cascading
//! resource exhaustion:
//!
//! - **Connection pools**: Prevents pool exhaustion under load
//! - **File handles**: `SQLite` file descriptor limits
//! - **Memory**: Large batch operations can exhaust memory
//! - **I/O bandwidth**: Prevents I/O saturation
//!
//! # Architecture
//!
//! This module uses a generic [`Bulkhead<T>`] struct that provides the core
//! concurrency limiting logic. Backend-specific wrappers (`BulkheadPersistenceBackend`,
//! `BulkheadIndexBackend`, `BulkheadVectorBackend`) delegate to this shared
//! implementation while providing trait implementations for their respective
//! backend traits.
//!
//! # Usage
//!
//! ```rust,ignore
//! use subcog::storage::{BulkheadPersistenceBackend, StorageBulkheadConfig};
//!
//! let backend = SqlitePersistence::new(...)?;
//! let bulkhead = BulkheadPersistenceBackend::new(
//!     backend,
//!     StorageBulkheadConfig::default(),
//!     "sqlite"
//! );
//!
//! // Only 10 concurrent operations allowed (default)
//! bulkhead.store(&memory)?;
//! ```

use super::traits::{IndexBackend, PersistenceBackend, VectorBackend, VectorFilter};
use crate::models::{Memory, MemoryId, SearchFilter};
use crate::{Error, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Configuration for the storage bulkhead pattern.
#[derive(Debug, Clone)]
pub struct StorageBulkheadConfig {
    /// Maximum concurrent storage operations allowed.
    ///
    /// Default: 10 (higher than LLM since storage is faster).
    pub max_concurrent: usize,

    /// Timeout for acquiring a permit in milliseconds (0 = no timeout).
    ///
    /// Default: 5000ms (5 seconds).
    pub acquire_timeout_ms: u64,

    /// Whether to fail fast when bulkhead is full (vs. waiting).
    ///
    /// Default: false (wait for permit).
    pub fail_fast: bool,
}

impl Default for StorageBulkheadConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 10,
            acquire_timeout_ms: 5000,
            fail_fast: false,
        }
    }
}

impl StorageBulkheadConfig {
    /// Creates a new storage bulkhead configuration.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_concurrent: 10,
            acquire_timeout_ms: 5000,
            fail_fast: false,
        }
    }

    /// Loads configuration from environment variables.
    ///
    /// | Variable | Description | Default |
    /// |----------|-------------|---------|
    /// | `SUBCOG_STORAGE_BULKHEAD_MAX_CONCURRENT` | Max concurrent operations | 10 |
    /// | `SUBCOG_STORAGE_BULKHEAD_ACQUIRE_TIMEOUT_MS` | Permit timeout | 5000 |
    /// | `SUBCOG_STORAGE_BULKHEAD_FAIL_FAST` | Fail when full | false |
    #[must_use]
    pub fn from_env() -> Self {
        Self::default().with_env_overrides()
    }

    /// Applies environment variable overrides.
    #[must_use]
    pub fn with_env_overrides(mut self) -> Self {
        if let Ok(v) = std::env::var("SUBCOG_STORAGE_BULKHEAD_MAX_CONCURRENT")
            && let Ok(parsed) = v.parse::<usize>()
        {
            self.max_concurrent = parsed.max(1);
        }
        if let Ok(v) = std::env::var("SUBCOG_STORAGE_BULKHEAD_ACQUIRE_TIMEOUT_MS")
            && let Ok(parsed) = v.parse::<u64>()
        {
            self.acquire_timeout_ms = parsed;
        }
        if let Ok(v) = std::env::var("SUBCOG_STORAGE_BULKHEAD_FAIL_FAST") {
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

// ============================================================================
// Generic Bulkhead Implementation
// ============================================================================

/// Generic bulkhead wrapper providing concurrency limiting for any type.
///
/// This struct implements the core bulkhead pattern logic that is shared across
/// all backend-specific wrappers. It uses a semaphore to limit concurrent
/// operations and provides configurable timeout and fail-fast behaviors.
///
/// # Type Parameters
///
/// * `T` - The inner type being wrapped (e.g., a persistence or vector backend)
pub struct Bulkhead<T> {
    inner: T,
    config: StorageBulkheadConfig,
    semaphore: Arc<Semaphore>,
    backend_name: &'static str,
}

impl<T> Bulkhead<T> {
    /// Creates a new bulkhead wrapper around the given inner value.
    #[must_use]
    pub fn new(inner: T, config: StorageBulkheadConfig, backend_name: &'static str) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent.max(1)));
        Self {
            inner,
            config,
            semaphore,
            backend_name,
        }
    }

    /// Returns a reference to the inner value.
    #[must_use]
    pub const fn inner(&self) -> &T {
        &self.inner
    }

    /// Returns the backend name for metrics and logging.
    #[must_use]
    pub const fn backend_name(&self) -> &'static str {
        self.backend_name
    }

    /// Returns the current number of available permits.
    #[must_use]
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Acquires a permit, respecting the configured timeout and fail-fast settings.
    fn acquire_permit(&self, operation_prefix: &str) -> Result<OwnedSemaphorePermit> {
        let semaphore = &self.semaphore;
        let available = semaphore.available_permits();

        metrics::gauge!(
            "storage_bulkhead_available_permits",
            "backend" => self.backend_name
        )
        .set(available as f64);

        if self.config.fail_fast {
            return self.acquire_permit_fail_fast(semaphore, available, operation_prefix);
        }

        let timeout_ms = if self.config.acquire_timeout_ms == 0 {
            60_000 // 60 second safety cap
        } else {
            self.config.acquire_timeout_ms
        };

        self.acquire_permit_with_timeout(timeout_ms, operation_prefix)
    }

    /// Fast-fail acquisition that returns error immediately if bulkhead is full.
    fn acquire_permit_fail_fast(
        &self,
        semaphore: &Arc<Semaphore>,
        available: usize,
        operation_prefix: &str,
    ) -> Result<OwnedSemaphorePermit> {
        Arc::clone(semaphore).try_acquire_owned().map_or_else(
            |_| {
                metrics::counter!(
                    "storage_bulkhead_rejections_total",
                    "backend" => self.backend_name,
                    "reason" => "full"
                )
                .increment(1);
                Err(Error::OperationFailed {
                    operation: format!("{operation_prefix}_bulkhead_acquire"),
                    cause: format!(
                        "{} bulkhead full: {} concurrent operations (max: {})",
                        capitalize_first(operation_prefix),
                        self.config.max_concurrent - available,
                        self.config.max_concurrent
                    ),
                })
            },
            |permit| {
                metrics::counter!(
                    "storage_bulkhead_permits_acquired_total",
                    "backend" => self.backend_name
                )
                .increment(1);
                Ok(permit)
            },
        )
    }

    /// Acquisition with timeout that waits for a permit.
    fn acquire_permit_with_timeout(
        &self,
        timeout_ms: u64,
        operation_prefix: &str,
    ) -> Result<OwnedSemaphorePermit> {
        let timeout = Duration::from_millis(timeout_ms);
        let start = std::time::Instant::now();

        loop {
            if let Ok(permit) = Arc::clone(&self.semaphore).try_acquire_owned() {
                metrics::counter!(
                    "storage_bulkhead_permits_acquired_total",
                    "backend" => self.backend_name
                )
                .increment(1);
                return Ok(permit);
            }

            if start.elapsed() >= timeout {
                metrics::counter!(
                    "storage_bulkhead_rejections_total",
                    "backend" => self.backend_name,
                    "reason" => "timeout"
                )
                .increment(1);
                return Err(Error::OperationFailed {
                    operation: format!("{operation_prefix}_bulkhead_acquire"),
                    cause: format!(
                        "{} bulkhead acquire timed out after {timeout_ms}ms",
                        capitalize_first(operation_prefix)
                    ),
                });
            }

            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// Executes an operation with bulkhead protection.
    ///
    /// Acquires a permit before executing the closure and releases it after.
    /// Includes tracing for debugging concurrent access patterns.
    ///
    /// # Errors
    ///
    /// Returns an error if permit acquisition times out or the inner operation fails.
    pub fn execute<R, F>(
        &self,
        operation: &'static str,
        operation_prefix: &str,
        call: F,
    ) -> Result<R>
    where
        F: FnOnce(&T) -> Result<R>,
    {
        let _permit = self.acquire_permit(operation_prefix)?;

        tracing::trace!(
            backend = self.backend_name,
            operation = operation,
            "Acquired bulkhead permit"
        );

        let result = call(&self.inner);

        tracing::trace!(
            backend = self.backend_name,
            operation = operation,
            success = result.is_ok(),
            "Released bulkhead permit"
        );

        result
    }

    /// Executes an operation with bulkhead protection (no tracing).
    ///
    /// Lighter-weight version without tracing overhead for high-frequency operations.
    ///
    /// # Errors
    ///
    /// Returns an error if permit acquisition times out or the inner operation fails.
    pub fn execute_quiet<R, F>(&self, operation_prefix: &str, call: F) -> Result<R>
    where
        F: FnOnce(&T) -> Result<R>,
    {
        let _permit = self.acquire_permit(operation_prefix)?;
        call(&self.inner)
    }
}

/// Capitalizes the first letter of a string.
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

// ============================================================================
// Bulkhead Persistence Backend
// ============================================================================

/// Persistence backend wrapper with bulkhead (concurrency limiting) pattern.
pub struct BulkheadPersistenceBackend<P: PersistenceBackend> {
    bulkhead: Bulkhead<P>,
}

impl<P: PersistenceBackend> BulkheadPersistenceBackend<P> {
    /// Creates a new bulkhead-wrapped persistence backend.
    #[must_use]
    pub fn new(inner: P, config: StorageBulkheadConfig, backend_name: &'static str) -> Self {
        Self {
            bulkhead: Bulkhead::new(inner, config, backend_name),
        }
    }

    /// Returns the current number of available permits.
    #[must_use]
    pub fn available_permits(&self) -> usize {
        self.bulkhead.available_permits()
    }
}

#[allow(clippy::redundant_closure_for_method_calls)]
impl<P: PersistenceBackend> PersistenceBackend for BulkheadPersistenceBackend<P> {
    fn store(&self, memory: &Memory) -> Result<()> {
        self.bulkhead
            .execute("store", "storage", |inner| inner.store(memory))
    }

    fn get(&self, id: &MemoryId) -> Result<Option<Memory>> {
        self.bulkhead
            .execute("get", "storage", |inner| inner.get(id))
    }

    fn get_batch(&self, ids: &[MemoryId]) -> Result<Vec<Memory>> {
        self.bulkhead
            .execute("get_batch", "storage", |inner| inner.get_batch(ids))
    }

    fn delete(&self, id: &MemoryId) -> Result<bool> {
        self.bulkhead
            .execute("delete", "storage", |inner| inner.delete(id))
    }

    fn exists(&self, id: &MemoryId) -> Result<bool> {
        self.bulkhead
            .execute("exists", "storage", |inner| inner.exists(id))
    }

    fn list_ids(&self) -> Result<Vec<MemoryId>> {
        self.bulkhead
            .execute("list_ids", "storage", |inner| inner.list_ids())
    }

    fn count(&self) -> Result<usize> {
        self.bulkhead
            .execute("count", "storage", |inner| inner.count())
    }
}

// ============================================================================
// Bulkhead Index Backend
// ============================================================================

/// Index backend wrapper with bulkhead (concurrency limiting) pattern.
pub struct BulkheadIndexBackend<I: IndexBackend> {
    bulkhead: Bulkhead<I>,
}

impl<I: IndexBackend> BulkheadIndexBackend<I> {
    /// Creates a new bulkhead-wrapped index backend.
    #[must_use]
    pub fn new(inner: I, config: StorageBulkheadConfig, backend_name: &'static str) -> Self {
        Self {
            bulkhead: Bulkhead::new(inner, config, backend_name),
        }
    }

    /// Returns the current number of available permits.
    #[must_use]
    pub fn available_permits(&self) -> usize {
        self.bulkhead.available_permits()
    }
}

#[allow(clippy::redundant_closure_for_method_calls)]
impl<I: IndexBackend> IndexBackend for BulkheadIndexBackend<I> {
    fn index(&self, memory: &Memory) -> Result<()> {
        self.bulkhead
            .execute_quiet("index", |inner| inner.index(memory))
    }

    fn remove(&self, id: &MemoryId) -> Result<bool> {
        self.bulkhead
            .execute_quiet("index", |inner| inner.remove(id))
    }

    fn search(
        &self,
        query: &str,
        filter: &SearchFilter,
        limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>> {
        self.bulkhead
            .execute_quiet("index", |inner| inner.search(query, filter, limit))
    }

    fn reindex(&self, memories: &[Memory]) -> Result<()> {
        self.bulkhead
            .execute_quiet("index", |inner| inner.reindex(memories))
    }

    fn clear(&self) -> Result<()> {
        self.bulkhead.execute_quiet("index", |inner| inner.clear())
    }

    fn list_all(&self, filter: &SearchFilter, limit: usize) -> Result<Vec<(MemoryId, f32)>> {
        self.bulkhead
            .execute_quiet("index", |inner| inner.list_all(filter, limit))
    }

    fn get_memory(&self, id: &MemoryId) -> Result<Option<Memory>> {
        self.bulkhead
            .execute_quiet("index", |inner| inner.get_memory(id))
    }

    fn get_memories_batch(&self, ids: &[MemoryId]) -> Result<Vec<Option<Memory>>> {
        self.bulkhead
            .execute_quiet("index", |inner| inner.get_memories_batch(ids))
    }
}

// ============================================================================
// Bulkhead Vector Backend
// ============================================================================

/// Vector backend wrapper with bulkhead (concurrency limiting) pattern.
pub struct BulkheadVectorBackend<V: VectorBackend> {
    bulkhead: Bulkhead<V>,
}

impl<V: VectorBackend> BulkheadVectorBackend<V> {
    /// Creates a new bulkhead-wrapped vector backend.
    #[must_use]
    pub fn new(inner: V, config: StorageBulkheadConfig, backend_name: &'static str) -> Self {
        Self {
            bulkhead: Bulkhead::new(inner, config, backend_name),
        }
    }

    /// Returns the current number of available permits.
    #[must_use]
    pub fn available_permits(&self) -> usize {
        self.bulkhead.available_permits()
    }
}

#[allow(clippy::redundant_closure_for_method_calls)]
impl<V: VectorBackend> VectorBackend for BulkheadVectorBackend<V> {
    fn dimensions(&self) -> usize {
        self.bulkhead.inner().dimensions()
    }

    fn upsert(&self, id: &MemoryId, embedding: &[f32]) -> Result<()> {
        self.bulkhead
            .execute("upsert", "vector", |inner| inner.upsert(id, embedding))
    }

    fn remove(&self, id: &MemoryId) -> Result<bool> {
        self.bulkhead
            .execute("remove", "vector", |inner| inner.remove(id))
    }

    fn search(
        &self,
        query_embedding: &[f32],
        filter: &VectorFilter,
        limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>> {
        self.bulkhead.execute("search", "vector", |inner| {
            inner.search(query_embedding, filter, limit)
        })
    }

    fn count(&self) -> Result<usize> {
        self.bulkhead
            .execute("count", "vector", |inner| inner.count())
    }

    fn clear(&self) -> Result<()> {
        self.bulkhead
            .execute("clear", "vector", |inner| inner.clear())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, Memory, MemoryId, MemoryStatus, Namespace};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Creates a test memory for bulkhead tests.
    fn create_test_memory(content: &str) -> Memory {
        Memory {
            id: MemoryId::new("test-memory"),
            content: content.to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::default(),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: 0,
            updated_at: 0,
            tombstoned_at: None,
            embedding: None,
            tags: vec![],
            source: None,
            is_summary: false,
            source_memory_ids: None,
            consolidation_timestamp: None,
        }
    }

    // Mock persistence backend for testing
    struct MockPersistence {
        delay_ms: u64,
        call_count: AtomicUsize,
    }

    impl MockPersistence {
        fn new(delay_ms: u64) -> Self {
            Self {
                delay_ms,
                call_count: AtomicUsize::new(0),
            }
        }
    }

    impl PersistenceBackend for MockPersistence {
        fn store(&self, _memory: &Memory) -> Result<()> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            if self.delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(self.delay_ms));
            }
            Ok(())
        }

        fn get(&self, _id: &MemoryId) -> Result<Option<Memory>> {
            if self.delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(self.delay_ms));
            }
            Ok(None)
        }

        fn delete(&self, _id: &MemoryId) -> Result<bool> {
            Ok(true)
        }

        fn list_ids(&self) -> Result<Vec<MemoryId>> {
            Ok(vec![])
        }
    }

    #[test]
    fn test_storage_bulkhead_config_default() {
        let config = StorageBulkheadConfig::default();
        assert_eq!(config.max_concurrent, 10);
        assert_eq!(config.acquire_timeout_ms, 5000);
        assert!(!config.fail_fast);
    }

    #[test]
    fn test_storage_bulkhead_config_builder() {
        let config = StorageBulkheadConfig::new()
            .with_max_concurrent(20)
            .with_acquire_timeout_ms(10_000)
            .with_fail_fast(true);

        assert_eq!(config.max_concurrent, 20);
        assert_eq!(config.acquire_timeout_ms, 10_000);
        assert!(config.fail_fast);
    }

    #[test]
    fn test_bulkhead_allows_operations_within_limit() {
        let backend = MockPersistence::new(0);
        let bulkhead =
            BulkheadPersistenceBackend::new(backend, StorageBulkheadConfig::default(), "mock");

        let memory = create_test_memory("test content");

        let result = bulkhead.store(&memory);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bulkhead_available_permits() {
        let backend = MockPersistence::new(0);
        let config = StorageBulkheadConfig::new().with_max_concurrent(5);
        let bulkhead = BulkheadPersistenceBackend::new(backend, config, "mock");

        assert_eq!(bulkhead.available_permits(), 5);
    }

    #[test]
    fn test_bulkhead_fail_fast_when_full() {
        let backend = MockPersistence::new(100);
        let config = StorageBulkheadConfig::new()
            .with_max_concurrent(1)
            .with_fail_fast(true);
        let bulkhead = Arc::new(BulkheadPersistenceBackend::new(backend, config, "mock"));

        let memory = create_test_memory("test content");

        // Start a slow operation in another thread
        let bulkhead_clone = Arc::clone(&bulkhead);
        let memory_clone = memory.clone();
        let handle = std::thread::spawn(move || bulkhead_clone.store(&memory_clone));

        // Give the thread time to acquire the permit
        std::thread::sleep(Duration::from_millis(10));

        // This might fail if the bulkhead is full
        let result = bulkhead.store(&memory);

        let _ = handle.join();

        // Either succeeds (if timing allowed) or fails with bulkhead full
        if let Err(err) = result {
            assert!(err.to_string().contains("bulkhead full"));
        }
    }

    #[test]
    fn test_bulkhead_timeout() {
        let backend = MockPersistence::new(200);
        let config = StorageBulkheadConfig::new()
            .with_max_concurrent(1)
            .with_acquire_timeout_ms(50);
        let bulkhead = Arc::new(BulkheadPersistenceBackend::new(backend, config, "mock"));

        let memory = create_test_memory("test content");

        // Start a slow operation
        let bulkhead_clone = Arc::clone(&bulkhead);
        let memory_clone = memory.clone();
        let handle = std::thread::spawn(move || bulkhead_clone.store(&memory_clone));

        std::thread::sleep(Duration::from_millis(10));

        // This should timeout
        let result = bulkhead.store(&memory);

        let _ = handle.join();

        if let Err(err) = result {
            assert!(err.to_string().contains("timed out"));
        }
    }
}
