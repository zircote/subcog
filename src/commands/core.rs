//! Core command handlers.
//!
//! Contains the implementation of core CLI commands:
//! capture, recall, status, sync, consolidate, reindex, namespaces.

use std::path::PathBuf;

use subcog::config::{SubcogConfig, parse_duration_to_seconds};
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
    ttl: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let services = subcog::services::ServiceContainer::from_current_dir_or_user()?;
    let service = services.capture();

    let tag_list = tags
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    // Parse TTL from duration string if provided
    let ttl_seconds = ttl.as_ref().and_then(|s| parse_duration_to_seconds(s));

    // Use context-aware domain: project if in git repo, user if not
    let request = CaptureRequest {
        content,
        namespace: parse_namespace(&namespace),
        domain: Domain::default_for_context(),
        tags: tag_list,
        source,
        skip_security_check: false,
        ttl_seconds,
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
    entity: Option<String>,
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
    // Apply entity filter if provided (comma-separated for OR logic)
    if let Some(ref entity_arg) = entity {
        let entities: Vec<String> = entity_arg
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect();
        filter = filter.with_entities(entities);
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
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn cmd_consolidate(
    config: &SubcogConfig,
    namespace: Vec<String>,
    days: Option<u32>,
    dry_run: bool,
    min_memories: Option<usize>,
    similarity: Option<f32>,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::str::FromStr;
    use std::sync::Arc;
    use subcog::cli::{
        build_anthropic_client, build_lmstudio_client, build_ollama_client, build_openai_client,
        build_resilience_config,
    };
    use subcog::config::{LlmProvider, StorageBackendType};
    use subcog::llm::ResilientLlmProvider;
    use subcog::models::Namespace;
    use subcog::services::{ConsolidationService, ServiceContainer};
    use subcog::storage::index::SqliteBackend;
    use subcog::storage::persistence::FilesystemBackend;

    println!("Running memory consolidation...");
    println!();

    // Check if consolidation is enabled
    if !config.consolidation.enabled {
        println!("Consolidation is disabled in configuration.");
        println!("To enable, set [consolidation] enabled = true in your config file");
        return Ok(());
    }

    // Build effective consolidation config from config file + CLI overrides
    let mut consolidation_config = config.consolidation.clone();

    // Override namespace filter if provided
    if !namespace.is_empty() {
        let parsed_namespaces: Result<Vec<Namespace>, _> =
            namespace.iter().map(|ns| Namespace::from_str(ns)).collect();

        match parsed_namespaces {
            Ok(namespaces) => {
                consolidation_config.namespace_filter = Some(namespaces);
            },
            Err(e) => {
                eprintln!("Error: Invalid namespace: {e}");
                return Err(e.into());
            },
        }
    }

    // Override time window if provided
    if let Some(d) = days {
        consolidation_config.time_window_days = Some(d);
    }

    // Override min memories if provided
    if let Some(min) = min_memories {
        consolidation_config.min_memories_to_consolidate = min;
    }

    // Override similarity threshold if provided
    if let Some(sim) = similarity {
        if !(0.0..=1.0).contains(&sim) {
            eprintln!("Error: Similarity threshold must be between 0.0 and 1.0");
            return Err("Invalid similarity threshold".into());
        }
        consolidation_config.similarity_threshold = sim;
    }

    // Handle dry-run mode
    if dry_run {
        println!("DRY RUN MODE - No changes will be made");
        println!();
    }

    let data_dir = &config.data_dir;
    let storage_config = &config.storage.project;

    // Ensure data directory exists
    if !data_dir.exists() {
        std::fs::create_dir_all(data_dir)?;
    }

    // Create service container for recall service
    let services = ServiceContainer::from_current_dir_or_user()?;
    let recall_service = services.recall()?;

    // Get index backend
    let index = Arc::new(services.index()?);

    // Build LLM provider (optional, for summarization)
    let llm_provider: Option<Arc<dyn subcog::llm::LlmProvider>> = {
        let llm_config = &config.llm;
        let resilience_config = build_resilience_config(llm_config);

        match llm_config.provider {
            LlmProvider::OpenAi => {
                let client = build_openai_client(llm_config);
                Some(Arc::new(ResilientLlmProvider::new(
                    client,
                    resilience_config,
                )))
            },
            LlmProvider::Anthropic => {
                let client = build_anthropic_client(llm_config);
                Some(Arc::new(ResilientLlmProvider::new(
                    client,
                    resilience_config,
                )))
            },
            LlmProvider::Ollama => {
                let client = build_ollama_client(llm_config);
                Some(Arc::new(ResilientLlmProvider::new(
                    client,
                    resilience_config,
                )))
            },
            LlmProvider::LmStudio => {
                let client = build_lmstudio_client(llm_config);
                Some(Arc::new(ResilientLlmProvider::new(
                    client,
                    resilience_config,
                )))
            },
            LlmProvider::None => None,
        }
    };

    // Display configuration
    println!("Configuration:");
    println!("  Storage backend: {:?}", storage_config.backend);
    if llm_provider.is_some() {
        println!("  LLM provider: {:?}", config.llm.provider);
    } else {
        println!("  LLM provider: None (will skip summarization)");
    }
    if let Some(ref namespaces) = consolidation_config.namespace_filter {
        println!("  Namespaces: {namespaces:?}");
    } else {
        println!("  Namespaces: all");
    }
    if let Some(d) = consolidation_config.time_window_days {
        println!("  Time window: {d} days");
    } else {
        println!("  Time window: all time");
    }
    println!(
        "  Similarity threshold: {}",
        consolidation_config.similarity_threshold
    );
    println!(
        "  Minimum memories: {}",
        consolidation_config.min_memories_to_consolidate
    );
    println!();

    // Create consolidation service based on configured backend
    println!("Finding related memory groups...");

    match storage_config.backend {
        StorageBackendType::Sqlite => {
            let db_path = storage_config
                .path
                .as_ref()
                .map_or_else(|| data_dir.join("memories.db"), std::path::PathBuf::from);

            let backend = SqliteBackend::new(&db_path)?;
            let mut service = ConsolidationService::new(backend).with_index(Arc::clone(&index));

            if let Some(llm) = llm_provider {
                service = service.with_llm(llm);
            }

            run_consolidation(
                &mut service,
                &recall_service,
                &consolidation_config,
                dry_run,
            )?;
        },
        StorageBackendType::Filesystem => {
            let backend = FilesystemBackend::new(data_dir);
            let mut service = ConsolidationService::new(backend).with_index(Arc::clone(&index));

            if let Some(llm) = llm_provider {
                service = service.with_llm(llm);
            }

            run_consolidation(
                &mut service,
                &recall_service,
                &consolidation_config,
                dry_run,
            )?;
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

/// Runs consolidation with the new API and prints results.
#[allow(clippy::excessive_nesting, clippy::too_many_lines)]
fn run_consolidation<P: PersistenceBackend>(
    service: &mut subcog::services::ConsolidationService<P>,
    recall_service: &subcog::services::RecallService,
    consolidation_config: &subcog::config::ConsolidationConfig,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if dry_run {
        // Dry-run mode: show what would be consolidated without making changes
        match service.find_related_memories(recall_service, consolidation_config) {
            Ok(groups) => {
                println!();
                println!("Dry-run results (no changes made):");
                println!();

                let mut total_groups = 0;
                let mut total_memories = 0;

                for (namespace, namespace_groups) in &groups {
                    if namespace_groups.is_empty() {
                        continue;
                    }

                    println!("Namespace: {namespace:?}");
                    for (idx, group) in namespace_groups.iter().enumerate() {
                        println!("  Group {}: {} memories", idx + 1, group.len());
                        total_groups += 1;
                        total_memories += group.len();
                    }
                    println!();
                }

                println!("Summary:");
                println!("  Would create {total_groups} summary node(s)");
                println!("  Would consolidate {total_memories} memory/memories");
                println!();
                println!("Run without --dry-run to apply changes");

                Ok(())
            },
            Err(e) => {
                eprintln!("Failed to find related memories: {e}");
                Err(e.into())
            },
        }
    } else {
        // Normal mode: perform actual consolidation with progress reporting

        // Step 1: Find related memory groups and show progress
        println!();
        print!("Analyzing memories... ");
        match service.find_related_memories(recall_service, consolidation_config) {
            Ok(groups) => {
                let mut total_groups = 0;
                let mut total_memories = 0;
                let mut namespaces_with_groups = 0;

                for namespace_groups in groups.values() {
                    if !namespace_groups.is_empty() {
                        namespaces_with_groups += 1;
                        for group in namespace_groups {
                            total_groups += 1;
                            total_memories += group.len();
                        }
                    }
                }

                println!("done");
                println!();

                if total_groups == 0 {
                    println!("No related memory groups found.");
                    println!("  No memories to consolidate");
                    return Ok(());
                }

                println!(
                    "Found {total_groups} group(s) across {namespaces_with_groups} namespace(s)"
                );
                println!("  Total memories to consolidate: {total_memories}");
                println!();

                // Show breakdown by namespace
                for (namespace, namespace_groups) in &groups {
                    if namespace_groups.is_empty() {
                        continue;
                    }
                    let ns_memories: usize = namespace_groups.iter().map(Vec::len).sum();
                    println!(
                        "  {:?}: {} group(s), {} memories",
                        namespace,
                        namespace_groups.len(),
                        ns_memories
                    );
                }
                println!();

                // Step 2: Perform consolidation
                println!("Creating summaries...");

                match service.consolidate_memories(recall_service, consolidation_config) {
                    Ok(stats) => {
                        println!();
                        println!("Consolidation completed:");
                        println!("  {}", stats.summary());

                        if stats.summaries_created > 0 {
                            let created = stats.summaries_created;
                            println!("  ✓ Created {created} summary node(s)");
                            println!("  ✓ Linked {total_memories} source memories via edges");
                        }

                        if stats.contradictions > 0 {
                            println!(
                                "  ⚠ {} potential contradiction(s) detected - review recommended",
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
            },
            Err(e) => {
                println!("failed");
                eprintln!("Failed to find related memories: {e}");
                Err(e.into())
            },
        }
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
