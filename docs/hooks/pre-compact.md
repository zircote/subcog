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

The hook integrates with `DeduplicationService` to prevent duplicate captures. Three checks are performed in order (short-circuit evaluation):

### 1. Exact Match

Computes SHA256 hash of normalized content and searches for existing memory with matching `hash:sha256:<prefix>` tag.

- **Hash format**: First 16 characters of SHA256 hex digest
- **Normalization**: Lowercase, collapse whitespace, trim
- **Tag search**: Uses `RecallService` tag filter

### 2. Semantic Similarity

Generates embedding with FastEmbed and searches vector index for similar memories.

- **Model**: all-MiniLM-L6-v2 (384 dimensions)
- **Default threshold**: 90% cosine similarity
- **Namespace thresholds**: Configurable per namespace:
  - `decisions`: 92%
  - `patterns`: 90%
  - `learnings`: 88%
  - `blockers`: 90%

### 3. Recent Capture

Checks in-memory LRU cache for recently captured content.

- **Cache size**: 1,000 entries
- **TTL**: 5 minutes
- **Key**: Content hash + namespace

### Graceful Degradation

If any checker fails:
- Error is logged with `tracing::warn!`
- Check continues to next tier
- Capture proceeds if all checks pass or fail

### Configuration

| Environment Variable | Description | Default |
|---------------------|-------------|---------|
| `SUBCOG_DEDUP_ENABLED` | Enable deduplication | `true` |
| `SUBCOG_DEDUP_DEFAULT_THRESHOLD` | Default similarity threshold | `0.90` |
| `SUBCOG_DEDUP_DECISIONS_THRESHOLD` | Decisions namespace threshold | `0.92` |
| `SUBCOG_DEDUP_PATTERNS_THRESHOLD` | Patterns namespace threshold | `0.90` |
| `SUBCOG_DEDUP_LEARNINGS_THRESHOLD` | Learnings namespace threshold | `0.88` |
| `SUBCOG_DEDUP_RECENT_TTL_SECONDS` | Recent capture TTL | `300` |
| `SUBCOG_DEDUP_RECENT_CACHE_SIZE` | Recent cache size | `1000` |
| `SUBCOG_DEDUP_MIN_SEMANTIC_LENGTH` | Min length for semantic check | `50` |

### Skipped Duplicates

When duplicates are found, they are reported in the hook response:

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreCompact",
    "additionalContext": "**Subcog Pre-Compact Auto-Capture**\n\nCaptured 1 memory...\n\nSkipped 2 duplicates:\n- `decisions`: subcog://global/decisions/abc123 (exact_match)\n- `learnings`: subcog://global/learnings/def456 (semantic_similar, 95% similar)"
  }
}
```

### Metrics

| Metric | Description |
|--------|-------------|
| `deduplication_duplicates_found_total` | Total duplicates detected |
| `deduplication_not_duplicates_total` | Total unique captures |
| `deduplication_check_duration_ms` | Check latency histogram |
| `hook_deduplication_skipped_total` | Skipped by hook (labels: namespace, reason) |

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
