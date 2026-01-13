# Subcog URN and URI Guide

This document describes the Uniform Resource Name (URN) and URI schemes used in Subcog for addressing and accessing memories.

---

## Table of Contents

1. [Overview](#overview)
2. [Canonical URN Format](#canonical-urn-format)
3. [MCP Resource URI Scheme](#mcp-resource-uri-scheme)
4. [Progressive Disclosure Pattern](#progressive-disclosure-pattern)
5. [Namespaces Reference](#namespaces-reference)
6. [Domain Scopes](#domain-scopes)
7. [Example Workflows](#example-workflows)
8. [Best Practices](#best-practices)

---

## Overview

Subcog uses two complementary addressing schemes:

| Scheme | Format | Purpose |
|--------|--------|---------|
| **URN** | `subcog://...` | Canonical, persistent identifier for memories |

The URN is the authoritative identifier stored with each memory and provides hierarchical navigation for the Model Context Protocol (MCP).

---

## Canonical URN Format

The canonical URN format for memories is:

```
subcog://{domain}/{namespace}/{id}
```

### Components

| Component | Description | Examples |
|-----------|-------------|----------|
| `domain` | Scope of the memory | `zircote/subcog`, `global`, `project` |
| `namespace` | Category of memory | `decisions`, `patterns`, `learnings` |
| `id` | Unique identifier | `dc58d23a...`, `1314b968...` (git SHA1 hashes) |

### Examples

```
subcog://project/decisions/dc58d23a35876f5a59426e81aaa81d796efa7fc1
subcog://global/learnings/1314b9681301b9337559b7c5ad7af7e22dc76fc7
subcog://zircote/subcog/patterns/a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0
```

### ID Formats

Memory IDs can be in different formats depending on the storage backend:

| Format | Example | Source |
|--------|---------|--------|
| Git SHA1 | `dc58d23a35876f5a59426e81aaa81d796efa7fc1` | Legacy import (deprecated) |
| Short UUID | `abc123def456` | Capture service (12 hex chars) |

IDs are opaque identifiers - use them as-is without parsing.

---

## MCP Resource URI Scheme

The `subcog://` URI scheme provides hierarchical access to memories via MCP resources.

### Help Resources

| URI | Description |
|-----|-------------|
| `subcog://help` | Help index with all available topics |
| `subcog://help/setup` | Installation and configuration guide |
| `subcog://help/concepts` | Core concepts: namespaces, domains, URNs |
| `subcog://help/capture` | How to capture memories |
| `subcog://help/search` | Using hybrid search |
| `subcog://help/workflows` | Integration workflows |
| `subcog://help/troubleshooting` | Common issues and solutions |
| `subcog://help/advanced` | LLM integration, consolidation |

### Memory Listing (Progressive Disclosure)

List endpoints return frontmatter only (no content) for efficient browsing:

| URI | Returns |
|-----|---------|
| `subcog://_` | All memories across all domains |
| `subcog://_/{namespace}` | All memories filtered by namespace |
| `subcog://_/_` | All memories, all namespaces (explicit wildcard) |
| `subcog://project` | Project-scoped memories |
| `subcog://project/{namespace}` | Project memories by namespace |
| `subcog://project/_` | Project memories, all namespaces (explicit wildcard) |
| `subcog://org` | Organization-scoped memories |
| `subcog://org/{namespace}` | Org memories by namespace |
| `subcog://org/_` | Org memories, all namespaces (explicit wildcard) |
| `subcog://global` | Global/user-scoped memories |
| `subcog://global/{namespace}` | Global memories by namespace |
| `subcog://global/_` | Global memories, all namespaces (explicit wildcard) |

> **Note:** The `_` wildcard can be used at both the domain level (`subcog://_`) and the namespace level (`subcog://project/_`) to match all values.

### Memory Fetch (Full Content)

Fetch endpoints return complete memory data including content:

| URI | Description |
|-----|-------------|
| `subcog://memory/{id}` | Cross-domain lookup by ID |
| `subcog://project/{namespace}/{id}` | Scoped lookup with namespace validation |
| `subcog://org/{namespace}/{id}` | Org-scoped lookup with validation |
| `subcog://global/{namespace}/{id}` | Global-scoped lookup with validation |

### URI Hierarchy

```
subcog://
    |
    +-- help/
    |      +-- setup
    |      +-- concepts
    |      +-- capture
    |      +-- search
    |      +-- workflows
    |      +-- troubleshooting
    |      +-- advanced
    |
    +-- _/                          (aggregate across all domains)
    |   +-- _/                      (all namespaces - wildcard)
    |   +-- {namespace}/            (filter by namespace)
    |
    +-- project/                    (project scope)
    |   +-- _/                      (all namespaces - wildcard)
    |   +-- {namespace}/            (filter by namespace)
    |       +-- {id}                (specific memory)
    |
    +-- org/                        (organization scope)
    |   +-- _/                      (all namespaces - wildcard)
    |   +-- {namespace}/            (filter by namespace)
    |       +-- {id}                (specific memory)
    |
    +-- global/                     (user-wide scope)
    |   +-- _/                      (all namespaces - wildcard)
    |   +-- {namespace}/            (filter by namespace)
    |       +-- {id}                (specific memory)
    |
    +-- memory/                     (direct ID lookup)
        +-- {id}                    (cross-domain fetch)
```

---

## Progressive Disclosure Pattern

Subcog uses progressive disclosure to optimize token usage and response size:

### 1. List Endpoints (Minimal)

List endpoints (`subcog://project`, `subcog://project/{namespace}`) return bare minimum for informed selection:

```json
{
  "count": 5,
  "memories": [
    {
      "id": "dc58d23a35876f5a59426e81aaa81d796efa7fc1",
      "ns": "decisions",
      "tags": ["database", "storage"],
      "uri": "subcog://memory/dc58d23a35876f5a59426e81aaa81d796efa7fc1"
    }
  ]
}
```

**Included:** id, ns, tags, uri

**Excluded:** content, domain, source, timestamps (use fetch for full details)

Tags provide context for selection decisions without loading full content.

### 2. Fetch Endpoints (Full Content)

Fetch endpoints (`subcog://memory/{id}`, `subcog://project/{namespace}/{id}`) return complete data:

```json
{
  "urn": "subcog://project/decisions/dc58d23a35876f5a59426e81aaa81d796efa7fc1",
  "id": "dc58d23a35876f5a59426e81aaa81d796efa7fc1",
  "namespace": "decisions",
  "domain": "zircote/subcog",
  "content": "Use PostgreSQL for primary storage because...",
  "tags": ["database", "storage", "architecture"],
  "source": "ARCHITECTURE.md",
  "status": "active",
  "created_at": 1703894400,
  "updated_at": 1703894400
}
```

### 3. Search Tool Detail Levels

The `subcog_recall` tool supports parametric detail levels:

| Level | Alias | Content Returned |
|-------|-------|------------------|
| `light` | `minimal`, `frontmatter` | No content (frontmatter only) |
| `medium` | `summary`, `default` | Truncated content (~200 chars) |
| `everything` | `full`, `all` | Complete content |

**Example:**

```json
{
  "tool": "subcog_recall",
  "arguments": {
    "query": "database storage",
    "detail": "light",
    "limit": 10
  }
}
```

---

## Namespaces Reference

| Namespace | Purpose | Signal Words |
|-----------|---------|--------------|
| `decisions` | Architectural and design decisions | "decided", "chose", "going with" |
| `patterns` | Discovered patterns and conventions | "always", "never", "convention" |
| `learnings` | Lessons learned from debugging | "TIL", "learned", "discovered" |
| `context` | Important background information | "because", "constraint", "requirement" |
| `tech-debt` | Technical debt tracking | "TODO", "FIXME", "temporary", "hack" |
| `blockers` | Blockers and impediments | "blocked", "waiting", "depends on" |
| `progress` | Work progress and milestones | "completed", "milestone", "shipped" |
| `apis` | API documentation and contracts | "endpoint", "request", "response" |
| `config` | Configuration details | "environment", "setting", "variable" |
| `security` | Security findings and notes | "vulnerability", "CVE", "auth" |
| `performance` | Optimization notes | "benchmark", "latency", "throughput" |
| `testing` | Test strategies and edge cases | "test", "edge case", "coverage" |

### System Namespace

| Namespace | Purpose |
|-----------|---------|
| `help` | Built-in help content (read-only) |

---

## Domain Scopes

| Scope | Description | Use Case |
|-------|-------------|----------|
| `project` | Current repository/project | Most common; repo-specific decisions |
| `org` | Organization-level | Cross-repo patterns, org standards |
| `global` | User-wide memories | Personal learnings, universal patterns |
| `_` | Aggregate (wildcard) | Search/browse across all domains |

### Domain Format

Domains can be specified in various formats:

| Format | Example | Description |
|--------|---------|-------------|
| `org/repo` | `zircote/subcog` | Full GitHub-style path |
| `org` | `zircote` | Organization only |
| `global` | `global` | User-wide scope |

---

## Example Workflows

### Workflow 1: Browsing Project Memories

Navigate hierarchically from broad to specific:

```
1. Start broad:           subcog://project
2. Filter by namespace:   subcog://project/decisions
3. Fetch specific memory: subcog://project/decisions/dc58d23a35876f5a59426e81aaa81d796efa7fc1
```

The three-part URI (`subcog://project/decisions/{id}`) validates that the memory exists in the specified namespace. If the memory is in a different namespace, an error is returned.

**Response flow:**

```
List (project)          -->  List (decisions)      -->  Full content + URN
{count: 42, ...}             {count: 8, ...}            {urn: "subcog://...", content: "..."}
```

### Workflow 2: Cross-Domain Lookup

When you know the memory ID but not the domain or namespace:

```
subcog://memory/dc58d23a35876f5a59426e81aaa81d796efa7fc1
```

This performs a cross-domain lookup and returns the full memory content regardless of which domain or namespace it resides in. Use this when you have an ID from search results or logs but don't know the full context.

### Workflow 3: Search with Detail Levels

Use the `subcog_recall` tool with progressive detail:

```json
// Step 1: Quick scan (frontmatter only)
{
  "tool": "subcog_recall",
  "arguments": {
    "query": "authentication patterns",
    "detail": "light",
    "limit": 20
  }
}

// Step 2: Review summaries of interesting results
{
  "tool": "subcog_recall",
  "arguments": {
    "query": "JWT token validation",
    "detail": "medium",
    "limit": 5
  }
}

// Step 3: Get full content of specific memories
{
  "resource": "subcog://memory/1314b9681301b9337559b7c5ad7af7e22dc76fc7"
}
```

### Workflow 4: Namespace-Filtered Aggregate Search

Search across all domains but filter by namespace:

```
subcog://_/decisions
```

Returns all decision memories from all domains (project, org, global).

### Workflow 5: Capture and Recall Consistency

Memories are stored with canonical URNs and can be recalled using the same addressing:

**Capture:**
```json
{
  "tool": "subcog_capture",
  "arguments": {
    "namespace": "decisions",
    "content": "Use RRF for hybrid search fusion"
  }
}
// Returns: { "urn": "subcog://project/decisions/dc58d23a...", "id": "dc58d23a..." }
```

**Recall by URN components:**
```
subcog://project/decisions/dc58d23a35876f5a59426e81aaa81d796efa7fc1
```

The scoped URI validates the memory exists in the `decisions` namespace and returns the canonical URN in the response.

---

## Best Practices

### 1. Start Broad, Drill Down

Use list endpoints to discover memories, then fetch specific ones:

```
subcog://project  -->  subcog://project/decisions  -->  subcog://memory/{id}
```

### 2. Use Appropriate Detail Levels

- `light`: Initial scanning, building context inventories
- `medium`: Reviewing search results (default)
- `everything`: Deep analysis, copying content

### 3. Prefer Scoped Lookups for Validation

When you expect a memory to be in a specific namespace, use scoped URIs:

```
subcog://project/decisions/dc58d23a35876f5a59426e81aaa81d796efa7fc1
```

This validates the memory exists in the expected namespace and returns an error if not.

### 4. Use Cross-Domain Lookup for Flexibility

When the namespace is unknown or you want flexibility:

```
subcog://memory/dc58d23a35876f5a59426e81aaa81d796efa7fc1
```

### 5. Leverage the Aggregate Scope

To search across all domains simultaneously:

```
subcog://_                    // All memories
subcog://_/patterns           // All patterns from all domains
```

### 6. Consistent URN References

When referencing memories in documentation or code comments, use the canonical URN format:

```
See: subcog://project/decisions/dc58d23a35876f5a59426e81aaa81d796efa7fc1
```

---

## API Reference

### MCP Tools

| Tool | Purpose | Key Parameters |
|------|---------|----------------|
| `subcog_capture` | Store a new memory | `namespace`, `content`, `tags`, `source` |
| `subcog_recall` | Search memories | `query`, `namespace`, `mode`, `detail`, `limit` |
| `subcog_status` | System status | (none) |
| `subcog_namespaces` | List namespaces | (none) |
| `subcog_sync` | Sync with git remote | `direction` |
| `subcog_consolidate` | LLM consolidation | `namespace`, `strategy`, `dry_run` |
| `subcog_enrich` | LLM enrichment | `memory_id`, `enrich_tags` |
| `subcog_reindex` | Rebuild search index | `repo_path` |

### Search Modes

| Mode | Algorithm | Best For |
|------|-----------|----------|
| `hybrid` | RRF fusion of vector + BM25 | General search (default) |
| `vector` | Semantic similarity only | Concept-based search |
| `text` | BM25 keyword matching | Exact term matching |

### Response Fields

**List Response:**
```json
{
  "count": 5,
  "memories": [{"id": "...", "ns": "...", "tags": [...], "uri": "..."}]
}
```

**Fetch Response:**
```json
{
  "urn": "subcog://...",
  "id": "...",
  "namespace": "...",
  "domain": "...",
  "content": "...",
  "tags": [...],
  "source": "...",
  "status": "...",
  "created_at": 1703894400,
  "updated_at": 1703894400
}
```

---

## See Also

- [ARCHITECTURE.md](spec/completed/2025-12-28-subcog-rust-rewrite/ARCHITECTURE.md) - System architecture
- [REQUIREMENTS.md](spec/completed/2025-12-28-subcog-rust-rewrite/REQUIREMENTS.md) - Product requirements
- [src/mcp/resources.rs](../src/mcp/resources.rs) - Resource handler implementation
- [src/mcp/tools.rs](../src/mcp/tools.rs) - Tool implementations
