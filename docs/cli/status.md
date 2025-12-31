# subcog status

Display system status and statistics.

## Synopsis

```
subcog status [OPTIONS]
```

## Description

The `status` command displays comprehensive information about the Subcog system including repository details, storage backend status, memory statistics, and feature flags.

## Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--json` | `-j` | Output in JSON format | `false` |
| `--verbose` | `-v` | Show detailed information | `false` |

## Output Sections

### Repository Information

- Current working directory
- Git repository status
- Domain scope (project/user/org)
- Organization/repository name

### Storage Status

For each storage layer:
- **Persistence**: Backend type and location
- **Index**: Backend type and statistics
- **Vector**: Backend type and index size

### Statistics

- Total memory count
- Count by namespace
- Recent activity
- Top tags

### Feature Flags

Status of optional features:
- Secrets filtering
- PII filtering
- Multi-domain support
- Audit logging
- LLM features
- Auto-capture
- Consolidation

## Examples

### Basic Status

```bash
subcog status
```

Output:
```
Subcog Memory System
════════════════════════════════════════════════════════════════

Repository
──────────
  Path: /Users/user/project
  Domain: project
  Scope: zircote/subcog

Storage
───────
  Persistence: Git Notes (refs/notes/subcog)
  Index: SQLite + FTS5 (~/.subcog/index.db)
  Vector: usearch HNSW (~/.subcog/vectors.usearch)

Statistics
──────────
  Total memories: 42

  By namespace:
    decisions    ████████████ 12
    patterns     ████████     8
    learnings    ███████████████ 15
    context      ███████      7

  Recent activity (7d): 8 captures

  Top tags:
    rust (15), database (8), api (6), security (4)

Features
────────
  ✓ Secrets filter
  ✓ PII filter
  ✓ Multi-domain
  ✗ Audit log (disabled)
  ✓ LLM features
  ✗ Auto-capture (disabled)
  ✗ Consolidation (disabled)
```

### JSON Output

```bash
subcog status --json
```

Output:
```json
{
  "repository": {
    "path": "/Users/user/project",
    "domain": "project",
    "scope": "zircote/subcog"
  },
  "storage": {
    "persistence": {
      "type": "git_notes",
      "ref": "refs/notes/subcog"
    },
    "index": {
      "type": "sqlite",
      "path": "~/.subcog/index.db",
      "fts5": true
    },
    "vector": {
      "type": "usearch",
      "path": "~/.subcog/vectors.usearch",
      "dimensions": 384
    }
  },
  "statistics": {
    "total": 42,
    "by_namespace": {
      "decisions": 12,
      "patterns": 8,
      "learnings": 15,
      "context": 7
    },
    "recent_7d": 8,
    "top_tags": [
      {"tag": "rust", "count": 15},
      {"tag": "database", "count": 8},
      {"tag": "api", "count": 6},
      {"tag": "security", "count": 4}
    ]
  },
  "features": {
    "secrets_filter": true,
    "pii_filter": true,
    "multi_domain": true,
    "audit_log": false,
    "llm_features": true,
    "auto_capture": false,
    "consolidation": false
  }
}
```

### Verbose Output

```bash
subcog status -v
```

Adds:
- Storage layer health checks
- Index rebuild status
- Vector index details
- Last sync timestamp
- Configuration source

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Status retrieved successfully |
| 1 | Error retrieving status |
| 3 | Configuration error |
| 4 | Storage error |

## Health Checks

When verbose mode is enabled, the following health checks are performed:

1. **Git repository**: Verifies `.git` directory exists
2. **Git notes**: Checks refs/notes/subcog exists
3. **SQLite index**: Verifies database is accessible
4. **Vector index**: Checks usearch index is loaded
5. **Embedding model**: Verifies model is available

## See Also

- [config](config.md) - Manage configuration
- [sync](sync.md) - Synchronize with remote
- [MCP subcog_status](../mcp/tools.md#subcog_status) - MCP equivalent
