# subcog serve

Run the MCP (Model Context Protocol) server.

## Synopsis

```
subcog serve [OPTIONS]
```

## Description

The `serve` command starts an MCP server that exposes Subcog functionality through the Model Context Protocol. This enables integration with AI assistants like Claude Code.

## Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--transport` | `-t` | Transport type (stdio, http) | `stdio` |
| `--host` | | HTTP server host | `127.0.0.1` |
| `--port` | `-p` | HTTP server port | `8080` |
| `--capabilities` | | Show server capabilities | `false` |

## Transports

### stdio (Default)

Uses standard input/output for communication. Ideal for integration with IDE extensions.

```bash
subcog serve
# or explicitly
subcog serve -t stdio
```

### HTTP

Runs an HTTP server for remote access.

```bash
subcog serve -t http --port 9000
```

**Security Note**: HTTP transport has no built-in authentication. Use only in trusted environments.

## Claude Code Integration

Add to your Claude Code configuration:

**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
**Linux**: `~/.config/Claude/claude_desktop_config.json`
**Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "subcog": {
      "command": "npx",
      "args": ["-y", "@zircote/subcog", "serve"],
      "env": {
        "SUBCOG_LOG_LEVEL": "info"
      }
    }
  }
}
```

## Server Capabilities

View what the server exposes:

```bash
subcog serve --capabilities
```

Output:
```json
{
  "tools": [
    "subcog_capture",
    "subcog_recall",
    "subcog_status",
    "subcog_namespaces",
    "subcog_consolidate",
    "subcog_enrich",
    "subcog_sync",
    "subcog_reindex",
    "prompt_save",
    "prompt_list",
    "prompt_get",
    "prompt_run",
    "prompt_delete"
  ],
  "resources": [
    "subcog://help",
    "subcog://project",
    "subcog://project/{namespace}",
    "subcog://memory/{id}",
    "subcog://topics",
    "subcog://_prompts"
  ],
  "prompts": [
    "subcog_capture",
    "subcog_recall",
    "subcog_browse",
    "subcog_tutorial",
    "intent_search",
    "discover",
    "generate_decision"
  ]
}
```

## MCP Protocol

The server implements JSON-RPC 2.0 over the selected transport.

### Message Format

Request:
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

Response:
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

## Logging

Server logs are written to stderr. Configure verbosity:

```bash
SUBCOG_LOG_LEVEL=debug subcog serve
```

Log levels:
- `trace` - Very detailed debugging
- `debug` - Debugging information
- `info` - Normal operation
- `warn` - Warnings
- `error` - Errors only

## Health Check

For HTTP transport, check server health:

```bash
curl http://localhost:8080/health
```

Response:
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 3600
}
```

## Examples

### Basic Server

```bash
subcog serve
```

### HTTP Server

```bash
subcog serve -t http -p 9000
```

### With Debug Logging

```bash
SUBCOG_LOG_LEVEL=debug subcog serve
```

### Background Process

```bash
subcog serve &> /tmp/subcog.log &
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Server stopped cleanly |
| 1 | Server error |
| 3 | Configuration error |

## See Also

- [MCP Tools](../mcp/tools.md) - Available MCP tools
- [MCP Resources](../mcp/resources.md) - Available MCP resources
- [MCP Prompts](../mcp/prompts.md) - Available MCP prompts
- [hook](hook.md) - Claude Code hooks
