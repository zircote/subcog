//! Session start hook handler.

use super::HookHandler;
use crate::Result;

/// Handles SessionStart hook events.
///
/// Injects relevant context at the start of a Claude Code session.
pub struct SessionStartHandler;

impl SessionStartHandler {
    /// Creates a new handler.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for SessionStartHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HookHandler for SessionStartHandler {
    fn event_type(&self) -> &'static str {
        "SessionStart"
    }

    fn handle(&self, _input: &serde_json::Value) -> Result<serde_json::Value> {
        // TODO: Implement context injection
        todo!("SessionStartHandler::handle not yet implemented")
    }
}
