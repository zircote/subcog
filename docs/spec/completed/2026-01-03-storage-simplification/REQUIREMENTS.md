---
document_type: requirements
project_id: SPEC-2026-01-03-001
version: 1.0.0
last_updated: 2026-01-03T01:00:00Z
status: draft
---

# Storage Architecture Simplification - Product Requirements Document

## Executive Summary

This specification defines the requirements for fundamentally simplifying subcog's storage architecture by removing git-notes as a persistence layer and consolidating to user-level storage with project/branch/path faceting. This change eliminates the complexity of per-repository git-notes storage (which contains a critical capture bug) while preserving project context through auto-detected facets.

**Key Outcomes:**
- Fix critical capture bug (all notes overwrite each other)
- Reduce storage complexity from 3 active tiers to 1
- Enable cross-project learning via faceted queries
- Improve backup and sync story (single location)
- Maintain full backward compatibility for API consumers

## Problem Statement

### The Problem

The current storage architecture has a **critical capture bug** and design mismatch:

1. **Critical Bug** (`src/services/capture.rs:198`): All captures attach to HEAD with `force=true`, causing each new memory to **overwrite** the previous one. The 645 existing notes were created by earlier tooling using blob-based storage.

2. **Design Mismatch**: Git notes are designed to annotate commits, not store arbitrary key-value data. The workaround (creating unique marker blobs) fights against git's design.

3. **Complexity**: Three storage tiers (org/project/user) create maintenance burden and sync complexity.

4. **Silos**: Per-repository storage prevents cross-project learning - insights from one project can't inform another.

### Impact

| Stakeholder | Impact |
|-------------|--------|
| End Users | Losing memories on every capture (critical) |
| Developers | Complex codebase with broken functionality |
| Operators | Multiple storage locations to backup/maintain |

### Current State

```
Current Architecture (broken):
┌─────────────────────────────────────────────────────────────┐
│ org/ -> configured, shared (works but unused) │
│ project/ -> git notes per repo (BROKEN - overwrites) │
│ user/ -> ~/.config/subcog/ (works) │
└─────────────────────────────────────────────────────────────┘
```

## Goals and Success Criteria

### Primary Goal

Eliminate git-notes storage and consolidate to user-level storage with project/branch/path facets, fixing the capture bug and simplifying the architecture.

### Success Metrics

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Capture reliability | 100% success rate | Integration tests |
| No data overwrites | 0 overwrites | Before/after counts |
| Single storage location | 1 active tier | Code inspection |
| API compatibility | No breaking changes | Existing test suite |
| Performance maintained | <50ms capture/recall | Benchmark suite |

### Non-Goals (Explicit Exclusions)

- **Migration tooling**: Legacy git-notes are artifacts from earlier tooling - fresh start approach
- **Remote sync for user scope**: Out of scope for this version (local SQLite only)
- **Org-scope implementation**: Design included, feature-gated for future
- **Cross-scope search**: Searching user + project simultaneously (future enhancement)

## User Analysis

### Primary Users

- **Who**: AI coding assistants (Claude Code) and developers using CLI
- **Needs**: Reliable memory capture/recall with project context awareness
- **Context**: Working in git repositories or non-git directories

### User Stories

1. As a **developer using subcog CLI**, I want my memories to be reliably captured without overwrites, so that I don't lose important decisions and learnings.

2. As a **Claude Code integration**, I want memories auto-tagged with project context (git remote, branch, path), so that recalls return contextually relevant results.

3. As an **operator**, I want a single storage location to backup, so that disaster recovery is simple.

4. As a **developer working on multiple projects**, I want to query memories across all projects or filter to a specific one, so that learnings transfer between similar codebases.

5. As a **developer on a feature branch**, I want memories captured during that branch to be tombstoned when the branch is deleted, so that stale context doesn't pollute recalls.

## Functional Requirements

### Must Have (P0)

| ID | Requirement | Rationale | Acceptance Criteria |
|----|-------------|-----------|---------------------|
| FR-001 | Remove git-notes persistence layer | Eliminate capture bug | `src/git/notes.rs` deleted, no git2 references in capture path |
| FR-002 | Add project/branch/path facets to Memory struct | Enable context-aware queries | `Memory` has `project_id`, `branch`, `file_path` fields |
| FR-003 | Auto-detect facets from git context | Seamless UX | Detects `git remote`, `git branch`, relative path automatically |
| FR-004 | Add facet columns to SQLite schema | Query by facets | Schema migration adds indexed columns |
| FR-005 | Add facet columns to PostgreSQL schema | Query by facets | Migration adds indexed columns |
| FR-006 | Update CaptureService to use configured backend | Fix capture path | No git-notes code in capture flow |
| FR-007 | Add facet filter flags to recall CLI | User-controlled queries | `--project`, `--branch`, `--path` flags work |
| FR-008 | Update MCP tool handlers for facets | API compatibility | `subcog_capture` accepts optional facet overrides |
| FR-009 | Implement context detection module | Centralize git detection | `src/context/detector.rs` provides facets |
| FR-010 | Update URN scheme for faceted model | Clear scope indicator | `subcog://{scope}/{namespace}/{id}` works |

### Should Have (P1)

| ID | Requirement | Rationale | Acceptance Criteria |
|----|-------------|-----------|---------------------|
| FR-101 | Implement branch garbage collection | Clean stale data | Tombstones memories for deleted branches |
| FR-102 | Add `subcog gc` CLI command | Manual cleanup | Command works with `--branch`, `--dry-run` flags |
| FR-103 | Lazy GC during recall | Automatic cleanup | Opportunistic GC on each recall |
| FR-104 | Add tombstone status to MemoryStatus | Soft delete | `MemoryStatus::Tombstoned` variant |
| FR-105 | Implement tombstone retention policy | Prevent accidental data loss | Tombstones hidden by default, never auto-deleted |
| FR-106 | Add `--include-tombstoned` flag | Archaeology use case | Recall can show tombstoned memories |
| FR-107 | Design org-scope storage | Future proofing | Code architecture supports org, feature-gated |
| FR-108 | Add automatic tombstone prompting | Discoverability | Hint when tombstones may be relevant |

### Nice to Have (P2)

| ID | Requirement | Rationale | Acceptance Criteria |
|----|-------------|-----------|---------------------|
| FR-201 | Git hook for immediate branch GC | Faster cleanup | Optional `reference-transaction` hook |
| FR-202 | Cross-project query mode | Power users | `--all-projects` flag |
| FR-203 | Path pattern matching | Monorepo support | `--path=services/auth/**` glob syntax |
| FR-204 | Tombstone purge command | Storage reclamation | `subcog gc --purge --older-than=30d` |
| FR-205 | Update pgvector schema for facets | Vector search scoping | `project_id`, `branch` columns |

## Non-Functional Requirements

### Performance

| Metric | Target | Rationale |
|--------|--------|-----------|
| Capture latency | <50ms P99 | Current baseline |
| Recall latency | <100ms P99 | Current baseline |
| GC overhead per recall | <10ms | Should not impact UX |
| Cold start | <10ms | Single-binary constraint |

### Data Integrity

| Requirement | Description |
|-------------|-------------|
| ACID transactions | All writes atomic (SQLite/PostgreSQL) |
| No data loss on capture | Every capture persists successfully or returns error |
| Graceful degradation | If facet detection fails, capture still succeeds |
| Tombstone safety | Tombstones are never auto-purged |

### Security

| Requirement | Description |
|-------------|-------------|
| Secret filtering | Existing patterns continue to work |
| PII detection | Existing redaction continues to work |
| File permissions | User storage 0600 |
| No credential logging | Git remote URLs may contain tokens - sanitize |

### Scalability

| Dimension | Target |
|-----------|--------|
| Memories per user | 100,000+ |
| Projects per user | 1,000+ |
| Branches per project | 100+ |
| Concurrent access | Single-user (CLI), multi-connection (MCP server) |

### Maintainability

| Requirement | Description |
|-------------|-------------|
| Code reduction | Remove ~500 LOC (git-notes modules) |
| Dependency reduction | Remove git2 if only used for notes |
| Test coverage | >90% for new code |
| Documentation | CLAUDE.md updated with new query patterns |

## Technical Constraints

### Technology Stack Requirements

| Layer | Constraint |
|-------|------------|
| Persistence | SQLite (default), PostgreSQL (configurable) |
| Index | SQLite FTS5 (default), PostgreSQL full-text |
| Vector | usearch (default), pgvector (configurable) |
| Embeddings | fastembed (all-MiniLM-L6-v2, 384 dims) |

### Integration Requirements

| Integration | Constraint |
|-------------|------------|
| Claude Code hooks | Response format unchanged |
| MCP protocol | Tool schemas backward compatible |
| CLI | Command syntax unchanged (new flags only) |

### Compatibility Requirements

| Requirement | Description |
|-------------|-------------|
| Rust MSRV | 1.85 |
| Platforms | macOS ARM64, Linux x86_64, Windows x86_64 |
| User data paths | `directories` crate for platform-specific |

## Dependencies

### Internal Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Domain model refactor | Part of this spec | Simplify Domain struct |
| ServiceContainer refactor | Part of this spec | Remove repo_path requirement |
| SQLite migrations | Part of this spec | Add facet columns |

### External Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| `directories` crate | Already used | For user data paths |
| `git2` crate | To be removed | Only if git-notes removed |
| `rusqlite` | Already used | Primary persistence |

## Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Capture regression | Low | Critical | Comprehensive test suite before/after |
| Performance degradation | Low | Medium | Benchmark suite comparison |
| User confusion about facets | Medium | Low | Clear documentation, sensible defaults |
| Branch detection edge cases | Medium | Low | Graceful fallback to null facet |
| Platform path issues | Low | Low | Use `directories` crate |

## Open Questions

- [x] ~~Should legacy git-notes be migrated?~~ **No - fresh start (user decision)**
- [x] ~~Should org-scope be included?~~ **Yes - design now, feature-gate (user decision)**
- [ ] Should we rename `Domain` to something clearer like `Scope` or `Context`?
- [ ] Should facets be stored in a separate table (normalized) or inline (denormalized)?

## Appendix

### Glossary

| Term | Definition |
|------|------------|
| Facet | A dimension for filtering (project, branch, path) |
| Tombstone | Soft-deleted memory, hidden by default |
| Scope | Storage tier (user, project, org) |
| URN | Uniform Resource Name for memories |

### References

- [GitHub Issue #43](https://github.com/zircote/subcog/issues/43) - RFC: Simplify Storage Architecture
- [GitHub Issue #42](https://github.com/zircote/subcog/issues/42) - Bug: CaptureService attaches all notes to HEAD
- [git-notes-memory-manager](https://github.com/zircote/git-notes-memory-manager) - Original Python implementation
