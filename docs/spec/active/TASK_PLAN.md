# Issue #45: Git Notes Removal & SQLite Consolidation

**Issue**: [#45](https://github.com/zircote/subcog/issues/45) - Storage backend config ignored
**Branch**: `plan/storage-simplification`
**Root Cause**: `CaptureService` still writes to git notes despite storage simplification spec claiming removal

---

## Phase 1: CaptureService SQLite Migration

- [ ] `src/services/capture.rs:193-216` - Replace git notes write with SQLite index write
- [ ] `src/services/capture.rs` - Generate memory ID from UUID instead of git note SHA
- [ ] `src/services/capture.rs` - Remove `NotesManager` import
- [ ] `src/services/capture.rs` - Ensure `IndexBackend::insert()` is called for every capture
- [ ] `src/services/capture.rs` - Update tests to verify SQLite-only storage

## Phase 2: RecallService SQLite Verification

- [ ] `src/services/recall.rs` - Verify reads from SQLite `IndexBackend` only
- [ ] `src/services/recall.rs` - Remove any git notes fallback logic if present
- [ ] `src/services/mod.rs` - Verify `ServiceContainer` passes SQLite backend to services

## Phase 3: Git Notes Module Deletion

- [ ] DELETE `src/git/notes.rs`
- [ ] `src/git/mod.rs` - Remove `pub mod notes` export
- [ ] `src/git/remote.rs` - Remove notes-specific sync logic (keep branch/context detection)

## Phase 4: Storage Persistence Layer Cleanup

- [ ] DELETE `src/storage/persistence/git_notes.rs`
- [ ] `src/storage/persistence/mod.rs` - Remove `GitNotesBackend` export
- [ ] `src/storage/mod.rs` - Remove GitNotes references
- [ ] `src/storage/traits/persistence.rs` - Remove GitNotes from documentation
- [ ] `src/storage/resilience.rs` - Remove GitNotes references

## Phase 5: Prompt Storage Migration

- [ ] DELETE `src/storage/prompt/git_notes.rs`
- [ ] `src/storage/prompt/mod.rs` - Remove git_notes module export
- [ ] `src/services/prompt.rs` - Remove GitNotes prompt backend, ensure SQLite-only
- [ ] `src/services/prompt.rs` - Verify `SqlitePromptStore` is used exclusively

## Phase 6: Config Cleanup

- [ ] `src/config/mod.rs` - Remove `StorageBackendType::GitNotes` enum variant
- [ ] `src/config/mod.rs` - Remove "git_notes" from `StorageBackendType::parse()`
- [ ] `src/config/mod.rs` - Remove `StorageConfig.project` field (consolidate to `user`)
- [ ] `example.config.toml` - Remove `[storage.project]` section
- [ ] `example.config.toml` - Document SQLite + facets architecture

## Phase 7: Commands Update

- [ ] `src/commands/core.rs` - Remove `StorageBackendType::GitNotes` match arm
- [ ] `src/commands/core.rs` - Update consolidate command to SQLite-only
- [ ] `src/commands/config.rs` - Remove GitNotes display logic
- [ ] `src/commands/config.rs` - Show SQLite path and facet info instead

## Phase 8: Services Cleanup

- [ ] `src/services/mod.rs` - Remove GitNotes references
- [ ] `src/services/data_subject.rs` - Remove GitNotes references
- [ ] `src/services/enrichment.rs` - Remove GitNotes references
- [ ] `src/services/sync.rs` - Update sync to work with SQLite (or remove if not needed)

## Phase 9: Documentation

- [ ] `README.md` - Update storage architecture section to SQLite + facets
- [ ] `CLAUDE.md` - Update storage documentation
- [ ] `commands/sync.md` - Update or deprecate based on new architecture
- [ ] Update completed spec `docs/spec/completed/2026-01-03-storage-simplification/` to reflect actual state

## Phase 10: Verification

- [ ] `make ci` passes (format, lint-strict, test, doc, deny, msrv, bench)
- [ ] `subcog capture` writes ONLY to SQLite
- [ ] `subcog recall` reads ONLY from SQLite
- [ ] `subcog status` shows SQLite database info
- [ ] No `refs/notes/subcog` created on new captures
- [ ] Close Issue #45 with PR reference

---

**Total: 42 tasks across 10 phases**

**Started**: 2026-01-03
**Status**: Not Started
