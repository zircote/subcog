//! Context builder service.
//!
//! Builds context for Claude Code hooks.

use crate::Result;

/// Service for building context for AI assistants.
pub struct ContextBuilderService {
    // TODO: Add recall service
}

impl ContextBuilderService {
    /// Creates a new context builder service.
    #[must_use]
    pub const fn new() -> Self {
        Self {}
    }

    /// Builds context for the current session.
    ///
    /// # Errors
    ///
    /// Returns an error if context building fails.
    pub fn build_context(&self, _max_tokens: usize) -> Result<String> {
        // TODO: Implement context building
        todo!("ContextBuilderService::build_context not yet implemented")
    }
}

impl Default for ContextBuilderService {
    fn default() -> Self {
        Self::new()
    }
}
