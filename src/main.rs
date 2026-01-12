//! Binary entry point for subcog.
//!
//! This binary provides the CLI interface for the subcog memory system.

#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
// Allow print_stderr in main binary for CLI output
#![allow(clippy::print_stderr)]
#![allow(clippy::print_stdout)]
// Allow match_same_arms for explicit command handling
#![allow(clippy::match_same_arms)]
// Allow unnecessary_wraps for consistent command function signatures
#![allow(clippy::unnecessary_wraps)]
// Allow needless_pass_by_value for command functions
#![allow(clippy::needless_pass_by_value)]
// Allow option_if_let_else for environment variable fallback chains
#![allow(clippy::option_if_let_else)]
// Allow multiple crate versions from transitive dependencies
#![allow(clippy::multiple_crate_versions)]

mod commands;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;
use subcog::config::SubcogConfig;
use subcog::mcp::{McpServer, Transport};
use subcog::observability::{
    self, InitOptions, RequestContext, enter_request_context, scope_request_context,
};
use subcog::security::AuditConfig;
use tracing::info_span;

use commands::{HookEvent, MigrateAction, PromptAction};

/// Subcog - A persistent memory system for AI coding assistants.
#[derive(Parser)]
#[command(name = "subcog")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Enable verbose output.
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Path to configuration file.
    #[arg(short, long, global = true)]
    config: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

/// Available commands.
#[derive(Subcommand)]
enum Commands {
    /// Capture a memory.
    Capture {
        /// The content to capture.
        content: String,

        /// Namespace for the memory.
        #[arg(short, long, default_value = "decisions")]
        namespace: String,

        /// Tags for the memory (comma-separated).
        #[arg(short, long)]
        tags: Option<String>,

        /// Source file or context.
        #[arg(short, long)]
        source: Option<String>,
    },

    /// Search for memories.
    Recall {
        /// The search query.
        query: String,

        /// Search mode: text, vector, or hybrid.
        #[arg(short, long, default_value = "hybrid")]
        mode: String,

        /// Filter by namespace.
        #[arg(short, long)]
        namespace: Option<String>,

        /// Maximum number of results.
        #[arg(short, long, default_value = "10")]
        limit: usize,

        /// Display raw (un-normalized) scores instead of normalized scores.
        #[arg(long)]
        raw: bool,

        /// Include tombstoned memories in results.
        #[arg(long)]
        include_tombstoned: bool,
    },

    /// Show status.
    Status,

    /// Run consolidation.
    Consolidate {
        /// Filter by namespace (can be specified multiple times).
        #[arg(short, long)]
        namespace: Vec<String>,

        /// Time window in days for memories to consolidate.
        #[arg(short, long)]
        days: Option<u32>,

        /// Show what would be consolidated without making changes.
        #[arg(long)]
        dry_run: bool,

        /// Minimum number of memories required to form a group.
        #[arg(long)]
        min_memories: Option<usize>,

        /// Similarity threshold (0.0-1.0) for grouping related memories.
        #[arg(long)]
        similarity: Option<f32>,
    },

    /// Rebuild search index from stored memories.
    Reindex {
        /// Path to the git repository (default: current directory).
        #[arg(short, long)]
        repo: Option<PathBuf>,
    },

    /// Enrich memories with LLM-generated tags.
    Enrich {
        /// Enrich all memories (those without tags).
        #[arg(long)]
        all: bool,

        /// Update all memories, even those with existing tags.
        #[arg(long)]
        update_all: bool,

        /// Enrich a specific memory by ID.
        #[arg(long)]
        id: Option<String>,

        /// Show what would be changed without applying.
        #[arg(long)]
        dry_run: bool,
    },

    /// Manage configuration.
    Config {
        /// Show current configuration.
        #[arg(long)]
        show: bool,

        /// Set a configuration value.
        #[arg(long)]
        set: Option<String>,
    },

    /// Start MCP server.
    Serve {
        /// Transport type: stdio or http.
        #[arg(short, long, default_value = "stdio")]
        transport: String,

        /// Port for HTTP transport.
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },

    /// Handle Claude Code hooks.
    Hook {
        /// Hook event type.
        #[command(subcommand)]
        event: HookEvent,
    },

    /// Manage prompt templates.
    Prompt {
        /// Prompt subcommand.
        #[command(subcommand)]
        action: PromptAction,
    },

    /// List available memory namespaces.
    Namespaces {
        /// Output format: table, json, or yaml.
        #[arg(short, long, default_value = "table")]
        format: String,

        /// Show signal words for each namespace.
        #[arg(short, long)]
        verbose: bool,
    },

    /// Migrate memories to new features.
    Migrate {
        /// Migration subcommand.
        #[command(subcommand)]
        action: MigrateAction,
    },

    /// Generate shell completion scripts.
    Completions {
        /// Shell type: bash, zsh, fish, powershell, or elvish.
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Garbage collect tombstoned memories.
    Gc {
        /// Dry-run mode (show what would be deleted).
        #[arg(long)]
        dry_run: bool,

        /// Purge tombstoned memories older than threshold.
        #[arg(long)]
        purge: bool,

        /// Age threshold in days for purging.
        #[arg(long, default_value = "30")]
        older_than: u64,
    },
}

/// Main entry point.
#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let config = match load_config(cli.config.as_deref()) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load configuration: {e}");
            return ExitCode::FAILURE;
        },
    };

    let expose_metrics = matches!(cli.command, Commands::Serve { .. });
    let mut observability_handle = match observability::init_from_config(
        &config.observability,
        InitOptions {
            verbose: cli.verbose,
            metrics_expose: expose_metrics,
        },
    ) {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("Failed to initialize observability: {e}");
            return ExitCode::FAILURE;
        },
    };

    let result = Box::pin(run_command(cli, config)).await;

    // Explicitly shutdown observability before async runtime exits
    observability_handle.shutdown();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        },
    }
}

/// Runs the selected command.
async fn run_command(cli: Cli, config: SubcogConfig) -> Result<(), Box<dyn std::error::Error>> {
    if config.features.audit_log {
        let audit_path = config.data_dir.join("audit.log");
        let audit_config = AuditConfig::new().with_log_path(audit_path);
        subcog::security::init_global(audit_config)?;
    }

    let command_name = match &cli.command {
        Commands::Capture { .. } => "capture",
        Commands::Recall { .. } => "recall",
        Commands::Status => "status",
        Commands::Consolidate { .. } => "consolidate",
        Commands::Reindex { .. } => "reindex",
        Commands::Enrich { .. } => "enrich",
        Commands::Config { .. } => "config",
        Commands::Serve { .. } => "serve",
        Commands::Hook { .. } => "hook",
        Commands::Prompt { .. } => "prompt",
        Commands::Namespaces { .. } => "namespaces",
        Commands::Migrate { .. } => "migrate",
        Commands::Completions { .. } => "completions",
        Commands::Gc { .. } => "gc",
    };

    let request_context = RequestContext::new();
    let blocking_context = request_context.clone();
    let request_id = request_context.request_id().to_string();

    Box::pin(scope_request_context(request_context, async move {
        let span = info_span!(
            "subcog.cli.command",
            request_id = %request_id,
            component = "cli",
            operation = command_name
        );
        let _span_guard = span.enter();
        let dispatch_span = span.clone();
        dispatch_command(cli, config, dispatch_span, blocking_context).await
    }))
    .await
}

#[allow(clippy::too_many_lines)]
async fn dispatch_command(
    cli: Cli,
    config: SubcogConfig,
    span: tracing::Span,
    blocking_context: RequestContext,
) -> Result<(), Box<dyn std::error::Error>> {
    macro_rules! run_blocking_cmd {
        ($body:expr) => {
            run_blocking(span.clone(), blocking_context.clone(), $body).await
        };
    }

    match cli.command {
        Commands::Capture {
            content,
            namespace,
            tags,
            source,
        } => {
            let config = config.clone();
            run_blocking_cmd!(move || {
                commands::cmd_capture(&config, content, namespace, tags, source)
                    .map_err(|e| e.to_string())
            })
        },
        Commands::Recall {
            query,
            mode,
            namespace,
            limit,
            raw,
            include_tombstoned,
        } => run_blocking_cmd!(move || {
            commands::cmd_recall(query, mode, namespace, limit, raw, include_tombstoned)
                .map_err(|e| e.to_string())
        }),
        Commands::Status => {
            let config = config.clone();
            run_blocking_cmd!(move || commands::cmd_status(&config).map_err(|e| e.to_string()))
        },
        Commands::Consolidate {
            namespace,
            days,
            dry_run,
            min_memories,
            similarity,
        } => {
            let config = config.clone();
            let namespace = namespace.clone();
            run_blocking_cmd!(move || {
                commands::cmd_consolidate(
                    &config,
                    namespace,
                    days,
                    dry_run,
                    min_memories,
                    similarity,
                )
                .map_err(|e| e.to_string())
            })
        },
        Commands::Reindex { repo } => {
            run_blocking_cmd!(move || commands::cmd_reindex(repo).map_err(|e| e.to_string()))
        },
        Commands::Enrich {
            all,
            update_all,
            id,
            dry_run,
        } => {
            let config = config.clone();
            run_blocking_cmd!(move || {
                commands::cmd_enrich(&config, all, update_all, id, dry_run)
                    .map_err(|e| e.to_string())
            })
        },
        Commands::Config { show, set } => run_blocking_cmd!(move || {
            commands::cmd_config(config, show, set).map_err(|e| e.to_string())
        }),
        Commands::Serve { transport, port } => cmd_serve(transport, port).await,
        Commands::Hook { event } => {
            let config = config.clone();
            run_blocking_cmd!(move || commands::cmd_hook(event, &config).map_err(|e| e.to_string()))
        },
        Commands::Prompt { action } => {
            run_blocking_cmd!(move || { commands::cmd_prompt(action).map_err(|e| e.to_string()) })
        },
        Commands::Namespaces { format, verbose } => run_blocking_cmd!(move || {
            use std::str::FromStr;
            use subcog::cli::{NamespacesOutputFormat, cmd_namespaces};
            let format = NamespacesOutputFormat::from_str(&format).unwrap_or_default();
            cmd_namespaces(format, verbose).map_err(|e| e.to_string())
        }),
        Commands::Migrate { action } => run_blocking_cmd!(move || {
            match action {
                MigrateAction::Embeddings {
                    repo,
                    dry_run,
                    force,
                } => commands::cmd_migrate_embeddings(repo, dry_run, force),
            }
            .map_err(|e| e.to_string())
        }),
        Commands::Completions { shell } => run_blocking_cmd!(move || {
            use clap::CommandFactory;
            use clap_complete::generate;
            use std::io;
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "subcog", &mut io::stdout());
            Ok(())
        }),
        Commands::Gc {
            dry_run,
            purge,
            older_than,
        } => run_blocking_cmd!(move || {
            subcog::cli::gc::execute(dry_run, purge, older_than).map_err(|e| e.to_string())
        }),
    }
}

async fn run_blocking<F>(
    span: tracing::Span,
    context: RequestContext,
    f: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce() -> Result<(), String> + Send + 'static,
{
    let result = tokio::task::spawn_blocking(move || {
        let _guard = enter_request_context(context);
        span.in_scope(f)
    })
    .await
    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    result.map_err(Into::into)
}

/// Loads configuration following a priority-based resolution order.
///
/// # Configuration Loading Order
///
/// Configuration sources are checked in the following priority order (first wins):
///
/// ```text
/// 1. CLI argument      subcog --config /path/to/config.toml
///    │
///    ├─ Found? ─── Use specified file
///    │
/// 2. Environment       SUBCOG_CONFIG_PATH=/path/to/config.toml
///    │
///    ├─ Found? ─── Use specified file
///    │
/// 3. Default locations (checked by SubcogConfig::load_default)
///    │
///    ├── ~/.config/subcog/config.toml  (user-level)
///    └── Fallback to built-in defaults
/// ```
///
/// # Environment Variables
///
/// | Variable | Description |
/// |----------|-------------|
/// | `SUBCOG_CONFIG_PATH` | Full path to config file (overrides defaults) |
///
/// # File Format
///
/// Configuration files use TOML format. See `SubcogConfig` for schema.
///
/// # Examples
///
/// ```bash
/// # Use specific config file
/// subcog --config ./custom.toml capture "..."
///
/// # Override via environment
/// SUBCOG_CONFIG_PATH=~/.config/subcog/prod.toml subcog recall "..."
///
/// # Use defaults (no flags)
/// subcog status
/// ```
fn load_config(path: Option<&str>) -> Result<SubcogConfig, Box<dyn std::error::Error>> {
    // Priority 1: CLI argument
    if let Some(config_path) = path {
        return SubcogConfig::load_from_file(std::path::Path::new(config_path))
            .map_err(std::convert::Into::into);
    }

    // Priority 2: Environment variable
    if let Ok(config_path) = std::env::var("SUBCOG_CONFIG_PATH")
        && !config_path.trim().is_empty()
    {
        return SubcogConfig::load_from_file(std::path::Path::new(&config_path))
            .map_err(std::convert::Into::into);
    }

    // Priority 3: Default locations
    Ok(SubcogConfig::load_default())
}

/// Serve command.
async fn cmd_serve(transport: String, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    // Set instance label for metrics to prevent MCP from overwriting hook metrics
    observability::set_instance_label("mcp");

    let transport_type = match transport.as_str() {
        "http" => Transport::Http,
        _ => Transport::Stdio,
    };

    let mut server = McpServer::new()
        .with_transport(transport_type)
        .with_port(port);

    #[cfg(feature = "http")]
    if matches!(transport_type, Transport::Http) {
        server = server.with_jwt_from_env().map_err(|e| e.to_string())?;
    }

    server.start().await.map_err(|e| e.to_string())?;

    Ok(())
}
