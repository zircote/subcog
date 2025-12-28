//! Hook CLI command.

/// Hook command handler.
pub struct HookCommand;

impl HookCommand {
    /// Creates a new hook command.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for HookCommand {
    fn default() -> Self {
        Self::new()
    }
}
