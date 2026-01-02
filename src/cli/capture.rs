//! Capture CLI command.

/// Capture command handler.
pub struct CaptureCommand;

impl CaptureCommand {
    /// Creates a new capture command.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CaptureCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_command_new() {
        let _cmd = CaptureCommand::new();
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn test_capture_command_default() {
        let _cmd = CaptureCommand::default();
    }
}
