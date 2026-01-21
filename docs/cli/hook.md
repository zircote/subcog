# subcog hook

Handle Claude Code hook events.

## Synopsis

```
subcog hook <EVENT> [OPTIONS]
```

## Description

The `hook` command processes Claude Code hook events. It is typically called automatically by Claude Code based on `hooks/hooks.json` configuration.

## Events

| Event | Description |
|-------|-------------|
| `session-start` | Called when a Claude Code session begins |
| `user-prompt-submit` | Called before user prompt is sent |
| `post-tool-use` | Called after a tool is executed |
| `pre-compact` | Called before context compaction |
| `stop` | Called when session ends |

## Event: session-start

Injects relevant context into the session.

### Synopsis

```
subcog hook session-start [OPTIONS]
```

### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--guidance` | Guidance level (minimal, standard, full) | `standard` |
| `--max-tokens` | Maximum context tokens | `1000` |

### Output

```json
{
  "hookSpecificOutput": {
    "hookEventName": "SessionStart",
    "additionalContext": "# Subcog Memory Context\n\n..."
  }
}
```

### Guidance Levels

| Level | Content |
|-------|---------|
| `minimal` | Available tools only |
| `standard` | Tools + usage hints + recent memories |
| `full` | Complete tutorial + all context |

---

## Event: user-prompt-submit

Detects search intent and injects relevant memories.

### Synopsis

```
subcog hook user-prompt-submit [OPTIONS] <PROMPT>
```

### Arguments

| Argument | Description |
|----------|-------------|
| `<PROMPT>` | User's prompt text |

### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--max-memories` | Maximum memories to inject | `15` |

### Output

```json
{
  "hookSpecificOutput": {
    "hookEventName": "UserPromptSubmit",
    "additionalContext": "## Relevant Memories\n\n..."
  }
}
```

### Search Intent Types

| Type | Patterns | Example |
|------|----------|---------|
| HowTo | "how do I", "implement" | "How do I add auth?" |
| Location | "where is", "find" | "Where is the config?" |
| Explanation | "what is", "explain" | "What is RRF?" |
| Comparison | "vs", "difference" | "PostgreSQL vs SQLite?" |
| Troubleshoot | "error", "not working" | "Why is this failing?" |
| General | "search", "show me" | "Search for patterns" |

---

## Event: post-tool-use

Surfaces related memories after tool execution.

### Synopsis

```
subcog hook post-tool-use [OPTIONS]
```

### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--tool` | Tool that was used | None |
| `--result` | Tool result (stdin) | None |

### Output

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PostToolUse",
    "additionalContext": "## Related Memories\n\n..."
  }
}
```

---

## Event: pre-compact

Auto-captures important context before compaction.

### Synopsis

```
subcog hook pre-compact [OPTIONS]
```

### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--context` | Context to analyze (stdin) | None |

### Output

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreCompact",
    "additionalContext": "Auto-captured 3 memories before compaction"
  }
}
```

### Auto-Capture Detection

Detects and captures:
- Decisions ("decided to", "going with")
- Patterns ("always", "never")
- Learnings ("TIL", "learned that")
- Blockers ("blocked by", "waiting on")

---

## Event: stop

Analyzes session and syncs memories.

### Synopsis

```
subcog hook stop [OPTIONS]
```

### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--sync` | Sync after analysis | `true` |

### Output

```json
{
  "hookSpecificOutput": {
    "hookEventName": "Stop",
    "additionalContext": "Session summary: 5 captures, 12 recalls"
  }
}
```

---

## hooks.json Configuration

Configure hooks in your project:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "npx --prefer-offline @zircote/subcog hook session-start 2>/dev/null || npx -y @zircote/subcog hook session-start"
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "npx --prefer-offline @zircote/subcog hook user-prompt-submit 2>/dev/null || npx -y @zircote/subcog hook user-prompt-submit"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Read|Write|Edit|Bash|Grep|Glob|LSP",
        "hooks": [
          {
            "type": "command",
            "command": "npx --prefer-offline @zircote/subcog hook post-tool-use 2>/dev/null || npx -y @zircote/subcog hook post-tool-use"
          }
        ]
      }
    ],
    "PreCompact": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "npx --prefer-offline @zircote/subcog hook pre-compact 2>/dev/null || npx -y @zircote/subcog hook pre-compact"
          }
        ]
      }
    ],
    "Stop": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "npx --prefer-offline @zircote/subcog hook stop 2>/dev/null || npx -y @zircote/subcog hook stop"
          }
        ]
      }
    ]
  }
}
```

## Output Format

All hooks return JSON in the Claude Code hook response format:

```json
{
  "hookSpecificOutput": {
    "hookEventName": "<EventName>",
    "additionalContext": "<context to inject>"
  }
}
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Hook executed successfully |
| 1 | Hook failed |
| 2 | Invalid arguments |

## See Also

- [Claude Code Hooks](../hooks/README.md) - Full hooks documentation
- [Search Intent](../hooks/search-intent.md) - Search intent detection
- [serve](serve.md) - MCP server
