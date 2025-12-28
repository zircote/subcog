//! MCP server setup and lifecycle.

use crate::Result;

/// MCP server for subcog.
pub struct McpServer {
    // TODO: Add rmcp server instance
}

impl McpServer {
    /// Creates a new MCP server.
    #[must_use]
    pub const fn new() -> Self {
        Self {}
    }

    /// Starts the MCP server.
    ///
    /// # Errors
    ///
    /// Returns an error if the server fails to start.
    pub fn start(&self) -> Result<()> {
        // TODO: Implement MCP server start
        todo!("McpServer::start not yet implemented")
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}
