# MCP Resources

Subcog exposes 26+ resources through the `subcog://` URI scheme for browsing and fetching memories.

## Resource Categories

| Category | Description |
|----------|-------------|
| [Help Resources](#help-resources) | Documentation and guides |
| [Memory Listing](#memory-listing) | Browse memories by scope |
| [Memory Fetch](#memory-fetch) | Get full memory content |
| [Topics](#topics) | Topic-based navigation |
| [Prompts](#prompts) | Prompt template access |

---

## Help Resources

Built-in help documentation.

| URI | Description |
|-----|-------------|
| `subcog://help` | Help index with all topics |
| `subcog://help/setup` | Installation and configuration |
| `subcog://help/concepts` | Core concepts: namespaces, domains, URNs |
| `subcog://help/capture` | How to capture memories |
| `subcog://help/search` | Using hybrid search |
| `subcog://help/workflows` | Integration workflows |
| `subcog://help/troubleshooting` | Common issues and solutions |
| `subcog://help/advanced` | LLM integration, consolidation |

**Example Response:**

```json
{
  "uri": "subcog://help",
  "mimeType": "text/markdown",
  "text": "# Subcog Help\n\nAvailable topics:\n- setup\n- concepts\n..."
}
```

---

## Memory Listing

List endpoints return frontmatter only (no content) for efficient browsing.

### Cross-Domain (Aggregate)

| URI | Description |
|-----|-------------|
| `subcog://_` | All memories across all domains |
| `subcog://_/{namespace}` | Filter by namespace, all domains |
| `subcog://_/_` | All memories (explicit wildcard) |

### Project Scope

| URI | Description |
|-----|-------------|
| `subcog://project` | Project-scoped memories |
| `subcog://project/{namespace}` | Project memories by namespace |
| `subcog://project/_` | Project memories, all namespaces |

### User Scope

| URI | Description |
|-----|-------------|
| `subcog://user` | User-scoped (global) memories |
| `subcog://user/{namespace}` | User memories by namespace |
| `subcog://user/_` | User memories, all namespaces |

### Org Scope

| URI | Description |
|-----|-------------|
| `subcog://org` | Organization-scoped memories |
| `subcog://org/{namespace}` | Org memories by namespace |
| `subcog://org/_` | Org memories, all namespaces |

**List Response Format:**

```json
{
  "uri": "subcog://project/decisions",
  "mimeType": "application/json",
  "text": "{\"count\": 5, \"memories\": [{\"id\": \"dc58d23a...\", \"ns\": \"decisions\", \"tags\": [\"database\"], \"uri\": \"subcog://memory/dc58d23a...\"}]}"
}
```

**Included in List:** id, ns, tags, uri
**Excluded from List:** content, domain, source, timestamps

---

## Memory Fetch

Fetch endpoints return complete memory data.

| URI | Description |
|-----|-------------|
| `subcog://memory/{id}` | Cross-domain lookup by ID |
| `subcog://project/{namespace}/{id}` | Scoped lookup with validation |
| `subcog://user/{namespace}/{id}` | User-scoped lookup |
| `subcog://org/{namespace}/{id}` | Org-scoped lookup |

**Cross-Domain Lookup:**

```
subcog://memory/dc58d23a35876f5a59426e81aaa81d796efa7fc1
```

Searches all domains and returns the memory regardless of scope.

**Scoped Lookup:**

```
subcog://project/decisions/dc58d23a35876f5a59426e81aaa81d796efa7fc1
```

Validates that the memory exists in the specified namespace. Returns error if not.

**Fetch Response Format:**

```json
{
  "uri": "subcog://memory/dc58d23a...",
  "mimeType": "application/json",
  "text": "{\"urn\": \"subcog://project/decisions/dc58d23a...\", \"id\": \"dc58d23a...\", \"namespace\": \"decisions\", \"domain\": \"zircote/subcog\", \"content\": \"Use PostgreSQL for primary storage...\", \"tags\": [\"database\", \"architecture\"], \"source\": \"ARCHITECTURE.md\", \"status\": \"active\", \"created_at\": 1703894400, \"updated_at\": 1703894400}"
}
```

---

## Topics

Topic-based navigation for discovering memories by subject.

| URI | Description |
|-----|-------------|
| `subcog://topics` | List all topics with memory counts |
| `subcog://topics/{topic}` | Get memories for a specific topic |

**Topics List Response:**

```json
{
  "uri": "subcog://topics",
  "mimeType": "application/json",
  "text": "{\"topics\": [{\"name\": \"database\", \"count\": 8}, {\"name\": \"api\", \"count\": 6}, {\"name\": \"authentication\", \"count\": 4}]}"
}
```

**Topic Memories Response:**

```json
{
  "uri": "subcog://topics/database",
  "mimeType": "application/json",
  "text": "{\"topic\": \"database\", \"memories\": [{\"id\": \"dc58d23a...\", \"ns\": \"decisions\", \"tags\": [\"database\", \"postgresql\"]}]}"
}
```

---

## Prompts

Access saved prompt templates via resources.

| URI | Description |
|-----|-------------|
| `subcog://_prompts` | List all prompts (all domains) |
| `subcog://project/_prompts` | List project prompts |
| `subcog://user/_prompts` | List user prompts |
| `subcog://org/_prompts` | List org prompts |
| `subcog://project/_prompts/{name}` | Get specific project prompt |
| `subcog://user/_prompts/{name}` | Get specific user prompt |
| `subcog://org/_prompts/{name}` | Get specific org prompt |

**Prompts List Response:**

```json
{
  "uri": "subcog://project/_prompts",
  "mimeType": "application/json",
  "text": "{\"prompts\": [{\"name\": \"code-review\", \"description\": \"Code review template\", \"domain\": \"project\", \"tags\": [\"review\"], \"variables\": [\"file\", \"issue_type\"]}]}"
}
```

**Prompt Detail Response:**

```json
{
  "uri": "subcog://project/_prompts/code-review",
  "mimeType": "application/json",
  "text": "{\"name\": \"code-review\", \"content\": \"Review {{file}} for...\", \"domain\": \"project\", \"variables\": [{\"name\": \"file\", \"required\": true}, {\"name\": \"issue_type\", \"default\": \"general\"}]}"
}
```

---

## Namespaces

List namespaces and their contents.

| URI | Description |
|-----|-------------|
| `subcog://namespaces` | List all namespaces |
| `subcog://namespaces/{ns}` | Get memories in namespace |

---

## URI Hierarchy

```
subcog://
├── help/
│   ├── setup
│   ├── concepts
│   ├── capture
│   ├── search
│   ├── workflows
│   ├── troubleshooting
│   └── advanced
│
├── _/                          (aggregate across all domains)
│   ├── _/                      (all namespaces - wildcard)
│   └── {namespace}/            (filter by namespace)
│
├── project/                    (project scope)
│   ├── _/                      (all namespaces)
│   ├── {namespace}/            (filter by namespace)
│   │   └── {id}                (specific memory)
│   └── _prompts/               (prompt templates)
│       └── {name}              (specific prompt)
│
├── user/                       (user-wide scope)
│   ├── _/                      (all namespaces)
│   ├── {namespace}/
│   │   └── {id}
│   └── _prompts/
│       └── {name}
│
├── org/                        (organization scope)
│   ├── _/                      (all namespaces)
│   ├── {namespace}/
│   │   └── {id}
│   └── _prompts/
│       └── {name}
│
├── memory/                     (direct ID lookup)
│   └── {id}                    (cross-domain fetch)
│
├── topics/                     (topic navigation)
│   └── {topic}                 (topic memories)
│
├── namespaces/                 (namespace navigation)
│   └── {namespace}             (namespace memories)
│
└── _prompts/                   (aggregate prompts)
    └── {name}                  (prompt by name)
```

---

## Progressive Disclosure

Resources implement progressive disclosure:

1. **List endpoints** (`/project`, `/project/{ns}`) return minimal data
2. **Fetch endpoints** (`/memory/{id}`) return complete data

This optimizes token usage in LLM interactions.

---

## Error Responses

| Status | Meaning |
|--------|---------|
| 404 | Resource not found |
| 400 | Invalid URI format |
| 500 | Internal error |

Error Response:

```json
{
  "error": {
    "code": 404,
    "message": "Memory not found: dc58d23a..."
  }
}
```

---

## See Also

- [URN Guide](../URN-GUIDE.md) - Complete URN/URI documentation
- [Tools](tools.md) - MCP tools reference
- [Prompts](./prompts.md) - MCP prompts reference
