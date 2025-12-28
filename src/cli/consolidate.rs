//! Consolidate CLI command.

/// Consolidate command handler.
pub struct ConsolidateCommand;

impl ConsolidateCommand {
    /// Creates a new consolidate command.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ConsolidateCommand {
    fn default() -> Self {
        Self::new()
    }
}
