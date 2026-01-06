# MCP Tools

Subcog provides 14 MCP tools for memory operations, garbage collection, and prompt management.

## Claude Code Invocation

All subcog MCP tools are accessible using the `subcog:` prefix in Claude Code:

| MCP Tool | Claude Code Syntax |
|----------|-------------------|
| `subcog_capture` | `subcog:capture` |
| `subcog_recall` | `subcog:recall` |
| `subcog_status` | `subcog:status` |
| `subcog_sync` | `subcog:sync` |
| `subcog_gc` | `subcog:gc` |
| `subcog_namespaces` | `subcog:namespaces` |
| `subcog_reindex` | `subcog:reindex` |
| `subcog_enrich` | `subcog:enrich` |
| `subcog_consolidate` | `subcog:consolidate` |
| `prompt_save` | `subcog:prompt:save` |
| `prompt_list` | `subcog:prompt:list` |
| `prompt_get` | `subcog:prompt:get` |
| `prompt_run` | `subcog:prompt:run` |
| `prompt_delete` | `subcog:prompt:delete` |

**Example - Claude Code:**
```
subcog:recall "database decision" --filter "ns:decisions since:7d"
subcog:prompt:run code-review --var file=src/main.rs --var issue_type=security
```

---

## Memory Tools

### subcog_capture

Capture a memory to persistent storage.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `content` | string | Yes | Memory content |
| `namespace` | string | Yes | Memory namespace |
| `tags` | array[string] | No | Tags for categorization |
| `source` | string | No | Source file reference |

**Namespaces:** `decisions`, `patterns`, `learnings`, `context`, `tech-debt`, `blockers`, `progress`, `apis`, `config`, `security`, `testing`

**Example:**

```json
{
  "name": "subcog_capture",
  "arguments": {
    "namespace": "decisions",
    "content": "Use PostgreSQL for primary storage because of JSONB support",
    "tags": ["database", "architecture"],
    "source": "ARCHITECTURE.md"
  }
}
```

**Response:**

```json
{
  "id": "dc58d23a35876f5a59426e81aaa81d796efa7fc1",
  "urn": "subcog://project/decisions/dc58d23a35876f5a59426e81aaa81d796efa7fc1"
}
```

---

### subcog_recall

Search for relevant memories using semantic and text search.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Search query |
| `namespace` | string | No | Filter by namespace |
| `filter` | string | No | Filter expression (see [Query Syntax](../QUERY_SYNTAX.md)) |
| `mode` | string | No | Search mode: `hybrid`, `vector`, `text` (default: `hybrid`) |
| `detail` | string | No | Detail level: `light`, `medium`, `everything` (default: `medium`) |
| `limit` | integer | No | Maximum results (default: 10, max: 50) |

**Example:**

```json
{
  "name": "subcog_recall",
  "arguments": {
    "query": "database storage decision",
    "filter": "ns:decisions since:7d",
    "mode": "hybrid",
    "detail": "medium",
    "limit": 5
  }
}
```

**Response:**

```json
{
  "results": [
    {
      "id": "dc58d23a35876f5a59426e81aaa81d796efa7fc1",
      "namespace": "decisions",
      "tags": ["database", "architecture"],
      "content": "Use PostgreSQL for primary storage...",
      "score": 0.85,
      "uri": "subcog://memory/dc58d23a..."
    }
  ],
  "total": 1
}
```

**Search Modes:**

| Mode | Algorithm | Best For |
|------|-----------|----------|
| `hybrid` | RRF(vector + BM25) | General search |
| `vector` | Semantic similarity | Concept-based |
| `text` | BM25 keyword | Exact terms |

**Detail Levels:**

| Level | Returns |
|-------|---------|
| `light` | id, namespace, tags, uri only |
| `medium` | Above + truncated content (~200 chars) |
| `everything` | Full content |

---

### subcog_status

Get memory system status and statistics.

**Parameters:** None

**Example:**

```json
{
  "name": "subcog_status",
  "arguments": {}
}
```

**Response:**

```json
{
  "repository": {
    "path": "/path/to/project",
    "project_id": "github.com/zircote/subcog",
    "branch": "main"
  },
  "storage": {
    "persistence": "sqlite",
    "index": "sqlite",
    "vector": "usearch"
  },
  "statistics": {
    "total_count": 42,
    "namespace_counts": {
      "decisions": 12,
      "patterns": 8,
      "learnings": 15
    },
    "recent_topics": ["database", "api", "auth"],
    "top_tags": [
      {"tag": "rust", "count": 15},
      {"tag": "database", "count": 8}
    ]
  }
}
```

---

### subcog_gc

Garbage collect memories from deleted branches.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `dry_run` | boolean | No | Preview without changes (default: false) |
| `branch` | string | No | Target specific branch |
| `purge` | boolean | No | Permanently delete tombstoned memories |
| `older_than` | string | No | Only purge tombstones older than duration (e.g., "30d") |

**Example:**

```json
{
  "name": "subcog_gc",
  "arguments": {
    "dry_run": true
  }
}
```

**Response:**

```json
{
  "stale_branches": ["feature/old-auth", "bugfix/issue-42"],
  "memories_affected": 17,
  "dry_run": true,
  "status": "preview"
}
```

---

### subcog_namespaces

List available memory namespaces.

**Parameters:** None

**Example:**

```json
{
  "name": "subcog_namespaces",
  "arguments": {}
}
```

**Response:**

```json
{
  "namespaces": [
    {
      "name": "decisions",
      "description": "Architectural and design decisions",
      "signal_words": ["decided", "chose", "going with"]
    },
    {
      "name": "patterns",
      "description": "Discovered patterns and conventions",
      "signal_words": ["always", "never", "convention"]
    }
  ]
}
```

---

### subcog_consolidate

Consolidate related memories using LLM to merge and summarize.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `namespace` | string | Yes | Namespace to consolidate |
| `strategy` | string | No | Strategy: `merge`, `summarize`, `dedupe` (default: `merge`) |
| `query` | string | No | Filter memories before consolidation |
| `dry_run` | boolean | No | Preview without changes (default: false) |

**Example:**

```json
{
  "name": "subcog_consolidate",
  "arguments": {
    "namespace": "decisions",
    "strategy": "merge",
    "dry_run": true
  }
}
```

**Response:**

```json
{
  "groups_found": 3,
  "candidate_merges": [
    {
      "memories": ["dc58d23a", "1314b968"],
      "similarity": 0.92,
      "proposed_content": "Merged: Use PostgreSQL..."
    }
  ],
  "dry_run": true
}
```

---

### subcog_enrich

Enrich a memory with better structure, tags, and context using LLM.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `memory_id` | string | Yes | ID of memory to enrich |
| `enrich_tags` | boolean | No | Generate/improve tags (default: true) |
| `enrich_structure` | boolean | No | Restructure content (default: true) |
| `add_context` | boolean | No | Add inferred context (default: false) |

**Example:**

```json
{
  "name": "subcog_enrich",
  "arguments": {
    "memory_id": "dc58d23a35876f5a59426e81aaa81d796efa7fc1",
    "enrich_tags": true,
    "enrich_structure": true,
    "add_context": true
  }
}
```

**Response:**

```json
{
  "memory_id": "dc58d23a...",
  "changes": {
    "tags_added": ["postgresql", "jsonb", "persistence"],
    "structure_improved": true,
    "context_added": "This decision impacts the storage layer..."
  }
}
```

---

### subcog_sync

Sync memories with git remote.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `direction` | string | No | `push`, `fetch`, or `full` (default: `full`) |

**Example:**

```json
{
  "name": "subcog_sync",
  "arguments": {
    "direction": "full"
  }
}
```

**Response:**

```json
{
  "direction": "full",
  "fetched": 3,
  "pushed": 5,
  "status": "success"
}
```

---

### subcog_reindex

Rebuild the search index from the persistence layer.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `repo_path` | string | No | Repository path (default: current) |

**Example:**

```json
{
  "name": "subcog_reindex",
  "arguments": {}
}
```

**Response:**

```json
{
  "indexed": 42,
  "duration_ms": 1250,
  "status": "success"
}
```

---

## Prompt Tools

### prompt_save

Save a user-defined prompt template.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Unique prompt name (kebab-case) |
| `content` | string | No* | Prompt content with `{{variable}}` placeholders |
| `file_path` | string | No* | Path to file containing prompt |
| `description` | string | No | Human-readable description |
| `domain` | string | No | `project`, `user`, or `org` (default: `project`) |
| `tags` | array[string] | No | Tags for categorization |
| `variables` | array[object] | No | Explicit variable definitions |
| `merge` | boolean | No | Preserve existing metadata when updating (default: `false`) |

*Either `content` or `file_path` required, not both (unless `merge: true`, which allows metadata-only updates).

**Variable Object:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Variable name (without braces) |
| `description` | string | No | Human-readable description |
| `required` | boolean | No | Whether required (default: true) |
| `default` | string | No | Default value if not provided |

**Example:**

```json
{
  "name": "prompt_save",
  "arguments": {
    "name": "code-review",
    "description": "Comprehensive code review",
    "content": "Review {{file}} for:\n- {{issue_type}} issues\n- Best practices\n- Edge cases",
    "domain": "project",
    "tags": ["review", "quality"],
    "variables": [
      {
        "name": "file",
        "description": "File path to review",
        "required": true
      },
      {
        "name": "issue_type",
        "description": "Type of issues to focus on",
        "default": "general"
      }
    ],
    "merge": true
  }
}
```

**Metadata-only update (preserves content):**

```json
{
  "name": "prompt_save",
  "arguments": {
    "name": "code-review",
    "description": "Updated description only",
    "merge": true
  }
}
```

**Response:**

```json
{
  "name": "code-review",
  "domain": "project",
  "status": "saved"
}
```

---

### prompt_list

List saved prompt templates with optional filtering.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `domain` | string | No | Filter by domain |
| `tags` | array[string] | No | Filter by tags (AND logic) |
| `name_pattern` | string | No | Filter by name pattern (glob) |
| `limit` | integer | No | Maximum results (default: 20, max: 100) |

**Example:**

```json
{
  "name": "prompt_list",
  "arguments": {
    "domain": "project",
    "tags": ["review"],
    "limit": 10
  }
}
```

**Response:**

```json
{
  "prompts": [
    {
      "name": "code-review",
      "description": "Comprehensive code review",
      "domain": "project",
      "tags": ["review", "quality"],
      "variables": ["file", "issue_type"]
    }
  ],
  "total": 1
}
```

---

### prompt_get

Get a prompt template by name.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Prompt name to retrieve |
| `domain` | string | No | Domain to search (cascades if not specified) |

**Domain Cascade:** If domain not specified, searches Project → User → Org.

**Example:**

```json
{
  "name": "prompt_get",
  "arguments": {
    "name": "code-review"
  }
}
```

**Response:**

```json
{
  "name": "code-review",
  "description": "Comprehensive code review",
  "content": "Review {{file}} for:\n- {{issue_type}} issues\n...",
  "domain": "project",
  "tags": ["review", "quality"],
  "variables": [
    {
      "name": "file",
      "description": "File path to review",
      "required": true
    },
    {
      "name": "issue_type",
      "description": "Type of issues to focus on",
      "required": false,
      "default": "general"
    }
  ]
}
```

---

### prompt_run

Run a saved prompt, substituting variable values.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Prompt name to run |
| `variables` | object | No | Variable values (key: value pairs) |
| `domain` | string | No | Domain to search |

**Example:**

```json
{
  "name": "prompt_run",
  "arguments": {
    "name": "code-review",
    "variables": {
      "file": "src/main.rs",
      "issue_type": "security"
    }
  }
}
```

**Response:**

```json
{
  "content": "Review src/main.rs for:\n- security issues\n- Best practices\n- Edge cases",
  "variables_used": {
    "file": "src/main.rs",
    "issue_type": "security"
  }
}
```

---

### prompt_delete

Delete a saved prompt template.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Prompt name to delete |
| `domain` | string | Yes | Domain scope (required for safety) |

**Example:**

```json
{
  "name": "prompt_delete",
  "arguments": {
    "name": "old-template",
    "domain": "project"
  }
}
```

**Response:**

```json
{
  "name": "old-template",
  "domain": "project",
  "status": "deleted"
}
```

---

## Error Codes

| Code | Meaning |
|------|---------|
| -32600 | Invalid request format |
| -32601 | Unknown tool |
| -32602 | Invalid parameters |
| -32603 | Internal error |
| -32604 | Memory not found |
| -32605 | Content blocked (security) |
| -32606 | LLM provider error |
| -32607 | Prompt not found |
| -32608 | Validation error |
