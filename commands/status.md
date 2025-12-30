---
description: Show memory system status, statistics, and health information
allowed-tools: mcp__subcog__subcog_status, mcp__subcog__subcog_namespaces, Bash
---

# /subcog:status

Display the current status of the Subcog memory system including storage backends, memory counts, and health metrics.

## Usage

```
/subcog:status
```

## Execution Strategy

<strategy>
**MCP-First Approach:**
1. Try `mcp__subcog__subcog_status` first
2. If MCP unavailable, fallback to CLI: `subcog status`
</strategy>

## Output

<output>
The status command returns:

**System Information:**
- Version number
- Operational status
- Active backends (persistence, index, vector)

**Storage Backends:**
- Persistence: git-notes | postgresql | filesystem
- Index: sqlite-fts5 | postgresql | redis
- Vector: usearch | pgvector | redis

**Statistics:**
- Total memories captured
- Memories by namespace breakdown
- Index entries count
- Vector embeddings count

**Sync Status:**
- Last sync timestamp
- Remote repository (if configured)
- Pending sync operations
</output>
