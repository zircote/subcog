//! Serve CLI command (MCP server).

/// Serve command handler.
pub struct ServeCommand;

impl ServeCommand {
    /// Creates a new serve command.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ServeCommand {
    fn default() -> Self {
        Self::new()
    }
}
