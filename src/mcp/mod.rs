//! MCP server implementation.
//!
//! Provides Model Context Protocol server for AI agent interoperability.
//!
//! ## Features
//!
//! - **Tools**: `subcog_capture`, `subcog_recall`, `subcog_status`, `subcog_namespaces`
//! - **Resources**: Help documentation via `subcog://help/{category}`
//! - **Prompts**: Tutorial, capture assistant, decision documentation
//!
//! ## Usage
//!
//! ### Stdio Transport (Claude Desktop)
//!
//! ```bash
//! subcog serve
//! ```
//!
//! ### Claude Desktop Configuration
//!
//! ```json
//! {
//!   "mcpServers": {
//!     "subcog": {
//!       "command": "subcog",
//!       "args": ["serve"]
//!     }
//!   }
//! }
//! ```

mod prompts;
mod resources;
mod server;
mod tools;

pub use prompts::{PromptArgument, PromptContent, PromptDefinition, PromptMessage, PromptRegistry};
pub use resources::{HelpCategory, ResourceContent, ResourceDefinition, ResourceHandler};
pub use server::{McpServer, Transport};
pub use tools::{ToolContent, ToolDefinition, ToolRegistry, ToolResult};
