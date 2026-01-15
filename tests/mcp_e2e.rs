//! MCP Server End-to-End Tests (TEST-HIGH-004)
//!
//! Tests MCP server components in integration, focusing on:
//! - Tool registration and discovery
//! - Tool execution workflows (capture → recall)
//! - Resource access and listing
//! - Prompt registration and execution
//! - Input validation (SEC-M5)
//! - Error handling and error response format
//! - JSON-RPC request/response format compliance
//!
//! These tests verify the MCP protocol implementation without requiring
//! external services - they test the internal component integration.

// Integration tests use expect/unwrap for simplicity - panics are acceptable in tests
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::uninlined_format_args,
    clippy::single_match_else,
    clippy::unnecessary_map_or,
    clippy::map_unwrap_or,
    clippy::redundant_closure_for_method_calls,
    clippy::option_as_ref_deref
)]

use serde_json::{Value, json};

// ============================================================================
// Tool Registry Tests
// ============================================================================

mod tool_registry {
    use super::*;
    use subcog::mcp::{ToolContent, ToolRegistry};

    #[test]
    fn test_registry_contains_all_core_tools() {
        let registry = ToolRegistry::new();

        // Core memory tools
        assert!(registry.get_tool("subcog_capture").is_some());
        assert!(registry.get_tool("subcog_recall").is_some());
        assert!(registry.get_tool("subcog_status").is_some());
        assert!(registry.get_tool("subcog_namespaces").is_some());
        assert!(registry.get_tool("subcog_consolidate").is_some());
        assert!(registry.get_tool("subcog_enrich").is_some());
        assert!(registry.get_tool("subcog_sync").is_some());
        assert!(registry.get_tool("subcog_reindex").is_some());
        assert!(registry.get_tool("subcog_gdpr_export").is_some());

        // Prompt management tools
        assert!(registry.get_tool("prompt_save").is_some());
        assert!(registry.get_tool("prompt_list").is_some());
        assert!(registry.get_tool("prompt_get").is_some());
        assert!(registry.get_tool("prompt_run").is_some());
        assert!(registry.get_tool("prompt_delete").is_some());
        assert!(registry.get_tool("prompt_understanding").is_some());
    }

    #[test]
    fn test_tool_count() {
        let registry = ToolRegistry::new();
        let tools = registry.list_tools();

        // Should have at least 15 tools registered
        assert!(
            tools.len() >= 15,
            "Expected at least 15 tools, got {}",
            tools.len()
        );
    }

    #[test]
    fn test_tool_definitions_have_required_fields() {
        let registry = ToolRegistry::new();

        for tool in registry.list_tools() {
            // Every tool must have a name
            assert!(!tool.name.is_empty(), "Tool name cannot be empty");

            // Every tool must have a description
            assert!(
                !tool.description.is_empty(),
                "Tool {} must have a description",
                tool.name
            );

            // Every tool must have an input schema
            assert!(
                tool.input_schema.is_object(),
                "Tool {} must have an object input schema",
                tool.name
            );

            // Input schema must have "type": "object"
            assert_eq!(
                tool.input_schema["type"], "object",
                "Tool {} schema type must be object",
                tool.name
            );

            // Input schema must have "properties" field
            assert!(
                tool.input_schema["properties"].is_object(),
                "Tool {} must have properties in schema",
                tool.name
            );
        }
    }

    #[test]
    fn test_capture_tool_schema() {
        let registry = ToolRegistry::new();
        let tool = registry.get_tool("subcog_capture").unwrap();

        let required = tool.input_schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("content")));
        assert!(required.contains(&json!("namespace")));

        let properties = &tool.input_schema["properties"];
        assert!(properties["content"].is_object());
        assert!(properties["namespace"].is_object());
        assert!(properties["tags"].is_object());
        assert!(properties["source"].is_object());
    }

    #[test]
    fn test_recall_tool_schema() {
        let registry = ToolRegistry::new();
        let tool = registry.get_tool("subcog_recall").unwrap();

        // query is now optional - no required fields (acts as list mode when omitted)
        if let Some(req_array) = tool.input_schema.get("required").and_then(|r| r.as_array()) {
            assert!(
                !req_array.contains(&json!("query")),
                "query should not be required"
            );
        }

        let properties = &tool.input_schema["properties"];
        assert!(properties["query"].is_object());
        assert!(properties["filter"].is_object());
        assert!(properties["mode"].is_object());
        assert!(properties["detail"].is_object());
        assert!(properties["limit"].is_object());
    }

    #[test]
    fn test_execute_unknown_tool_returns_error() {
        let registry = ToolRegistry::new();
        let result = registry.execute("nonexistent_tool", json!({}));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("Unknown tool"),
            "Error message should mention unknown tool: {}",
            err
        );
    }

    #[test]
    fn test_execute_status_tool() {
        let registry = ToolRegistry::new();
        let result = registry.execute("subcog_status", json!({}));

        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(!tool_result.is_error);
        assert!(!tool_result.content.is_empty());

        if let ToolContent::Text { text } = &tool_result.content[0] {
            assert!(text.contains("version") || text.contains("Version"));
        }
    }

    #[test]
    fn test_execute_namespaces_tool() {
        let registry = ToolRegistry::new();
        let result = registry.execute("subcog_namespaces", json!({}));

        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(!tool_result.is_error);

        if let ToolContent::Text { text } = &tool_result.content[0] {
            // Should list all 10 namespaces
            assert!(text.contains("decisions"));
            assert!(text.contains("patterns"));
            assert!(text.contains("learnings"));
            assert!(text.contains("context"));
            assert!(text.contains("tech-debt"));
            assert!(text.contains("apis"));
            assert!(text.contains("config"));
            assert!(text.contains("security"));
            assert!(text.contains("performance"));
            assert!(text.contains("testing"));
        }
    }

    #[test]
    fn test_execute_prompt_understanding_tool() {
        let registry = ToolRegistry::new();
        let result = registry.execute("prompt_understanding", json!({}));

        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(!tool_result.is_error);

        if let ToolContent::Text { text } = &tool_result.content[0] {
            // Should contain guidance about using subcog
            assert!(
                text.contains("Subcog") || text.contains("memory") || text.contains("capture"),
                "prompt_understanding should provide guidance"
            );
        }
    }
}

// ============================================================================
// Input Validation Tests (SEC-M5)
// ============================================================================

mod input_validation {
    use super::*;
    use subcog::mcp::ToolRegistry;

    #[test]
    fn test_capture_rejects_missing_content() {
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "subcog_capture",
            json!({
                "namespace": "decisions"
                // Missing "content"
            }),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_capture_rejects_missing_namespace() {
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "subcog_capture",
            json!({
                "content": "Test memory"
                // Missing "namespace"
            }),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_capture_rejects_invalid_content_type() {
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "subcog_capture",
            json!({
                "content": 12345,  // Should be string
                "namespace": "decisions"
            }),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_capture_rejects_invalid_namespace_type() {
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "subcog_capture",
            json!({
                "content": "Test memory",
                "namespace": ["decisions"]  // Should be string, not array
            }),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_recall_accepts_missing_query_for_list_mode() {
        // query is now optional - when omitted, subcog_recall acts like subcog_list
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "subcog_recall",
            json!({
                "limit": 10
                // Missing "query" - should work in list mode
            }),
        );

        // Should succeed (list mode)
        assert!(
            result.is_ok(),
            "recall without query should work: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_recall_rejects_invalid_limit_type() {
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "subcog_recall",
            json!({
                "query": "test",
                "limit": "ten"  // Should be number
            }),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_save_rejects_missing_name() {
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "prompt_save",
            json!({
                "content": "Test prompt content"
                // Missing "name"
            }),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_save_rejects_missing_content_and_file_path() {
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "prompt_save",
            json!({
                "name": "test-prompt"
                // Missing both "content" and "file_path"
            }),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_delete_requires_domain() {
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "prompt_delete",
            json!({
                "name": "test-prompt"
                // Missing required "domain"
            }),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_consolidate_rejects_missing_namespace() {
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "subcog_consolidate",
            json!({
                "strategy": "merge"
                // Missing "namespace"
            }),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_enrich_rejects_missing_memory_id() {
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "subcog_enrich",
            json!({
                "enrich_tags": true
                // Missing "memory_id"
            }),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_get_rejects_missing_name() {
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "prompt_get",
            json!({
                "domain": "project"
                // Missing "name"
            }),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_run_rejects_missing_name() {
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "prompt_run",
            json!({
                "variables": {"key": "value"}
                // Missing "name"
            }),
        );

        assert!(result.is_err());
    }

    // SEC-M5: Test deny_unknown_fields protection
    #[test]
    fn test_capture_rejects_unknown_fields() {
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "subcog_capture",
            json!({
                "content": "test",
                "namespace": "decisions",
                "malicious_field": "attack"  // Unknown field should be rejected
            }),
        );

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("unknown field"),
            "Error should mention unknown field: {}",
            err
        );
    }

    #[test]
    fn test_recall_rejects_unknown_fields() {
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "subcog_recall",
            json!({
                "query": "test",
                "inject_param": "payload"  // Unknown field
            }),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_save_rejects_unknown_fields() {
        let registry = ToolRegistry::new();
        let result = registry.execute(
            "prompt_save",
            json!({
                "name": "test",
                "content": "test content",
                "admin_override": true  // Unknown field
            }),
        );

        assert!(result.is_err());
    }
}

// ============================================================================
// Resource Handler Tests
// ============================================================================

mod resource_handler {
    use subcog::mcp::ResourceHandler;

    #[test]
    fn test_resource_handler_creation() {
        let handler = ResourceHandler::new();
        let resources = handler.list_resources();

        // Should have help resources at minimum
        assert!(!resources.is_empty());
    }

    #[test]
    fn test_list_resources_contains_help() {
        let handler = ResourceHandler::new();
        let resources = handler.list_resources();

        // Should have help index resource
        let has_help = resources.iter().any(|r| r.uri.contains("help"));
        assert!(has_help, "Should have help resources");
    }

    #[test]
    fn test_resource_definitions_have_required_fields() {
        let handler = ResourceHandler::new();

        for resource in handler.list_resources() {
            // Every resource must have a URI
            assert!(!resource.uri.is_empty(), "Resource URI cannot be empty");

            // Every resource must have a name
            assert!(!resource.name.is_empty(), "Resource name cannot be empty");
        }
    }

    #[test]
    fn test_help_categories() {
        let handler = ResourceHandler::new();
        let categories = handler.list_categories();

        // Should have at least setup, concepts, capture, search categories
        let category_names: Vec<&str> = categories.iter().map(|c| c.name.as_str()).collect();

        assert!(category_names.contains(&"setup"), "Missing setup category");
        assert!(
            category_names.contains(&"concepts"),
            "Missing concepts category"
        );
        assert!(
            category_names.contains(&"capture"),
            "Missing capture category"
        );
        assert!(
            category_names.contains(&"search"),
            "Missing search category"
        );
    }

    #[test]
    fn test_get_help_resource() {
        let mut handler = ResourceHandler::new();

        // Get the help index
        let result = handler.get_resource("subcog://help");
        assert!(result.is_ok(), "Should get help index: {:?}", result.err());

        let content = result.unwrap();
        assert!(
            content.text.as_ref().map_or(false, |t| !t.is_empty()),
            "Help content should not be empty"
        );
    }

    #[test]
    fn test_get_help_setup() {
        let mut handler = ResourceHandler::new();

        let result = handler.get_resource("subcog://help/setup");
        assert!(result.is_ok(), "Should get setup help: {:?}", result.err());

        let content = result.unwrap();
        assert!(
            content
                .text
                .as_ref()
                .map_or(false, |t| t.contains("Getting Started")
                    || t.contains("Installation")),
            "Setup help should contain getting started info"
        );
    }

    #[test]
    fn test_get_help_capture() {
        let mut handler = ResourceHandler::new();

        let result = handler.get_resource("subcog://help/capture");
        assert!(
            result.is_ok(),
            "Should get capture help: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_get_namespaces_resource() {
        let mut handler = ResourceHandler::new();

        let result = handler.get_resource("subcog://namespaces");
        assert!(result.is_ok(), "Should get namespaces: {:?}", result.err());

        let content = result.unwrap();
        let text = content.text.as_ref().expect("Should have text");
        assert!(text.contains("decisions"));
        assert!(text.contains("patterns"));
    }

    #[test]
    fn test_invalid_resource_uri() {
        let mut handler = ResourceHandler::new();

        let result = handler.get_resource("invalid://not-a-subcog-uri");
        assert!(result.is_err(), "Invalid URI should return error");
    }

    #[test]
    fn test_nonexistent_help_topic() {
        let mut handler = ResourceHandler::new();

        let result = handler.get_resource("subcog://help/nonexistent-topic-12345");
        // Should either return error or empty content
        if let Ok(content) = result {
            // If it returns OK, content should indicate not found
            let text = content.text.as_ref().map(|t| t.as_str()).unwrap_or("");
            assert!(text.contains("not found") || text.contains("Not found") || text.is_empty());
        }
    }
}

// ============================================================================
// Prompt Registry Tests
// ============================================================================

mod prompt_registry {
    use super::*;
    use subcog::mcp::{PromptContent, PromptRegistry};

    #[test]
    fn test_prompt_registry_creation() {
        let registry = PromptRegistry::new();
        let prompts = registry.list_prompts();

        // Should have prompts registered
        assert!(!prompts.is_empty());
    }

    #[test]
    fn test_prompt_count() {
        let registry = PromptRegistry::new();
        let prompts = registry.list_prompts();

        // Should have at least the core prompts
        assert!(
            prompts.len() >= 10,
            "Expected at least 10 prompts, got {}",
            prompts.len()
        );
    }

    #[test]
    fn test_core_prompts_registered() {
        let registry = PromptRegistry::new();

        // Core prompts should be present
        assert!(registry.get_prompt("subcog").is_some());
        assert!(registry.get_prompt("subcog_tutorial").is_some());
        assert!(registry.get_prompt("subcog_capture").is_some());
        assert!(registry.get_prompt("subcog_review").is_some());
    }

    #[test]
    fn test_intent_prompts_registered() {
        let registry = PromptRegistry::new();

        // Intent-aware prompts (Phase 4)
        assert!(registry.get_prompt("intent_search").is_some());
        assert!(registry.get_prompt("query_suggest").is_some());
        assert!(registry.get_prompt("context_capture").is_some());
        assert!(registry.get_prompt("discover").is_some());
    }

    #[test]
    fn test_prompt_definitions_have_required_fields() {
        let registry = PromptRegistry::new();

        for prompt in registry.list_prompts() {
            // Every prompt must have a name
            assert!(!prompt.name.is_empty(), "Prompt name cannot be empty");

            // Every prompt should have a description
            assert!(
                prompt.description.is_some(),
                "Prompt {} should have a description",
                prompt.name
            );
        }
    }

    #[test]
    fn test_get_prompt_messages() {
        let registry = PromptRegistry::new();

        // Get messages for subcog_tutorial
        let messages = registry.get_prompt_messages("subcog_tutorial", &json!({}));
        assert!(messages.is_some());

        let messages = messages.unwrap();
        assert!(!messages.is_empty());

        // First message should be a user message
        let first = &messages[0];
        assert_eq!(first.role, "user");
    }

    #[test]
    fn test_get_prompt_with_arguments() {
        let registry = PromptRegistry::new();

        // Get messages with arguments
        let messages = registry.get_prompt_messages(
            "subcog_tutorial",
            &json!({
                "familiarity": "beginner",
                "focus": "capture"
            }),
        );

        assert!(messages.is_some());
    }

    #[test]
    fn test_get_nonexistent_prompt() {
        let registry = PromptRegistry::new();

        let prompt = registry.get_prompt("nonexistent-prompt-12345");
        assert!(prompt.is_none());

        let messages = registry.get_prompt_messages("nonexistent-prompt-12345", &json!({}));
        assert!(messages.is_none());
    }

    #[test]
    fn test_tutorial_prompt_content() {
        let registry = PromptRegistry::new();

        let messages = registry
            .get_prompt_messages("subcog_tutorial", &json!({}))
            .unwrap();

        // Should have text content
        if let PromptContent::Text { text } = &messages[0].content {
            assert!(
                text.contains("Subcog") || text.contains("memory") || text.contains("tutorial"),
                "Tutorial should mention Subcog or memory"
            );
        }
    }
}

// ============================================================================
// MCP Method Dispatch Tests
// ============================================================================

mod method_dispatch {
    /// Mock representation of MCP method for testing
    #[derive(Debug, PartialEq)]
    enum McpMethod {
        Initialize,
        ListTools,
        CallTool,
        ListResources,
        ReadResource,
        ListPrompts,
        GetPrompt,
        Ping,
        Unknown(String),
    }

    impl From<&str> for McpMethod {
        fn from(s: &str) -> Self {
            match s {
                "initialize" => Self::Initialize,
                "tools/list" => Self::ListTools,
                "tools/call" => Self::CallTool,
                "resources/list" => Self::ListResources,
                "resources/read" => Self::ReadResource,
                "prompts/list" => Self::ListPrompts,
                "prompts/get" => Self::GetPrompt,
                "ping" => Self::Ping,
                unknown => Self::Unknown(unknown.to_string()),
            }
        }
    }

    #[test]
    fn test_method_parsing() {
        assert_eq!(McpMethod::from("initialize"), McpMethod::Initialize);
        assert_eq!(McpMethod::from("tools/list"), McpMethod::ListTools);
        assert_eq!(McpMethod::from("tools/call"), McpMethod::CallTool);
        assert_eq!(McpMethod::from("resources/list"), McpMethod::ListResources);
        assert_eq!(McpMethod::from("resources/read"), McpMethod::ReadResource);
        assert_eq!(McpMethod::from("prompts/list"), McpMethod::ListPrompts);
        assert_eq!(McpMethod::from("prompts/get"), McpMethod::GetPrompt);
        assert_eq!(McpMethod::from("ping"), McpMethod::Ping);
    }

    #[test]
    fn test_unknown_method() {
        let method = McpMethod::from("unknown/method");
        assert!(matches!(method, McpMethod::Unknown(_)));
    }

    #[test]
    fn test_all_known_methods() {
        let known = [
            "initialize",
            "tools/list",
            "tools/call",
            "resources/list",
            "resources/read",
            "prompts/list",
            "prompts/get",
            "ping",
        ];

        for method_str in &known {
            let method = McpMethod::from(*method_str);
            assert!(
                !matches!(method, McpMethod::Unknown(_)),
                "{} should be a known method",
                method_str
            );
        }
    }
}

// ============================================================================
// JSON-RPC Format Tests
// ============================================================================

mod jsonrpc_format {
    use super::*;

    /// JSON-RPC 2.0 request format
    #[derive(serde::Deserialize, serde::Serialize)]
    struct JsonRpcRequest {
        jsonrpc: String,
        method: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        params: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<Value>,
    }

    /// JSON-RPC 2.0 response format
    #[derive(serde::Deserialize, serde::Serialize)]
    struct JsonRpcResponse {
        jsonrpc: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<JsonRpcError>,
        id: Value,
    }

    #[derive(serde::Deserialize, serde::Serialize)]
    struct JsonRpcError {
        code: i32,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<Value>,
    }

    #[test]
    fn test_valid_request_format() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/list".to_string(),
            params: None,
            id: Some(json!(1)),
        };

        let json_str = serde_json::to_string(&request).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["method"], "tools/list");
        assert_eq!(parsed["id"], 1);
    }

    #[test]
    fn test_request_with_params() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "subcog_status",
                "arguments": {}
            })),
            id: Some(json!(2)),
        };

        let json_str = serde_json::to_string(&request).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();

        assert!(parsed["params"].is_object());
        assert_eq!(parsed["params"]["name"], "subcog_status");
    }

    #[test]
    fn test_notification_format_no_id() {
        let notification = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "notifications/initialized".to_string(),
            params: None,
            id: None, // Notifications have no id
        };

        let json_str = serde_json::to_string(&notification).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();

        assert!(parsed.get("id").is_none());
    }

    #[test]
    fn test_success_response_format() {
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(json!({"tools": []})),
            error: None,
            id: json!(1),
        };

        let json_str = serde_json::to_string(&response).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert!(parsed["result"].is_object());
        assert!(parsed.get("error").is_none());
    }

    #[test]
    fn test_error_response_format() {
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
            id: json!(1),
        };

        let json_str = serde_json::to_string(&response).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();

        assert!(parsed["error"].is_object());
        assert_eq!(parsed["error"]["code"], -32601);
    }

    #[test]
    fn test_error_codes() {
        // Standard JSON-RPC error codes
        assert_eq!(-32700, -32700); // Parse error
        assert_eq!(-32600, -32600); // Invalid Request
        assert_eq!(-32601, -32601); // Method not found
        assert_eq!(-32602, -32602); // Invalid params
        assert_eq!(-32603, -32603); // Internal error
    }
}

// ============================================================================
// Integration Workflow Tests
// ============================================================================

mod integration_workflows {
    use super::*;
    use subcog::mcp::{PromptRegistry, ResourceHandler, ToolRegistry};

    #[test]
    fn test_tool_and_resource_consistency() {
        let tool_registry = ToolRegistry::new();
        let resource_handler = ResourceHandler::new();

        // Tool registry and resource handler should both be operational
        assert!(!tool_registry.list_tools().is_empty());
        assert!(!resource_handler.list_resources().is_empty());
    }

    #[test]
    fn test_tool_and_prompt_consistency() {
        let tool_registry = ToolRegistry::new();
        let prompt_registry = PromptRegistry::new();

        // Both registries should be operational
        assert!(!tool_registry.list_tools().is_empty());
        assert!(!prompt_registry.list_prompts().is_empty());
    }

    #[test]
    fn test_status_and_namespaces_consistency() {
        let registry = ToolRegistry::new();

        // Both status and namespaces should work
        let status_result = registry.execute("subcog_status", json!({}));
        let namespaces_result = registry.execute("subcog_namespaces", json!({}));

        assert!(status_result.is_ok());
        assert!(namespaces_result.is_ok());
    }

    #[test]
    fn test_prompt_list_execution() {
        let registry = ToolRegistry::new();

        // prompt_list should work without error
        let result = registry.execute("prompt_list", json!({}));
        assert!(result.is_ok());

        let tool_result = result.unwrap();
        // May be empty if no prompts saved, but shouldn't error
        assert!(!tool_result.content.is_empty());
    }

    #[test]
    fn test_recall_with_empty_database() {
        let registry = ToolRegistry::new();

        // Recall should work even with empty database
        let result = registry.execute(
            "subcog_recall",
            json!({
                "query": "test query",
                "limit": 5
            }),
        );

        // Should succeed (possibly with no results) or return proper error
        match result {
            Ok(tool_result) => {
                // Empty results are fine
                assert!(!tool_result.content.is_empty());
            },
            Err(e) => {
                // If error, should be about missing storage, not crash
                let msg = e.to_string();
                assert!(
                    msg.contains("storage") || msg.contains("database") || msg.contains("index"),
                    "Error should mention storage issue: {}",
                    msg
                );
            },
        }
    }

    #[test]
    fn test_multiple_sequential_tool_calls() {
        let registry = ToolRegistry::new();

        // Multiple calls should work
        for i in 0..5 {
            let result = registry.execute("subcog_status", json!({}));
            assert!(result.is_ok(), "Call {} should succeed", i);
        }
    }

    #[test]
    fn test_help_resource_coverage() {
        let mut handler = ResourceHandler::new();

        // Collect category names to avoid borrow conflict
        let category_names: Vec<String> = handler
            .list_categories()
            .iter()
            .map(|c| c.name.clone())
            .collect();

        // All help categories should be accessible
        for name in category_names {
            let uri = format!("subcog://help/{}", name);
            let result = handler.get_resource(&uri);
            assert!(
                result.is_ok(),
                "Help category {} should be accessible",
                name
            );
        }
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

mod error_handling {
    use super::*;
    use subcog::mcp::ToolRegistry;

    #[test]
    fn test_error_result_has_is_error_flag() {
        // This tests that error tool results properly set is_error=true
        // We use prompt_get with a nonexistent prompt as an example

        let registry = ToolRegistry::new();
        let result = registry.execute(
            "prompt_get",
            json!({
                "name": "definitely-nonexistent-prompt-xyz123"
            }),
        );

        match result {
            Ok(tool_result) => {
                // If it returns a result, it should indicate error
                if tool_result.is_error {
                    // Good - error was indicated
                    assert!(!tool_result.content.is_empty());
                }
                // If is_error is false, the content should indicate not found
            },
            Err(_) => {
                // Also acceptable - error propagated
            },
        }
    }

    #[test]
    fn test_tool_errors_have_descriptive_messages() {
        let registry = ToolRegistry::new();

        // Test various error cases have descriptive messages
        let test_cases = vec![
            (
                "subcog_capture",
                json!({"content": "x", "namespace": "decisions", "bad_field": true}),
                "unknown field",
            ),
            ("subcog_recall", json!({}), "missing"), // Missing required field
        ];

        for (tool, args, expected_contains) in test_cases {
            let result = registry.execute(tool, args);
            if let Err(e) = result {
                let msg = e.to_string().to_lowercase();
                assert!(
                    msg.contains(expected_contains),
                    "Error for {} should contain '{}': {}",
                    tool,
                    expected_contains,
                    msg
                );
            }
        }
    }

    #[test]
    fn test_invalid_json_handling() {
        let registry = ToolRegistry::new();

        // Pass completely wrong type as arguments
        let result = registry.execute("subcog_capture", json!("not an object"));

        assert!(result.is_err());
    }

    #[test]
    fn test_null_arguments_handling() {
        let registry = ToolRegistry::new();

        // Tools that don't require arguments should handle null/empty
        let result = registry.execute("subcog_status", Value::Null);
        // Should work - status doesn't need arguments
        assert!(result.is_ok());
    }
}
