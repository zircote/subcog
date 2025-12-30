//! Post tool use hook handler.

use super::HookHandler;
use crate::Result;

/// Handles `PostToolUse` hook events.
///
/// Surfaces related memories after tool usage.
pub struct PostToolUseHandler;

impl PostToolUseHandler {
    /// Creates a new handler.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PostToolUseHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HookHandler for PostToolUseHandler {
    fn event_type(&self) -> &'static str {
        "PostToolUse"
    }

    fn handle(&self, _input: &serde_json::Value) -> Result<serde_json::Value> {
        // TODO: Implement memory surfacing
        todo!("PostToolUseHandler::handle not yet implemented")
    }
}
