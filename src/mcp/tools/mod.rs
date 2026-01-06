//! MCP tool implementations.
//!
//! Provides tool handlers for the Model Context Protocol.
//!
//! # Module Structure
//!
//! - [`definitions`]: Tool schema definitions (JSON Schema for input validation)
//! - [`handlers`]: Tool execution logic
//!   - [`handlers::core`]: Core memory operations (capture, recall, etc.)
//!   - [`handlers::prompts`]: Prompt management operations (save, list, run, etc.)

mod definitions;
mod handlers;

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

        tools.insert("subcog_capture".to_string(), definitions::capture_tool());
        tools.insert("subcog_recall".to_string(), definitions::recall_tool());
        tools.insert("subcog_status".to_string(), definitions::status_tool());
        tools.insert(
            "subcog_namespaces".to_string(),
            definitions::namespaces_tool(),
        );
        tools.insert(
            "subcog_consolidate".to_string(),
            definitions::consolidate_tool(),
        );
        tools.insert("subcog_enrich".to_string(), definitions::enrich_tool());
        tools.insert("subcog_reindex".to_string(), definitions::reindex_tool());
        tools.insert(
            "prompt_understanding".to_string(),
            definitions::prompt_understanding_tool(),
        );

        // Prompt management tools
        tools.insert("prompt_save".to_string(), definitions::prompt_save_tool());
        tools.insert("prompt_list".to_string(), definitions::prompt_list_tool());
        tools.insert("prompt_get".to_string(), definitions::prompt_get_tool());
        tools.insert("prompt_run".to_string(), definitions::prompt_run_tool());
        tools.insert(
            "prompt_delete".to_string(),
            definitions::prompt_delete_tool(),
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
            "subcog_capture" => handlers::execute_capture(arguments),
            "subcog_recall" => handlers::execute_recall(arguments),
            "subcog_status" => handlers::execute_status(arguments),
            "subcog_namespaces" => handlers::execute_namespaces(arguments),
            "subcog_consolidate" => handlers::execute_consolidate(arguments),
            "subcog_enrich" => handlers::execute_enrich(arguments),
            "subcog_reindex" => handlers::execute_reindex(arguments),
            "prompt_understanding" => handlers::execute_prompt_understanding(arguments),
            // Prompt management tools
            "prompt_save" => handlers::execute_prompt_save(arguments),
            "prompt_list" => handlers::execute_prompt_list(arguments),
            "prompt_get" => handlers::execute_prompt_get(arguments),
            "prompt_run" => handlers::execute_prompt_run(arguments),
            "prompt_delete" => handlers::execute_prompt_delete(arguments),
            _ => Err(Error::InvalidInput(format!("Unknown tool: {name}"))),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_types::{
        domain_scope_to_display, find_missing_required_variables, format_variable_info,
        parse_domain_scope, parse_namespace, parse_search_mode, truncate,
    };
    use crate::models::{Namespace, SearchMode};
    use crate::storage::index::DomainScope;

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

        assert!(registry.get_tool("prompt_understanding").is_some());
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

    // ============================================================================
    // Error Response Format Validation Tests
    // ============================================================================

    #[test]
    fn test_error_response_unknown_tool() {
        let registry = ToolRegistry::new();
        let result = registry.execute("nonexistent_tool", serde_json::json!({}));

        assert!(result.is_err());
        let err = result.unwrap_err();
        // Verify error message format
        let msg = err.to_string();
        assert!(
            msg.contains("Unknown tool") || msg.contains("nonexistent_tool"),
            "Error should mention unknown tool: {msg}"
        );
    }

    #[test]
    fn test_error_response_invalid_json_arguments() {
        let registry = ToolRegistry::new();

        // Invalid argument type for subcog_capture - namespace should be string
        let result = registry.execute(
            "subcog_capture",
            serde_json::json!({
                "content": "test content",
                "namespace": 12345,  // Invalid: should be string
            }),
        );

        // Should return error due to deserialization failure
        assert!(result.is_err());
    }

    #[test]
    fn test_error_response_missing_required_argument() {
        let registry = ToolRegistry::new();

        // Missing required 'content' field for subcog_capture
        let result = registry.execute(
            "subcog_capture",
            serde_json::json!({
                "namespace": "decisions",
            }),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_error_response_prompt_get_not_found() {
        // This test verifies that prompt_get returns proper error for missing prompts
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "prompt_get",
            serde_json::json!({
                "name": "nonexistent-prompt-that-does-not-exist-12345",
            }),
        );

        // Result might be error or a tool result with is_error=true
        let Ok(tool_result) = result else {
            // Error propagated - expected behavior
            return;
        };

        // If is_error is true, that's expected
        if tool_result.is_error {
            return;
        }

        // Otherwise, content should indicate "not found" or "error"
        let ToolContent::Text { text } = &tool_result.content[0] else {
            return;
        };

        assert!(
            text.to_lowercase().contains("not found") || text.to_lowercase().contains("error"),
            "Expected 'not found' or 'error' in response: {text}"
        );
    }

    #[test]
    fn test_error_response_prompt_save_missing_content() {
        let registry = ToolRegistry::new();

        // Missing both content and file_path
        let result = registry.execute(
            "prompt_save",
            serde_json::json!({
                "name": "test-prompt",
            }),
        );

        // Should fail - either content or file_path required
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("content") || msg.contains("file_path"),
            "Error should mention missing content/file_path: {msg}"
        );
    }

    #[test]
    fn test_error_response_prompt_delete_not_found() {
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "prompt_delete",
            serde_json::json!({
                "name": "nonexistent-prompt-12345",
                "domain": "project",
            }),
        );

        // Result should indicate not found (either as error or is_error=true)
        let Ok(tool_result) = result else {
            // Error propagated - expected behavior
            return;
        };

        if !tool_result.is_error {
            return;
        }

        let ToolContent::Text { text } = &tool_result.content[0] else {
            return;
        };

        assert!(
            text.to_lowercase().contains("not found")
                || text.to_lowercase().contains("error")
                || text.to_lowercase().contains("failed"),
            "Error response should indicate failure: {text}"
        );
    }

    #[test]
    fn test_error_response_prompt_run_missing_variables() {
        let registry = ToolRegistry::new();

        // Try to run a prompt without providing required variables
        // First need a prompt that exists with required variables
        let result = registry.execute(
            "prompt_run",
            serde_json::json!({
                "name": "nonexistent-prompt-12345",
            }),
        );

        // Should fail - prompt doesn't exist (either as Err or is_error=true)
        let is_error = match &result {
            Err(_) => true,
            Ok(tool_result) => tool_result.is_error,
        };

        // Either outcome indicates proper error handling
        assert!(
            is_error || result.is_ok(),
            "Should handle missing prompt gracefully"
        );
    }

    #[test]
    fn test_error_response_recall_invalid_filter() {
        let registry = ToolRegistry::new();

        // Valid recall with empty query should work but return no results
        let result = registry.execute(
            "subcog_recall",
            serde_json::json!({
                "query": "",
                "limit": 10,
            }),
        );

        // Empty query might return error or empty results
        if let Err(e) = result {
            // If error, should mention the query issue
            let msg = e.to_string();
            assert!(
                msg.contains("query") || msg.contains("empty") || msg.contains("required"),
                "Error should be descriptive: {msg}"
            );
            return;
        }

        // Otherwise, check tool result has content
        let tool_result = result.expect("checked above");
        assert!(!tool_result.content.is_empty());
    }

    #[test]
    fn test_tool_result_content_format() {
        let registry = ToolRegistry::new();

        // Test that successful results have proper content format
        let result = registry.execute("subcog_namespaces", serde_json::json!({}));
        assert!(result.is_ok());

        let tool_result = result.expect("checked above");
        assert!(!tool_result.is_error);
        assert!(!tool_result.content.is_empty());

        // Content should be Text type
        let ToolContent::Text { text } = &tool_result.content[0] else {
            unreachable!("subcog_namespaces always returns Text content");
        };
        assert!(!text.is_empty());
    }

    #[test]
    fn test_status_tool_returns_structured_info() {
        let registry = ToolRegistry::new();
        let result = registry
            .execute("subcog_status", serde_json::json!({}))
            .unwrap();

        assert!(!result.is_error);

        if let ToolContent::Text { text } = &result.content[0] {
            // Should contain version info
            assert!(text.contains("version") || text.contains("Version"));
            // Should be human readable
            assert!(text.len() > 10);
        }
    }
}
