# subcog consolidate

Merge and deduplicate similar memories.

## Synopsis

```
subcog consolidate [OPTIONS] --namespace <NS>
```

## Description

The `consolidate` command uses LLM-powered analysis to identify similar or redundant memories and merge them into consolidated entries. This helps maintain a clean, non-redundant memory store.

## Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--namespace` | `-n` | Namespace to consolidate (required) | None |
| `--strategy` | `-s` | Consolidation strategy | `merge` |
| `--query` | `-q` | Filter memories before consolidation | None |
| `--dry-run` | | Show what would be consolidated | `false` |
| `--threshold` | `-t` | Similarity threshold (0.0-1.0) | `0.8` |

## Strategies

| Strategy | Description |
|----------|-------------|
| `merge` | Combine similar memories into one |
| `summarize` | Create summary from related memories |
| `dedupe` | Remove exact duplicates only |

## Requirements

This command requires LLM features to be enabled:
```bash
export SUBCOG_LLM_PROVIDER=anthropic
export ANTHROPIC_API_KEY=sk-...
```

Or in configuration:
```yaml
llm:
  provider: anthropic
  model: claude-sonnet-4-20250514
```

## Examples

### Basic Consolidation

```bash
subcog consolidate -n decisions
```

Output:
```
Analyzing decisions namespace...
Found 15 memories

Consolidation candidates:
  Group 1 (similarity: 0.92):
    - dc58d23a: "Use PostgreSQL for storage"
    - 1314b968: "Decided on PostgreSQL with JSONB"
    - a1b2c3d4: "PostgreSQL chosen for persistence"

  Group 2 (similarity: 0.87):
    - e5f6a7b8: "API versioning with /v1/ prefix"
    - c9d0e1f2: "Version APIs using URL prefix"

Proceed with consolidation? [y/N]
```

### Dry Run

```bash
subcog consolidate -n learnings --dry-run
```

### Filter Before Consolidation

```bash
subcog consolidate -n patterns -q "tag:rust"
```

### Summarize Strategy

```bash
subcog consolidate -n context -s summarize
```

Creates a new summary memory linking to the original memories.

### Deduplicate Only

```bash
subcog consolidate -n learnings -s dedupe
```

Only removes exact duplicates, no LLM analysis.

### Custom Threshold

```bash
subcog consolidate -n decisions -t 0.9
```

Higher threshold = stricter matching.

## Output

### Merge Result

```json
{
  "groups_found": 3,
  "memories_merged": 8,
  "new_memories": 3,
  "archived_memories": 5,
  "dry_run": false
}
```

### What Happens to Merged Memories

1. New consolidated memory is created
2. Original memories are marked with `status: archived`
3. Original memories retain a reference to the consolidated memory

## LLM Providers

| Provider | Environment Variable |
|----------|---------------------|
| Anthropic | `ANTHROPIC_API_KEY` |
| OpenAI | `OPENAI_API_KEY` |
| Ollama | `OLLAMA_HOST` (local) |
| LM Studio | `LMSTUDIO_HOST` (local) |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Consolidation successful |
| 1 | Consolidation failed |
| 2 | Invalid arguments |
| 7 | LLM provider error |

## Performance

| Memories | Typical Time |
|----------|--------------|
| 10 | ~5s |
| 50 | ~20s |
| 100 | ~45s |

Time depends on LLM provider latency.

## See Also

- [capture](./capture.md) - Capture memories
- [recall](./recall.md) - Search memories
- [MCP subcog_consolidate](../mcp/tools.md#subcog_consolidate) - MCP equivalent
