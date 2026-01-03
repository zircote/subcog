# Storage Architecture Research for Issue #43

This document summarizes best practices research for the storage architecture changes proposed in GitHub Issue #43.

---

## Table of Contents

1. [SQLite Faceted Queries with Optional Filters](#1-sqlite-faceted-queries-with-optional-filters)
2. [PostgreSQL Faceted Search and Indexing](#2-postgresql-faceted-search-and-indexing)
3. [Tombstone Patterns and Garbage Collection](#3-tombstone-patterns-and-garbage-collection)
4. [Git Context Detection](#4-git-context-detection)
5. [Platform-Specific Paths with directories Crate](#5-platform-specific-paths-with-directories-crate)
6. [Recommendations for Subcog](#6-recommendations-for-subcog)

---

## 1. SQLite Faceted Queries with Optional Filters

### Problem Statement

Subcog needs to support queries with optional project, branch, and path filters. Users may specify any combination of these filters (all, some, or none).

### Best Practices

#### 1.1 Dynamic WHERE Clause Pattern (Recommended)

The most robust approach for optional filters is **dynamic SQL generation** with parameterized queries:

```rust
fn build_filter_clause(filter: &SearchFilter) -> (String, Vec<String>) {
    let mut conditions = Vec::new();
    let mut params = Vec::new();
    let mut param_idx = 1;

    // Only add conditions for non-empty filter values
    if let Some(project) = &filter.project {
        conditions.push(format!("project = ?{}", param_idx));
        params.push(project.clone());
        param_idx += 1;
    }

    if let Some(branch) = &filter.branch {
        conditions.push(format!("branch = ?{}", param_idx));
        params.push(branch.clone());
        param_idx += 1;
    }

    if let Some(path_pattern) = &filter.path_pattern {
        conditions.push(format!("path LIKE ?{}", param_idx));
        params.push(format!("{}%", path_pattern));
        param_idx += 1;
    }

    let clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    (clause, params)
}
```

**Why this approach:**
- Avoids the "smart logic" anti-pattern that harms query plan optimization
- Each query variant can be optimized by SQLite's query planner
- Parameterized queries prevent SQL injection

#### 1.2 Anti-Pattern: COALESCE/OR NULL Trick

Avoid this pattern for optional filters:

```sql
-- ANTI-PATTERN: Do not use
SELECT * FROM memories
WHERE (project = :project OR :project IS NULL)
  AND (branch = :branch OR :branch IS NULL)
```

This pattern prevents SQLite from using indexes effectively because the query planner cannot determine which filters will be active at runtime.

**Source:** [Use The Index, Luke - Conditional WHERE clauses](https://use-the-index-luke.com/sql/where-clause/obfuscation/smart-logic)

#### 1.3 Index Strategy for Faceted Queries

Create indexes that match common query patterns:

```sql
-- Single-column indexes for independent filtering
CREATE INDEX idx_memories_project ON memories(project);
CREATE INDEX idx_memories_branch ON memories(branch);
CREATE INDEX idx_memories_path ON memories(path);

-- Composite index for common filter combinations
CREATE INDEX idx_memories_project_branch ON memories(project, branch);

-- Time-based index for recency queries
CREATE INDEX idx_memories_created_at ON memories(created_at DESC);
```

**Source:** [SQLite FTS5 Extension](https://sqlite.org/fts5.html)

#### 1.4 FTS5 Integration with Metadata Filters

The current Subcog implementation correctly separates FTS content from metadata:

```sql
-- FTS5 table for full-text search
CREATE VIRTUAL TABLE memories_fts USING fts5(id, content, tags);

-- Metadata table with filterable columns
CREATE TABLE memories (
    id TEXT PRIMARY KEY,
    namespace TEXT NOT NULL,
    domain TEXT,
    project TEXT,        -- NEW: project filter
    branch TEXT,         -- NEW: branch filter
    path TEXT,           -- NEW: path filter
    status TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

-- Query combining FTS and metadata filters
SELECT f.id, bm25(memories_fts) as score
FROM memories_fts f
JOIN memories m ON f.id = m.id
WHERE memories_fts MATCH ?1
  AND m.project = ?2
  AND m.branch = ?3
ORDER BY score
LIMIT ?4;
```

**Source:** [Datasette Facets](https://docs.datasette.io/en/stable/facets.html)

---

## 2. PostgreSQL Faceted Search and Indexing

### 2.1 GIN Index for Full-Text Search

PostgreSQL's GIN (Generalized Inverted Index) is ideal for full-text search:

```sql
-- Create tsvector column with weighted terms
ALTER TABLE memories ADD COLUMN search_vector TSVECTOR
    GENERATED ALWAYS AS (
        setweight(to_tsvector('english', coalesce(content, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(array_to_string(tags, ' '), '')), 'B')
    ) STORED;

-- GIN index on the tsvector
CREATE INDEX memories_search_idx ON memories USING GIN (search_vector);
```

**Performance characteristics:**
- GIN index lookups are ~3x faster than GiST
- GIN indexes take ~3x longer to build than GiST
- Best for static data; use fast-update mode for dynamic data

**Source:** [PostgreSQL GIN Index Documentation](https://www.postgresql.org/docs/current/gin.html)

### 2.2 Partial Indexes for Faceted Metadata

Partial indexes are excellent for filtering on common status values:

```sql
-- Index only active memories (common query pattern)
CREATE INDEX idx_memories_active
ON memories (project, branch, created_at DESC)
WHERE status = 'active';

-- Index only non-deleted memories
CREATE INDEX idx_memories_not_deleted
ON memories (namespace, created_at DESC)
WHERE status != 'deleted';

-- Index for pending sync items
CREATE INDEX idx_memories_pending_sync
ON memories (updated_at)
WHERE sync_status = 'pending';
```

**Benefits:**
- Dramatically smaller index size (up to 10,000x in some cases)
- Faster writes because index is not updated for excluded rows
- Better cache utilization

**Important:** The query predicate must exactly match the index predicate for PostgreSQL to use it.

**Source:** [PostgreSQL Partial Indexes](https://www.postgresql.org/docs/current/indexes-partial.html), [Heap - Speeding Up PostgreSQL](https://www.heap.io/blog/speeding-up-postgresql-queries-with-partial-indexes)

### 2.3 Combined B-tree and GIN Strategy

For queries that filter on both text and metadata:

```sql
-- Option 1: Separate indexes (PostgreSQL may use BitmapAnd)
CREATE INDEX idx_memories_project ON memories (project);
CREATE INDEX memories_search_idx ON memories USING GIN (search_vector);

-- Option 2: Include filter columns in GIN (if using JSONB)
CREATE INDEX idx_memories_metadata ON memories USING GIN (metadata jsonb_path_ops);
```

**Source:** [pganalyze - Understanding GIN Indexes](https://pganalyze.com/blog/gin-index)

### 2.4 Faceted Search Query Pattern

```sql
-- Get memories with facet counts
WITH filtered AS (
    SELECT * FROM memories
    WHERE search_vector @@ websearch_to_tsquery('english', $1)
      AND ($2::text IS NULL OR project = $2)
      AND ($3::text IS NULL OR branch = $3)
),
facets AS (
    SELECT
        'project' as facet_type,
        project as facet_value,
        count(*) as count
    FROM filtered
    GROUP BY project
    UNION ALL
    SELECT
        'branch' as facet_type,
        branch as facet_value,
        count(*) as count
    FROM filtered
    GROUP BY branch
)
SELECT * FROM facets ORDER BY facet_type, count DESC;
```

**Source:** [Xata - PostgreSQL Full-Text Search Engine](https://xata.io/blog/postgres-full-text-search-engine)

---

## 3. Tombstone Patterns and Garbage Collection

### 3.1 Soft Delete Implementation

#### Schema Design

```sql
-- Add soft delete columns
ALTER TABLE memories ADD COLUMN deleted_at TIMESTAMP NULL;
ALTER TABLE memories ADD COLUMN deleted_by TEXT NULL;
ALTER TABLE memories ADD COLUMN delete_reason TEXT NULL;

-- Partial index excluding deleted records (for normal queries)
CREATE INDEX idx_memories_active ON memories (namespace, created_at DESC)
WHERE deleted_at IS NULL;
```

#### Query Patterns

```rust
// Normal queries automatically exclude deleted
fn search(&self, query: &str, filter: &SearchFilter) -> Result<Vec<Memory>> {
    // WHERE deleted_at IS NULL is implicit via partial index
    // ...
}

// Explicit include deleted for admin/recovery
fn search_including_deleted(&self, query: &str) -> Result<Vec<Memory>> {
    // Use full table scan or separate index
    // ...
}
```

**Source:** [Brandur - Soft Deletion](https://brandur.org/soft-deletion), [Jmix - To Delete or Soft Delete](https://www.jmix.io/blog/to-delete-or-to-soft-delete-that-is-the-question/)

### 3.2 Retention Policy and Scheduled Purge

#### Configuration

```toml
[garbage_collection]
# Soft-deleted items are purgeable after this period
retention_days = 90

# Run garbage collection at this interval
purge_interval = "24h"

# Maximum items to purge per run (to limit impact)
purge_batch_size = 1000

# Purge schedule (cron format)
purge_schedule = "0 4 * * *"  # 4:00 AM daily
```

#### Implementation Pattern

```rust
/// Garbage collection service
pub struct GarbageCollector {
    retention_period: Duration,
    batch_size: usize,
}

impl GarbageCollector {
    /// Purge soft-deleted records older than retention period
    pub async fn purge_expired(&self, storage: &dyn PersistenceBackend) -> Result<PurgeResult> {
        let cutoff = Utc::now() - self.retention_period;

        // Query for purgeable records
        let to_purge = storage.list_deleted_before(cutoff, self.batch_size).await?;

        let mut purged = 0;
        let mut errors = 0;

        for memory_id in to_purge {
            match storage.hard_delete(&memory_id).await {
                Ok(()) => {
                    purged += 1;
                    // Also remove from index and vector layers
                    self.index.remove(&memory_id).await?;
                    self.vector.remove(&memory_id).await?;
                }
                Err(e) => {
                    tracing::warn!(id = %memory_id, error = %e, "Failed to purge");
                    errors += 1;
                }
            }
        }

        Ok(PurgeResult { purged, errors, remaining: to_purge.len() > purged })
    }
}
```

**Source:** [OpenStack Nova - Soft Delete and Shadow Tables](https://docs.openstack.org/nova/latest/admin/soft-delete-shadow-tables.html), [Azure Key Vault Soft Delete](https://learn.microsoft.com/en-us/azure/key-vault/general/soft-delete-overview)

### 3.3 Tombstone Considerations for Distributed Systems

If Subcog syncs across multiple machines:

1. **Tombstone propagation**: Deleted records must sync before being purged
2. **Grace period**: Retention period must exceed maximum sync delay
3. **Conflict resolution**: "Delete wins" or "last-write wins" policy needed

```rust
/// Check if a tombstone is safe to purge
fn is_safe_to_purge(memory: &Memory, last_sync: DateTime<Utc>) -> bool {
    match memory.deleted_at {
        Some(deleted_at) => {
            // Only purge if:
            // 1. Deleted before last successful sync (tombstone propagated)
            // 2. AND retention period has passed
            deleted_at < last_sync &&
            deleted_at + RETENTION_PERIOD < Utc::now()
        }
        None => false, // Not deleted
    }
}
```

**Source:** [ScyllaDB - Tombstone Garbage Collection](https://www.scylladb.com/2022/06/30/preventing-data-resurrection-with-repair-based-tombstone-collection/)

---

## 4. Git Context Detection

### 4.1 Repository Discovery with git2

```rust
use git2::Repository;
use std::path::{Path, PathBuf};

/// Git context for the current working directory
pub struct GitContext {
    /// Repository root path
    pub repo_root: PathBuf,
    /// Remote URL (e.g., git@github.com:user/repo.git)
    pub remote_url: Option<String>,
    /// Current branch name
    pub branch: Option<String>,
    /// Repository name derived from remote or directory
    pub repo_name: String,
}

impl GitContext {
    /// Discover git context from a path
    pub fn discover(path: impl AsRef<Path>) -> Result<Self> {
        // Search upward for .git directory
        let repo = Repository::discover(path.as_ref())?;

        let repo_root = repo.workdir()
            .ok_or_else(|| Error::BareRepository)?
            .to_path_buf();

        // Get remote URL (prefer "origin")
        let remote_url = repo.find_remote("origin")
            .ok()
            .and_then(|r| r.url().map(String::from));

        // Get current branch
        let branch = repo.head()
            .ok()
            .and_then(|head| head.shorthand().map(String::from));

        // Derive repo name from remote or directory
        let repo_name = Self::extract_repo_name(&remote_url, &repo_root);

        Ok(Self {
            repo_root,
            remote_url,
            branch,
            repo_name,
        })
    }

    /// Get relative path from repo root
    pub fn relative_path(&self, absolute_path: &Path) -> Option<PathBuf> {
        absolute_path.strip_prefix(&self.repo_root)
            .ok()
            .map(PathBuf::from)
    }

    /// Extract repository name from remote URL or directory
    fn extract_repo_name(remote_url: &Option<String>, repo_root: &Path) -> String {
        if let Some(url) = remote_url {
            // Parse git@github.com:user/repo.git or https://github.com/user/repo.git
            if let Some(name) = Self::parse_repo_name_from_url(url) {
                return name;
            }
        }

        // Fall back to directory name
        repo_root.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    fn parse_repo_name_from_url(url: &str) -> Option<String> {
        // Handle SSH format: git@github.com:user/repo.git
        if let Some(path) = url.strip_prefix("git@") {
            if let Some(repo_part) = path.split(':').nth(1) {
                return Some(repo_part.trim_end_matches(".git").to_string());
            }
        }

        // Handle HTTPS format: https://github.com/user/repo.git
        if url.starts_with("https://") || url.starts_with("http://") {
            if let Ok(parsed) = reqwest::Url::parse(url) {
                let path = parsed.path().trim_start_matches('/');
                return Some(path.trim_end_matches(".git").to_string());
            }
        }

        None
    }
}
```

**Source:** [git2 Repository Documentation](https://docs.rs/git2/latest/git2/struct.Repository.html), [24 Days of Rust - git2](https://zsiciarz.github.io/24daysofrust/book/vol2/day16.html)

### 4.2 Handling Edge Cases

```rust
impl GitContext {
    /// Check if we're inside a git worktree
    pub fn is_worktree(&self) -> bool {
        self.repo_root.join(".git").is_file()  // Worktrees have .git as file, not directory
    }

    /// Get the main repository path (for worktrees)
    pub fn main_repo_path(&self) -> Result<PathBuf> {
        let repo = Repository::open(&self.repo_root)?;

        if repo.is_worktree() {
            // Read the gitdir from .git file
            let gitdir = repo.path();  // Points to .git/worktrees/<name>
            gitdir.parent()
                .and_then(|p| p.parent())  // Go up from worktrees/<name>
                .map(PathBuf::from)
                .ok_or(Error::InvalidWorktree)
        } else {
            Ok(self.repo_root.clone())
        }
    }
}
```

### 4.3 Branch and Remote Detection

```rust
/// Get detailed remote information
pub fn get_remote_info(repo: &Repository) -> RemoteInfo {
    let remotes = repo.remotes()
        .map(|r| r.iter().flatten().map(String::from).collect::<Vec<_>>())
        .unwrap_or_default();

    // Get upstream tracking branch
    let upstream = repo.head()
        .ok()
        .and_then(|head| head.resolve().ok())
        .and_then(|resolved| {
            let branch = Branch::wrap(resolved);
            branch.upstream().ok()
        })
        .and_then(|upstream| upstream.name().ok().flatten().map(String::from));

    RemoteInfo {
        remotes,
        upstream_branch: upstream,
        default_remote: remotes.first().cloned(),
    }
}
```

**Source:** [git2-rs Remote struct](https://docs.rs/git2/latest/git2/struct.Remote.html)

---

## 5. Platform-Specific Paths with directories Crate

### 5.1 Using ProjectDirs for Application Data

```rust
use directories::ProjectDirs;

/// Subcog directory configuration
pub struct SubcogDirs {
    /// Configuration directory (subcog.toml)
    pub config_dir: PathBuf,
    /// Data directory (SQLite, usearch index)
    pub data_dir: PathBuf,
    /// Cache directory (temporary files, model cache)
    pub cache_dir: PathBuf,
}

impl SubcogDirs {
    /// Create directory paths for Subcog
    pub fn new() -> Option<Self> {
        // qualifier, organization, application
        let proj_dirs = ProjectDirs::from("io", "subcog", "subcog")?;

        Some(Self {
            config_dir: proj_dirs.config_dir().to_path_buf(),
            data_dir: proj_dirs.data_dir().to_path_buf(),
            cache_dir: proj_dirs.cache_dir().to_path_buf(),
        })
    }

    /// Get path to SQLite database
    pub fn index_db_path(&self) -> PathBuf {
        self.data_dir.join("index.db")
    }

    /// Get path to usearch vector index
    pub fn vector_index_path(&self) -> PathBuf {
        self.data_dir.join("vectors.usearch")
    }

    /// Get path to embedding model cache
    pub fn model_cache_path(&self) -> PathBuf {
        self.cache_dir.join("models")
    }

    /// Ensure all directories exist
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.cache_dir)?;
        Ok(())
    }
}
```

### 5.2 Platform-Specific Paths

| Directory | Linux | macOS | Windows |
|-----------|-------|-------|---------|
| `config_dir` | `~/.config/subcog/` | `~/Library/Application Support/io.subcog.subcog/` | `C:\Users\<user>\AppData\Roaming\subcog\subcog\config\` |
| `data_dir` | `~/.local/share/subcog/` | `~/Library/Application Support/io.subcog.subcog/` | `C:\Users\<user>\AppData\Roaming\subcog\subcog\data\` |
| `cache_dir` | `~/.cache/subcog/` | `~/Library/Caches/io.subcog.subcog/` | `C:\Users\<user>\AppData\Local\subcog\subcog\cache\` |

**Source:** [directories crate documentation](https://docs.rs/directories/latest/directories/), [crates.io directories](https://crates.io/crates/directories)

### 5.3 XDG Compliance on Linux

The `directories` crate respects XDG environment variables:

```bash
# Override default locations via environment
export XDG_CONFIG_HOME=~/.config
export XDG_DATA_HOME=~/.local/share
export XDG_CACHE_HOME=~/.cache
```

### 5.4 Per-Project Data Storage

For project-specific data (not user-level):

```rust
/// Get project-specific data directory within a git repository
pub fn project_data_dir(git_context: &GitContext) -> PathBuf {
    git_context.repo_root.join(".subcog")
}

/// Get user-level data directory (for cross-project data)
pub fn user_data_dir() -> Option<PathBuf> {
    SubcogDirs::new().map(|d| d.data_dir)
}

/// Determine storage location based on domain
pub fn storage_path_for_domain(domain: &Domain, git_context: Option<&GitContext>) -> PathBuf {
    match domain.scope() {
        Scope::Project => {
            git_context
                .map(|ctx| project_data_dir(ctx))
                .unwrap_or_else(|| user_data_dir().expect("No home directory"))
        }
        Scope::User => user_data_dir().expect("No home directory"),
        Scope::Organization => {
            // Organization data stored in user dir with org subdirectory
            user_data_dir()
                .expect("No home directory")
                .join("orgs")
                .join(domain.organization.as_deref().unwrap_or("default"))
        }
    }
}
```

---

## 6. Recommendations for Subcog

### 6.1 SQLite Faceted Query Implementation

**Recommendation:** Extend the current `build_filter_clause_numbered` method to support project, branch, and path filters:

```rust
// Add to SearchFilter struct
pub struct SearchFilter {
    // Existing fields...
    pub project: Option<String>,
    pub branch: Option<String>,
    pub path_pattern: Option<String>,  // Glob pattern like "src/**/*.rs"
}

// Update build_filter_clause_numbered
fn build_filter_clause_numbered(&self, filter: &SearchFilter, start_param: usize)
    -> (String, Vec<String>, usize)
{
    // ... existing implementation ...

    // Add project filter
    if let Some(project) = &filter.project {
        conditions.push(format!("m.project = ?{}", param_idx));
        params.push(project.clone());
        param_idx += 1;
    }

    // Add branch filter
    if let Some(branch) = &filter.branch {
        conditions.push(format!("m.branch = ?{}", param_idx));
        params.push(branch.clone());
        param_idx += 1;
    }

    // Add path pattern filter (glob to LIKE conversion)
    if let Some(pattern) = &filter.path_pattern {
        let sql_pattern = pattern.replace("**", "%").replace("*", "%").replace("?", "_");
        conditions.push(format!("m.path LIKE ?{} ESCAPE '\\'", param_idx));
        params.push(sql_pattern);
        param_idx += 1;
    }

    // ...
}
```

### 6.2 PostgreSQL Indexing Strategy

**Recommendation:** Add partial indexes for common query patterns:

```sql
-- Migration: Add project/branch/path columns and indexes
ALTER TABLE memories_index ADD COLUMN project TEXT;
ALTER TABLE memories_index ADD COLUMN branch TEXT;
ALTER TABLE memories_index ADD COLUMN path TEXT;

-- Partial index for active memories (most common query)
CREATE INDEX idx_active_memories ON memories_index (project, branch, created_at DESC)
WHERE status = 'active';

-- GIN index already exists for full-text search
-- Composite filter with full-text will use BitmapAnd
```

### 6.3 Soft Delete with Scheduled Purge

**Recommendation:** Implement soft delete with configurable retention:

1. Add `deleted_at` column to memories table
2. Create partial index excluding deleted records
3. Add `GarbageCollector` service with scheduled purge
4. Default retention: 90 days
5. Run purge during `subcog sync` or via background task

### 6.4 Git Context Detection

**Recommendation:** Create a `GitContext` service that:

1. Uses `Repository::discover()` to find repo from cwd
2. Caches context for session duration
3. Provides `relative_path()` for file references
4. Falls back gracefully when not in a git repo

### 6.5 Platform Paths

**Recommendation:** Use `directories` crate with this structure:

- **User-level index:** `data_dir/index.db`
- **User-level vectors:** `data_dir/vectors.usearch`
- **Project-level data:** `{repo_root}/.subcog/` (git-ignored)
- **Model cache:** `cache_dir/models/`

---

## Sources

### SQLite
- [SQLite FTS5 Extension](https://sqlite.org/fts5.html)
- [Use The Index, Luke - Conditional WHERE clauses](https://use-the-index-luke.com/sql/where-clause/obfuscation/smart-logic)
- [Datasette Facets](https://docs.datasette.io/en/stable/facets.html)
- [SQLite COALESCE Function](https://www.sqlitetutorial.net/sqlite-functions/sqlite-coalesce/)

### PostgreSQL
- [PostgreSQL GIN Index Documentation](https://www.postgresql.org/docs/current/gin.html)
- [PostgreSQL Partial Indexes](https://www.postgresql.org/docs/current/indexes-partial.html)
- [pganalyze - Understanding GIN Indexes](https://pganalyze.com/blog/gin-index)
- [Heap - Speeding Up PostgreSQL](https://www.heap.io/blog/speeding-up-postgresql-queries-with-partial-indexes)
- [Xata - PostgreSQL Full-Text Search Engine](https://xata.io/blog/postgres-full-text-search-engine)

### Soft Delete / Tombstones
- [Brandur - Soft Deletion](https://brandur.org/soft-deletion)
- [Jmix - To Delete or Soft Delete](https://www.jmix.io/blog/to-delete-or-to-soft-delete-that-is-the-question/)
- [OpenStack Nova - Soft Delete and Shadow Tables](https://docs.openstack.org/nova/latest/admin/soft-delete-shadow-tables.html)
- [Azure Key Vault Soft Delete](https://learn.microsoft.com/en-us/azure/key-vault/general/soft-delete-overview)
- [ScyllaDB - Tombstone Garbage Collection](https://www.scylladb.com/2022/06/30/preventing-data-resurrection-with-repair-based-tombstone-collection/)
- [Cultured Systems - Avoiding the Soft Delete Anti-Pattern](https://www.cultured.systems/2024/04/24/Soft-delete/)

### Git / git2
- [git2 Repository Documentation](https://docs.rs/git2/latest/git2/struct.Repository.html)
- [git2 Remote Documentation](https://docs.rs/git2/latest/git2/struct.Remote.html)
- [24 Days of Rust - git2](https://zsiciarz.github.io/24daysofrust/book/vol2/day16.html)
- [Git Notes Documentation](https://tylercipriani.com/blog/2022/11/19/git-notes-gits-coolest-most-unloved-feature/)
- [Git Namespaces Documentation](https://git-scm.com/docs/gitnamespaces)

### Platform Directories
- [directories crate - docs.rs](https://docs.rs/directories/latest/directories/)
- [directories crate - crates.io](https://crates.io/crates/directories)
- [dirs crate - crates.io](https://crates.io/crates/dirs)

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2026-01-03 | Claude Opus 4.5 | Initial research for Issue #43 |
