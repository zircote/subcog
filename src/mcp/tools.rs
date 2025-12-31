//! MCP tool implementations.
//!
//! Provides tool handlers for the Model Context Protocol.

use crate::models::{
    CaptureRequest, DetailLevel, Domain, MemoryStatus, Namespace, PromptTemplate, SearchFilter,
    SearchMode, substitute_variables,
};
use crate::services::{
    PromptFilter, PromptParser, PromptService, ServiceContainer, parse_filter_query,
};
use crate::storage::index::DomainScope;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Registry of MCP tools.
pub struct ToolRegistry {
    /// Available tools.
    tools: HashMap<String, ToolDefinition>,
}

impl ToolRegistry {
    /// Creates a new tool registry with all subcog tools.
    #[must_use]
    pub fn new() -> Self {
        let mut tools = HashMap::new();

        tools.insert("subcog_capture".to_string(), Self::capture_tool());
        tools.insert("subcog_recall".to_string(), Self::recall_tool());
        tools.insert("subcog_status".to_string(), Self::status_tool());
        tools.insert("subcog_namespaces".to_string(), Self::namespaces_tool());
        tools.insert("subcog_consolidate".to_string(), Self::consolidate_tool());
        tools.insert("subcog_enrich".to_string(), Self::enrich_tool());
        tools.insert("subcog_sync".to_string(), Self::sync_tool());
        tools.insert("subcog_reindex".to_string(), Self::reindex_tool());

        // Prompt management tools
        tools.insert("prompt_save".to_string(), Self::prompt_save_tool());
        tools.insert("prompt_list".to_string(), Self::prompt_list_tool());
        tools.insert("prompt_get".to_string(), Self::prompt_get_tool());
        tools.insert("prompt_run".to_string(), Self::prompt_run_tool());
        tools.insert("prompt_delete".to_string(), Self::prompt_delete_tool());

        Self { tools }
    }

    /// Defines the capture tool.
    fn capture_tool() -> ToolDefinition {
        ToolDefinition {
            name: "subcog_capture".to_string(),
            description: "Capture a memory (decision, learning, pattern, etc.) for future recall"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The memory content to capture"
                    },
                    "namespace": {
                        "type": "string",
                        "description": "Memory category: decisions, patterns, learnings, context, tech-debt, apis, config, security, performance, testing",
                        "enum": ["decisions", "patterns", "learnings", "context", "tech-debt", "apis", "config", "security", "performance", "testing"]
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional tags for categorization"
                    },
                    "source": {
                        "type": "string",
                        "description": "Optional source reference (file path, URL)"
                    }
                },
                "required": ["content", "namespace"]
            }),
        }
    }

    /// Defines the recall tool.
    fn recall_tool() -> ToolDefinition {
        ToolDefinition {
            name: "subcog_recall".to_string(),
            description: "Search for relevant memories using semantic and text search".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    },
                    "filter": {
                        "type": "string",
                        "description": "Filter query using GitHub-style syntax: ns:decisions tag:rust -tag:test since:7d source:src/*"
                    },
                    "namespace": {
                        "type": "string",
                        "description": "Optional: Filter by namespace (deprecated, use filter instead)",
                        "enum": ["decisions", "patterns", "learnings", "context", "tech-debt", "apis", "config", "security", "performance", "testing"]
                    },
                    "mode": {
                        "type": "string",
                        "description": "Search mode: hybrid (default), vector, text",
                        "enum": ["hybrid", "vector", "text"]
                    },
                    "detail": {
                        "type": "string",
                        "description": "Detail level: light (frontmatter only), medium (+ summary), everything (full content). Default: medium",
                        "enum": ["light", "medium", "everything"]
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results (default: 10)",
                        "minimum": 1,
                        "maximum": 50
                    }
                },
                "required": ["query"]
            }),
        }
    }

    /// Defines the status tool.
    fn status_tool() -> ToolDefinition {
        ToolDefinition {
            name: "subcog_status".to_string(),
            description: "Get memory system status and statistics".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    /// Defines the namespaces tool.
    fn namespaces_tool() -> ToolDefinition {
        ToolDefinition {
            name: "subcog_namespaces".to_string(),
            description: "List available memory namespaces and their descriptions".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    /// Defines the consolidate tool.
    fn consolidate_tool() -> ToolDefinition {
        ToolDefinition {
            name: "subcog_consolidate".to_string(),
            description: "Consolidate related memories using LLM to merge and summarize. Uses MCP sampling to request LLM completion.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "namespace": {
                        "type": "string",
                        "description": "Namespace to consolidate",
                        "enum": ["decisions", "patterns", "learnings", "context", "tech-debt", "apis", "config", "security", "performance", "testing"]
                    },
                    "query": {
                        "type": "string",
                        "description": "Optional query to filter memories for consolidation"
                    },
                    "strategy": {
                        "type": "string",
                        "description": "Consolidation strategy: merge (combine similar), summarize (create summary), dedupe (remove duplicates)",
                        "enum": ["merge", "summarize", "dedupe"],
                        "default": "merge"
                    },
                    "dry_run": {
                        "type": "boolean",
                        "description": "If true, show what would be consolidated without making changes",
                        "default": false
                    }
                },
                "required": ["namespace"]
            }),
        }
    }

    /// Defines the enrich tool.
    fn enrich_tool() -> ToolDefinition {
        ToolDefinition {
            name: "subcog_enrich".to_string(),
            description: "Enrich a memory with better structure, tags, and context using LLM. Uses MCP sampling to request LLM completion.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "memory_id": {
                        "type": "string",
                        "description": "ID of the memory to enrich"
                    },
                    "enrich_tags": {
                        "type": "boolean",
                        "description": "Generate or improve tags",
                        "default": true
                    },
                    "enrich_structure": {
                        "type": "boolean",
                        "description": "Restructure content for clarity",
                        "default": true
                    },
                    "add_context": {
                        "type": "boolean",
                        "description": "Add inferred context and rationale",
                        "default": false
                    }
                },
                "required": ["memory_id"]
            }),
        }
    }

    /// Defines the sync tool.
    fn sync_tool() -> ToolDefinition {
        ToolDefinition {
            name: "subcog_sync".to_string(),
            description: "Sync memories with git remote (push, fetch, or full sync)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "direction": {
                        "type": "string",
                        "description": "Sync direction: push (upload), fetch (download), full (both)",
                        "enum": ["push", "fetch", "full"],
                        "default": "full"
                    }
                },
                "required": []
            }),
        }
    }

    /// Defines the reindex tool.
    fn reindex_tool() -> ToolDefinition {
        ToolDefinition {
            name: "subcog_reindex".to_string(),
            description: "Rebuild the search index from git notes. Use when index is out of sync with stored memories.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repo_path": {
                        "type": "string",
                        "description": "Path to git repository (default: current directory)"
                    }
                },
                "required": []
            }),
        }
    }

    // ============================================================================
    // Prompt Management Tools
    // ============================================================================

    /// Defines the prompt.save tool.
    fn prompt_save_tool() -> ToolDefinition {
        ToolDefinition {
            name: "prompt_save".to_string(),
            description: "Save a user-defined prompt template. Provide either 'content' or 'file_path' (not both)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Unique prompt name (kebab-case, e.g., 'code-review')"
                    },
                    "content": {
                        "type": "string",
                        "description": "Prompt content with {{variable}} placeholders (required if file_path not provided)"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Path to file containing prompt (alternative to content)"
                    },
                    "description": {
                        "type": "string",
                        "description": "Human-readable description of the prompt"
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Tags for categorization and search"
                    },
                    "domain": {
                        "type": "string",
                        "description": "Storage scope: project (default), user, or org",
                        "enum": ["project", "user", "org"],
                        "default": "project"
                    },
                    "variables": {
                        "type": "array",
                        "description": "Explicit variable definitions with metadata",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string", "description": "Variable name (without braces)" },
                                "description": { "type": "string", "description": "Human-readable description" },
                                "default": { "type": "string", "description": "Default value if not provided" },
                                "required": { "type": "boolean", "default": true, "description": "Whether variable is required" }
                            },
                            "required": ["name"]
                        }
                    }
                },
                "required": ["name"],
                "additionalProperties": false
            }),
        }
    }

    /// Defines the prompt.list tool.
    fn prompt_list_tool() -> ToolDefinition {
        ToolDefinition {
            name: "prompt_list".to_string(),
            description: "List saved prompt templates with optional filtering".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "domain": {
                        "type": "string",
                        "description": "Filter by domain scope",
                        "enum": ["project", "user", "org"]
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Filter by tags (AND logic - must have all)"
                    },
                    "name_pattern": {
                        "type": "string",
                        "description": "Filter by name pattern (glob-style, e.g., 'code-*')"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results",
                        "minimum": 1,
                        "maximum": 100,
                        "default": 20
                    }
                },
                "required": []
            }),
        }
    }

    /// Defines the prompt.get tool.
    fn prompt_get_tool() -> ToolDefinition {
        ToolDefinition {
            name: "prompt_get".to_string(),
            description: "Get a prompt template by name".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Prompt name to retrieve"
                    },
                    "domain": {
                        "type": "string",
                        "description": "Domain to search (if not specified, searches Project → User → Org)",
                        "enum": ["project", "user", "org"]
                    }
                },
                "required": ["name"]
            }),
        }
    }

    /// Defines the prompt.run tool.
    fn prompt_run_tool() -> ToolDefinition {
        ToolDefinition {
            name: "prompt_run".to_string(),
            description: "Run a saved prompt, substituting variable values".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Prompt name to run"
                    },
                    "variables": {
                        "type": "object",
                        "description": "Variable values to substitute (key: value pairs)",
                        "additionalProperties": { "type": "string" }
                    },
                    "domain": {
                        "type": "string",
                        "description": "Domain to search for the prompt",
                        "enum": ["project", "user", "org"]
                    }
                },
                "required": ["name"]
            }),
        }
    }

    /// Defines the prompt.delete tool.
    fn prompt_delete_tool() -> ToolDefinition {
        ToolDefinition {
            name: "prompt_delete".to_string(),
            description: "Delete a saved prompt template".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Prompt name to delete"
                    },
                    "domain": {
                        "type": "string",
                        "description": "Domain scope to delete from (required for safety)",
                        "enum": ["project", "user", "org"]
                    }
                },
                "required": ["name", "domain"]
            }),
        }
    }

    /// Returns all tool definitions.
    #[must_use]
    pub fn list_tools(&self) -> Vec<&ToolDefinition> {
        self.tools.values().collect()
    }

    /// Gets a tool definition by name.
    #[must_use]
    pub fn get_tool(&self, name: &str) -> Option<&ToolDefinition> {
        self.tools.get(name)
    }

    /// Executes a tool with the given arguments.
    ///
    /// # Errors
    ///
    /// Returns an error if the tool execution fails.
    pub fn execute(&self, name: &str, arguments: Value) -> Result<ToolResult> {
        match name {
            "subcog_capture" => self.execute_capture(arguments),
            "subcog_recall" => self.execute_recall(arguments),
            "subcog_status" => self.execute_status(arguments),
            "subcog_namespaces" => self.execute_namespaces(arguments),
            "subcog_consolidate" => self.execute_consolidate(arguments),
            "subcog_enrich" => self.execute_enrich(arguments),
            "subcog_sync" => self.execute_sync(arguments),
            "subcog_reindex" => self.execute_reindex(arguments),
            // Prompt management tools
            "prompt_save" => self.execute_prompt_save(arguments),
            "prompt_list" => self.execute_prompt_list(arguments),
            "prompt_get" => self.execute_prompt_get(arguments),
            "prompt_run" => self.execute_prompt_run(arguments),
            "prompt_delete" => self.execute_prompt_delete(arguments),
            _ => Err(Error::InvalidInput(format!("Unknown tool: {name}"))),
        }
    }

    /// Executes the capture tool.
    fn execute_capture(&self, arguments: Value) -> Result<ToolResult> {
        let args: CaptureArgs =
            serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

        let namespace = parse_namespace(&args.namespace);

        let request = CaptureRequest {
            content: args.content,
            namespace,
            domain: Domain::default(),
            tags: args.tags.unwrap_or_default(),
            source: args.source,
            skip_security_check: false,
        };

        let services = ServiceContainer::from_current_dir()?;
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
    fn execute_recall(&self, arguments: Value) -> Result<ToolResult> {
        let args: RecallArgs =
            serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

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

        let services = ServiceContainer::from_current_dir()?;
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

            // Build rich URN: subcog://{scope}/{namespace}/{id}
            // Scope: project (default), org/{name}, or global
            let scope = hit.memory.domain.to_scope_string();
            let urn = format!(
                "subcog://{}/{}/{}",
                scope, hit.memory.namespace, hit.memory.id
            );

            output.push_str(&format!(
                "{}. {} | {:.2}{}{}\n\n",
                i + 1,
                urn,
                hit.score,
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
    fn execute_status(&self, _arguments: Value) -> Result<ToolResult> {
        // For now, return basic status
        let status = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "status": "operational",
            "backends": {
                "persistence": "git-notes",
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
    fn execute_namespaces(&self, _arguments: Value) -> Result<ToolResult> {
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
    fn execute_consolidate(&self, arguments: Value) -> Result<ToolResult> {
        let args: ConsolidateArgs =
            serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

        let namespace = parse_namespace(&args.namespace);
        let strategy = args.strategy.as_deref().unwrap_or("merge");
        let dry_run = args.dry_run.unwrap_or(false);

        // Fetch memories for consolidation
        let services = ServiceContainer::from_current_dir()?;
        let filter = SearchFilter::new().with_namespace(namespace);
        let query = args.query.as_deref().unwrap_or("*");
        let recall = services.recall()?;
        let result = recall.search(query, SearchMode::Hybrid, &filter, 50)?;

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

        // Build context for sampling request
        let memories_text: String = result
            .memories
            .iter()
            .enumerate()
            .map(|(i, hit)| format!("{}. [ID: {}] {}", i + 1, hit.memory.id, hit.memory.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        let sampling_prompt = match strategy {
            "merge" => format!(
                "Analyze these {} memories from the '{}' namespace and identify groups that should be merged:\n\n{}\n\nFor each group, provide:\n1. IDs to merge\n2. Merged content\n3. Rationale",
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
    fn execute_enrich(&self, arguments: Value) -> Result<ToolResult> {
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
    fn execute_sync(&self, arguments: Value) -> Result<ToolResult> {
        let args: SyncArgs =
            serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

        let direction = args.direction.as_deref().unwrap_or("full");

        let services = ServiceContainer::from_current_dir()?;
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
    fn execute_reindex(&self, arguments: Value) -> Result<ToolResult> {
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

    // ============================================================================
    // Prompt Tool Handlers
    // ============================================================================

    /// Executes the prompt.save tool.
    fn execute_prompt_save(&self, arguments: Value) -> Result<ToolResult> {
        let args: PromptSaveArgs =
            serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

        // Parse domain scope
        let domain = parse_domain_scope(args.domain.as_deref());

        // Build template either from content or file
        let template = if let Some(content) = args.content {
            let mut t = PromptTemplate::new(&args.name, &content);
            if let Some(desc) = args.description {
                t = t.with_description(&desc);
            }
            if let Some(tags) = args.tags {
                t = t.with_tags(tags);
            }
            if let Some(vars) = args.variables {
                use crate::models::PromptVariable;
                let variables: Vec<PromptVariable> = vars
                    .into_iter()
                    .map(|v| PromptVariable {
                        name: v.name,
                        description: v.description,
                        default: v.default,
                        required: v.required.unwrap_or(true),
                    })
                    .collect();
                t = t.with_variables(variables);
            }
            t
        } else if let Some(file_path) = args.file_path {
            PromptParser::from_file(&file_path)?
        } else {
            return Err(Error::InvalidInput(
                "Either 'content' or 'file_path' must be provided".to_string(),
            ));
        };

        // Get repo path and create service
        let services = ServiceContainer::from_current_dir()?;
        let repo_path = services
            .repo_path()
            .ok_or_else(|| Error::InvalidInput("Repository path not available".to_string()))?
            .clone();

        let mut prompt_service = PromptService::default().with_repo_path(&repo_path);
        let memory_id = prompt_service.save(&template, domain)?;

        Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "Prompt saved successfully!\n\n\
                     Name: {}\n\
                     ID: {}\n\
                     Domain: {}\n\
                     Variables: {}",
                    template.name,
                    memory_id,
                    domain_scope_to_display(domain),
                    if template.variables.is_empty() {
                        "none".to_string()
                    } else {
                        template
                            .variables
                            .iter()
                            .map(|v| v.name.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    }
                ),
            }],
            is_error: false,
        })
    }

    /// Executes the prompt.list tool.
    fn execute_prompt_list(&self, arguments: Value) -> Result<ToolResult> {
        let args: PromptListArgs =
            serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

        // Build filter
        let mut filter = PromptFilter::new();
        if let Some(domain) = args.domain {
            filter = filter.with_domain(parse_domain_scope(Some(&domain)));
        }
        if let Some(tags) = args.tags {
            filter = filter.with_tags(tags);
        }
        if let Some(pattern) = args.name_pattern {
            filter = filter.with_name_pattern(pattern);
        }
        if let Some(limit) = args.limit {
            filter = filter.with_limit(limit);
        } else {
            filter = filter.with_limit(20);
        }

        // Get prompts
        let services = ServiceContainer::from_current_dir()?;
        let repo_path = services
            .repo_path()
            .ok_or_else(|| Error::InvalidInput("Repository path not available".to_string()))?
            .clone();

        let mut prompt_service = PromptService::default().with_repo_path(&repo_path);
        let prompts = prompt_service.list(&filter)?;

        if prompts.is_empty() {
            return Ok(ToolResult {
                content: vec![ToolContent::Text {
                    text: "No prompts found matching the filter.".to_string(),
                }],
                is_error: false,
            });
        }

        let mut output = format!("Found {} prompt(s):\n\n", prompts.len());
        for (i, prompt) in prompts.iter().enumerate() {
            let tags_display = if prompt.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", prompt.tags.join(", "))
            };

            let vars_count = prompt.variables.len();
            let usage_info = if prompt.usage_count > 0 {
                format!(" (used {} times)", prompt.usage_count)
            } else {
                String::new()
            };

            output.push_str(&format!(
                "{}. **{}**{}{}\n   {}\n   Variables: {}\n\n",
                i + 1,
                prompt.name,
                tags_display,
                usage_info,
                if prompt.description.is_empty() {
                    "(no description)"
                } else {
                    &prompt.description
                },
                if vars_count == 0 {
                    "none".to_string()
                } else {
                    format!(
                        "{} ({})",
                        vars_count,
                        prompt
                            .variables
                            .iter()
                            .map(|v| v.name.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            ));
        }

        Ok(ToolResult {
            content: vec![ToolContent::Text { text: output }],
            is_error: false,
        })
    }

    /// Executes the prompt.get tool.
    fn execute_prompt_get(&self, arguments: Value) -> Result<ToolResult> {
        let args: PromptGetArgs =
            serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

        let domain = args.domain.map(|d| parse_domain_scope(Some(&d)));

        let services = ServiceContainer::from_current_dir()?;
        let repo_path = services
            .repo_path()
            .ok_or_else(|| Error::InvalidInput("Repository path not available".to_string()))?
            .clone();

        let mut prompt_service = PromptService::default().with_repo_path(&repo_path);
        let prompt = prompt_service.get(&args.name, domain)?;

        match prompt {
            Some(p) => {
                let vars_info: Vec<String> = p.variables.iter().map(format_variable_info).collect();

                Ok(ToolResult {
                    content: vec![ToolContent::Text {
                        text: format!(
                            "**{}**\n\n\
                             {}\n\n\
                             **Variables:**\n{}\n\n\
                             **Content:**\n```\n{}\n```\n\n\
                             Tags: {}\n\
                             Usage count: {}",
                            p.name,
                            if p.description.is_empty() {
                                "(no description)".to_string()
                            } else {
                                p.description.clone()
                            },
                            if vars_info.is_empty() {
                                "none".to_string()
                            } else {
                                vars_info.join("\n")
                            },
                            p.content,
                            if p.tags.is_empty() {
                                "none".to_string()
                            } else {
                                p.tags.join(", ")
                            },
                            p.usage_count
                        ),
                    }],
                    is_error: false,
                })
            },
            None => Ok(ToolResult {
                content: vec![ToolContent::Text {
                    text: format!("Prompt '{}' not found.", args.name),
                }],
                is_error: true,
            }),
        }
    }

    /// Executes the prompt.run tool.
    fn execute_prompt_run(&self, arguments: Value) -> Result<ToolResult> {
        let args: PromptRunArgs =
            serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

        let domain = args.domain.map(|d| parse_domain_scope(Some(&d)));

        let services = ServiceContainer::from_current_dir()?;
        let repo_path = services
            .repo_path()
            .ok_or_else(|| Error::InvalidInput("Repository path not available".to_string()))?
            .clone();

        let mut prompt_service = PromptService::default().with_repo_path(&repo_path);
        let prompt = prompt_service.get(&args.name, domain)?;

        match prompt {
            Some(p) => {
                // Convert variables to HashMap
                let values: HashMap<String, String> = args.variables.unwrap_or_default();

                // Check for missing required variables
                let missing: Vec<&str> = find_missing_required_variables(&p.variables, &values);

                if !missing.is_empty() {
                    return Ok(ToolResult {
                        content: vec![ToolContent::Text {
                            text: format!(
                                "Missing required variables: {}\n\n\
                                 Use the 'variables' parameter to provide values:\n\
                                 ```json\n{{\n  \"variables\": {{\n{}\n  }}\n}}\n```",
                                missing.join(", "),
                                missing
                                    .iter()
                                    .map(|n| format!("    \"{n}\": \"<value>\""))
                                    .collect::<Vec<_>>()
                                    .join(",\n")
                            ),
                        }],
                        is_error: true,
                    });
                }

                // Substitute variables
                let result = substitute_variables(&p.content, &values, &p.variables)?;

                // Increment usage count (best effort)
                if let Some(scope) = domain {
                    let _ = prompt_service.increment_usage(&args.name, scope);
                }

                Ok(ToolResult {
                    content: vec![ToolContent::Text {
                        text: format!(
                            "**Prompt: {}**\n\n{}\n\n---\n_Variables substituted: {}_",
                            p.name,
                            result,
                            if values.is_empty() {
                                "none (defaults used)".to_string()
                            } else {
                                values.keys().cloned().collect::<Vec<_>>().join(", ")
                            }
                        ),
                    }],
                    is_error: false,
                })
            },
            None => Ok(ToolResult {
                content: vec![ToolContent::Text {
                    text: format!("Prompt '{}' not found.", args.name),
                }],
                is_error: true,
            }),
        }
    }

    /// Executes the prompt.delete tool.
    fn execute_prompt_delete(&self, arguments: Value) -> Result<ToolResult> {
        let args: PromptDeleteArgs =
            serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

        let domain = parse_domain_scope(Some(&args.domain));

        let services = ServiceContainer::from_current_dir()?;
        let repo_path = services
            .repo_path()
            .ok_or_else(|| Error::InvalidInput("Repository path not available".to_string()))?
            .clone();

        let mut prompt_service = PromptService::default().with_repo_path(&repo_path);
        let deleted = prompt_service.delete(&args.name, domain)?;

        if deleted {
            Ok(ToolResult {
                content: vec![ToolContent::Text {
                    text: format!(
                        "Prompt '{}' deleted from {} scope.",
                        args.name,
                        domain_scope_to_display(domain)
                    ),
                }],
                is_error: false,
            })
        } else {
            Ok(ToolResult {
                content: vec![ToolContent::Text {
                    text: format!(
                        "Prompt '{}' not found in {} scope.",
                        args.name,
                        domain_scope_to_display(domain)
                    ),
                }],
                is_error: true,
            })
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Definition of an MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name.
    pub name: String,
    /// Tool description.
    pub description: String,
    /// JSON Schema for input validation.
    pub input_schema: Value,
}

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Content returned by the tool.
    pub content: Vec<ToolContent>,
    /// Whether the result represents an error.
    #[serde(default)]
    pub is_error: bool,
}

/// Content types that can be returned by tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolContent {
    /// Text content.
    Text {
        /// The text content.
        text: String,
    },
    /// Image content (base64 encoded).
    Image {
        /// Base64-encoded image data.
        data: String,
        /// MIME type of the image.
        mime_type: String,
    },
}

/// Arguments for the capture tool.
#[derive(Debug, Deserialize)]
struct CaptureArgs {
    content: String,
    namespace: String,
    tags: Option<Vec<String>>,
    source: Option<String>,
}

/// Arguments for the recall tool.
#[derive(Debug, Deserialize)]
struct RecallArgs {
    query: String,
    filter: Option<String>,
    namespace: Option<String>,
    mode: Option<String>,
    detail: Option<String>,
    limit: Option<usize>,
}

/// Arguments for the consolidate tool.
#[derive(Debug, Deserialize)]
struct ConsolidateArgs {
    namespace: String,
    query: Option<String>,
    strategy: Option<String>,
    dry_run: Option<bool>,
}

/// Arguments for the enrich tool.
#[derive(Debug, Deserialize)]
struct EnrichArgs {
    memory_id: String,
    enrich_tags: Option<bool>,
    enrich_structure: Option<bool>,
    add_context: Option<bool>,
}

/// Arguments for the sync tool.
#[derive(Debug, Deserialize)]
struct SyncArgs {
    direction: Option<String>,
}

/// Arguments for the reindex tool.
#[derive(Debug, Deserialize)]
struct ReindexArgs {
    repo_path: Option<String>,
}

// ============================================================================
// Prompt Tool Arguments
// ============================================================================

/// Arguments for the prompt.save tool.
#[derive(Debug, Deserialize)]
struct PromptSaveArgs {
    name: String,
    content: Option<String>,
    file_path: Option<String>,
    description: Option<String>,
    tags: Option<Vec<String>>,
    domain: Option<String>,
    variables: Option<Vec<PromptVariableArg>>,
}

/// Variable definition argument for prompt.save.
#[derive(Debug, Deserialize)]
struct PromptVariableArg {
    name: String,
    description: Option<String>,
    default: Option<String>,
    required: Option<bool>,
}

/// Arguments for the prompt.list tool.
#[derive(Debug, Deserialize)]
struct PromptListArgs {
    domain: Option<String>,
    tags: Option<Vec<String>>,
    name_pattern: Option<String>,
    limit: Option<usize>,
}

/// Arguments for the prompt.get tool.
#[derive(Debug, Deserialize)]
struct PromptGetArgs {
    name: String,
    domain: Option<String>,
}

/// Arguments for the prompt.run tool.
#[derive(Debug, Deserialize)]
struct PromptRunArgs {
    name: String,
    variables: Option<HashMap<String, String>>,
    domain: Option<String>,
}

/// Arguments for the prompt.delete tool.
#[derive(Debug, Deserialize)]
struct PromptDeleteArgs {
    name: String,
    domain: String,
}

/// Parses a namespace string to Namespace enum.
fn parse_namespace(s: &str) -> Namespace {
    match s.to_lowercase().as_str() {
        "decisions" => Namespace::Decisions,
        "patterns" => Namespace::Patterns,
        "learnings" => Namespace::Learnings,
        "context" => Namespace::Context,
        "tech-debt" | "techdebt" => Namespace::TechDebt,
        "apis" => Namespace::Apis,
        "config" => Namespace::Config,
        "security" => Namespace::Security,
        "performance" => Namespace::Performance,
        "testing" => Namespace::Testing,
        _ => Namespace::Decisions,
    }
}

/// Parses a search mode string to `SearchMode` enum.
fn parse_search_mode(s: &str) -> SearchMode {
    match s.to_lowercase().as_str() {
        "vector" => SearchMode::Vector,
        "text" => SearchMode::Text,
        _ => SearchMode::Hybrid,
    }
}

/// Parses a domain scope string to `DomainScope` enum.
fn parse_domain_scope(s: Option<&str>) -> DomainScope {
    match s.map(str::to_lowercase).as_deref() {
        Some("user") => DomainScope::User,
        Some("org") => DomainScope::Org,
        _ => DomainScope::Project,
    }
}

/// Converts a `DomainScope` to a display string.
const fn domain_scope_to_display(scope: DomainScope) -> &'static str {
    match scope {
        DomainScope::Project => "project",
        DomainScope::User => "user",
        DomainScope::Org => "org",
    }
}

/// Formats a `PromptVariable` for display.
fn format_variable_info(v: &crate::models::PromptVariable) -> String {
    let mut info = format!("- **{{{{{}}}}}**", v.name);
    if let Some(ref desc) = v.description {
        info.push_str(&format!(": {desc}"));
    }
    if let Some(ref default) = v.default {
        info.push_str(&format!(" (default: `{default}`)"));
    }
    if !v.required {
        info.push_str(" [optional]");
    }
    info
}

/// Finds missing required variables.
fn find_missing_required_variables<'a>(
    variables: &'a [crate::models::PromptVariable],
    values: &HashMap<String, String>,
) -> Vec<&'a str> {
    variables
        .iter()
        .filter(|v| v.required && v.default.is_none() && !values.contains_key(&v.name))
        .map(|v| v.name.as_str())
        .collect()
}

/// Truncates a string to a maximum length.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Formats content based on detail level.
fn format_content_for_detail(content: &str, detail: DetailLevel) -> String {
    if content.is_empty() {
        return String::new();
    }
    match detail {
        DetailLevel::Light => String::new(),
        DetailLevel::Medium => format!("\n   {}", truncate(content, 200)),
        DetailLevel::Everything => format!("\n   {content}"),
    }
}

/// Builds a human-readable description of the active filters.
fn build_filter_description(filter: &SearchFilter) -> String {
    let mut parts = Vec::new();

    if !filter.namespaces.is_empty() {
        let ns_list: Vec<_> = filter.namespaces.iter().map(Namespace::as_str).collect();
        parts.push(format!("ns:{}", ns_list.join(",")));
    }

    if !filter.tags.is_empty() {
        for tag in &filter.tags {
            parts.push(format!("tag:{tag}"));
        }
    }

    if !filter.tags_any.is_empty() {
        parts.push(format!("tag:{}", filter.tags_any.join(",")));
    }

    if !filter.excluded_tags.is_empty() {
        for tag in &filter.excluded_tags {
            parts.push(format!("-tag:{tag}"));
        }
    }

    if let Some(ref pattern) = filter.source_pattern {
        parts.push(format!("source:{pattern}"));
    }

    if !filter.statuses.is_empty() {
        let status_list: Vec<_> = filter.statuses.iter().map(MemoryStatus::as_str).collect();
        parts.push(format!("status:{}", status_list.join(",")));
    }

    if filter.created_after.is_some() {
        parts.push("since:active".to_string());
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!(", filter: {}", parts.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_registry_creation() {
        let registry = ToolRegistry::new();
        let tools = registry.list_tools();

        assert!(!tools.is_empty());
        assert!(registry.get_tool("subcog_capture").is_some());
        assert!(registry.get_tool("subcog_recall").is_some());
        assert!(registry.get_tool("subcog_status").is_some());
        assert!(registry.get_tool("subcog_namespaces").is_some());
    }

    #[test]
    fn test_tool_definitions() {
        let registry = ToolRegistry::new();

        let capture = registry.get_tool("subcog_capture").unwrap();
        assert!(capture.description.contains("memory"));
        assert!(
            capture.input_schema["required"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("content"))
        );
    }

    #[test]
    fn test_execute_namespaces() {
        let registry = ToolRegistry::new();
        let result = registry
            .execute("subcog_namespaces", serde_json::json!({}))
            .unwrap();

        assert!(!result.is_error);
        assert!(!result.content.is_empty());

        if let ToolContent::Text { text } = &result.content[0] {
            assert!(text.contains("decisions"));
            assert!(text.contains("patterns"));
        }
    }

    #[test]
    fn test_execute_status() {
        let registry = ToolRegistry::new();
        let result = registry
            .execute("subcog_status", serde_json::json!({}))
            .unwrap();

        assert!(!result.is_error);
        if let ToolContent::Text { text } = &result.content[0] {
            assert!(text.contains("version"));
        }
    }

    #[test]
    fn test_execute_unknown_tool() {
        let registry = ToolRegistry::new();
        let result = registry.execute("unknown_tool", serde_json::json!({}));

        assert!(result.is_err());
    }

    #[test]
    fn test_parse_namespace() {
        assert_eq!(parse_namespace("decisions"), Namespace::Decisions);
        assert_eq!(parse_namespace("PATTERNS"), Namespace::Patterns);
        assert_eq!(parse_namespace("tech-debt"), Namespace::TechDebt);
    }

    #[test]
    fn test_parse_search_mode() {
        assert_eq!(parse_search_mode("vector"), SearchMode::Vector);
        assert_eq!(parse_search_mode("TEXT"), SearchMode::Text);
        assert_eq!(parse_search_mode("hybrid"), SearchMode::Hybrid);
        assert_eq!(parse_search_mode("unknown"), SearchMode::Hybrid);
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("this is a long string", 10), "this is...");
    }

    // ============================================================================
    // Prompt Tool Tests
    // ============================================================================

    #[test]
    fn test_prompt_tools_registered() {
        let registry = ToolRegistry::new();

        assert!(registry.get_tool("prompt_save").is_some());
        assert!(registry.get_tool("prompt_list").is_some());
        assert!(registry.get_tool("prompt_get").is_some());
        assert!(registry.get_tool("prompt_run").is_some());
        assert!(registry.get_tool("prompt_delete").is_some());
    }

    #[test]
    fn test_prompt_save_tool_schema() {
        let registry = ToolRegistry::new();
        let tool = registry.get_tool("prompt_save").unwrap();

        assert!(tool.description.contains("Save"));
        assert!(tool.input_schema["properties"]["name"].is_object());
        assert!(tool.input_schema["properties"]["content"].is_object());
        assert!(tool.input_schema["properties"]["file_path"].is_object());
        assert!(tool.input_schema["properties"]["domain"].is_object());
        assert!(tool.input_schema["properties"]["variables"].is_object());
    }

    #[test]
    fn test_prompt_list_tool_schema() {
        let registry = ToolRegistry::new();
        let tool = registry.get_tool("prompt_list").unwrap();

        assert!(tool.description.contains("List"));
        assert!(tool.input_schema["properties"]["domain"].is_object());
        assert!(tool.input_schema["properties"]["tags"].is_object());
        assert!(tool.input_schema["properties"]["name_pattern"].is_object());
    }

    #[test]
    fn test_prompt_get_tool_schema() {
        let registry = ToolRegistry::new();
        let tool = registry.get_tool("prompt_get").unwrap();

        assert!(tool.description.contains("Get"));
        let required = tool.input_schema["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::json!("name")));
    }

    #[test]
    fn test_prompt_run_tool_schema() {
        let registry = ToolRegistry::new();
        let tool = registry.get_tool("prompt_run").unwrap();

        assert!(tool.description.contains("Run"));
        assert!(tool.input_schema["properties"]["variables"].is_object());
    }

    #[test]
    fn test_prompt_delete_tool_schema() {
        let registry = ToolRegistry::new();
        let tool = registry.get_tool("prompt_delete").unwrap();

        assert!(tool.description.contains("Delete"));
        let required = tool.input_schema["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::json!("name")));
        assert!(required.contains(&serde_json::json!("domain")));
    }

    #[test]
    fn test_parse_domain_scope() {
        assert_eq!(parse_domain_scope(Some("project")), DomainScope::Project);
        assert_eq!(parse_domain_scope(Some("PROJECT")), DomainScope::Project);
        assert_eq!(parse_domain_scope(Some("user")), DomainScope::User);
        assert_eq!(parse_domain_scope(Some("org")), DomainScope::Org);
        assert_eq!(parse_domain_scope(None), DomainScope::Project);
        assert_eq!(parse_domain_scope(Some("unknown")), DomainScope::Project);
    }

    #[test]
    fn test_domain_scope_to_display() {
        assert_eq!(domain_scope_to_display(DomainScope::Project), "project");
        assert_eq!(domain_scope_to_display(DomainScope::User), "user");
        assert_eq!(domain_scope_to_display(DomainScope::Org), "org");
    }

    #[test]
    fn test_format_variable_info() {
        use crate::models::PromptVariable;

        // Required variable with description and default
        let var = PromptVariable {
            name: "name".to_string(),
            description: Some("User name".to_string()),
            default: Some("World".to_string()),
            required: true,
        };
        let info = format_variable_info(&var);
        assert!(info.contains("**{{name}}**"));
        assert!(info.contains("User name"));
        assert!(info.contains("World"));
        assert!(!info.contains("[optional]"));

        // Optional variable
        let var = PromptVariable {
            name: "extra".to_string(),
            description: None,
            default: None,
            required: false,
        };
        let info = format_variable_info(&var);
        assert!(info.contains("[optional]"));
    }

    #[test]
    fn test_find_missing_required_variables() {
        use crate::models::PromptVariable;

        let variables = vec![
            PromptVariable {
                name: "required_var".to_string(),
                description: None,
                default: None,
                required: true,
            },
            PromptVariable {
                name: "optional_var".to_string(),
                description: None,
                default: None,
                required: false,
            },
            PromptVariable {
                name: "with_default".to_string(),
                description: None,
                default: Some("default_value".to_string()),
                required: true,
            },
        ];

        // No values provided - only required_var should be missing
        let values = HashMap::new();
        let missing = find_missing_required_variables(&variables, &values);
        assert_eq!(missing, vec!["required_var"]);

        // With required_var provided - nothing missing
        let mut values = HashMap::new();
        values.insert("required_var".to_string(), "value".to_string());
        let missing = find_missing_required_variables(&variables, &values);
        assert!(missing.is_empty());
    }
}
