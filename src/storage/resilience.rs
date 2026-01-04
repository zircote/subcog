//! Storage resilience wrapper with circuit breaking.
//!
//! Provides circuit breaker protection for storage backends to prevent cascade failures
//! when backends become unhealthy.
//!
//! # Circuit Breaker States
//!
//! ```text
//! +--------+     failures >= threshold     +------+
//! | Closed | --------------------------->  | Open |
//! +--------+                               +------+
//!     ^                                        |
//!     |  success                               | timeout elapsed
//!     |                                        v
//!     +--------------------------------  +-----------+
//!                                        | Half-Open |
//!                                        +-----------+
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use subcog::storage::resilience::{StorageResilienceConfig, ResilientPersistenceBackend};
//! use subcog::storage::index::SqliteBackend;
//!
//! let backend = SqliteBackend::new(db_path)?;
//! let config = StorageResilienceConfig::default();
//! let resilient = ResilientPersistenceBackend::new(backend, config, "sqlite");
//!
//! // Operations are now protected by circuit breaker
//! resilient.store(&memory)?;
//! ```

use crate::{Error, Result};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Resilience configuration for storage backends.
#[derive(Debug, Clone)]
pub struct StorageResilienceConfig {
    /// Consecutive failures before opening the circuit.
    pub breaker_failure_threshold: u32,
    /// How long to keep the circuit open before half-open.
    pub breaker_reset_timeout_ms: u64,
    /// Maximum trial calls while half-open.
    pub breaker_half_open_max_calls: u32,
}

impl Default for StorageResilienceConfig {
    fn default() -> Self {
        Self {
            breaker_failure_threshold: 5,
            breaker_reset_timeout_ms: 30_000,
            breaker_half_open_max_calls: 1,
        }
    }
}

impl StorageResilienceConfig {
    /// Loads resilience configuration from environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        Self::default().with_env_overrides()
    }

    /// Applies environment variable overrides.
    #[must_use]
    pub fn with_env_overrides(mut self) -> Self {
        if let Ok(v) = std::env::var("SUBCOG_STORAGE_BREAKER_FAILURE_THRESHOLD") {
            if let Ok(parsed) = v.parse::<u32>() {
                self.breaker_failure_threshold = parsed.max(1);
            }
        }
        if let Ok(v) = std::env::var("SUBCOG_STORAGE_BREAKER_RESET_MS") {
            if let Ok(parsed) = v.parse::<u64>() {
                self.breaker_reset_timeout_ms = parsed;
            }
        }
        if let Ok(v) = std::env::var("SUBCOG_STORAGE_BREAKER_HALF_OPEN_MAX_CALLS") {
            if let Ok(parsed) = v.parse::<u32>() {
                self.breaker_half_open_max_calls = parsed.max(1);
            }
        }
        self
    }

    /// Sets the failure threshold.
    #[must_use]
    pub const fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.breaker_failure_threshold = threshold;
        self
    }

    /// Sets the reset timeout in milliseconds.
    #[must_use]
    pub const fn with_reset_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.breaker_reset_timeout_ms = timeout_ms;
        self
    }

    /// Sets the half-open max calls.
    #[must_use]
    pub const fn with_half_open_max_calls(mut self, max_calls: u32) -> Self {
        self.breaker_half_open_max_calls = max_calls;
        self
    }
}

/// Circuit breaker state machine.
#[derive(Debug)]
enum BreakerState {
    Closed { failures: u32 },
    Open { opened_at: Instant },
    HalfOpen { attempts: u32 },
}

/// Circuit breaker for storage backends.
#[derive(Debug)]
pub struct CircuitBreaker {
    state: BreakerState,
    failure_threshold: u32,
    reset_timeout: Duration,
    half_open_max_calls: u32,
    backend_name: &'static str,
}

impl CircuitBreaker {
    /// Creates a new circuit breaker with the given configuration.
    #[must_use]
    pub fn new(config: &StorageResilienceConfig, backend_name: &'static str) -> Self {
        Self {
            state: BreakerState::Closed { failures: 0 },
            failure_threshold: config.breaker_failure_threshold.max(1),
            reset_timeout: Duration::from_millis(config.breaker_reset_timeout_ms),
            half_open_max_calls: config.breaker_half_open_max_calls.max(1),
            backend_name,
        }
    }

    /// Checks if a request is allowed through the circuit breaker.
    ///
    /// Returns `true` if the request should proceed, `false` if rejected.
    pub fn allow(&mut self) -> bool {
        match self.state {
            BreakerState::Closed { .. } => true,
            BreakerState::Open { opened_at } => {
                if opened_at.elapsed() >= self.reset_timeout {
                    tracing::info!(
                        backend = self.backend_name,
                        "Circuit breaker transitioning to half-open"
                    );
                    self.state = BreakerState::HalfOpen { attempts: 0 };
                    true
                } else {
                    false
                }
            },
            BreakerState::HalfOpen { ref mut attempts } => {
                if *attempts >= self.half_open_max_calls {
                    false
                } else {
                    *attempts += 1;
                    true
                }
            },
        }
    }

    /// Records a successful operation, potentially closing the circuit.
    pub fn on_success(&mut self) {
        if !matches!(self.state, BreakerState::Closed { failures: 0 }) {
            tracing::info!(
                backend = self.backend_name,
                "Circuit breaker closing after success"
            );
        }
        self.state = BreakerState::Closed { failures: 0 };
    }

    /// Records a failed operation, potentially opening the circuit.
    ///
    /// Returns `true` if the circuit just opened (tripped).
    pub fn on_failure(&mut self) -> bool {
        match self.state {
            BreakerState::Closed { ref mut failures } => {
                *failures += 1;
                if *failures >= self.failure_threshold {
                    tracing::warn!(
                        backend = self.backend_name,
                        failures = *failures,
                        threshold = self.failure_threshold,
                        "Circuit breaker opened after consecutive failures"
                    );
                    self.state = BreakerState::Open {
                        opened_at: Instant::now(),
                    };
                    return true;
                }
            },
            BreakerState::HalfOpen { .. } => {
                tracing::warn!(
                    backend = self.backend_name,
                    "Circuit breaker re-opened after half-open failure"
                );
                self.state = BreakerState::Open {
                    opened_at: Instant::now(),
                };
                return true;
            },
            BreakerState::Open { .. } => {},
        }
        false
    }

    /// Returns the current state as a numeric value for metrics.
    ///
    /// - 0: Closed
    /// - 1: Open
    /// - 2: Half-Open
    #[must_use]
    pub const fn state_value(&self) -> u8 {
        match self.state {
            BreakerState::Closed { .. } => 0,
            BreakerState::Open { .. } => 1,
            BreakerState::HalfOpen { .. } => 2,
        }
    }

    /// Returns the backend name.
    #[must_use]
    pub const fn backend_name(&self) -> &'static str {
        self.backend_name
    }
}

// ============================================================================
// Resilient Persistence Backend
// ============================================================================

use super::traits::PersistenceBackend;
use crate::models::{Memory, MemoryId};

/// Persistence backend wrapper with circuit breaker protection.
pub struct ResilientPersistenceBackend<P: PersistenceBackend> {
    inner: P,
    breaker: Mutex<CircuitBreaker>,
    backend_name: &'static str,
}

impl<P: PersistenceBackend> ResilientPersistenceBackend<P> {
    /// Creates a new resilient persistence backend wrapper.
    #[must_use]
    pub fn new(inner: P, config: StorageResilienceConfig, backend_name: &'static str) -> Self {
        Self {
            inner,
            breaker: Mutex::new(CircuitBreaker::new(&config, backend_name)),
            backend_name,
        }
    }

    fn execute<T, F>(&self, operation: &'static str, call: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if !breaker.allow() {
            let state = breaker.state_value();
            drop(breaker);
            Self::record_metrics(self.backend_name, operation, "circuit_open", state);
            return Err(Error::OperationFailed {
                operation: format!("storage_{operation}"),
                cause: format!("circuit breaker open for backend '{}'", self.backend_name),
            });
        }
        drop(breaker);

        let result = call();

        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        match &result {
            Ok(_) => {
                breaker.on_success();
                let state = breaker.state_value();
                drop(breaker);
                Self::record_metrics(self.backend_name, operation, "success", state);
            },
            Err(_) => {
                let tripped = breaker.on_failure();
                let state = breaker.state_value();
                drop(breaker);
                Self::record_metrics(self.backend_name, operation, "error", state);
                if tripped {
                    metrics::counter!(
                        "storage_circuit_breaker_trips_total",
                        "backend" => self.backend_name,
                        "operation" => operation
                    )
                    .increment(1);
                }
            },
        }

        result
    }

    fn record_metrics(
        backend: &'static str,
        operation: &'static str,
        status: &'static str,
        state: u8,
    ) {
        metrics::counter!(
            "storage_requests_total",
            "backend" => backend,
            "operation" => operation,
            "status" => status
        )
        .increment(1);
        metrics::gauge!(
            "storage_circuit_breaker_state",
            "backend" => backend
        )
        .set(f64::from(state));
    }
}

impl<P: PersistenceBackend> PersistenceBackend for ResilientPersistenceBackend<P> {
    fn store(&self, memory: &Memory) -> Result<()> {
        self.execute("store", || self.inner.store(memory))
    }

    fn get(&self, id: &MemoryId) -> Result<Option<Memory>> {
        self.execute("get", || self.inner.get(id))
    }

    fn delete(&self, id: &MemoryId) -> Result<bool> {
        self.execute("delete", || self.inner.delete(id))
    }

    fn list_ids(&self) -> Result<Vec<MemoryId>> {
        self.execute("list_ids", || self.inner.list_ids())
    }

    fn exists(&self, id: &MemoryId) -> Result<bool> {
        self.execute("exists", || self.inner.exists(id))
    }

    fn count(&self) -> Result<usize> {
        self.execute("count", || self.inner.count())
    }
}

// ============================================================================
// Resilient Index Backend
// ============================================================================

use super::traits::IndexBackend;
use crate::models::SearchFilter;

/// Index backend wrapper with circuit breaker protection.
pub struct ResilientIndexBackend<I: IndexBackend> {
    inner: I,
    breaker: Mutex<CircuitBreaker>,
    backend_name: &'static str,
}

impl<I: IndexBackend> ResilientIndexBackend<I> {
    /// Creates a new resilient index backend wrapper.
    #[must_use]
    pub fn new(inner: I, config: StorageResilienceConfig, backend_name: &'static str) -> Self {
        Self {
            inner,
            breaker: Mutex::new(CircuitBreaker::new(&config, backend_name)),
            backend_name,
        }
    }

    fn execute<T, F>(&self, operation: &'static str, call: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if !breaker.allow() {
            let state = breaker.state_value();
            drop(breaker);
            Self::record_metrics(self.backend_name, operation, "circuit_open", state);
            return Err(Error::OperationFailed {
                operation: format!("index_{operation}"),
                cause: format!("circuit breaker open for backend '{}'", self.backend_name),
            });
        }
        drop(breaker);

        let result = call();

        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        match &result {
            Ok(_) => {
                breaker.on_success();
                let state = breaker.state_value();
                drop(breaker);
                Self::record_metrics(self.backend_name, operation, "success", state);
            },
            Err(_) => {
                let tripped = breaker.on_failure();
                let state = breaker.state_value();
                drop(breaker);
                Self::record_metrics(self.backend_name, operation, "error", state);
                if tripped {
                    metrics::counter!(
                        "storage_circuit_breaker_trips_total",
                        "backend" => self.backend_name,
                        "operation" => operation
                    )
                    .increment(1);
                }
            },
        }

        result
    }

    fn record_metrics(
        backend: &'static str,
        operation: &'static str,
        status: &'static str,
        state: u8,
    ) {
        metrics::counter!(
            "storage_requests_total",
            "backend" => backend,
            "operation" => operation,
            "status" => status
        )
        .increment(1);
        metrics::gauge!(
            "storage_circuit_breaker_state",
            "backend" => backend
        )
        .set(f64::from(state));
    }
}

impl<I: IndexBackend> IndexBackend for ResilientIndexBackend<I> {
    fn index(&self, memory: &Memory) -> Result<()> {
        self.execute("index", || self.inner.index(memory))
    }

    fn remove(&self, id: &MemoryId) -> Result<bool> {
        self.execute("remove", || self.inner.remove(id))
    }

    fn search(
        &self,
        query: &str,
        filter: &SearchFilter,
        limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>> {
        self.execute("search", || self.inner.search(query, filter, limit))
    }

    fn reindex(&self, memories: &[Memory]) -> Result<()> {
        self.execute("reindex", || self.inner.reindex(memories))
    }

    fn clear(&self) -> Result<()> {
        self.execute("clear", || self.inner.clear())
    }

    fn list_all(&self, filter: &SearchFilter, limit: usize) -> Result<Vec<(MemoryId, f32)>> {
        self.execute("list_all", || self.inner.list_all(filter, limit))
    }

    fn get_memory(&self, id: &MemoryId) -> Result<Option<Memory>> {
        self.execute("get_memory", || self.inner.get_memory(id))
    }

    fn get_memories_batch(&self, ids: &[MemoryId]) -> Result<Vec<Option<Memory>>> {
        self.execute("get_memories_batch", || self.inner.get_memories_batch(ids))
    }
}

// ============================================================================
// Resilient Vector Backend
// ============================================================================

use super::traits::{VectorBackend, VectorFilter};

/// Vector backend wrapper with circuit breaker protection.
pub struct ResilientVectorBackend<V: VectorBackend> {
    inner: V,
    breaker: Mutex<CircuitBreaker>,
    backend_name: &'static str,
}

impl<V: VectorBackend> ResilientVectorBackend<V> {
    /// Creates a new resilient vector backend wrapper.
    #[must_use]
    pub fn new(inner: V, config: StorageResilienceConfig, backend_name: &'static str) -> Self {
        Self {
            inner,
            breaker: Mutex::new(CircuitBreaker::new(&config, backend_name)),
            backend_name,
        }
    }

    fn execute<T, F>(&self, operation: &'static str, call: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if !breaker.allow() {
            let state = breaker.state_value();
            drop(breaker);
            Self::record_metrics(self.backend_name, operation, "circuit_open", state);
            return Err(Error::OperationFailed {
                operation: format!("vector_{operation}"),
                cause: format!("circuit breaker open for backend '{}'", self.backend_name),
            });
        }
        drop(breaker);

        let result = call();

        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        match &result {
            Ok(_) => {
                breaker.on_success();
                let state = breaker.state_value();
                drop(breaker);
                Self::record_metrics(self.backend_name, operation, "success", state);
            },
            Err(_) => {
                let tripped = breaker.on_failure();
                let state = breaker.state_value();
                drop(breaker);
                Self::record_metrics(self.backend_name, operation, "error", state);
                if tripped {
                    metrics::counter!(
                        "storage_circuit_breaker_trips_total",
                        "backend" => self.backend_name,
                        "operation" => operation
                    )
                    .increment(1);
                }
            },
        }

        result
    }

    fn record_metrics(
        backend: &'static str,
        operation: &'static str,
        status: &'static str,
        state: u8,
    ) {
        metrics::counter!(
            "storage_requests_total",
            "backend" => backend,
            "operation" => operation,
            "status" => status
        )
        .increment(1);
        metrics::gauge!(
            "storage_circuit_breaker_state",
            "backend" => backend
        )
        .set(f64::from(state));
    }
}

impl<V: VectorBackend> VectorBackend for ResilientVectorBackend<V> {
    fn dimensions(&self) -> usize {
        // dimensions() is a pure getter, no circuit breaker needed
        self.inner.dimensions()
    }

    fn upsert(&self, id: &MemoryId, embedding: &[f32]) -> Result<()> {
        self.execute("upsert", || self.inner.upsert(id, embedding))
    }

    fn remove(&self, id: &MemoryId) -> Result<bool> {
        self.execute("remove", || self.inner.remove(id))
    }

    fn search(
        &self,
        query_embedding: &[f32],
        filter: &VectorFilter,
        limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>> {
        self.execute("search", || {
            self.inner.search(query_embedding, filter, limit)
        })
    }

    fn count(&self) -> Result<usize> {
        self.execute("count", || self.inner.count())
    }

    fn clear(&self) -> Result<()> {
        self.execute("clear", || self.inner.clear())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Circuit Breaker Tests
    // =========================================================================

    #[test]
    fn test_circuit_breaker_starts_closed() {
        let config = StorageResilienceConfig::default();
        let breaker = CircuitBreaker::new(&config, "test");
        assert_eq!(breaker.state_value(), 0); // Closed = 0
    }

    #[test]
    fn test_circuit_breaker_allows_calls_when_closed() {
        let config = StorageResilienceConfig::default();
        let mut breaker = CircuitBreaker::new(&config, "test");
        assert!(breaker.allow());
        assert!(breaker.allow());
        assert!(breaker.allow());
    }

    #[test]
    fn test_circuit_breaker_opens_after_threshold_failures() {
        let config = StorageResilienceConfig {
            breaker_failure_threshold: 3,
            ..Default::default()
        };
        let mut breaker = CircuitBreaker::new(&config, "test");

        // First two failures don't trip the breaker
        breaker.on_failure();
        assert_eq!(breaker.state_value(), 0); // Still closed
        breaker.on_failure();
        assert_eq!(breaker.state_value(), 0); // Still closed

        // Third failure trips the breaker
        let tripped = breaker.on_failure();
        assert!(tripped);
        assert_eq!(breaker.state_value(), 1); // Open = 1
    }

    #[test]
    fn test_circuit_breaker_rejects_when_open() {
        let config = StorageResilienceConfig {
            breaker_failure_threshold: 1,
            breaker_reset_timeout_ms: 10_000, // Long timeout
            ..Default::default()
        };
        let mut breaker = CircuitBreaker::new(&config, "test");

        // Trip the breaker
        breaker.on_failure();
        assert_eq!(breaker.state_value(), 1); // Open

        // Should reject calls
        assert!(!breaker.allow());
        assert!(!breaker.allow());
    }

    #[test]
    fn test_circuit_breaker_transitions_to_half_open_after_timeout() {
        let config = StorageResilienceConfig {
            breaker_failure_threshold: 1,
            breaker_reset_timeout_ms: 0, // Immediate reset
            breaker_half_open_max_calls: 1,
        };
        let mut breaker = CircuitBreaker::new(&config, "test");

        // Trip the breaker
        breaker.on_failure();
        assert_eq!(breaker.state_value(), 1); // Open

        // Should allow call after timeout (immediate since reset_timeout_ms=0)
        std::thread::sleep(Duration::from_millis(1));
        assert!(breaker.allow());
        assert_eq!(breaker.state_value(), 2); // Half-open = 2
    }

    #[test]
    fn test_circuit_breaker_half_open_limits_calls() {
        let config = StorageResilienceConfig {
            breaker_failure_threshold: 1,
            breaker_reset_timeout_ms: 0,
            breaker_half_open_max_calls: 2,
        };
        let mut breaker = CircuitBreaker::new(&config, "test");

        // Trip and transition to half-open
        breaker.on_failure();
        std::thread::sleep(Duration::from_millis(1));

        // First call transitions to half-open and is allowed
        assert!(breaker.allow());
        assert_eq!(breaker.state_value(), 2); // Half-open

        // Two more calls allowed (attempts = 1, 2)
        assert!(breaker.allow()); // attempts = 1
        assert!(breaker.allow()); // attempts = 2

        // Next call rejected (attempts >= max_calls)
        assert!(!breaker.allow());
    }

    #[test]
    fn test_circuit_breaker_closes_on_success() {
        let config = StorageResilienceConfig {
            breaker_failure_threshold: 1,
            breaker_reset_timeout_ms: 0,
            ..Default::default()
        };
        let mut breaker = CircuitBreaker::new(&config, "test");

        // Trip and transition to half-open
        breaker.on_failure();
        std::thread::sleep(Duration::from_millis(1));
        breaker.allow();

        // Success closes the breaker
        breaker.on_success();
        assert_eq!(breaker.state_value(), 0); // Closed
    }

    #[test]
    fn test_circuit_breaker_reopens_on_failure_when_half_open() {
        let config = StorageResilienceConfig {
            breaker_failure_threshold: 1,
            breaker_reset_timeout_ms: 0,
            ..Default::default()
        };
        let mut breaker = CircuitBreaker::new(&config, "test");

        // Trip and transition to half-open
        breaker.on_failure();
        std::thread::sleep(Duration::from_millis(1));
        breaker.allow();
        assert_eq!(breaker.state_value(), 2); // Half-open

        // Failure reopens the breaker
        let tripped = breaker.on_failure();
        assert!(tripped);
        assert_eq!(breaker.state_value(), 1); // Open
    }

    // =========================================================================
    // Configuration Tests
    // =========================================================================

    #[test]
    fn test_config_default_values() {
        let config = StorageResilienceConfig::default();
        assert_eq!(config.breaker_failure_threshold, 5);
        assert_eq!(config.breaker_reset_timeout_ms, 30_000);
        assert_eq!(config.breaker_half_open_max_calls, 1);
    }

    #[test]
    fn test_config_builder_pattern() {
        let config = StorageResilienceConfig::default()
            .with_failure_threshold(10)
            .with_reset_timeout_ms(60_000)
            .with_half_open_max_calls(3);

        assert_eq!(config.breaker_failure_threshold, 10);
        assert_eq!(config.breaker_reset_timeout_ms, 60_000);
        assert_eq!(config.breaker_half_open_max_calls, 3);
    }

    #[test]
    fn test_config_minimum_values() {
        // Threshold of 0 should become 1 in CircuitBreaker::new
        let config = StorageResilienceConfig {
            breaker_failure_threshold: 0,
            breaker_reset_timeout_ms: 0,
            breaker_half_open_max_calls: 0,
        };
        let breaker = CircuitBreaker::new(&config, "test");

        // Internal values should be clamped to minimum 1
        assert_eq!(breaker.failure_threshold, 1);
        assert_eq!(breaker.half_open_max_calls, 1);
    }
}
