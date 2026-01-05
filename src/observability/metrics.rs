//! Prometheus metrics.

use crate::config::{MetricsPushGatewaySettings, MetricsSettings};
use crate::{Error, Result};
use metrics_exporter_prometheus::PrometheusBuilder;
use metrics_exporter_prometheus::PrometheusHandle;
use metrics_exporter_prometheus::PrometheusRecorder;
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, OnceLock};
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

/// Global metrics handle for flush-on-demand.
static GLOBAL_METRICS: OnceLock<Arc<MetricsHandle>> = OnceLock::new();

/// Global metrics instance label for push gateway grouping.
static METRICS_INSTANCE: OnceLock<String> = OnceLock::new();

/// Sets the metrics instance label for push gateway grouping.
///
/// This allows hooks and MCP server to push to separate metric groups.
/// Must be called before any metrics are flushed. Only the first call
/// takes effect (subsequent calls are ignored).
pub fn set_instance_label(label: &str) {
    let _ = METRICS_INSTANCE.set(label.to_string());
}

/// Gets the configured instance label, if any.
fn get_instance_label() -> Option<&'static str> {
    METRICS_INSTANCE.get().map(String::as_str)
}

/// Flushes metrics to the push gateway if configured.
///
/// This can be called from anywhere to push metrics immediately,
/// useful for short-lived processes like MCP server requests.
pub fn flush_global() {
    if let Some(handle) = GLOBAL_METRICS.get() {
        flush(handle);
    }
}

/// Installs the Prometheus metrics recorder and HTTP listener.
pub fn install_prometheus(config: &MetricsConfig, expose: bool) -> Result<Option<MetricsHandle>> {
    if !config.enabled {
        return Ok(None);
    }

    let builder = PrometheusBuilder::new();
    let prometheus_handle = if expose {
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

    let handle = MetricsHandle {
        prometheus: prometheus_handle,
        push_gateway: config.push_gateway.clone(),
    };

    // Store globally for flush_global() access
    let _ = GLOBAL_METRICS.set(Arc::new(MetricsHandle {
        prometheus: handle.prometheus.clone(),
        push_gateway: handle.push_gateway.clone(),
    }));

    Ok(Some(handle))
}

/// Flushes metrics to the push gateway if configured.
///
/// When called from within a tokio runtime, this spawns a separate thread
/// to avoid runtime nesting issues with `reqwest::blocking::Client`.
pub fn flush(handle: &MetricsHandle) {
    let Some(push_gateway) = &handle.push_gateway else {
        tracing::debug!("No push gateway configured, skipping flush");
        return;
    };

    let mut payload = handle.prometheus.render();

    // Ensure payload ends with newline (required by push gateway)
    if !payload.ends_with('\n') {
        payload.push('\n');
    }

    // Support instance label for push gateway grouping
    // This allows hooks and MCP server to push to separate metric groups
    let endpoint = get_instance_label().map_or_else(
        || push_gateway.endpoint.clone(),
        |instance| {
            format!(
                "{}/instance/{instance}",
                push_gateway.endpoint.trim_end_matches('/')
            )
        },
    );
    let use_http_post = push_gateway.use_http_post;
    let username = push_gateway.username.clone();
    let password = push_gateway.password.clone();

    tracing::debug!(
        bytes = payload.len(),
        endpoint = %endpoint,
        instance = ?get_instance_label(),
        "Pushing metrics to push gateway"
    );

    // Check if we're in a tokio runtime - if so, spawn a thread to avoid
    // runtime nesting issues with reqwest::blocking::Client
    if tokio::runtime::Handle::try_current().is_ok() {
        // Spawn thread and wait for completion to ensure metrics are pushed
        let handle = thread::spawn(move || {
            flush_to_gateway(
                &endpoint,
                payload,
                use_http_post,
                username.as_deref(),
                password.as_deref(),
            );
        });
        // Wait for the flush to complete (with timeout)
        let _ = handle.join();
    } else {
        flush_to_gateway(
            &endpoint,
            payload,
            use_http_post,
            username.as_deref(),
            password.as_deref(),
        );
    }
}

/// Internal function to push metrics to the gateway.
fn flush_to_gateway(
    endpoint: &str,
    payload: String,
    use_http_post: bool,
    username: Option<&str>,
    password: Option<&str>,
) {
    let client = Client::new();

    let request = if use_http_post {
        client.post(endpoint)
    } else {
        client.put(endpoint)
    };

    let request = if let Some(username) = username {
        request.basic_auth(username, password)
    } else {
        request
    };

    // Use timeout to ensure connection is properly established
    let response = request
        .header(CONTENT_TYPE, "text/plain; version=0.0.4")
        .timeout(std::time::Duration::from_secs(5))
        .body(payload)
        .send();

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                tracing::debug!(status = %resp.status(), "Metrics pushed successfully");
            } else {
                tracing::warn!(status = %resp.status(), "Metrics push failed");
            }
        },
        Err(err) => {
            tracing::warn!("Failed to push metrics: {err}");
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_registry_smoke() {
        let recorder = PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();
        if metrics::set_boxed_recorder(Box::new(recorder)).is_err() {
            return;
        }

        metrics::counter!("test_metrics_registry_total").increment(1);
        let rendered = handle.render();
        assert!(rendered.contains("test_metrics_registry_total"));
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
    // Default to POST (accumulates metrics) instead of PUT (replaces all metrics)
    // POST is better for multi-hook scenarios where each hook pushes independently
    let use_http_post = settings.use_http_post.unwrap_or(true);

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
