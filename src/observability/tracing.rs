//! Distributed tracing and OTLP logging.

use crate::config::TracingSettings;
use crate::{Error, Result};
use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::{RandomIdGenerator, Sampler, SdkTracerProvider};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::Registry;

use super::otlp::{OtlpConfig, OtlpProtocol, endpoint_from_env};

const DEFAULT_TRACE_SAMPLE_RATIO: f64 = 1.0;

/// Tracing configuration.
#[derive(Debug, Clone)]
pub struct TracingConfig {
    /// Whether tracing is enabled.
    pub enabled: bool,
    /// OTLP exporter configuration.
    pub otlp: OtlpConfig,
    /// Sample ratio for trace sampling (0.0 - 1.0).
    pub sample_ratio: f64,
    /// Service name for telemetry.
    pub service_name: String,
    /// Service version for telemetry.
    pub service_version: String,
    /// Additional resource attributes.
    pub resource_attributes: Vec<KeyValue>,
}

impl TracingConfig {
    /// Builds tracing configuration from environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_settings(None)
    }

    /// Builds tracing configuration from config settings with env overrides.
    #[must_use]
    pub fn from_settings(settings: Option<&TracingSettings>) -> Self {
        let otlp = OtlpConfig::from_settings(settings.and_then(|config| config.otlp.as_ref()));
        let endpoint_present = otlp.endpoint.is_some();

        let enabled = settings
            .and_then(|config| config.enabled)
            .unwrap_or(endpoint_present);
        let sample_ratio = settings
            .and_then(|config| config.sample_ratio)
            .unwrap_or(DEFAULT_TRACE_SAMPLE_RATIO);
        let service_name = settings
            .and_then(|config| config.service_name.clone())
            .unwrap_or_else(|| env!("CARGO_PKG_NAME").to_string());
        let service_version = env!("CARGO_PKG_VERSION").to_string();
        let resource_attributes = settings
            .and_then(|config| config.resource_attributes.clone())
            .map(parse_resource_attributes_from_settings)
            .unwrap_or_default();

        let mut config = Self {
            enabled,
            otlp,
            sample_ratio,
            service_name,
            service_version,
            resource_attributes,
        };

        apply_env_overrides(&mut config);
        config
    }
}

/// Tracing initialization output.
pub struct TracingInit {
    /// `OpenTelemetry` layer for tracing subscriber.
    pub layer: OpenTelemetryLayer<Registry, opentelemetry_sdk::trace::Tracer>,
    /// Tracer provider for shutdown flushing.
    pub provider: SdkTracerProvider,
    /// OTLP logging layer for tracing subscriber.
    pub logs_layer:
        OpenTelemetryTracingBridge<SdkLoggerProvider, opentelemetry_sdk::logs::SdkLogger>,
    /// Logger provider for shutdown flushing.
    pub logger_provider: SdkLoggerProvider,
    /// Tokio runtime for gRPC exporters when no runtime exists.
    pub runtime: Option<tokio::runtime::Runtime>,
}

/// Tracer for distributed tracing.
pub struct Tracer;

impl Tracer {
    /// Creates a new tracer.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Tracer {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds tracing layer and provider for the given configuration.
pub fn build_tracing(config: &TracingConfig) -> Result<Option<TracingInit>> {
    if !config.enabled {
        return Ok(None);
    }

    let endpoint = config
        .otlp
        .endpoint
        .clone()
        .ok_or_else(|| Error::OperationFailed {
            operation: "tracing_init".to_string(),
            cause: "OTLP endpoint required when tracing is enabled".to_string(),
        })?;

    let runtime = match (config.otlp.protocol, tokio::runtime::Handle::try_current()) {
        (OtlpProtocol::Grpc, Err(_)) => Some(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| Error::OperationFailed {
                    operation: "otlp_runtime_init".to_string(),
                    cause: e.to_string(),
                })?,
        ),
        _ => None,
    };

    let _guard = runtime.as_ref().map(tokio::runtime::Runtime::enter);

    // Build trace exporter
    let trace_exporter = match config.otlp.protocol {
        OtlpProtocol::Grpc => SpanExporter::builder()
            .with_tonic()
            .with_endpoint(&endpoint)
            .build()
            .map_err(|e| Error::OperationFailed {
                operation: "otlp_exporter_build".to_string(),
                cause: e.to_string(),
            })?,
        OtlpProtocol::Http => SpanExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(&endpoint)
            .build()
            .map_err(|e| Error::OperationFailed {
                operation: "otlp_exporter_build".to_string(),
                cause: e.to_string(),
            })?,
    };

    // Build logs exporter
    let log_exporter = match config.otlp.protocol {
        OtlpProtocol::Grpc => LogExporter::builder()
            .with_tonic()
            .with_endpoint(&endpoint)
            .build()
            .map_err(|e| Error::OperationFailed {
                operation: "otlp_log_exporter_build".to_string(),
                cause: e.to_string(),
            })?,
        OtlpProtocol::Http => LogExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(&endpoint)
            .build()
            .map_err(|e| Error::OperationFailed {
                operation: "otlp_log_exporter_build".to_string(),
                cause: e.to_string(),
            })?,
    };

    let mut attributes = vec![
        KeyValue::new("service.name", config.service_name.clone()),
        KeyValue::new("service.version", config.service_version.clone()),
    ];
    attributes.extend(config.resource_attributes.clone());

    let sampler = build_sampler(config.sample_ratio);
    let resource = Resource::builder()
        .with_attributes(attributes.clone())
        .build();
    let provider = SdkTracerProvider::builder()
        .with_sampler(sampler)
        .with_id_generator(RandomIdGenerator::default())
        .with_resource(resource.clone())
        .with_batch_exporter(trace_exporter)
        .build();

    // Build logger provider
    let logger_provider = SdkLoggerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(log_exporter)
        .build();

    global::set_text_map_propagator(TraceContextPropagator::new());
    global::set_tracer_provider(provider.clone());

    let tracer = provider.tracer(config.service_name.clone());
    let layer = OpenTelemetryLayer::new(tracer);
    let logs_layer = OpenTelemetryTracingBridge::new(&logger_provider);

    Ok(Some(TracingInit {
        layer,
        provider,
        logs_layer,
        logger_provider,
        runtime,
    }))
}

fn parse_resource_attributes() -> Vec<KeyValue> {
    let Ok(raw) = std::env::var("OTEL_RESOURCE_ATTRIBUTES") else {
        return Vec::new();
    };

    raw.split(',')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() || value.is_empty() {
                return None;
            }
            Some(KeyValue::new(key.to_string(), value.to_string()))
        })
        .collect()
}

fn parse_resource_attributes_from_settings(values: Vec<String>) -> Vec<KeyValue> {
    values
        .into_iter()
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() || value.is_empty() {
                return None;
            }
            Some(KeyValue::new(key.to_string(), value.to_string()))
        })
        .collect()
}

fn parse_sample_ratio() -> Option<f64> {
    if let Ok(value) = std::env::var("SUBCOG_TRACE_SAMPLE_RATIO") {
        return value.parse::<f64>().ok().map(|v| v.clamp(0.0, 1.0));
    }

    if let Ok(value) = std::env::var("OTEL_TRACES_SAMPLER_ARG") {
        return value.parse::<f64>().ok().map(|v| v.clamp(0.0, 1.0));
    }

    None
}

fn build_sampler(sample_ratio: f64) -> Sampler {
    let sampler_env = std::env::var("SUBCOG_TRACING_SAMPLER")
        .ok()
        .or_else(|| std::env::var("OTEL_TRACES_SAMPLER").ok());
    match sampler_env.as_deref() {
        Some("always_on") => Sampler::AlwaysOn,
        Some("always_off") => Sampler::AlwaysOff,
        Some("traceidratio") => Sampler::TraceIdRatioBased(sample_ratio),
        Some("parentbased_traceidratio") => {
            Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(sample_ratio)))
        },
        Some("parentbased_always_on") => Sampler::ParentBased(Box::new(Sampler::AlwaysOn)),
        Some("parentbased_always_off") => Sampler::ParentBased(Box::new(Sampler::AlwaysOff)),
        _ => Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(sample_ratio))),
    }
}

fn parse_bool_env(key: &str) -> Option<bool> {
    std::env::var(key).ok().map(|value| {
        let value = value.to_lowercase();
        value == "true" || value == "1" || value == "yes"
    })
}

fn apply_env_overrides(config: &mut TracingConfig) {
    if let Some(enabled) = parse_bool_env("SUBCOG_TRACING_ENABLED") {
        config.enabled = enabled;
    } else if endpoint_from_env().is_some() {
        config.enabled = true;
    }

    if let Some(sample_ratio) = parse_sample_ratio() {
        config.sample_ratio = sample_ratio;
    }

    if let Ok(service_name) = std::env::var("OTEL_SERVICE_NAME") {
        config.service_name = service_name;
    }

    if std::env::var("OTEL_RESOURCE_ATTRIBUTES").is_ok() {
        config.resource_attributes = parse_resource_attributes();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_otlp_tracing_init_smoke() {
        let config = TracingConfig {
            enabled: true,
            otlp: OtlpConfig {
                endpoint: Some("http://localhost:4318".to_string()),
                protocol: OtlpProtocol::Http,
            },
            sample_ratio: 1.0,
            service_name: "subcog-test".to_string(),
            service_version: "0.0.0".to_string(),
            resource_attributes: Vec::new(),
        };

        let init = build_tracing(&config).expect("build tracing").expect("init tracing");
        let _ = init.provider.shutdown();
        let _ = init.logger_provider.shutdown();
    }
}
