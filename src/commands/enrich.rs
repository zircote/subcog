//! Enrich command handler.
//!
//! Contains the implementation of the `enrich` CLI command for
//! LLM-powered memory tag enrichment.

use std::sync::Arc;

use subcog::cli::{
    build_anthropic_client, build_lmstudio_client, build_ollama_client, build_openai_client,
    build_resilience_config,
};
use subcog::config::{LlmProvider, SubcogConfig};
use subcog::llm::LlmProvider as LlmProviderTrait;
use subcog::services::ServiceContainer;
use subcog::storage::traits::IndexBackend;

/// Enrich command.
pub fn cmd_enrich(
    config: &SubcogConfig,
    all: bool,
    update_all: bool,
    id: Option<String>,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
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
        ),
        LlmProvider::Anthropic => run_enrich_with_client(
            build_anthropic_client(llm_config),
            llm_config,
            all,
            update_all,
            id,
            dry_run,
        ),
        LlmProvider::Ollama => run_enrich_with_client(
            build_ollama_client(llm_config),
            llm_config,
            all,
            update_all,
            id,
            dry_run,
        ),
        LlmProvider::LmStudio => run_enrich_with_client(
            build_lmstudio_client(llm_config),
            llm_config,
            all,
            update_all,
            id,
            dry_run,
        ),
        LlmProvider::None => {
            eprintln!("Error: LLM provider is set to 'none'. Enrichment requires an LLM provider.");
            eprintln!(
                "Configure a provider in subcog.toml or set SUBCOG_LLM_PROVIDER environment variable."
            );
            Err("LLM provider not configured".into())
        },
    }
}

fn run_enrich_with_client<P: LlmProviderTrait>(
    client: P,
    llm_config: &subcog::config::LlmConfig,
    all: bool,
    update_all: bool,
    id: Option<String>,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create service container to get index backend
    let container = ServiceContainer::from_current_dir_or_user()?;
    let index: Arc<dyn IndexBackend> = Arc::new(container.index()?);

    let resilience_config = build_resilience_config(llm_config);
    let client = subcog::llm::ResilientLlmProvider::new(client, resilience_config);
    run_enrichment(
        subcog::services::EnrichmentService::new(client, index),
        all,
        update_all,
        id,
        dry_run,
    )
}

/// Runs the enrichment operation with the given service.
fn run_enrichment<P: LlmProviderTrait>(
    service: subcog::services::EnrichmentService<P>,
    all: bool,
    update_all: bool,
    id: Option<String>,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
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
                    let services = ServiceContainer::from_current_dir_or_user()?;
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
