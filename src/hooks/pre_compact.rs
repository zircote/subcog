//! Pre-compact hook handler.

use super::HookHandler;
use crate::Result;

/// Handles `PreCompact` hook events.
///
/// Auto-captures memories before context compaction.
pub struct PreCompactHandler;

impl PreCompactHandler {
    /// Creates a new handler.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PreCompactHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HookHandler for PreCompactHandler {
    fn event_type(&self) -> &'static str {
        "PreCompact"
    }

    fn handle(&self, _input: &serde_json::Value) -> Result<serde_json::Value> {
        // TODO: Implement auto-capture
        todo!("PreCompactHandler::handle not yet implemented")
    }
}
