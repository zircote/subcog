# Claude Code Hooks

Subcog integrates with Claude Code through hooks that inject context, surface memories, and capture knowledge automatically.

## Overview

| Hook | Event | Purpose |
|------|-------|---------|
| [session-start](session-start.md) | Session begins | Inject context and guidance |
| [user-prompt-submit](user-prompt-submit.md) | Before prompt sent | Detect intent, surface memories |
| [post-tool-use](post-tool-use.md) | After tool execution | Surface related memories |
| [pre-compact](pre-compact.md) | Before compaction | Auto-capture important context |
| [stop](stop.md) | Session ends | Analyze session, sync memories |

## Hook Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Claude Code                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │  Session Start  │  │  User Prompt    │  │    Stop     │ │
│  └────────┬────────┘  └────────┬────────┘  └──────┬──────┘ │
└───────────┼────────────────────┼─────────────────┼─────────┘
            │                    │                  │
            ▼                    ▼                  ▼
┌───────────────────────────────────────────────────────────┐
│                      Subcog Hooks                          │
│  ┌─────────────────┐  ┌─────────────────┐  ┌───────────┐ │
│  │ Context Inject  │  │  Intent Detect  │  │   Sync    │ │
│  │ Memory Surface  │  │  Memory Surface │  │  Analyze  │ │
│  └─────────────────┘  └─────────────────┘  └───────────┘ │
└───────────────────────────────────────────────────────────┘
```

## Configuration

### hooks.json

Create `hooks/hooks.json` in your project root:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "subcog hook session-start"
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
            "command": "subcog hook user-prompt-submit"
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
            "command": "subcog hook post-tool-use"
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
            "command": "subcog hook pre-compact"
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
            "command": "subcog hook stop"
          }
        ]
      }
    ]
  }
}
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_HOOK_ENABLED` | Enable/disable hooks | `true` |
| `SUBCOG_SEARCH_INTENT_ENABLED` | Enable intent detection | `true` |
| `SUBCOG_SEARCH_INTENT_USE_LLM` | Use LLM for detection | `true` |
| `SUBCOG_SEARCH_INTENT_LLM_TIMEOUT_MS` | LLM timeout | `200` |
| `SUBCOG_SEARCH_INTENT_MIN_CONFIDENCE` | Minimum confidence | `0.5` |

## Response Format

All hooks return JSON in Claude Code's expected format:

```json
{
  "hookSpecificOutput": {
    "hookEventName": "<EventName>",
    "additionalContext": "<markdown content to inject>"
  }
}
```

## Search Intent Detection

The [user-prompt-submit](user-prompt-submit.md) hook includes intelligent search intent detection.

### Intent Types

| Type | Patterns | Memory Focus |
|------|----------|--------------|
| HowTo | "how do I", "implement" | patterns, learnings |
| Location | "where is", "find" | apis, config |
| Explanation | "what is", "explain" | decisions, context |
| Comparison | "vs", "difference" | decisions, patterns |
| Troubleshoot | "error", "not working" | blockers, learnings |
| General | "search", "show me" | balanced |

See [Search Intent](search-intent.md) for details.

## Adaptive Memory Injection

Memory injection adapts based on detection confidence:

| Confidence | Memories | Behavior |
|------------|----------|----------|
| ≥ 0.8 | 15 | Full context injection |
| ≥ 0.5 | 10 | Standard injection |
| < 0.5 | 5 | Minimal injection |

## Performance

| Hook | Target | Actual |
|------|--------|--------|
| session-start | <100ms | ~50ms |
| user-prompt-submit | <50ms | ~30ms |
| post-tool-use | <50ms | ~20ms |
| pre-compact | <100ms | ~80ms |
| stop | <200ms | ~150ms |

## Graceful Degradation

Hooks degrade gracefully when components fail:

| Component | Fallback |
|-----------|----------|
| LLM unavailable | Keyword-only detection |
| Embeddings down | Text search (BM25) |
| Index down | Skip memory injection |
| Git unavailable | Skip sync |

## Debugging

Enable debug logging:

```bash
SUBCOG_LOG_LEVEL=debug subcog hook session-start
```

Check hook output:

```bash
subcog hook user-prompt-submit "how do I implement auth" 2>&1 | jq
```

## See Also

- [session-start](session-start.md) - Session initialization
- [user-prompt-submit](user-prompt-submit.md) - Prompt processing
- [post-tool-use](post-tool-use.md) - Post-tool surfacing
- [pre-compact](pre-compact.md) - Pre-compaction capture
- [stop](stop.md) - Session finalization
- [Search Intent](search-intent.md) - Intent detection system
