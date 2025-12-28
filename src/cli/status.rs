//! Status CLI command.

/// Status command handler.
pub struct StatusCommand;

impl StatusCommand {
    /// Creates a new status command.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for StatusCommand {
    fn default() -> Self {
        Self::new()
    }
}
