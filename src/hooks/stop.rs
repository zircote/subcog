//! Stop hook handler.

use super::HookHandler;
use crate::Result;

/// Handles Stop hook events.
///
/// Performs session analysis and sync at session end.
pub struct StopHandler;

impl StopHandler {
    /// Creates a new handler.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for StopHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HookHandler for StopHandler {
    fn event_type(&self) -> &'static str {
        "Stop"
    }

    fn handle(&self, _input: &serde_json::Value) -> Result<serde_json::Value> {
        // TODO: Implement session analysis and sync
        todo!("StopHandler::handle not yet implemented")
    }
}
