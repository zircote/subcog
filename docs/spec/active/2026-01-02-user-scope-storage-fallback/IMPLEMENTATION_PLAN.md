# Implementation Plan: ServiceContainer User-Scope Storage Fallback

## Document Control

| Field | Value |
|-------|-------|
| Version | 1.0 |
| Status | Draft |
| Last Updated | 2026-01-02 |
| Author | Claude (Architect) |

## Overview

This plan implements automatic fallback to user-scoped SQLite storage when subcog operates outside a git repository. Estimated effort: 4-8 hours across 4 phases.

## Phase Summary

| Phase | Focus | Duration | Dependencies |
|-------|-------|----------|--------------|
| 1 | User-Scope Storage Infrastructure | 1-2 hours | None |
| 2 | CaptureService SQLite-Only Mode | 1-2 hours | Phase 1 |
| 3 | ServiceContainer Factory Methods | 1-2 hours | Phase 2 |
| 4 | MCP/CLI Integration & Testing | 1-2 hours | Phase 3 |

---

## Phase 1: User-Scope Storage Infrastructure

**Objective**: Create the foundational storage layer for user-scoped memories.

### Tasks

- [ ] **1.1** Add `get_user_data_dir()` helper function
  - File: `src/storage/index/domain.rs`
  - Use `directories::BaseDirs` for cross-platform support
  - Return `~/.local/share/subcog/` (Linux), `~/Library/Application Support/subcog/` (macOS), etc.
  - Already exists but verify it's exported

- [ ] **1.2** Create SQLite persistence backend for user scope
  - File: `src/storage/persistence/sqlite.rs` (new file)
  - Implement `PersistenceBackend` trait
  - Methods: `store()`, `get()`, `list()`, `delete()`
  - Schema: `memories` table with id, content, namespace, domain, tags, created_at, updated_at

- [ ] **1.3** Add migration for SQLite persistence schema
  - File: `src/storage/persistence/sqlite.rs`
  - Create `memories` table on first connection
  - Add index on namespace, created_at

- [ ] **1.4** Export new components from storage module
  - File: `src/storage/mod.rs`
  - Export `SqlitePersistenceBackend`

- [ ] **1.5** Add unit tests for SQLite persistence
  - File: `src/storage/persistence/sqlite.rs`
  - Test: store and retrieve memory
  - Test: list by namespace
  - Test: delete memory

### Acceptance Criteria

- [ ] `get_user_data_dir()` returns correct path per platform
- [ ] `SqlitePersistenceBackend` implements `PersistenceBackend`
- [ ] Unit tests pass
- [ ] Directory created automatically if missing

---

## Phase 2: CaptureService SQLite-Only Mode

**Objective**: Enable CaptureService to persist memories without git notes.

### Tasks

- [ ] **2.1** Add `use_git_notes` field to CaptureService
  - File: `src/services/capture.rs`
  - Type: `bool`
  - Default: `true` (existing behavior)

- [ ] **2.2** Add `sqlite_persistence` field to CaptureService
  - File: `src/services/capture.rs`
  - Type: `Option<Arc<dyn PersistenceBackend + Send + Sync>>`
  - Used when `use_git_notes = false`

- [ ] **2.3** Create `with_backends_no_git()` constructor
  - File: `src/services/capture.rs`
  - Sets `use_git_notes = false`
  - Sets `sqlite_persistence` to SQLite backend
  - Uses same embedder/index/vector backends

- [ ] **2.4** Modify `capture()` to conditionally use git notes
  - File: `src/services/capture.rs`
  - If `use_git_notes`: store to git notes (existing)
  - If `!use_git_notes`: store to `sqlite_persistence`
  - Always store to index and vector (unchanged)

- [ ] **2.5** Add unit tests for no-git capture
  - File: `src/services/capture.rs`
  - Test: capture without git notes succeeds
  - Test: memory persisted to SQLite
  - Test: memory searchable via index

### Acceptance Criteria

- [ ] `CaptureService::with_backends_no_git()` exists
- [ ] Capture succeeds without git repository
- [ ] Memory persisted to SQLite
- [ ] Existing git notes behavior unchanged

---

## Phase 3: ServiceContainer Factory Methods

**Objective**: Add factory methods that automatically select storage scope.

### Tasks

- [ ] **3.1** Add `for_user()` method to ServiceContainer
  - File: `src/services/mod.rs`
  - Create user data directory if needed
  - Initialize SQLite persistence backend
  - Initialize index and vector backends at user paths
  - Create CaptureService with `with_backends_no_git()`
  - Create no-op SyncService

- [ ] **3.2** Add `no_op()` constructor to SyncService
  - File: `src/services/sync.rs`
  - Returns SyncService that does nothing
  - `sync()`, `push()`, `fetch()` return empty stats

- [ ] **3.3** Add `from_current_dir_or_user()` method
  - File: `src/services/mod.rs`
  - Try `from_current_dir()` first
  - If error, fall back to `for_user()`
  - Log which scope is being used

- [ ] **3.4** Update `DomainIndexManager` for user-only mode
  - File: `src/storage/index/domain.rs`
  - Support `repo_path: None` configuration
  - Default to user scope when no repo

- [ ] **3.5** Add unit tests for factory methods
  - File: `src/services/mod.rs`
  - Test: `for_user()` creates valid container
  - Test: `from_current_dir_or_user()` returns project scope in git repo
  - Test: `from_current_dir_or_user()` returns user scope outside git repo

### Acceptance Criteria

- [ ] `ServiceContainer::for_user()` creates functional container
- [ ] `ServiceContainer::from_current_dir_or_user()` selects correct scope
- [ ] User-scope container has functional capture and recall
- [ ] No-op sync service works correctly

---

## Phase 4: MCP/CLI Integration & Testing

**Objective**: Update all entry points to use context-aware factory.

### Tasks

- [ ] **4.1** Update MCP `execute_capture()` handler
  - File: `src/mcp/tools/handlers/core.rs`
  - Change from `ServiceContainer::from_current_dir()?`
  - To `ServiceContainer::from_current_dir_or_user()?`

- [ ] **4.2** Update MCP `execute_recall()` handler
  - File: `src/mcp/tools/handlers/core.rs`
  - Change from `ServiceContainer::from_current_dir()?`
  - To `ServiceContainer::from_current_dir_or_user()?`

- [ ] **4.3** Update CLI `cmd_capture()` function
  - File: `src/commands/core.rs`
  - Use context-aware service creation
  - Handle both project and user scope

- [ ] **4.4** Update CLI `cmd_recall()` function
  - File: `src/commands/core.rs`
  - Use context-aware service creation

- [ ] **4.5** Add integration tests
  - File: `tests/user_scope_integration.rs` (new)
  - Test: capture in `/tmp` (no git) succeeds
  - Test: recall in `/tmp` returns user-scope memories
  - Test: MCP capture outside git repo
  - Test: MCP recall outside git repo

- [ ] **4.6** Add manual testing script
  - File: `scripts/test_user_scope.sh` (new)
  - Creates temp directory (no git)
  - Runs capture and recall via CLI
  - Verifies URN format is `subcog://user/...`
  - Cleans up

- [ ] **4.7** Update documentation
  - File: `CLAUDE.md`
  - Document user-scope behavior
  - Document storage paths
  - Document URN format difference

- [ ] **4.8** Run full CI checks
  - `cargo fmt -- --check`
  - `cargo clippy --all-targets --all-features`
  - `cargo test`
  - `cargo doc --no-deps`

### Acceptance Criteria

- [ ] MCP tools work outside git repository
- [ ] CLI commands work outside git repository
- [ ] Integration tests pass
- [ ] All CI checks pass
- [ ] Documentation updated

---

## Verification Commands

### Phase 1 Verification

```bash
# Run storage tests
cargo test storage::persistence::sqlite --release

# Verify user data dir
cargo run --release -- status
# Should show user data directory path
```

### Phase 2 Verification

```bash
# Run capture tests
cargo test capture --release

# Test capture without git notes (unit test)
cargo test capture_without_git --release
```

### Phase 3 Verification

```bash
# Run service container tests
cargo test service_container --release

# Test factory methods
cargo test from_current_dir_or_user --release
cargo test for_user --release
```

### Phase 4 Verification

```bash
# Integration test
cd /tmp && subcog capture --namespace learnings "Test memory"
# Should succeed with URN: subcog://user/learnings/...

cd /tmp && subcog recall "Test"
# Should return the captured memory

# Full CI
make ci
```

---

## Risk Mitigation

### Risk 1: SQLite File Locking Issues

**Mitigation**:
- Use WAL mode for better concurrency
- Add timeout for lock acquisition
- Graceful error message on lock failure

### Risk 2: Platform-Specific Path Issues

**Mitigation**:
- Use `directories` crate (already a dependency)
- Test on macOS (primary), document Linux/Windows paths
- Add CI matrix for multiple platforms (future)

### Risk 3: User Directory Permission Errors

**Mitigation**:
- Catch permission errors explicitly
- Provide actionable error message: "Cannot create ~/.local/share/subcog/. Please create manually with: mkdir -p ~/.local/share/subcog"
- Don't crash; fail gracefully

---

## Rollout Checklist

### Pre-Implementation

- [ ] Review existing `PersistenceBackend` trait
- [ ] Verify `directories` crate is available
- [ ] Check `SqliteBackend` (index) for reference patterns

### Implementation

- [ ] Phase 1: Storage infrastructure
- [ ] Phase 2: CaptureService modifications
- [ ] Phase 3: ServiceContainer factory methods
- [ ] Phase 4: MCP/CLI integration

### Post-Implementation

- [ ] All tests passing
- [ ] Manual verification in `/tmp`
- [ ] Documentation updated
- [ ] Ready for merge

---

## Success Criteria

| Metric | Target |
|--------|--------|
| Capture outside git | 100% success |
| Recall outside git | 100% success |
| Project-scope regression | 0 failures |
| New test coverage | > 90% |
| CI checks | All green |

---

## File Change Summary

| File | Changes |
|------|---------|
| `src/storage/persistence/sqlite.rs` | New file - SQLite persistence backend |
| `src/storage/persistence/mod.rs` | Export SqlitePersistenceBackend |
| `src/storage/mod.rs` | Re-export persistence types |
| `src/services/capture.rs` | Add `use_git_notes`, `with_backends_no_git()` |
| `src/services/sync.rs` | Add `no_op()` constructor |
| `src/services/mod.rs` | Add `for_user()`, `from_current_dir_or_user()` |
| `src/storage/index/domain.rs` | Support `repo_path: None` |
| `src/mcp/tools/handlers/core.rs` | Use context-aware factory |
| `src/commands/core.rs` | Use context-aware factory |
| `tests/user_scope_integration.rs` | New integration tests |
| `CLAUDE.md` | Documentation updates |
