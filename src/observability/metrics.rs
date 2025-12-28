//! Prometheus metrics.

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
