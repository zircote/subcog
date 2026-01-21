# MCP Integration

Subcog implements the Model Context Protocol (MCP) to provide AI assistants with access to persistent memory capabilities.

## Overview

The MCP server exposes three types of capabilities:

| Type | Description | Count |
|------|-------------|-------|
| [Tools](tools.md) | Callable functions for memory operations | ~22 |
| [Resources](resources.md) | URI-based data access | 26+ |
| [Prompts](./prompts.md) | Pre-defined prompt templates | 11 |

## Quick Reference

### Memory Tools

| Tool | Description |
|------|-------------|
| `subcog_capture` | Capture a memory |
| `subcog_recall` | Search memories (omit query to list all) |
| `subcog_get` | Retrieve a memory by ID |
| `subcog_update` | Update memory content and/or tags |
| `subcog_delete` | Delete a memory (soft or hard) |
| `subcog_status` | System status |
| `subcog_namespaces` | List namespaces |
| `subcog_consolidate` | Merge similar memories |
| `subcog_enrich` | Enhance with LLM |
| `subcog_reindex` | Rebuild search index |

### Consolidated Tools (v0.8.0+)

| Tool | Actions | Description |
|------|---------|-------------|
| `subcog_prompts` | save, list, get, run, delete | Prompt template management |
| `subcog_templates` | save, list, get, render, delete | Context template management |
| `subcog_graph` | neighbors, path, stats, visualize | Knowledge graph operations |
| `subcog_entities` | create, get, list, delete, extract, merge | Entity management |
| `subcog_relationships` | create, get, list, delete, infer | Relationship management |
| `subcog_groups` | create, list, get, add_member, remove_member, update_role, delete | Group management (feature-gated) |

### Deprecated Tools

| Tool | Replacement |
|------|-------------|
| `subcog_sync` | SQLite is now authoritative |
| `subcog_list` | Use `subcog_recall` without query |
| `prompt_save/list/get/run/delete` | Use `subcog_prompts` with action |
| `context_template_*` | Use `subcog_templates` with action |
| `subcog_graph_query/visualize` | Use `subcog_graph` with operation |
| `subcog_extract_entities` | Use `subcog_entities` with `action: extract` |
| `subcog_entity_merge` | Use `subcog_entities` with `action: merge` |
| `subcog_relationship_infer` | Use `subcog_relationships` with `action: infer` |

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
      "command": "npx",
      "args": ["-y", "@zircote/subcog", "serve"],
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
