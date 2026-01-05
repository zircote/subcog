//! Core tool execution handlers.
//!
//! Contains handlers for subcog's core memory operations:
//! capture, recall, status, namespaces, consolidate, enrich, sync, reindex.

use crate::mcp::tool_types::{
    CaptureArgs, ConsolidateArgs, EnrichArgs, RecallArgs, ReindexArgs, SyncArgs,
    build_filter_description, format_content_for_detail, parse_namespace, parse_search_mode,
};
use crate::models::{CaptureRequest, DetailLevel, Domain, SearchFilter, SearchMode, SearchResult};
use crate::services::{ServiceContainer, parse_filter_query};
use crate::{Error, Result};
use serde_json::Value;

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

    // Use context-aware domain: project if in git repo, user if not
    let request = CaptureRequest {
        content: args.content,
        namespace,
        domain: Domain::default_for_context(),
        tags: args.tags.unwrap_or_default(),
        source: args.source,
        skip_security_check: false,
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
pub fn execute_recall(arguments: Value) -> Result<ToolResult> {
    let args: RecallArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    // SEC-M5: Validate query length before processing
    validate_input_length(&args.query, "query", MAX_QUERY_LENGTH)?;

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

    let limit = args.limit.unwrap_or(10).min(50);

    let services = ServiceContainer::from_current_dir_or_user()?;
    let recall = services.recall()?;

    // Use list_all for wildcard queries or filter-only queries
    // Use search for actual text queries
    let result = if args.query == "*" || args.query.is_empty() {
        recall.list_all(&filter, limit)?
    } else {
        recall.search(&args.query, mode, &filter, limit)?
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

/// Executes the status tool.
pub fn execute_status(_arguments: Value) -> Result<ToolResult> {
    // For now, return basic status
    let status = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "status": "operational",
        "backends": {
            "persistence": "sqlite",
            "index": "sqlite-fts5",
            "vector": "usearch"
        },
        "features": {
            "semantic_search": true,
            "secret_detection": true,
            "hooks": true
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
/// Returns a sampling request for the LLM to perform consolidation.
pub fn execute_consolidate(arguments: Value) -> Result<ToolResult> {
    let args: ConsolidateArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    // SEC-M5: Validate query length if provided
    if let Some(ref query) = args.query {
        validate_input_length(query, "query", MAX_QUERY_LENGTH)?;
    }

    let namespace = parse_namespace(&args.namespace);
    let strategy = args.strategy.as_deref().unwrap_or("merge");
    let dry_run = args.dry_run.unwrap_or(false);

    // Fetch memories for consolidation
    let services = ServiceContainer::from_current_dir_or_user()?;
    let filter = SearchFilter::new().with_namespace(namespace);
    let recall = services.recall()?;
    let result = fetch_consolidation_candidates(&recall, &filter, args.query.as_deref(), 50)?;

    if result.memories.is_empty() {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "No memories found in namespace '{}' to consolidate.",
                    args.namespace
                ),
            }],
            is_error: false,
        });
    }

    // Build context for sampling request - use full URNs
    let memories_text: String = result
        .memories
        .iter()
        .enumerate()
        .map(|(i, hit)| {
            let domain_part = if hit.memory.domain.is_project_scoped() {
                "project".to_string()
            } else {
                hit.memory.domain.to_string()
            };
            let urn = format!(
                "subcog://{}/{}/{}",
                domain_part,
                hit.memory.namespace.as_str(),
                hit.memory.id.as_str()
            );
            format!("{}. [{}] {}", i + 1, urn, hit.memory.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let sampling_prompt = match strategy {
        "merge" => format!(
            "Analyze these {} memories from the '{}' namespace and identify groups that should be merged:\n\n{}\n\nFor each group, provide:\n1. URNs to merge\n2. Merged content\n3. Rationale",
            result.memories.len(),
            args.namespace,
            memories_text
        ),
        "summarize" => format!(
            "Create a comprehensive summary of these {} memories from the '{}' namespace:\n\n{}\n\nProvide a structured summary that captures key themes, decisions, and patterns.",
            result.memories.len(),
            args.namespace,
            memories_text
        ),
        "dedupe" => format!(
            "Identify duplicate or near-duplicate memories from these {} entries in the '{}' namespace:\n\n{}\n\nFor each duplicate set, identify which to keep and which to remove.",
            result.memories.len(),
            args.namespace,
            memories_text
        ),
        _ => format!(
            "Analyze these {} memories from the '{}' namespace:\n\n{}",
            result.memories.len(),
            args.namespace,
            memories_text
        ),
    };

    // Return sampling request
    Ok(ToolResult {
        content: vec![ToolContent::Text {
            text: if dry_run {
                format!(
                    "DRY RUN: Would consolidate {} memories using '{}' strategy.\n\nSampling prompt:\n{}",
                    result.memories.len(),
                    strategy,
                    sampling_prompt
                )
            } else {
                format!(
                    "SAMPLING_REQUEST\n\nstrategy: {}\nnamespace: {}\nmemory_count: {}\n\nprompt: {}",
                    strategy,
                    args.namespace,
                    result.memories.len(),
                    sampling_prompt
                )
            },
        }],
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

/// Executes the sync tool.
pub fn execute_sync(arguments: Value) -> Result<ToolResult> {
    let args: SyncArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    let direction = args.direction.as_deref().unwrap_or("full");

    let services = ServiceContainer::from_current_dir_or_user()?;
    let result = match direction {
        "push" => services.sync().push(),
        "fetch" => services.sync().fetch(),
        _ => services.sync().sync(),
    };

    match result {
        Ok(sync_result) => Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "Sync completed!\n\nDirection: {}\nPushed: {}\nPulled: {}\nConflicts: {}",
                    direction, sync_result.pushed, sync_result.pulled, sync_result.conflicts
                ),
            }],
            is_error: false,
        }),
        Err(e) => Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!("Sync failed: {e}"),
            }],
            is_error: true,
        }),
    }
}

/// Executes the reindex tool.
pub fn execute_reindex(arguments: Value) -> Result<ToolResult> {
    let args: ReindexArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    // Use provided repo path or current directory
    let repo_path = args.repo_path.map_or_else(
        || std::env::current_dir().unwrap_or_else(|_| ".".into()),
        std::path::PathBuf::from,
    );

    let services = ServiceContainer::for_repo(&repo_path, None)?;
    match services.reindex() {
        Ok(count) => Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "Reindex completed successfully!\n\nMemories indexed: {}\nRepository: {}",
                    count,
                    repo_path.display()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Memory, MemoryId, MemoryStatus, Namespace};
    use crate::storage::index::SqliteBackend;
    use crate::storage::traits::IndexBackend;
    use crate::services::RecallService;

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
            embedding: None,
            tags: Vec::new(),
            source: None,
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
