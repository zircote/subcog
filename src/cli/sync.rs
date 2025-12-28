//! Sync CLI command.

/// Sync command handler.
pub struct SyncCommand;

impl SyncCommand {
    /// Creates a new sync command.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for SyncCommand {
    fn default() -> Self {
        Self::new()
    }
}
