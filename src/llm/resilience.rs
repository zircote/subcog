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
            max_retries: 0,
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
        if let Ok(v) = std::env::var("SUBCOG_LLM_MAX_RETRIES") {
            if let Ok(parsed) = v.parse::<u32>() {
                self.max_retries = parsed;
            }
        }
        if let Ok(v) = std::env::var("SUBCOG_LLM_RETRY_BACKOFF_MS") {
            if let Ok(parsed) = v.parse::<u64>() {
                self.retry_backoff_ms = parsed;
            }
        }
        if let Ok(v) = std::env::var("SUBCOG_LLM_BREAKER_FAILURE_THRESHOLD") {
            if let Ok(parsed) = v.parse::<u32>() {
                self.breaker_failure_threshold = parsed.max(1);
            }
        }
        if let Ok(v) = std::env::var("SUBCOG_LLM_BREAKER_RESET_MS") {
            if let Ok(parsed) = v.parse::<u64>() {
                self.breaker_reset_timeout_ms = parsed;
            }
        }
        if let Ok(v) = std::env::var("SUBCOG_LLM_BREAKER_HALF_OPEN_MAX_CALLS") {
            if let Ok(parsed) = v.parse::<u32>() {
                self.breaker_half_open_max_calls = parsed.max(1);
            }
        }
        if let Ok(v) = std::env::var("SUBCOG_LLM_LATENCY_SLO_MS") {
            if let Ok(parsed) = v.parse::<u64>() {
                self.latency_slo_ms = parsed;
            }
        }
        if let Ok(v) = std::env::var("SUBCOG_LLM_ERROR_BUDGET_RATIO") {
            if let Ok(parsed) = v.parse::<f64>() {
                self.error_budget_ratio = parsed.clamp(0.0, 1.0);
            }
        }
        if let Ok(v) = std::env::var("SUBCOG_LLM_ERROR_BUDGET_WINDOW_SECS") {
            if let Ok(parsed) = v.parse::<u64>() {
                self.error_budget_window_secs = parsed.max(1);
            }
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
        let retryable = is_timeout && attempts < max_attempts;

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
                std::thread::sleep(Duration::from_millis(self.config.retry_backoff_ms));
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
