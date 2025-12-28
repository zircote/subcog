//! Distributed tracing.

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
