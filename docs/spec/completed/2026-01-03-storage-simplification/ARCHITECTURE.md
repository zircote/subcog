---
document_type: architecture
project_id: SPEC-2026-01-03-001
version: 1.0.0
last_updated: 2026-01-03T01:15:00Z
status: draft
---

# Storage Architecture Simplification - Technical Architecture

## System Overview

This architecture document describes the simplified storage model for subcog, consolidating from three storage tiers (org/project/user) to a single user-level tier with project/branch/path faceting.

### Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ CURRENT ARCHITECTURE │
├─────────────────────────────────────────────────────────────────────────────┤
│ │
│ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ │
│ │ org/ │ │ project/ │ │ user/ │ │
│ │ (unused) │ │ (BROKEN) │ │ (works) │ │
│ └─────────────┘ └─────────────┘ └─────────────┘ │
│ │ │ │ │
│ ▼ ▼ ▼ │
│ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ │
│ │ PostgreSQL │ │ Git Notes │ │ SQLite │ │
│ │ (config) │ │ (overwrites)│ │ (works) │ │
│ └─────────────┘ └─────────────┘ └─────────────┘ │
│ │
└─────────────────────────────────────────────────────────────────────────────┘


┌─────────────────────────────────────────────────────────────────────────────┐
│ PROPOSED ARCHITECTURE │
├─────────────────────────────────────────────────────────────────────────────┤
│ │
│ ┌─────────────────────────────────────┐ │
│ │ User-Level Storage │ │
│ │ ~/.local/share/subcog/ (Linux) │ │
│ │ ~/Library/App.../subcog/ (macOS) │ │
│ └─────────────────────────────────────┘ │
│ │ │
│ ┌─────────────────────────┼─────────────────────────┐ │
│ ▼ ▼ ▼ │
│ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ │
│ │ memories.db │ │ index.db │ │ vectors.idx │ │
│ │ (persist) │ │ (FTS5) │ │ (usearch) │ │
│ └─────────────┘ └─────────────┘ └─────────────┘ │
│ │ │ │ │
│ └─────────────────────────┼─────────────────────────┘ │
│ ▼ │
│ ┌─────────────────────────────────────┐ │
│ │ Faceted Queries │ │
│ │ WHERE project_id =? AND branch =?│ │
│ │ AND file_path LIKE? │ │
│ └─────────────────────────────────────┘ │
│ │
│ ┌──────────────────────────────────────────────────────────────────────┐ │
│ │ Org-Scope (Feature-Gated) │ │
│ │ Future: PostgreSQL with shared org database │ │
│ └──────────────────────────────────────────────────────────────────────┘ │
│ │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Remove git-notes | Yes | Critical capture bug, design mismatch |
| Single active tier | User-level | Simplicity, single backup location |
| Faceting approach | Inline columns | Denormalized for query performance |
| Context detection | Auto-detect from git | Seamless UX, no config needed |
| Org-scope | Feature-gated | Design now, implement later |
| Default backend | SQLite | Already a dependency, ACID, portable |

## Component Design

### Component 1: Context Detector

**Purpose**: Detect git context (project, branch, path) from current working directory
**Responsibilities**:
- Detect if in a git repository
- Extract git remote URL (sanitized of credentials)
- Get current branch name
- Compute relative path from repo root

**Location**: `src/context/detector.rs` (new file)

**Interfaces**:
```rust
pub struct GitContext {
 /// Git remote URL (e.g., "github.com/zircote/subcog")
 pub project_id: Option<String>,
 /// Current branch name (e.g., "main", "feature/auth")
 pub branch: Option<String>,
 /// Path relative to repo root (e.g., "src/services")
 pub file_path: Option<String>,
 /// Whether we're in a git repository
 pub is_git_repo: bool,
}

impl GitContext {
 /// Detect context from current working directory
 pub fn from_cwd() -> Self {... }

 /// Detect context from a specific path
 pub fn from_path(path: &Path) -> Self {... }

 /// Parse and sanitize git remote URL
 fn parse_remote_url(url: &str) -> Option<String> {... }
}
```

**Dependencies**: `git2` crate (existing)

**Detection Logic**:
```
1. Repository::discover(path) - find.git upward
2. Get remote "origin" URL, sanitize credentials
3. Parse URL to extract "owner/repo" format
4. Get current branch from HEAD
5. Compute relative path: cwd.strip_prefix(repo.workdir())
```

### Component 2: Memory Model (Extended)

**Purpose**: Core data structure for memories with facet fields
**Responsibilities**: Store memory content and metadata including facets

**Location**: `src/models/memory.rs` (modify existing)

**Changes**:
```rust
pub struct Memory {
 // Existing fields
 pub id: MemoryId,
 pub content: String,
 pub namespace: Namespace,
 pub domain: Domain,
 pub status: MemoryStatus,
 pub created_at: u64,
 pub updated_at: u64,
 pub embedding: Option<Vec<f32>>,
 pub tags: Vec<String>,
 pub source: Option<String>,

 // NEW: Facet fields
 /// Git remote identifier (e.g., "github.com/zircote/subcog")
 pub project_id: Option<String>,
 /// Git branch name at capture time
 pub branch: Option<String>,
 /// File path relative to repository root
 pub file_path: Option<String>,
 /// When the memory was tombstoned (soft delete)
 pub tombstoned_at: Option<u64>,
}
```

### Component 3: MemoryStatus (Extended)

**Purpose**: Track memory lifecycle including tombstones
**Location**: `src/models/domain.rs` (modify existing)

**Changes**:
```rust
pub enum MemoryStatus {
 Active,
 Archived,
 Superseded,
 Pending,
 Deleted,
 Tombstoned, // NEW: Soft-deleted, hidden by default
}
```

### Component 4: SQLite Schema (Extended)

**Purpose**: Persist memories with facet columns
**Location**: `src/storage/index/sqlite.rs` (modify existing)

**Schema Migration** (`v2_facets.sql`):
```sql
-- Migration: Add facet columns
ALTER TABLE memories ADD COLUMN project_id TEXT;
ALTER TABLE memories ADD COLUMN branch TEXT;
ALTER TABLE memories ADD COLUMN file_path TEXT;
ALTER TABLE memories ADD COLUMN tombstoned_at INTEGER;

-- Indexes for faceted queries
CREATE INDEX IF NOT EXISTS idx_memories_project ON memories(project_id);
CREATE INDEX IF NOT EXISTS idx_memories_branch ON memories(branch);
CREATE INDEX IF NOT EXISTS idx_memories_path ON memories(file_path);
CREATE INDEX IF NOT EXISTS idx_memories_project_branch ON memories(project_id, branch);

-- Partial index for active memories (exclude tombstoned)
CREATE INDEX IF NOT EXISTS idx_memories_active
ON memories(namespace, status)
WHERE status!= 'tombstoned';
```

### Component 5: PostgreSQL Schema (Extended)

**Purpose**: Persist memories with facet columns for production deployments
**Location**: `src/storage/persistence/postgresql.rs` (modify existing)

**Schema Migration**:
```sql
-- Migration 4: Add facet columns
ALTER TABLE {table} ADD COLUMN IF NOT EXISTS project_id TEXT;
ALTER TABLE {table} ADD COLUMN IF NOT EXISTS branch TEXT;
ALTER TABLE {table} ADD COLUMN IF NOT EXISTS file_path TEXT;
ALTER TABLE {table} ADD COLUMN IF NOT EXISTS tombstoned_at BIGINT;

-- Indexes
CREATE INDEX IF NOT EXISTS idx_{table}_project ON {table} (project_id);
CREATE INDEX IF NOT EXISTS idx_{table}_branch ON {table} (branch);
CREATE INDEX IF NOT EXISTS idx_{table}_path ON {table} USING btree (file_path text_pattern_ops);
CREATE INDEX IF NOT EXISTS idx_{table}_project_branch ON {table} (project_id, branch);

-- Partial index for active memories
CREATE INDEX IF NOT EXISTS idx_{table}_active
ON {table} (namespace, status)
WHERE status!= 'tombstoned';
```

### Component 6: SearchFilter (Extended)

**Purpose**: Filter search results by facets
**Location**: `src/models/search.rs` (modify existing)

**Changes**:
```rust
pub struct SearchFilter {
 // Existing fields
 pub namespaces: Option<Vec<Namespace>>,
 pub statuses: Option<Vec<MemoryStatus>>,
 pub tags: Option<Vec<String>>,
 pub since: Option<u64>,
 pub until: Option<u64>,

 // NEW: Facet filters
 /// Filter by project ID (exact match)
 pub project_id: Option<String>,
 /// Filter by branch name (exact match)
 pub branch: Option<String>,
 /// Filter by file path (prefix match or glob pattern)
 pub file_path_pattern: Option<String>,
 /// Include tombstoned memories (default: false)
 pub include_tombstoned: bool,
}
```

### Component 7: CaptureService (Modified)

**Purpose**: Capture memories to storage
**Location**: `src/services/capture.rs` (modify existing)

**Key Changes**:

1. **Remove git-notes path** (delete lines 183-206):
```rust
// REMOVE THIS BLOCK:
if let Some(ref repo_path) = self.config.repo_path {
 let notes = NotesManager::new(repo_path);
 let note_oid = notes.add_to_head(&note_content)?;
 //...
}
```

2. **Add context detection**:
```rust
pub fn capture(&self, mut request: CaptureRequest) -> Result<CaptureResult> {
 // Auto-detect facets if not provided
 if request.project_id.is_none() {
 let context = GitContext::from_cwd();
 request.project_id = context.project_id;
 request.branch = context.branch;
 request.file_path = context.file_path;
 }

 // Generate ID (UUID, no longer git SHA)
 let memory_id = MemoryId::new(uuid::Uuid::new_v4()...);

 // Create memory with facets
 let memory = Memory {
 id: memory_id,
 project_id: request.project_id,
 branch: request.branch,
 file_path: request.file_path,
 //... other fields
 };

 // Store via configured backend (SQLite/PostgreSQL)
 self.persistence.store(&memory)?;
 self.index.index(&memory)?;
 //...
}
```

### Component 8: RecallService (Modified)

**Purpose**: Search and retrieve memories
**Location**: `src/services/recall.rs` (modify existing)

**Key Changes**:

1. **Add facet filtering to queries**:
```rust
pub fn search(&self, query: &str, filter: &SearchFilter, limit: usize) -> Result<SearchResult> {
 // Build WHERE clause with facets
 let mut conditions = vec![];

 if let Some(ref project) = filter.project_id {
 conditions.push(format!("project_id = '{}'", project));
 }
 if let Some(ref branch) = filter.branch {
 conditions.push(format!("branch = '{}'", branch));
 }
 if let Some(ref path_pattern) = filter.file_path_pattern {
 conditions.push(format!("file_path LIKE '{}'", path_pattern));
 }
 if!filter.include_tombstoned {
 conditions.push("status!= 'tombstoned'".to_string());
 }

 // Execute search with conditions
 //...
}
```

2. **Add lazy GC check**:
```rust
pub fn search(&self, query: &str, filter: &SearchFilter, limit: usize) -> Result<SearchResult> {
 // Opportunistic GC for current project
 if let Some(ref project) = filter.project_id {
 self.gc_stale_branches(project)?;
 }

 // Normal search
 //...
}
```

### Component 9: Branch Garbage Collection

**Purpose**: Tombstone memories for deleted branches
**Location**: `src/gc/branch.rs` (new file)

**Implementation**:
```rust
pub struct BranchGarbageCollector {
 index: Arc<dyn IndexBackend>,
}

impl BranchGarbageCollector {
 /// Garbage collect memories for deleted branches in a project
 pub fn gc_stale_branches(&self, project_id: &str) -> Result<usize> {
 // Get current branches from git
 let current_branches = self.get_current_branches()?;

 // Get distinct branches from memories for this project
 let memory_branches = self.index.get_distinct_branches(project_id)?;

 // Tombstone memories for branches that no longer exist
 let mut tombstoned = 0;
 for branch in memory_branches {
 if!current_branches.contains(&branch) {
 tombstoned += self.tombstone_branch(project_id, &branch)?;
 }
 }

 Ok(tombstoned)
 }

 fn tombstone_branch(&self, project_id: &str, branch: &str) -> Result<usize> {
 self.index.update_status(
 &format!("project_id = '{}' AND branch = '{}'", project_id, branch),
 MemoryStatus::Tombstoned,
 )
 }
}
```

### Component 10: ServiceContainer (Modified)

**Purpose**: Factory for creating services with configured backends
**Location**: `src/services/mod.rs` (modify existing)

**Key Changes**:

1. **Remove `repo_path` requirement**:
```rust
pub struct ServiceContainer {
 capture: CaptureService,
 sync: SyncService,
 index_manager: Mutex<DomainIndexManager>,
 // REMOVED: repo_path: Option<PathBuf>,
 embedder: Option<Arc<dyn Embedder>>,
 vector: Option<Arc<dyn VectorBackend + Send + Sync>>,
 gc: Option<BranchGarbageCollector>, // NEW
}
```

2. **Simplify factory methods**:
```rust
impl ServiceContainer {
 /// Create container for user-scope storage (default)
 pub fn for_user() -> Result<Self> {
 let paths = PathManager::for_user();
 let backends = BackendFactory::create_all(&paths);
 //...
 }

 /// Create container with PostgreSQL for org-scope (feature-gated)
 #[cfg(feature = "org-scope")]
 pub fn for_org(config: OrgConfig) -> Result<Self> {
 //...
 }
}
```

## Data Design

### Data Models

#### Memory (with Facets)

```
┌────────────────────────────────────────────────────────────┐
│ Memory │
├────────────────────────────────────────────────────────────┤
│ id: MemoryId (UUID) │
│ content: String │
│ namespace: Namespace │
│ status: MemoryStatus │
│ created_at: u64 │
│ updated_at: u64 │
│ embedding: Option<Vec<f32>> │
│ tags: Vec<String> │
│ source: Option<String> │
├────────────────────────────────────────────────────────────┤
│ FACETS │
│ project_id: Option<String> // "github.com/zircote/subcog"│
│ branch: Option<String> // "main", "feature/auth" │
│ file_path: Option<String> // "src/services" │
│ tombstoned_at: Option<u64> // Soft delete timestamp │
└────────────────────────────────────────────────────────────┘
```

### Data Flow

```
┌─────────────┐ ┌─────────────────┐ ┌──────────────────┐
│ CLI/MCP │────▶│ ContextDetector │────▶│ CaptureService │
│ Request │ │ (auto-detect) │ │ (store+index) │
└─────────────┘ └─────────────────┘ └──────────────────┘
 │
 ┌────────────────────────────────┤
 ▼ ▼
 ┌─────────────┐ ┌─────────────┐
 │ SQLite/PG │ │ usearch/ │
 │ Persistence │ │ pgvector │
 └─────────────┘ └─────────────┘


┌─────────────┐ ┌─────────────────┐ ┌──────────────────┐
│ CLI/MCP │────▶│ SearchFilter │────▶│ RecallService │
│ Query │ │ (facets) │ │ (search+rank) │
└─────────────┘ └─────────────────┘ └──────────────────┘
 │
 ┌─────────────────────────┤
 ▼ ▼
 ┌─────────────────┐ ┌─────────────┐
 │ Branch GC │ │ FTS5/ │
 │ (opportunistic)│ │ websearch │
 └─────────────────┘ └─────────────┘
```

### Storage Strategy

| Layer | Purpose | Technology | Location |
|-------|---------|------------|----------|
| Persistence | Authoritative storage | SQLite/PostgreSQL | `memories.db` |
| Index | Full-text search | SQLite FTS5/PG websearch | `index.db` |
| Vector | Semantic similarity | usearch/pgvector | `vectors.idx` |

**User Data Paths** (via `directories` crate):

| Platform | Path |
|----------|------|
| Linux | `~/.local/share/subcog/` |
| macOS | `~/Library/Application Support/io.subcog.subcog/` |
| Windows | `C:\Users\<User>\AppData\Roaming\subcog\data\` |

## API Design

### CLI Interface

```bash
# Capture with auto-detected facets
subcog capture --namespace decisions "Use PostgreSQL"
# -> project_id, branch, file_path auto-filled from cwd

# Capture with explicit facets
subcog capture --namespace decisions --project "my-project" "Use PostgreSQL"

# Recall with default filters (current project, exclude tombstoned)
subcog recall "PostgreSQL"

# Recall across all projects
subcog recall "PostgreSQL" --all-projects

# Recall specific project/branch
subcog recall "PostgreSQL" --project "github.com/zircote/subcog" --branch "main"

# Include tombstoned memories
subcog recall "PostgreSQL" --include-tombstoned

# Garbage collection
subcog gc # GC stale branches in current project
subcog gc --branch "feature-x" # GC specific branch
subcog gc --dry-run # Show what would be tombstoned
subcog gc --purge --older-than=30d # Permanently delete old tombstones
```

### MCP Tool Interface

**subcog_capture** (modified):
```json
{
 "name": "subcog_capture",
 "description": "Capture a memory with optional facets",
 "inputSchema": {
 "type": "object",
 "properties": {
 "content": { "type": "string" },
 "namespace": { "type": "string" },
 "tags": { "type": "array", "items": { "type": "string" } },
 "project_id": { "type": "string", "description": "Optional: override detected project" },
 "branch": { "type": "string", "description": "Optional: override detected branch" },
 "file_path": { "type": "string", "description": "Optional: override detected path" }
 },
 "required": ["content", "namespace"]
 }
}
```

**subcog_recall** (modified):
```json
{
 "name": "subcog_recall",
 "description": "Search memories with facet filters",
 "inputSchema": {
 "type": "object",
 "properties": {
 "query": { "type": "string" },
 "limit": { "type": "integer", "default": 10 },
 "project_id": { "type": "string" },
 "branch": { "type": "string" },
 "file_path_pattern": { "type": "string" },
 "include_tombstoned": { "type": "boolean", "default": false }
 },
 "required": ["query"]
 }
}
```

## Integration Points

### Internal Integrations

| System | Integration Type | Purpose |
|--------|-----------------|---------|
| CaptureService | Direct call | Store memories with facets |
| RecallService | Direct call | Search with facet filters |
| Hooks | Event handler | Context injection, auto-capture |
| ServiceContainer | Factory | Create configured services |

### External Integrations

| Service | Integration Type | Purpose |
|---------|-----------------|---------|
| Claude Code | MCP protocol | Memory capture/recall |
| Git | git2 crate | Context detection |

## Security Design

### Credential Protection

Git remote URLs may contain credentials that must be sanitized:

```rust
fn parse_remote_url(url: &str) -> Option<String> {
 // Remove credentials from URLs like:
 // https://user:token@github.com/owner/repo.git
 // git@github.com:owner/repo.git

 let sanitized = url
.replace(regex!(r"https?://[^@]+@"), "https://")
.replace(regex!(r"\.git$"), "");

 // Extract owner/repo
 //...
}
```

### File Permissions

```rust
fn create_user_storage() -> Result<PathBuf> {
 let path = directories::ProjectDirs::from("io", "subcog", "subcog")
.ok_or(Error::NoHomeDirectory)?
.data_dir()
.to_path_buf();

 std::fs::create_dir_all(&path)?;

 #[cfg(unix)]
 {
 use std::os::unix::fs::PermissionsExt;
 std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
 }

 Ok(path)
}
```

## Performance Considerations

### Expected Load

| Metric | Value |
|--------|-------|
| Memories per user | 100,000+ |
| Captures per session | 10-50 |
| Recalls per session | 50-200 |
| Concurrent sessions | 1 (CLI) or 10+ (MCP server) |

### Performance Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| Capture latency | <50ms P99 | Current baseline |
| Recall latency | <100ms P99 | Current baseline |
| GC overhead | <10ms | Should not impact UX |
| Index size | ~1KB per memory | SQLite + FTS5 |

### Optimization Strategies

1. **Indexed facet columns**: B-tree indexes on `project_id`, `branch`
2. **Partial indexes**: Exclude tombstoned for active queries
3. **Lazy GC**: Only run on matching project queries
4. **Connection pooling**: Reuse SQLite/PostgreSQL connections
5. **WAL mode**: Better concurrent read performance

## Testing Strategy

### Unit Testing

| Component | Coverage Target | Focus |
|-----------|-----------------|-------|
| ContextDetector | 95% | Edge cases (no git, detached HEAD, worktrees) |
| Memory model | 100% | Serialization, facet fields |
| SearchFilter | 100% | Query building |
| BranchGC | 95% | Tombstone logic |

### Integration Testing

| Scenario | Test |
|----------|------|
| Capture in git repo | Verify facets auto-detected |
| Capture outside git | Verify graceful fallback |
| Recall with facets | Verify filtering works |
| Branch deletion | Verify tombstoning |
| Tombstone exclusion | Verify default behavior |

### End-to-End Testing

| Flow | Test |
|------|------|
| CLI capture -> recall | Full roundtrip |
| MCP capture -> recall | Full roundtrip via protocol |
| Hook integration | Session start/stop flow |

## Deployment Considerations

### Migration

No data migration needed - fresh start approach per user decision.

### Rollout Strategy

1. **Phase 1**: Add facet fields (backward compatible)
2. **Phase 2**: Remove git-notes code
3. **Phase 3**: Enable branch GC
4. **Phase 4**: Feature-gate org-scope

### Rollback Plan

1. Revert to previous version (captures still work via UUID IDs)
2. No data loss (SQLite is authoritative)

## Future Considerations

1. **Remote sync**: Push/pull user storage to cloud backup
2. **Cross-scope search**: Query user + project simultaneously
3. **Org-scope implementation**: PostgreSQL with shared database
4. **Memory deduplication**: Detect and merge similar memories across projects
