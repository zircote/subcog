//! MCP pre-defined prompts.

/// Registry of pre-defined prompts.
pub struct PromptRegistry {
    // TODO: Add prompt handlers
}

impl PromptRegistry {
    /// Creates a new prompt registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {}
    }
}

impl Default for PromptRegistry {
    fn default() -> Self {
        Self::new()
    }
}
