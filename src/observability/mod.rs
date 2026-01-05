//! Observability and telemetry.

mod event_bus;
mod logging;
mod metrics;
mod otlp;
mod request_context;
mod tracing;

pub use event_bus::{EventBus, global_event_bus};
pub use logging::{LogFormat, Logger, LoggingConfig};
pub use metrics::{Metrics, MetricsConfig, flush_global as flush_metrics, set_instance_label};
pub use otlp::{OtlpConfig, OtlpExporter, OtlpProtocol};
pub use request_context::{
    RequestContext, current_request_id, enter_request_context, scope_request_context,
};
pub use tracing::{Tracer, TracingConfig};

use crate::config::ObservabilitySettings;
use crate::{Error, Result};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
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
    logger_provider: Option<opentelemetry_sdk::logs::SdkLoggerProvider>,
    metrics_handle: Option<metrics::MetricsHandle>,
    tracing_runtime: Option<tokio::runtime::Runtime>,
}

static OBSERVABILITY_INIT: OnceLock<()> = OnceLock::new();

impl ObservabilityHandle {
    /// Explicitly shuts down observability components.
    ///
    /// This should be called before dropping the handle when running inside
    /// an async context to avoid panics from blocking operations.
    pub fn shutdown(&mut self) {
        if let Some(handle) = self.metrics_handle.take() {
            metrics::flush(&handle);
        }

        // Force flush and shutdown providers
        let tracer = self.tracer_provider.take();
        let logger = self.logger_provider.take();

        if tracer.is_none() && logger.is_none() {
            let _ = self.tracing_runtime.take();
            return;
        }

        // Take the runtime regardless of context - we need to handle it properly
        let tracing_runtime = self.tracing_runtime.take();

        // Check if we're in an async context
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            // Use block_in_place to safely run blocking operations from async context
            // This avoids "Cannot drop a runtime in a context where blocking is not allowed"
            tokio::task::block_in_place(|| {
                Self::wait_for_batch_export(&handle);
                Self::flush_and_shutdown(tracer, logger);
                Self::shutdown_runtime(tracing_runtime);
            });
        } else if let Some(rt) = tracing_runtime {
            // Use block_on to ensure gRPC batch exporter can flush
            rt.block_on(async {
                // Give the runtime a moment to process pending exports
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            });
            // Now shutdown synchronously within the runtime context
            {
                let _guard = rt.enter();
                Self::flush_and_shutdown(tracer, logger);
            }
            // Let runtime finish any remaining work
            rt.shutdown_timeout(std::time::Duration::from_secs(2));
        } else {
            Self::flush_and_shutdown(tracer, logger);
        }
    }

    /// Flushes and shuts down tracer and logger providers.
    fn flush_and_shutdown(
        tracer: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
        logger: Option<opentelemetry_sdk::logs::SdkLoggerProvider>,
    ) {
        if let Some(ref t) = tracer {
            let _ = t.force_flush();
        }
        if let Some(ref l) = logger {
            let _ = l.force_flush();
        }
        Self::shutdown_provider(tracer);
        Self::shutdown_provider(logger);
    }

    /// Shuts down the tracing runtime with a timeout.
    fn shutdown_runtime(runtime: Option<tokio::runtime::Runtime>) {
        if let Some(rt) = runtime {
            rt.shutdown_timeout(std::time::Duration::from_secs(2));
        }
    }

    /// Waits briefly for the batch exporter to process pending exports.
    fn wait_for_batch_export(handle: &tokio::runtime::Handle) {
        handle.block_on(async {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        });
    }

    /// Shuts down a tracer provider if present.
    fn shutdown_provider<T: ShutdownProvider>(provider: Option<T>) {
        if let Some(p) = provider {
            let _ = p.shutdown();
        }
    }
}

/// Trait for providers that can be shut down.
trait ShutdownProvider {
    /// Shuts down the provider.
    fn shutdown(self) -> opentelemetry_sdk::error::OTelSdkResult;
}

impl ShutdownProvider for opentelemetry_sdk::trace::SdkTracerProvider {
    fn shutdown(self) -> opentelemetry_sdk::error::OTelSdkResult {
        Self::shutdown(&self)
    }
}

impl ShutdownProvider for opentelemetry_sdk::logs::SdkLoggerProvider {
    fn shutdown(self) -> opentelemetry_sdk::error::OTelSdkResult {
        Self::shutdown(&self)
    }
}

impl Drop for ObservabilityHandle {
    fn drop(&mut self) {
        // If components weren't already shut down, do it now
        // This handles the case where shutdown() wasn't called explicitly
        if self.metrics_handle.is_some()
            || self.tracer_provider.is_some()
            || self.logger_provider.is_some()
        {
            self.shutdown();
        }
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
#[allow(clippy::too_many_lines)]
pub fn init(config: ObservabilityConfig) -> Result<ObservabilityHandle> {
    if OBSERVABILITY_INIT.get().is_some() {
        return Err(Error::OperationFailed {
            operation: "observability_init".to_string(),
            cause: "observability already initialized".to_string(),
        });
    }

    let metrics_handle = metrics::install_prometheus(&config.metrics, config.metrics_expose)?;

    let tracing_init = tracing::build_tracing(&config.tracing)?;
    let (tracing_layer, tracer_provider, logs_layer, logger_provider, tracing_runtime) =
        match tracing_init {
            Some(init) => (
                Some(init.layer),
                Some(init.provider),
                Some(init.logs_layer),
                Some(init.logger_provider),
                init.runtime,
            ),
            None => (None, None, None, None, None),
        };

    // Initialize logging based on format and optional file output
    match (&config.logging.file, config.logging.format) {
        (Some(log_file), LogFormat::Json) => {
            let writer = open_log_file(log_file)?;
            tracing_subscriber::registry()
                .with(tracing_layer)
                .with(logs_layer)
                .with(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .with_writer(writer)
                        .with_current_span(true)
                        .with_span_list(true)
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_thread_names(true),
                )
                .with(config.logging.filter)
                .try_init()
                .map_err(init_error)?;
        },
        (Some(log_file), LogFormat::Pretty) => {
            let writer = open_log_file(log_file)?;
            tracing_subscriber::registry()
                .with(tracing_layer)
                .with(logs_layer)
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_writer(writer)
                        .with_ansi(false)
                        .with_current_span(true)
                        .with_span_list(true)
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_thread_names(true),
                )
                .with(config.logging.filter)
                .try_init()
                .map_err(init_error)?;
        },
        (None, LogFormat::Json) => {
            tracing_subscriber::registry()
                .with(tracing_layer)
                .with(logs_layer)
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
                .map_err(init_error)?;
        },
        (None, LogFormat::Pretty) => {
            tracing_subscriber::registry()
                .with(tracing_layer)
                .with(logs_layer)
                .with(
                    tracing_subscriber::fmt::layer()
                        .pretty()
                        .with_current_span(true)
                        .with_span_list(true)
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_thread_names(true),
                )
                .with(config.logging.filter)
                .try_init()
                .map_err(init_error)?;
        },
    }

    OBSERVABILITY_INIT
        .set(())
        .map_err(|()| Error::OperationFailed {
            operation: "observability_init".to_string(),
            cause: "failed to mark observability initialized".to_string(),
        })?;

    Ok(ObservabilityHandle {
        tracer_provider,
        logger_provider,
        metrics_handle,
        tracing_runtime,
    })
}

/// Thread-safe file writer for logging.
#[derive(Clone)]
struct LogFileWriter {
    file: Arc<Mutex<File>>,
}

impl Write for LogFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut guard = self
            .file
            .lock()
            .map_err(|e| io::Error::other(e.to_string()))?;
        guard.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut guard = self
            .file
            .lock()
            .map_err(|e| io::Error::other(e.to_string()))?;
        guard.flush()
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogFileWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Opens a log file for appending.
fn open_log_file(path: &Path) -> Result<LogFileWriter> {
    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::OperationFailed {
            operation: "create_log_dir".to_string(),
            cause: e.to_string(),
        })?;
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| Error::OperationFailed {
            operation: "open_log_file".to_string(),
            cause: format!("{}: {}", path.display(), e),
        })?;

    Ok(LogFileWriter {
        file: Arc::new(Mutex::new(file)),
    })
}

/// Helper to convert init errors.
#[allow(clippy::needless_pass_by_value)]
fn init_error(e: tracing_subscriber::util::TryInitError) -> Error {
    Error::OperationFailed {
        operation: "observability_init".to_string(),
        cause: e.to_string(),
    }
}
