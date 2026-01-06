# MCP Integration

Subcog implements the Model Context Protocol (MCP) to provide AI assistants with access to persistent memory capabilities.

## Overview

The MCP server exposes two types of capabilities:

| Type | Description | Count |
|------|-------------|-------|
| [Tools](tools.md) | Callable functions for memory operations | 13 |
| [Resources](resources.md) | URI-based data access | 26+ |

## Quick Reference

### Memory Tools

| Tool | Description |
|------|-------------|
| `subcog_capture` | Capture a memory |
| `subcog_recall` | Search memories |
| `subcog_status` | System status |
| `subcog_namespaces` | List namespaces |
| `subcog_consolidate` | Merge similar memories |
| `subcog_enrich` | Enhance with LLM |
| `subcog_reindex` | Rebuild search index |
| `prompt_understanding` | Guidance for using Subcog MCP tools |

### Prompt Tools

| Tool | Description |
|------|-------------|
| `prompt_save` | Save a prompt template |
| `prompt_list` | List prompts |
| `prompt_get` | Get a prompt |
| `prompt_run` | Execute a prompt |
| `prompt_delete` | Delete a prompt |

### Key Resources

| Resource | Description |
|----------|-------------|
| `subcog://help` | Help documentation |
| `subcog://project` | Project memories |
| `subcog://memory/{id}` | Fetch memory by ID |
| `subcog://topics` | List all topics |
| `subcog://_prompts` | List prompt templates |

## Starting the MCP Server

### stdio Transport (Default)

```bash
subcog serve
```

### HTTP Transport

```bash
subcog serve --transport http --port 8080
```

## Claude Code Configuration

Add to your Claude Code settings:

**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
**Linux**: `~/.config/Claude/claude_desktop_config.json`
**Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "subcog": {
      "command": "subcog",
      "args": ["serve"],
      "env": {
        "SUBCOG_LOG_LEVEL": "info"
      }
    }
  }
}
```

## Protocol Details

See [Protocol](protocol.md) for:
- JSON-RPC message format
- Error handling
- Transport specifications
- Capability negotiation

## Usage Examples

### Capture a Memory

```json
{
  "method": "tools/call",
  "params": {
    "name": "subcog_capture",
    "arguments": {
      "namespace": "decisions",
      "content": "Use PostgreSQL for storage",
      "tags": ["database", "architecture"]
    }
  }
}
```

### Search Memories

```json
{
  "method": "tools/call",
  "params": {
    "name": "subcog_recall",
    "arguments": {
      "query": "database storage",
      "mode": "hybrid",
      "limit": 10
    }
  }
}
```

### Read a Resource

```json
{
  "method": "resources/read",
  "params": {
    "uri": "subcog://project/decisions"
  }
}
```

### Execute a Prompt

```json
{
  "method": "prompts/get",
  "params": {
    "name": "subcog_capture",
    "arguments": {
      "content": "Important decision made today"
    }
  }
}
```

## Progressive Disclosure

Subcog implements progressive disclosure to optimize token usage:

1. **List endpoints** return minimal data (id, namespace, tags, uri)
2. **Fetch endpoints** return full content
3. **Search tool** supports detail levels (light, medium, everything)

See [URN Guide](../URN-GUIDE.md) for complete addressing documentation.

## Error Handling

All tools return errors in MCP format:

```json
{
  "error": {
    "code": -32602,
    "message": "Invalid namespace: unknown"
  }
}
```

Common error codes:
| Code | Meaning |
|------|---------|
| -32600 | Invalid request |
| -32601 | Method not found |
| -32602 | Invalid params |
| -32603 | Internal error |
