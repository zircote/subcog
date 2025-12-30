//! OTLP exporter.

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
