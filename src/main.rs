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
use subcog::cli::{
    build_anthropic_client, build_hook_llm_provider, build_lmstudio_client, build_ollama_client,
    build_openai_client, build_resilience_config,
};
use subcog::config::SubcogConfig;
use subcog::hooks::{
    AdaptiveContextConfig, HookHandler, PostToolUseHandler, PreCompactHandler, SessionStartHandler,
    StopHandler, UserPromptHandler,
};
use subcog::mcp::{McpServer, Transport};
use subcog::observability::{self, InitOptions, flush_metrics};
use subcog::security::AuditConfig;
use subcog::services::ContextBuilderService;
use subcog::storage::index::SqliteBackend;
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

    /// Rebuild search index from git notes.
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

impl HookEvent {
    /// Returns the hook event as a lowercase hyphenated string.
    const fn as_str(&self) -> &'static str {
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
enum PromptAction {
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
        #[arg(short, long = "var")]
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

    let result = run_command(cli, config);

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
fn run_command(cli: Cli, config: SubcogConfig) -> Result<(), Box<dyn std::error::Error>> {
    if config.features.audit_log {
        let audit_path = config.data_dir.join("audit.log");
        let audit_config = AuditConfig::new().with_log_path(audit_path);
        subcog::security::init_global(audit_config)?;
    }

    match cli.command {
        Commands::Capture {
            content,
            namespace,
            tags,
            source,
        } => cmd_capture(&config, content, namespace, tags, source),

        Commands::Recall {
            query,
            mode,
            namespace,
            limit,
        } => cmd_recall(query, mode, namespace, limit),

        Commands::Status => cmd_status(&config),

        Commands::Sync { push, fetch } => cmd_sync(&config, push, fetch),

        Commands::Consolidate => cmd_consolidate(&config),

        Commands::Reindex { repo } => cmd_reindex(repo),

        Commands::Enrich {
            all,
            update_all,
            id,
            dry_run,
        } => cmd_enrich(&config, all, update_all, id, dry_run),

        Commands::Config { show, set } => cmd_config(config, show, set),

        Commands::Serve { transport, port } => cmd_serve(transport, port),

        Commands::Hook { event } => cmd_hook(event, &config),

        Commands::Prompt { action } => cmd_prompt(action),
    }
}

/// Loads configuration.
fn load_config(path: Option<&str>) -> Result<SubcogConfig, Box<dyn std::error::Error>> {
    // If a path is provided, load from that file
    if let Some(config_path) = path {
        return SubcogConfig::load_from_file(std::path::Path::new(config_path))
            .map_err(std::convert::Into::into);
    }

    // Environment override for config path
    if let Ok(config_path) = std::env::var("SUBCOG_CONFIG_PATH") {
        if !config_path.trim().is_empty() {
            return SubcogConfig::load_from_file(std::path::Path::new(&config_path))
                .map_err(std::convert::Into::into);
        }
    }

    // Otherwise, load from default location
    Ok(SubcogConfig::load_default())
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
    config: &SubcogConfig,
    content: String,
    namespace: String,
    tags: Option<String>,
    source: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get repo path so captures are stored to git notes
    let cwd = std::env::current_dir()?;
    let mut service_config = subcog::config::Config::from(config.clone());
    service_config = service_config.with_repo_path(&cwd);
    let service = CaptureService::new(service_config);

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

/// Recall command.
fn cmd_recall(
    query: String,
    mode: String,
    namespace: Option<String>,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use subcog::services::ServiceContainer;

    // Use domain-scoped index (project-local .subcog/index.db)
    let services = ServiceContainer::from_current_dir()?;
    let service = services.recall()?;

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
fn cmd_status(config: &SubcogConfig) -> Result<(), Box<dyn std::error::Error>> {
    println!("Subcog Status");
    println!("=============");
    println!();
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    println!();

    // Check git repository
    let git_dir = config.repo_path.join(".git");
    let git_status = if git_dir.exists() {
        "Available"
    } else {
        "Not found (.git missing)"
    };
    println!("Git Repository: {git_status}");
    println!("  Path: {}", config.repo_path.display());

    // Check data directory
    let data_status = if config.data_dir.exists() {
        "Configured"
    } else {
        "Will be created on first use"
    };
    println!("Data Directory: {data_status}");
    println!("  Path: {}", config.data_dir.display());

    // Check SQLite index
    let sqlite_path = config.data_dir.join("index.sqlite");
    let sqlite_status = if sqlite_path.exists() {
        "Available"
    } else {
        "Not initialized"
    };
    println!("SQLite Index: {sqlite_status}");

    // Check usearch index
    let usearch_path = config.data_dir.join("vectors.usearch");
    let usearch_status = if usearch_path.exists() {
        "Available"
    } else {
        "Not initialized"
    };
    println!("Vector Index: {usearch_status}");

    // Check git notes
    let notes_status = check_git_notes_status(&config.repo_path);
    println!("Git Notes: {notes_status}");

    println!();
    println!("Use 'subcog config --show' to view full configuration");

    Ok(())
}

/// Check git notes status.
fn check_git_notes_status(repo_path: &std::path::Path) -> &'static str {
    use std::process::Command;

    let result = Command::new("git")
        .args(["notes", "--ref=subcog/memories", "list"])
        .current_dir(repo_path)
        .output();

    match result {
        Ok(output) if output.status.success() => {
            let count = String::from_utf8_lossy(&output.stdout).lines().count();
            if count > 0 {
                "Available (has memories)"
            } else {
                "Available (empty)"
            }
        },
        Ok(_) => "Initialized (no memories yet)",
        Err(_) => "Not available (git error)",
    }
}

/// Sync command.
fn cmd_sync(
    config: &SubcogConfig,
    push: bool,
    fetch: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get current directory as repo path, or use config
    let cwd = std::env::current_dir()?;
    let service_config = subcog::config::Config::from(config.clone()).with_repo_path(&cwd);
    let service = SyncService::new(service_config);

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
fn cmd_consolidate(config: &SubcogConfig) -> Result<(), Box<dyn std::error::Error>> {
    use subcog::config::StorageBackendType;
    use subcog::services::ConsolidationService;
    use subcog::storage::index::SqliteBackend;
    use subcog::storage::persistence::FilesystemBackend;

    let data_dir = &config.data_dir;
    let storage_config = &config.storage.project;

    // Ensure data directory exists
    if !data_dir.exists() {
        std::fs::create_dir_all(data_dir)?;
    }

    println!("Running memory consolidation...");
    println!("Data directory: {}", data_dir.display());
    println!("Storage backend: {:?}", storage_config.backend);
    println!();

    // Run consolidation based on configured backend
    match storage_config.backend {
        StorageBackendType::Sqlite => {
            let db_path = storage_config
                .path
                .as_ref()
                .map_or_else(|| data_dir.join("memories.db"), std::path::PathBuf::from);
            println!("SQLite path: {}", db_path.display());

            let backend = SqliteBackend::new(&db_path)?;
            let mut service = ConsolidationService::new(backend);
            run_consolidation(&mut service)?;
        },
        StorageBackendType::Filesystem | StorageBackendType::GitNotes => {
            let backend = FilesystemBackend::new(data_dir);
            let mut service = ConsolidationService::new(backend);
            run_consolidation(&mut service)?;
        },
        StorageBackendType::PostgreSQL => {
            eprintln!("PostgreSQL consolidation not yet implemented");
            return Err("PostgreSQL consolidation not yet implemented".into());
        },
        StorageBackendType::Redis => {
            eprintln!("Redis consolidation not yet implemented");
            return Err("Redis consolidation not yet implemented".into());
        },
    }

    Ok(())
}

/// Runs consolidation and prints results.
fn run_consolidation<P: subcog::storage::PersistenceBackend>(
    service: &mut subcog::services::ConsolidationService<P>,
) -> Result<(), Box<dyn std::error::Error>> {
    match service.consolidate() {
        Ok(stats) => {
            println!("Consolidation completed:");
            println!("  {}", stats.summary());
            if stats.contradictions > 0 {
                println!(
                    "  Note: {} potential contradiction(s) detected - review recommended",
                    stats.contradictions
                );
            }
            Ok(())
        },
        Err(e) => {
            eprintln!("Consolidation failed: {e}");
            Err(e.into())
        },
    }
}

/// Reindex command.
fn cmd_reindex(repo: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    use subcog::services::ServiceContainer;

    // Use provided repo path or current directory
    let repo_path = repo.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));

    println!("Reindexing memories from git notes...");
    println!("Repository: {}", repo_path.display());
    println!();

    let services = ServiceContainer::for_repo(&repo_path, None)?;
    match services.reindex() {
        Ok(count) => {
            println!("Reindex completed successfully!");
            println!("Memories indexed: {count}");
        },
        Err(e) => {
            eprintln!("Reindex failed: {e}");
            return Err(e.into());
        },
    }

    Ok(())
}

/// Enrich command.
fn cmd_enrich(
    config: &SubcogConfig,
    all: bool,
    update_all: bool,
    id: Option<String>,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use subcog::config::LlmProvider;

    // Get repository path
    let cwd = std::env::current_dir()?;

    // Create the appropriate LLM client based on config
    let llm_config = &config.llm;
    println!(
        "Using LLM provider: {:?}{}",
        llm_config.provider,
        llm_config
            .model
            .as_ref()
            .map_or(String::new(), |m| format!(" (model: {m})"))
    );
    // Build enrichment service with configured provider
    match llm_config.provider {
        LlmProvider::OpenAi => run_enrich_with_client(
            build_openai_client(llm_config),
            llm_config,
            all,
            update_all,
            id,
            dry_run,
            &cwd,
        ),
        LlmProvider::Anthropic => run_enrich_with_client(
            build_anthropic_client(llm_config),
            llm_config,
            all,
            update_all,
            id,
            dry_run,
            &cwd,
        ),
        LlmProvider::Ollama => run_enrich_with_client(
            build_ollama_client(llm_config),
            llm_config,
            all,
            update_all,
            id,
            dry_run,
            &cwd,
        ),
        LlmProvider::LmStudio => run_enrich_with_client(
            build_lmstudio_client(llm_config),
            llm_config,
            all,
            update_all,
            id,
            dry_run,
            &cwd,
        ),
    }
}

// LLM factory functions imported from cli module

fn run_enrich_with_client<P: subcog::llm::LlmProvider>(
    client: P,
    llm_config: &subcog::config::LlmConfig,
    all: bool,
    update_all: bool,
    id: Option<String>,
    dry_run: bool,
    cwd: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let resilience_config = build_resilience_config(llm_config);
    let client = subcog::llm::ResilientLlmProvider::new(client, resilience_config);
    run_enrichment(
        subcog::services::EnrichmentService::new(client, cwd),
        all,
        update_all,
        id,
        dry_run,
        cwd,
    )
}

/// Runs the enrichment operation with the given service.
fn run_enrichment<P: subcog::llm::LlmProvider>(
    service: subcog::services::EnrichmentService<P>,
    all: bool,
    update_all: bool,
    id: Option<String>,
    dry_run: bool,
    cwd: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use subcog::services::ServiceContainer;

    if let Some(memory_id) = id {
        // Enrich a single memory
        println!("Enriching memory: {memory_id}");
        match service.enrich_one(&memory_id, dry_run) {
            Ok(result) => {
                if result.applied {
                    println!("Enriched with tags: {:?}", result.new_tags);
                } else {
                    println!("Would enrich with tags: {:?}", result.new_tags);
                }
            },
            Err(e) => {
                eprintln!("Enrichment failed: {e}");
                return Err(e.into());
            },
        }
    } else if all || update_all {
        // Enrich all memories
        println!("Enriching memories...");
        if update_all {
            println!("Mode: Update all (including those with existing tags)");
        } else {
            println!("Mode: Enrich only (memories without tags)");
        }
        if dry_run {
            println!("Dry run: No changes will be applied");
        }
        println!();

        match service.enrich_all(dry_run, update_all) {
            Ok(stats) => {
                println!();
                println!("{}", stats.summary());

                // Trigger reindex if changes were made
                if !dry_run && (stats.enriched > 0 || stats.updated > 0) {
                    println!();
                    println!("Reindexing to update search index...");
                    let services = ServiceContainer::for_repo(cwd, None)?;
                    match services.reindex() {
                        Ok(count) => println!("Reindexed {count} memories"),
                        Err(e) => eprintln!("Reindex failed: {e}"),
                    }
                }
            },
            Err(e) => {
                eprintln!("Enrichment failed: {e}");
                return Err(e.into());
            },
        }
    } else {
        println!("Usage:");
        println!("  subcog enrich --all          Enrich memories without tags");
        println!("  subcog enrich --update-all   Update all memories (regenerate tags)");
        println!("  subcog enrich --id <ID>      Enrich a specific memory");
        println!();
        println!("Options:");
        println!("  --dry-run    Show what would be changed without applying");
    }

    Ok(())
}

/// Helper to display tracing configuration.
fn display_tracing_config(config: &SubcogConfig) {
    let tracing_enabled = config
        .observability
        .tracing
        .as_ref()
        .and_then(|t| t.enabled)
        .unwrap_or(false);
    println!(
        "  Tracing: {}",
        if tracing_enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    let Some(ref tracing) = config.observability.tracing else {
        return;
    };
    if !tracing_enabled {
        return;
    }
    if let Some(ref otlp) = tracing.otlp {
        if let Some(ref endpoint) = otlp.endpoint {
            println!("    OTLP Endpoint: {endpoint}");
        }
        if let Some(ref protocol) = otlp.protocol {
            println!("    Protocol: {protocol}");
        }
    }
    if let Some(ratio) = tracing.sample_ratio {
        println!("    Sample Ratio: {ratio}");
    }
}

/// Helper to display metrics configuration.
fn display_metrics_config(config: &SubcogConfig) {
    let metrics_enabled = config
        .observability
        .metrics
        .as_ref()
        .and_then(|m| m.enabled)
        .unwrap_or(false);
    println!(
        "  Metrics: {}",
        if metrics_enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    let Some(ref metrics) = config.observability.metrics else {
        return;
    };
    if !metrics_enabled {
        return;
    }
    if let Some(port) = metrics.port {
        println!("    Prometheus Port: {port}");
    }
    if let Some(ref push_gw) = metrics.push_gateway {
        if let Some(ref endpoint) = push_gw.endpoint {
            println!("    Push Gateway: {endpoint}");
        }
    }
}

/// Helper to display logging configuration.
fn display_logging_config(config: &SubcogConfig) {
    let Some(ref logging) = config.observability.logging else {
        return;
    };
    println!("  Logging:");
    if let Some(ref format) = logging.format {
        println!("    Format: {format}");
    }
    if let Some(ref level) = logging.level {
        println!("    Level: {level}");
    }
    if let Some(ref filter) = logging.filter {
        println!("    Filter: {filter}");
    }
}

/// Helper to display storage configuration.
fn display_storage_config(config: &SubcogConfig) {
    use subcog::config::StorageBackendType;

    let storage = &config.storage;

    // Project storage
    print!("  Project: {:?}", storage.project.backend);
    if let Some(ref path) = storage.project.path {
        print!(" (path: {path})");
    }
    if let Some(ref conn) = storage.project.connection_string {
        let display = if conn.len() > 30 {
            format!("{}...", &conn[..30])
        } else {
            conn.clone()
        };
        print!(" (connection: {display})");
    }
    println!();

    // User storage
    print!("  User: {:?}", storage.user.backend);
    match storage.user.backend {
        StorageBackendType::Sqlite => {
            if let Some(ref path) = storage.user.path {
                print!(" (path: {path})");
            } else {
                print!(" (path: ~/.config/subcog/memories.db)");
            }
        },
        StorageBackendType::Filesystem => {
            if let Some(ref path) = storage.user.path {
                print!(" (path: {path})");
            } else {
                print!(" (path: ~/.config/subcog/prompts/)");
            }
        },
        StorageBackendType::PostgreSQL | StorageBackendType::Redis => {
            if let Some(ref conn) = storage.user.connection_string {
                let display = if conn.len() > 30 {
                    format!("{}...", &conn[..30])
                } else {
                    conn.clone()
                };
                print!(" (connection: {display})");
            }
        },
        StorageBackendType::GitNotes => {
            print!(" (project-local git notes)");
        },
    }
    println!();

    // Org storage
    print!("  Org: {:?}", storage.org.backend);
    if let Some(ref conn) = storage.org.connection_string {
        let display = if conn.len() > 30 {
            format!("{}...", &conn[..30])
        } else {
            conn.clone()
        };
        print!(" (connection: {display})");
    }
    println!(" (not yet implemented)");
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

        // Show config file sources
        println!("Config Files Loaded:");
        if config.config_sources.is_empty() {
            println!("  (none - using defaults)");
        } else {
            for source in &config.config_sources {
                println!("  - {}", source.display());
            }
        }
        println!();

        println!("Repository Path: {}", config.repo_path.display());
        println!("Data Directory: {}", config.data_dir.display());
        println!("Max Results: {}", config.max_results);
        println!("Default Search Mode: {:?}", config.default_search_mode);
        println!();

        println!("Observability:");
        display_tracing_config(&config);
        display_metrics_config(&config);
        display_logging_config(&config);
        println!();

        println!("Feature Flags:");
        println!("  Secrets Filter: {}", config.features.secrets_filter);
        println!("  PII Filter: {}", config.features.pii_filter);
        println!("  Multi-Domain: {}", config.features.multi_domain);
        println!("  Audit Log: {}", config.features.audit_log);
        println!("  LLM Features: {}", config.features.llm_features);
        println!("  Auto-Capture: {}", config.features.auto_capture);
        println!("  Consolidation: {}", config.features.consolidation);
        println!();
        println!("LLM Configuration:");
        println!("  Provider: {:?}", config.llm.provider);
        println!(
            "  Model: {}",
            config.llm.model.as_deref().unwrap_or("(default)")
        );
        println!(
            "  Base URL: {}",
            config.llm.base_url.as_deref().unwrap_or("(default)")
        );

        println!("\nSearch Intent:");
        println!("  Enabled: {}", config.search_intent.enabled);
        println!("  Use LLM: {}", config.search_intent.use_llm);
        println!("  LLM Timeout: {}ms", config.search_intent.llm_timeout_ms);
        println!(
            "  Min Confidence: {:.2}",
            config.search_intent.min_confidence
        );
        println!(
            "  Memory Count: {}-{}",
            config.search_intent.base_count, config.search_intent.max_count
        );
        println!("  Max Tokens: {}", config.search_intent.max_tokens);

        // Prompt customization
        let has_prompt_config = config.prompt.identity_addendum.is_some()
            || config.prompt.additional_guidance.is_some()
            || config.prompt.operation_guidance.capture.is_some()
            || config.prompt.operation_guidance.search.is_some()
            || config.prompt.operation_guidance.enrichment.is_some()
            || config.prompt.operation_guidance.consolidation.is_some();

        if has_prompt_config {
            println!("\nPrompt Customization:");
            if let Some(ref addendum) = config.prompt.identity_addendum {
                let preview: String = addendum.chars().take(50).collect();
                println!("  Identity Addendum: {}...", preview.replace('\n', " "));
            }
            if let Some(ref guidance) = config.prompt.additional_guidance {
                let preview: String = guidance.chars().take(50).collect();
                println!("  Additional Guidance: {}...", preview.replace('\n', " "));
            }
            if config.prompt.operation_guidance.capture.is_some() {
                println!("  Capture Guidance: (configured)");
            }
            if config.prompt.operation_guidance.search.is_some() {
                println!("  Search Guidance: (configured)");
            }
            if config.prompt.operation_guidance.enrichment.is_some() {
                println!("  Enrichment Guidance: (configured)");
            }
            if config.prompt.operation_guidance.consolidation.is_some() {
                println!("  Consolidation Guidance: (configured)");
            }
        }

        // Storage info
        println!("\nStorage:");
        display_storage_config(&config);
    } else {
        println!("Use --show to display configuration");
        println!("Use --set KEY=VALUE to set a value");
    }

    Ok(())
}

/// Serve command.
fn cmd_serve(transport: String, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    // Set instance label for metrics to prevent MCP from overwriting hook metrics
    crate::observability::set_instance_label("mcp");

    let transport_type = match transport.as_str() {
        "http" => Transport::Http,
        _ => Transport::Stdio,
    };

    let mut server = McpServer::new()
        .with_transport(transport_type)
        .with_port(port);

    server.start().map_err(|e| e.to_string())?;

    Ok(())
}

/// Hook command.
fn cmd_hook(event: HookEvent, config: &SubcogConfig) -> Result<(), Box<dyn std::error::Error>> {
    // Set instance label for metrics including hook type to prevent metric collision
    // Each hook type gets its own instance (hooks-session-start, hooks-user-prompt-submit, etc.)
    let instance_label = format!("hooks-{}", event.as_str());
    crate::observability::set_instance_label(&instance_label);

    // Read input from stdin as a string
    let input = read_hook_input()?;

    // Try to initialize services for hooks (may fail if no data dir)
    let recall_service = try_init_recall_service();

    // Get repo path so captures are stored to git notes
    let cwd = std::env::current_dir().ok();
    let mut capture_config = subcog::config::Config::from(config.clone());
    if let Some(path) = cwd.as_ref() {
        capture_config = capture_config.with_repo_path(path);
    }
    let capture_service = CaptureService::new(capture_config.clone());
    let sync_service = SyncService::default();

    let response = match event {
        HookEvent::SessionStart => {
            // SessionStart with context builder for memory injection
            let handler = if let Some(recall) = recall_service {
                SessionStartHandler::new()
                    .with_context_builder(ContextBuilderService::with_recall(recall))
            } else {
                SessionStartHandler::new()
            };
            handler.handle(&input)?
        },
        HookEvent::UserPromptSubmit => {
            let context_config =
                AdaptiveContextConfig::from_search_intent_config(&config.search_intent);
            // Create separate capture service for auto-capture in this handler
            let handler_capture_service = CaptureService::new(capture_config);
            let mut handler = UserPromptHandler::new()
                .with_search_intent_config(config.search_intent.clone())
                .with_context_config(context_config)
                .with_capture_service(handler_capture_service)
                .with_auto_capture(config.features.auto_capture);
            if let Some(provider) = build_hook_llm_provider(config) {
                handler = handler.with_llm_provider(provider);
            }
            handler.handle(&input)?
        },
        HookEvent::PostToolUse => {
            // PostToolUse with recall service for memory surfacing
            let handler = if let Some(recall) = recall_service {
                PostToolUseHandler::new().with_recall(recall)
            } else {
                PostToolUseHandler::new()
            };
            handler.handle(&input)?
        },
        HookEvent::PreCompact => {
            // PreCompact with capture service for auto-capture
            let handler = PreCompactHandler::new().with_capture(capture_service);
            handler.handle(&input)?
        },
        HookEvent::Stop => {
            // Stop with sync service for session-end sync
            let handler = StopHandler::new().with_sync(sync_service);
            handler.handle(&input)?
        },
    };

    // Output response (already JSON string)
    println!("{response}");

    // Delay to ensure spawned threads complete metric recording.
    // Note: LLM threads use recv_timeout, so if HTTP timeout < search_intent timeout,
    // the thread will complete before recv_timeout expires. This delay is just a buffer
    // for any remaining metric recording after channel communication.
    std::thread::sleep(std::time::Duration::from_millis(250));

    // Flush metrics to push gateway before exit
    flush_metrics();

    Ok(())
}

/// Tries to initialize a recall service with `SQLite` backend.
fn try_init_recall_service() -> Option<RecallService> {
    let data_dir = directories::BaseDirs::new().map_or_else(
        || std::path::PathBuf::from(".").join(".subcog"),
        |b| b.data_local_dir().join("subcog"),
    );

    // Ensure data directory exists
    if std::fs::create_dir_all(&data_dir).is_err() {
        return None;
    }

    let db_path = data_dir.join("index.db");
    SqliteBackend::new(&db_path)
        .ok()
        .map(RecallService::with_index)
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

/// Prompt command.
fn cmd_prompt(action: PromptAction) -> Result<(), Box<dyn std::error::Error>> {
    use subcog::cli::{
        cmd_prompt_delete, cmd_prompt_export, cmd_prompt_get, cmd_prompt_list, cmd_prompt_run,
        cmd_prompt_save,
    };

    match action {
        PromptAction::Save {
            name,
            content,
            description,
            tags,
            domain,
            from_file,
            from_stdin,
        } => cmd_prompt_save(
            name,
            content,
            description,
            tags,
            domain,
            from_file,
            from_stdin,
        ),

        PromptAction::List {
            domain,
            tags,
            name,
            format,
            limit,
        } => cmd_prompt_list(domain, tags, name, format, limit),

        PromptAction::Get {
            name,
            domain,
            format,
        } => cmd_prompt_get(name, domain, format),

        PromptAction::Run {
            name,
            variables,
            domain,
            interactive,
        } => cmd_prompt_run(name, variables, domain, interactive),

        PromptAction::Delete {
            name,
            domain,
            force,
        } => cmd_prompt_delete(name, domain, force),

        PromptAction::Export {
            name,
            output,
            format,
            domain,
        } => cmd_prompt_export(name, output, format, domain),
    }
}
