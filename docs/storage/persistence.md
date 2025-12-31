# Persistence Layer

The persistence layer is the authoritative source of truth for all memories.

## Backends

| Backend | Description | Use Case |
|---------|-------------|----------|
| Git Notes | Git-based storage | Default, distributed teams |
| PostgreSQL | Relational database | High-performance, enterprise |
| Filesystem | Local file storage | Development, offline |

## Git Notes (Default)

Stores memories as Git notes attached to the repository.

### Configuration

```yaml
storage:
  persistence: git_notes
```

### How It Works

Memories are stored in:
- `refs/notes/subcog` - Memory storage
- `refs/notes/_prompts` - Prompt templates

Each memory is stored as a blob with YAML frontmatter:

```yaml
---
id: dc58d23a35876f5a59426e81aaa81d796efa7fc1
namespace: decisions
tags: [database, postgresql]
source: ARCHITECTURE.md
created_at: 2024-01-15T10:30:00Z
updated_at: 2024-01-15T10:30:00Z
status: active
---
Use PostgreSQL for primary storage because of JSONB support
and excellent performance characteristics.
```

### Sync

Sync with remote:

```bash
subcog sync
```

Equivalent git commands:
```bash
git fetch origin refs/notes/subcog:refs/notes/subcog
git push origin refs/notes/subcog
```

### Advantages

- No external dependencies
- Works offline
- Distributed via Git
- Version history built-in
- Works with existing Git infrastructure

### Limitations

- Large repositories may slow down
- Binary-unfriendly (text-based)
- Eventual consistency with remotes

---

## PostgreSQL

Stores memories in a PostgreSQL database.

### Configuration

```yaml
storage:
  persistence: postgresql

postgresql:
  host: localhost
  port: 5432
  database: subcog
  user: subcog
  password: ${SUBCOG_PG_PASSWORD}
  ssl_mode: prefer
```

Or via connection URL:

```bash
export DATABASE_URL="postgres://subcog:pass@localhost:5432/subcog"
```

### Schema

```sql
CREATE TABLE memories (
    id UUID PRIMARY KEY,
    namespace VARCHAR(50) NOT NULL,
    domain VARCHAR(255) NOT NULL,
    content TEXT NOT NULL,
    tags TEXT[] DEFAULT '{}',
    source VARCHAR(255),
    status VARCHAR(20) DEFAULT 'active',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_memories_namespace ON memories(namespace);
CREATE INDEX idx_memories_domain ON memories(domain);
CREATE INDEX idx_memories_tags ON memories USING GIN(tags);
CREATE INDEX idx_memories_created ON memories(created_at DESC);
```

### Advantages

- ACID transactions
- Concurrent access
- Complex queries
- Scalable

### Limitations

- Requires PostgreSQL setup
- External dependency
- Operational overhead

---

## Filesystem

Stores memories as files in a directory structure.

### Configuration

```yaml
storage:
  persistence: filesystem
  filesystem_path: ~/.subcog/memories
```

### Structure

```
~/.subcog/memories/
├── project/
│   └── zircote-subcog/
│       ├── decisions/
│       │   └── dc58d23a.yaml
│       ├── patterns/
│       │   └── 1314b968.yaml
│       └── learnings/
│           └── a1b2c3d4.yaml
├── user/
│   └── learnings/
│       └── e5f6a7b8.yaml
└── org/
    └── zircote/
        └── patterns/
            └── c9d0e1f2.yaml
```

### File Format

Same YAML format as Git Notes:

```yaml
---
id: dc58d23a35876f5a59426e81aaa81d796efa7fc1
namespace: decisions
tags: [database]
created_at: 2024-01-15T10:30:00Z
---
Content here
```

### Advantages

- Simple, human-readable
- Easy to backup
- No dependencies
- Good for debugging

### Limitations

- No built-in sync
- Single machine only
- Manual backup needed

---

## PersistenceBackend Trait

All backends implement:

```rust
#[async_trait]
pub trait PersistenceBackend: Send + Sync {
    async fn store(&self, memory: &Memory) -> Result<MemoryId>;
    async fn retrieve(&self, id: &MemoryId) -> Result<Option<Memory>>;
    async fn delete(&self, id: &MemoryId) -> Result<bool>;
    async fn list(&self, filter: &PersistenceFilter) -> Result<Vec<Memory>>;
    async fn exists(&self, id: &MemoryId) -> Result<bool>;
}
```

## Choosing a Backend

| Criteria | Git Notes | PostgreSQL | Filesystem |
|----------|-----------|------------|------------|
| Setup | Minimal | Complex | Minimal |
| Sync | Git push/pull | Replication | Manual |
| Scale | Small-Medium | Large | Small |
| Team | Distributed | Centralized | Single |
| Offline | Yes | No | Yes |

## See Also

- [Index Layer](index.md) - Searchable index
- [Vector Layer](vector.md) - Vector embeddings
- [sync command](../cli/sync.md) - Git sync
