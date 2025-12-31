# Subcog Quickstart Guide

Get up and running with Subcog in 5 minutes.

## Prerequisites

- Rust 1.85 or later
- Git
- Claude Code (optional, for IDE integration)

## Installation

### Build from Source

```bash
# Clone the repository
git clone https://github.com/zircote/subcog.git
cd subcog

# Build release binary
cargo build --release

# Install to PATH (optional)
cargo install --path .
```

### Verify Installation

```bash
subcog --version
subcog status
```

## Basic Usage

### Capture a Memory

```bash
# Capture a decision
subcog capture --namespace decisions "Use PostgreSQL for primary storage"

# Capture a learning with tags
subcog capture --namespace learnings --tags "rust,error-handling" \
  "Always use Result types, never unwrap in library code"

# Capture with source reference
subcog capture --namespace patterns --source "src/main.rs" \
  "Builder pattern for complex configuration"
```

### Search Memories

```bash
# Simple search
subcog recall "database storage"

# Filter by namespace
subcog recall --filter "ns:decisions" "storage"

# Filter by tags
subcog recall --filter "tag:rust" "error handling"

# Combine filters
subcog recall --filter "ns:learnings since:7d" "debugging"
```

### Check Status

```bash
subcog status
```

Output:
```
Subcog Memory System
────────────────────
Repository: /path/to/project
Domain: project (zircote/subcog)

Storage:
  Persistence: Git Notes (refs/notes/subcog)
  Index: SQLite + FTS5
  Vector: usearch (HNSW)

Statistics:
  Total memories: 42
  By namespace:
    decisions: 12
    patterns: 8
    learnings: 15
    context: 7
```

## Claude Code Integration

### Configure MCP Server

Add Subcog to your Claude Code configuration:

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

### Configure Hooks

Create `hooks/hooks.json` in your project:

```json
{
  "hooks": [
    {
      "matcher": { "event": "session_start" },
      "hooks": [{
        "type": "command",
        "command": "subcog hook session-start"
      }]
    },
    {
      "matcher": { "event": "user_prompt_submit" },
      "hooks": [{
        "type": "command",
        "command": "sh -c 'subcog hook user-prompt-submit \"$PROMPT\"'"
      }]
    },
    {
      "matcher": { "event": "stop" },
      "hooks": [{
        "type": "command",
        "command": "subcog hook stop"
      }]
    }
  ]
}
```

## Using MCP Tools

Once configured, use these tools in Claude Code:

### Capture

```
Use subcog_capture to save: "Always validate input at API boundaries"
with namespace: security, tags: validation, api
```

### Search

```
Use subcog_recall to search for: authentication patterns
```

### Browse Resources

```
Read subcog://project/decisions to list all decisions
Read subcog://memory/{id} to get full content
```

## Prompt Templates

### Create a Template

```bash
# Create from command line
subcog prompt save code-review --content "Review {{file}} for {{issue_type}} issues"

# Create from file
subcog prompt save security-audit --file prompts/security-audit.md
```

### Use a Template

```bash
# Run with variables
subcog prompt run code-review --var file=src/main.rs --var issue_type=security
```

### List Templates

```bash
subcog prompt list
subcog prompt list --domain user
```

## Sync with Remote

```bash
# Push memories to remote
subcog sync push

# Fetch from remote
subcog sync fetch

# Full sync (fetch + push)
subcog sync
```

## Filter Syntax

| Filter | Description | Example |
|--------|-------------|---------|
| `ns:` | Namespace filter | `ns:decisions` |
| `tag:` | Tag filter (comma=OR) | `tag:rust,python` |
| `-tag:` | Exclude tag | `-tag:test` |
| `since:` | Time filter | `since:7d` |
| `source:` | Source file | `source:src/*` |
| `status:` | Memory status | `status:active` |

Combine filters with spaces (AND logic):

```bash
subcog recall --filter "ns:learnings tag:rust since:7d -tag:test" "error"
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_LOG_LEVEL` | Log level (trace, debug, info, warn, error) | `info` |
| `SUBCOG_CONFIG_PATH` | Custom config file path | Auto-detected |
| `SUBCOG_DOMAIN` | Override domain scope | Auto-detected |
| `SUBCOG_GIT_DIR` | Git directory path | `.git` |

## Next Steps

- [CLI Reference](cli/README.md) - Complete command documentation
- [MCP Integration](mcp/README.md) - MCP tools, resources, and prompts
- [Configuration](configuration/README.md) - Full configuration reference
- [Storage Architecture](storage/README.md) - How data is stored
- [URN Guide](URN-GUIDE.md) - Understanding URNs and URIs
