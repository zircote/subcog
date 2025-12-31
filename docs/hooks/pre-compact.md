# pre-compact Hook

Auto-captures important context before Claude Code compacts the conversation.

## Synopsis

```bash
subcog hook pre-compact [OPTIONS]
```

## Description

The pre-compact hook runs before Claude Code compacts the conversation to save tokens. It:
1. Analyzes the conversation for uncaptured decisions, patterns, and learnings
2. Auto-captures important context that would be lost
3. Returns a summary of what was captured

## Options

| Option | Description | Default |
|--------|-------------|---------|
| `--context` | Conversation context (via stdin) | None |
| `--dry-run` | Show what would be captured | `false` |

## Response Format

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreCompact",
    "additionalContext": "Auto-captured 3 memories before compaction:\n- decisions: Use PostgreSQL...\n- patterns: Error handling..."
  }
}
```

## Capture Detection

The hook scans for uncaptured content using signal patterns:

### Decision Signals
- "decided to", "going with", "chose"
- "we'll use", "let's go with"
- "the approach is", "we're implementing"

**Namespace:** `decisions`

### Pattern Signals
- "always", "never", "convention"
- "the pattern is", "standard approach"
- "we follow", "best practice"

**Namespace:** `patterns`

### Learning Signals
- "TIL", "learned that", "discovered"
- "turns out", "I found that"
- "the issue was", "root cause"

**Namespace:** `learnings`

### Blocker Signals
- "blocked by", "waiting on", "stuck"
- "can't proceed", "depends on"
- "issue:", "problem:"

**Namespace:** `blockers`

### Context Signals
- "because", "constraint", "requirement"
- "the reason is", "context:"
- "important:", "note:"

**Namespace:** `context`

## Auto-Capture Logic

1. **Extract candidates** - Find text matching signal patterns
2. **Deduplicate** - Check against existing memories
3. **Validate** - Ensure content is substantial (>20 chars)
4. **Capture** - Store with appropriate namespace and auto-tag

## Example Output

```bash
$ echo "We decided to use Redis for caching. TIL: Redis requires explicit TTL." | \
    subcog hook pre-compact 2>/dev/null | jq
```

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreCompact",
    "additionalContext": "Auto-captured 2 memories before compaction:\n\n1. **decisions**: Use Redis for caching\n   ID: abc123\n\n2. **learnings**: Redis requires explicit TTL\n   ID: def456"
  }
}
```

## Configuration

### hooks.json

```json
{
  "matcher": { "event": "pre_compact" },
  "hooks": [{
    "type": "command",
    "command": "subcog hook pre-compact"
  }]
}
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_AUTO_CAPTURE_ENABLED` | Enable auto-capture | `true` |
| `SUBCOG_AUTO_CAPTURE_DRY_RUN` | Dry run mode | `false` |
| `SUBCOG_AUTO_CAPTURE_MIN_LENGTH` | Minimum content length | `20` |

## Dry Run Mode

Preview what would be captured:

```bash
$ echo "We decided to use PostgreSQL." | \
    subcog hook pre-compact --dry-run 2>/dev/null | jq
```

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreCompact",
    "additionalContext": "Would capture 1 memory:\n\n1. **decisions**: Use PostgreSQL\n   (dry run - not captured)"
  }
}
```

## Deduplication

The hook checks for existing similar memories:
1. **Exact match** - Skips if identical content exists
2. **Semantic similarity** - Skips if >90% similar memory exists
3. **Recent capture** - Skips if captured in last 5 minutes

## Auto-Tagging

Captured memories are auto-tagged based on:
- File paths mentioned in context
- Technical terms detected
- Project-specific keywords

## Performance

- Target: <100ms
- Typical: ~80ms
- With LLM analysis: ~300ms

## Best Practices

1. **Review auto-captures** - Check status to see what was captured
2. **Adjust sensitivity** - Set `MIN_LENGTH` to control capture threshold
3. **Use dry run** - Test before enabling auto-capture
4. **Clean up** - Use consolidate to merge similar auto-captures

## See Also

- [stop](stop.md) - Session end hook
- [user-prompt-submit](user-prompt-submit.md) - Capture signal detection
- [subcog consolidate](../cli/consolidate.md) - Merge similar memories
