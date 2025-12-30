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

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;
use subcog::config::SubcogConfig;
use subcog::hooks::{
    HookHandler, PostToolUseHandler, PreCompactHandler, SessionStartHandler, StopHandler,
    UserPromptHandler,
};
use subcog::storage::index::SqliteBackend;
use subcog::mcp::{McpServer, Transport};
use subcog::{
    CaptureRequest, CaptureService, Domain, Namespace, RecallService, SearchFilter, SearchMode,
    SyncService,
};

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
    },

    /// Show status.
    Status,

    /// Sync with remote.
    Sync {
        /// Push changes to remote.
        #[arg(long)]
        push: bool,

        /// Fetch changes from remote.
        #[arg(long)]
        fetch: bool,
    },

    /// Run consolidation.
    Consolidate,

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
}

/// Hook events.
#[derive(Subcommand)]
enum HookEvent {
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

/// Main entry point.
fn main() -> ExitCode {
    let cli = Cli::parse();

    // Initialize tracing if verbose
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("subcog=debug")
            .init();
    }

    let result = run_command(cli);

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        },
    }
}

/// Runs the selected command.
fn run_command(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config(cli.config.as_deref())?;

    match cli.command {
        Commands::Capture {
            content,
            namespace,
            tags,
            source,
        } => cmd_capture(content, namespace, tags, source),

        Commands::Recall {
            query,
            mode,
            namespace,
            limit,
        } => cmd_recall(query, mode, namespace, limit),

        Commands::Status => cmd_status(),

        Commands::Sync { push, fetch } => cmd_sync(push, fetch),

        Commands::Consolidate => cmd_consolidate(),

        Commands::Config { show, set } => cmd_config(config, show, set),

        Commands::Serve { transport, port } => cmd_serve(transport, port),

        Commands::Hook { event } => cmd_hook(event),
    }
}

/// Loads configuration.
fn load_config(_path: Option<&str>) -> Result<SubcogConfig, Box<dyn std::error::Error>> {
    // For now, use default config
    // TODO: Load from file if path provided
    Ok(SubcogConfig::default())
}

/// Parses namespace string.
fn parse_namespace(s: &str) -> Namespace {
    match s.to_lowercase().as_str() {
        "decisions" => Namespace::Decisions,
        "patterns" => Namespace::Patterns,
        "learnings" => Namespace::Learnings,
        "blockers" => Namespace::Blockers,
        "progress" => Namespace::Progress,
        "context" => Namespace::Context,
        "tech-debt" | "techdebt" => Namespace::TechDebt,
        "apis" => Namespace::Apis,
        "config" => Namespace::Config,
        "security" => Namespace::Security,
        "testing" => Namespace::Testing,
        _ => Namespace::Decisions,
    }
}

/// Parses search mode string.
fn parse_search_mode(s: &str) -> SearchMode {
    match s.to_lowercase().as_str() {
        "text" => SearchMode::Text,
        "vector" => SearchMode::Vector,
        "hybrid" => SearchMode::Hybrid,
        _ => SearchMode::Hybrid,
    }
}

/// Capture command.
fn cmd_capture(
    content: String,
    namespace: String,
    tags: Option<String>,
    source: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let service = CaptureService::default();

    let tag_list = tags
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    let request = CaptureRequest {
        content,
        namespace: parse_namespace(&namespace),
        domain: Domain::default(),
        tags: tag_list,
        source,
        skip_security_check: false,
    };

    let result = service.capture(request)?;

    println!("Memory captured:");
    println!("  ID: {}", result.memory_id.as_str());
    println!("  URN: {}", result.urn);
    if result.content_modified {
        println!("  Note: Content was redacted for security");
    }

    Ok(())
}

/// Get the data directory for subcog storage.
fn get_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SUBCOG_DATA_DIR") {
        PathBuf::from(dir)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".subcog")
    } else if let Ok(home) = std::env::var("USERPROFILE") {
        // Windows fallback
        PathBuf::from(home).join(".subcog")
    } else {
        PathBuf::from(".subcog")
    }
}

/// Recall command.
fn cmd_recall(
    query: String,
    mode: String,
    namespace: Option<String>,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    // Set up SQLite index backend
    let data_dir = get_data_dir();
    std::fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join("index.db");

    let index = SqliteBackend::new(&db_path)?;
    let service = RecallService::with_index(index);

    let mut filter = SearchFilter::new();
    if let Some(ns) = namespace {
        filter = filter.with_namespace(parse_namespace(&ns));
    }

    let result = service.search(&query, parse_search_mode(&mode), &filter, limit);

    match result {
        Ok(search_result) => {
            println!("Found {} memories:", search_result.total_count);
            println!();

            for hit in &search_result.memories {
                println!(
                    "  [{:.2}] {} ({})",
                    hit.score,
                    hit.memory.id.as_str(),
                    hit.memory.namespace
                );
                // Truncate content for display
                let content = if hit.memory.content.len() > 100 {
                    format!("{}...", &hit.memory.content[..100])
                } else {
                    hit.memory.content.clone()
                };
                println!("       {content}");
                println!();
            }

            println!("Search completed in {}ms", search_result.execution_time_ms);
        },
        Err(e) => {
            eprintln!("Search failed: {e}");
            eprintln!("Note: Make sure a storage backend is configured");
        },
    }

    Ok(())
}

/// Status command.
fn cmd_status() -> Result<(), Box<dyn std::error::Error>> {
    println!("Subcog Status");
    println!("=============");
    println!();
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    println!("Storage: Not configured");
    println!("Sync: Not configured");
    println!();
    println!("Use 'subcog config --show' to view configuration");

    Ok(())
}

/// Sync command.
fn cmd_sync(push: bool, fetch: bool) -> Result<(), Box<dyn std::error::Error>> {
    let service = SyncService::default();

    if push && fetch {
        // Full sync
        match service.sync() {
            Ok(stats) => {
                println!("Sync completed: {}", stats.summary());
            },
            Err(e) => {
                eprintln!("Sync failed: {e}");
            },
        }
    } else if push {
        match service.push() {
            Ok(stats) => {
                println!("Push completed: {} memories pushed", stats.pushed);
            },
            Err(e) => {
                eprintln!("Push failed: {e}");
            },
        }
    } else if fetch {
        match service.fetch() {
            Ok(stats) => {
                println!("Fetch completed: {} memories pulled", stats.pulled);
            },
            Err(e) => {
                eprintln!("Fetch failed: {e}");
            },
        }
    } else {
        // Default to full sync
        match service.sync() {
            Ok(stats) => {
                println!("Sync completed: {}", stats.summary());
            },
            Err(e) => {
                eprintln!("Sync failed: {e}");
            },
        }
    }

    Ok(())
}

/// Consolidate command.
fn cmd_consolidate() -> Result<(), Box<dyn std::error::Error>> {
    println!("Consolidation requires a configured storage backend.");
    println!("Configure storage in subcog.toml or use environment variables.");

    Ok(())
}

/// Config command.
fn cmd_config(
    config: SubcogConfig,
    show: bool,
    _set: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if show {
        println!("Current Configuration");
        println!("=====================");
        println!();
        println!("Repository Path: {}", config.repo_path.display());
        println!("Data Directory: {}", config.data_dir.display());
        println!("Max Results: {}", config.max_results);
        println!("Default Search Mode: {:?}", config.default_search_mode);
        println!();
        println!("Feature Flags:");
        println!("  Secrets Filter: {}", config.features.secrets_filter);
        println!("  PII Filter: {}", config.features.pii_filter);
        println!("  Multi-Domain: {}", config.features.multi_domain);
        println!("  Audit Log: {}", config.features.audit_log);
        println!("  LLM Features: {}", config.features.llm_features);
        println!("  Auto-Capture: {}", config.features.auto_capture);
        println!("  Consolidation: {}", config.features.consolidation);
    } else {
        println!("Use --show to display configuration");
        println!("Use --set KEY=VALUE to set a value");
    }

    Ok(())
}

/// Serve command.
fn cmd_serve(transport: String, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let transport_type = match transport.as_str() {
        "http" => Transport::Http,
        _ => Transport::Stdio,
    };

    let server = McpServer::new()
        .with_transport(transport_type)
        .with_port(port);

    server.start().map_err(|e| e.to_string())?;

    Ok(())
}

/// Hook command.
fn cmd_hook(event: HookEvent) -> Result<(), Box<dyn std::error::Error>> {
    // Read input from stdin as a string
    let input = read_hook_input()?;

    let response = match event {
        HookEvent::SessionStart => {
            let handler = SessionStartHandler::new();
            handler.handle(&input)?
        },
        HookEvent::UserPromptSubmit => {
            let handler = UserPromptHandler::new();
            handler.handle(&input)?
        },
        HookEvent::PostToolUse => {
            let handler = PostToolUseHandler::new();
            handler.handle(&input)?
        },
        HookEvent::PreCompact => {
            let handler = PreCompactHandler::new();
            handler.handle(&input)?
        },
        HookEvent::Stop => {
            let handler = StopHandler::new();
            handler.handle(&input)?
        },
    };

    // Output response (already JSON string)
    println!("{response}");

    Ok(())
}

/// Reads hook input from stdin as a string.
fn read_hook_input() -> Result<String, Box<dyn std::error::Error>> {
    use std::io::{self, Read};

    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    if input.trim().is_empty() {
        Ok("{}".to_string())
    } else {
        Ok(input)
    }
}
