---
document_type: architecture
project_id: SPEC-2026-01-04-001
version: 1.0.0
last_updated: 2026-01-04T04:30:00Z
status: draft
---

# MCP Server JSON-RPC Notification Compliance - Technical Architecture

## System Overview

The fix modifies the MCP server's message processing pipeline to detect and properly handle JSON-RPC notifications. The change is localized to `src/mcp/server.rs` with minimal impact on the rest of the system.

### Architecture Diagram

```
                    ┌─────────────────────────────────────────────────────────┐
                    │                    MCP Server                            │
                    │                                                          │
  stdin ──────────► │  ┌──────────┐    ┌─────────────────┐    ┌────────────┐ │
                    │  │  Parse   │───►│ Notification    │───►│  Dispatch  │ │
                    │  │  JSON    │    │ Detection (NEW) │    │  Method    │ │
                    │  └──────────┘    └─────────────────┘    └────────────┘ │
                    │                         │                      │        │
                    │                         │ is_notification?     │        │
                    │                         ▼                      ▼        │
                    │                   ┌──────────┐          ┌──────────┐   │
                    │                   │  Skip    │          │  Format  │   │
                    │                   │ Response │          │ Response │   │
                    │                   └──────────┘          └──────────┘   │
                    │                         │                      │        │
                    │                         ▼                      ▼        │
  stdout ◄───────── │                    (nothing)              response      │
                    │                                                          │
                    └─────────────────────────────────────────────────────────┘
```

### Key Design Decisions

1. **Early detection**: Check for notification immediately after parsing, before dispatch
2. **No response path**: Notifications skip the entire response formatting and output
3. **Error response fix**: Ensure `id` is always present in error responses (use `null` as fallback)

## Component Design

### Component 1: Notification Detection

**Location**: `src/mcp/server.rs` - new helper method

**Purpose**: Determine if a parsed message is a notification

**Implementation**:

```rust
impl McpServer {
    /// Returns true if the request is a notification (no id field).
    /// Per JSON-RPC 2.0: "A Notification is a Request object without an 'id' member."
    fn is_notification(request: &JsonRpcRequest) -> bool {
        request.id.is_none()
    }
}
```

**Rationale**: Simple, explicit check. The `id` field is already `Option<Value>` in `JsonRpcRequest`.

### Component 2: Response Suppression

**Location**: `src/mcp/server.rs` - `process_request()` and `handle_http_request()`

**Purpose**: Skip response generation and output for notifications

**Current Code** (lines 654-664):

```rust
// Current: Always processes and returns response
let result = self.dispatch_method(&req.method, req.params);
self.format_response(req.id, result)
```

**New Code**:

```rust
// Check if notification before processing
if Self::is_notification(&req) {
    // Log at debug level for observability
    tracing::debug!(method = %req.method, "Received notification, no response");

    // Increment notification metric
    metrics::increment_counter!(
        "mcp_notifications_total",
        "method" => req.method.clone()
    );

    // Return empty string - caller checks and skips output
    return String::new();
}

// Normal request processing continues...
let result = self.dispatch_method(&req.method, req.params);
self.format_response(req.id, result)
```

**Caller Changes** (stdio transport, lines 441-449):

```rust
// Current: Always writes response
let response = self.process_request(&request_str);
writeln!(stdout, "{response}")?;

// New: Skip write if empty (notification)
let response = self.process_request(&request_str);
if !response.is_empty() {
    writeln!(stdout, "{response}")?;
}
```

### Component 3: Error Response `id` Fix

**Location**: `src/mcp/server.rs` - `JsonRpcResponse` struct and serialization

**Current Issue**: The `id` field has `#[serde(skip_serializing_if = "Option::is_none")]`, so when `id` is `None`, it's omitted from the JSON output.

**Problem**: For error responses, JSON-RPC 2.0 requires:
> "id - This member is REQUIRED. It MUST be the same as the value of the id member in the Request Object. If there was an error in detecting the id in the Request object (e.g. Parse error/Invalid Request), it MUST be Null."

**Solution**: Remove `skip_serializing_if` from error responses, or always include `id` with explicit `null` for parse errors.

**Option A - Separate error response struct**:

```rust
#[derive(Debug, Serialize)]
struct JsonRpcErrorResponse {
    jsonrpc: String,
    id: Value,  // Always present, can be null
    error: JsonRpcError,
}
```

**Option B - Conditional serialization** (simpler):

```rust
fn format_error(&self, id: Option<Value>, code: i32, message: &str) -> String {
    let response = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: Some(id.unwrap_or(Value::Null)),  // Always Some, may contain null
        result: None,
        error: Some(JsonRpcError {
            code,
            message: message.to_string(),
            data: None,
        }),
    };
    serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string())
}
```

**Recommendation**: Option B - minimal change, ensures `id` is always present in error responses.

## Data Flow

### Notification Flow (NEW)

```
1. stdin: {"jsonrpc":"2.0","method":"notifications/initialized"}
2. Parse: JsonRpcRequest { id: None, method: "notifications/initialized", ... }
3. is_notification() → true
4. Log: debug!("Received notification, no response")
5. Metric: mcp_notifications_total{method="notifications/initialized"} += 1
6. Return: "" (empty string)
7. Caller: if !response.is_empty() { ... } → skipped
8. stdout: (nothing)
```

### Request Flow (unchanged)

```
1. stdin: {"jsonrpc":"2.0","id":1,"method":"initialize","params":{...}}
2. Parse: JsonRpcRequest { id: Some(1), method: "initialize", ... }
3. is_notification() → false
4. Dispatch: handle_initialize(params)
5. Format: JsonRpcResponse { id: Some(1), result: {...}, ... }
6. Return: "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{...}}"
7. Caller: if !response.is_empty() { ... } → writes response
8. stdout: {"jsonrpc":"2.0","id":1,"result":{...}}
```

### Parse Error Flow (fixed)

```
1. stdin: {"invalid json
2. Parse: Err(serde_json::Error)
3. Format error with id: null
4. stdout: {"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}
```

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_notification_with_id() {
        let req = JsonRpcRequest {
            _jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(1.into())),
            method: "initialize".to_string(),
            params: None,
        };
        assert!(!McpServer::is_notification(&req));
    }

    #[test]
    fn test_is_notification_without_id() {
        let req = JsonRpcRequest {
            _jsonrpc: "2.0".to_string(),
            id: None,
            method: "notifications/initialized".to_string(),
            params: None,
        };
        assert!(McpServer::is_notification(&req));
    }

    #[test]
    fn test_notification_returns_empty_response() {
        let mut server = McpServer::new();
        let notification = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let response = server.process_request(notification);
        assert!(response.is_empty());
    }

    #[test]
    fn test_error_response_includes_id_null() {
        let server = McpServer::new();
        let error_response = server.format_error(None, -32700, "Parse error");
        let parsed: Value = serde_json::from_str(&error_response).unwrap();
        assert!(parsed.get("id").is_some());
        assert!(parsed["id"].is_null());
    }
}
```

### Integration Tests

```rust
#[test]
fn test_notification_no_output() {
    // Spawn subcog serve, send notification, verify no response
    let output = Command::new("target/debug/subcog")
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start server");

    // Send initialize (request) + notifications/initialized (notification)
    let input = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}"#;

    // Write input, read output
    // Expect exactly 1 response (for initialize), not 2
}
```

## Metrics & Observability

### New Metric

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `mcp_notifications_total` | Counter | `method` | Count of notifications received |

### Tracing

- Debug log for each notification received
- No change to existing request tracing

## Deployment Considerations

### Backward Compatibility

- Clients should not be relying on error responses for notifications
- The fix makes the server more compliant, not less
- Existing request/response handling is unchanged

### Rollout

- No configuration changes required
- No migration needed
- Drop-in replacement

## Summary of Changes

| File | Change | Lines (est.) |
|------|--------|--------------|
| `src/mcp/server.rs` | Add `is_notification()` helper | +5 |
| `src/mcp/server.rs` | Add notification check in `process_request()` | +10 |
| `src/mcp/server.rs` | Conditional response output in stdio loop | +3 |
| `src/mcp/server.rs` | Conditional response output in http handler | +3 |
| `src/mcp/server.rs` | Fix `format_error()` to always include `id` | +2 |
| `src/mcp/server.rs` | Add unit tests | +40 |
| **Total** | | ~65 lines |
