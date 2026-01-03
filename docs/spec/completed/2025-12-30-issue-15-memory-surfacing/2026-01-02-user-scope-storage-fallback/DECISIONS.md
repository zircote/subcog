# Architecture Decision Records: User-Scope Storage Fallback

## Document Control

| Field | Value |
|-------|-------|
| Version | 1.0 |
| Status | Draft |
| Last Updated | 2026-01-02 |
| Author | Claude (Architect) |

---

## ADR-001: SQLite for User-Scope Persistence

### Status

Accepted

### Context

When operating outside a git repository, we need an alternative persistence layer for user-scoped memories. Options considered:
1. SQLite database
2. JSON files in user directory
3. TOML/YAML files
4. No persistence (memory only)

### Decision

Use SQLite for user-scope persistence.

### Rationale

- **Already a dependency**: `rusqlite` is used for the index layer
- **ACID transactions**: Prevents data corruption on crash
- **Single file**: Easy to backup/move (`memories.db`)
- **Query support**: Can filter by namespace, search by content
- **Proven pattern**: Index layer already uses SQLite successfully

### Consequences

**Positive**:
- No new dependencies
- Familiar patterns from index implementation
- Atomic operations prevent data loss

**Negative**:
- Not human-readable like JSON/YAML
- Requires schema migrations for future changes

### Alternatives Rejected

- **JSON files**: No ACID, harder to query, file-per-memory creates clutter
- **TOML/YAML**: Same issues as JSON
- **Memory only**: Data lost on restart, unacceptable for memories

---

## ADR-002: Conditional Git Notes in CaptureService

### Status

Accepted

### Context

CaptureService currently always attempts to store to git notes. For user-scope, git notes are unavailable. Options:
1. Add `use_git_notes: bool` flag to CaptureService
2. Create separate `UserCaptureService` class
3. Make persistence layer fully pluggable

### Decision

Add `use_git_notes: bool` flag to CaptureService.

### Rationale

- **Minimal change**: Single field addition
- **Clear semantics**: Flag name self-documents behavior
- **Reuses existing code**: Same embedding, indexing, vector logic
- **Easy testing**: Can test both modes with same test harness

### Consequences

**Positive**:
- No new service class to maintain
- Existing tests continue to work
- Easy to understand behavior switch

**Negative**:
- Slight complexity increase in `capture()` method
- Must ensure flag is set correctly by factories

### Alternatives Rejected

- **Separate UserCaptureService**: Code duplication, maintenance burden
- **Pluggable persistence**: Over-engineering for current requirements

---

## ADR-003: Factory Method Pattern for ServiceContainer

### Status

Accepted

### Context

Need to create ServiceContainer instances for different contexts (project vs user). Options:
1. Factory methods on ServiceContainer
2. Builder pattern
3. Separate factory class
4. Configuration-based instantiation

### Decision

Use factory methods on ServiceContainer: `for_repo()`, `for_user()`, `from_current_dir_or_user()`.

### Rationale

- **Existing pattern**: `for_repo()` and `from_current_dir()` already exist
- **Discoverable**: All creation methods in one place
- **Self-documenting**: Method names describe intent
- **No new types**: Avoids adding builder/factory classes

### Consequences

**Positive**:
- Consistent with existing API
- Easy to use and understand
- IDE autocomplete shows all options

**Negative**:
- ServiceContainer impl block grows larger
- Each factory has its own initialization logic

### Alternatives Rejected

- **Builder pattern**: Overkill for 3-4 factory methods
- **Separate factory class**: Adds indirection without benefit
- **Configuration-based**: Too implicit, harder to debug

---

## ADR-004: No-Op SyncService for User Scope

### Status

Accepted

### Context

User-scope has no git remote to sync with. SyncService must handle this. Options:
1. Return error when sync called on user scope
2. Return empty success (no-op)
3. Omit SyncService from user-scope container
4. Future: cloud backup sync

### Decision

Create `SyncService::no_op()` that returns empty success stats.

### Rationale

- **Graceful handling**: Sync commands don't crash
- **Consistent API**: ServiceContainer always has SyncService
- **Future-proof**: Can add cloud sync later
- **User-friendly**: No confusing errors for user-scope

### Consequences

**Positive**:
- No special-casing in callers
- Clear intent in method name
- Easy to extend later

**Negative**:
- Silent no-op might confuse users expecting sync
- Should document this behavior

### Alternatives Rejected

- **Return error**: Poor UX for scripts that call sync unconditionally
- **Omit SyncService**: Breaking API change, requires Option<> everywhere
- **Cloud sync now**: Out of scope, significant additional work

---

## ADR-005: User Data Directory Location

### Status

Accepted

### Context

Need to determine where user-scope data should be stored. Options:
1. XDG Base Directory Specification (`~/.local/share/subcog/`)
2. Home directory dotfile (`~/.subcog/`)
3. Platform-specific locations (using `directories` crate)
4. Configurable via environment variable

### Decision

Use `directories` crate for platform-specific locations.

### Rationale

- **Already a dependency**: No new crate needed
- **Platform conventions**: Follows OS expectations
  - macOS: `~/Library/Application Support/subcog/`
  - Linux: `~/.local/share/subcog/`
  - Windows: `C:\Users\<User>\AppData\Local\subcog\`
- **Tested**: `directories` is widely used and well-maintained
- **No home clutter**: Doesn't add dotfiles to home directory

### Consequences

**Positive**:
- Professional, follows platform conventions
- Users know where to find data
- Works across all major platforms

**Negative**:
- Paths differ by platform (documentation complexity)
- `directories` crate might fail on unusual systems

### Alternatives Rejected

- **`~/.subcog/`**: Clutters home directory, non-standard on macOS/Windows
- **XDG only**: Doesn't apply to macOS/Windows
- **Environment variable**: Added complexity, users must configure

---

## ADR-006: URN Format for User Scope

### Status

Accepted

### Context

User-scope memories need URN identifiers. Current project-scope uses `subcog://global/{namespace}/{id}`. Options:
1. Use `subcog://user/{namespace}/{id}`
2. Use `subcog://local/{namespace}/{id}`
3. Use `subcog://~/{namespace}/{id}`
4. Keep `global` for all scopes

### Decision

Use `subcog://user/{namespace}/{id}` for user-scoped memories.

### Rationale

- **Clear scope indicator**: "user" clearly indicates personal scope
- **Consistent with Domain**: `Domain::for_user()` already exists
- **Disambiguates from project**: Easy to tell scope at a glance
- **Parseable**: Standard URN format maintained

### Consequences

**Positive**:
- Users can immediately identify memory scope
- Easy to filter by scope in tools
- Consistent with internal Domain representation

**Negative**:
- Two URN prefixes to document
- Search results might mix scopes (if cross-scope search added)

### Alternatives Rejected

- **`local`**: Ambiguous (local to what?)
- **`~`**: Not URL-safe without encoding
- **Keep `global`**: No way to distinguish scopes

---

## ADR-007: Fallback Order in `from_current_dir_or_user()`

### Status

Accepted

### Context

When automatically selecting scope, need to decide priority. Options:
1. Try project first, fall back to user
2. Try user first, fall back to project
3. Check `.git` explicitly, then decide
4. Always prefer user scope

### Decision

Try project scope first via `from_current_dir()`, fall back to user scope on error.

### Rationale

- **Preserves existing behavior**: Project scope is default when available
- **Simple implementation**: Just catch the error
- **Git repo = project intent**: If user is in git repo, they likely want project memories
- **Explicit fallback**: Only uses user scope when project is impossible

### Consequences

**Positive**:
- No change in behavior for existing git repo users
- Clear fallback chain
- Easy to understand and debug

**Negative**:
- Slight overhead trying project first (negligible)
- Error might be logged before fallback

### Alternatives Rejected

- **User first**: Would break existing workflows in git repos
- **Explicit check**: Same result, more code
- **Always user**: Defeats purpose of project-scoped memories

---

## Decision Summary

| ADR | Decision | Key Rationale |
|-----|----------|---------------|
| 001 | SQLite for persistence | Already a dependency, ACID |
| 002 | `use_git_notes` flag | Minimal change, clear |
| 003 | Factory methods | Existing pattern |
| 004 | No-op SyncService | Graceful handling |
| 005 | `directories` crate | Platform conventions |
| 006 | `subcog://user/...` URN | Clear scope indicator |
| 007 | Project-first fallback | Preserves existing behavior |
