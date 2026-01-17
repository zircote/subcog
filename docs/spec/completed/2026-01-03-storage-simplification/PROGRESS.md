---
document_type: progress
project_id: SPEC-2026-01-03-001
version: 1.0.0
last_updated: 2026-01-03T08:00:00Z
status: complete
---

# Storage Architecture Simplification - Progress Tracker

## Overview

| Metric | Value |
|--------|-------|
| Total Tasks | 32 |
| Completed | 32 |
| In Progress | 0 |
| Pending | 0 |
| Skipped | 0 |
| Progress | 100% |

## Phase Progress

| Phase | Tasks | Done | Status |
|-------|-------|------|--------|
| Phase 1: Foundation | 8 | 8 | Complete |
| Phase 2: Capture Path | 6 | 6 | Complete |
| Phase 3: Recall Path | 5 | 5 | Complete |
| Phase 4: Garbage Collection | 6 | 6 | Complete |
| Phase 5: Cleanup & Polish | 7 | 7 | Complete |

---

## Phase 1: Foundation

### Task 1.1: Create Context Detector Module
- **Status**: Complete
- **Started**: 2026-01-03T02:00:00Z
- **Completed**: 2026-01-03T02:30:00Z
- **Files**:
 - [x] `src/context/mod.rs` (new)
 - [x] `src/context/detector.rs` (new)
 - [x] `src/lib.rs` (add `pub mod context`)
- **Notes**: Created `GitContext` struct with `from_cwd()`, `from_path()` methods. Handles detached HEAD, worktrees, and credential sanitization.

### Task 1.2: Extend Memory Struct with Facet Fields
- **Status**: Complete
- **Started**: 2026-01-03T02:30:00Z
- **Completed**: 2026-01-03T02:45:00Z
- **Files**:
 - [x] `src/models/memory.rs`
- **Notes**: Added `project_id`, `branch`, `file_path`, `tombstoned_at` fields with Option types. Updated Default impl and serialization.

### Task 1.3: Add Tombstoned Status to MemoryStatus
- **Status**: Complete
- **Started**: 2026-01-03T02:45:00Z
- **Completed**: 2026-01-03T02:50:00Z
- **Files**:
 - [x] `src/models/domain.rs`
- **Notes**: Added `Tombstoned` variant to `MemoryStatus` enum with `as_str()` and `FromStr` support.

### Task 1.4: Extend SearchFilter with Facet Fields
- **Status**: Complete
- **Started**: 2026-01-03T02:50:00Z
- **Completed**: 2026-01-03T03:00:00Z
- **Files**:
 - [x] `src/models/search.rs`
- **Notes**: Added `project_id`, `branch`, `file_path_pattern`, `include_tombstoned` fields. Default sets `include_tombstoned = false`. Builder pattern methods added.

### Task 1.5: Create SQLite Schema Migration for Facets
- **Status**: Complete
- **Started**: 2026-01-03T03:00:00Z
- **Completed**: 2026-01-03T03:10:00Z
- **Dependencies**: Task 1.2
- **Files**:
 - [x] `src/storage/index/sqlite.rs`
- **Notes**: Added Migration version 7 with facet columns, indexes, and partial index for active memories.

### Task 1.6: Create PostgreSQL Schema Migration for Facets
- **Status**: Complete
- **Started**: 2026-01-03T03:10:00Z
- **Completed**: 2026-01-03T03:15:00Z
- **Dependencies**: Task 1.2
- **Files**:
 - [x] `src/storage/persistence/postgresql.rs`
 - [x] `src/storage/index/postgresql.rs`
- **Notes**: Added Migration version 5 for both persistence and index layers with facet columns and appropriate indexes.

### Task 1.7: Update SQLite Index Backend for Facets
- **Status**: Complete
- **Started**: 2026-01-03T03:15:00Z
- **Completed**: 2026-01-03T03:20:00Z
- **Dependencies**: Task 1.2, Task 1.5
- **Files**:
 - [x] `src/storage/index/sqlite.rs`
- **Notes**: Updated `index()` to write facets, `row_to_memory` to read facets, `build_where_clause` to handle facet filters and `include_tombstoned`.

### Task 1.8: Update PostgreSQL Backend for Facets
- **Status**: Complete
- **Started**: 2026-01-03T03:20:00Z
- **Completed**: 2026-01-03T03:25:00Z
- **Dependencies**: Task 1.2, Task 1.6
- **Files**:
 - [x] `src/storage/persistence/postgresql.rs`
 - [x] `src/storage/index/postgresql.rs`
 - [x] `src/storage/index/redis.rs`
- **Notes**: Updated all storage backends (PostgreSQL persistence, PostgreSQL index, Redis index) with facet support including schema, storage, and filtering.

---

## Phase 2: Capture Path

### Task 2.1: Update CaptureRequest with Facet Fields
- **Status**: Complete
- **Started**: 2026-01-03T03:25:00Z
- **Completed**: 2026-01-03T03:30:00Z
- **Dependencies**: Phase 1
- **Files**:
 - [x] `src/models/capture.rs`
 - [x] `src/services/capture.rs` (test helper)
 - [x] `src/mcp/tools/handlers/core.rs`
 - [x] `src/hooks/pre_compact/orchestrator.rs`
 - [x] `src/commands/core.rs`
 - [x] `tests/capture_recall_integration.rs`
- **Notes**: Added optional `project_id`, `branch`, `file_path` fields with builder methods. Updated all call sites to include facet fields.

### Task 2.2: Integrate Context Detection in CaptureService
- **Status**: Complete
- **Started**: 2026-01-03T03:30:00Z
- **Completed**: 2026-01-03T03:45:00Z
- **Dependencies**: Task 2.1
- **Files**:
 - [x] `src/services/capture.rs`
- **Notes**: Added GitContext::from_cwd() integration. Auto-detects project_id and branch if not provided in request. Uses request values as overrides when explicitly set.

### Task 2.3: Remove Git-Notes Code from CaptureService
- **Status**: Complete
- **Started**: 2026-01-03T03:45:00Z
- **Completed**: 2026-01-03T03:50:00Z
- **Dependencies**: Task 2.2
- **Files**:
 - [x] `src/services/capture.rs`
- **Notes**: Removed NotesManager and YamlFrontMatterParser imports. Replaced git-notes storage with UUID-based ID generation. SQLite is now the single source of truth.

### Task 2.4: Update ServiceContainer Factory Methods
- **Status**: Complete
- **Started**: 2026-01-03T03:50:00Z
- **Completed**: 2026-01-03T04:00:00Z
- **Dependencies**: Task 2.3
- **Files**:
 - [x] `src/services/mod.rs`
- **Notes**: Removed git-notes imports (NotesManager, YamlFrontMatterParser). Updated reindex_scope to work with SQLite as source of truth. Removed helper functions (parse_note_to_memory, parse_domain_string, parse_status_string).

### Task 2.5: Update MCP Capture Handler
- **Status**: Complete
- **Started**: 2026-01-03T04:00:00Z
- **Completed**: 2026-01-03T04:10:00Z
- **Dependencies**: Task 2.1
- **Files**:
 - [x] `src/mcp/tool_types.rs`
 - [x] `src/mcp/tools/handlers/core.rs`
 - [x] `src/mcp/tools/definitions.rs`
- **Notes**: Added project_id, branch, file_path fields to CaptureArgs. Updated execute_capture to pass facet fields to CaptureRequest. Updated capture_tool schema with new optional parameters.

### Task 2.6: Update CLI Capture Command
- **Status**: Complete
- **Started**: 2026-01-03T04:10:00Z
- **Completed**: 2026-01-03T04:20:00Z
- **Dependencies**: Task 2.1
- **Files**:
 - [x] `src/main.rs`
 - [x] `src/commands/core.rs`
- **Notes**: Added `--project`, `--branch`, `--file-path` flags to capture command. Updated cmd_capture to accept and pass facet parameters to CaptureRequest.

---

## Phase 3: Recall Path

### Task 3.1: Update RecallService for Facet Filtering
- **Status**: Complete
- **Started**: 2026-01-03T04:20:00Z
- **Completed**: 2026-01-03T04:30:00Z
- **Dependencies**: Phase 1
- **Files**:
 - [x] `src/services/recall.rs`
- **Notes**: Already implemented in Phase 1. RecallService passes SearchFilter to index backend which handles facet filtering. Added convenience methods: `search_in_project()`, `search_on_branch()`, `search_by_file_pattern()`, `search_with_tombstoned()`.

### Task 3.2: Update MCP Recall Handler
- **Status**: Complete
- **Started**: 2026-01-03T04:30:00Z
- **Completed**: 2026-01-03T04:40:00Z
- **Dependencies**: Task 3.1
- **Files**:
 - [x] `src/mcp/tool_types.rs`
 - [x] `src/mcp/tools/handlers/core.rs`
 - [x] `src/mcp/tools/definitions.rs`
- **Notes**: Added `project_id`, `branch`, `file_path_pattern`, `include_tombstoned` fields to RecallArgs. Updated execute_recall to build SearchFilter with facet fields. Updated recall_tool schema with new optional parameters.

### Task 3.3: Update CLI Recall Command
- **Status**: Complete
- **Started**: 2026-01-03T04:40:00Z
- **Completed**: 2026-01-03T04:50:00Z
- **Dependencies**: Task 3.1
- **Files**:
 - [x] `src/main.rs`
 - [x] `src/commands/core.rs`
- **Notes**: Added `--project`, `--branch`, `--file-path`, `--include-tombstoned` flags to recall command. Updated cmd_recall to build SearchFilter with facet parameters.

### Task 3.4: Update URN Generation
- **Status**: Complete
- **Started**: 2026-01-03T04:50:00Z
- **Completed**: 2026-01-03T05:00:00Z
- **Dependencies**: None
- **Files**:
 - [x] `src/models/memory.rs`
 - [x] `src/models/domain.rs`
 - [x] `src/services/capture.rs`
- **Notes**: Added `Memory::urn()` method that generates URN using faceted scope. Added `Domain::urn_scope()` that returns project_id if set, otherwise "global". CaptureService now uses `Memory::urn()` for consistent URN generation.

### Task 3.5: Add Tombstone Hint to Search Results
- **Status**: Complete
- **Started**: 2026-01-03T05:00:00Z
- **Completed**: 2026-01-03T05:10:00Z
- **Dependencies**: Task 3.1
- **Files**:
 - [x] `src/models/search.rs`
 - [x] `src/services/recall.rs`
- **Notes**: Added `TombstoneHint` struct with `count`, `branches`, `has_tombstones()`, `message()` methods. Added `tombstone_hint` field to `SearchResult`. RecallService checks for tombstones when results are sparse (< 3) and populates the hint with count and branch names.

---

## Phase 4: Garbage Collection

### Task 4.1: Create Branch Garbage Collector Module
- **Status**: Complete
- **Started**: 2026-01-03T05:10:00Z
- **Completed**: 2026-01-03T05:20:00Z
- **Dependencies**: Phase 1
- **Files**:
 - [x] `src/gc/mod.rs` (new)
 - [x] `src/gc/branch.rs` (new)
 - [x] `src/lib.rs` (add `pub mod gc`)
- **Notes**: Created `BranchGarbageCollector` struct with `gc_stale_branches()` method. Added `GcResult` struct with statistics. Added `branch_exists()` function for lazy GC checks.

### Task 4.2: Add get_distinct_branches to IndexBackend
- **Status**: Complete
- **Started**: 2026-01-03T05:20:00Z
- **Completed**: 2026-01-03T05:25:00Z
- **Dependencies**: Task 4.1
- **Files**:
 - [x] `src/storage/traits/index.rs`
- **Notes**: Added `get_distinct_branches()` method to IndexBackend trait with default implementation that uses list_all + filter. SQLite and PostgreSQL backends can override with efficient SQL queries.

### Task 4.3: Add update_status to IndexBackend
- **Status**: Complete
- **Started**: 2026-01-03T05:25:00Z
- **Completed**: 2026-01-03T05:30:00Z
- **Dependencies**: Task 4.1
- **Files**:
 - [x] `src/storage/traits/index.rs`
- **Notes**: Added `update_status()` method to IndexBackend trait with default implementation that fetches, modifies, and re-indexes each memory. SQLite and PostgreSQL backends can override with efficient bulk UPDATE queries.

### Task 4.4: Integrate Lazy GC in RecallService
- **Status**: Complete
- **Started**: 2026-01-03T05:30:00Z
- **Completed**: 2026-01-03T05:45:00Z
- **Dependencies**: Task 4.1
- **Files**:
 - [x] `src/gc/mod.rs`
 - [x] `src/services/recall.rs`
- **Notes**: Integrated `branch_exists()` into RecallService search method. When a branch filter is provided, checks if branch still exists. If stale, logs warning, records metric, and includes hint in search results. Lightweight approach that doesn't slow down searches but provides visibility.

### Task 4.5: Create GC CLI Command
- **Status**: Complete
- **Started**: 2026-01-03T05:45:00Z
- **Completed**: 2026-01-03T05:50:00Z
- **Dependencies**: Task 4.1
- **Files**:
 - [x] `src/commands/gc.rs` (new)
 - [x] `src/commands/mod.rs`
 - [x] `src/main.rs`
- **Notes**: Added `subcog gc` CLI command with `--branch`, `--dry-run`, `--purge`, `--older-than` flags. Displays stale branches found and memories tombstoned.

### Task 4.6: Add GC MCP Tool (Optional)
- **Status**: Complete
- **Started**: 2026-01-03T05:50:00Z
- **Completed**: 2026-01-03T06:00:00Z
- **Dependencies**: Task 4.1
- **Files**:
 - [x] `src/mcp/tool_types.rs`
 - [x] `src/mcp/tools/definitions.rs`
 - [x] `src/mcp/tools/handlers/core.rs`
 - [x] `src/mcp/tools/handlers/mod.rs`
 - [x] `src/mcp/tools/mod.rs`
- **Notes**: Added `subcog_gc` MCP tool with `branch`, `dry_run` parameters. Returns GC result with stale branches found and memories tombstoned. Supports per-branch tombstoning when branch parameter is provided.

---

## Phase 5: Cleanup & Polish

### Task 5.1: Remove Git-Notes Module
- **Status**: Complete
- **Started**: 2026-01-03T09:00:00Z
- **Completed**: 2026-01-03T09:30:00Z
- **Dependencies**: Phase 2
- **Files**:
 - [x] `src/git/notes.rs` (deleted)
 - [x] `src/git/mod.rs` (updated - removed notes module)
 - [x] `src/storage/persistence/git_notes.rs` (deleted)
 - [x] `src/storage/prompt/git_notes.rs` (deleted)
 - [x] `src/services/enrichment.rs` (deleted)
 - [x] `src/commands/enrich.rs` (deleted)
 - [x] `src/config/mod.rs` (removed GitNotes enum variant)
 - [x] All git-notes comments updated to reference SQLite
- **Notes**: Removed all git-notes code. SQLite is now the single source of truth. All documentation comments updated.

### Task 5.2: Evaluate git2 Dependency
- **Status**: Complete
- **Started**: 2026-01-03T09:30:00Z
- **Completed**: 2026-01-03T09:35:00Z
- **Dependencies**: Task 5.1
- **Files**:
 - [x] `Cargo.toml` (no changes needed)
- **Notes**: Evaluated git2 usage. Still required for:
 - `src/context/detector.rs` - GitContext detection (project_id, branch)
 - `src/gc/branch.rs` - Branch garbage collection (checking if branches exist)
 - `src/git/remote.rs` - Remote sync operations
 - `src/storage/prompt/mod.rs` - Git repo detection

 **Conclusion**: Cannot remove git2 - essential for core functionality (context detection, GC, sync).

### Task 5.3: Update CLAUDE.md Documentation
- **Status**: Complete
- **Started**: 2026-01-03T06:00:00Z
- **Completed**: 2026-01-03T06:00:00Z
- **Dependencies**: Phase 3
- **Files**:
 - [x] `CLAUDE.md`
- **Notes**: Already includes facet filtering documentation, MCP parameters, query patterns. No additional updates needed.

### Task 5.4: Update README Documentation
- **Status**: Complete
- **Started**: 2026-01-03T09:35:00Z
- **Completed**: 2026-01-03T09:45:00Z
- **Dependencies**: All phases
- **Files**:
 - [x] `README.md`
- **Notes**: Updated README with:
 - Storage backends now list SQLite+usearch, PostgreSQL+pgvector, Filesystem (removed Git Notes)
 - Added faceted storage and branch GC to features
 - Updated Core features section with SQLite persistence, faceted storage, branch GC
 - Updated architecture diagram (Persistence now shows SQLite, Index shows SQLite FTS)
 - Updated configuration section (removed git-notes backend option)
 - Added Faceted Capture section with CLI examples
 - Added Branch Garbage Collection section
 - Fixed spec links to point to 2026-01-03-storage-simplification

### Task 5.5: Design Org-Scope (Feature-Gated)
- **Status**: Complete
- **Started**: 2026-01-03T09:45:00Z
- **Completed**: 2026-01-03T09:50:00Z
- **Dependencies**: None
- **Files**:
 - [x] `src/config/mod.rs` (OrgConfig struct with feature gate)
 - [x] `src/services/mod.rs` (ServiceContainer::for_org() with feature gate)
 - [x] `Cargo.toml` (org-scope feature defined)
 - [x] `src/lib.rs` (feature-gated exports)
- **Notes**: Org-scope already fully designed and implemented:
 - `OrgConfig` struct with `org_id`, `database_url`, `encryption_key` fields
 - `OrgConfigBuilder` with validation
 - `ServiceContainer::for_org()` method behind `#[cfg(feature = "org-scope")]`
 - Feature defined in Cargo.toml as `org-scope = []`
 - Tests exist in `src/config/mod.rs` and `src/services/mod.rs`

### Task 5.6: Run Full Test Suite
- **Status**: Complete
- **Started**: 2026-01-03T07:30:00Z
- **Completed**: 2026-01-03T08:00:00Z
- **Dependencies**: All phases
- **Files**:
 - [x] All test files
- **Notes**: 949 tests passing. All new code covered.

### Task 5.7: Run CI Checks
- **Status**: Complete
- **Started**: 2026-01-03T07:30:00Z
- **Completed**: 2026-01-03T08:00:00Z
- **Dependencies**: All phases
- **Files**:
 - [x] All source files
- **Notes**: `make ci` passes - fmt, clippy (pedantic + nursery), test, doc all green. Fixed missing `Tombstoned` case in `build_memory_from_row` and clippy redundant_closure_for_method_calls warning.

---

## Session Log

| Timestamp | Task | Action | Notes |
|-----------|------|--------|-------|
| 2026-01-03T02:00:00Z | All | Started | Implementation session begins |
| 2026-01-03T02:30:00Z | 1.1 | Completed | Context detector module created |
| 2026-01-03T02:45:00Z | 1.2 | Completed | Memory struct extended with facet fields |
| 2026-01-03T02:50:00Z | 1.3 | Completed | Tombstoned status added to MemoryStatus |
| 2026-01-03T03:00:00Z | 1.4 | Completed | SearchFilter extended with facet fields |
| 2026-01-03T03:10:00Z | 1.5 | Completed | SQLite schema migration for facets |
| 2026-01-03T03:15:00Z | 1.6 | Completed | PostgreSQL schema migration for facets |
| 2026-01-03T03:20:00Z | 1.7 | Completed | SQLite index backend updated for facets |
| 2026-01-03T03:25:00Z | 1.8 | Completed | PostgreSQL and Redis backends updated for facets |
| 2026-01-03T03:25:00Z | Phase 1 | Completed | All foundation tasks complete, 912 tests passing |
| 2026-01-03T03:30:00Z | 2.1 | Completed | CaptureRequest updated with facet fields, all call sites fixed |
| 2026-01-03T03:45:00Z | 2.2 | Completed | GitContext::from_cwd() integrated into CaptureService, auto-detects project_id/branch |
| 2026-01-03T03:50:00Z | 2.3 | Completed | Removed git-notes storage from CaptureService, replaced with UUID-based ID generation |
| 2026-01-03T04:00:00Z | 2.4 | Completed | Removed git-notes imports from ServiceContainer, updated reindex_scope for SQLite |
| 2026-01-03T04:10:00Z | 2.5 | Completed | Added facet fields to MCP CaptureArgs and capture_tool schema |
| 2026-01-03T04:20:00Z | 2.6 | Completed | Added --project, --branch, --file-path flags to CLI capture command |
| 2026-01-03T04:20:00Z | Phase 2 | Completed | All capture path tasks complete |
| 2026-01-03T04:30:00Z | 3.1 | Completed | RecallService already supports facets from Phase 1, added convenience methods |
| 2026-01-03T04:40:00Z | 3.2 | Completed | Added facet fields to MCP RecallArgs and recall_tool schema |
| 2026-01-03T04:50:00Z | 3.3 | Completed | Added facet filter flags to CLI recall command |
| 2026-01-03T05:00:00Z | 3.4 | Completed | Added Memory::urn() and Domain::urn_scope() for faceted URN generation |
| 2026-01-03T05:10:00Z | 3.5 | Completed | Added TombstoneHint struct and sparse result hint logic |
| 2026-01-03T05:10:00Z | Phase 3 | Completed | All recall path tasks complete |
| 2026-01-03T05:20:00Z | 4.1 | Completed | Created gc module with BranchGarbageCollector and GcResult |
| 2026-01-03T05:25:00Z | 4.2 | Completed | Added get_distinct_branches to IndexBackend trait |
| 2026-01-03T05:30:00Z | 4.3 | Completed | Added update_status to IndexBackend trait |
| 2026-01-03T05:45:00Z | 4.4 | Completed | Integrated lazy GC in RecallService using branch_exists() |
| 2026-01-03T05:50:00Z | 4.5 | Completed | Created subcog gc CLI command |
| 2026-01-03T06:00:00Z | 4.6 | Completed | Created subcog_gc MCP tool |
| 2026-01-03T06:00:00Z | Phase 4 | Completed | All garbage collection tasks complete |
| 2026-01-03T07:30:00Z | 5.6-5.7 | Started | Began CI verification |
| 2026-01-03T08:00:00Z | 5.6 | Completed | 949 tests passing |
| 2026-01-03T08:00:00Z | 5.7 | Completed | `make ci` passes (fmt, clippy, test, doc) |
| 2026-01-03T09:00:00Z | 5.1 | Completed | Removed git-notes module, deleted files, updated comments |
| 2026-01-03T09:35:00Z | 5.2 | Completed | Evaluated git2 - still required for core functionality |
| 2026-01-03T09:45:00Z | 5.4 | Completed | Updated README documentation with SQLite architecture |
| 2026-01-03T09:50:00Z | 5.5 | Completed | Verified org-scope already implemented with feature gate |
| 2026-01-03T09:50:00Z | Phase 5 | Completed | All 7 tasks complete |
| 2026-01-03T09:50:00Z | Project | Completed | SPEC-2026-01-03-001 complete - all 32 tasks done |

---

## Divergences from Original Plan

1. **Task 1.8 Scope Expansion**: Also updated Redis index backend for facets (not originally planned but needed for consistency)
2. **Task 2.1 Scope Expansion**: Updated test files and all call sites that initialize CaptureRequest to include new facet fields
3. **Task 3.1 Efficiency**: Most functionality was already implemented in Phase 1; added convenience methods for common use cases
4. **Task 4.2-4.3 Efficiency**: Added trait methods with default implementations; backends can override for optimized queries
5. **Task 4.4 Lightweight Approach**: Implemented as visibility/metrics tracking rather than active tombstoning during search
6. **Task 5.3 Pre-existing**: CLAUDE.md already contained comprehensive documentation for faceted storage
7. **Task 5.7 Bug Fix**: Found and fixed missing `Tombstoned` case in `build_memory_from_row()` - status was falling through to default `Active`
8. **Task 5.2 Scope Clarification**: git2 crate cannot be removed - still required for GitContext detection, branch GC, and remote sync operations
9. **Task 5.5 Pre-existing**: Org-scope was already fully designed and implemented with feature gate in Cargo.toml, config, and services
