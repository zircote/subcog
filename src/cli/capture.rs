//! Capture CLI command.

/// Capture command handler.
pub struct CaptureCommand;

impl CaptureCommand {
    /// Creates a new capture command.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CaptureCommand {
    fn default() -> Self {
        Self::new()
    }
}
