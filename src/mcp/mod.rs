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

// Allow unused_self for methods kept for API consistency or future use.
#![allow(clippy::unused_self)]
// Allow unnecessary wraps for methods that return Result for API consistency.
#![allow(clippy::unnecessary_wraps)]
// Allow ok_or with function calls - the error path is uncommon.
#![allow(clippy::or_fun_call)]
// Allow format_push_string - we prefer readability over micro-optimization here.
#![allow(clippy::format_push_string)]
// Allow option_if_let_else for clearer match statements.
#![allow(clippy::option_if_let_else)]
// Allow match_same_arms for explicit enum handling with default fallback.
#![allow(clippy::match_same_arms)]

mod prompts;
mod resources;
mod server;
mod tools;

pub use prompts::{PromptArgument, PromptContent, PromptDefinition, PromptMessage, PromptRegistry};
pub use resources::{HelpCategory, ResourceContent, ResourceDefinition, ResourceHandler};
pub use server::{McpServer, Transport};
pub use tools::{ToolContent, ToolDefinition, ToolRegistry, ToolResult};
