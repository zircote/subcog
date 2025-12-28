//! MCP resource handlers.

/// Handler for MCP resources (URN scheme).
pub struct ResourceHandler {
    // TODO: Add resource handlers
}

impl ResourceHandler {
    /// Creates a new resource handler.
    #[must_use]
    pub const fn new() -> Self {
        Self {}
    }
}

impl Default for ResourceHandler {
    fn default() -> Self {
        Self::new()
    }
}
