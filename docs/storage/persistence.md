# Persistence Layer

The persistence layer is the authoritative source of truth for all memories.

## Backends

| Backend | Description | Use Case |
|---------|-------------|----------|
| SQLite | Local database | Default, single-machine |
| PostgreSQL | Relational database | High-performance, enterprise |
| Filesystem | Local file storage | Development, offline |

## SQLite (Default)

Stores memories in a local SQLite database with ACID guarantees.

### Configuration

```yaml
storage:
  persistence: sqlite
  data_dir: ~/.local/share/subcog
```

### How It Works

Memories are stored in `~/.local/share/subcog/subcog.db` with:
- Full ACID compliance
- Faceted storage (project_id, branch, file_path)
- Tombstone support for soft deletes
- Automatic schema migrations

### Schema

```sql
CREATE TABLE memories (
    id TEXT PRIMARY KEY,
    namespace TEXT NOT NULL,
    domain TEXT NOT NULL,
    content TEXT NOT NULL,
    tags TEXT DEFAULT '[]',
    source TEXT,
    status TEXT DEFAULT 'active',
    project_id TEXT,
    branch TEXT,
    file_path TEXT,
    tombstoned_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_memories_namespace ON memories(namespace);
CREATE INDEX idx_memories_domain ON memories(domain);
CREATE INDEX idx_memories_project ON memories(project_id);
CREATE INDEX idx_memories_branch ON memories(branch);
CREATE INDEX idx_memories_project_branch ON memories(project_id, branch);
CREATE INDEX idx_memories_active ON memories(status) WHERE status = 'active';
```

### Advantages

- No external dependencies
- ACID transactions
- Works offline
- Fast local queries
- Automatic migrations

### Limitations

- Single-machine only
- No built-in replication
- Manual backup needed

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
    project_id VARCHAR(255),
    branch VARCHAR(255),
    file_path VARCHAR(1024),
    tombstoned_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_memories_namespace ON memories(namespace);
CREATE INDEX idx_memories_domain ON memories(domain);
CREATE INDEX idx_memories_tags ON memories USING GIN(tags);
CREATE INDEX idx_memories_project ON memories(project_id);
CREATE INDEX idx_memories_branch ON memories(branch);
CREATE INDEX idx_memories_project_branch ON memories(project_id, branch);
CREATE INDEX idx_memories_active ON memories(status) WHERE status = 'active';
CREATE INDEX idx_memories_created ON memories(created_at DESC);
```

### Advantages

- ACID transactions
- Concurrent access
- Complex queries
- Scalable
- Built-in replication

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

YAML format with frontmatter:

```yaml
---
id: dc58d23a35876f5a59426e81aaa81d796efa7fc1
namespace: decisions
tags: [database]
project_id: github.com/zircote/subcog
branch: main
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
- No ACID guarantees

---

## Faceted Storage

All persistence backends support faceted storage:

| Field | Type | Description |
|-------|------|-------------|
| `project_id` | String | Git remote URL (sanitized) |
| `branch` | String | Git branch name |
| `file_path` | String | Source file path |
| `tombstoned_at` | Timestamp | Soft delete marker |

### Auto-Detection

When capturing memories, facets are auto-detected from git context:

```rust
let context = GitContext::from_cwd()?;
// context.project_id = "github.com/zircote/subcog"
// context.branch = "feature/storage-simplification"
```

### Tombstoning

Memories are soft-deleted by setting `tombstoned_at`:

```rust
memory.status = MemoryStatus::Tombstoned;
memory.tombstoned_at = Some(Utc::now().timestamp() as u64);
```

Tombstoned memories are excluded from search by default but can be included:

```bash
subcog recall "old decision" --include-tombstoned
```

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

| Criteria | SQLite | PostgreSQL | Filesystem |
|----------|--------|------------|------------|
| Setup | Minimal | Complex | Minimal |
| ACID | Yes | Yes | No |
| Scale | Small-Medium | Large | Small |
| Team | Single | Centralized | Single |
| Offline | Yes | No | Yes |

## See Also

- [Index Layer](index.md) - Searchable index
- [Vector Layer](vector.md) - Vector embeddings
- [gc command](../cli/gc.md) - Garbage collection
