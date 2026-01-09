//! LLM resilience wrapper with circuit breaking and budget instrumentation.

use super::{CaptureAnalysis, LlmProvider};
use crate::{Error, Result};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Resilience configuration for LLM calls.
#[derive(Debug, Clone)]
pub struct LlmResilienceConfig {
    /// Maximum number of retries for retryable failures.
    pub max_retries: u32,
    /// Backoff between retries in milliseconds.
    pub retry_backoff_ms: u64,
    /// Consecutive failures before opening the circuit.
    pub breaker_failure_threshold: u32,
    /// How long to keep the circuit open before half-open.
    pub breaker_reset_timeout_ms: u64,
    /// Maximum trial calls while half-open.
    pub breaker_half_open_max_calls: u32,
    /// Latency budget in milliseconds for LLM calls.
    pub latency_slo_ms: u64,
    /// Error budget ratio threshold.
    pub error_budget_ratio: f64,
    /// Error budget window in seconds.
    pub error_budget_window_secs: u64,
}

impl Default for LlmResilienceConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_backoff_ms: 100,
            breaker_failure_threshold: 3,
            breaker_reset_timeout_ms: 30_000,
            breaker_half_open_max_calls: 1,
            latency_slo_ms: 2_000,
            error_budget_ratio: 0.05,
            error_budget_window_secs: 300,
        }
    }
}

impl LlmResilienceConfig {
    /// Loads resilience configuration from environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        Self::default().with_env_overrides()
    }

    /// Loads resilience configuration from config file settings.
    #[must_use]
    pub fn from_config(config: &crate::config::LlmConfig) -> Self {
        let mut settings = Self::default();
        if let Some(max_retries) = config.max_retries {
            settings.max_retries = max_retries;
        }
        if let Some(retry_backoff_ms) = config.retry_backoff_ms {
            settings.retry_backoff_ms = retry_backoff_ms;
        }
        if let Some(threshold) = config.breaker_failure_threshold {
            settings.breaker_failure_threshold = threshold.max(1);
        }
        if let Some(reset_ms) = config.breaker_reset_ms {
            settings.breaker_reset_timeout_ms = reset_ms;
        }
        if let Some(half_open) = config.breaker_half_open_max_calls {
            settings.breaker_half_open_max_calls = half_open.max(1);
        }
        if let Some(latency_slo_ms) = config.latency_slo_ms {
            settings.latency_slo_ms = latency_slo_ms;
        }
        if let Some(ratio) = config.error_budget_ratio {
            settings.error_budget_ratio = ratio.clamp(0.0, 1.0);
        }
        if let Some(window_secs) = config.error_budget_window_secs {
            settings.error_budget_window_secs = window_secs.max(1);
        }
        settings
    }

    /// Applies environment variable overrides.
    #[must_use]
    pub fn with_env_overrides(mut self) -> Self {
        if let Some(parsed) = std::env::var("SUBCOG_LLM_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
        {
            self.max_retries = parsed;
        }
        if let Some(parsed) = std::env::var("SUBCOG_LLM_RETRY_BACKOFF_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            self.retry_backoff_ms = parsed;
        }
        if let Some(parsed) = std::env::var("SUBCOG_LLM_BREAKER_FAILURE_THRESHOLD")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
        {
            self.breaker_failure_threshold = parsed.max(1);
        }
        if let Some(parsed) = std::env::var("SUBCOG_LLM_BREAKER_RESET_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            self.breaker_reset_timeout_ms = parsed;
        }
        if let Some(parsed) = std::env::var("SUBCOG_LLM_BREAKER_HALF_OPEN_MAX_CALLS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
        {
            self.breaker_half_open_max_calls = parsed.max(1);
        }
        if let Some(parsed) = std::env::var("SUBCOG_LLM_LATENCY_SLO_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            self.latency_slo_ms = parsed;
        }
        if let Some(parsed) = std::env::var("SUBCOG_LLM_ERROR_BUDGET_RATIO")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
        {
            self.error_budget_ratio = parsed.clamp(0.0, 1.0);
        }
        if let Some(parsed) = std::env::var("SUBCOG_LLM_ERROR_BUDGET_WINDOW_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            self.error_budget_window_secs = parsed.max(1);
        }

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

#[derive(Debug)]
struct CircuitBreaker {
    state: BreakerState,
    failure_threshold: u32,
    reset_timeout: Duration,
    half_open_max_calls: u32,
}

impl CircuitBreaker {
    fn new(config: &LlmResilienceConfig) -> Self {
        Self {
            state: BreakerState::Closed { failures: 0 },
            failure_threshold: config.breaker_failure_threshold.max(1),
            reset_timeout: Duration::from_millis(config.breaker_reset_timeout_ms),
            half_open_max_calls: config.breaker_half_open_max_calls.max(1),
        }
    }

    fn allow(&mut self) -> bool {
        match self.state {
            BreakerState::Closed { .. } => true,
            BreakerState::Open { opened_at } => {
                if opened_at.elapsed() >= self.reset_timeout {
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

    const fn on_success(&mut self) {
        self.state = BreakerState::Closed { failures: 0 };
    }

    fn on_failure(&mut self) -> bool {
        match self.state {
            BreakerState::Closed { ref mut failures } => {
                *failures += 1;
                if *failures >= self.failure_threshold {
                    self.state = BreakerState::Open {
                        opened_at: Instant::now(),
                    };
                    return true;
                }
            },
            BreakerState::HalfOpen { .. } => {
                self.state = BreakerState::Open {
                    opened_at: Instant::now(),
                };
                return true;
            },
            BreakerState::Open { .. } => {},
        }
        false
    }

    const fn state_value(&self) -> u8 {
        match self.state {
            BreakerState::Closed { .. } => 0,
            BreakerState::Open { .. } => 1,
            BreakerState::HalfOpen { .. } => 2,
        }
    }
}

#[derive(Debug)]
struct BudgetTracker {
    window: Duration,
    requests: VecDeque<Instant>,
    errors: VecDeque<Instant>,
}

impl BudgetTracker {
    const fn new(window: Duration) -> Self {
        Self {
            window,
            requests: VecDeque::new(),
            errors: VecDeque::new(),
        }
    }

    fn record(&mut self, now: Instant, is_error: bool) -> f64 {
        self.requests.push_back(now);
        if is_error {
            self.errors.push_back(now);
        }
        self.evict_expired(now);
        if self.requests.is_empty() {
            0.0
        } else {
            let error_count = u32::try_from(self.errors.len()).unwrap_or(u32::MAX);
            let request_count = u32::try_from(self.requests.len()).unwrap_or(u32::MAX);
            f64::from(error_count) / f64::from(request_count)
        }
    }

    fn evict_expired(&mut self, now: Instant) {
        let threshold = now.checked_sub(self.window).unwrap_or(now);
        while self
            .requests
            .front()
            .is_some_and(|timestamp| *timestamp < threshold)
        {
            self.requests.pop_front();
        }
        while self
            .errors
            .front()
            .is_some_and(|timestamp| *timestamp < threshold)
        {
            self.errors.pop_front();
        }
    }
}

/// LLM provider wrapper with circuit breaker and budget instrumentation.
pub struct ResilientLlmProvider<P: LlmProvider> {
    inner: P,
    config: LlmResilienceConfig,
    breaker: Mutex<CircuitBreaker>,
    budget: Mutex<BudgetTracker>,
}

enum FailureAction {
    Retry(Error),
    Fail(Error),
}

impl<P: LlmProvider> ResilientLlmProvider<P> {
    /// Creates a new resilient LLM provider wrapper.
    #[must_use]
    pub fn new(inner: P, config: LlmResilienceConfig) -> Self {
        let window = Duration::from_secs(config.error_budget_window_secs.max(1));
        let breaker = CircuitBreaker::new(&config);
        Self {
            inner,
            config,
            breaker: Mutex::new(breaker),
            budget: Mutex::new(BudgetTracker::new(window)),
        }
    }

    fn execute<T, F>(&self, operation: &'static str, mut call: F) -> Result<T>
    where
        F: FnMut() -> Result<T>,
    {
        let provider: &'static str = self.inner.name();
        let span = tracing::info_span!(
            "llm.request",
            provider = provider,
            operation = operation,
            status = tracing::field::Empty,
            error = tracing::field::Empty
        );
        let _enter = span.enter();

        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !breaker.allow() {
            let breaker_state = breaker.state_value();
            drop(breaker);
            Self::record_breaker_state(provider, breaker_state);
            span.record("status", "circuit_open");
            metrics::counter!(
                "llm_requests_total",
                "provider" => provider,
                "operation" => operation,
                "status" => "circuit_open"
            )
            .increment(1);
            metrics::counter!(
                "llm_circuit_breaker_rejections_total",
                "provider" => provider,
                "operation" => operation
            )
            .increment(1);
            return Err(Error::OperationFailed {
                operation: format!("llm_{operation}"),
                cause: "circuit breaker open".to_string(),
            });
        }
        drop(breaker);

        let mut attempts = 0;
        let max_attempts = self.config.max_retries + 1;
        let mut last_error = None;

        while attempts < max_attempts {
            attempts += 1;
            let attempt_start = Instant::now();
            let result = call();
            let elapsed = attempt_start.elapsed();

            if let Ok(value) = result {
                self.record_success(provider, operation, elapsed);
                let breaker_state = self.record_breaker_success_state();
                Self::record_breaker_state(provider, breaker_state);
                span.record("status", "success");
                return Ok(value);
            }

            let Err(err) = result else {
                unreachable!("checked Ok above")
            };
            match self.handle_failure(provider, operation, err, elapsed, attempts, max_attempts) {
                FailureAction::Retry(err) => {
                    last_error = Some(err);
                },
                FailureAction::Fail(err) => return Err(err),
            }
        }

        Err(last_error.unwrap_or_else(|| Error::OperationFailed {
            operation: format!("llm_{operation}"),
            cause: "exhausted retries".to_string(),
        }))
    }

    fn record_success(&self, provider: &'static str, operation: &'static str, elapsed: Duration) {
        self.record_request_metrics(provider, operation, elapsed, false, false);
        let ratio = self
            .budget
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .record(Instant::now(), false);
        self.record_budget_metrics(provider, operation, ratio);
    }

    fn handle_failure(
        &self,
        provider: &'static str,
        operation: &'static str,
        err: Error,
        elapsed: Duration,
        attempts: u32,
        max_attempts: u32,
    ) -> FailureAction {
        let is_timeout = is_timeout_error(&err);
        // CHAOS-CRIT-001/002/003: Retry on all transient errors, not just timeouts
        let retryable = is_retryable_error(&err) && attempts < max_attempts;

        self.record_failure(provider, operation, elapsed, is_timeout, retryable);
        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let tripped = breaker.on_failure();
        let breaker_state = breaker.state_value();
        drop(breaker);
        Self::record_breaker_state(provider, breaker_state);
        if tripped {
            metrics::counter!(
                "llm_circuit_breaker_trips_total",
                "provider" => provider,
                "operation" => operation
            )
            .increment(1);
            tracing::warn!(
                "LLM circuit breaker opened for provider={provider} operation={operation}"
            );
        }

        let status = if is_timeout { "timeout" } else { "error" };
        let span = tracing::Span::current();
        span.record("status", status);
        span.record("error", tracing::field::display(&err));

        if retryable {
            metrics::counter!(
                "llm_retries_total",
                "provider" => provider,
                "operation" => operation
            )
            .increment(1);
            if self.config.retry_backoff_ms > 0 {
                let delay = Self::calculate_retry_delay(self.config.retry_backoff_ms, attempts);
                std::thread::sleep(Duration::from_millis(delay));
            }
            return FailureAction::Retry(err);
        }

        FailureAction::Fail(err)
    }

    fn record_failure(
        &self,
        provider: &'static str,
        operation: &'static str,
        elapsed: Duration,
        is_timeout: bool,
        retryable: bool,
    ) {
        self.record_request_metrics(provider, operation, elapsed, true, is_timeout);
        let ratio = self
            .budget
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .record(Instant::now(), true);
        self.record_budget_metrics(provider, operation, ratio);

        if retryable {
            tracing::warn!(
                "Retrying LLM call provider={provider} operation={operation} elapsed_ms={}",
                elapsed.as_millis()
            );
        }
    }

    fn record_breaker_success_state(&self) -> u8 {
        let mut breaker = self
            .breaker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        breaker.on_success();
        breaker.state_value()
    }

    fn record_request_metrics(
        &self,
        provider: &'static str,
        operation: &'static str,
        elapsed: Duration,
        is_error: bool,
        is_timeout: bool,
    ) {
        let status = if is_timeout {
            "timeout"
        } else if is_error {
            "error"
        } else {
            "success"
        };

        metrics::counter!(
            "llm_requests_total",
            "provider" => provider,
            "operation" => operation,
            "status" => status
        )
        .increment(1);
        metrics::histogram!(
            "llm_request_duration_ms",
            "provider" => provider,
            "operation" => operation,
            "status" => status
        )
        .record(elapsed.as_secs_f64() * 1000.0);

        if is_timeout {
            metrics::counter!(
                "llm_timeouts_total",
                "provider" => provider,
                "operation" => operation
            )
            .increment(1);
        }

        let elapsed_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        if self.config.latency_slo_ms > 0 && elapsed_ms > self.config.latency_slo_ms {
            metrics::counter!(
                "llm_latency_budget_exceeded_total",
                "provider" => provider,
                "operation" => operation
            )
            .increment(1);
        }
    }

    fn record_budget_metrics(&self, provider: &'static str, operation: &'static str, ratio: f64) {
        metrics::gauge!(
            "llm_error_budget_ratio",
            "provider" => provider,
            "operation" => operation
        )
        .set(ratio);

        if ratio > self.config.error_budget_ratio {
            metrics::counter!(
                "llm_error_budget_exceeded_total",
                "provider" => provider,
                "operation" => operation
            )
            .increment(1);
        }
    }

    fn record_breaker_state(provider: &'static str, breaker_state: u8) {
        metrics::gauge!("llm_circuit_breaker_state", "provider" => provider)
            .set(f64::from(breaker_state));
    }

    /// Calculates retry delay with exponential backoff and jitter (CHAOS-CRIT-001/002/003).
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
        let jitter = Self::calculate_jitter(capped_delay);
        let total_delay = capped_delay.saturating_add(jitter);

        tracing::debug!(
            "Retry backoff: attempt={}, base={}ms, exponential={}ms, jitter={}ms, total={}ms",
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
}

impl<P: LlmProvider> LlmProvider for ResilientLlmProvider<P> {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn complete(&self, prompt: &str) -> Result<String> {
        self.execute("complete", || self.inner.complete(prompt))
    }

    fn analyze_for_capture(&self, content: &str) -> Result<CaptureAnalysis> {
        self.execute("analyze_for_capture", || {
            self.inner.analyze_for_capture(content)
        })
    }
}

/// Checks if an error is a timeout error.
fn is_timeout_error(err: &Error) -> bool {
    match err {
        Error::OperationFailed { cause, .. } => {
            let lower = cause.to_lowercase();
            lower.contains("timeout")
                || lower.contains("timed out")
                || lower.contains("deadline")
                || lower.contains("elapsed")
        },
        _ => false,
    }
}

/// Checks if an error is retryable (transient failures that may succeed on retry).
///
/// Retryable errors include:
/// - Timeouts
/// - Connection errors (network issues, DNS failures)
/// - Server errors (5xx)
/// - Rate limiting (429)
fn is_retryable_error(err: &Error) -> bool {
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
                // Server errors (5xx)
                || lower.contains("500")
                || lower.contains("502")
                || lower.contains("503")
                || lower.contains("504")
                || lower.contains("internal server error")
                || lower.contains("bad gateway")
                || lower.contains("service unavailable")
                || lower.contains("gateway timeout")
                // Rate limiting
                || lower.contains("429")
                || lower.contains("rate limit")
                || lower.contains("too many requests")
                || lower.contains("overloaded")
        },
        _ => false,
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
        let config = LlmResilienceConfig::default();
        let breaker = CircuitBreaker::new(&config);
        assert_eq!(breaker.state_value(), 0); // Closed = 0
    }

    #[test]
    fn test_circuit_breaker_allows_calls_when_closed() {
        let config = LlmResilienceConfig::default();
        let mut breaker = CircuitBreaker::new(&config);
        assert!(breaker.allow());
        assert!(breaker.allow());
        assert!(breaker.allow());
    }

    #[test]
    fn test_circuit_breaker_opens_after_threshold_failures() {
        let config = LlmResilienceConfig {
            breaker_failure_threshold: 3,
            ..Default::default()
        };
        let mut breaker = CircuitBreaker::new(&config);

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
        let config = LlmResilienceConfig {
            breaker_failure_threshold: 1,
            breaker_reset_timeout_ms: 10_000, // Long timeout
            ..Default::default()
        };
        let mut breaker = CircuitBreaker::new(&config);

        // Trip the breaker
        breaker.on_failure();
        assert_eq!(breaker.state_value(), 1); // Open

        // Should reject calls
        assert!(!breaker.allow());
        assert!(!breaker.allow());
    }

    #[test]
    fn test_circuit_breaker_transitions_to_half_open_after_timeout() {
        let config = LlmResilienceConfig {
            breaker_failure_threshold: 1,
            breaker_reset_timeout_ms: 0, // Immediate reset
            breaker_half_open_max_calls: 1,
            ..Default::default()
        };
        let mut breaker = CircuitBreaker::new(&config);

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
        let config = LlmResilienceConfig {
            breaker_failure_threshold: 1,
            breaker_reset_timeout_ms: 0,
            breaker_half_open_max_calls: 2, // Allow 2 more calls in half-open state
            ..Default::default()
        };
        let mut breaker = CircuitBreaker::new(&config);

        // Trip and transition to half-open
        breaker.on_failure();
        std::thread::sleep(Duration::from_millis(1));

        // First call transitions to half-open and is allowed (free transition call)
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
        let config = LlmResilienceConfig {
            breaker_failure_threshold: 1,
            breaker_reset_timeout_ms: 0,
            ..Default::default()
        };
        let mut breaker = CircuitBreaker::new(&config);

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
        let config = LlmResilienceConfig {
            breaker_failure_threshold: 1,
            breaker_reset_timeout_ms: 0,
            ..Default::default()
        };
        let mut breaker = CircuitBreaker::new(&config);

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
    // Budget Tracker Tests
    // =========================================================================

    #[test]
    fn test_budget_tracker_starts_empty() {
        let tracker = BudgetTracker::new(Duration::from_secs(60));
        assert!(tracker.requests.is_empty());
        assert!(tracker.errors.is_empty());
    }

    #[test]
    fn test_budget_tracker_records_successful_requests() {
        let mut tracker = BudgetTracker::new(Duration::from_secs(60));
        let now = Instant::now();

        let ratio = tracker.record(now, false);
        assert!(ratio.abs() < f64::EPSILON); // No errors

        let ratio = tracker.record(now, false);
        assert!(ratio.abs() < f64::EPSILON); // Still no errors
    }

    #[test]
    fn test_budget_tracker_records_error_requests() {
        let mut tracker = BudgetTracker::new(Duration::from_secs(60));
        let now = Instant::now();

        tracker.record(now, false);
        tracker.record(now, false);
        let ratio = tracker.record(now, true);

        // 1 error out of 3 requests = 0.333...
        assert!((ratio - 0.333).abs() < 0.01);
    }

    #[test]
    fn test_budget_tracker_calculates_error_ratio() {
        let mut tracker = BudgetTracker::new(Duration::from_secs(60));
        let now = Instant::now();

        // 50% error rate
        let ratio = tracker.record(now, true);
        assert!((ratio - 1.0).abs() < f64::EPSILON); // 1/1

        let ratio = tracker.record(now, false);
        assert!((ratio - 0.5).abs() < f64::EPSILON); // 1/2

        let ratio = tracker.record(now, true);
        // 2 errors / 3 total = 0.666...
        assert!((ratio - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_budget_tracker_evicts_expired_entries() {
        // Very short window for testing
        let mut tracker = BudgetTracker::new(Duration::from_millis(10));
        let now = Instant::now();

        tracker.record(now, true);
        assert_eq!(tracker.requests.len(), 1);
        assert_eq!(tracker.errors.len(), 1);

        // Wait for entries to expire
        std::thread::sleep(Duration::from_millis(15));

        // Recording a new request should evict old entries
        let new_now = Instant::now();
        tracker.record(new_now, false);

        // Old entries should be evicted
        assert_eq!(tracker.requests.len(), 1);
        assert_eq!(tracker.errors.len(), 0);
    }

    // =========================================================================
    // Configuration Tests
    // =========================================================================

    #[test]
    fn test_config_default_values() {
        let config = LlmResilienceConfig::default();
        assert_eq!(config.max_retries, 3); // CHAOS-CRIT-001/002/003: 3 retries by default
        assert_eq!(config.retry_backoff_ms, 100);
        assert_eq!(config.breaker_failure_threshold, 3);
        assert_eq!(config.breaker_reset_timeout_ms, 30_000);
        assert_eq!(config.breaker_half_open_max_calls, 1);
        assert_eq!(config.latency_slo_ms, 2_000);
        assert!((config.error_budget_ratio - 0.05).abs() < 0.001);
        assert_eq!(config.error_budget_window_secs, 300);
    }

    #[test]
    fn test_config_from_config_file() {
        let llm_config = crate::config::LlmConfig {
            max_retries: Some(5),
            retry_backoff_ms: Some(200),
            breaker_failure_threshold: Some(10),
            breaker_reset_ms: Some(60_000),
            breaker_half_open_max_calls: Some(3),
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
        assert_eq!(config.breaker_half_open_max_calls, 3);
        assert_eq!(config.latency_slo_ms, 5_000);
        assert!((config.error_budget_ratio - 0.10).abs() < 0.001);
        assert_eq!(config.error_budget_window_secs, 600);
    }

    #[test]
    fn test_config_clamps_values() {
        let llm_config = crate::config::LlmConfig {
            breaker_failure_threshold: Some(0),   // Should be clamped to 1
            breaker_half_open_max_calls: Some(0), // Should be clamped to 1
            error_budget_ratio: Some(2.0),        // Should be clamped to 1.0
            error_budget_window_secs: Some(0),    // Should be clamped to 1
            ..Default::default()
        };

        let config = LlmResilienceConfig::from_config(&llm_config);
        assert_eq!(config.breaker_failure_threshold, 1);
        assert_eq!(config.breaker_half_open_max_calls, 1);
        assert!((config.error_budget_ratio - 1.0).abs() < 0.001);
        assert_eq!(config.error_budget_window_secs, 1);
    }

    // =========================================================================
    // Error Detection Tests
    // =========================================================================

    #[test]
    fn test_is_timeout_error_detects_timeout() {
        let err = Error::OperationFailed {
            operation: "test".to_string(),
            cause: "Connection timeout".to_string(),
        };
        assert!(is_timeout_error(&err));
    }

    #[test]
    fn test_is_timeout_error_detects_timed_out() {
        let err = Error::OperationFailed {
            operation: "test".to_string(),
            cause: "Request timed out".to_string(),
        };
        assert!(is_timeout_error(&err));
    }

    #[test]
    fn test_is_timeout_error_detects_deadline() {
        let err = Error::OperationFailed {
            operation: "test".to_string(),
            cause: "Deadline exceeded".to_string(),
        };
        assert!(is_timeout_error(&err));
    }

    #[test]
    fn test_is_timeout_error_detects_elapsed() {
        let err = Error::OperationFailed {
            operation: "test".to_string(),
            cause: "Time elapsed".to_string(),
        };
        assert!(is_timeout_error(&err));
    }

    #[test]
    fn test_is_timeout_error_returns_false_for_other_errors() {
        let err = Error::OperationFailed {
            operation: "test".to_string(),
            cause: "Connection refused".to_string(),
        };
        assert!(!is_timeout_error(&err));
    }

    #[test]
    fn test_is_timeout_error_is_case_insensitive() {
        let err = Error::OperationFailed {
            operation: "test".to_string(),
            cause: "TIMEOUT ERROR".to_string(),
        };
        assert!(is_timeout_error(&err));
    }

    // =========================================================================
    // Retryable Error Detection Tests (CHAOS-CRIT-001/002/003)
    // =========================================================================

    #[test]
    fn test_is_retryable_error_detects_connection_errors() {
        let test_cases = [
            "connect error: connection refused",
            "Connection reset by peer",
            "network error: no route to host",
            "DNS lookup failed",
            "Failed to resolve hostname",
        ];

        for cause in test_cases {
            let err = Error::OperationFailed {
                operation: "test".to_string(),
                cause: cause.to_string(),
            };
            assert!(is_retryable_error(&err), "Should be retryable: {cause}");
        }
    }

    #[test]
    fn test_is_retryable_error_detects_server_errors() {
        let test_cases = [
            "API returned status: 500",
            "502 Bad Gateway",
            "503 Service Unavailable",
            "504 Gateway Timeout",
            "Internal Server Error",
            "Bad Gateway error",
            "Service unavailable",
            "Gateway timeout exceeded",
        ];

        for cause in test_cases {
            let err = Error::OperationFailed {
                operation: "test".to_string(),
                cause: cause.to_string(),
            };
            assert!(is_retryable_error(&err), "Should be retryable: {cause}");
        }
    }

    #[test]
    fn test_is_retryable_error_detects_rate_limiting() {
        let test_cases = [
            "API returned status: 429",
            "Rate limit exceeded",
            "Too many requests",
            "Server overloaded",
        ];

        for cause in test_cases {
            let err = Error::OperationFailed {
                operation: "test".to_string(),
                cause: cause.to_string(),
            };
            assert!(is_retryable_error(&err), "Should be retryable: {cause}");
        }
    }

    #[test]
    fn test_is_retryable_error_includes_timeout_errors() {
        let err = Error::OperationFailed {
            operation: "test".to_string(),
            cause: "Request timeout".to_string(),
        };
        assert!(is_retryable_error(&err));
    }

    #[test]
    fn test_is_retryable_error_returns_false_for_client_errors() {
        let test_cases = [
            "API returned status: 400 - Bad Request",
            "401 Unauthorized",
            "403 Forbidden",
            "404 Not Found",
            "Invalid API key format",
            "Malformed JSON in request",
        ];

        for cause in test_cases {
            let err = Error::OperationFailed {
                operation: "test".to_string(),
                cause: cause.to_string(),
            };
            assert!(
                !is_retryable_error(&err),
                "Should NOT be retryable: {cause}"
            );
        }
    }

    #[test]
    fn test_is_retryable_error_is_case_insensitive() {
        let err = Error::OperationFailed {
            operation: "test".to_string(),
            cause: "CONNECTION REFUSED".to_string(),
        };
        assert!(is_retryable_error(&err));
    }
}
