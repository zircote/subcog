---
document_type: decisions
project_id: SPEC-2026-01-03-001
version: 1.0.0
last_updated: 2026-01-03T01:30:00Z
---

# Architecture Decision Records

This document captures key architectural decisions for the Storage Architecture Simplification project.

## ADR Index

| ID | Title | Status | Date |
|----|-------|--------|------|
| ADR-001 | Remove Git-Notes Storage Layer | Accepted | 2026-01-03 |
| ADR-002 | Consolidate to User-Level Storage with Faceting | Accepted | 2026-01-03 |
| ADR-003 | Inline Facet Columns (Denormalized) | Accepted | 2026-01-03 |
| ADR-004 | Fresh Start - No Migration of Legacy Data | Accepted | 2026-01-03 |
| ADR-005 | Feature-Gate Org-Scope Implementation | Accepted | 2026-01-03 |
| ADR-006 | Lazy Branch Garbage Collection | Accepted | 2026-01-03 |
| ADR-007 | Tombstone Pattern for Soft Deletes | Accepted | 2026-01-03 |

---

## ADR-001: Remove Git-Notes Storage Layer

### Status
Accepted

### Context

The current storage architecture uses git-notes as the primary persistence layer for project-scoped memories. However, this approach has fundamental problems:

1. **Critical Capture Bug**: The `CaptureService::capture()` method at `src/services/capture.rs:198` calls `notes.add_to_head()` which uses `force=true` at `src/git/notes.rs:113`. This causes **every new memory to overwrite the previous one** - only one memory can exist at a time.

2. **Design Mismatch**: Git notes are designed to annotate specific commits, not to store arbitrary key-value data. The current approach fights against git's design by trying to attach unrelated data to HEAD.

3. **Workaround Complexity**: A proper fix would require creating unique marker blobs for each memory, essentially implementing a content-addressed store on top of git - adding significant complexity.

4. **Sync Issues**: Git notes require special fetch/push refspecs, and conflicts during sync can lose data silently.

### Decision

**Remove git-notes as a persistence layer entirely.**

- Delete `src/git/notes.rs` and related git-notes code
- Remove git2 crate dependency if it's only used for notes
- Route all captures through SQLite/PostgreSQL backends
- Use `directories` crate for platform-specific user data paths

### Consequences

**Positive:**
- Eliminates the critical capture overwrite bug
- Simplifies codebase by ~500 LOC
- Removes complex sync logic
- Single source of truth for all memories

**Negative:**
- Loses the "memories travel with the repo" feature
- Requires explicit backup strategy for user data directory
- Breaking change for existing workflows (mitigated by fresh start decision)

**Neutral:**
- Git remains useful for source context detection, just not storage

### Alternatives Considered

1. **Fix git-notes with unique blobs**: Create a marker blob for each memory and attach notes to that blob. Rejected because it fights against git's design and adds complexity.

2. **Use git-notes only for export**: Keep git-notes as a "publish" mechanism rather than primary storage. Rejected as premature - can be added later if needed.

3. **Switch to git-backed file storage**: Store memories as files in `.subcog/` directory and commit them. Rejected because it pollutes repository history and requires special handling for ignored directories.

---

## ADR-002: Consolidate to User-Level Storage with Faceting

### Status
Accepted

### Context

The current architecture has three storage tiers with complex routing:

```
org/ -> Shared PostgreSQL (configured, rarely used)
project/ -> Git notes per repo (BROKEN)
user/ -> ~/.config/subcog/ (works)
```

This creates several problems:
- Complexity in deciding where to store/retrieve
- Per-repository storage prevents cross-project learning
- Backup and sync story is complicated (3 locations)

### Decision

**Consolidate to user-level storage with project/branch/path facets.**

- Single storage location: `~/.local/share/subcog/` (Linux), `~/Library/Application Support/subcog/` (macOS), `%APPDATA%\subcog\` (Windows)
- Auto-detect facets from git context when capturing
- Query by facets instead of switching storage tiers

Facet fields added to Memory struct:
```rust
pub struct Memory {
 //... existing fields...
 pub project_id: Option<String>, // Normalized git remote URL
 pub branch: Option<String>, // Current branch name
 pub file_path: Option<String>, // Relative path from repo root
}
```

### Consequences

**Positive:**
- Single location to backup
- Cross-project learning enabled via faceted queries
- Simpler mental model for users
- Reduced codebase complexity

**Negative:**
- Memories don't automatically travel with repository clones
- Requires careful facet normalization (remote URL variations)

**Neutral:**
- Org-scope can be added later as a separate configured backend

---

## ADR-003: Inline Facet Columns (Denormalized)

### Status
Accepted

### Context

There are two approaches to storing facets:

1. **Normalized**: Separate `projects` and `branches` tables with foreign keys
2. **Denormalized**: Inline columns directly on the `memories` table

### Decision

**Use inline (denormalized) facet columns on the memories table.**

```sql
ALTER TABLE memories ADD COLUMN project_id TEXT;
ALTER TABLE memories ADD COLUMN branch TEXT;
ALTER TABLE memories ADD COLUMN file_path TEXT;

CREATE INDEX idx_memories_project ON memories(project_id);
CREATE INDEX idx_memories_branch ON memories(project_id, branch);
CREATE INDEX idx_memories_path ON memories(file_path);
```

### Rationale

1. **Simplicity**: No joins required for common queries
2. **Performance**: Direct filtering without join overhead
3. **Flexibility**: Facets can be NULL for non-git contexts
4. **Garbage Collection**: Simple `UPDATE SET status='tombstoned' WHERE branch=?`

### Consequences

**Positive:**
- Simpler queries
- Better query performance
- Easier garbage collection
- No referential integrity issues

**Negative:**
- Storage overhead from repeated project_id strings
- No enforced consistency for project names

**Mitigations:**
- Project IDs are typically <100 chars, overhead is minimal
- Normalization can be applied at query time if needed

---

## ADR-004: Fresh Start - No Migration of Legacy Data

### Status
Accepted

### Context

There are 645 memories in existing git-notes across various repositories. These were created by earlier tooling (the Python git-notes-memory-manager) that used a different storage approach.

Options:
1. Build migration tooling to extract and re-import
2. Provide export-only tooling for manual inspection
3. Fresh start - legacy data remains in git-notes history

### Decision

**Fresh start - no automatic migration of legacy git-notes data.**

Rationale from user:
> "The 645 memories are artifacts from earlier tooling. Fresh start is acceptable."

### Consequences

**Positive:**
- No complex migration code to build and maintain
- No risk of importing corrupted/duplicate data
- Clean slate with known-good architecture

**Negative:**
- Historical decisions and learnings not immediately available
- Users must re-capture important memories manually

**Mitigations:**
- Legacy git-notes remain in repository history (not deleted)
- Users can manually review and re-capture high-value memories
- A future "import" command could be added if demand exists

---

## ADR-005: Feature-Gate Org-Scope Implementation

### Status
Accepted

### Context

The original architecture included an org-scope tier for shared PostgreSQL storage across teams. This enables:
- Shared decisions across organization
- Centralized backup and compliance
- Team knowledge aggregation

### Decision

**Include org-scope in the design, but feature-gate the implementation.**

```rust
// config/features.rs
pub struct FeatureFlags {
 pub org_scope_enabled: bool, // Default: false
 //...
}
```

The architecture supports org-scope but it's disabled by default. When enabled:
- Requires PostgreSQL connection configuration
- Sync behavior TBD (push-only, pull-only, bidirectional)
- Admin controls for namespace visibility

### Consequences

**Positive:**
- Future-proofs the architecture
- No immediate implementation burden
- Can be enabled when demand materializes

**Negative:**
- Design complexity for unused feature
- May need redesign when actually implemented

**Mitigations:**
- Keep org-scope interfaces minimal and abstract
- Document intended behavior in ARCHITECTURE.md

---

## ADR-006: Lazy Branch Garbage Collection

### Status
Accepted

### Context

When a git branch is deleted, memories captured in that branch context become stale. They may reference decisions or patterns that are no longer relevant.

Options for cleanup:
1. **Immediate GC via git hooks**: `reference-transaction` hook triggers GC
2. **Lazy GC during recall**: Check branch existence opportunistically
3. **Scheduled GC job**: Background process runs periodically
4. **Manual GC only**: User runs `subcog gc` explicitly

### Decision

**Implement lazy garbage collection during recall, with manual override.**

```rust
// During recall, after fetching results:
for memory in &mut results {
 if let Some(branch) = &memory.branch {
 if!branch_exists(branch) && memory.status!= MemoryStatus::Tombstoned {
 self.tombstone_memory(&memory.id)?;
 memory.status = MemoryStatus::Tombstoned;
 }
 }
}
```

Also provide:
- `subcog gc` command for immediate cleanup
- `--dry-run` flag to preview tombstones
- `--branch` flag to target specific branch

### Consequences

**Positive:**
- No external dependencies (git hooks, schedulers)
- GC happens naturally during use
- Stale memories surface (then tombstone) organically
- Manual override for power users

**Negative:**
- First recall after branch deletion slightly slower
- May return stale results briefly before tombstoning
- Branch existence check adds latency (mitigated by caching)

**Mitigations:**
- Cache branch existence for 5 minutes
- Tombstoning is a background task, doesn't block results
- Consider git hook as P2 enhancement

---

## ADR-007: Tombstone Pattern for Soft Deletes

### Status
Accepted

### Context

When memories become stale (branch deleted, manual cleanup), we need to remove them from active results. Options:

1. **Hard delete**: Remove rows permanently
2. **Soft delete (tombstone)**: Mark as deleted, filter by default
3. **Archive**: Move to separate table

### Decision

**Use tombstone status for soft deletes with configurable visibility.**

```rust
pub enum MemoryStatus {
 Active,
 Tombstoned,
}
```

Behavior:
- Tombstoned memories are **hidden by default** in recall
- `--include-tombstoned` flag shows them
- Tombstones are **never auto-purged** (data safety)
- Manual purge via `subcog gc --purge --older-than=30d`

### Consequences

**Positive:**
- No accidental data loss
- Archaeology possible (find old decisions)
- Simple implementation (just a status flag)
- Consistent with existing `MemoryStatus` enum

**Negative:**
- Storage never automatically reclaimed
- Index bloat over time (mitigated by purge command)

**Mitigations:**
- Document purge workflow
- Consider automatic tombstone indexing for filtered queries
- Add `tombstoned_at` timestamp for retention policies

---

## Decision Summary

| Decision | Key Rationale |
|----------|---------------|
| Remove git-notes | Critical bug, design mismatch |
| User-level with facets | Simplicity, cross-project learning |
| Inline facet columns | Query performance, simplicity |
| Fresh start | Clean slate, no migration complexity |
| Feature-gate org-scope | Future-proof without immediate burden |
| Lazy GC | No external dependencies, natural cleanup |
| Tombstone pattern | Data safety, archaeology support |

These decisions collectively achieve:
- **Data Integrity**: No more overwrites, ACID transactions
- **Simplicity**: One storage location, fewer code paths
- **Flexibility**: Faceted queries, cross-project insights
- **Safety**: Soft deletes, no auto-purge
