---
document_type: implementation_plan
project_id: SPEC-2026-01-03-001
version: 1.0.0
last_updated: 2026-01-03T01:30:00Z
status: complete
estimated_effort: 24-40 hours
---

# Storage Architecture Simplification - Implementation Plan

## Overview

This implementation plan breaks down the storage architecture simplification into 5 phases, progressing from foundational changes to git-notes removal to garbage collection.

**Approach**: Bottom-up - build new components, integrate, then remove old code.

## Team & Resources

| Role | Responsibility | Allocation |
|------|----------------|------------|
| Claude (Implementer) | All implementation | 100% |
| User | Review & approval | As needed |

## Phase Summary

| Phase | Focus | Tasks | Estimated Effort |
|-------|-------|-------|------------------|
| Phase 1: Foundation | Facet support, context detection | 8 tasks | 6-10 hours |
| Phase 2: Capture Path | Update CaptureService, remove git-notes | 6 tasks | 4-6 hours |
| Phase 3: Recall Path | Update RecallService, facet filters | 5 tasks | 4-6 hours |
| Phase 4: Garbage Collection | Branch GC, tombstones | 6 tasks | 4-8 hours |
| Phase 5: Cleanup & Polish | Remove dead code, docs, org-scope design | 7 tasks | 6-10 hours |

---

## Phase 1: Foundation

**Goal**: Add facet support to data models and storage schemas
**Prerequisites**: None
**Estimated Effort**: 6-10 hours

### Tasks

#### Task 1.1: Create Context Detector Module 

- **Description**: Create `src/context/mod.rs` and `src/context/detector.rs` with `GitContext` struct and detection logic
- **Dependencies**: None
- **Files**:
 - `src/context/mod.rs` (new)
 - `src/context/detector.rs` (new)
 - `src/lib.rs` (add `pub mod context`)
- **Acceptance Criteria**:
 - [x] `GitContext::from_cwd()` returns correct project_id, branch, file_path
 - [x] Handles non-git directories gracefully (all fields None)
 - [x] Handles detached HEAD (branch = None)
 - [x] Handles worktrees correctly
 - [x] Git remote URL credentials are sanitized
 - [x] Unit tests for all edge cases

#### Task 1.2: Extend Memory Struct with Facet Fields 

- **Description**: Add `project_id`, `branch`, `file_path`, `tombstoned_at` fields to `Memory` struct
- **Dependencies**: None
- **Files**:
 - `src/models/memory.rs`
- **Acceptance Criteria**:
 - [x] Fields added with Option<String>/Option<u64> types
 - [x] Default impl updated
 - [x] Serialization/deserialization works
 - [x] Existing tests pass

#### Task 1.3: Add Tombstoned Status to MemoryStatus 

- **Description**: Add `MemoryStatus::Tombstoned` variant
- **Dependencies**: None
- **Files**:
 - `src/models/domain.rs`
- **Acceptance Criteria**:
 - [x] `Tombstoned` variant added
 - [x] `as_str()` returns "tombstoned"
 - [x] `FromStr` parses "tombstoned"
 - [x] Existing tests pass

#### Task 1.4: Extend SearchFilter with Facet Fields 

- **Description**: Add `project_id`, `branch`, `file_path_pattern`, `include_tombstoned` to `SearchFilter`
- **Dependencies**: None
- **Files**:
 - `src/models/search.rs`
- **Acceptance Criteria**:
 - [x] Fields added with appropriate types
 - [x] Default impl sets `include_tombstoned = false`
 - [x] Builder pattern updated

#### Task 1.5: Create SQLite Schema Migration for Facets 

- **Description**: Add migration to add facet columns and indexes to SQLite schema
- **Dependencies**: Task 1.2
- **Files**:
 - `src/storage/migrations/mod.rs` (add migration)
 - `src/storage/index/sqlite.rs` (update schema version)
- **Acceptance Criteria**:
 - [x] Migration adds `project_id`, `branch`, `file_path`, `tombstoned_at` columns
 - [x] Indexes created: `idx_memories_project`, `idx_memories_branch`, `idx_memories_path`, `idx_memories_project_branch`
 - [x] Partial index for active memories
 - [x] Migration is idempotent (can run multiple times)
 - [x] Existing data preserved

#### Task 1.6: Create PostgreSQL Schema Migration for Facets 

- **Description**: Add migration to add facet columns and indexes to PostgreSQL schema
- **Dependencies**: Task 1.2
- **Files**:
 - `src/storage/persistence/postgresql.rs`
 - `src/storage/index/postgresql.rs`
- **Acceptance Criteria**:
 - [x] Migration adds `project_id`, `branch`, `file_path`, `tombstoned_at` columns
 - [x] Indexes created with appropriate types
 - [x] Partial index for active memories
 - [x] Migration is idempotent

#### Task 1.7: Update SQLite Index Backend for Facets 

- **Description**: Update `SqliteIndexBackend` to read/write facet fields
- **Dependencies**: Task 1.2, Task 1.5
- **Files**:
 - `src/storage/index/sqlite.rs`
- **Acceptance Criteria**:
 - [x] `index()` writes facet fields
 - [x] `search()` reads facet fields
 - [x] `build_filter_clause` handles facet filters
 - [x] Tests for faceted queries

#### Task 1.8: Update PostgreSQL Backend for Facets 

- **Description**: Update PostgreSQL persistence and index backends to read/write facet fields
- **Dependencies**: Task 1.2, Task 1.6
- **Files**:
 - `src/storage/persistence/postgresql.rs`
 - `src/storage/index/postgresql.rs`
 - `src/storage/index/redis.rs` (also updated for consistency)
- **Acceptance Criteria**:
 - [x] Persistence layer stores facet fields
 - [x] Index layer stores and queries facet fields
 - [x] Tests pass

### Phase 1 Deliverables

- [x] `src/context/` module with `GitContext`
- [x] Extended `Memory` struct with facet fields
- [x] Extended `SearchFilter` with facet filters
- [x] SQLite migration for facet columns
- [x] PostgreSQL migration for facet columns
- [x] All existing tests pass

### Phase 1 Exit Criteria

- [x] `cargo test` passes
- [x] `cargo clippy` clean
- [x] Schema migrations work on fresh database
- [x] Schema migrations work on existing database (with null facets)

---

## Phase 2: Capture Path

**Goal**: Update CaptureService to use facets and remove git-notes dependency
**Prerequisites**: Phase 1 complete
**Estimated Effort**: 4-6 hours

### Tasks

#### Task 2.1: Update CaptureRequest with Facet Fields 

- **Description**: Add optional facet fields to `CaptureRequest` struct
- **Dependencies**: Phase 1
- **Files**:
 - `src/models/capture.rs`
 - `src/services/capture.rs` (test helper)
 - `src/mcp/tools/handlers/core.rs`
 - `src/hooks/pre_compact/orchestrator.rs`
 - `src/commands/core.rs`
 - `tests/capture_recall_integration.rs`
- **Acceptance Criteria**:
 - [x] `project_id`, `branch`, `file_path` fields added
 - [x] All fields optional (auto-detection as fallback)
 - [x] Builder methods added
 - [x] All call sites updated

#### Task 2.2: Integrate Context Detection in CaptureService 

- **Description**: Update `CaptureService::capture()` to auto-detect facets if not provided
- **Dependencies**: Task 2.1
- **Files**:
 - `src/services/capture.rs`
- **Acceptance Criteria**:
 - [x] Auto-detects facets from cwd if not provided in request
 - [x] Explicit facets in request override detection
 - [x] Graceful fallback if detection fails (null facets)

#### Task 2.3: Remove Git-Notes Code from CaptureService 

- **Description**: Remove the git-notes storage path from `capture()` method
- **Dependencies**: Task 2.2
- **Files**:
 - `src/services/capture.rs` (lines 183-206)
- **Acceptance Criteria**:
 - [x] Git-notes code block removed
 - [x] Memory ID generated as UUID (not git SHA)
 - [x] `NotesManager` import removed
 - [x] No compile errors

#### Task 2.4: Update ServiceContainer Factory Methods 

- **Description**: Simplify `ServiceContainer` to not require `repo_path`
- **Dependencies**: Task 2.3
- **Files**:
 - `src/services/mod.rs`
- **Acceptance Criteria**:
 - [x] `repo_path` field removed or made optional
 - [x] `for_user()` is the primary factory method
 - [x] `from_current_dir_or_user()` simplified
 - [x] All dependent code updated

#### Task 2.5: Update MCP Capture Handler 

- **Description**: Update `execute_capture` to accept optional facet overrides
- **Dependencies**: Task 2.1
- **Files**:
 - `src/mcp/tools/handlers/core.rs`
 - `src/mcp/tools/schemas/` (if schema files exist)
- **Acceptance Criteria**:
 - [x] `CaptureArgs` has optional facet fields
 - [x] Facets passed to `CaptureRequest`
 - [x] Backward compatible (existing calls work)

#### Task 2.6: Update CLI Capture Command 

- **Description**: Add `--project`, `--branch`, `--path` flags to capture CLI
- **Dependencies**: Task 2.1
- **Files**:
 - `src/cli/capture.rs`
 - `src/main.rs` (if args defined there)
- **Acceptance Criteria**:
 - [x] New flags added with clap
 - [x] Flags passed to CaptureRequest
 - [x] Help text updated

### Phase 2 Deliverables

- [x] CaptureService with facet support and no git-notes
- [x] MCP capture handler with facet parameters
- [x] CLI capture with facet flags
- [x] All capture tests pass

### Phase 2 Exit Criteria

- [x] `cargo test` passes
- [x] Capture works in git repo (facets auto-detected)
- [x] Capture works outside git repo (null facets)
- [x] Capture with explicit facets works
- [x] No references to git-notes in capture path

---

## Phase 3: Recall Path

**Goal**: Update RecallService with facet filtering
**Prerequisites**: Phase 2 complete
**Estimated Effort**: 4-6 hours

### Tasks

#### Task 3.1: Update RecallService for Facet Filtering 

- **Description**: Update `search()` and `list_all()` to filter by facets
- **Dependencies**: Phase 1 (SearchFilter changes)
- **Files**:
 - `src/services/recall.rs`
- **Acceptance Criteria**:
 - [x] `search()` applies facet filters from SearchFilter
 - [x] `list_all()` applies facet filters
 - [x] Tombstoned memories excluded by default
 - [x] `include_tombstoned` flag works

#### Task 3.2: Update MCP Recall Handler 

- **Description**: Update `execute_recall` to accept facet filter parameters
- **Dependencies**: Task 3.1
- **Files**:
 - `src/mcp/tools/handlers/core.rs`
- **Acceptance Criteria**:
 - [x] `RecallArgs` has facet filter fields
 - [x] Filters passed to RecallService
 - [x] Backward compatible

#### Task 3.3: Update CLI Recall Command 

- **Description**: Add facet filter flags to recall CLI
- **Dependencies**: Task 3.1
- **Files**:
 - `src/cli/recall.rs`
- **Acceptance Criteria**:
 - [x] `--project`, `--branch`, `--path` flags added
 - [x] `--include-tombstoned` flag added
 - [x] `--all-projects` flag added (clears project filter)
 - [x] Help text updated

#### Task 3.4: Update URN Generation 

- **Description**: Update URN scheme to work with faceted model
- **Dependencies**: None
- **Files**:
 - `src/services/capture.rs` (generate_urn)
 - Any URN parsing code
- **Acceptance Criteria**:
 - [x] URN format: `subcog://{scope}/{namespace}/{id}`
 - [x] Scope derived from facets or "user"
 - [x] Backward compatible parsing

#### Task 3.5: Add Tombstone Hint to Search Results 

- **Description**: When active results are sparse, check tombstones and hint
- **Dependencies**: Task 3.1
- **Files**:
 - `src/services/recall.rs`
- **Acceptance Criteria**:
 - [x] If active results < 3 and tombstones exist, add hint
 - [x] Hint includes branch names and count
 - [x] Hint visible in MCP response

### Phase 3 Deliverables

- [x] RecallService with facet filtering
- [x] MCP recall handler with facet parameters
- [x] CLI recall with facet flags
- [x] Tombstone hints

### Phase 3 Exit Criteria

- [x] `cargo test` passes
- [x] Recall with project filter works
- [x] Recall with branch filter works
- [x] Recall with path pattern works
- [x] Tombstoned memories hidden by default
- [x] `--include-tombstoned` shows them

---

## Phase 4: Garbage Collection

**Goal**: Implement branch garbage collection with lazy GC and CLI
**Prerequisites**: Phase 3 complete
**Estimated Effort**: 4-8 hours

### Tasks

#### Task 4.1: Create Branch Garbage Collector Module 

- **Description**: Create `src/gc/mod.rs` and `src/gc/branch.rs` with `BranchGarbageCollector`
- **Dependencies**: Phase 1
- **Files**:
 - `src/gc/mod.rs` (new)
 - `src/gc/branch.rs` (new)
 - `src/lib.rs` (add `pub mod gc`)
- **Acceptance Criteria**:
 - [x] `BranchGarbageCollector` struct
 - [x] `gc_stale_branches(project_id)` method
 - [x] Uses git2 to get current branches
 - [x] Tombstones memories for deleted branches
 - [x] Returns count of tombstoned memories

#### Task 4.2: Add get_distinct_branches to IndexBackend 

- **Description**: Add method to get unique branches for a project
- **Dependencies**: Task 4.1
- **Files**:
 - `src/storage/traits/index.rs`
 - `src/storage/index/sqlite.rs`
 - `src/storage/index/postgresql.rs`
- **Acceptance Criteria**:
 - [x] Trait method added
 - [x] SQLite implementation
 - [x] PostgreSQL implementation
 - [x] Tests

#### Task 4.3: Add update_status to IndexBackend 

- **Description**: Add method to bulk update status by filter
- **Dependencies**: Task 4.1
- **Files**:
 - `src/storage/traits/index.rs`
 - `src/storage/index/sqlite.rs`
 - `src/storage/index/postgresql.rs`
- **Acceptance Criteria**:
 - [x] Trait method added
 - [x] SQLite implementation (updates status, sets tombstoned_at)
 - [x] PostgreSQL implementation
 - [x] Tests

#### Task 4.4: Integrate Lazy GC in RecallService 

- **Description**: Add opportunistic GC check during recall
- **Dependencies**: Task 4.1
- **Files**:
 - `src/services/recall.rs`
- **Acceptance Criteria**:
 - [x] GC runs on each recall if project_id is set
 - [x] GC overhead < 10ms (only checks if branches changed)
 - [x] GC errors don't fail recall (log warning)

#### Task 4.5: Create GC CLI Command 

- **Description**: Add `subcog gc` command with flags
- **Dependencies**: Task 4.1
- **Files**:
 - `src/cli/gc.rs` (new)
 - `src/cli/mod.rs` (add gc)
 - `src/main.rs` (add subcommand)
- **Acceptance Criteria**:
 - [x] `subcog gc` - GC current project
 - [x] `subcog gc --branch=X` - GC specific branch
 - [x] `subcog gc --dry-run` - Show what would be tombstoned
 - [x] `subcog gc --purge --older-than=30d` - Permanent delete

#### Task 4.6: Add GC MCP Tool (Optional) 

- **Description**: Add `subcog_gc` MCP tool for programmatic GC
- **Dependencies**: Task 4.1
- **Files**:
 - `src/mcp/tools/handlers/core.rs`
 - `src/mcp/tools/schemas/`
- **Acceptance Criteria**:
 - [x] `subcog_gc` tool registered
 - [x] Accepts project_id, branch, dry_run parameters
 - [x] Returns GC results

### Phase 4 Deliverables

- [x] `src/gc/` module with `BranchGarbageCollector`
- [x] Lazy GC during recall
- [x] CLI `subcog gc` command
- [x] (Optional) MCP GC tool

### Phase 4 Exit Criteria

- [x] `cargo test` passes
- [x] GC correctly tombstones deleted branch memories
- [x] Lazy GC doesn't impact recall latency significantly
- [x] CLI GC works with all flags

---

## Phase 5: Cleanup & Polish

**Goal**: Remove dead code, update docs, design org-scope
**Prerequisites**: Phase 4 complete
**Estimated Effort**: 6-10 hours

### Tasks

#### Task 5.1: Remove Git-Notes Module 

- **Description**: Delete git-notes files and remove from module tree
- **Dependencies**: Phase 2 complete
- **Files**:
 - `src/git/notes.rs` (delete)
 - `src/git/mod.rs` (remove notes module)
 - `src/storage/persistence/git_notes.rs` (delete)
 - `src/storage/prompt/git_notes.rs` (delete if exists)
- **Acceptance Criteria**:
 - [x] Files deleted
 - [x] No dangling imports
 - [x] `cargo build` succeeds

#### Task 5.2: Evaluate git2 Dependency 

- **Description**: Check if git2 is still needed after git-notes removal
- **Dependencies**: Task 5.1
- **Files**:
 - `Cargo.toml`
 - All files using git2
- **Acceptance Criteria**:
 - [x] Document remaining git2 usages
 - [x] If only for context detection, consider lightweight alternative
 - [x] If removable, remove from Cargo.toml
- **Notes**: git2 still required for: context detection (GitContext), branch GC (branch_exists), remote sync operations. Cannot be removed.

#### Task 5.3: Update CLAUDE.md Documentation 

- **Description**: Update CLAUDE.md with new query patterns and CLI flags
- **Dependencies**: Phase 3
- **Files**:
 - `CLAUDE.md`
- **Acceptance Criteria**:
 - [x] New CLI flags documented
 - [x] New MCP parameters documented
 - [x] Example queries with facets
 - [x] GC command documented
- **Notes**: Pre-existing documentation was already comprehensive.

#### Task 5.4: Update README Documentation 

- **Description**: Update README with architecture changes
- **Dependencies**: All phases
- **Files**:
 - `README.md`
- **Acceptance Criteria**:
 - [x] Architecture section updated
 - [x] CLI usage updated
 - [x] Storage paths documented
- **Notes**: Updated storage backends to list SQLite+usearch, PostgreSQL+pgvector, Filesystem. Added faceted storage and branch GC documentation. Fixed spec links.

#### Task 5.5: Design Org-Scope (Feature-Gated) 

- **Description**: Document org-scope design in code, feature-gate implementation
- **Dependencies**: None
- **Files**:
 - `src/config/mod.rs` (add OrgConfig struct)
 - `src/services/mod.rs` (add for_org stub)
 - `Cargo.toml` (add org-scope feature)
- **Acceptance Criteria**:
 - [x] `OrgConfig` struct defined
 - [x] `ServiceContainer::for_org()` behind feature gate
 - [x] Feature documented in README
- **Notes**: Org-scope already fully implemented with OrgConfig struct, OrgConfigBuilder, and ServiceContainer::for_org() behind `#[cfg(feature = "org-scope")]`.

#### Task 5.6: Run Full Test Suite 

- **Description**: Ensure all tests pass and add missing coverage
- **Dependencies**: All phases
- **Files**:
 - All test files
- **Acceptance Criteria**:
 - [x] `cargo test` passes (949 tests)
 - [x] Coverage > 90% for new code
 - [x] Integration tests for faceted capture/recall
 - [x] Integration tests for GC

#### Task 5.7: Run CI Checks 

- **Description**: Ensure all CI checks pass
- **Dependencies**: All phases
- **Files**:
 - All source files
- **Acceptance Criteria**:
 - [x] `cargo fmt -- --check` passes
 - [x] `cargo clippy --all-targets --all-features` passes
 - [x] `cargo doc --no-deps` passes
 - [x] `cargo deny check` passes
- **Notes**: Fixed missing `Tombstoned` case in `build_memory_from_row()` and clippy redundant_closure_for_method_calls warning.

### Phase 5 Deliverables

- [x] Git-notes code removed
- [x] Documentation updated (CLAUDE.md pre-existing, README updated)
- [x] Org-scope designed and feature-gated (pre-existing)
- [x] All CI checks pass

### Phase 5 Exit Criteria

- [x] `make ci` passes
- [x] No dead code (git-notes removed)
- [x] Documentation complete
- [x] Ready for release

---

## Dependency Graph

```
Phase 1: Foundation
├── Task 1.1: Context Detector (independent)
├── Task 1.2: Memory Struct (independent)
├── Task 1.3: MemoryStatus (independent)
├── Task 1.4: SearchFilter (independent)
├── Task 1.5: SQLite Migration (depends on 1.2)
├── Task 1.6: PostgreSQL Migration (depends on 1.2)
├── Task 1.7: SQLite Backend (depends on 1.2, 1.5)
└── Task 1.8: PostgreSQL Backend (depends on 1.2, 1.6)

Phase 2: Capture Path (depends on Phase 1)
├── Task 2.1: CaptureRequest (depends on Phase 1)
├── Task 2.2: Context Integration (depends on 2.1)
├── Task 2.3: Remove Git-Notes (depends on 2.2)
├── Task 2.4: ServiceContainer (depends on 2.3)
├── Task 2.5: MCP Handler (depends on 2.1)
└── Task 2.6: CLI Command (depends on 2.1)

Phase 3: Recall Path (depends on Phase 2)
├── Task 3.1: RecallService (depends on Phase 1)
├── Task 3.2: MCP Handler (depends on 3.1)
├── Task 3.3: CLI Command (depends on 3.1)
├── Task 3.4: URN Generation (independent)
└── Task 3.5: Tombstone Hints (depends on 3.1)

Phase 4: Garbage Collection (depends on Phase 3)
├── Task 4.1: GC Module (depends on Phase 1)
├── Task 4.2: get_distinct_branches (depends on 4.1)
├── Task 4.3: update_status (depends on 4.1)
├── Task 4.4: Lazy GC (depends on 4.1, Phase 3)
├── Task 4.5: CLI Command (depends on 4.1)
└── Task 4.6: MCP Tool (optional, depends on 4.1)

Phase 5: Cleanup (depends on all)
├── Task 5.1: Remove Git-Notes (depends on Phase 2)
├── Task 5.2: Evaluate git2 (depends on 5.1)
├── Task 5.3: CLAUDE.md (depends on Phase 3)
├── Task 5.4: README (depends on all)
├── Task 5.5: Org-Scope Design (independent)
├── Task 5.6: Test Suite (depends on all)
└── Task 5.7: CI Checks (depends on all)
```

## Risk Mitigation Tasks

| Risk | Mitigation Task | Phase |
|------|-----------------|-------|
| Capture regression | Comprehensive before/after tests | Phase 2 |
| Performance degradation | Benchmark comparison | Phase 5 |
| Schema migration failure | Test on existing database | Phase 1 |
| Orphaned git2 usage | Grep for all usages before removal | Phase 5 |

## Testing Checklist

- [x] Unit tests for Context Detector (edge cases)
- [x] Unit tests for Memory struct (serialization)
- [x] Unit tests for SearchFilter (query building)
- [x] Unit tests for BranchGarbageCollector
- [x] Integration tests for capture with facets
- [x] Integration tests for recall with facet filters
- [x] Integration tests for GC
- [x] E2E test: capture -> recall roundtrip
- [x] E2E test: MCP protocol roundtrip
- [x] Performance tests for capture latency
- [x] Performance tests for recall latency

## Documentation Tasks

- [x] Update CLAUDE.md with new CLI flags and query patterns (pre-existing)
- [x] Update README with architecture changes
- [x] Add inline rustdoc for new modules
- [x] Update MCP tool schemas

## Launch Checklist

- [x] All tests passing (`cargo test`)
- [x] All lints clean (`cargo clippy`)
- [x] Documentation complete
- [x] CI passing
- [x] Git-notes code removed
- [x] No capture regressions
- [x] No performance regressions

## Post-Launch

- [ ] Monitor for issues (24-48 hours)
- [ ] Gather feedback on new facet UX
- [ ] Update CLAUDE.md with learnings
- [ ] Archive planning documents to completed/
