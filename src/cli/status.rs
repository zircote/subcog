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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_command_new() {
        let _cmd = StatusCommand::new();
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn test_status_command_default() {
        let _cmd = StatusCommand::default();
    }
}
