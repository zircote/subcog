//! MCP tool implementations.
//!
//! Provides tool handlers for the Model Context Protocol.

use crate::models::{CaptureRequest, Domain, Namespace, SearchFilter, SearchMode};
use crate::services::ServiceContainer;
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
                    "namespace": {
                        "type": "string",
                        "description": "Optional: Filter by namespace",
                        "enum": ["decisions", "patterns", "learnings", "context", "tech-debt", "apis", "config", "security", "performance", "testing"]
                    },
                    "mode": {
                        "type": "string",
                        "description": "Search mode: hybrid (default), vector, text",
                        "enum": ["hybrid", "vector", "text"]
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

        let services = ServiceContainer::get()?;
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

        let mut filter = SearchFilter::new();
        if let Some(ns) = &args.namespace {
            filter = filter.with_namespace(parse_namespace(ns));
        }

        let limit = args.limit.unwrap_or(10).min(50);

        let services = ServiceContainer::get()?;
        let result = services
            .recall()
            .search(&args.query, mode, &filter, limit)?;

        let mut output = format!(
            "Found {} memories (searched in {}ms using {} mode)\n\n",
            result.total_count, result.execution_time_ms, result.mode
        );

        for (i, hit) in result.memories.iter().enumerate() {
            output.push_str(&format!(
                "{}. [{}] {}\n   Score: {:.2}\n   ID: {}\n\n",
                i + 1,
                hit.memory.namespace,
                truncate(&hit.memory.content, 100),
                hit.score,
                hit.memory.id
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
        let services = ServiceContainer::get()?;
        let filter = SearchFilter::new().with_namespace(namespace);
        let query = args.query.as_deref().unwrap_or("*");
        let result = services
            .recall()
            .search(query, SearchMode::Hybrid, &filter, 50)?;

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

        let services = ServiceContainer::get()?;
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
    namespace: Option<String>,
    mode: Option<String>,
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

/// Truncates a string to a maximum length.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
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
}
