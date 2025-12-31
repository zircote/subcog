# Prompt Template Storage

Prompt templates are stored in domain-scoped locations using the same storage backends as memories.

## Storage Backends

| Backend | Location | Use Case |
|---------|----------|----------|
| Git Notes | `refs/notes/_prompts` | Default, distributed |
| SQLite | `~/.subcog/prompts.db` | Local, fast |
| Filesystem | `~/.subcog/prompts/` | Simple, debuggable |
| PostgreSQL | `prompts` table | Enterprise |

## Domain Scoping

Templates are scoped to domains:

| Domain | Scope | Priority |
|--------|-------|----------|
| `project` | Current repository | Highest |
| `user` | User-wide | Medium |
| `org` | Organization | Lowest |

### Resolution Order

When getting a prompt, domains are searched in order:

```
Project → User → Org
```

First match wins.

## Git Notes Storage

Default storage using Git notes.

### Reference

```
refs/notes/_prompts
```

### Structure

```
refs/notes/_prompts/
├── project/
│   └── zircote-subcog/
│       ├── code-review
│       └── security-audit
├── user/
│   └── my-template
└── org/
    └── zircote/
        └── team-review
```

### Note Format

```yaml
---
name: code-review
description: Code review template
domain: project
tags: [review, quality]
created_at: 2024-01-15T10:30:00Z
updated_at: 2024-01-15T10:30:00Z
---
content: |
  Review {{file}} for {{issue_type}} issues
variables:
  - name: file
    required: true
  - name: issue_type
    default: general
```

### Sync

Prompt templates sync with memories:

```bash
subcog sync
# Syncs refs/notes/subcog AND refs/notes/_prompts
```

## SQLite Storage

Embedded database for local access.

### Configuration

```yaml
storage:
  prompts: sqlite
  prompts_sqlite_path: ~/.subcog/prompts.db
```

### Schema

```sql
CREATE TABLE prompts (
    name TEXT NOT NULL,
    domain TEXT NOT NULL,
    description TEXT,
    content TEXT NOT NULL,
    variables TEXT,  -- JSON array
    tags TEXT,       -- JSON array
    created_at INTEGER,
    updated_at INTEGER,
    PRIMARY KEY (name, domain)
);

CREATE INDEX idx_prompts_domain ON prompts(domain);
CREATE INDEX idx_prompts_tags ON prompts(tags);
```

## Filesystem Storage

Simple file-based storage.

### Configuration

```yaml
storage:
  prompts: filesystem
  prompts_path: ~/.subcog/prompts
```

### Structure

```
~/.subcog/prompts/
├── project/
│   └── zircote-subcog/
│       ├── code-review.yaml
│       └── security-audit.yaml
├── user/
│   └── my-template.yaml
└── org/
    └── zircote/
        └── team-review.yaml
```

### File Format

Same YAML format as Git notes content.

## PostgreSQL Storage

Enterprise-grade storage.

### Configuration

```yaml
storage:
  prompts: postgresql

postgresql:
  host: localhost
  database: subcog
```

### Schema

```sql
CREATE TABLE prompts (
    id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    domain VARCHAR(50) NOT NULL,
    description TEXT,
    content TEXT NOT NULL,
    variables JSONB DEFAULT '[]',
    tags TEXT[] DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (name, domain)
);

CREATE INDEX idx_prompts_domain ON prompts(domain);
CREATE INDEX idx_prompts_tags ON prompts USING GIN(tags);
```

## Domain Operations

### Save to Domain

```bash
# Project (default)
subcog prompt save template --content "..."

# User
subcog prompt save template --content "..." --domain user

# Org
subcog prompt save template --content "..." --domain org
```

### List by Domain

```bash
# All domains
subcog prompt list

# Specific domain
subcog prompt list --domain user
```

### Get with Cascade

```bash
# Searches: project → user → org
subcog prompt get template-name

# Specific domain only
subcog prompt get template-name --domain user
```

### Delete (Requires Domain)

```bash
# Must specify domain for safety
subcog prompt delete template-name --domain project
```

## MCP Resources

Access templates via MCP:

| Resource | Description |
|----------|-------------|
| `subcog://_prompts` | All prompts |
| `subcog://project/_prompts` | Project prompts |
| `subcog://user/_prompts` | User prompts |
| `subcog://org/_prompts` | Org prompts |
| `subcog://project/_prompts/{name}` | Specific prompt |

## Sharing Templates

### Share Across Domains

```bash
# Copy from project to org
subcog prompt share code-review --from project --to org
```

### Export/Import

```bash
# Export from one system
subcog prompt export code-review -o code-review.yaml

# Import on another system
subcog prompt save --from-file code-review.yaml --domain project
```

## Best Practices

1. **Use project for repo-specific templates**
2. **Use user for personal preferences**
3. **Use org for team standards**
4. **Sync regularly** to share across machines
5. **Export before major changes** for backup

## See Also

- [Formats](formats.md) - Template file formats
- [Domains](../storage/domains.md) - Domain scoping
- [sync command](../cli/sync.md) - Syncing templates
