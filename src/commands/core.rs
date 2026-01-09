//! Core command handlers.
//!
//! Contains the implementation of core CLI commands:
//! capture, recall, status, sync, consolidate, reindex, namespaces.

use std::path::PathBuf;

use subcog::config::SubcogConfig;
use subcog::storage::PersistenceBackend;
use subcog::{CaptureRequest, Domain, Namespace, SearchFilter, SearchMode};

/// Parses namespace string.
pub fn parse_namespace(s: &str) -> Namespace {
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
pub fn parse_search_mode(s: &str) -> SearchMode {
    match s.to_lowercase().as_str() {
        "text" => SearchMode::Text,
        "vector" => SearchMode::Vector,
        "hybrid" => SearchMode::Hybrid,
        _ => SearchMode::Hybrid,
    }
}

/// Capture command.
pub fn cmd_capture(
    _config: &SubcogConfig,
    content: String,
    namespace: String,
    tags: Option<String>,
    source: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let services = subcog::services::ServiceContainer::from_current_dir_or_user()?;
    let service = services.capture();

    let tag_list = tags
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    // Use context-aware domain: project if in git repo, user if not
    let request = CaptureRequest {
        content,
        namespace: parse_namespace(&namespace),
        domain: Domain::default_for_context(),
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
///
/// # Arguments
///
/// * `query` - The search query
/// * `mode` - Search mode: text, vector, or hybrid
/// * `namespace` - Optional namespace filter
/// * `limit` - Maximum number of results
/// * `raw` - If true, display raw (un-normalized) scores instead of normalized scores
pub fn cmd_recall(
    query: String,
    mode: String,
    namespace: Option<String>,
    limit: usize,
    raw: bool,
    include_tombstoned: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use subcog::services::ServiceContainer;

    // Use domain-scoped index (user-level storage with project facets)
    let services = ServiceContainer::from_current_dir_or_user()?;
    let service = services.recall()?;

    let mut filter = SearchFilter::new();
    if let Some(ns) = namespace {
        filter = filter.with_namespace(parse_namespace(&ns));
    }
    if include_tombstoned {
        filter = filter.with_include_tombstoned(true);
    }

    let result = service.search(&query, parse_search_mode(&mode), &filter, limit);

    match result {
        Ok(search_result) => {
            println!("Found {} memories:", search_result.total_count);
            println!();

            for hit in &search_result.memories {
                // Use raw_score if --raw flag is set, otherwise use normalized score
                let display_score = if raw { hit.raw_score } else { hit.score };
                println!(
                    "  [{:.4}] {} ({})",
                    display_score,
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

            let score_type = if raw { " (raw)" } else { "" };
            println!(
                "Search completed in {}ms{}",
                search_result.execution_time_ms, score_type
            );
        },
        Err(e) => {
            eprintln!("Search failed: {e}");
            eprintln!("Note: Make sure a storage backend is configured");
        },
    }

    Ok(())
}

/// Status command.
pub fn cmd_status(config: &SubcogConfig) -> Result<(), Box<dyn std::error::Error>> {
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

    println!();
    println!("Use 'subcog config --show' to view full configuration");

    Ok(())
}

/// Consolidate command.
pub fn cmd_consolidate(config: &SubcogConfig) -> Result<(), Box<dyn std::error::Error>> {
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
        StorageBackendType::Filesystem => {
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
fn run_consolidation<P: PersistenceBackend>(
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
pub fn cmd_reindex(repo: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    use subcog::services::ServiceContainer;

    let services = match repo {
        Some(repo_path) => ServiceContainer::for_repo(&repo_path, None)?,
        None => ServiceContainer::from_current_dir_or_user()?,
    };

    println!("Reindexing memories from SQLite storage...");
    match services.repo_path() {
        Some(repo_root) => println!("Repository: {}", repo_root.display()),
        None => println!("Scope: user"),
    }
    println!();

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
