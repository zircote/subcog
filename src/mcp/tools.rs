//! MCP tool implementations.
//!
//! Provides tool handlers for the Model Context Protocol.

use crate::models::{CaptureRequest, Domain, Namespace, SearchFilter, SearchMode};
use crate::services::{CaptureService, RecallService};
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

        // Capture tool
        tools.insert(
            "subcog_capture".to_string(),
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
            },
        );

        // Recall tool
        tools.insert(
            "subcog_recall".to_string(),
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
            },
        );

        // Status tool
        tools.insert(
            "subcog_status".to_string(),
            ToolDefinition {
                name: "subcog_status".to_string(),
                description: "Get memory system status and statistics".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        );

        // Namespaces tool
        tools.insert(
            "subcog_namespaces".to_string(),
            ToolDefinition {
                name: "subcog_namespaces".to_string(),
                description: "List available memory namespaces and their descriptions".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        );

        Self { tools }
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

        let service = CaptureService::default();
        let result = service.capture(request)?;

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
            .map(parse_search_mode)
            .unwrap_or(SearchMode::Hybrid);

        let mut filter = SearchFilter::new();
        if let Some(ns) = &args.namespace {
            filter = filter.with_namespace(parse_namespace(ns));
        }

        let limit = args.limit.unwrap_or(10).min(50);

        let service = RecallService::default();
        let result = service.search(&args.query, mode, &filter, limit)?;

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

/// Parses a search mode string to SearchMode enum.
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
        assert!(capture.input_schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("content")));
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
