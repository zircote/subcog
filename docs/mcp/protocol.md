# MCP Protocol

Subcog implements the Model Context Protocol (MCP) for AI assistant integration.

## Overview

MCP is a JSON-RPC 2.0 based protocol that enables AI assistants to interact with external tools, resources, and prompts.

## Transport

### stdio (Default)

Communication via standard input/output. Messages are newline-delimited JSON.

```bash
subcog serve
```

**Message Flow:**
```
[Client] -> stdin -> [Subcog Server] -> stdout -> [Client]
 -> stderr -> [Logs]
```

### HTTP

HTTP transport for remote access.

```bash
subcog serve --transport http --port 8080
```

**Endpoints:**
- `POST /` - JSON-RPC endpoint
- `GET /health` - Health check

## Message Format

### Request

```json
{
 "jsonrpc": "2.0",
 "id": 1,
 "method": "tools/call",
 "params": {
 "name": "subcog_capture",
 "arguments": {
 "namespace": "decisions",
 "content": "Use PostgreSQL"
 }
 }
}
```

### Response (Success)

```json
{
 "jsonrpc": "2.0",
 "id": 1,
 "result": {
 "content": [
 {
 "type": "text",
 "text": "{\"id\": \"dc58d23a...\", \"urn\": \"subcog://...\"}"
 }
 ]
 }
}
```

### Response (Error)

```json
{
 "jsonrpc": "2.0",
 "id": 1,
 "error": {
 "code": -32602,
 "message": "Invalid namespace: unknown"
 }
}
```

## Methods

### initialize

Initialize the connection and negotiate capabilities.

**Request:**
```json
{
 "jsonrpc": "2.0",
 "id": 1,
 "method": "initialize",
 "params": {
 "protocolVersion": "2024-11-05",
 "capabilities": {
 "roots": {"listChanged": true}
 },
 "clientInfo": {
 "name": "claude-code",
 "version": "1.0.0"
 }
 }
}
```

**Response:**
```json
{
 "jsonrpc": "2.0",
 "id": 1,
 "result": {
 "protocolVersion": "2024-11-05",
 "capabilities": {
 "tools": {"listChanged": false},
 "resources": {"subscribe": false, "listChanged": false},
 "prompts": {"listChanged": false}
 },
 "serverInfo": {
 "name": "subcog",
 "version": "0.1.0"
 }
 }
}
```

### tools/list

List available tools.

**Request:**
```json
{
 "jsonrpc": "2.0",
 "id": 2,
 "method": "tools/list"
}
```

**Response:**
```json
{
 "jsonrpc": "2.0",
 "id": 2,
 "result": {
 "tools": [
 {
 "name": "subcog_capture",
 "description": "Capture a memory to persistent storage",
 "inputSchema": {
 "type": "object",
 "properties": {
 "content": {"type": "string"},
 "namespace": {"type": "string", "enum": ["decisions", "patterns",...]},
 "tags": {"type": "array", "items": {"type": "string"}},
 "source": {"type": "string"}
 },
 "required": ["content", "namespace"]
 }
 }
 ]
 }
}
```

### tools/call

Execute a tool.

**Request:**
```json
{
 "jsonrpc": "2.0",
 "id": 3,
 "method": "tools/call",
 "params": {
 "name": "subcog_capture",
 "arguments": {
 "namespace": "decisions",
 "content": "Use PostgreSQL for storage"
 }
 }
}
```

**Response:**
```json
{
 "jsonrpc": "2.0",
 "id": 3,
 "result": {
 "content": [
 {
 "type": "text",
 "text": "{\"id\": \"dc58d23a...\", \"urn\": \"subcog://project/decisions/dc58d23a...\"}"
 }
 ]
 }
}
```

### resources/list

List available resources.

**Request:**
```json
{
 "jsonrpc": "2.0",
 "id": 4,
 "method": "resources/list"
}
```

**Response:**
```json
{
 "jsonrpc": "2.0",
 "id": 4,
 "result": {
 "resources": [
 {
 "uri": "subcog://help",
 "name": "Help Documentation",
 "description": "Subcog help and documentation",
 "mimeType": "text/markdown"
 },
 {
 "uri": "subcog://project",
 "name": "Project Memories",
 "description": "List project-scoped memories",
 "mimeType": "application/json"
 }
 ]
 }
}
```

### resources/read

Read a resource.

**Request:**
```json
{
 "jsonrpc": "2.0",
 "id": 5,
 "method": "resources/read",
 "params": {
 "uri": "subcog://project/decisions"
 }
}
```

**Response:**
```json
{
 "jsonrpc": "2.0",
 "id": 5,
 "result": {
 "contents": [
 {
 "uri": "subcog://project/decisions",
 "mimeType": "application/json",
 "text": "{\"count\": 5, \"memories\": [...]}"
 }
 ]
 }
}
```

### prompts/list

List available prompts.

**Request:**
```json
{
 "jsonrpc": "2.0",
 "id": 6,
 "method": "prompts/list"
}
```

**Response:**
```json
{
 "jsonrpc": "2.0",
 "id": 6,
 "result": {
 "prompts": [
 {
 "name": "subcog_capture",
 "description": "Guided memory capture",
 "arguments": [
 {
 "name": "content",
 "description": "Content to capture",
 "required": true
 }
 ]
 }
 ]
 }
}
```

### prompts/get

Get a prompt with arguments.

**Request:**
```json
{
 "jsonrpc": "2.0",
 "id": 7,
 "method": "prompts/get",
 "params": {
 "name": "subcog_capture",
 "arguments": {
 "content": "Important decision"
 }
 }
}
```

**Response:**
```json
{
 "jsonrpc": "2.0",
 "id": 7,
 "result": {
 "description": "Guided memory capture",
 "messages": [
 {
 "role": "user",
 "content": {
 "type": "text",
 "text": "Capture this memory:\n\nContent: Important decision\n\nSuggested namespace: decisions..."
 }
 }
 ]
 }
}
```

## Error Codes

### Standard JSON-RPC Errors

| Code | Message | Description |
|------|---------|-------------|
| -32700 | Parse error | Invalid JSON |
| -32600 | Invalid Request | Invalid request structure |
| -32601 | Method not found | Unknown method |
| -32602 | Invalid params | Invalid parameters |
| -32603 | Internal error | Server error |

### Subcog-Specific Errors

| Code | Message | Description |
|------|---------|-------------|
| -32604 | Memory not found | Memory ID doesn't exist |
| -32605 | Content blocked | Security: secrets/PII detected |
| -32606 | LLM error | LLM provider error |
| -32607 | Prompt not found | Prompt template not found |
| -32608 | Validation error | Input validation failed |
| -32609 | Storage error | Storage backend error |
| -32610 | Sync error | Git sync failed |

## Notifications

Subcog supports these notification methods:

### notifications/tools/list_changed

Sent when tools list changes.

```json
{
 "jsonrpc": "2.0",
 "method": "notifications/tools/list_changed"
}
```

### notifications/resources/list_changed

Sent when resources list changes.

```json
{
 "jsonrpc": "2.0",
 "method": "notifications/resources/list_changed"
}
```

## Capability Negotiation

During initialization, capabilities are negotiated:

**Client Capabilities:**
```json
{
 "roots": {"listChanged": true},
 "sampling": {}
}
```

**Server Capabilities:**
```json
{
 "tools": {"listChanged": false},
 "resources": {"subscribe": false, "listChanged": false},
 "prompts": {"listChanged": false},
 "logging": {}
}
```

## Sampling (LLM Integration)

Subcog can request LLM completions via MCP sampling:

**Request:**
```json
{
 "jsonrpc": "2.0",
 "id": 10,
 "method": "sampling/createMessage",
 "params": {
 "messages": [
 {
 "role": "user",
 "content": {
 "type": "text",
 "text": "Suggest tags for: Use PostgreSQL for storage"
 }
 }
 ],
 "maxTokens": 100
 }
}
```

Used by:
- `subcog_enrich` tool
- `subcog_consolidate` tool
- Intent detection (optional)

## Logging

Log messages are sent as notifications:

```json
{
 "jsonrpc": "2.0",
 "method": "notifications/message",
 "params": {
 "level": "info",
 "logger": "subcog",
 "data": "Captured memory dc58d23a..."
 }
}
```

Log levels: `debug`, `info`, `warning`, `error`

## Best Practices

1. **Batch requests** where possible to reduce round-trips
2. **Use progressive disclosure** - fetch full content only when needed
3. **Handle errors gracefully** - implement retry logic for transient errors
4. **Log at appropriate levels** - use debug for detailed tracing

## See Also

- [Tools](tools.md) - Available MCP tools
- [Resources](resources.md) - Available MCP resources
- [Prompts](./prompts.md) - Available MCP prompts
- [MCP Specification](https://modelcontextprotocol.io/) - Official MCP documentation
