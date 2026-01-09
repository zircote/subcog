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
    /// Maximum number of retries for retryable failures (CHAOS-HIGH-003).
    pub max_retries: u32,
    /// Base backoff between retries in milliseconds (exponential with jitter).
    pub retry_backoff_ms: u64,
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
            max_retries: 3,
            retry_backoff_ms: 100,
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
        // Retry configuration (CHAOS-HIGH-003)
        if let Ok(v) = std::env::var("SUBCOG_STORAGE_MAX_RETRIES")
            && let Ok(parsed) = v.parse::<u32>()
        {
            self.max_retries = parsed;
        }
        if let Ok(v) = std::env::var("SUBCOG_STORAGE_RETRY_BACKOFF_MS")
            && let Ok(parsed) = v.parse::<u64>()
        {
            self.retry_backoff_ms = parsed;
        }
        // Circuit breaker configuration
        if let Ok(v) = std::env::var("SUBCOG_STORAGE_BREAKER_FAILURE_THRESHOLD")
            && let Ok(parsed) = v.parse::<u32>()
        {
            self.breaker_failure_threshold = parsed.max(1);
        }
        if let Ok(v) = std::env::var("SUBCOG_STORAGE_BREAKER_RESET_MS")
            && let Ok(parsed) = v.parse::<u64>()
        {
            self.breaker_reset_timeout_ms = parsed;
        }
        if let Ok(v) = std::env::var("SUBCOG_STORAGE_BREAKER_HALF_OPEN_MAX_CALLS")
            && let Ok(parsed) = v.parse::<u32>()
        {
            self.breaker_half_open_max_calls = parsed.max(1);
        }
        self
    }

    /// Sets the maximum number of retries.
    #[must_use]
    pub const fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Sets the base retry backoff in milliseconds.
    #[must_use]
    pub const fn with_retry_backoff_ms(mut self, backoff_ms: u64) -> Self {
        self.retry_backoff_ms = backoff_ms;
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
// Retry Helper Functions (CHAOS-HIGH-003)
// ============================================================================

/// Calculates retry delay with exponential backoff and jitter.
///
/// Formula: `base_delay * 2^(attempt-1) + jitter`
/// - Exponential growth prevents overwhelming a recovering service
/// - Jitter (0-50% of delay) prevents thundering herd problem
/// - Maximum delay capped at 10 seconds
fn calculate_retry_delay(base_delay_ms: u64, attempt: u32) -> u64 {
    // Exponential: base * 2^(attempt-1), so attempt 1 = base, attempt 2 = 2x, attempt 3 = 4x
    let exponent = attempt.saturating_sub(1);
    let exponential_delay = base_delay_ms.saturating_mul(1u64 << exponent.min(10));

    // Cap at 10 seconds max delay
    let capped_delay = exponential_delay.min(10_000);

    // Add jitter: use system time nanoseconds as pseudo-random source
    // Jitter range: 0-50% of capped delay to prevent thundering herd
    let jitter = calculate_jitter(capped_delay);
    let total_delay = capped_delay.saturating_add(jitter);

    tracing::debug!(
        "Storage retry backoff: attempt={}, base={}ms, exponential={}ms, jitter={}ms, total={}ms",
        attempt,
        base_delay_ms,
        capped_delay,
        jitter,
        total_delay
    );

    total_delay
}

/// Calculates jitter for retry backoff using system time as pseudo-random source.
fn calculate_jitter(delay_ms: u64) -> u64 {
    let jitter_max = delay_ms / 2;
    if jitter_max == 0 {
        return 0;
    }

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);

    u64::from(nanos) % jitter_max
}

/// Checks if a storage error is retryable (transient failures that may succeed on retry).
///
/// Retryable errors include:
/// - Timeouts
/// - Connection errors (network issues, DNS failures)
/// - Lock/busy errors (database locked, pool exhausted)
/// - Temporary I/O errors
pub fn is_retryable_storage_error(err: &Error) -> bool {
    match err {
        Error::OperationFailed { cause, .. } => {
            let lower = cause.to_lowercase();
            // Timeout errors
            lower.contains("timeout")
                || lower.contains("timed out")
                || lower.contains("deadline")
                || lower.contains("elapsed")
                // Connection errors
                || lower.contains("connect")
                || lower.contains("connection")
                || lower.contains("network")
                || lower.contains("dns")
                || lower.contains("resolve")
                // Database lock/busy errors
                || lower.contains("locked")
                || lower.contains("busy")
                || lower.contains("pool")
                || lower.contains("exhausted")
                // Temporary I/O errors
                || lower.contains("temporary")
                || lower.contains("try again")
                || lower.contains("interrupted")
        },
        _ => false,
    }
}

// ============================================================================
// Connection Retry Helper (CHAOS-HIGH-003)
// ============================================================================

/// Executes a connection operation with retry and exponential backoff.
///
/// This function is designed for initial connection establishment where transient
/// failures (network issues, database starting up, etc.) should be retried.
///
/// # Arguments
///
/// * `config` - Resilience configuration with retry settings
/// * `backend_name` - Name of the backend for logging
/// * `operation` - Description of the operation for error messages
/// * `connect_fn` - The connection function to retry
///
/// # Example
///
/// ```rust,ignore
/// use subcog::storage::resilience::{retry_connection, StorageResilienceConfig};
///
/// let pool = retry_connection(
///     &StorageResilienceConfig::default(),
///     "postgres",
///     "create_pool",
///     || create_postgres_pool(connection_url),
/// )?;
/// ```
pub fn retry_connection<T, F>(
    config: &StorageResilienceConfig,
    backend_name: &str,
    operation: &str,
    mut connect_fn: F,
) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    let max_attempts = config.max_retries + 1;
    let mut attempts = 0;
    let mut last_error = None;

    while attempts < max_attempts {
        attempts += 1;

        match connect_fn() {
            Ok(result) => {
                if attempts > 1 {
                    tracing::info!(
                        backend = backend_name,
                        operation = operation,
                        attempts = attempts,
                        "Connection succeeded after retries"
                    );
                }
                return Ok(result);
            },
            Err(err) => {
                let retryable = is_retryable_connection_error(&err) && attempts < max_attempts;

                if !retryable {
                    tracing::warn!(
                        backend = backend_name,
                        operation = operation,
                        attempts = attempts,
                        error = %err,
                        "Connection failed with non-retryable error"
                    );
                    return Err(err);
                }

                let delay_ms = calculate_retry_delay(config.retry_backoff_ms, attempts);
                tracing::debug!(
                    backend = backend_name,
                    operation = operation,
                    attempt = attempts,
                    max_attempts = max_attempts,
                    delay_ms = delay_ms,
                    error = %err,
                    "Connection failed, retrying with backoff"
                );

                metrics::counter!(
                    "storage_connection_retries_total",
                    "backend" => backend_name.to_string(),
                    "operation" => operation.to_string()
                )
                .increment(1);

                std::thread::sleep(Duration::from_millis(delay_ms));
                last_error = Some(err);
            },
        }
    }

    let err = last_error.unwrap_or_else(|| Error::OperationFailed {
        operation: format!("{backend_name}_{operation}"),
        cause: "exhausted connection retries".to_string(),
    });

    tracing::error!(
        backend = backend_name,
        operation = operation,
        max_attempts = max_attempts,
        error = %err,
        "Connection failed after exhausting all retries"
    );

    Err(err)
}

/// Checks if a connection error is retryable.
///
/// Connection-specific retryable errors include:
/// - Connection refused (server starting up)
/// - Network unreachable
/// - DNS resolution failures
/// - Timeouts
/// - Pool creation failures
fn is_retryable_connection_error(err: &Error) -> bool {
    match err {
        Error::OperationFailed { cause, .. } => {
            let lower = cause.to_lowercase();
            // Connection establishment errors
            lower.contains("connection refused")
                || lower.contains("connection reset")
                || lower.contains("connection timed out")
                || lower.contains("network")
                || lower.contains("unreachable")
                || lower.contains("dns")
                || lower.contains("resolve")
                || lower.contains("timeout")
                || lower.contains("timed out")
                || lower.contains("pool")
                || lower.contains("exhausted")
                // Database not ready errors
                || lower.contains("not ready")
                || lower.contains("starting")
                || lower.contains("unavailable")
                || lower.contains("service")
                // Generic transient errors
                || lower.contains("temporary")
                || lower.contains("try again")
                || lower.contains("econnrefused")
                || lower.contains("etimedout")
        },
        _ => false,
    }
}

// ============================================================================
// Resilient Persistence Backend
// ============================================================================

use super::traits::PersistenceBackend;
use crate::models::{Memory, MemoryId};

/// Persistence backend wrapper with circuit breaker and retry protection.
pub struct ResilientPersistenceBackend<P: PersistenceBackend> {
    inner: P,
    config: StorageResilienceConfig,
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
            config,
            backend_name,
        }
    }

    fn execute<T, F>(&self, operation: &'static str, mut call: F) -> Result<T>
    where
        F: FnMut() -> Result<T>,
    {
        let max_attempts = self.config.max_retries + 1;
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < max_attempts {
            attempts += 1;

            if let Some(err) = self.check_circuit_breaker(operation) {
                return Err(err);
            }

            match call() {
                Ok(value) => return Ok(self.handle_success(operation, value)),
                Err(err) => {
                    last_error = self.handle_error(operation, err, attempts, max_attempts)?;
                },
            }
        }

        Err(last_error.unwrap_or_else(|| Error::OperationFailed {
            operation: format!("storage_{operation}"),
            cause: "exhausted retries".to_string(),
        }))
    }

    /// Checks circuit breaker and returns error if open.
    fn check_circuit_breaker(&self, operation: &'static str) -> Option<Error> {
        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if !breaker.allow() {
            let state = breaker.state_value();
            drop(breaker);
            Self::record_metrics(self.backend_name, operation, "circuit_open", state);
            return Some(Error::OperationFailed {
                operation: format!("storage_{operation}"),
                cause: format!("circuit breaker open for backend '{}'", self.backend_name),
            });
        }
        None
    }

    /// Handles successful operation result.
    fn handle_success<T>(&self, operation: &'static str, value: T) -> T {
        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        breaker.on_success();
        let state = breaker.state_value();
        drop(breaker);
        Self::record_metrics(self.backend_name, operation, "success", state);
        value
    }

    /// Handles operation error. Returns Ok(Some(err)) to continue retrying, Err to stop.
    fn handle_error(
        &self,
        operation: &'static str,
        err: Error,
        attempts: u32,
        max_attempts: u32,
    ) -> Result<Option<Error>> {
        let retryable = is_retryable_storage_error(&err) && attempts < max_attempts;

        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let tripped = breaker.on_failure();
        let state = breaker.state_value();
        drop(breaker);

        Self::record_metrics(self.backend_name, operation, "error", state);

        if tripped {
            Self::record_circuit_trip(self.backend_name, operation);
        }

        if !retryable {
            return Err(err);
        }

        Self::log_retry_attempt(self.backend_name, operation, attempts, max_attempts, &err);
        self.apply_retry_backoff(attempts);
        Ok(Some(err))
    }

    /// Applies retry backoff delay if configured.
    fn apply_retry_backoff(&self, attempts: u32) {
        if self.config.retry_backoff_ms > 0 {
            let delay = calculate_retry_delay(self.config.retry_backoff_ms, attempts);
            std::thread::sleep(Duration::from_millis(delay));
        }
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

    /// Records metrics and logs when circuit breaker trips.
    fn record_circuit_trip(backend: &'static str, operation: &'static str) {
        metrics::counter!(
            "storage_circuit_breaker_trips_total",
            "backend" => backend,
            "operation" => operation
        )
        .increment(1);
        tracing::warn!(
            backend = backend,
            operation = operation,
            "Storage circuit breaker opened"
        );
    }

    /// Records metrics and logs for retry attempts.
    fn log_retry_attempt(
        backend: &'static str,
        operation: &'static str,
        attempt: u32,
        max_attempts: u32,
        err: &Error,
    ) {
        metrics::counter!(
            "storage_retries_total",
            "backend" => backend,
            "operation" => operation
        )
        .increment(1);
        tracing::debug!(
            backend = backend,
            operation = operation,
            attempt = attempt,
            max_attempts = max_attempts,
            error = %err,
            "Retrying storage operation"
        );
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

/// Index backend wrapper with circuit breaker and retry protection.
pub struct ResilientIndexBackend<I: IndexBackend> {
    inner: I,
    config: StorageResilienceConfig,
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
            config,
            backend_name,
        }
    }

    fn execute<T, F>(&self, operation: &'static str, mut call: F) -> Result<T>
    where
        F: FnMut() -> Result<T>,
    {
        let max_attempts = self.config.max_retries + 1;
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < max_attempts {
            attempts += 1;

            if let Some(err) = self.check_circuit_breaker(operation) {
                return Err(err);
            }

            match call() {
                Ok(value) => return Ok(self.handle_success(operation, value)),
                Err(err) => {
                    last_error = self.handle_error(operation, err, attempts, max_attempts)?;
                },
            }
        }

        Err(last_error.unwrap_or_else(|| Error::OperationFailed {
            operation: format!("index_{operation}"),
            cause: "exhausted retries".to_string(),
        }))
    }

    /// Checks circuit breaker and returns error if open.
    fn check_circuit_breaker(&self, operation: &'static str) -> Option<Error> {
        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if !breaker.allow() {
            let state = breaker.state_value();
            drop(breaker);
            Self::record_metrics(self.backend_name, operation, "circuit_open", state);
            return Some(Error::OperationFailed {
                operation: format!("index_{operation}"),
                cause: format!("circuit breaker open for backend '{}'", self.backend_name),
            });
        }
        None
    }

    /// Handles successful operation result.
    fn handle_success<T>(&self, operation: &'static str, value: T) -> T {
        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        breaker.on_success();
        let state = breaker.state_value();
        drop(breaker);
        Self::record_metrics(self.backend_name, operation, "success", state);
        value
    }

    /// Handles operation error. Returns Ok(Some(err)) to continue retrying, Err to stop.
    fn handle_error(
        &self,
        operation: &'static str,
        err: Error,
        attempts: u32,
        max_attempts: u32,
    ) -> Result<Option<Error>> {
        let retryable = is_retryable_storage_error(&err) && attempts < max_attempts;

        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let tripped = breaker.on_failure();
        let state = breaker.state_value();
        drop(breaker);

        Self::record_metrics(self.backend_name, operation, "error", state);

        if tripped {
            Self::record_circuit_trip(self.backend_name, operation);
        }

        if !retryable {
            return Err(err);
        }

        Self::log_retry(self.backend_name, operation);
        self.apply_retry_backoff(attempts);
        Ok(Some(err))
    }

    /// Applies retry backoff delay if configured.
    fn apply_retry_backoff(&self, attempts: u32) {
        if self.config.retry_backoff_ms > 0 {
            let delay = calculate_retry_delay(self.config.retry_backoff_ms, attempts);
            std::thread::sleep(Duration::from_millis(delay));
        }
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

    /// Records metrics and logs when circuit breaker trips.
    fn record_circuit_trip(backend: &'static str, operation: &'static str) {
        metrics::counter!(
            "storage_circuit_breaker_trips_total",
            "backend" => backend,
            "operation" => operation
        )
        .increment(1);
        tracing::warn!(
            backend = backend,
            operation = operation,
            "Index circuit breaker opened"
        );
    }

    /// Records metrics for retry attempts.
    fn log_retry(backend: &'static str, operation: &'static str) {
        metrics::counter!(
            "storage_retries_total",
            "backend" => backend,
            "operation" => operation
        )
        .increment(1);
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

/// Vector backend wrapper with circuit breaker and retry protection.
pub struct ResilientVectorBackend<V: VectorBackend> {
    inner: V,
    config: StorageResilienceConfig,
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
            config,
            backend_name,
        }
    }

    /// Checks circuit breaker and returns error if open.
    fn check_circuit_breaker(&self, operation: &'static str) -> Option<Error> {
        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if !breaker.allow() {
            let state = breaker.state_value();
            drop(breaker);
            Self::record_metrics(self.backend_name, operation, "circuit_open", state);
            return Some(Error::OperationFailed {
                operation: format!("vector_{operation}"),
                cause: format!("circuit breaker open for backend '{}'", self.backend_name),
            });
        }
        drop(breaker);
        None
    }

    /// Handles successful operation result.
    fn handle_success<T>(&self, operation: &'static str, value: T) -> T {
        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        breaker.on_success();
        let state = breaker.state_value();
        drop(breaker);
        Self::record_metrics(self.backend_name, operation, "success", state);
        value
    }

    /// Handles operation error. Returns Ok(Some(err)) to continue retrying, Err to stop.
    fn handle_error(
        &self,
        operation: &'static str,
        err: Error,
        attempts: u32,
        max_attempts: u32,
    ) -> Result<Option<Error>> {
        let retryable = is_retryable_storage_error(&err) && attempts < max_attempts;

        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let tripped = breaker.on_failure();
        let state = breaker.state_value();
        drop(breaker);

        Self::record_metrics(self.backend_name, operation, "error", state);

        if tripped {
            Self::record_circuit_trip(self.backend_name, operation);
        }

        if !retryable {
            return Err(err);
        }

        self.apply_retry_backoff(operation, attempts, &err);
        Ok(Some(err))
    }

    /// Applies retry backoff delay if configured.
    fn apply_retry_backoff(&self, operation: &'static str, attempts: u32, err: &Error) {
        let delay_ms = calculate_retry_delay(self.config.retry_backoff_ms, attempts);
        Self::log_retry_attempt(self.backend_name, operation, attempts, delay_ms, err);
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    }

    fn execute<T, F>(&self, operation: &'static str, mut call: F) -> Result<T>
    where
        F: FnMut() -> Result<T>,
    {
        let max_attempts = self.config.max_retries + 1;
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < max_attempts {
            attempts += 1;

            if let Some(err) = self.check_circuit_breaker(operation) {
                return Err(err);
            }

            match call() {
                Ok(value) => return Ok(self.handle_success(operation, value)),
                Err(err) => {
                    last_error = self.handle_error(operation, err, attempts, max_attempts)?;
                },
            }
        }

        Err(last_error.unwrap_or_else(|| Error::OperationFailed {
            operation: format!("vector_{operation}"),
            cause: "exhausted retries".to_string(),
        }))
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

    /// Records metrics and logs when circuit breaker trips.
    fn record_circuit_trip(backend: &'static str, operation: &'static str) {
        metrics::counter!(
            "storage_circuit_breaker_trips_total",
            "backend" => backend,
            "operation" => operation
        )
        .increment(1);
        tracing::warn!(
            backend = backend,
            operation = operation,
            "Vector circuit breaker opened"
        );
    }

    /// Records metrics and logs for retry attempts.
    fn log_retry_attempt(
        backend: &'static str,
        operation: &'static str,
        attempt: u32,
        delay_ms: u64,
        err: &Error,
    ) {
        metrics::counter!(
            "storage_retries_total",
            "backend" => backend,
            "operation" => operation
        )
        .increment(1);
        tracing::debug!(
            backend = backend,
            operation = operation,
            attempt = attempt,
            delay_ms = delay_ms,
            error = %err,
            "Retrying vector operation"
        );
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
            max_retries: 3,
            retry_backoff_ms: 100,
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
            max_retries: 3,
            retry_backoff_ms: 100,
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
            max_retries: 0,
            retry_backoff_ms: 0,
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
