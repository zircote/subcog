//! Core tool execution handlers.
//!
//! Contains handlers for subcog's core memory operations:
//! capture, recall, status, namespaces, prompt understanding, consolidate, enrich, reindex.

use crate::config::{
    ConsolidationConfig, LlmProvider, StorageBackendType, SubcogConfig, parse_duration_to_seconds,
};
use crate::llm::ResilientLlmProvider;
use crate::mcp::prompt_understanding::PROMPT_UNDERSTANDING;
use crate::mcp::tool_types::{
    CaptureArgs, ConsolidateArgs, DeleteArgs, EnrichArgs, GetArgs, InitArgs, RecallArgs,
    ReindexArgs, UpdateArgs, build_filter_description, format_content_for_detail,
    parse_domain_scope, parse_namespace, parse_search_mode,
};
#[cfg(test)]
use crate::models::SearchResult;
use crate::models::{
    CaptureRequest, DetailLevel, Domain, EventMeta, MemoryEvent, MemoryId, MemoryStatus, Namespace,
    SearchFilter, SearchMode,
};
use crate::observability::current_request_id;
use crate::security::record_event;
use crate::services::{ConsolidationService, ServiceContainer, parse_filter_query};
use crate::storage::index::SqliteBackend;
use crate::storage::persistence::FilesystemBackend;
use crate::{Error, Result};
use serde_json::Value;
use std::str::FromStr;
use std::sync::Arc;

use super::super::{ToolContent, ToolResult};

/// Maximum allowed input length for content fields (SEC-M5).
///
/// Prevents `DoS` attacks via extremely large inputs that could exhaust memory
/// or cause excessive processing time. Set to 1MB which is generous for
/// any reasonable memory content while preventing abuse.
const MAX_CONTENT_LENGTH: usize = 1_048_576; // 1 MB

/// Maximum allowed input length for query fields (SEC-M5).
///
/// Queries should be concise - 10KB is more than enough for any search query.
const MAX_QUERY_LENGTH: usize = 10_240; // 10 KB

/// Validates that a string input does not exceed the maximum allowed length.
///
/// # Errors
///
/// Returns `Error::InvalidInput` if the input exceeds `max_length`.
fn validate_input_length(input: &str, field_name: &str, max_length: usize) -> Result<()> {
    if input.len() > max_length {
        return Err(Error::InvalidInput(format!(
            "{field_name} exceeds maximum length ({} > {max_length} bytes)",
            input.len()
        )));
    }
    Ok(())
}

#[cfg(test)]
fn fetch_consolidation_candidates(
    recall: &crate::services::RecallService,
    filter: &SearchFilter,
    query: Option<&str>,
    limit: usize,
) -> Result<SearchResult> {
    let query = query.unwrap_or("*");
    if query == "*" || query.is_empty() {
        recall.list_all(filter, limit)
    } else {
        recall.search(query, SearchMode::Hybrid, filter, limit)
    }
}

/// Executes the capture tool.
pub fn execute_capture(arguments: Value) -> Result<ToolResult> {
    let args: CaptureArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    // SEC-M5: Validate input length before processing
    validate_input_length(&args.content, "content", MAX_CONTENT_LENGTH)?;

    let namespace = parse_namespace(&args.namespace);

    // Parse TTL from duration string if provided
    let ttl_seconds = args.ttl.as_ref().and_then(|s| parse_duration_to_seconds(s));

    // Parse domain scope from argument, defaulting to context-aware detection
    let scope = parse_domain_scope(args.domain.as_deref());

    // Determine domain based on scope
    let domain = match scope {
        crate::storage::index::DomainScope::User => Domain::for_user(),
        crate::storage::index::DomainScope::Org => Domain::for_org(),
        crate::storage::index::DomainScope::Project => Domain::default_for_context(),
    };

    let request = CaptureRequest {
        content: args.content,
        namespace,
        domain,
        tags: args.tags.unwrap_or_default(),
        source: args.source,
        skip_security_check: false,
        ttl_seconds,
        scope: Some(scope),
        #[cfg(feature = "group-scope")]
        group_id: None,
    };

    let services = ServiceContainer::from_current_dir_or_user()?;
    let result = services.capture().capture(request)?;

    Ok(ToolResult {
        content: vec![ToolContent::Text {
            text: format!(
                "Memory captured successfully!\n\nID: {}\nURN: {}\nRedacted: {}",
                result.memory_id, result.urn, result.content_modified
            ),
        }],
        is_error: false,
    })
}

/// Executes the recall tool.
///
/// When `query` is omitted or empty, behaves like `subcog_list` and returns
/// all memories matching the filter criteria (with pagination support).
pub fn execute_recall(arguments: Value) -> Result<ToolResult> {
    let args: RecallArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    // Get query, treating None and empty string as "list all"
    let query = args.query.as_deref().unwrap_or("");
    let is_list_mode = query.is_empty() || query == "*";

    // SEC-M5: Validate query length before processing (skip for empty/wildcard)
    if !is_list_mode {
        validate_input_length(query, "query", MAX_QUERY_LENGTH)?;
    }

    let mode = args
        .mode
        .as_deref()
        .map_or(SearchMode::Hybrid, parse_search_mode);

    let detail = args
        .detail
        .as_deref()
        .and_then(DetailLevel::parse)
        .unwrap_or_default();

    // Build filter from the filter query string
    let mut filter = if let Some(filter_query) = &args.filter {
        parse_filter_query(filter_query)
    } else {
        SearchFilter::new()
    };

    // Support legacy namespace parameter (deprecated but still works)
    if let Some(ns) = &args.namespace {
        filter = filter.with_namespace(parse_namespace(ns));
    }

    // Apply entity filter if provided (comma-separated for OR logic)
    if let Some(ref entity_arg) = args.entity {
        let entities: Vec<String> = entity_arg
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect();
        filter = filter.with_entities(entities);
    }

    // Apply user_id and agent_id filters if provided (for multi-tenant scoping)
    // These are added as tag filters: user:<id> and agent:<id>
    if let Some(ref user_id) = args.user_id {
        filter = filter.with_tag(format!("user:{user_id}"));
    }
    if let Some(ref agent_id) = args.agent_id {
        filter = filter.with_tag(format!("agent:{agent_id}"));
    }

    // Different defaults for search vs list mode
    // Search: default 10, max 50
    // List: default 50, max 1000
    let limit = if is_list_mode {
        args.limit.unwrap_or(50).min(1000)
    } else {
        args.limit.unwrap_or(10).min(50)
    };

    let services = ServiceContainer::from_current_dir_or_user()?;
    let recall = services.recall()?;

    // Use list_all for wildcard queries or filter-only queries
    // Use search for actual text queries
    let result = if is_list_mode {
        recall.list_all(&filter, limit)?
    } else {
        recall.search(query, mode, &filter, limit)?
    };

    // Build filter description for output
    let filter_desc = build_filter_description(&filter);

    let mut output = format!(
        "Found {} memories (searched in {}ms using {} mode, detail: {}{})\n\n",
        result.total_count, result.execution_time_ms, result.mode, detail, filter_desc
    );

    for (i, hit) in result.memories.iter().enumerate() {
        // Format content based on detail level
        let content_display = format_content_for_detail(&hit.memory.content, detail);

        let tags_display = if hit.memory.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", hit.memory.tags.join(", "))
        };

        // Build URN: subcog://{domain}/{namespace}/{id}
        // Domain: project, user, or org/repo path
        let domain_part = if hit.memory.domain.is_project_scoped() {
            "project".to_string()
        } else {
            hit.memory.domain.to_string()
        };
        let urn = format!(
            "subcog://{}/{}/{}",
            domain_part, hit.memory.namespace, hit.memory.id
        );

        // Display both normalized score and raw score for transparency
        // Format: "1.00 (raw: 0.0325)" or just "1.00" if they're the same
        let score_display = if (hit.score - hit.raw_score).abs() < f32::EPSILON {
            format!("{:.2}", hit.score)
        } else {
            format!("{:.2} (raw: {:.4})", hit.score, hit.raw_score)
        };

        output.push_str(&format!(
            "{}. {} | {}{}{}\n\n",
            i + 1,
            urn,
            score_display,
            tags_display,
            content_display,
        ));
    }

    Ok(ToolResult {
        content: vec![ToolContent::Text { text: output }],
        is_error: false,
    })
}

/// Health status for a backend component.
#[derive(Debug, Clone, serde::Serialize)]
struct ComponentHealth {
    /// Component name.
    name: String,
    /// Health status: "healthy", "degraded", or "unhealthy".
    status: String,
    /// Optional details about the health check.
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
    /// Response time in milliseconds (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    response_time_ms: Option<u64>,
}

impl ComponentHealth {
    fn healthy(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: "healthy".to_string(),
            details: None,
            response_time_ms: None,
        }
    }

    fn healthy_with_time(name: impl Into<String>, elapsed: std::time::Duration) -> Self {
        // Convert duration to ms, saturating at u64::MAX for safety
        let response_time_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        Self {
            name: name.into(),
            status: "healthy".to_string(),
            details: None,
            response_time_ms: Some(response_time_ms),
        }
    }

    fn unhealthy(name: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: "unhealthy".to_string(),
            details: Some(details.into()),
            response_time_ms: None,
        }
    }

    fn degraded(name: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: "degraded".to_string(),
            details: Some(details.into()),
            response_time_ms: None,
        }
    }
}

/// Checks health of the persistence layer by attempting a simple operation.
fn check_persistence_health(services: &ServiceContainer) -> ComponentHealth {
    let start = std::time::Instant::now();
    match services.recall() {
        Ok(recall) => {
            // Try a simple list operation with limit 1
            let filter = SearchFilter::new();
            match recall.list_all(&filter, 1) {
                Ok(_) => {
                    ComponentHealth::healthy_with_time("persistence (sqlite)", start.elapsed())
                },
                Err(e) => ComponentHealth::degraded("persistence (sqlite)", e.to_string()),
            }
        },
        Err(e) => ComponentHealth::unhealthy("persistence (sqlite)", e.to_string()),
    }
}

/// Checks health of the index layer.
fn check_index_health(services: &ServiceContainer) -> ComponentHealth {
    let start = std::time::Instant::now();
    match services.recall() {
        Ok(recall) => {
            // Try a simple search operation
            let filter = SearchFilter::new();
            match recall.search("health_check_probe", SearchMode::Text, &filter, 1) {
                Ok(_) => ComponentHealth::healthy_with_time("index (sqlite-fts5)", start.elapsed()),
                Err(e) => ComponentHealth::degraded("index (sqlite-fts5)", e.to_string()),
            }
        },
        Err(e) => ComponentHealth::unhealthy("index (sqlite-fts5)", e.to_string()),
    }
}

/// Checks health of the vector layer (embedding search).
fn check_vector_health(services: &ServiceContainer) -> ComponentHealth {
    let start = std::time::Instant::now();
    match services.recall() {
        Ok(recall) => {
            // Try a vector search - this will work even without embeddings
            let filter = SearchFilter::new();
            match recall.search("health_check_probe", SearchMode::Vector, &filter, 1) {
                Ok(_) => ComponentHealth::healthy_with_time("vector (usearch)", start.elapsed()),
                Err(e) => {
                    // Vector search may fail if no embeddings exist - that's degraded, not unhealthy
                    if e.to_string().contains("no embeddings")
                        || e.to_string().contains("not configured")
                    {
                        ComponentHealth::degraded("vector (usearch)", "No embeddings available")
                    } else {
                        ComponentHealth::degraded("vector (usearch)", e.to_string())
                    }
                },
            }
        },
        Err(e) => ComponentHealth::unhealthy("vector (usearch)", e.to_string()),
    }
}

/// Checks health of the capture service.
fn check_capture_health(services: &ServiceContainer) -> ComponentHealth {
    // capture() returns &CaptureService directly (infallible)
    // Just verify the service exists
    let _capture = services.capture();
    ComponentHealth::healthy("capture_service")
}

/// Executes the status tool with comprehensive health checks (CHAOS-HIGH-006).
///
/// Performs actual health probes against all backend components:
/// - Persistence layer (`SQLite`)
/// - Index layer (`SQLite` FTS5)
/// - Vector layer (usearch)
/// - Capture service
///
/// Returns overall system health based on component health:
/// - "healthy": All components operational
/// - "degraded": Some components have issues but system is functional
/// - "unhealthy": Critical components are down
pub fn execute_status(_arguments: Value) -> Result<ToolResult> {
    let mut components = Vec::new();
    let mut any_unhealthy = false;
    let mut any_degraded = false;

    // Try to get services - if this fails, the system is unhealthy
    let services_result = ServiceContainer::from_current_dir_or_user();

    match services_result {
        Ok(services) => {
            // Check persistence health
            let persistence = check_persistence_health(&services);
            if persistence.status == "unhealthy" {
                any_unhealthy = true;
            } else if persistence.status == "degraded" {
                any_degraded = true;
            }
            components.push(persistence);

            // Check index health
            let index = check_index_health(&services);
            if index.status == "unhealthy" {
                any_unhealthy = true;
            } else if index.status == "degraded" {
                any_degraded = true;
            }
            components.push(index);

            // Check vector health
            let vector = check_vector_health(&services);
            if vector.status == "unhealthy" {
                any_unhealthy = true;
            } else if vector.status == "degraded" {
                any_degraded = true;
            }
            components.push(vector);

            // Check capture service health
            let capture = check_capture_health(&services);
            if capture.status == "unhealthy" {
                any_unhealthy = true;
            } else if capture.status == "degraded" {
                any_degraded = true;
            }
            components.push(capture);
        },
        Err(e) => {
            any_unhealthy = true;
            components.push(ComponentHealth::unhealthy(
                "service_container",
                e.to_string(),
            ));
        },
    }

    // Determine overall status
    let overall_status = if any_unhealthy {
        "unhealthy"
    } else if any_degraded {
        "degraded"
    } else {
        "healthy"
    };

    let status = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "status": overall_status,
        "components": components,
        "features": {
            "semantic_search": true,
            "secret_detection": true,
            "hooks": true,
            "circuit_breakers": true,
            "bulkhead_isolation": true,
            "configurable_timeouts": true
        }
    });

    Ok(ToolResult {
        content: vec![ToolContent::Text {
            text: serde_json::to_string_pretty(&status)
                .unwrap_or_else(|_| "Status unavailable".to_string()),
        }],
        is_error: false,
    })
}

/// Executes the `prompt_understanding` tool.
pub fn execute_prompt_understanding(_arguments: Value) -> Result<ToolResult> {
    Ok(ToolResult {
        content: vec![ToolContent::Text {
            text: PROMPT_UNDERSTANDING.to_string(),
        }],
        is_error: false,
    })
}

/// Executes the namespaces tool.
pub fn execute_namespaces(_arguments: Value) -> Result<ToolResult> {
    let namespaces = vec![
        ("decisions", "Architectural and design decisions"),
        ("patterns", "Discovered patterns and conventions"),
        ("learnings", "Lessons learned from debugging or issues"),
        ("context", "Important contextual information"),
        ("tech-debt", "Technical debts and future improvements"),
        ("apis", "API endpoints and contracts"),
        ("config", "Configuration and environment details"),
        ("security", "Security-related information"),
        ("performance", "Performance optimizations and benchmarks"),
        ("testing", "Testing strategies and edge cases"),
    ];

    let mut output = "Available Memory Namespaces:\n\n".to_string();
    for (name, desc) in namespaces {
        output.push_str(&format!("- **{name}**: {desc}\n"));
    }

    Ok(ToolResult {
        content: vec![ToolContent::Text { text: output }],
        is_error: false,
    })
}

/// Executes the consolidate tool.
/// Triggers memory consolidation and returns statistics.
#[allow(clippy::too_many_lines)]
pub fn execute_consolidate(arguments: Value) -> Result<ToolResult> {
    let args: ConsolidateArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    let dry_run = args.dry_run.unwrap_or(false);

    // Load config to check if consolidation is enabled
    let config = SubcogConfig::load_default();

    if !config.consolidation.enabled {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: "Consolidation is disabled in configuration. To enable, set [consolidation] enabled = true in your config file.".to_string(),
            }],
            is_error: false,
        });
    }

    // Build effective consolidation config from config file + MCP args
    let mut consolidation_config = config.consolidation.clone();

    // Override namespace filter if provided
    if let Some(ref namespaces) = args.namespaces {
        let parsed_namespaces: std::result::Result<Vec<Namespace>, _> = namespaces
            .iter()
            .map(|ns| Namespace::from_str(ns))
            .collect();

        match parsed_namespaces {
            Ok(ns) => {
                consolidation_config.namespace_filter = Some(ns);
            },
            Err(e) => {
                return Ok(ToolResult {
                    content: vec![ToolContent::Text {
                        text: format!("Invalid namespace: {e}"),
                    }],
                    is_error: true,
                });
            },
        }
    }

    // Override time window if provided
    if let Some(d) = args.days {
        consolidation_config.time_window_days = Some(d);
    }

    // Override min memories if provided
    if let Some(min) = args.min_memories {
        consolidation_config.min_memories_to_consolidate = min;
    }

    // Override similarity threshold if provided
    if let Some(sim) = args.similarity {
        if !(0.0..=1.0).contains(&sim) {
            return Ok(ToolResult {
                content: vec![ToolContent::Text {
                    text: "Similarity threshold must be between 0.0 and 1.0".to_string(),
                }],
                is_error: true,
            });
        }
        consolidation_config.similarity_threshold = sim;
    }

    let data_dir = &config.data_dir;
    let storage_config = &config.storage.project;

    // Create service container for recall service and index
    let services = ServiceContainer::from_current_dir_or_user()?;
    let recall_service = services.recall()?;
    let index = Arc::new(services.index()?);

    // Build LLM provider (optional, for summarization)
    let llm_provider: Option<Arc<dyn crate::llm::LlmProvider + Send + Sync>> =
        build_llm_provider_from_config(&config.llm);

    // Run consolidation based on configured backend
    let result_text = match storage_config.backend {
        StorageBackendType::Sqlite => {
            let db_path = storage_config
                .path
                .as_ref()
                .map_or_else(|| data_dir.join("memories.db"), std::path::PathBuf::from);

            let backend = SqliteBackend::new(&db_path)?;
            let mut service = ConsolidationService::new(backend).with_index(index);

            if let Some(llm) = llm_provider {
                service = service.with_llm(llm);
            }

            run_mcp_consolidation(
                &mut service,
                &recall_service,
                &consolidation_config,
                dry_run,
            )?
        },
        StorageBackendType::Filesystem => {
            let backend = FilesystemBackend::new(data_dir);
            let mut service = ConsolidationService::new(backend).with_index(index);

            if let Some(llm) = llm_provider {
                service = service.with_llm(llm);
            }

            run_mcp_consolidation(
                &mut service,
                &recall_service,
                &consolidation_config,
                dry_run,
            )?
        },
        StorageBackendType::PostgreSQL | StorageBackendType::Redis => {
            return Ok(ToolResult {
                content: vec![ToolContent::Text {
                    text: format!(
                        "{:?} consolidation not yet implemented",
                        storage_config.backend
                    ),
                }],
                is_error: true,
            });
        },
    };

    Ok(ToolResult {
        content: vec![ToolContent::Text { text: result_text }],
        is_error: false,
    })
}

/// Builds an LLM provider from configuration.
///
/// Returns `None` if the provider is set to `None` or if client creation fails.
fn build_llm_provider_from_config(
    llm_config: &crate::config::LlmConfig,
) -> Option<Arc<dyn crate::llm::LlmProvider + Send + Sync>> {
    use crate::llm::{
        AnthropicClient, LlmResilienceConfig, LmStudioClient, OllamaClient, OpenAiClient,
    };

    // Build resilience config from LLM settings
    let resilience_config = LlmResilienceConfig {
        max_retries: llm_config.max_retries.unwrap_or(3),
        retry_backoff_ms: llm_config.retry_backoff_ms.unwrap_or(1000),
        breaker_failure_threshold: llm_config.breaker_failure_threshold.unwrap_or(5),
        breaker_reset_timeout_ms: llm_config.breaker_reset_ms.unwrap_or(30_000),
        breaker_half_open_max_calls: llm_config.breaker_half_open_max_calls.unwrap_or(3),
        latency_slo_ms: llm_config.latency_slo_ms.unwrap_or(5000),
        error_budget_ratio: llm_config.error_budget_ratio.unwrap_or(0.01),
        error_budget_window_secs: llm_config.error_budget_window_secs.unwrap_or(3600),
    };

    match llm_config.provider {
        LlmProvider::OpenAi => {
            let client = OpenAiClient::new();
            Some(Arc::new(ResilientLlmProvider::new(
                client,
                resilience_config,
            )))
        },
        LlmProvider::Anthropic => {
            let client = AnthropicClient::new();
            Some(Arc::new(ResilientLlmProvider::new(
                client,
                resilience_config,
            )))
        },
        LlmProvider::Ollama => {
            let client = OllamaClient::new();
            Some(Arc::new(ResilientLlmProvider::new(
                client,
                resilience_config,
            )))
        },
        LlmProvider::LmStudio => {
            let client = LmStudioClient::new();
            Some(Arc::new(ResilientLlmProvider::new(
                client,
                resilience_config,
            )))
        },
        LlmProvider::None => None,
    }
}

/// Helper function to run consolidation and format results for MCP tool response.
fn run_mcp_consolidation<P: crate::storage::PersistenceBackend>(
    service: &mut ConsolidationService<P>,
    recall_service: &crate::services::RecallService,
    consolidation_config: &ConsolidationConfig,
    dry_run: bool,
) -> Result<String> {
    if dry_run {
        // Dry-run mode: show what would be consolidated
        let groups = service.find_related_memories(recall_service, consolidation_config)?;

        let mut output = String::from("**Dry-run results (no changes made)**\n\n");

        let mut total_groups = 0;
        let mut total_memories = 0;

        for (namespace, namespace_groups) in &groups {
            if namespace_groups.is_empty() {
                continue;
            }

            output.push_str(&format!(
                "**{:?}**: {} group(s)\n",
                namespace,
                namespace_groups.len()
            ));
            for (idx, group) in namespace_groups.iter().enumerate() {
                output.push_str(&format!(
                    "  - Group {}: {} memories\n",
                    idx + 1,
                    group.len()
                ));
                total_groups += 1;
                total_memories += group.len();
            }
            output.push('\n');
        }

        if total_groups == 0 {
            output.push_str("No related memory groups found.\n");
        } else {
            output.push_str("**Summary:**\n");
            output.push_str(&format!(
                "  - Would create {total_groups} summary node(s)\n"
            ));
            output.push_str(&format!(
                "  - Would consolidate {total_memories} memory/memories\n"
            ));
        }

        Ok(output)
    } else {
        // Normal mode: perform actual consolidation
        let stats = service.consolidate_memories(recall_service, consolidation_config)?;

        let mut output = String::from("**Consolidation completed**\n\n");

        if stats.processed == 0 {
            output.push_str("No memories found to consolidate.\n");
        } else {
            output.push_str("**Statistics:**\n");
            output.push_str(&format!("  - Processed: {} memories\n", stats.processed));
            output.push_str(&format!(
                "  - Summary nodes created: {}\n",
                stats.summaries_created
            ));
            output.push_str(&format!("  - Merged: {}\n", stats.merged));
            output.push_str(&format!("  - Archived: {}\n", stats.archived));
            if stats.contradictions > 0 {
                output.push_str(&format!(
                    "  - Contradictions detected: {}\n",
                    stats.contradictions
                ));
            }
        }

        Ok(output)
    }
}

/// Formats a single source memory entry for display.
fn format_source_memory_entry<I: crate::storage::traits::IndexBackend>(
    index: &I,
    source_id: &crate::models::MemoryId,
    position: usize,
) -> Result<String> {
    let mut entry = String::new();

    match index.get_memory(source_id)? {
        Some(source_memory) => {
            entry.push_str(&format!("{}. **{}**\n", position, source_id.as_str()));
            entry.push_str(&format!("   - Namespace: {:?}\n", source_memory.namespace));
            if !source_memory.tags.is_empty() {
                entry.push_str(&format!("   - Tags: {}\n", source_memory.tags.join(", ")));
            }
            // Show truncated content (first 150 chars)
            let preview: &str = if source_memory.content.len() > 150 {
                &source_memory.content[..150]
            } else {
                &source_memory.content
            };
            if source_memory.content.len() > 150 {
                entry.push_str(&format!("   - Content: {preview}...\n"));
            } else {
                entry.push_str(&format!("   - Content: {preview}\n"));
            }
            entry.push('\n');
        },
        None => {
            entry.push_str(&format!(
                "{}. {} (not found)\n\n",
                position,
                source_id.as_str()
            ));
        },
    }

    Ok(entry)
}

/// Executes the get summary tool.
/// Retrieves a summary memory and its linked source memories.
pub fn execute_get_summary(arguments: Value) -> Result<ToolResult> {
    use crate::mcp::tool_types::GetSummaryArgs;
    use crate::models::{EdgeType, MemoryId};
    use crate::storage::traits::IndexBackend;

    let args: GetSummaryArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    let services = ServiceContainer::from_current_dir_or_user()?;
    let index = services.index()?;

    // Get the summary memory using IndexBackend::get_memory
    let memory_id = MemoryId::new(args.memory_id.clone());
    let memory = index
        .get_memory(&memory_id)?
        .ok_or_else(|| Error::InvalidInput(format!("Memory not found: {}", args.memory_id)))?;

    // Check if this is actually a summary
    if !memory.is_summary {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "Memory '{}' is not a summary node.\n\nTo retrieve a regular memory, use subcog_recall instead.",
                    args.memory_id
                ),
            }],
            is_error: true,
        });
    }

    let mut output = String::from("**Summary Memory**\n\n");

    // Display summary content
    output.push_str(&format!("**ID:** {}\n", memory.id.as_str()));
    output.push_str(&format!("**Namespace:** {:?}\n", memory.namespace));
    if !memory.tags.is_empty() {
        output.push_str(&format!("**Tags:** {}\n", memory.tags.join(", ")));
    }
    if let Some(ts) = memory.consolidation_timestamp {
        output.push_str(&format!("**Consolidated at:** {ts}\n"));
    }
    output.push_str(&format!("\n**Summary:**\n{}\n\n", memory.content));

    // Query for source memories using SourceOf edges
    let source_ids = index.query_edges(&memory_id, EdgeType::SourceOf)?;

    if source_ids.is_empty() {
        output.push_str(
            "**Source Memories:** None (edges not stored or summary created without service)\n",
        );
    } else {
        output.push_str(&format!("**Source Memories ({}):**\n\n", source_ids.len()));

        // Retrieve each source memory using IndexBackend::get_memory
        for (idx, source_id) in source_ids.iter().enumerate() {
            let entry = format_source_memory_entry(&index, source_id, idx + 1)?;
            output.push_str(&entry);
        }
    }

    Ok(ToolResult {
        content: vec![ToolContent::Text { text: output }],
        is_error: false,
    })
}

/// Executes the enrich tool.
/// Returns a sampling request for the LLM to enrich a memory.
pub fn execute_enrich(arguments: Value) -> Result<ToolResult> {
    let args: EnrichArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    let enrich_tags = args.enrich_tags.unwrap_or(true);
    let enrich_structure = args.enrich_structure.unwrap_or(true);
    let add_context = args.add_context.unwrap_or(false);

    // For now, return a sampling request template
    // In full implementation, would fetch the memory by ID first
    let mut enrichments = Vec::new();
    if enrich_tags {
        enrichments.push("- Generate relevant tags for searchability");
    }
    if enrich_structure {
        enrichments
            .push("- Restructure content for clarity (add context, rationale, consequences)");
    }
    if add_context {
        enrichments.push("- Infer and add missing context or rationale");
    }

    let sampling_prompt = format!(
        "Enrich the memory with ID '{}'.\n\nRequested enrichments:\n{}\n\nProvide the enriched version with:\n1. Improved content structure\n2. Suggested tags (if requested)\n3. Inferred namespace (if content suggests different category)",
        args.memory_id,
        enrichments.join("\n")
    );

    Ok(ToolResult {
        content: vec![ToolContent::Text {
            text: format!(
                "SAMPLING_REQUEST\n\nmemory_id: {}\nenrich_tags: {}\nenrich_structure: {}\nadd_context: {}\n\nprompt: {}",
                args.memory_id, enrich_tags, enrich_structure, add_context, sampling_prompt
            ),
        }],
        is_error: false,
    })
}

/// Executes the reindex tool.
pub fn execute_reindex(arguments: Value) -> Result<ToolResult> {
    let args: ReindexArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    let services = match args.repo_path {
        Some(repo_path) => ServiceContainer::for_repo(std::path::PathBuf::from(repo_path), None)?,
        None => ServiceContainer::from_current_dir_or_user()?,
    };

    let scope_label = match services.repo_path() {
        Some(repo_root) => format!("Repository: {}", repo_root.display()),
        None => "Scope: user".to_string(),
    };

    match services.reindex() {
        Ok(count) => Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "Reindex completed successfully!\n\nMemories indexed: {count}\n{scope_label}"
                ),
            }],
            is_error: false,
        }),
        Err(e) => Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!("Reindex failed: {e}"),
            }],
            is_error: true,
        }),
    }
}

/// Executes the GDPR data export tool.
///
/// Implements GDPR Article 20 (Right to Data Portability).
/// Returns all user data in a portable JSON format.
pub fn execute_gdpr_export(_arguments: Value) -> Result<ToolResult> {
    let services = ServiceContainer::from_current_dir_or_user()?;
    let data_subject = services.data_subject()?;

    match data_subject.export_user_data() {
        Ok(export) => {
            // Format the export as pretty JSON for readability
            let json =
                serde_json::to_string_pretty(&export).map_err(|e| Error::OperationFailed {
                    operation: "serialize_export".to_string(),
                    cause: e.to_string(),
                })?;

            Ok(ToolResult {
                content: vec![ToolContent::Text {
                    text: format!(
                        "GDPR Data Export (Article 20 - Right to Data Portability)\n\n\
                         Exported {} memories at {}\n\
                         Format: {}\n\
                         Generator: {} v{}\n\n\
                         ---\n\n{}",
                        export.memory_count,
                        export.exported_at,
                        export.metadata.format,
                        export.metadata.generator,
                        export.metadata.generator_version,
                        json
                    ),
                }],
                is_error: false,
            })
        },
        Err(e) => Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!("GDPR export failed: {e}"),
            }],
            is_error: true,
        }),
    }
}

// ============================================================================
// Core CRUD Handlers (Industry Parity: Mem0, Zep, LangMem)
// ============================================================================

/// Executes the get tool - retrieves a memory by ID.
///
/// This is a fundamental CRUD operation that provides direct access to
/// a specific memory without requiring a search query.
pub fn execute_get(arguments: Value) -> Result<ToolResult> {
    use crate::storage::traits::IndexBackend;

    let args: GetArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    let services = ServiceContainer::from_current_dir_or_user()?;
    let index = services.index()?;

    let memory_id = MemoryId::new(&args.memory_id);

    match index.get_memory(&memory_id)? {
        Some(memory) => {
            // Build URN for the memory
            let domain_part = if memory.domain.is_project_scoped() {
                "project".to_string()
            } else {
                memory.domain.to_string()
            };
            let urn = format!(
                "subcog://{}/{}/{}",
                domain_part, memory.namespace, memory.id
            );

            // Format tags
            let tags_display = if memory.tags.is_empty() {
                "None".to_string()
            } else {
                memory.tags.join(", ")
            };

            // Format status
            let status_display = match memory.status {
                MemoryStatus::Active => "Active",
                MemoryStatus::Archived => "Archived",
                MemoryStatus::Superseded => "Superseded",
                MemoryStatus::Pending => "Pending",
                MemoryStatus::Deleted => "Deleted",
                MemoryStatus::Tombstoned => "Tombstoned (soft deleted)",
                MemoryStatus::Consolidated => "Consolidated",
            };

            let output = format!(
                "**Memory: {}**\n\n\
                 **URN:** {}\n\
                 **Namespace:** {:?}\n\
                 **Status:** {}\n\
                 **Tags:** {}\n\
                 **Created:** {}\n\
                 **Updated:** {}\n\
                 {}\n\
                 **Content:**\n{}",
                memory.id.as_str(),
                urn,
                memory.namespace,
                status_display,
                tags_display,
                memory.created_at,
                memory.updated_at,
                memory
                    .source
                    .as_ref()
                    .map_or(String::new(), |s| format!("**Source:** {s}\n")),
                memory.content
            );

            Ok(ToolResult {
                content: vec![ToolContent::Text { text: output }],
                is_error: false,
            })
        },
        None => Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!("Memory not found: {}", args.memory_id),
            }],
            is_error: true,
        }),
    }
}

/// Executes the delete tool - soft or hard deletes a memory.
///
/// Defaults to soft delete (tombstone) which can be restored later.
/// Use `hard: true` for permanent deletion.
pub fn execute_delete(arguments: Value) -> Result<ToolResult> {
    use crate::storage::traits::IndexBackend;
    use chrono::TimeZone;

    let args: DeleteArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    let services = ServiceContainer::from_current_dir_or_user()?;
    let index = services.index()?;

    let memory_id = MemoryId::new(&args.memory_id);

    // First check if memory exists
    let Some(memory) = index.get_memory(&memory_id)? else {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!("Memory not found: {}", args.memory_id),
            }],
            is_error: true,
        });
    };

    if args.hard {
        // Hard delete - permanent removal
        if index.remove(&memory_id)? {
            record_event(MemoryEvent::Deleted {
                meta: EventMeta::new("mcp.delete", current_request_id()),
                memory_id,
                reason: "mcp.subcog_delete --hard".to_string(),
            });

            metrics::counter!("mcp_delete_hard_total").increment(1);

            Ok(ToolResult {
                content: vec![ToolContent::Text {
                    text: format!(
                        "Memory permanently deleted: {}\n\n\
                         ⚠️ This action is irreversible.",
                        args.memory_id
                    ),
                }],
                is_error: false,
            })
        } else {
            Ok(ToolResult {
                content: vec![ToolContent::Text {
                    text: format!("Failed to delete memory: {}", args.memory_id),
                }],
                is_error: true,
            })
        }
    } else {
        // Soft delete - tombstone the memory
        let now = crate::current_timestamp();
        let now_i64 = i64::try_from(now).unwrap_or(i64::MAX);
        let now_dt = chrono::Utc
            .timestamp_opt(now_i64, 0)
            .single()
            .unwrap_or_else(chrono::Utc::now);

        let mut updated_memory = memory;
        updated_memory.status = MemoryStatus::Tombstoned;
        updated_memory.tombstoned_at = Some(now_dt);
        updated_memory.updated_at = now;

        // Re-index with updated status (INSERT OR REPLACE)
        index.index(&updated_memory)?;

        record_event(MemoryEvent::Updated {
            meta: EventMeta::with_timestamp("mcp.delete", current_request_id(), now),
            memory_id,
            modified_fields: vec!["status".to_string(), "tombstoned_at".to_string()],
        });

        metrics::counter!("mcp_delete_soft_total").increment(1);

        Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "Memory tombstoned (soft deleted): {}\n\n\
                     The memory can be restored or permanently purged with `subcog gc --purge`.",
                    args.memory_id
                ),
            }],
            is_error: false,
        })
    }
}

/// Executes the update tool - modifies an existing memory's content and/or tags.
///
/// This is a partial update operation - only provided fields are changed.
pub fn execute_update(arguments: Value) -> Result<ToolResult> {
    use crate::storage::traits::IndexBackend;

    let args: UpdateArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    // Validate that at least one field is being updated
    if args.content.is_none() && args.tags.is_none() {
        return Err(Error::InvalidInput(
            "At least one of 'content' or 'tags' must be provided for update".to_string(),
        ));
    }

    // SEC-M5: Validate content length if provided
    if let Some(ref content) = args.content {
        validate_input_length(content, "content", MAX_CONTENT_LENGTH)?;
    }

    let services = ServiceContainer::from_current_dir_or_user()?;
    let index = services.index()?;

    let memory_id = MemoryId::new(&args.memory_id);

    // Get existing memory
    let Some(mut memory) = index.get_memory(&memory_id)? else {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!("Memory not found: {}", args.memory_id),
            }],
            is_error: true,
        });
    };

    // Check if memory is tombstoned
    if memory.status == MemoryStatus::Tombstoned {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "Cannot update tombstoned memory: {}\n\n\
                     Restore the memory first or create a new one.",
                    args.memory_id
                ),
            }],
            is_error: true,
        });
    }

    // Track what we're updating for the audit log
    let mut modified_fields = Vec::new();

    // Update content if provided
    if let Some(new_content) = args.content {
        memory.content = new_content;
        modified_fields.push("content".to_string());
    }

    // Update tags if provided
    if let Some(new_tags) = args.tags {
        memory.tags = new_tags;
        modified_fields.push("tags".to_string());
    }

    // Update timestamp
    let now = crate::current_timestamp();
    memory.updated_at = now;
    modified_fields.push("updated_at".to_string());

    // Re-index the memory (INSERT OR REPLACE)
    index.index(&memory)?;

    record_event(MemoryEvent::Updated {
        meta: EventMeta::with_timestamp("mcp.update", current_request_id(), now),
        memory_id,
        modified_fields: modified_fields.clone(),
    });

    metrics::counter!("mcp_update_total").increment(1);

    // Format response
    let fields_updated = modified_fields
        .iter()
        .filter(|f| *f != "updated_at")
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");

    let tags_display = if memory.tags.is_empty() {
        "None".to_string()
    } else {
        memory.tags.join(", ")
    };

    Ok(ToolResult {
        content: vec![ToolContent::Text {
            text: format!(
                "Memory updated: {}\n\n\
                 **Updated fields:** {}\n\
                 **Current tags:** {}\n\
                 **Updated at:** {}",
                args.memory_id, fields_updated, tags_display, now
            ),
        }],
        is_error: false,
    })
}

// ============================================================================
// Mem0 Parity: List, DeleteAll, Restore, History
// ============================================================================

/// Executes the list tool - lists memories with optional filtering and pagination.
///
/// Unlike recall, this doesn't require a search query. Matches Mem0's `get_all()`
/// and Zep's `list_memories()` patterns.
pub fn execute_list(arguments: Value) -> Result<ToolResult> {
    use crate::mcp::tool_types::ListArgs;

    let args: ListArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    // Build filter from the filter query string
    let mut filter = if let Some(filter_query) = &args.filter {
        parse_filter_query(filter_query)
    } else {
        SearchFilter::new()
    };

    // Apply user_id filter via tag if provided
    if let Some(ref user_id) = args.user_id {
        filter = filter.with_tag(format!("user:{user_id}"));
    }

    // Apply agent_id filter via tag if provided
    if let Some(ref agent_id) = args.agent_id {
        filter = filter.with_tag(format!("agent:{agent_id}"));
    }

    let limit = args.limit.unwrap_or(50).min(1000);
    let offset = args.offset.unwrap_or(0);

    let services = ServiceContainer::from_current_dir_or_user()?;
    let recall = services.recall()?;

    // Use list_all which returns all matching memories without a search query
    // We fetch limit + offset and then skip manually for pagination
    let fetch_count = limit.saturating_add(offset);
    let result = recall.list_all(&filter, fetch_count)?;

    // Apply offset pagination manually
    let paginated_memories: Vec<_> = result
        .memories
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect();
    let displayed_count = paginated_memories.len();

    // Build filter description for output
    let filter_desc = build_filter_description(&filter);

    let mut output = format!(
        "**Memory List** (showing {displayed_count} of {} total, offset: {offset}{filter_desc})\n\n",
        result.total_count
    );

    if paginated_memories.is_empty() {
        output.push_str("No memories found matching the criteria.\n");
    } else {
        for (i, hit) in paginated_memories.iter().enumerate() {
            let tags_display = if hit.memory.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", hit.memory.tags.join(", "))
            };

            // Build URN
            let domain_part = if hit.memory.domain.is_project_scoped() {
                "project".to_string()
            } else {
                hit.memory.domain.to_string()
            };
            let urn = format!(
                "subcog://{}/{}/{}",
                domain_part, hit.memory.namespace, hit.memory.id
            );

            // Status indicator for non-active memories
            let status_indicator = match hit.memory.status {
                MemoryStatus::Tombstoned => " ⚠️ [tombstoned]",
                MemoryStatus::Archived => " 📦 [archived]",
                MemoryStatus::Superseded => " ↩️ [superseded]",
                _ => "",
            };

            output.push_str(&format!(
                "{}. {}{}{}\n",
                offset + i + 1,
                urn,
                tags_display,
                status_indicator
            ));
        }
    }

    // Add pagination hint if there are more results
    if result.total_count > offset + displayed_count {
        output.push_str(&format!(
            "\n_Use offset={} to see more results._",
            offset + displayed_count
        ));
    }

    metrics::counter!("mcp_list_total").increment(1);

    Ok(ToolResult {
        content: vec![ToolContent::Text { text: output }],
        is_error: false,
    })
}

/// Executes the `delete_all` tool - bulk deletes memories matching filter criteria.
///
/// Defaults to dry-run mode for safety. Implements Mem0's `delete_all()` pattern.
#[allow(clippy::too_many_lines)]
pub fn execute_delete_all(arguments: Value) -> Result<ToolResult> {
    use crate::mcp::tool_types::DeleteAllArgs;

    let args: DeleteAllArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    // Build filter from the filter query string
    let mut filter = args
        .filter
        .as_ref()
        .map_or_else(SearchFilter::new, |q| parse_filter_query(q));

    // Apply user_id filter via tag if provided
    if let Some(ref user_id) = args.user_id {
        filter = filter.with_tag(format!("user:{user_id}"));
    }

    // Safety check: require at least some filter criteria
    if !has_any_filter_criteria(&filter, args.user_id.is_some()) {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: "**Safety check failed**: At least one filter criterion is required for bulk deletion.\n\n\
                       Examples:\n\
                       - `filter: \"ns:decisions\"` - delete all decisions\n\
                       - `filter: \"tag:deprecated\"` - delete tagged memories\n\
                       - `user_id: \"user123\"` - delete user's memories".to_string(),
            }],
            is_error: true,
        });
    }

    let services = ServiceContainer::from_current_dir_or_user()?;
    let recall = services.recall()?;
    let index = services.index()?;

    // Fetch all matching memories (up to 10000 for safety)
    let result = recall.list_all(&filter, 10000)?;
    let matching_count = result.memories.len();

    if matching_count == 0 {
        let msg = if args.dry_run {
            "[DRY-RUN] No memories would be deleted - no memories found matching the filter criteria."
        } else {
            "No memories found matching the filter criteria."
        };
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: msg.to_string(),
            }],
            is_error: false,
        });
    }

    let filter_desc = build_filter_description(&filter);

    if args.dry_run {
        let output =
            build_dry_run_output(&result.memories, matching_count, &filter_desc, args.hard);
        return Ok(ToolResult {
            content: vec![ToolContent::Text { text: output }],
            is_error: false,
        });
    }

    // Execute the deletion
    let (deleted_count, failed_count) = execute_bulk_delete(&result.memories, args.hard, &index)?;

    let output = build_delete_result_output(deleted_count, failed_count, &filter_desc, args.hard);

    metrics::counter!(
        "mcp_delete_all_total",
        "type" => if args.hard { "hard" } else { "soft" }
    )
    .increment(deleted_count);

    Ok(ToolResult {
        content: vec![ToolContent::Text { text: output }],
        is_error: false,
    })
}

/// Checks if the filter has any criteria set.
const fn has_any_filter_criteria(filter: &SearchFilter, has_user_id: bool) -> bool {
    !filter.namespaces.is_empty()
        || !filter.tags.is_empty()
        || !filter.excluded_tags.is_empty()
        || has_user_id
        || filter.created_after.is_some()
        || filter.created_before.is_some()
}

/// Builds the dry-run output message.
fn build_dry_run_output(
    memories: &[crate::models::SearchHit],
    matching_count: usize,
    filter_desc: &str,
    hard: bool,
) -> String {
    let delete_type = if hard {
        "permanently delete"
    } else {
        "tombstone (soft delete)"
    };

    let mut output = format!(
        "**Dry-run: Would delete {matching_count} memories**{filter_desc}\n\n\
         Action: {delete_type}\n\n"
    );

    // Show first 10 memories that would be deleted
    let preview_count = matching_count.min(10);
    output.push_str(&format!("Preview (first {preview_count}):\n"));

    for hit in memories.iter().take(preview_count) {
        let domain_part = if hit.memory.domain.is_project_scoped() {
            "project".to_string()
        } else {
            hit.memory.domain.to_string()
        };
        let urn = format!(
            "subcog://{}/{}/{}",
            domain_part, hit.memory.namespace, hit.memory.id
        );
        output.push_str(&format!("  - {urn}\n"));
    }

    if matching_count > preview_count {
        output.push_str(&format!(
            "  ... and {} more\n",
            matching_count - preview_count
        ));
    }

    output.push_str("\n_Set `dry_run: false` to execute the deletion._");
    output
}

/// Executes bulk deletion and returns (`deleted_count`, `failed_count`).
fn execute_bulk_delete(
    memories: &[crate::models::SearchHit],
    hard: bool,
    index: &crate::storage::index::SqliteBackend,
) -> Result<(u64, u64)> {
    let mut deleted_count = 0u64;
    let mut failed_count = 0u64;
    let now = crate::current_timestamp();

    for hit in memories {
        let memory_id = hit.memory.id.clone();
        let success = if hard {
            delete_memory_hard(index, &memory_id, now)
        } else {
            delete_memory_soft(index, &hit.memory, now)
        };

        if success {
            deleted_count += 1;
        } else {
            failed_count += 1;
        }
    }

    Ok((deleted_count, failed_count))
}

/// Performs hard (permanent) deletion of a memory.
fn delete_memory_hard(
    index: &crate::storage::index::SqliteBackend,
    memory_id: &MemoryId,
    now: u64,
) -> bool {
    use crate::storage::traits::IndexBackend;

    match index.remove(memory_id) {
        Ok(true) => {
            record_event(MemoryEvent::Deleted {
                meta: EventMeta::with_timestamp("mcp.delete_all", current_request_id(), now),
                memory_id: memory_id.clone(),
                reason: "mcp.subcog_delete_all --hard".to_string(),
            });
            true
        },
        _ => false,
    }
}

/// Performs soft deletion (tombstone) of a memory.
fn delete_memory_soft(
    index: &crate::storage::index::SqliteBackend,
    memory: &crate::models::Memory,
    now: u64,
) -> bool {
    use crate::storage::traits::IndexBackend;
    use chrono::TimeZone;

    let now_i64 = i64::try_from(now).unwrap_or(i64::MAX);
    let now_dt = chrono::Utc
        .timestamp_opt(now_i64, 0)
        .single()
        .unwrap_or_else(chrono::Utc::now);

    let mut updated_memory = memory.clone();
    updated_memory.status = MemoryStatus::Tombstoned;
    updated_memory.tombstoned_at = Some(now_dt);
    updated_memory.updated_at = now;

    match index.index(&updated_memory) {
        Ok(()) => {
            record_event(MemoryEvent::Updated {
                meta: EventMeta::with_timestamp("mcp.delete_all", current_request_id(), now),
                memory_id: memory.id.clone(),
                modified_fields: vec!["status".to_string(), "tombstoned_at".to_string()],
            });
            true
        },
        Err(_) => false,
    }
}

/// Builds the deletion result output message.
fn build_delete_result_output(
    deleted_count: u64,
    failed_count: u64,
    filter_desc: &str,
    hard: bool,
) -> String {
    let delete_type = if hard {
        "permanently deleted"
    } else {
        "tombstoned"
    };

    let mut output = format!(
        "**Bulk delete completed**{filter_desc}\n\n\
         - {delete_type}: {deleted_count}\n"
    );

    if failed_count > 0 {
        output.push_str(&format!("- Failed: {failed_count}\n"));
    }

    if !hard {
        output.push_str(
            "\n_Tombstoned memories can be restored with `subcog_restore` \
             or permanently removed with `subcog gc --purge`._",
        );
    }

    output
}

/// Executes the restore tool - restores a tombstoned (soft-deleted) memory.
///
/// Sets the memory status back to Active and clears the tombstone timestamp.
pub fn execute_restore(arguments: Value) -> Result<ToolResult> {
    use crate::mcp::tool_types::RestoreArgs;
    use crate::storage::traits::IndexBackend;

    let args: RestoreArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    let services = ServiceContainer::from_current_dir_or_user()?;
    let index = services.index()?;

    let memory_id = MemoryId::new(&args.memory_id);

    // Get existing memory
    let Some(mut memory) = index.get_memory(&memory_id)? else {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!("Memory not found: {}", args.memory_id),
            }],
            is_error: true,
        });
    };

    // Check if memory is actually tombstoned
    if memory.status != MemoryStatus::Tombstoned {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "Memory '{}' is not tombstoned (current status: {:?}).\n\n\
                     Only tombstoned memories can be restored.",
                    args.memory_id, memory.status
                ),
            }],
            is_error: true,
        });
    }

    // Restore the memory
    let now = crate::current_timestamp();
    memory.status = MemoryStatus::Active;
    memory.tombstoned_at = None;
    memory.updated_at = now;

    // Re-index with updated status
    index.index(&memory)?;

    record_event(MemoryEvent::Updated {
        meta: EventMeta::with_timestamp("mcp.restore", current_request_id(), now),
        memory_id,
        modified_fields: vec!["status".to_string(), "tombstoned_at".to_string()],
    });

    metrics::counter!("mcp_restore_total").increment(1);

    // Build URN for response
    let domain_part = if memory.domain.is_project_scoped() {
        "project".to_string()
    } else {
        memory.domain.to_string()
    };
    let urn = format!(
        "subcog://{}/{}/{}",
        domain_part, memory.namespace, memory.id
    );

    Ok(ToolResult {
        content: vec![ToolContent::Text {
            text: format!(
                "Memory restored: {}\n\n\
                 **URN:** {}\n\
                 **Status:** Active\n\
                 **Restored at:** {}",
                args.memory_id, urn, now
            ),
        }],
        is_error: false,
    })
}

/// Executes the history tool - retrieves change history for a memory.
///
/// Queries the event log for events related to the specified memory ID.
/// Note: This provides audit trail visibility but doesn't store full version snapshots.
pub fn execute_history(arguments: Value) -> Result<ToolResult> {
    use crate::mcp::tool_types::HistoryArgs;
    use crate::storage::traits::IndexBackend;

    let args: HistoryArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    let services = ServiceContainer::from_current_dir_or_user()?;
    let index = services.index()?;

    let memory_id = MemoryId::new(&args.memory_id);
    let _limit = args.limit.unwrap_or(20).min(100);

    // Query event log for this memory
    // Note: The event log is currently stored via tracing/metrics, not in SQLite.
    // For a full implementation, we'd need to query a persistent event store.
    // For now, we provide status info and suggest using external log aggregation.

    let mut output = format!("**Memory History: {}**\n\n", args.memory_id);

    if let Some(memory) = index.get_memory(&memory_id)? {
        // Build basic history from memory metadata
        output.push_str("**Current State:**\n");
        output.push_str(&format!("- Status: {:?}\n", memory.status));
        output.push_str(&format!("- Created: {}\n", memory.created_at));
        output.push_str(&format!("- Last Updated: {}\n", memory.updated_at));

        if let Some(ref tombstoned_at) = memory.tombstoned_at {
            output.push_str(&format!("- Tombstoned: {tombstoned_at}\n"));
        }

        if memory.is_summary {
            output.push_str("- Type: Summary node\n");
            if let Some(ref source_ids) = memory.source_memory_ids {
                output.push_str(&format!(
                    "- Consolidated from: {} memories\n",
                    source_ids.len()
                ));
            }
        }

        output.push_str("\n**Event Types Tracked:**\n");
        output.push_str("- `Captured`: Initial creation\n");
        output.push_str("- `Updated`: Content or tag changes\n");
        output.push_str("- `Deleted`: Soft or hard deletion\n");
        output.push_str("- `Archived`: Archival status change\n");

        output.push_str(&format!(
            "\n_Note: Full event history requires log aggregation. \
             Events are emitted via tracing and can be queried from your \
             log backend (e.g., Elasticsearch, Datadog, Splunk). \
             Filter by `memory_id=\"{}\"`._",
            args.memory_id
        ));
    } else {
        output.push_str("⚠️ Memory not found in current storage.\n\n");
        output.push_str("The memory may have been:\n");
        output.push_str("- Permanently deleted (hard delete)\n");
        output.push_str("- Never existed with this ID\n");
        output.push_str("- Stored in a different domain scope\n");

        output.push_str(&format!(
            "\n_Check your log backend for historical events with `memory_id=\"{}\"`._",
            args.memory_id
        ));
    }

    metrics::counter!("mcp_history_total").increment(1);

    Ok(ToolResult {
        content: vec![ToolContent::Text { text: output }],
        is_error: false,
    })
}

/// Formats the status section for init output.
fn format_init_status(services: &ServiceContainer) -> String {
    let mut output = String::new();

    let Ok(recall) = services.recall() else {
        output.push_str("- **Status**: ✅ Healthy\n");
        return output;
    };

    let filter = SearchFilter::new();
    let Ok(result) = recall.search("*", SearchMode::Text, &filter, 1000) else {
        output.push_str("- **Status**: ✅ Healthy\n");
        return output;
    };

    output.push_str(&format!(
        "- **Total memories**: {}\n",
        result.memories.len()
    ));

    // Count by namespace
    let mut ns_counts = std::collections::HashMap::new();
    for hit in &result.memories {
        *ns_counts
            .entry(format!("{:?}", hit.memory.namespace))
            .or_insert(0) += 1;
    }
    if !ns_counts.is_empty() {
        output.push_str("- **By namespace**: ");
        let ns_summary: Vec<String> = ns_counts
            .iter()
            .map(|(ns, count)| format!("{ns}: {count}"))
            .collect();
        output.push_str(&ns_summary.join(", "));
        output.push('\n');
    }
    output.push_str("- **Status**: ✅ Healthy\n");
    output
}

/// Formats the recall section for init output.
fn format_init_recall(services: &ServiceContainer, query: &str, limit: usize) -> String {
    let mut output = String::new();

    let Ok(recall) = services.recall() else {
        output.push_str("_Could not access recall service._\n");
        return output;
    };

    let filter = SearchFilter::new();
    match recall.search(query, SearchMode::Hybrid, &filter, limit) {
        Ok(result) if !result.memories.is_empty() => {
            output.push_str(&format!(
                "Found **{}** relevant memories for context:\n\n",
                result.memories.len()
            ));
            for (i, hit) in result.memories.iter().enumerate() {
                let preview = if hit.memory.content.len() > 150 {
                    format!("{}...", &hit.memory.content[..150])
                } else {
                    hit.memory.content.clone()
                };
                output.push_str(&format!(
                    "{}. **{:?}** (score: {:.2})\n   {}\n\n",
                    i + 1,
                    hit.memory.namespace,
                    hit.score,
                    preview.replace('\n', " ")
                ));
            }
        },
        Ok(_) => {
            output.push_str("_No existing context memories found. This may be a new project._\n\n");
            output.push_str("**Tip**: Capture decisions, patterns, and learnings as you work!\n");
        },
        Err(e) => {
            output.push_str(&format!("_Could not recall context: {e}_\n"));
        },
    }
    output
}

/// Executes the init tool for session initialization.
///
/// Combines `prompt_understanding`, status, and optional context recall into
/// a single initialization call. Marks the session as initialized.
pub fn execute_init(arguments: Value) -> Result<ToolResult> {
    let args: InitArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    // Mark session as initialized
    crate::mcp::session::mark_initialized();

    let mut output = String::new();

    // Section 1: Guidance (prompt_understanding)
    output.push_str("# Subcog Session Initialized\n\n");
    output.push_str("## 📚 Usage Guidance\n\n");
    output.push_str(PROMPT_UNDERSTANDING);
    output.push_str("\n\n---\n\n");

    // Section 2: Status check
    output.push_str("## 🔍 System Status\n\n");
    let services_result = ServiceContainer::from_current_dir_or_user();
    match &services_result {
        Ok(services) => output.push_str(&format_init_status(services)),
        Err(e) => output.push_str(&format!("- **Status**: ⚠️ Degraded ({e})\n")),
    }
    output.push_str("\n---\n\n");

    // Section 3: Optional context recall
    if args.include_recall {
        output.push_str("## 🧠 Project Context\n\n");
        let query = args
            .recall_query
            .unwrap_or_else(|| "project setup OR architecture OR conventions".to_string());
        let limit = args.recall_limit.unwrap_or(5).min(20) as usize;

        if let Ok(services) = &services_result {
            output.push_str(&format_init_recall(services, &query, limit));
        }
    } else {
        output.push_str("_Context recall skipped (include_recall=false)_\n");
    }

    output.push_str("\n---\n\n");
    output.push_str("✅ **Session initialized.** You now have full memory context.\n\n");
    output.push_str("**Next steps**:\n");
    output.push_str("- Use `subcog_recall` to search for relevant memories\n");
    output.push_str("- Use `subcog_capture` to store decisions, patterns, and learnings\n");
    output.push_str("- Use `subcog_status` for detailed health checks\n");

    metrics::counter!("mcp_init_total").increment(1);

    Ok(ToolResult {
        content: vec![ToolContent::Text { text: output }],
        is_error: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Memory, MemoryId, MemoryStatus, Namespace};
    use crate::services::RecallService;
    use crate::storage::index::SqliteBackend;
    use crate::storage::traits::IndexBackend;

    #[test]
    fn test_validate_input_length_within_limit() {
        let input = "a".repeat(100);
        assert!(validate_input_length(&input, "test", 1000).is_ok());
    }

    #[test]
    fn test_validate_input_length_at_limit() {
        let input = "a".repeat(1000);
        assert!(validate_input_length(&input, "test", 1000).is_ok());
    }

    #[test]
    fn test_validate_input_length_exceeds_limit() {
        let input = "a".repeat(1001);
        let result = validate_input_length(&input, "test", 1000);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
        assert!(err.to_string().contains("exceeds maximum length"));
        assert!(err.to_string().contains("1001 > 1000"));
    }

    #[test]
    fn test_validate_input_length_empty() {
        assert!(validate_input_length("", "test", 1000).is_ok());
    }

    #[test]
    fn test_max_content_length_constant() {
        // Verify constant is 1 MB
        assert_eq!(MAX_CONTENT_LENGTH, 1_048_576);
    }

    #[test]
    fn test_max_query_length_constant() {
        // Verify constant is 10 KB
        assert_eq!(MAX_QUERY_LENGTH, 10_240);
    }

    #[test]
    fn test_capture_rejects_oversized_content() {
        let oversized_content = "x".repeat(MAX_CONTENT_LENGTH + 1);
        let args = serde_json::json!({
            "content": oversized_content,
            "namespace": "decisions"
        });

        let result = execute_capture(args);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
        assert!(err.to_string().contains("content"));
    }

    #[test]
    fn test_recall_rejects_oversized_query() {
        let oversized_query = "x".repeat(MAX_QUERY_LENGTH + 1);
        let args = serde_json::json!({
            "query": oversized_query
        });

        let result = execute_recall(args);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
        assert!(err.to_string().contains("query"));
    }

    fn create_test_memory(id: &str, content: &str, namespace: Namespace) -> Memory {
        Memory {
            id: MemoryId::new(id),
            content: content.to_string(),
            namespace,
            domain: Domain::new(),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: 1,
            updated_at: 1,
            tombstoned_at: None,
            expires_at: None,
            embedding: None,
            tags: Vec::new(),
            #[cfg(feature = "group-scope")]
            group_id: None,
            source: None,
            is_summary: false,
            source_memory_ids: None,
            consolidation_timestamp: None,
        }
    }

    #[test]
    fn test_consolidate_candidates_uses_list_all_for_wildcard() {
        let backend = SqliteBackend::in_memory().unwrap();
        let memory = create_test_memory("id1", "hello world", Namespace::Decisions);
        backend.index(&memory).unwrap();

        let recall = RecallService::with_index(backend);
        let filter = SearchFilter::new().with_namespace(Namespace::Decisions);
        let result = fetch_consolidation_candidates(&recall, &filter, Some("*"), 10).unwrap();

        assert_eq!(result.mode, SearchMode::Text);
        assert_eq!(result.memories.len(), 1);
    }

    #[test]
    fn test_consolidate_candidates_uses_search_for_query() {
        let backend = SqliteBackend::in_memory().unwrap();
        let memory = create_test_memory("id1", "hello world", Namespace::Decisions);
        backend.index(&memory).unwrap();

        let recall = RecallService::with_index(backend);
        let filter = SearchFilter::new().with_namespace(Namespace::Decisions);
        let result = fetch_consolidation_candidates(&recall, &filter, Some("hello"), 10).unwrap();

        assert_eq!(result.mode, SearchMode::Hybrid);
        assert_eq!(result.memories.len(), 1);
    }
}
