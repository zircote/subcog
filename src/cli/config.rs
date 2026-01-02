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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_command_new() {
        let _cmd = ConfigCommand::new();
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn test_config_command_default() {
        let _cmd = ConfigCommand::default();
    }
}
