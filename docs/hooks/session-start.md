# session-start Hook

Injects context and guidance when a Claude Code session begins.

## Synopsis

```bash
subcog hook session-start [OPTIONS]
```

## Description

The session-start hook runs at the beginning of each Claude Code session. It:
1. Loads relevant memories based on the working directory
2. Generates guidance based on the guidance level
3. Returns context to be injected into the session

## Options

| Option | Description | Default |
|--------|-------------|---------|
| `--guidance` | Guidance level | `standard` |
| `--max-tokens` | Maximum context tokens | `1000` |

## Guidance Levels

### minimal

Returns only tool availability information.

```markdown
# Subcog Memory Context

## Available Tools
- subcog_capture
- subcog_recall
- subcog_status
- subcog_namespaces
```

### standard (Default)

Includes tools, usage hints, and relevant memories.

```markdown
# Subcog Memory Context

Session: abc123-...
Working Directory: /path/to/project

## Subcog Memory Protocol

You have access to subcog, a persistent memory system.

### Available Tools
| Tool | Description |
|------|-------------|
| subcog_capture | Capture a memory |
| subcog_recall | Search memories |
| subcog_status | System status |
| subcog_namespaces | List namespaces |

### Capture Memories
When the user makes a decision, discovers a pattern, or learns something:
- Use subcog_capture to record it
- Choose the appropriate namespace

### Recall Memories
Before making recommendations:
- Use subcog_recall to search for relevant prior context

### Recent Memories
[List of 5-10 recent relevant memories]
```

### full

Complete tutorial plus all available context.

```markdown
# Subcog Memory Context

[Everything from standard level]

### Complete Tutorial
[Detailed usage guide]

### Project Context
[All project-scoped memories]

### User Context
[Recent user-scoped learnings]
```

## Response Format

```json
{
  "hookSpecificOutput": {
    "hookEventName": "SessionStart",
    "additionalContext": "# Subcog Memory Context\n\n..."
  }
}
```

## Memory Selection

The hook selects relevant memories based on:

1. **Working directory** - Project-scoped memories
2. **Git branch** - Branch-specific context
3. **Recent activity** - Most recent 10 memories
4. **File presence** - Memories referencing existing files

## Adaptive Token Budget

Token allocation adapts to available budget:

| Budget | Content |
|--------|---------|
| <500 | Tools only |
| 500-1000 | Tools + hints |
| 1000-2000 | Standard + memories |
| >2000 | Full context |

## Configuration

### hooks.json

```json
{
  "matcher": { "event": "session_start" },
  "hooks": [{
    "type": "command",
    "command": "subcog hook session-start --guidance standard"
  }]
}
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_SESSION_GUIDANCE` | Default guidance level | `standard` |
| `SUBCOG_SESSION_MAX_TOKENS` | Maximum tokens | `1000` |

## Example Output

```bash
$ subcog hook session-start 2>/dev/null | jq -r '.hookSpecificOutput.additionalContext' | head -30
```

```markdown
# Subcog Memory Context

Session: fea5608c-85f8-4a89-b155-a830dfcda507
Working Directory: /Users/user/project

## Subcog Memory Protocol

You have access to subcog, a persistent memory system. MCP tools are available.

### Available Tools
| Short Name | Full MCP Tool Name |
|------------|-------------------|
| subcog_capture | mcp__plugin_subcog_subcog__subcog_capture |
| subcog_recall | mcp__plugin_subcog_subcog__subcog_recall |
| subcog_status | mcp__plugin_subcog_subcog__subcog_status |

### Proactive Behavior
- **Decisions**: When the user says "we'll use X", capture it
- **Patterns**: When identifying recurring patterns, capture them
- **Learnings**: When discovering gotchas or insights, capture them
```

## Performance

- Target: <100ms
- Typical: ~50ms
- With LLM analysis: ~150ms

## Error Handling

If the hook fails, it returns minimal context:

```json
{
  "hookSpecificOutput": {
    "hookEventName": "SessionStart",
    "additionalContext": "Subcog available. Use subcog_recall to search memories."
  }
}
```

## See Also

- [user-prompt-submit](user-prompt-submit.md) - Next hook in flow
- [Configuration](../configuration/README.md) - Environment configuration
