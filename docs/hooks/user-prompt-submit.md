# user-prompt-submit Hook

Detects search intent and surfaces relevant memories before the prompt is processed.

## Synopsis

```bash
subcog hook user-prompt-submit [OPTIONS] <PROMPT>
```

## Description

The user-prompt-submit hook runs before each user prompt is sent to Claude. It:
1. Analyzes the prompt for search intent
2. Detects capture signals (decisions, patterns, learnings)
3. Searches for relevant memories
4. Returns context to be injected with the prompt

## Arguments

| Argument | Description |
|----------|-------------|
| `<PROMPT>` | The user's prompt text |

## Options

| Option | Description | Default |
|--------|-------------|---------|
| `--max-memories` | Maximum memories to inject | `15` |
| `--skip-detection` | Skip intent detection | `false` |

## Response Format

```json
{
  "hookSpecificOutput": {
    "hookEventName": "UserPromptSubmit",
    "additionalContext": "## Relevant Memories\n\n..."
  }
}
```

## Search Intent Detection

The hook detects six types of search intent:

### HowTo
**Patterns:** "how do I", "how to", "implement", "create", "build"

**Example:** "How do I implement authentication?"

**Namespace weights:**
- patterns: 1.5
- learnings: 1.3
- apis: 1.2

### Location
**Patterns:** "where is", "find", "locate", "which file"

**Example:** "Where is the database configuration?"

**Namespace weights:**
- apis: 1.5
- config: 1.5
- context: 1.2

### Explanation
**Patterns:** "what is", "explain", "describe", "what does"

**Example:** "What is the ServiceContainer?"

**Namespace weights:**
- decisions: 1.5
- context: 1.4
- patterns: 1.2

### Comparison
**Patterns:** "difference between", "vs", "compare", "versus"

**Example:** "PostgreSQL vs SQLite for this use case?"

**Namespace weights:**
- decisions: 1.5
- patterns: 1.3

### Troubleshoot
**Patterns:** "error", "fix", "not working", "debug", "issue", "problem"

**Example:** "Why is this test failing?"

**Namespace weights:**
- blockers: 1.5
- learnings: 1.4
- testing: 1.2

### General
**Patterns:** "search", "show me", "list", "find memories"

**Example:** "Search for recent patterns"

**Namespace weights:** Balanced across all namespaces

## Detection Modes

### Keyword Detection (Default)
- Fast pattern matching (<10ms)
- Falls back when LLM unavailable

### LLM Detection (Optional)
- Enhanced accuracy with context understanding
- 200ms timeout
- Requires `SUBCOG_SEARCH_INTENT_USE_LLM=true`

### Hybrid Mode
- Combines keyword and LLM detection
- Uses LLM to refine keyword-based detection
- Best accuracy with graceful fallback

## Memory Injection

Based on detection confidence:

| Confidence | Memories | Detail Level |
|------------|----------|--------------|
| ≥ 0.8 (high) | 15 | medium |
| ≥ 0.5 (medium) | 10 | light |
| < 0.5 (low) | 5 | light |

## Capture Signal Detection

The hook also detects capture signals in prompts:

| Signal | Patterns | Namespace |
|--------|----------|-----------|
| Decision | "decided", "going with", "chose" | decisions |
| Pattern | "always", "never", "convention" | patterns |
| Learning | "TIL", "learned", "discovered" | learnings |
| Blocker | "blocked", "waiting", "stuck" | blockers |

When detected, suggests capture without auto-capturing.

## Example Output

```bash
$ subcog hook user-prompt-submit "How do I implement OAuth2?" 2>/dev/null | jq
```

```json
{
  "hookSpecificOutput": {
    "hookEventName": "UserPromptSubmit",
    "additionalContext": "## Relevant Memories\n\n**Intent detected:** HowTo (confidence: 0.85)\n\n### patterns\n- Use PKCE for OAuth2 in SPAs (id: abc123)\n- Always validate redirect URIs server-side (id: def456)\n\n### learnings\n- TIL: refresh tokens need secure storage (id: ghi789)\n\n### apis\n- OAuth2 endpoint: POST /auth/token (id: jkl012)"
  }
}
```

## Configuration

### hooks.json

```json
{
  "matcher": { "event": "user_prompt_submit" },
  "hooks": [{
    "type": "command",
    "command": "sh -c 'subcog hook user-prompt-submit \"$PROMPT\"'"
  }]
}
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_SEARCH_INTENT_ENABLED` | Enable detection | `true` |
| `SUBCOG_SEARCH_INTENT_USE_LLM` | Use LLM | `true` |
| `SUBCOG_SEARCH_INTENT_LLM_TIMEOUT_MS` | LLM timeout | `200` |
| `SUBCOG_SEARCH_INTENT_MIN_CONFIDENCE` | Min confidence | `0.5` |

## Performance

- Target: <50ms (keyword only)
- With LLM: <200ms (with timeout)
- Typical: ~30ms

## Graceful Degradation

| Failure | Fallback |
|---------|----------|
| LLM timeout | Keyword detection |
| Embeddings fail | Text search |
| Index unavailable | Return empty context |

## See Also

- [Search Intent](search-intent.md) - Detailed intent detection
- [session-start](session-start.md) - Session initialization
- [post-tool-use](post-tool-use.md) - Post-tool surfacing
