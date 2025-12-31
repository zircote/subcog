//! OTLP exporter configuration.

use crate::config::OtlpSettings;

/// OTLP transport protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtlpProtocol {
    /// gRPC transport (4317 default).
    Grpc,
    /// HTTP/protobuf transport (4318 default).
    Http,
}

impl OtlpProtocol {
    /// Parses protocol from environment variable value.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_lowercase().as_str() {
            "grpc" => Some(Self::Grpc),
            "http" | "http/protobuf" | "http_binary" | "http-binary" => Some(Self::Http),
            _ => None,
        }
    }
}

/// OTLP exporter configuration.
#[derive(Debug, Clone)]
pub struct OtlpConfig {
    /// Collector endpoint URL.
    pub endpoint: Option<String>,
    /// Transport protocol.
    pub protocol: OtlpProtocol,
}

impl OtlpConfig {
    /// Builds OTLP configuration from environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_settings(None)
    }

    /// Builds OTLP configuration from config settings with env overrides.
    #[must_use]
    pub fn from_settings(settings: Option<&OtlpSettings>) -> Self {
        let protocol_explicit = settings
            .and_then(|config| config.protocol.as_deref())
            .is_some();
        let endpoint = settings.and_then(|config| config.endpoint.clone());
        let protocol = settings
            .and_then(|config| config.protocol.as_deref())
            .and_then(OtlpProtocol::parse)
            .unwrap_or_else(|| protocol_from_endpoint(endpoint.as_deref()));

        let mut config = Self { endpoint, protocol };
        config.apply_env_overrides(protocol_explicit);
        config
    }
}

/// `OpenTelemetry` Protocol exporter.
pub struct OtlpExporter;

impl OtlpExporter {
    /// Creates a new OTLP exporter.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for OtlpExporter {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) fn endpoint_from_env() -> Option<String> {
    std::env::var("SUBCOG_OTLP_ENDPOINT")
        .ok()
        .or_else(|| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok())
}

fn protocol_from_env_override() -> Option<OtlpProtocol> {
    std::env::var("SUBCOG_OTLP_PROTOCOL")
        .or_else(|_| std::env::var("OTEL_EXPORTER_OTLP_PROTOCOL"))
        .ok()
        .and_then(|value| OtlpProtocol::parse(&value))
}

fn protocol_from_endpoint(endpoint: Option<&str>) -> OtlpProtocol {
    if let Some(endpoint) = endpoint {
        if endpoint.contains(":4317") {
            return OtlpProtocol::Grpc;
        }
    }

    OtlpProtocol::Http
}

impl OtlpConfig {
    fn apply_env_overrides(&mut self, protocol_explicit: bool) {
        let endpoint_override = endpoint_from_env();
        let endpoint_overridden = endpoint_override.is_some();
        if let Some(endpoint) = endpoint_override {
            self.endpoint = Some(endpoint);
        }

        if let Some(protocol) = protocol_from_env_override() {
            self.protocol = protocol;
            return;
        }

        if endpoint_overridden && !protocol_explicit {
            self.protocol = protocol_from_endpoint(self.endpoint.as_deref());
        }
    }
}
