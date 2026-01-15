# MCP Tools

Subcog provides ~22 MCP tools for memory operations, knowledge graph, and prompt/template management.

## Tool Architecture (v0.8.0+)

Subcog uses **action-based tool consolidation** to reduce tool count while maintaining functionality:

- **Consolidated tools**: Single tool with `action` parameter for related operations
- **Legacy tools**: Still available for backward compatibility
- **Security**: All tools use `additionalProperties: false` for parameter validation

## Claude Code Invocation

All subcog MCP tools are accessible using the `subcog:` prefix in Claude Code:

| MCP Tool | Claude Code Syntax |
|----------|-------------------|
| `subcog_capture` | `subcog:capture` |
| `subcog_recall` | `subcog:recall` |
| `subcog_get` | `subcog:get` |
| `subcog_update` | `subcog:update` |
| `subcog_delete` | `subcog:delete` |
| `subcog_status` | `subcog:status` |
| `subcog_gc` | `subcog:gc` |
| `subcog_namespaces` | `subcog:namespaces` |
| `subcog_reindex` | `subcog:reindex` |
| `subcog_enrich` | `subcog:enrich` |
| `subcog_consolidate` | `subcog:consolidate` |
| `subcog_prompts` | `subcog:prompts` |
| `subcog_templates` | `subcog:templates` |
| `subcog_graph` | `subcog:graph` |
| `subcog_entities` | `subcog:entities` |
| `subcog_relationships` | `subcog:relationships` |

**Example - Claude Code:**
```
subcog:recall "database decision" --filter "ns:decisions since:7d"
subcog:prompts --action run --name code-review --variables '{"file":"src/main.rs"}'
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

Search for relevant memories using semantic and text search, or list all memories when query is omitted.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | No | Search query (omit to list all memories) |
| `namespace` | string | No | Filter by namespace |
| `filter` | string | No | Filter expression (see [Query Syntax](../QUERY_SYNTAX.md)) |
| `mode` | string | No | Search mode: `hybrid`, `vector`, `text` (default: `hybrid`) |
| `detail` | string | No | Detail level: `light`, `medium`, `everything` (default: `medium`) |
| `limit` | integer | No | Maximum results (default: 10 for search, 50 for list) |
| `offset` | integer | No | Pagination offset for list mode |
| `user_id` | string | No | Filter by user ID (multi-tenant) |
| `agent_id` | string | No | Filter by agent ID (multi-tenant) |

> **Note**: `subcog_recall` now subsumes `subcog_list`. Omit the `query` parameter to list all memories with filtering and pagination support.

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

### subcog_sync (DEPRECATED)

> **⚠️ Deprecated**: SQLite is now the authoritative storage layer. This tool is a no-op and will be removed in a future version.

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
  "message": "subcog_sync is deprecated. SQLite is now the authoritative storage layer.",
  "status": "deprecated"
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

## Consolidated Tools (v0.8.0+)

### subcog_prompts

Unified prompt template management with action-based dispatch.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `action` | string | Yes | One of: `save`, `list`, `get`, `run`, `delete` |

**Action-specific parameters:**

#### action: save

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Unique prompt name (kebab-case) |
| `content` | string | Yes | Prompt content with `{{variable}}` placeholders |
| `description` | string | No | Human-readable description |
| `domain` | string | No | `project`, `user`, or `org` (default: `project`) |
| `tags` | array[string] | No | Tags for categorization |

#### action: list

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `domain` | string | No | Filter by domain |
| `tags` | array[string] | No | Filter by tags (AND logic) |
| `limit` | integer | No | Maximum results (default: 20) |

#### action: get

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Prompt name to retrieve |
| `domain` | string | No | Domain to search (cascades: project → user → org) |

#### action: run

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Prompt name to run |
| `variables` | object | No | Variable values (key: value pairs) |
| `domain` | string | No | Domain to search |

#### action: delete

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Prompt name to delete |
| `domain` | string | Yes | Domain scope (required for safety) |

**Example - Save:**

```json
{
  "name": "subcog_prompts",
  "arguments": {
    "action": "save",
    "name": "code-review",
    "description": "Comprehensive code review",
    "content": "Review {{file}} for:\n- {{issue_type}} issues\n- Best practices",
    "domain": "project",
    "tags": ["review", "quality"]
  }
}
```

**Example - Run:**

```json
{
  "name": "subcog_prompts",
  "arguments": {
    "action": "run",
    "name": "code-review",
    "variables": {
      "file": "src/main.rs",
      "issue_type": "security"
    }
  }
}
```

---

### subcog_templates

Unified context template management for formatting memories in hooks and responses.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `action` | string | Yes | One of: `save`, `list`, `get`, `render`, `delete` |

**Action-specific parameters:**

#### action: save

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Template name (kebab-case) |
| `content` | string | Yes | Template content with `{{variable}}` syntax |
| `description` | string | No | Human-readable description |
| `domain` | string | No | `project`, `user`, or `org` (default: `project`) |
| `tags` | array[string] | No | Tags for categorization |

#### action: render

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Template name to render |
| `query` | string | No | Search query to populate `{{memories}}` |
| `limit` | integer | No | Max memories to include |
| `format` | string | No | Output format: `markdown`, `json`, `xml` |
| `variables` | object | No | Additional variables to substitute |

**Example - Save:**

```json
{
  "name": "subcog_templates",
  "arguments": {
    "action": "save",
    "name": "search-results",
    "content": "# {{title}}\n\n{{#each memories}}\n- **{{memory.namespace}}**: {{memory.content}}\n{{/each}}",
    "description": "Format search results for display",
    "domain": "user"
  }
}
```

**Example - Render:**

```json
{
  "name": "subcog_templates",
  "arguments": {
    "action": "render",
    "name": "search-results",
    "query": "authentication patterns",
    "limit": 10,
    "variables": { "title": "Auth Patterns" }
  }
}
```

---

### subcog_graph

Unified knowledge graph operations for entity-centric memory retrieval.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `operation` | string | Yes | One of: `neighbors`, `path`, `stats`, `visualize` |

**Operation-specific parameters:**

#### operation: neighbors

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `entity_id` | string | Yes | Entity ID to find neighbors for |
| `depth` | integer | No | Traversal depth (default: 1, max: 3) |
| `relationship_types` | array[string] | No | Filter by relationship types |

#### operation: path

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `from_entity` | string | Yes | Starting entity ID |
| `to_entity` | string | Yes | Target entity ID |
| `max_hops` | integer | No | Maximum path length (default: 5) |

#### operation: stats

No additional parameters. Returns graph statistics.

#### operation: visualize

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `format` | string | No | Output format: `mermaid`, `dot`, `json` (default: `mermaid`) |
| `entity_id` | string | No | Center visualization on entity |
| `entity_types` | array[string] | No | Filter by entity types |
| `depth` | integer | No | Visualization depth (default: 2) |
| `limit` | integer | No | Max nodes to include (default: 50) |

**Example - Neighbors:**

```json
{
  "name": "subcog_graph",
  "arguments": {
    "operation": "neighbors",
    "entity_id": "entity_postgres",
    "depth": 2
  }
}
```

**Example - Visualize:**

```json
{
  "name": "subcog_graph",
  "arguments": {
    "operation": "visualize",
    "format": "mermaid",
    "entity_types": ["Person", "Technology"],
    "depth": 2
  }
}
```

---

### subcog_entities

Unified entity management with CRUD, extraction, and merge operations.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `action` | string | Yes | One of: `create`, `get`, `list`, `delete`, `extract`, `merge` |

**Action-specific parameters:**

#### action: create

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Entity name |
| `entity_type` | string | Yes | One of: `Person`, `Organization`, `Technology`, `Concept`, `File` |
| `properties` | object | No | Additional properties |

#### action: list

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `entity_type` | string | No | Filter by type |
| `limit` | integer | No | Maximum results (default: 50) |

#### action: extract (LLM-powered)

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `content` | string | Yes | Text to extract entities from |
| `store` | boolean | No | Store extracted entities (default: false) |
| `min_confidence` | float | No | Minimum confidence threshold (default: 0.6) |

#### action: merge

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `entity_ids` | array[string] | Yes | Entity IDs to merge |
| `canonical_name` | string | No | Name for merged entity |
| `dry_run` | boolean | No | Preview without applying (default: false) |

**Example - Extract:**

```json
{
  "name": "subcog_entities",
  "arguments": {
    "action": "extract",
    "content": "Alice from Anthropic uses Rust to build the Claude API.",
    "store": true,
    "min_confidence": 0.7
  }
}
```

---

### subcog_relationships

Unified relationship management with CRUD and inference operations.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `action` | string | Yes | One of: `create`, `get`, `list`, `delete`, `infer` |

**Relationship types:** `WorksAt`, `Created`, `Uses`, `Implements`, `PartOf`, `RelatesTo`, `MentionedIn`, `Supersedes`, `ConflictsWith`

#### action: create

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `from_entity` | string | Yes | Source entity ID |
| `to_entity` | string | Yes | Target entity ID |
| `relationship_type` | string | Yes | Type of relationship |
| `properties` | object | No | Additional properties |

#### action: infer (LLM-powered)

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `entity_ids` | array[string] | Yes | Entities to infer relationships between |
| `store` | boolean | No | Store inferred relationships (default: false) |
| `min_confidence` | float | No | Minimum confidence threshold (default: 0.7) |

**Example - Infer:**

```json
{
  "name": "subcog_relationships",
  "arguments": {
    "action": "infer",
    "entity_ids": ["entity_alice", "entity_postgres"],
    "store": true,
    "min_confidence": 0.7
  }
}
```

---

## Legacy Prompt Tools (Deprecated)

> **⚠️ Deprecated**: Use `subcog_prompts` with the appropriate `action` parameter instead. Legacy tools remain available for backward compatibility.

### prompt_save, prompt_list, prompt_get, prompt_run, prompt_delete

These tools are deprecated. Use `subcog_prompts` with `action: save|list|get|run|delete` instead.

See [subcog_prompts](#subcog_prompts) documentation above for the new API.

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
