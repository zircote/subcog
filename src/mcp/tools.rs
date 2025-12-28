//! MCP tool implementations.

/// Registry of MCP tools.
pub struct ToolRegistry {
    // TODO: Add tool handlers
}

impl ToolRegistry {
    /// Creates a new tool registry with all subcog tools.
    #[must_use]
    pub const fn new() -> Self {
        Self {}
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
