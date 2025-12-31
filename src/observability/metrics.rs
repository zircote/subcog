//! Prometheus metrics.

use crate::config::{MetricsPushGatewaySettings, MetricsSettings};
use crate::{Error, Result};
use metrics_exporter_prometheus::PrometheusBuilder;
use metrics_exporter_prometheus::PrometheusHandle;
use metrics_exporter_prometheus::PrometheusRecorder;
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::thread;

/// Push gateway configuration.
#[derive(Debug, Clone)]
pub struct PushGatewayConfig {
    /// Push gateway endpoint URI.
    pub endpoint: String,
    /// Optional username for basic auth.
    pub username: Option<String>,
    /// Optional password for basic auth.
    pub password: Option<String>,
    /// Whether to use HTTP POST instead of PUT.
    pub use_http_post: bool,
}

/// Metrics configuration.
#[derive(Debug, Clone)]
pub struct MetricsConfig {
    /// Whether metrics are enabled.
    pub enabled: bool,
    /// Address to bind the metrics exporter.
    pub listen_addr: SocketAddr,
    /// Optional push gateway configuration.
    pub push_gateway: Option<PushGatewayConfig>,
}

impl MetricsConfig {
    /// Builds metrics configuration from environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_settings(None)
    }

    /// Builds metrics configuration from config settings with env overrides.
    #[must_use]
    pub fn from_settings(settings: Option<&MetricsSettings>) -> Self {
        let enabled = settings.and_then(|config| config.enabled).unwrap_or(false);
        let port = settings.and_then(|config| config.port).unwrap_or(9090);
        let push_gateway = settings
            .and_then(|config| config.push_gateway.as_ref())
            .and_then(parse_push_gateway_settings);

        let mut config = Self {
            enabled,
            listen_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port),
            push_gateway,
        };

        if let Some(enabled) = parse_bool_env("SUBCOG_METRICS_ENABLED") {
            config.enabled = enabled;
        }
        if let Some(port) = parse_port_env("SUBCOG_METRICS_PORT") {
            config.listen_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);
        }
        apply_push_gateway_env_overrides(&mut config);

        config
    }
}

/// Metrics collector.
pub struct Metrics;

impl Metrics {
    /// Creates a new metrics collector.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics handle for flushing on shutdown.
#[derive(Debug)]
pub struct MetricsHandle {
    prometheus: PrometheusHandle,
    push_gateway: Option<PushGatewayConfig>,
}

/// Installs the Prometheus metrics recorder and HTTP listener.
pub fn install_prometheus(config: &MetricsConfig, expose: bool) -> Result<Option<MetricsHandle>> {
    if !config.enabled {
        return Ok(None);
    }

    let builder = PrometheusBuilder::new();
    let handle = if expose {
        let builder = builder.with_http_listener(config.listen_addr);
        install_listener(builder)?
    } else {
        builder
            .install_recorder()
            .map_err(|e| Error::OperationFailed {
                operation: "metrics_recorder_install".to_string(),
                cause: e.to_string(),
            })?
    };

    Ok(Some(MetricsHandle {
        prometheus: handle,
        push_gateway: config.push_gateway.clone(),
    }))
}

/// Flushes metrics to the push gateway if configured.
pub fn flush(handle: &MetricsHandle) {
    let Some(push_gateway) = &handle.push_gateway else {
        return;
    };

    let client = Client::new();
    let payload = handle.prometheus.render();
    let request = if push_gateway.use_http_post {
        client.post(&push_gateway.endpoint)
    } else {
        client.put(&push_gateway.endpoint)
    };

    let request = if let Some(ref username) = push_gateway.username {
        request.basic_auth(username, push_gateway.password.as_ref())
    } else {
        request
    };

    let response = request
        .header(CONTENT_TYPE, "text/plain; version=0.0.4")
        .body(payload)
        .send();

    if let Err(err) = response {
        tracing::warn!("Failed to push metrics: {err}");
    }
}

fn install_listener(builder: PrometheusBuilder) -> Result<PrometheusHandle> {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        return install_with_runtime(builder, &handle);
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| Error::OperationFailed {
            operation: "metrics_runtime_init".to_string(),
            cause: e.to_string(),
        })?;
    let handle = runtime.handle().clone();
    let prometheus = install_with_runtime(builder, &handle)?;
    let thread_name = "metrics-exporter-prometheus-http".to_string();
    thread::Builder::new()
        .name(thread_name)
        .spawn(move || runtime.block_on(async { std::future::pending::<()>().await }))
        .map_err(|e| Error::OperationFailed {
            operation: "metrics_runtime_thread".to_string(),
            cause: e.to_string(),
        })?;
    Ok(prometheus)
}

fn install_with_runtime(
    builder: PrometheusBuilder,
    runtime_handle: &tokio::runtime::Handle,
) -> Result<PrometheusHandle> {
    let (recorder, exporter) = {
        let _guard = runtime_handle.enter();
        builder.build().map_err(|e| Error::OperationFailed {
            operation: "metrics_exporter_build".to_string(),
            cause: e.to_string(),
        })?
    };
    let handle = recorder.handle();
    set_global_recorder(recorder)?;
    runtime_handle.spawn(exporter);
    Ok(handle)
}

fn set_global_recorder(recorder: PrometheusRecorder) -> Result<()> {
    metrics::set_global_recorder(recorder).map_err(|e| Error::OperationFailed {
        operation: "metrics_recorder_install".to_string(),
        cause: e.to_string(),
    })
}

fn parse_bool_env(key: &str) -> Option<bool> {
    std::env::var(key).ok().map(|value| {
        let value = value.to_lowercase();
        value == "true" || value == "1" || value == "yes"
    })
}

fn parse_port_env(key: &str) -> Option<u16> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
}

fn parse_string_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_push_gateway_settings(settings: &MetricsPushGatewaySettings) -> Option<PushGatewayConfig> {
    let endpoint = settings
        .endpoint
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;

    let username = settings
        .username
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let password = settings
        .password
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let use_http_post = settings.use_http_post.unwrap_or(false);

    Some(PushGatewayConfig {
        endpoint,
        username,
        password,
        use_http_post,
    })
}

fn apply_push_gateway_env_overrides(config: &mut MetricsConfig) {
    let endpoint = parse_string_env("SUBCOG_METRICS_PUSH_GATEWAY_ENDPOINT");
    let username = parse_string_env("SUBCOG_METRICS_PUSH_GATEWAY_USERNAME");
    let password = parse_string_env("SUBCOG_METRICS_PUSH_GATEWAY_PASSWORD");
    let use_http_post = parse_bool_env("SUBCOG_METRICS_PUSH_GATEWAY_USE_POST");

    if endpoint.is_none() && username.is_none() && password.is_none() && use_http_post.is_none() {
        return;
    }

    let mut current = config.push_gateway.clone().unwrap_or(PushGatewayConfig {
        endpoint: String::new(),
        username: None,
        password: None,
        use_http_post: false,
    });

    if let Some(endpoint) = endpoint {
        current.endpoint = endpoint;
    }
    if let Some(username) = username {
        current.username = Some(username);
    }
    if let Some(password) = password {
        current.password = Some(password);
    }
    if let Some(use_http_post) = use_http_post {
        current.use_http_post = use_http_post;
    }

    if current.endpoint.trim().is_empty() {
        return;
    }

    config.push_gateway = Some(current);
}
