//! Migration CLI command.
//!
//! Provides commands for migrating existing memories to use new features
//! like real embeddings.

/// Migration command handler.
pub struct MigrateCommand;

impl MigrateCommand {
    /// Creates a new migration command.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for MigrateCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrate_command_new() {
        let _cmd = MigrateCommand::new();
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn test_migrate_command_default() {
        let _cmd = MigrateCommand::default();
    }
}
