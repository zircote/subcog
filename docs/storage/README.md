# Storage Architecture

Subcog uses a three-layer storage architecture to provide flexible, performant, and reliable memory persistence.

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│ Application Layer │
│ (Services, MCP, CLI) │
└─────────────────────────────────────────────────────────────┘
 │
 ▼
┌─────────────────────────────────────────────────────────────┐
│ CompositeStorage<P, I, V> │
│ │
│ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ │
│ │ Persistence │ │ Index │ │ Vector │ │
│ │ Layer │ │ Layer │ │ Layer │ │
│ │ (Source │ │ (Searchable │ │ (Embeddings)│ │
│ │ of Truth) │ │ Cache) │ │ │ │
│ └──────┬──────┘ └──────┬──────┘ └──────┬──────┘ │
└─────────┼────────────────┼────────────────┼─────────────────┘
 │ │ │
 ▼ ▼ ▼
┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│ SQLite │ │ SQLite │ │ usearch │
│ PostgreSQL │ │ PostgreSQL │ │ pgvector │
│ Filesystem │ │ Redis │ │ Redis │
└─────────────┘ └─────────────┘ └─────────────┘
```

## Layers

| Layer | Purpose | Backends |
|-------|---------|----------|
| [Persistence](persistence.md) | Authoritative storage | SQLite, PostgreSQL, Filesystem |
| [Index](index.md) | Full-text search | SQLite + FTS5, PostgreSQL, Redis |
| [Vector](vector.md) | Semantic search | usearch, pgvector, Redis |

## Layer Responsibilities

### Persistence Layer (Source of Truth)

- Authoritative storage for all memories
- ACID-compliant transactions
- Faceted storage with project, branch, and file path
- Tombstone support for soft deletes

### Index Layer (Searchable Cache)

- Full-text search with BM25 ranking
- Metadata filtering (namespace, tags, date, facets)
- Can be rebuilt from persistence layer
- Optimized for query performance

### Vector Layer (Embeddings)

- Semantic similarity search
- High-dimensional vector storage (384 dimensions)
- HNSW index for approximate nearest neighbor
- Can be rebuilt from persistence layer

## Default Configuration

```yaml
storage:
 persistence: sqlite
 index: sqlite
 vector: usearch
```

This configuration:
- Uses SQLite for persistence (single file, ACID-compliant)
- Uses SQLite + FTS5 for full-text search (local, no setup)
- Uses usearch for vector search (embedded, fast)

## Backend Combinations

### Local Development (Default)

```yaml
storage:
 persistence: sqlite
 index: sqlite
 vector: usearch
```

**Pros:** No external dependencies, works offline, ACID transactions
**Cons:** Single-machine only

### High-Performance

```yaml
storage:
 persistence: postgresql
 index: postgresql
 vector: pgvector
```

**Pros:** Single database, ACID transactions, scalable
**Cons:** Requires PostgreSQL setup

### Distributed/Cloud

```yaml
storage:
 persistence: postgresql
 index: redis
 vector: redis
```

**Pros:** Horizontally scalable, real-time
**Cons:** Complex setup, operational overhead

## Faceted Storage

Memories are stored with optional facet fields for filtering:

| Field | Description | Auto-detected |
|-------|-------------|---------------|
| `project_id` | Git remote URL (sanitized) | Yes |
| `branch` | Current git branch | Yes |
| `file_path` | Source file path | Optional |
| `tombstoned_at` | Soft delete timestamp | System |

### Capture with Facets

```bash
# Auto-detect from git context
subcog capture --namespace decisions "Use PostgreSQL"

# Explicit facets
subcog capture --namespace decisions --project my-project --branch feature/auth "Added JWT support"
```

### Search with Facets

```bash
# Search within a project
subcog recall "authentication" --project my-project

# Search within a branch
subcog recall "bug fix" --branch feature/auth

# Include tombstoned memories
subcog recall "old decision" --include-tombstoned
```

## Branch Garbage Collection

Clean up memories from deleted branches:

```bash
# GC current project (dry-run)
subcog gc --dry-run

# GC specific branch
subcog gc --branch feature/old-branch

# Purge tombstoned memories older than 30 days
subcog gc --purge --older-than 30d
```

## Data Flow

### Capture

```
Content -> Security Check -> Embedding -> Persistence -> Index -> Vector
 │ │ │
 ▼ ▼ ▼
 SQLite SQLite usearch
```

### Search

```
Query -> Embedding -> Vector Search ──┐
 │
Query -> Index Search ───────────────┼──-> RRF Fusion -> Results
 │
Filter -> Metadata Filter ───────────┘
```

### Rebuild

If index or vector layers become corrupted:

```bash
subcog reindex
```

Rebuilds from persistence layer:

```
Persistence -> Read All -> Re-embed -> Index -> Vector
```

## Best Practices

1. **Always use persistence layer as source of truth**
2. **Backup persistence layer regularly** (database backups)
3. **Index and vector can be rebuilt** if needed
4. **Choose backends based on scale and team size**
5. **Use facets** to organize memories by project and branch
6. **Run GC periodically** to clean up stale branch memories

## See Also

- [Persistence Layer](persistence.md) - SQLite, PostgreSQL, Filesystem
- [Index Layer](index.md) - SQLite, PostgreSQL, Redis
- [Vector Layer](vector.md) - usearch, pgvector, Redis
- [Domains](domains.md) - Scoping and multi-domain
