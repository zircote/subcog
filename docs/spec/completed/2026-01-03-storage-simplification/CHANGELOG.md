# Changelog

All notable changes to this specification will be documented in this file.

## [COMPLETED] - 2026-01-03

### Project Closed
- Final status: **Success**
- Actual effort: ~10 hours (planned: 24-40 hours)
- Moved to: `docs/spec/completed/2026-01-03-storage-simplification/`
- Satisfaction: Very satisfied

### Retrospective Summary
- **What went well**: Rapid execution (9 hours vs 3 months), bottom-up approach eliminated rework, comprehensive code review (176 findings fixed), all CI gates passing
- **What to improve**: Run code review during implementation (not after), set realistic timelines based on velocity, integrate documentation inline
- **Scope changes**: Added 176 code review remediation tasks (7 CRITICAL, 30 HIGH, 77 MEDIUM, 62 LOW deferred)

### Final Deliverables
- 32 implementation tasks completed
- 176 code review findings addressed
- 896+ tests passing
- All clippy lints resolved
- PR #44 merged to `develop`

## [1.1.0] - 2026-01-03

### Completed
- **All 32 tasks complete** across 5 phases
- **921 tests passing** with full CI verification

### Phase 1: Foundation
- Created `src/context/` module with `GitContext` struct for project/branch detection
- Extended `Memory` struct with `project_id`, `branch`, `file_path`, `tombstoned_at` fields
- Added `Tombstoned` variant to `MemoryStatus` enum
- Extended `SearchFilter` with facet fields and `include_tombstoned` flag
- Created SQLite and PostgreSQL schema migrations for facet columns

### Phase 2: Capture Path
- Updated `CaptureRequest` with optional facet fields
- Integrated context detection in `CaptureService` (auto-detects project/branch from git)
- Removed git-notes code from `CaptureService` - SQLite is now single source of truth
- Updated MCP capture handler and CLI with facet parameters

### Phase 3: Recall Path
- Updated `RecallService` with facet filtering
- Added convenience methods: `search_in_project()`, `search_on_branch()`, etc.
- Updated MCP recall handler and CLI with facet filter flags
- Added `Memory::urn()` for faceted URN generation
- Added `TombstoneHint` for sparse result warnings

### Phase 4: Garbage Collection
- Created `src/gc/` module with `BranchGarbageCollector`
- Added `get_distinct_branches()` and `update_status()` to `IndexBackend` trait
- Integrated lazy GC in `RecallService` (checks branch existence during search)
- Created `subcog gc` CLI command with `--dry-run`, `--purge`, `--older-than` flags
- Created `subcog_gc` MCP tool

### Phase 5: Cleanup & Polish
- Removed git-notes module (`src/git/notes.rs`, `src/storage/persistence/git_notes.rs`, etc.)
- Evaluated git2 dependency - still required for context detection, branch GC, remote sync
- Updated README.md with SQLite architecture, faceted storage, branch GC documentation
- Verified org-scope already implemented with feature gate

### Status
- Moved to **Complete** - all tasks implemented and verified

## [1.0.0] - 2026-01-03

### Added
- **REQUIREMENTS.md**: Complete PRD with 10 P0, 8 P1, and 5 P2 requirements
- **ARCHITECTURE.md**: Technical design with 10 component specifications
- **IMPLEMENTATION_PLAN.md**: 5-phase plan with 32 tasks (24-40 hours estimated)
- **DECISIONS.md**: 7 Architecture Decision Records (ADRs)
- **RESEARCH_NOTES.md**: Codebase analysis and best practices research

### Key Decisions
- ADR-001: Remove git-notes storage layer (fixes critical capture bug)
- ADR-002: Consolidate to user-level storage with faceting
- ADR-003: Inline facet columns (denormalized)
- ADR-004: Fresh start - no migration of legacy data
- ADR-005: Feature-gate org-scope implementation
- ADR-006: Lazy branch garbage collection
- ADR-007: Tombstone pattern for soft deletes

### Status
- Moved to **In Review** - ready for stakeholder approval

## [0.1.0] - 2026-01-03

### Added
- Initial project creation from GitHub Issue #43
- Requirements elicitation completed
- Project workspace initialized at `docs/spec/active/2026-01-03-storage-simplification/`
