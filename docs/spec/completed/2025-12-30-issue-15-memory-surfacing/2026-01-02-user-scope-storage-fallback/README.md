# SPEC-2026-01-02: User-Scope Storage Fallback

## Overview

| Field | Value |
|-------|-------|
| Spec ID | SPEC-2026-01-02-USER-SCOPE |
| Status | Ready for Implementation |
| Priority | P0 (Critical) |
| Estimated Effort | 4-8 hours |
| Author | Claude (Architect) |
| Created | 2026-01-02 |

## Problem Statement

When subcog operates outside a git repository, `ServiceContainer::from_current_dir()` fails because it requires a git repo for project-scoped storage. This breaks capture and recall operations for users working in non-git directories.

**Current behavior:**
```bash
cd /tmp  # No .git folder
subcog capture --namespace learnings "TIL about Rust"
# Error: No git repository found starting from: /tmp
```

**Desired behavior:**
```bash
cd /tmp  # No .git folder
subcog capture --namespace learnings "TIL about Rust"
# Memory captured: subcog://user/learnings/abc123
```

## Solution Summary

Add automatic fallback to user-scoped SQLite storage when no git repository is detected:

1. **New factory method**: `ServiceContainer::from_current_dir_or_user()`
2. **User-scope storage**: SQLite database at `~/.local/share/subcog/` (platform-specific)
3. **Modified CaptureService**: Support SQLite-only persistence (no git notes)
4. **Updated entry points**: MCP handlers and CLI commands use context-aware factory

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Persistence backend | SQLite | Already a dependency, ACID transactions |
| Scope detection | Check `.git` folder | Simple, reliable |
| Fallback order | Project first, then user | Preserves existing behavior |
| URN format | `subcog://user/...` | Clear scope indicator |
| Sync behavior | No-op for user scope | No git remote available |

## Documents

| Document | Description |
|----------|-------------|
| [REQUIREMENTS.md](./REQUIREMENTS.md) | Functional and non-functional requirements |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Technical design and component changes |
| [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md) | Phased implementation tasks |
| [DECISIONS.md](./DECISIONS.md) | Architecture decision records |

## Implementation Phases

| Phase | Focus | Tasks | Duration |
|-------|-------|-------|----------|
| 1 | Storage Infrastructure | SQLite persistence backend | 1-2 hours |
| 2 | CaptureService | SQLite-only mode | 1-2 hours |
| 3 | ServiceContainer | Factory methods | 1-2 hours |
| 4 | Integration | MCP/CLI updates, testing | 1-2 hours |

## Success Criteria

- [ ] Capture works outside git repository (100% success rate)
- [ ] Recall works outside git repository (100% success rate)
- [ ] No regression for project-scoped operations
- [ ] All CI checks pass
- [ ] Test coverage > 90% for new code

## Dependencies

### Already Implemented (Previous Session)

- `Domain::default_for_context()` - Returns `for_user()` outside git repo
- `Domain::for_user()` - Creates user-scoped domain
- `is_in_git_repo()` - Detects git repository presence
- `DomainScope::default_for_context()` - Returns appropriate scope

### To Be Implemented

- `ServiceContainer::for_user()` - Creates user-scoped container
- `ServiceContainer::from_current_dir_or_user()` - Context-aware factory
- `CaptureService::with_backends_no_git()` - SQLite-only capture
- `SyncService::no_op()` - No-op sync for user scope
- `SqlitePersistenceBackend` - SQLite persistence layer

## Storage Paths

| Platform | User Data Directory |
|----------|---------------------|
| macOS | `~/Library/Application Support/subcog/` |
| Linux | `~/.local/share/subcog/` |
| Windows | `C:\Users\<User>\AppData\Local\subcog\` |

Files within:
- `memories.db` - SQLite persistence (replaces git notes)
- `index.db` - SQLite FTS5 index
- `vectors.idx` - usearch HNSW vectors

## Quick Start (After Implementation)

```bash
# Works in any directory
cd /tmp
subcog capture --namespace learnings "User-scoped memory"
# Memory captured: subcog://user/learnings/abc123

subcog recall "User-scoped"
# Returns the memory

# Still works in git repos (unchanged)
cd ~/my-project  # Has .git folder
subcog capture --namespace decisions "Project decision"
# Memory captured: subcog://global/decisions/def456
```

## Related Issues

- Discovered during testing of domain-aware storage (SPEC-2026-01-02-MEMORY-SYSTEM)
- Domain detection works correctly; ServiceContainer lacks fallback path

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| SQLite file locking | Medium | Use WAL mode, timeout |
| Platform path issues | Low | Use `directories` crate |
| Permission errors | Low | Graceful error message |

## Out of Scope

- Org-scope storage (requires separate configuration)
- Cross-scope search (searching user + project simultaneously)
- Sync between scopes (moving memories from user to project)
- Cloud backup for user scope (future enhancement)
