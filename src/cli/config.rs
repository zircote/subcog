//! Config CLI command.

/// Config command handler.
pub struct ConfigCommand;

impl ConfigCommand {
    /// Creates a new config command.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ConfigCommand {
    fn default() -> Self {
        Self::new()
    }
}
