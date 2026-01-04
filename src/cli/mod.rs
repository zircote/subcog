//! CLI command implementations.
//!
//! This module provides the command-line interface for Subcog. Each submodule
//! implements a specific CLI command.
//!
//! # Commands
//!
//! | Command | Description |
//! |---------|-------------|
//! | `capture` | Capture a memory to persistent storage |
//! | `recall` | Search for memories using hybrid (vector + text) search |
//! | `status` | Show memory system status and statistics |
//! | `sync` | Synchronize memories with git remote |
//! | `consolidate` | Consolidate related memories |
//! | `serve` | Run as MCP server (stdio or HTTP) |
//! | `hook` | Claude Code hook handlers |
//! | `config` | Configuration management |
//! | `prompt` | Prompt template management |
//! | `namespaces` | List available namespaces |
//!
//! # Example Usage
//!
//! ```bash
//! # Capture a decision
//! subcog capture --namespace decisions "Use PostgreSQL for primary storage"
//!
//! # Search memories
//! subcog recall "database storage"
//!
//! # Run as MCP server
//! subcog serve
//!
//! # Save a prompt template
//! subcog prompt save my-prompt --content "Review {{file}} for {{issue}}"
//! ```
//!
//! # LLM Client Factory
//!
//! The `llm_factory` submodule provides builder functions for creating LLM clients
//! from configuration. These are used by hooks and other components that need
//! LLM capabilities.

mod capture;
mod config;
mod consolidate;
pub mod gc;
mod hook;
mod llm_factory;
mod migrate;
mod namespaces;
mod prompt;
mod recall;
mod serve;
mod status;
mod sync;

pub use capture::CaptureCommand;
pub use config::ConfigCommand;
pub use consolidate::ConsolidateCommand;
pub use hook::HookCommand;
pub use llm_factory::{
    build_anthropic_client, build_hook_llm_provider, build_http_config, build_lmstudio_client,
    build_ollama_client, build_openai_client, build_resilience_config,
};
pub use migrate::MigrateCommand;
pub use namespaces::{NamespaceInfo, NamespacesOutputFormat, cmd_namespaces, get_all_namespaces};
pub use prompt::{
    OutputFormat, PromptCommand, SavePromptArgs, cmd_prompt_delete, cmd_prompt_export,
    cmd_prompt_get, cmd_prompt_import, cmd_prompt_list, cmd_prompt_run, cmd_prompt_save,
    cmd_prompt_save_with_args, cmd_prompt_share,
};
pub use recall::RecallCommand;
pub use serve::ServeCommand;
pub use status::StatusCommand;
pub use sync::SyncCommand;
