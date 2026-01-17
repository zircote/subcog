# Subcog Query Syntax

This document describes the filter syntax used by `subcog_recall` and `subcog_browse` for discovering and filtering memories.

> **v0.8.0+**: The `subcog_recall` tool now supports listing all memories when the `query` parameter is omitted. The legacy `subcog_list` prompt is deprecated.

## Overview

Subcog provides a GitHub-style filter syntax that is familiar to developers and supports combining multiple filters with AND/OR logic.

## Filter Types

### Namespace Filter (`ns:`)

Filter memories by namespace:

```
ns:decisions          # Only decisions namespace
ns:patterns           # Only patterns namespace
ns:learnings          # Only learnings namespace
```

**Available namespaces:**
- `decisions` - Architectural and design decisions
- `patterns` - Discovered patterns and conventions
- `learnings` - Lessons learned from debugging
- `context` - Important background information
- `tech-debt` - Technical debt tracking
- `apis` - API documentation and contracts
- `config` - Configuration details
- `security` - Security findings and notes
- `performance` - Optimization notes
- `testing` - Test strategies and edge cases

### Tag Filter (`tag:`)

Filter memories by tags:

```
tag:rust              # Memories tagged with "rust"
tag:error-handling    # Memories tagged with "error-handling"
```

#### OR Logic (comma-separated)

Match ANY of the specified tags:

```
tag:rust,python       # Tagged with "rust" OR "python"
tag:api,rest,graphql  # Tagged with any of these
```

#### AND Logic (space-separated)

Match ALL of the specified tags:

```
tag:rust tag:error    # Tagged with BOTH "rust" AND "error"
tag:api tag:security  # Tagged with BOTH "api" AND "security"
```

#### Exclude Tags (`-tag:`)

Exclude memories with specific tags:

```
-tag:test             # Exclude memories tagged "test"
-tag:deprecated       # Exclude deprecated items
```

### Time Filter (`since:`)

Filter memories by creation time:

```
since:1d              # Created in the last 1 day
since:7d              # Created in the last 7 days
since:30d             # Created in the last 30 days
since:90d             # Created in the last 90 days
```

### Source Filter (`source:`)

Filter by source file reference:

```
source:src/*          # From any file in src/
source:src/auth.rs    # From specific file
source:*.rs           # From any Rust file
```

### Project Filter (`project:`)

Filter by project identifier (normalized git remote URL):

```
project:github.com/org/repo
project:github.com/zircote/subcog
```

### Branch Filter (`branch:`)

Filter by git branch:

```
branch:main
branch:feature/auth
```

### Path Filter (`path:`)

Filter by file path relative to repo root:

```
path:src/main.rs
path:src/services/*
```

### Status Filter (`status:`)

Filter by memory status:

```
status:active         # Active memories (default)
status:archived       # Archived memories
status:superseded     # Superseded by newer memories
status:pending        # Awaiting review
```

## Combining Filters

Filters can be combined for precise queries. Multiple filters are combined with AND logic:

```
ns:decisions tag:database
# Decisions namespace AND tagged with "database"

ns:patterns tag:rust tag:error
# Patterns namespace AND tagged with BOTH "rust" AND "error"

ns:learnings since:7d -tag:test
# Learnings from last 7 days, excluding test-related
```

## Examples

### Common Queries

```
# Recent architecture decisions
ns:decisions since:30d

# All Rust-related patterns and learnings
tag:rust ns:patterns
tag:rust ns:learnings

# Security findings excluding test code
ns:security -tag:test

# API documentation for REST or GraphQL
ns:apis tag:rest,graphql

# Technical debt from the auth module
ns:tech-debt source:src/auth/*

# Decisions scoped to a project branch
ns:decisions project:github.com/org/repo branch:main

# Memories tied to a specific file path
path:src/storage/index/sqlite.rs

# All memories about error handling
tag:error,error-handling,exceptions
```

### Discovery Workflows

**Starting a new feature:**
```
ns:patterns tag:feature-name
ns:decisions tag:feature-name
```

**Debugging an issue:**
```
tag:error,bug,fix since:30d
ns:learnings tag:debugging
```

**Security review:**
```
ns:security status:active
tag:security,auth,secrets
```

**Before refactoring:**
```
ns:tech-debt source:src/target-module/*
ns:decisions tag:architecture
```

## MCP Tools and Prompts

### `subcog_recall` (Tool)

Search for memories or list all memories when query is omitted:

```
Arguments:
  query   - Search query (optional; omit to list all)
  filter  - Filter expression (see syntax above)
  mode    - "hybrid" | "vector" | "text" (default: hybrid)
  detail  - "light" | "medium" | "everything" (default: medium)
  limit   - Maximum results (default: 10 for search, 50 for list)
  offset  - Pagination offset (for list mode)
```

### `subcog_browse` (Prompt)

Interactive memory browser with faceted discovery dashboard:

```
Arguments:
  filter  - Filter expression (see syntax above)
  view    - "dashboard" | "list" (default: dashboard)
  top     - Number of items per facet (default: 10)
```

The dashboard view shows:
- Tag distribution with counts
- Namespace breakdown
- Recent activity timeline
- Source file clusters

### `subcog_list` (Prompt - DEPRECATED)

> **⚠️ Deprecated**: Use `subcog_recall` without a query parameter instead.

Formatted memory listing for export:

```
Arguments:
  filter  - Filter expression (see syntax above)
  format  - "table" | "json" | "markdown" (default: table)
  limit   - Maximum results (default: 50)
```

## MCP Resources

For direct access without filtering, use resources:

| Resource | Description |
|----------|-------------|
| `subcog://project/_` | List all memories |
| `subcog://memory/{id}` | Get specific memory by ID |

## Best Practices

1. **Start broad, narrow down**: Begin with a namespace or broad tag, then add filters
2. **Use OR for exploration**: `tag:a,b,c` casts a wider net
3. **Use AND for precision**: `tag:a tag:b` when you need specific combinations
4. **Combine with time**: Add `since:7d` to focus on recent context
5. **Exclude noise**: Use `-tag:test` to filter out test-related memories
