//! MCP server setup and lifecycle.
//!
//! Implements a JSON-RPC based MCP server over stdio or HTTP transport.

use crate::mcp::{PromptRegistry, ResourceHandler, ToolRegistry};
use crate::services::ServiceContainer;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};

/// MCP protocol version.
const PROTOCOL_VERSION: &str = "2024-11-05";

/// Server name.
const SERVER_NAME: &str = "subcog";

/// Transport type for the MCP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Transport {
    /// Standard input/output (default for Claude Desktop).
    #[default]
    Stdio,
    /// HTTP transport.
    Http,
}

/// MCP server for subcog.
pub struct McpServer {
    /// Tool registry.
    tools: ToolRegistry,
    /// Resource handler.
    resources: ResourceHandler,
    /// Prompt registry.
    prompts: PromptRegistry,
    /// Transport type.
    transport: Transport,
    /// HTTP port (if using HTTP transport).
    port: u16,
}

impl McpServer {
    /// Creates a new MCP server.
    #[must_use]
    pub fn new() -> Self {
        // Try to initialize RecallService for memory browsing
        let resources = Self::try_init_resources();

        Self {
            tools: ToolRegistry::new(),
            resources,
            prompts: PromptRegistry::new(),
            transport: Transport::Stdio,
            port: 3000,
        }
    }

    /// Tries to initialize `ResourceHandler` with `RecallService`.
    ///
    /// Uses domain-scoped index (project-local `.subcog/index.db`).
    fn try_init_resources() -> ResourceHandler {
        // Use ServiceContainer for domain-scoped index access
        ServiceContainer::from_current_dir()
            .and_then(|services| services.recall())
            .map_or_else(
                |_| ResourceHandler::new(),
                |recall| ResourceHandler::new().with_recall_service(recall),
            )
    }

    /// Sets the transport type.
    #[must_use]
    pub const fn with_transport(mut self, transport: Transport) -> Self {
        self.transport = transport;
        self
    }

    /// Sets the HTTP port.
    #[must_use]
    pub const fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Starts the MCP server.
    ///
    /// # Errors
    ///
    /// Returns an error if the server fails to start.
    pub fn start(&self) -> Result<()> {
        match self.transport {
            Transport::Stdio => self.run_stdio(),
            Transport::Http => self.run_http(),
        }
    }

    /// Runs the server over stdio.
    fn run_stdio(&self) -> Result<()> {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        let reader = BufReader::new(stdin.lock());

        for line in reader.lines() {
            let line = line.map_err(|e| Error::OperationFailed {
                operation: "read_stdin".to_string(),
                cause: e.to_string(),
            })?;

            if line.is_empty() {
                continue;
            }

            let response = self.handle_request(&line);

            writeln!(stdout, "{response}").map_err(|e| Error::OperationFailed {
                operation: "write_stdout".to_string(),
                cause: e.to_string(),
            })?;

            stdout.flush().map_err(|e| Error::OperationFailed {
                operation: "flush_stdout".to_string(),
                cause: e.to_string(),
            })?;
        }

        Ok(())
    }

    /// Runs the server over HTTP.
    fn run_http(&self) -> Result<()> {
        // HTTP transport would require an async runtime and HTTP server
        // For now, return an error indicating it's not fully implemented
        Err(Error::NotImplemented(format!(
            "HTTP transport on port {} requires async runtime",
            self.port
        )))
    }

    /// Handles a JSON-RPC request.
    fn handle_request(&self, request: &str) -> String {
        let parsed: std::result::Result<JsonRpcRequest, _> = serde_json::from_str(request);

        match parsed {
            Ok(req) => {
                let result = self.dispatch_method(&req.method, req.params);
                self.format_response(req.id, result)
            },
            Err(e) => self.format_error(None, -32700, &format!("Parse error: {e}")),
        }
    }

    /// Dispatches a method call.
    fn dispatch_method(&self, method: &str, params: Option<Value>) -> DispatchResult {
        match method {
            "initialize" => self.handle_initialize(params),
            "tools/list" => self.handle_list_tools(),
            "tools/call" => self.handle_call_tool(params),
            "resources/list" => self.handle_list_resources(),
            "resources/read" => self.handle_read_resource(params),
            "prompts/list" => self.handle_list_prompts(),
            "prompts/get" => self.handle_get_prompt(params),
            "ping" => Ok(serde_json::json!({})),
            _ => Err((-32601, format!("Method not found: {method}"))),
        }
    }

    /// Handles the initialize method.
    fn handle_initialize(&self, _params: Option<Value>) -> DispatchResult {
        Ok(serde_json::json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": {},
                "resources": {},
                "prompts": {},
                "sampling": {}
            },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": env!("CARGO_PKG_VERSION")
            }
        }))
    }

    /// Handles tools/list.
    fn handle_list_tools(&self) -> DispatchResult {
        let tools: Vec<Value> = self
            .tools
            .list_tools()
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": t.input_schema
                })
            })
            .collect();

        Ok(serde_json::json!({ "tools": tools }))
    }

    /// Handles tools/call.
    fn handle_call_tool(&self, params: Option<Value>) -> DispatchResult {
        let params = params.ok_or((-32602, "Missing params".to_string()))?;

        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or((-32602, "Missing tool name".to_string()))?;

        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        match self.tools.execute(name, arguments) {
            Ok(result) => Ok(serde_json::json!({
                "content": result.content,
                "isError": result.is_error
            })),
            Err(e) => Ok(serde_json::json!({
                "content": [{ "type": "text", "text": e.to_string() }],
                "isError": true
            })),
        }
    }

    /// Handles resources/list.
    fn handle_list_resources(&self) -> DispatchResult {
        let resources: Vec<Value> = self
            .resources
            .list_resources()
            .iter()
            .map(|r| {
                serde_json::json!({
                    "uri": r.uri,
                    "name": r.name,
                    "description": r.description,
                    "mimeType": r.mime_type
                })
            })
            .collect();

        Ok(serde_json::json!({ "resources": resources }))
    }

    /// Handles resources/read.
    fn handle_read_resource(&self, params: Option<Value>) -> DispatchResult {
        let params = params.ok_or((-32602, "Missing params".to_string()))?;

        let uri = params
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or((-32602, "Missing resource URI".to_string()))?;

        match self.resources.get_resource(uri) {
            Ok(content) => Ok(serde_json::json!({
                "contents": [{
                    "uri": content.uri,
                    "mimeType": content.mime_type,
                    "text": content.text
                }]
            })),
            Err(e) => Err((-32603, e.to_string())),
        }
    }

    /// Handles prompts/list.
    fn handle_list_prompts(&self) -> DispatchResult {
        let prompts: Vec<Value> = self
            .prompts
            .list_prompts()
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "description": p.description,
                    "arguments": p.arguments.iter().map(|a| {
                        serde_json::json!({
                            "name": a.name,
                            "description": a.description,
                            "required": a.required
                        })
                    }).collect::<Vec<Value>>()
                })
            })
            .collect();

        Ok(serde_json::json!({ "prompts": prompts }))
    }

    /// Handles prompts/get.
    fn handle_get_prompt(&self, params: Option<Value>) -> DispatchResult {
        let params = params.ok_or((-32602, "Missing params".to_string()))?;

        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or((-32602, "Missing prompt name".to_string()))?;

        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        match self.prompts.get_prompt_messages(name, &arguments) {
            Some(messages) => {
                let msgs: Vec<Value> = messages
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "role": m.role,
                            "content": m.content
                        })
                    })
                    .collect();

                Ok(serde_json::json!({ "messages": msgs }))
            },
            None => Err((-32602, format!("Unknown prompt: {name}"))),
        }
    }

    /// Formats a successful response.
    fn format_response(&self, id: Option<Value>, result: DispatchResult) -> String {
        match result {
            Ok(value) => {
                let response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(value),
                    error: None,
                };
                serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string())
            },
            Err((code, message)) => self.format_error(id, code, &message),
        }
    }

    /// Formats an error response.
    fn format_error(&self, id: Option<Value>, code: i32, message: &str) -> String {
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
                data: None,
            }),
        };
        serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string())
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Result type for method dispatch.
type DispatchResult = std::result::Result<Value, (i32, String)>;

/// JSON-RPC request.
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    /// JSON-RPC version (required by protocol but not used in code).
    #[serde(rename = "jsonrpc")]
    _jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

/// JSON-RPC response.
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

/// JSON-RPC error.
#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_creation() {
        let server = McpServer::new();
        assert_eq!(server.transport, Transport::Stdio);
    }

    #[test]
    fn test_with_transport() {
        let server = McpServer::new()
            .with_transport(Transport::Http)
            .with_port(8080);
        assert_eq!(server.transport, Transport::Http);
        assert_eq!(server.port, 8080);
    }

    #[test]
    fn test_handle_initialize() {
        let server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let response = server.handle_request(request);

        assert!(response.contains("protocolVersion"));
        assert!(response.contains(PROTOCOL_VERSION));
        assert!(response.contains(SERVER_NAME));
    }

    #[test]
    fn test_handle_list_tools() {
        let server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        let response = server.handle_request(request);

        assert!(response.contains("tools"));
        assert!(response.contains("subcog_capture"));
        assert!(response.contains("subcog_recall"));
    }

    #[test]
    fn test_handle_list_resources() {
        let server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"resources/list"}"#;
        let response = server.handle_request(request);

        assert!(response.contains("resources"));
        assert!(response.contains("subcog://help"));
    }

    #[test]
    fn test_handle_list_prompts() {
        let server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"prompts/list"}"#;
        let response = server.handle_request(request);

        assert!(response.contains("prompts"));
        assert!(response.contains("subcog_tutorial"));
    }

    #[test]
    fn test_handle_call_tool() {
        let server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"subcog_status","arguments":{}}}"#;
        let response = server.handle_request(request);

        assert!(response.contains("content"));
        assert!(response.contains("version"));
    }

    #[test]
    fn test_handle_read_resource() {
        let server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"subcog://help"}}"#;
        let response = server.handle_request(request);

        assert!(response.contains("contents"));
        assert!(response.contains("Subcog Help"));
    }

    #[test]
    fn test_handle_get_prompt() {
        let server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"prompts/get","params":{"name":"subcog_tutorial","arguments":{"familiarity":"beginner"}}}"#;
        let response = server.handle_request(request);

        assert!(response.contains("messages"));
    }

    #[test]
    fn test_handle_ping() {
        let server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let response = server.handle_request(request);

        assert!(response.contains("result"));
    }

    #[test]
    fn test_handle_unknown_method() {
        let server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"unknown/method"}"#;
        let response = server.handle_request(request);

        assert!(response.contains("error"));
        assert!(response.contains("-32601"));
    }

    #[test]
    fn test_handle_parse_error() {
        let server = McpServer::new();
        let request = "not valid json";
        let response = server.handle_request(request);

        assert!(response.contains("error"));
        assert!(response.contains("-32700"));
    }

    #[test]
    fn test_handle_missing_params() {
        let server = McpServer::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call"}"#;
        let response = server.handle_request(request);

        assert!(response.contains("error"));
    }
}
