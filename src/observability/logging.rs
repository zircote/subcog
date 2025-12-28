//! Structured logging.

/// Logger for structured logging.
pub struct Logger;

impl Logger {
    /// Creates a new logger.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new()
    }
}
