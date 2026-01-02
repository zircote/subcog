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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recall_command_new() {
        let _cmd = RecallCommand::new();
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn test_recall_command_default() {
        let _cmd = RecallCommand::default();
    }
}
