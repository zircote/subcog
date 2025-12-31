# Storage Architecture

Subcog uses a three-layer storage architecture to provide flexible, performant, and reliable memory persistence.

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      Application Layer                       │
│                   (Services, MCP, CLI)                       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    CompositeStorage<P, I, V>                 │
│                                                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │ Persistence │  │    Index    │  │   Vector    │         │
│  │   Layer     │  │    Layer    │  │    Layer    │         │
│  │  (Source    │  │ (Searchable │  │ (Embeddings)│         │
│  │   of Truth) │  │    Cache)   │  │             │         │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘         │
└─────────┼────────────────┼────────────────┼─────────────────┘
          │                │                │
          ▼                ▼                ▼
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│  Git Notes  │    │   SQLite    │    │   usearch   │
│ PostgreSQL  │    │  PostgreSQL │    │  pgvector   │
│ Filesystem  │    │    Redis    │    │    Redis    │
└─────────────┘    └─────────────┘    └─────────────┘
```

## Layers

| Layer | Purpose | Backends |
|-------|---------|----------|
| [Persistence](persistence.md) | Authoritative storage | Git Notes, PostgreSQL, Filesystem |
| [Index](index.md) | Full-text search | SQLite + FTS5, PostgreSQL, Redis |
| [Vector](vector.md) | Semantic search | usearch, pgvector, Redis |

## Layer Responsibilities

### Persistence Layer (Source of Truth)

- Authoritative storage for all memories
- Durable, replicated storage
- Git-based sync and versioning
- Recovery source if other layers fail

### Index Layer (Searchable Cache)

- Full-text search with BM25 ranking
- Metadata filtering (namespace, tags, date)
- Can be rebuilt from persistence layer
- Optimized for query performance

### Vector Layer (Embeddings)

- Semantic similarity search
- High-dimensional vector storage
- HNSW index for approximate nearest neighbor
- Can be rebuilt from persistence layer

## Default Configuration

```yaml
storage:
  persistence: git_notes
  index: sqlite
  vector: usearch
```

This configuration:
- Uses Git Notes for persistence (works with any Git repo)
- Uses SQLite + FTS5 for full-text search (local, no setup)
- Uses usearch for vector search (embedded, fast)

## Backend Combinations

### Local Development (Default)

```yaml
storage:
  persistence: git_notes
  index: sqlite
  vector: usearch
```

**Pros:** No external dependencies, works offline
**Cons:** Single-machine only

### Team Collaboration

```yaml
storage:
  persistence: git_notes
  index: sqlite
  vector: usearch
```

With git remote for sync:
```bash
subcog sync
```

**Pros:** Distributed via Git, simple setup
**Cons:** Eventual consistency

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

## Domain Scoping

See [Domains](domains.md) for details on:
- Project scope
- User scope
- Organization scope
- Cross-domain queries

## Data Flow

### Capture

```
Content → Security Check → Embedding → Persistence → Index → Vector
                                           │            │        │
                                           ▼            ▼        ▼
                                       Git Notes    SQLite    usearch
```

### Search

```
Query → Embedding → Vector Search ──┐
                                    │
Query → Index Search ───────────────┼──→ RRF Fusion → Results
                                    │
Filter → Metadata Filter ───────────┘
```

### Rebuild

If index or vector layers become corrupted:

```bash
subcog reindex
```

Rebuilds from persistence layer:

```
Persistence → Read All → Re-embed → Index → Vector
```

## Best Practices

1. **Always use persistence layer as source of truth**
2. **Backup persistence layer regularly** (git push)
3. **Index and vector can be rebuilt** if needed
4. **Choose backends based on scale and team size**

## See Also

- [Persistence Layer](persistence.md) - Git Notes, PostgreSQL, Filesystem
- [Index Layer](index.md) - SQLite, PostgreSQL, Redis
- [Vector Layer](vector.md) - usearch, pgvector, Redis
- [Domains](domains.md) - Scoping and multi-domain
