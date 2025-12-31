//! Observability and telemetry.

mod logging;
mod metrics;
mod otlp;
mod tracing;

pub use logging::{LogFormat, Logger, LoggingConfig};
pub use metrics::{Metrics, MetricsConfig};
pub use otlp::{OtlpConfig, OtlpExporter, OtlpProtocol};
pub use tracing::{Tracer, TracingConfig};

use crate::config::ObservabilitySettings;
use crate::{Error, Result};
use std::sync::OnceLock;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Full observability configuration.
#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    /// Logging configuration.
    pub logging: LoggingConfig,
    /// Tracing configuration.
    pub tracing: TracingConfig,
    /// Metrics configuration.
    pub metrics: MetricsConfig,
    /// Whether to expose metrics via HTTP listener.
    pub metrics_expose: bool,
}

/// Options for environment-based initialization.
#[derive(Debug, Clone, Copy)]
pub struct InitOptions {
    /// Whether verbose output was requested via CLI.
    pub verbose: bool,
    /// Whether to expose metrics via HTTP listener.
    pub metrics_expose: bool,
}

/// Handle for observability runtime components.
pub struct ObservabilityHandle {
    tracer_provider: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
    metrics_handle: Option<metrics::MetricsHandle>,
    tracing_runtime: Option<tokio::runtime::Runtime>,
}

static OBSERVABILITY_INIT: OnceLock<()> = OnceLock::new();

impl Drop for ObservabilityHandle {
    fn drop(&mut self) {
        if let Some(handle) = self.metrics_handle.take() {
            metrics::flush(&handle);
        }
        if let Some(provider) = self.tracer_provider.take() {
            let _ = provider.shutdown();
        }
        let _ = self.tracing_runtime.take();
    }
}

/// Initializes observability using environment variables.
///
/// # Errors
///
/// Returns an error if observability has already been initialized or if any
/// telemetry components fail to initialize.
pub fn init_from_env(options: InitOptions) -> Result<ObservabilityHandle> {
    let config = build_config(None, options);

    init(config)
}

/// Initializes observability from config settings with env overrides.
///
/// # Errors
///
/// Returns an error if observability has already been initialized or if any
/// telemetry components fail to initialize.
pub fn init_from_config(
    settings: &ObservabilitySettings,
    options: InitOptions,
) -> Result<ObservabilityHandle> {
    let config = build_config(Some(settings), options);

    init(config)
}

fn build_config(
    settings: Option<&ObservabilitySettings>,
    options: InitOptions,
) -> ObservabilityConfig {
    let logging = LoggingConfig::from_settings(
        settings.and_then(|cfg| cfg.logging.as_ref()),
        options.verbose,
    );
    let tracing = TracingConfig::from_settings(settings.and_then(|cfg| cfg.tracing.as_ref()));
    let metrics = MetricsConfig::from_settings(settings.and_then(|cfg| cfg.metrics.as_ref()));

    ObservabilityConfig {
        logging,
        tracing,
        metrics,
        metrics_expose: options.metrics_expose,
    }
}

/// Initializes logging, tracing, and metrics for the process.
///
/// # Errors
///
/// Returns an error if observability has already been initialized or if any
/// telemetry components fail to initialize.
pub fn init(config: ObservabilityConfig) -> Result<ObservabilityHandle> {
    if OBSERVABILITY_INIT.get().is_some() {
        return Err(Error::OperationFailed {
            operation: "observability_init".to_string(),
            cause: "observability already initialized".to_string(),
        });
    }

    let metrics_handle = metrics::install_prometheus(&config.metrics, config.metrics_expose)?;

    let tracing_init = tracing::build_tracing(&config.tracing)?;
    let (tracing_layer, tracer_provider, tracing_runtime) = match tracing_init {
        Some(init) => (Some(init.layer), Some(init.provider), init.runtime),
        None => (None, None, None),
    };

    match (config.logging.format, tracing_layer) {
        (LogFormat::Json, tracing_layer) => tracing_subscriber::registry()
            .with(tracing_layer)
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_current_span(true)
                    .with_span_list(true)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_thread_names(true),
            )
            .with(config.logging.filter)
            .try_init()
            .map_err(
                |e: tracing_subscriber::util::TryInitError| Error::OperationFailed {
                    operation: "observability_init".to_string(),
                    cause: e.to_string(),
                },
            )?,
        (LogFormat::Pretty, tracing_layer) => tracing_subscriber::registry()
            .with(tracing_layer)
            .with(
                tracing_subscriber::fmt::layer()
                    .pretty()
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_thread_names(true),
            )
            .with(config.logging.filter)
            .try_init()
            .map_err(
                |e: tracing_subscriber::util::TryInitError| Error::OperationFailed {
                    operation: "observability_init".to_string(),
                    cause: e.to_string(),
                },
            )?,
    }

    OBSERVABILITY_INIT
        .set(())
        .map_err(|()| Error::OperationFailed {
            operation: "observability_init".to_string(),
            cause: "failed to mark observability initialized".to_string(),
        })?;

    Ok(ObservabilityHandle {
        tracer_provider,
        metrics_handle,
        tracing_runtime,
    })
}
