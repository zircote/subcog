//! User prompt submit hook handler.

use super::HookHandler;
use crate::Result;

/// Handles `UserPromptSubmit` hook events.
///
/// Detects signals for memory capture in user prompts.
pub struct UserPromptHandler;

impl UserPromptHandler {
    /// Creates a new handler.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for UserPromptHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HookHandler for UserPromptHandler {
    fn event_type(&self) -> &'static str {
        "UserPromptSubmit"
    }

    fn handle(&self, _input: &serde_json::Value) -> Result<serde_json::Value> {
        // TODO: Implement signal detection
        todo!("UserPromptHandler::handle not yet implemented")
    }
}
