//! Recall CLI command.

/// Recall command handler.
pub struct RecallCommand;

impl RecallCommand {
    /// Creates a new recall command.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for RecallCommand {
    fn default() -> Self {
        Self::new()
    }
}
