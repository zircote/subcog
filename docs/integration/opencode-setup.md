# Subcog Integration Guide for OpenCode

This guide provides instructions for integrating Subcog's persistent memory system with [OpenCode](https://github.com/opencode-ai/opencode), the open-source AI coding assistant.

## Overview

OpenCode can interact with Subcog through:
1. **MCP Server** - Native Model Context Protocol support
2. **CLI commands** - Direct shell access
3. **Configuration** - Protocol guidance in config files

---

## Quick Start

### 1. Install Subcog

```bash
# Install from crates.io
cargo install subcog

# Verify installation
subcog --version
subcog status
```

### 2. Configure MCP Server

Add to your OpenCode configuration (`~/.config/opencode/config.json` or equivalent):

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

### 3. Verify Connection

Start OpenCode and verify Subcog tools are available:

```
> /tools
subcog_capture    - Store a memory
subcog_recall     - Search memories
subcog_status     - System health
...
```

---

## Configuration File Integration

Add to your OpenCode system prompt or configuration:

```markdown
## Subcog Memory Protocol

Subcog provides persistent memory across coding sessions via MCP tools.

### Required Workflow

1. **Session Start**: Call `subcog_status` to verify system health
2. **Before Responses**: Call `subcog_recall` with relevant keywords
3. **Capture Knowledge**: Use `subcog_capture` for decisions, patterns, learnings

### MCP Tools

| Tool | Purpose |
|------|---------|
| `subcog_capture` | Store memory (required: content, namespace) |
| `subcog_recall` | Search memories (semantic + text hybrid) |
| `subcog_status` | Check system health |
| `subcog_get` | Get memory by ID |
| `subcog_update` | Update memory |
| `subcog_delete` | Remove memory |

### Namespaces

| Namespace | When to Use |
|-----------|-------------|
| `decisions` | Architecture choices, technology selections |
| `patterns` | Coding conventions, standards |
| `learnings` | Debugging insights, gotchas |
| `context` | Project background |
| `tech-debt` | Known issues, TODOs |

### Capture Signals

Capture immediately when detecting:
- "we decided", "going with" → `decisions`
- "always", "never", "convention" → `patterns`
- "turns out", "gotcha" → `learnings`
- "TODO", "temporary" → `tech-debt`

### Search Filters

```
ns:decisions tag:database since:30d
ns:patterns source:src/api/*
-tag:deprecated
```
```

---

## Hooks Configuration

If OpenCode supports hooks, configure Subcog hooks:

```json
{
  "hooks": {
    "session_start": {
      "command": "subcog hook session-start"
    },
    "pre_response": {
      "command": "subcog hook user-prompt-submit \"$PROMPT\""
    },
    "post_tool": {
      "command": "subcog hook post-tool-use"
    }
  }
}
```

---

## CLI Fallback

If MCP is not available, use CLI commands directly:

```bash
# Capture a memory
subcog capture -n decisions -c "Using SQLite for persistence" -t database,sqlite

# Search memories
subcog recall "database architecture"

# Filter search
subcog recall --filter "ns:patterns tag:rust"

# Get status
subcog status
```

---

## Project-Level Configuration

Create `.opencode/subcog.md` in your project root:

```markdown
# Subcog Memory Protocol

## Project Context

This project uses Subcog for persistent memory. Always check existing memories before making decisions.

## Workflow

1. Search: `subcog_recall "relevant keywords"`
2. Review existing decisions in `ns:decisions`
3. Capture new decisions and learnings

## Project-Specific Tags

Use these tags for this project:
- `api` - API-related decisions
- `frontend` - UI decisions
- `backend` - Server-side decisions
- `testing` - Test strategy decisions
```

---

## Tool Reference

### subcog_capture

```yaml
subcog_capture:
  content: "Decided to use PostgreSQL for ACID compliance"
  namespace: decisions
  tags: [database, postgresql, acid]
  source: docs/architecture.md
```

### subcog_recall

```yaml
subcog_recall:
  query: "database selection"
  filter: "ns:decisions since:30d"
  limit: 10
  detail: medium
```

### subcog_status

```yaml
subcog_status: {}
```

Returns:
```json
{
  "status": "healthy",
  "memory_count": 42,
  "namespaces": {
    "decisions": 15,
    "patterns": 12,
    "learnings": 10,
    "context": 5
  }
}
```

---

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_LOG_LEVEL` | Logging verbosity | `info` |
| `SUBCOG_DEDUP_ENABLED` | Deduplication | `true` |
| `SUBCOG_SEARCH_INTENT_ENABLED` | Intent detection | `true` |

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| MCP tools not found | Check config path, restart OpenCode |
| Connection refused | Ensure `subcog serve` runs |
| No memories | Check `subcog status`, verify domain |
| Slow search | Run `subcog reindex` |

---

## See Also

- [MCP Integration](../prompts/mcp.md) - MCP tool reference
- [CLI Reference](../cli/README.md) - Full CLI documentation
- [Configuration](../configuration/README.md) - Configuration options
