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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serve_command_new() {
        let _cmd = ServeCommand::new();
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn test_serve_command_default() {
        let _cmd = ServeCommand::default();
    }
}
