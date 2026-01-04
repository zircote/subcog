# Requirements: ServiceContainer User-Scope Storage Fallback

## Document Control

| Field | Value |
|-------|-------|
| Version | 1.0 |
| Status | Draft |
| Last Updated | 2026-01-02 |
| Author | Claude (Architect) |

## Executive Summary

When subcog operates outside a git repository, `ServiceContainer::from_current_dir()` fails because it requires a git repo for project-scoped storage. This spec adds automatic fallback to user-scoped SQLite storage when no git repository is detected.

## Problem Statement

### Current Behavior

1. User runs subcog capture/recall in a directory without `.git`
2. `ServiceContainer::from_current_dir()` calls `find_repo_root()`
3. `find_repo_root()` fails with "No git repository found"
4. Capture/recall operations fail entirely

### Desired Behavior

1. User runs subcog capture/recall in any directory
2. If in a git repo → use project scope (git notes + `.subcog/index.db`)
3. If NOT in a git repo → fall back to user scope (`~/.local/share/subcog/memories.db`)
4. Operations succeed with appropriate storage backend

## Functional Requirements

### FR-1: Context-Aware ServiceContainer Factory

**ID**: FR-1
**Priority**: P0 (Critical)
**Description**: Add a new factory method that automatically selects the appropriate storage scope based on git repository presence.

**Acceptance Criteria**:
- [ ] `ServiceContainer::from_current_dir_or_user()` method exists
- [ ] Returns project-scoped container when in git repo
- [ ] Returns user-scoped container when NOT in git repo
- [ ] No error thrown when git repo is absent

### FR-2: User-Scope Storage Backend

**ID**: FR-2
**Priority**: P0 (Critical)
**Description**: Implement user-scoped storage that works without a git repository.

**Acceptance Criteria**:
- [ ] SQLite database at `~/.local/share/subcog/memories.db` (Linux/macOS) or equivalent
- [ ] Index at `~/.local/share/subcog/index.db`
- [ ] Vector store at `~/.local/share/subcog/vectors.idx`
- [ ] Directory created automatically if missing
- [ ] Cross-platform path resolution (using `directories` crate)

### FR-3: User-Scope CaptureService

**ID**: FR-3
**Priority**: P0 (Critical)
**Description**: CaptureService must work without git notes persistence.

**Acceptance Criteria**:
- [ ] CaptureService can use SQLite-only persistence (no git notes)
- [ ] Memories captured to user scope are persisted to SQLite
- [ ] `Domain::for_user()` memories bypass git notes layer
- [ ] Capture succeeds and returns valid URN

### FR-4: MCP and CLI Integration

**ID**: FR-4
**Priority**: P1 (High)
**Description**: MCP tools and CLI commands must use the context-aware factory.

**Acceptance Criteria**:
- [ ] `execute_capture()` uses `from_current_dir_or_user()`
- [ ] `execute_recall()` uses `from_current_dir_or_user()`
- [ ] CLI `cmd_capture()` uses `from_current_dir_or_user()`
- [ ] CLI `cmd_recall()` uses `from_current_dir_or_user()`
- [ ] Graceful operation in any directory

### FR-5: Domain Routing

**ID**: FR-5
**Priority**: P1 (High)
**Description**: Ensure Domain correctly routes to storage backend.

**Acceptance Criteria**:
- [ ] `Domain::default_for_context()` already implemented (returns `for_user()` outside git)
- [ ] User-scoped memories use `subcog://user/{namespace}/{id}` URN format
- [ ] User memories searchable across all directories (not repo-specific)

## Non-Functional Requirements

### NFR-1: Performance

- User-scope operations should have < 50ms latency for capture
- User-scope search should have < 100ms latency
- No performance regression for project-scope operations

### NFR-2: Reliability

- No data loss when switching between directories
- User memories persist across sessions
- Graceful degradation if user data directory unwritable

### NFR-3: Compatibility

- Existing project-scoped workflows unaffected
- No breaking changes to public API
- Works on macOS, Linux, and Windows

### NFR-4: Storage Paths

| Platform | User Data Directory |
|----------|---------------------|
| macOS | `~/Library/Application Support/subcog/` |
| Linux | `~/.local/share/subcog/` |
| Windows | `C:\Users\<User>\AppData\Local\subcog\` |

## Use Cases

### UC-1: Capture Outside Git Repo

**Actor**: Developer
**Precondition**: Working in `/tmp/scratch` (no `.git` folder)
**Flow**:
1. Developer runs `subcog capture --namespace learnings "TIL about Rust"`
2. System detects no git repository
3. System creates user-scope storage if needed
4. Memory captured to `~/.local/share/subcog/memories.db`
5. URN returned: `subcog://user/learnings/abc123`

### UC-2: Recall Outside Git Repo

**Actor**: Developer
**Precondition**: Has previously captured user-scope memories
**Flow**:
1. Developer runs `subcog recall "Rust"` from any directory
2. System detects no git repository
3. System searches user-scope SQLite index
4. Returns matching user-scope memories

### UC-3: MCP Tool Usage

**Actor**: Claude Code via MCP
**Precondition**: Working in directory without git
**Flow**:
1. Claude calls `subcog_capture` tool
2. MCP handler uses `ServiceContainer::from_current_dir_or_user()`
3. Memory captured to user scope
4. Success response returned

## Out of Scope

1. **Org-scope storage** - Requires separate configuration, not automatic
2. **Cross-scope search** - Searching both user and project memories simultaneously
3. **Sync between scopes** - Moving memories from user to project scope
4. **User-to-git migration** - Converting user memories to git notes

## Dependencies

### Existing Components (Ready)

- `Domain::default_for_context()` - Detects context and returns appropriate domain
- `Domain::for_user()` - Creates user-scoped domain
- `is_in_git_repo()` - Checks for git repository presence
- `DomainScope::default_for_context()` - Returns Project or User based on context

### Components Needing Modification

- `ServiceContainer` - Add `from_current_dir_or_user()` and `for_user()` methods
- `CaptureService` - Support SQLite-only persistence (no git notes)
- MCP handlers - Use context-aware factory
- CLI commands - Use context-aware factory

## Success Metrics

| Metric | Target |
|--------|--------|
| Capture outside git repo | 100% success rate |
| Recall outside git repo | 100% success rate |
| Project-scope regression | 0 (no change) |
| Test coverage | > 90% for new code |

## Risks and Mitigations

### Risk 1: User Doesn't Realize Memories Aren't in Git

**Impact**: Medium
**Likelihood**: Medium
**Mitigation**: Include `domain: user` in output and URN format (`subcog://user/...`)

### Risk 2: Platform-Specific Path Issues

**Impact**: High
**Likelihood**: Low
**Mitigation**: Use `directories` crate for cross-platform path resolution (already a dependency)

### Risk 3: Permission Errors Creating User Directory

**Impact**: Medium
**Likelihood**: Low
**Mitigation**: Graceful error message with suggested fix (create directory manually)
