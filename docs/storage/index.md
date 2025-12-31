# Index Layer

The index layer provides fast full-text search and metadata filtering.

## Backends

| Backend | Description | Use Case |
|---------|-------------|----------|
| SQLite + FTS5 | Embedded database | Default, local development |
| PostgreSQL | Full-text search | Enterprise, shared access |
| Redis | RediSearch | High-performance, distributed |

## SQLite + FTS5 (Default)

Embedded database with full-text search.

### Configuration

```yaml
storage:
  index: sqlite
  sqlite_path: ~/.subcog/index.db
```

### Schema

```sql
-- Main table
CREATE TABLE memories (
    id TEXT PRIMARY KEY,
    namespace TEXT NOT NULL,
    domain TEXT NOT NULL,
    content TEXT NOT NULL,
    tags TEXT,  -- JSON array
    source TEXT,
    status TEXT DEFAULT 'active',
    created_at INTEGER,
    updated_at INTEGER
);

-- FTS5 virtual table
CREATE VIRTUAL TABLE memories_fts USING fts5(
    id,
    content,
    tags,
    content='memories',
    content_rowid='rowid'
);

-- Indexes
CREATE INDEX idx_namespace ON memories(namespace);
CREATE INDEX idx_domain ON memories(domain);
CREATE INDEX idx_created ON memories(created_at DESC);
```

### Search

BM25 full-text search:

```sql
SELECT id, bm25(memories_fts) AS score
FROM memories_fts
WHERE memories_fts MATCH 'database storage'
ORDER BY score;
```

### Advantages

- Zero configuration
- No external dependencies
- Fast for small-medium datasets
- ACID transactions

### Limitations

- Single-writer lock
- Limited scalability
- Local only

---

## PostgreSQL Full-Text

PostgreSQL's built-in full-text search.

### Configuration

```yaml
storage:
  index: postgresql

postgresql:
  host: localhost
  port: 5432
  database: subcog
```

### Schema

```sql
CREATE TABLE memory_index (
    id UUID PRIMARY KEY,
    namespace VARCHAR(50) NOT NULL,
    domain VARCHAR(255) NOT NULL,
    content TEXT NOT NULL,
    tags TEXT[],
    source VARCHAR(255),
    status VARCHAR(20) DEFAULT 'active',
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    -- Full-text search vector
    tsv TSVECTOR GENERATED ALWAYS AS (
        setweight(to_tsvector('english', coalesce(tags::text, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(content, '')), 'B')
    ) STORED
);

-- GIN index for fast FTS
CREATE INDEX idx_memory_tsv ON memory_index USING GIN(tsv);
```

### Search

```sql
SELECT id, ts_rank(tsv, query) AS score
FROM memory_index, to_tsquery('english', 'database & storage') query
WHERE tsv @@ query
ORDER BY score DESC;
```

### Advantages

- Concurrent access
- Shared across instances
- Advanced text search features
- Integrates with persistence layer

### Limitations

- Requires PostgreSQL
- More complex setup

---

## Redis (RediSearch)

Redis with RediSearch module.

### Configuration

```yaml
storage:
  index: redis

redis:
  url: redis://localhost:6379
  prefix: subcog:idx:
```

### Schema

```
FT.CREATE subcog:memories
  ON HASH
  PREFIX 1 subcog:memory:
  SCHEMA
    id TEXT
    namespace TAG
    domain TAG
    content TEXT
    tags TAG SEPARATOR ","
    source TEXT
    status TAG
    created_at NUMERIC SORTABLE
```

### Search

```
FT.SEARCH subcog:memories
  "@content:database storage"
  SORTBY created_at DESC
  LIMIT 0 10
```

### Advantages

- Very fast
- Real-time indexing
- Distributed/replicated
- Rich query language

### Limitations

- Requires Redis + RediSearch
- Memory-intensive
- Operational complexity

---

## IndexBackend Trait

All backends implement:

```rust
#[async_trait]
pub trait IndexBackend: Send + Sync {
    async fn index(&self, memory: &Memory) -> Result<()>;
    async fn remove(&self, id: &MemoryId) -> Result<bool>;
    async fn search(&self, query: &IndexQuery) -> Result<Vec<IndexResult>>;
    async fn filter(&self, filter: &IndexFilter) -> Result<Vec<MemoryId>>;
    async fn rebuild(&self, memories: &[Memory]) -> Result<usize>;
}
```

## Query Structure

```rust
pub struct IndexQuery {
    pub text: String,           // Search text
    pub namespace: Option<Namespace>,
    pub tags_include: Vec<String>,
    pub tags_exclude: Vec<String>,
    pub since: Option<DateTime>,
    pub source_pattern: Option<String>,
    pub limit: usize,
    pub offset: usize,
}
```

## Rebuilding the Index

If the index becomes corrupted:

```bash
subcog reindex
```

This reads all memories from persistence and rebuilds the index.

## Choosing a Backend

| Criteria | SQLite | PostgreSQL | Redis |
|----------|--------|------------|-------|
| Setup | Zero | Medium | Medium |
| Speed | Fast | Fast | Very Fast |
| Concurrency | Limited | High | High |
| Memory | Low | Medium | High |
| Distribution | No | Yes | Yes |

## See Also

- [Persistence Layer](persistence.md) - Source of truth
- [Vector Layer](vector.md) - Semantic search
- [Query Syntax](../QUERY_SYNTAX.md) - Search filters
