//! MCP server implementation.
//!
//! Provides Model Context Protocol server for AI agent interoperability.

mod prompts;
mod resources;
mod server;
mod tools;

pub use prompts::PromptRegistry;
pub use resources::ResourceHandler;
pub use server::McpServer;
pub use tools::ToolRegistry;
