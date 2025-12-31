# subcog recall

Search and retrieve memories.

## Synopsis

```
subcog recall [OPTIONS] <QUERY>
subcog recall [OPTIONS] --filter <FILTER>
```

## Description

The `recall` command searches the memory index using hybrid search (combining vector similarity and BM25 text matching). Results are ranked using Reciprocal Rank Fusion (RRF).

## Arguments

| Argument | Description |
|----------|-------------|
| `<QUERY>` | Search query (natural language or keywords) |

## Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--filter` | `-f` | Filter expression | None |
| `--limit` | `-l` | Maximum results | `10` |
| `--offset` | `-o` | Skip first N results | `0` |
| `--mode` | `-m` | Search mode (hybrid, vector, text) | `hybrid` |
| `--detail` | `-d` | Detail level (light, medium, everything) | `medium` |
| `--format` | | Output format (table, json, markdown) | `table` |
| `--namespace` | `-n` | Filter by namespace | None |

## Search Modes

| Mode | Algorithm | Best For |
|------|-----------|----------|
| `hybrid` | RRF(vector + BM25) | General search (default) |
| `vector` | Semantic similarity | Concept-based search |
| `text` | BM25 keyword matching | Exact term matching |

## Detail Levels

| Level | Content Returned |
|-------|------------------|
| `light` | Frontmatter only (id, namespace, tags) |
| `medium` | Truncated content (~200 chars) |
| `everything` | Full content |

## Filter Syntax

### Namespace Filter

```bash
subcog recall -f "ns:decisions" "storage"
subcog recall -f "ns:patterns" "error"
```

### Tag Filter

```bash
# Single tag
subcog recall -f "tag:rust" "memory"

# OR logic (comma-separated)
subcog recall -f "tag:rust,python" "error"

# AND logic (space-separated)
subcog recall -f "tag:rust tag:async" "pattern"

# Exclude tag
subcog recall -f "-tag:test" "security"
```

### Time Filter

```bash
subcog recall -f "since:1d" "recent"
subcog recall -f "since:7d" "this week"
subcog recall -f "since:30d" "this month"
```

### Source Filter

```bash
subcog recall -f "source:src/*" "implementation"
subcog recall -f "source:*.rs" "rust code"
```

### Combined Filters

```bash
subcog recall -f "ns:learnings tag:rust since:7d -tag:test" "error handling"
```

## Examples

### Simple Search

```bash
subcog recall "database storage decision"
```

### Search with Namespace

```bash
subcog recall -n decisions "PostgreSQL"
```

### Semantic Search

```bash
subcog recall -m vector "how to handle authentication"
```

### Exact Match Search

```bash
subcog recall -m text "Result<T, E>"
```

### Get Full Content

```bash
subcog recall -d everything "specific decision"
```

### JSON Output

```bash
subcog recall --format json "patterns" | jq '.[] | .content'
```

### Pagination

```bash
# First page
subcog recall -l 10 "topic"

# Second page
subcog recall -l 10 -o 10 "topic"
```

## Output Formats

### Table (default)

```
ID          NS          TAGS            CONTENT
dc58d23a    decisions   [database]      Use PostgreSQL for primary...
1314b968    learnings   [rust,async]    TIL: async closures need...
```

### JSON

```json
[
  {
    "id": "dc58d23a35876f5a59426e81aaa81d796efa7fc1",
    "namespace": "decisions",
    "tags": ["database"],
    "content": "Use PostgreSQL for primary storage...",
    "score": 0.85
  }
]
```

### Markdown

```markdown
## dc58d23a (decisions)

**Tags**: database

Use PostgreSQL for primary storage...

---
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Search successful (may have 0 results) |
| 1 | Search failed |
| 2 | Invalid arguments |
| 4 | Index error |

## Performance

- Cold search: <50ms
- Warm search: <20ms
- Vector embedding: ~30ms (cached)

## See Also

- [capture](capture.md) - Capture new memories
- [MCP subcog_recall](../mcp/tools.md#subcog_recall) - MCP equivalent
- [Query Syntax](../QUERY_SYNTAX.md) - Full filter syntax reference
