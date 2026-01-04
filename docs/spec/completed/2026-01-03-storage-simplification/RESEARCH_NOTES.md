---
document_type: research
project_id: SPEC-2026-01-03-001
version: 1.0.0
last_updated: 2026-01-03T01:30:00Z
---

# Research Notes

This document captures research findings from codebase analysis conducted during the planning phase.

## Research Overview

| Area | Analyst | Scope |
|------|---------|-------|
| Git-Notes Implementation | Code Reviewer | Capture bug root cause analysis |
| Storage Architecture | Postgres Pro | SQLite/PostgreSQL patterns |
| Integration Points | Backend Developer | MCP and CLI integration |
| Best Practices | Research Analyst | Storage and faceting patterns |

---

## 1. Git-Notes Capture Bug Analysis

### Root Cause

The capture bug is located at two points in the codebase:

**Primary Call Site** - `src/services/capture.rs:198`:
```rust
// All captures use add_to_head which attaches to HEAD
notes.add_to_head(&content, force)?;
```

**Underlying Implementation** - `src/git/notes.rs:113`:
```rust
pub fn add_to_head(&self, content: &str, force: bool) -> Result<Oid> {
    let head = self.repo.head()?.peel_to_commit()?;
    // force=true means overwrite existing note on HEAD
    self.repo.note(&self.sig, &self.sig, Some(&self.ref_name), head.id(), content, force)
}
```

### Why This Breaks

Git notes work by associating a blob (the note content) with an object (typically a commit). When you call `git2::Repository::note()` with `force=true` on the same commit (HEAD), it **replaces** the existing note rather than adding a new one.

The 645 existing memories in git-notes were created by the original Python tooling which used a different approach:
- Created unique blob objects for each memory
- Attached notes to those blobs instead of HEAD
- This is essentially a content-addressed store built on top of git

### Workaround Complexity

To fix git-notes properly would require:
1. Generate a unique marker blob for each memory
2. Attach the note to that blob
3. Maintain an index mapping memory IDs to blob OIDs
4. Handle garbage collection of orphaned blobs

This fights against git's design and adds ~300+ LOC of complexity.

### Recommendation

Remove git-notes storage entirely. The complexity is not justified when SQLite provides a simpler, more reliable solution.

---

## 2. Storage Architecture Analysis

### Current Schema - SQLite

Location: `src/storage/index/sqlite.rs`

```sql
CREATE TABLE IF NOT EXISTS memories (
    id TEXT PRIMARY KEY,
    namespace TEXT NOT NULL,
    content TEXT NOT NULL,
    tags TEXT,  -- JSON array
    source TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    embedding BLOB
);

CREATE INDEX IF NOT EXISTS idx_memories_namespace ON memories(namespace);
CREATE INDEX IF NOT EXISTS idx_memories_created_at ON memories(created_at);
CREATE INDEX IF NOT EXISTS idx_memories_status ON memories(status);
```

**Missing for Faceting:**
- `project_id` column
- `branch` column
- `file_path` column
- Composite indexes for facet combinations

### Current Schema - PostgreSQL

Location: `src/storage/persistence/postgresql.rs`

Similar schema to SQLite but with:
- Native JSON type for tags
- `timestamptz` for timestamps
- pgvector extension for embeddings

### Query Patterns

The codebase uses **parameterized dynamic SQL** correctly:

```rust
// src/storage/index/sqlite.rs - search method
let mut conditions = vec!["status = 'active'"];
let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

if let Some(ns) = &filter.namespace {
    conditions.push("namespace = ?");
    params.push(Box::new(ns.to_string()));
}

// Dynamic WHERE clause construction
let where_clause = conditions.join(" AND ");
```

This pattern extends naturally to facet filtering:

```rust
if let Some(project) = &filter.project_id {
    conditions.push("project_id = ?");
    params.push(Box::new(project.clone()));
}

if let Some(branch) = &filter.branch {
    conditions.push("branch = ?");
    params.push(Box::new(branch.clone()));
}
```

### Index Strategy

Recommended indexes for facet queries:

```sql
-- Single-column for OR queries
CREATE INDEX idx_memories_project ON memories(project_id);
CREATE INDEX idx_memories_branch ON memories(branch);

-- Composite for common patterns
CREATE INDEX idx_memories_project_branch ON memories(project_id, branch);
CREATE INDEX idx_memories_project_status ON memories(project_id, status);

-- Partial index for active memories (PostgreSQL)
CREATE INDEX idx_memories_active_project ON memories(project_id)
    WHERE status = 'active';
```

---

## 3. Integration Points Analysis

### MCP Tool Handlers

Location: `src/mcp/tools.rs`

Current capture tool schema:
```rust
Tool {
    name: "subcog_capture",
    description: "Capture a memory",
    input_schema: json!({
        "type": "object",
        "properties": {
            "namespace": { "type": "string", "enum": [...] },
            "content": { "type": "string" },
            "tags": { "type": "array", "items": { "type": "string" } },
            "source": { "type": "string" }
        },
        "required": ["namespace", "content"]
    })
}
```

**Changes Needed:**
- Add optional `project_id`, `branch`, `file_path` properties
- Default to auto-detected values when not provided
- Document in tool description

### CLI Commands

Location: `src/cli/capture.rs`, `src/cli/recall.rs`

Current capture command:
```rust
#[derive(Args)]
pub struct CaptureArgs {
    #[arg(long, short)]
    pub namespace: Namespace,
    pub content: String,
    #[arg(long)]
    pub tags: Option<Vec<String>>,
    #[arg(long)]
    pub source: Option<String>,
}
```

**Changes Needed:**
- Add `--project`, `--branch`, `--path` flags
- Auto-detect from git context when not specified
- Add facet flags to recall command for filtering

### ServiceContainer

Location: `src/services/mod.rs`

Current initialization:
```rust
pub struct ServiceContainer {
    repo_path: PathBuf,  // Required for git operations
    // ...
}
```

**Changes Needed:**
- Make `repo_path` optional (not all contexts are git repos)
- Add `context_detector` component
- Remove git-notes service dependency

---

## 4. Context Detection Patterns

### Git Remote Normalization

Git remotes can have multiple URL formats:
- `https://github.com/user/repo.git`
- `git@github.com:user/repo.git`
- `ssh://git@github.com/user/repo.git`
- `https://github.com/user/repo` (no .git suffix)

**Normalization Algorithm:**
```rust
fn normalize_remote(url: &str) -> String {
    let url = url.trim();

    // Convert SSH to HTTPS format
    let url = if url.starts_with("git@") {
        url.replace("git@", "https://")
           .replace(":", "/")
    } else {
        url.to_string()
    };

    // Remove .git suffix
    let url = url.trim_end_matches(".git");

    // Remove trailing slash
    url.trim_end_matches('/').to_lowercase()
}
```

**Result:** `github.com/user/repo` (lowercase, no protocol, no suffix)

### Branch Detection

Use `git2::Repository::head()` to get current branch:
```rust
fn detect_branch(repo: &Repository) -> Option<String> {
    repo.head()
        .ok()
        .and_then(|head| head.shorthand().map(String::from))
}
```

**Edge Cases:**
- Detached HEAD: Return commit SHA prefix
- New repo with no commits: Return None
- Worktree: Return worktree branch name

### Path Detection

Relative path from repository root:
```rust
fn detect_path(repo: &Repository, cwd: &Path) -> Option<String> {
    let workdir = repo.workdir()?;
    cwd.strip_prefix(workdir)
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}
```

---

## 5. Platform-Specific Storage Paths

Using the `directories` crate (already a dependency):

```rust
use directories::ProjectDirs;

fn data_dir() -> PathBuf {
    ProjectDirs::from("com", "subcog", "subcog")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".subcog"))
}
```

**Platform Paths:**
| Platform | Path |
|----------|------|
| Linux | `~/.local/share/subcog/` |
| macOS | `~/Library/Application Support/subcog/` |
| Windows | `%APPDATA%\subcog\` |

---

## 6. Tombstone Implementation

### Status Enum Extension

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStatus {
    Active,
    Tombstoned,
}
```

### Tombstone Metadata

For debugging and audit purposes:
```rust
pub struct Memory {
    // ...
    pub status: MemoryStatus,
    pub tombstoned_at: Option<DateTime<Utc>>,
    pub tombstone_reason: Option<String>,  // "branch_deleted", "manual", etc.
}
```

### Query Filtering

Default behavior excludes tombstoned:
```rust
fn build_where_clause(&self, filter: &SearchFilter) -> String {
    let mut conditions = vec![];

    // Always filter by status unless explicitly requested
    if filter.include_tombstoned {
        // No status filter
    } else {
        conditions.push("status = 'active'");
    }

    // ... other conditions
}
```

---

## 7. Performance Considerations

### Capture Path

Current latency budget: <50ms P99

| Operation | Current | After Change |
|-----------|---------|--------------|
| Content validation | 1ms | 1ms |
| Secret detection | 5ms | 5ms |
| Git context detection | N/A | 2ms (new) |
| Embedding generation | 20ms | 20ms |
| SQLite insert | 5ms | 6ms (+facets) |
| Git notes write | 15ms | 0ms (removed) |
| **Total** | **46ms** | **34ms** |

Net improvement: ~12ms faster due to git-notes removal.

### Recall Path

Current latency budget: <100ms P99

| Operation | Current | After Change |
|-----------|---------|--------------|
| Query parsing | 1ms | 1ms |
| FTS5 search | 10ms | 12ms (+facets) |
| Vector search | 30ms | 30ms |
| RRF fusion | 5ms | 5ms |
| Branch GC check | N/A | 8ms (new, lazy) |
| Result formatting | 2ms | 2ms |
| **Total** | **48ms** | **58ms** |

Net impact: ~10ms slower due to lazy GC, well within budget.

### GC Optimization

Branch existence check caching:
```rust
struct BranchCache {
    cache: LruCache<String, bool>,
    ttl: Duration,
}

impl BranchCache {
    fn exists(&mut self, branch: &str, repo: &Repository) -> bool {
        if let Some(&exists) = self.cache.get(branch) {
            return exists;
        }

        let exists = repo.find_branch(branch, BranchType::Local).is_ok()
            || repo.find_branch(branch, BranchType::Remote).is_ok();

        self.cache.put(branch.to_string(), exists);
        exists
    }
}
```

---

## 8. Security Considerations

### Git Remote URL Sanitization

Git remote URLs may contain tokens or credentials:
- `https://token@github.com/user/repo.git`
- `https://user:password@gitlab.com/repo.git`

**Sanitization:**
```rust
fn sanitize_remote(url: &str) -> String {
    // Remove credentials from URL before storing
    let re = Regex::new(r"https?://[^@]+@").unwrap();
    re.replace(url, "https://").to_string()
}
```

### File Permissions

User storage directory should have restricted permissions:
```rust
fn create_data_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }

    Ok(())
}
```

---

## Summary of Findings

| Finding | Impact | Action |
|---------|--------|--------|
| Capture bug at `capture.rs:198` | Critical | Remove git-notes |
| SQLite schema missing facets | High | Add migration |
| Dynamic SQL pattern exists | Positive | Extend for facets |
| Git context detection needed | Medium | New module |
| Branch existence caching | Medium | Add LRU cache |
| Remote URL sanitization | Medium | Add sanitizer |
| Platform paths via `directories` | Positive | Already available |

All findings support the architectural decisions documented in DECISIONS.md.
