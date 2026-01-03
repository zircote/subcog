---
document_type: progress
project_id: SPEC-2026-01-03-001
version: 1.0.0
last_updated: 2026-01-03T03:30:00Z
status: in_progress
---

# Storage Architecture Simplification - Progress Tracker

## Overview

| Metric | Value |
|--------|-------|
| Total Tasks | 32 |
| Completed | 9 |
| In Progress | 1 |
| Pending | 22 |
| Skipped | 0 |
| Progress | 28% |

## Phase Progress

| Phase | Tasks | Done | Status |
|-------|-------|------|--------|
| Phase 1: Foundation | 8 | 8 | ✅ Complete |
| Phase 2: Capture Path | 6 | 1 | 🔄 In Progress |
| Phase 3: Recall Path | 5 | 0 | ⏳ Pending |
| Phase 4: Garbage Collection | 6 | 0 | ⏳ Pending |
| Phase 5: Cleanup & Polish | 7 | 0 | ⏳ Pending |

---

## Phase 1: Foundation

### Task 1.1: Create Context Detector Module
- **Status**: ✅ Complete
- **Started**: 2026-01-03T02:00:00Z
- **Completed**: 2026-01-03T02:30:00Z
- **Files**:
  - [x] `src/context/mod.rs` (new)
  - [x] `src/context/detector.rs` (new)
  - [x] `src/lib.rs` (add `pub mod context`)
- **Notes**: Created `GitContext` struct with `from_cwd()`, `from_path()` methods. Handles detached HEAD, worktrees, and credential sanitization.

### Task 1.2: Extend Memory Struct with Facet Fields
- **Status**: ✅ Complete
- **Started**: 2026-01-03T02:30:00Z
- **Completed**: 2026-01-03T02:45:00Z
- **Files**:
  - [x] `src/models/memory.rs`
- **Notes**: Added `project_id`, `branch`, `file_path`, `tombstoned_at` fields with Option types. Updated Default impl and serialization.

### Task 1.3: Add Tombstoned Status to MemoryStatus
- **Status**: ✅ Complete
- **Started**: 2026-01-03T02:45:00Z
- **Completed**: 2026-01-03T02:50:00Z
- **Files**:
  - [x] `src/models/domain.rs`
- **Notes**: Added `Tombstoned` variant to `MemoryStatus` enum with `as_str()` and `FromStr` support.

### Task 1.4: Extend SearchFilter with Facet Fields
- **Status**: ✅ Complete
- **Started**: 2026-01-03T02:50:00Z
- **Completed**: 2026-01-03T03:00:00Z
- **Files**:
  - [x] `src/models/search.rs`
- **Notes**: Added `project_id`, `branch`, `file_path_pattern`, `include_tombstoned` fields. Default sets `include_tombstoned = false`. Builder pattern methods added.

### Task 1.5: Create SQLite Schema Migration for Facets
- **Status**: ✅ Complete
- **Started**: 2026-01-03T03:00:00Z
- **Completed**: 2026-01-03T03:10:00Z
- **Dependencies**: Task 1.2
- **Files**:
  - [x] `src/storage/index/sqlite.rs`
- **Notes**: Added Migration version 7 with facet columns, indexes, and partial index for active memories.

### Task 1.6: Create PostgreSQL Schema Migration for Facets
- **Status**: ✅ Complete
- **Started**: 2026-01-03T03:10:00Z
- **Completed**: 2026-01-03T03:15:00Z
- **Dependencies**: Task 1.2
- **Files**:
  - [x] `src/storage/persistence/postgresql.rs`
  - [x] `src/storage/index/postgresql.rs`
- **Notes**: Added Migration version 5 for both persistence and index layers with facet columns and appropriate indexes.

### Task 1.7: Update SQLite Index Backend for Facets
- **Status**: ✅ Complete
- **Started**: 2026-01-03T03:15:00Z
- **Completed**: 2026-01-03T03:20:00Z
- **Dependencies**: Task 1.2, Task 1.5
- **Files**:
  - [x] `src/storage/index/sqlite.rs`
- **Notes**: Updated `index()` to write facets, `row_to_memory` to read facets, `build_where_clause` to handle facet filters and `include_tombstoned`.

### Task 1.8: Update PostgreSQL Backend for Facets
- **Status**: ✅ Complete
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
- **Status**: ✅ Complete
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
- **Status**: 🔄 In Progress
- **Started**: 2026-01-03T03:30:00Z
- **Completed**: -
- **Dependencies**: Task 2.1
- **Files**:
  - [ ] `src/services/capture.rs`
- **Notes**: Auto-detect facets if not provided

### Task 2.3: Remove Git-Notes Code from CaptureService
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: Task 2.2
- **Files**:
  - [ ] `src/services/capture.rs`
- **Notes**: Remove git-notes storage path, generate UUID-based IDs

### Task 2.4: Update ServiceContainer Factory Methods
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: Task 2.3
- **Files**:
  - [ ] `src/services/mod.rs`
- **Notes**: Remove `repo_path` requirement, simplify factory

### Task 2.5: Update MCP Capture Handler
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: Task 2.1
- **Files**:
  - [ ] `src/mcp/tools/handlers/mod.rs`
  - [ ] `src/mcp/tools/definitions.rs`
- **Notes**: Add facet overrides to `CaptureArgs`

### Task 2.6: Update CLI Capture Command
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: Task 2.1
- **Files**:
  - [ ] `src/cli/capture.rs`
- **Notes**: Add `--project`, `--branch`, `--path` flags

---

## Phase 3: Recall Path

### Task 3.1: Update RecallService for Facet Filtering
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: Phase 1
- **Files**:
  - [ ] `src/services/recall.rs`
- **Notes**: Apply facet filters, exclude tombstoned by default

### Task 3.2: Update MCP Recall Handler
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: Task 3.1
- **Files**:
  - [ ] `src/mcp/tools/handlers/mod.rs`
  - [ ] `src/mcp/tools/definitions.rs`
- **Notes**: Add facet filter parameters

### Task 3.3: Update CLI Recall Command
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: Task 3.1
- **Files**:
  - [ ] `src/cli/recall.rs`
- **Notes**: Add facet filter and tombstone flags

### Task 3.4: Update URN Generation
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: None
- **Files**:
  - [ ] `src/services/capture.rs`
- **Notes**: Update URN scheme for faceted model

### Task 3.5: Add Tombstone Hint to Search Results
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: Task 3.1
- **Files**:
  - [ ] `src/services/recall.rs`
- **Notes**: Hint when tombstones may be relevant

---

## Phase 4: Garbage Collection

### Task 4.1: Create Branch Garbage Collector Module
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: Phase 1
- **Files**:
  - [ ] `src/gc/mod.rs` (new)
  - [ ] `src/gc/branch.rs` (new)
  - [ ] `src/lib.rs` (add `pub mod gc`)
- **Notes**: `BranchGarbageCollector` struct

### Task 4.2: Add get_distinct_branches to IndexBackend
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: Task 4.1
- **Files**:
  - [ ] `src/storage/traits/index.rs`
  - [ ] `src/storage/index/sqlite.rs`
  - [ ] `src/storage/index/postgresql.rs`
- **Notes**: Trait method for unique branches

### Task 4.3: Add update_status to IndexBackend
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: Task 4.1
- **Files**:
  - [ ] `src/storage/traits/index.rs`
  - [ ] `src/storage/index/sqlite.rs`
  - [ ] `src/storage/index/postgresql.rs`
- **Notes**: Bulk status update method

### Task 4.4: Integrate Lazy GC in RecallService
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: Task 4.1
- **Files**:
  - [ ] `src/services/recall.rs`
- **Notes**: Opportunistic GC on recall

### Task 4.5: Create GC CLI Command
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: Task 4.1
- **Files**:
  - [ ] `src/cli/gc.rs` (new)
  - [ ] `src/cli/mod.rs`
  - [ ] `src/main.rs`
- **Notes**: `subcog gc` command with flags

### Task 4.6: Add GC MCP Tool (Optional)
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: Task 4.1
- **Files**:
  - [ ] `src/mcp/tools/handlers/mod.rs`
  - [ ] `src/mcp/tools/definitions.rs`
- **Notes**: Optional `subcog_gc` MCP tool

---

## Phase 5: Cleanup & Polish

### Task 5.1: Remove Git-Notes Module
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: Phase 2
- **Files**:
  - [ ] `src/git/notes.rs` (delete)
  - [ ] `src/git/mod.rs`
  - [ ] `src/storage/persistence/git_notes.rs` (delete)
  - [ ] `src/storage/prompt/git_notes.rs` (delete)
- **Notes**: Delete files, remove from module tree

### Task 5.2: Evaluate git2 Dependency
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: Task 5.1
- **Files**:
  - [ ] `Cargo.toml`
- **Notes**: Check if git2 still needed for context detection

### Task 5.3: Update CLAUDE.md Documentation
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: Phase 3
- **Files**:
  - [ ] `CLAUDE.md`
- **Notes**: New CLI flags, MCP parameters, query patterns

### Task 5.4: Update README Documentation
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: All phases
- **Files**:
  - [ ] `README.md`
- **Notes**: Architecture changes, CLI usage

### Task 5.5: Design Org-Scope (Feature-Gated)
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: None
- **Files**:
  - [ ] `src/config/mod.rs`
  - [ ] `src/services/mod.rs`
  - [ ] `Cargo.toml`
- **Notes**: `OrgConfig`, feature-gated `for_org()`

### Task 5.6: Run Full Test Suite
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: All phases
- **Files**:
  - [ ] All test files
- **Notes**: Coverage >90% for new code

### Task 5.7: Run CI Checks
- **Status**: ⏳ Pending
- **Started**: -
- **Completed**: -
- **Dependencies**: All phases
- **Files**:
  - [ ] All source files
- **Notes**: fmt, clippy, doc, deny

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

---

## Divergences from Original Plan

1. **Task 1.8 Scope Expansion**: Also updated Redis index backend for facets (not originally planned but needed for consistency)
2. **Task 2.1 Scope Expansion**: Updated test files and all call sites that initialize CaptureRequest to include new facet fields
