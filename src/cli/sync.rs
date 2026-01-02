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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_command_new() {
        let _cmd = SyncCommand::new();
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn test_sync_command_default() {
        let _cmd = SyncCommand::default();
    }
}
