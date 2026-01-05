//! Command handlers module.
//!
//! This module organizes the CLI command implementations into separate files:
//! - `core.rs`: Core commands (capture, recall, status, sync, consolidate, reindex)
//! - `config.rs`: Configuration display command
//! - `enrich.rs`: LLM-powered tag enrichment command
//! - `hook.rs`: Claude Code hook event handlers
//! - `migrate.rs`: Migration commands (embeddings)
//! - `prompt.rs`: Prompt template management

mod config;
mod core;
mod enrich;
mod hook;
mod migrate;
mod prompt;

use std::path::PathBuf;

use clap::Subcommand;

// Re-export command functions
pub use config::cmd_config;
pub use core::{cmd_capture, cmd_consolidate, cmd_recall, cmd_reindex, cmd_status, cmd_sync};
pub use enrich::cmd_enrich;
pub use hook::cmd_hook;
pub use migrate::cmd_migrate_embeddings;
pub use prompt::cmd_prompt;

/// Migrate subcommands.
#[derive(Subcommand)]
pub enum MigrateAction {
    /// Generate embeddings for memories that don't have them.
    Embeddings {
        /// Path to the git repository (default: current directory).
        #[arg(short, long)]
        repo: Option<PathBuf>,

        /// Show what would be migrated without making changes.
        #[arg(long)]
        dry_run: bool,

        /// Re-generate embeddings for all memories, even those that already have them.
        #[arg(long)]
        force: bool,
    },
}

/// Hook events.
#[derive(Subcommand)]
pub enum HookEvent {
    /// Session start hook.
    SessionStart,
    /// User prompt submit hook.
    UserPromptSubmit,
    /// Post tool use hook.
    PostToolUse,
    /// Pre-compact hook.
    PreCompact,
    /// Stop hook.
    Stop,
}

impl HookEvent {
    /// Returns the hook event as a lowercase hyphenated string.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::SessionStart => "session-start",
            Self::UserPromptSubmit => "user-prompt-submit",
            Self::PostToolUse => "post-tool-use",
            Self::PreCompact => "pre-compact",
            Self::Stop => "stop",
        }
    }
}

/// Prompt subcommands.
#[derive(Subcommand)]
pub enum PromptAction {
    /// Save a prompt template.
    Save {
        /// Prompt name (kebab-case).
        #[arg(short, long)]
        name: String,

        /// Prompt content with {{variable}} placeholders.
        content: Option<String>,

        /// Description of the prompt.
        #[arg(short, long)]
        description: Option<String>,

        /// Tags for the prompt (comma-separated).
        #[arg(short, long)]
        tags: Option<String>,

        /// Domain scope: project, user, or org.
        #[arg(long, default_value = "project")]
        domain: Option<String>,

        /// Path to file containing prompt.
        #[arg(long)]
        from_file: Option<PathBuf>,

        /// Read prompt from stdin.
        #[arg(long)]
        from_stdin: bool,

        /// Skip LLM-powered metadata enrichment.
        #[arg(long)]
        no_enrich: bool,

        /// Show enriched template without saving.
        #[arg(long)]
        dry_run: bool,
    },

    /// List saved prompts.
    List {
        /// Filter by domain scope.
        #[arg(long)]
        domain: Option<String>,

        /// Filter by tags (comma-separated).
        #[arg(short, long)]
        tags: Option<String>,

        /// Filter by name pattern (glob).
        #[arg(short, long)]
        name: Option<String>,

        /// Output format: table or json.
        #[arg(short, long, default_value = "table")]
        format: Option<String>,

        /// Maximum number of results.
        #[arg(short, long)]
        limit: Option<usize>,
    },

    /// Get a prompt by name.
    Get {
        /// Prompt name.
        name: String,

        /// Domain scope to search.
        #[arg(long)]
        domain: Option<String>,

        /// Output format: template, json, markdown, or yaml.
        #[arg(short, long, default_value = "template")]
        format: Option<String>,
    },

    /// Run a prompt with variable substitution.
    Run {
        /// Prompt name.
        name: String,

        /// Variable values as KEY=VALUE.
        #[arg(short = 'V', long = "var")]
        variables: Vec<String>,

        /// Domain scope to search.
        #[arg(long)]
        domain: Option<String>,

        /// Prompt for missing variables interactively.
        #[arg(short, long)]
        interactive: bool,
    },

    /// Delete a prompt.
    Delete {
        /// Prompt name.
        name: String,

        /// Domain scope (required).
        #[arg(long)]
        domain: String,

        /// Skip confirmation.
        #[arg(short, long)]
        force: bool,
    },

    /// Export a prompt to a file.
    Export {
        /// Prompt name.
        name: String,

        /// Output file path.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Export format: markdown, yaml, or json.
        #[arg(short, long)]
        format: Option<String>,

        /// Domain scope to search.
        #[arg(long)]
        domain: Option<String>,
    },

    /// Import a prompt from a file or URL.
    Import {
        /// Source file path or URL.
        source: String,

        /// Target domain scope: project, user, or org.
        #[arg(short, long, default_value = "project")]
        domain: String,

        /// Override the prompt name.
        #[arg(short, long)]
        name: Option<String>,

        /// Skip validation.
        #[arg(long)]
        no_validate: bool,
    },

    /// Share a prompt (export with full metadata).
    Share {
        /// Prompt name.
        name: String,

        /// Output file path.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Export format: yaml (default), json, or markdown.
        #[arg(short, long, default_value = "yaml")]
        format: String,

        /// Domain scope to search.
        #[arg(long)]
        domain: Option<String>,

        /// Include usage statistics.
        #[arg(long)]
        include_stats: bool,
    },
}
