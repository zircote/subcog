---
project_id: SPEC-2026-01-03-001
project_name: "Storage Architecture Simplification"
slug: storage-simplification
status: completed
created: 2026-01-03T00:56:00Z
approved: 2026-01-03T02:00:00Z
started: 2026-01-03T02:00:00Z
completed: 2026-01-03T22:00:00Z
final_effort: ~10 hours
outcome: success
expires: 2026-04-03T00:56:00Z
superseded_by: null
tags: [storage, architecture, git-notes, refactoring, simplification]
stakeholders: []
github_issue: 43
worktree:
  branch: plan/storage-simplification
  base_branch: develop
---

# Storage Architecture Simplification

## Overview

| Field | Value |
|-------|-------|
| Spec ID | SPEC-2026-01-03-001 |
| Status | Complete |
| Priority | P0 (Critical - fixes data loss bug) |
| GitHub Issue | [#43](https://github.com/zircote/subcog/issues/43) |
| Author | Claude (Architect) |
| Created | 2026-01-03 |

## Problem Statement

The current storage architecture uses git-notes as the primary persistence layer, but this approach has fundamental problems:

1. **Critical Bug**: All captures attach to HEAD with `force=true`, causing each new memory to overwrite the previous one
2. **Design Mismatch**: Git notes are designed to annotate commits, not store arbitrary key-value data
3. **Complexity**: Three storage tiers (org/project/user) create synchronization and backup complexity
4. **Silos**: Per-repository git-notes storage prevents cross-project learning

## Proposed Solution

Consolidate to **user-level storage with project/branch/path faceting**:

- Remove git-notes storage entirely
- Use user-level SQLite/PostgreSQL/Redis as the single active storage tier
- Auto-detect project context from git remote, branch, and cwd
- Query by facets instead of switching storage locations

## Documents

| Document | Description |
|----------|-------------|
| [REQUIREMENTS.md](./REQUIREMENTS.md) | Functional and non-functional requirements |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Technical design and component changes |
| [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md) | Phased implementation tasks |
| [DECISIONS.md](./DECISIONS.md) | Architecture decision records |
| [RESEARCH_NOTES.md](./RESEARCH_NOTES.md) | Research findings |
| [CHANGELOG.md](./CHANGELOG.md) | Specification history |

## Related

- **GitHub Issue**: [#43 - RFC: Simplify Storage Architecture](https://github.com/zircote/subcog/issues/43)
- **Related Bug**: [#42 - CaptureService attaches all notes to HEAD](https://github.com/zircote/subcog/issues/42)
- **Related Spec**: `2026-01-02-user-scope-storage-fallback` (tactical fix, this supersedes strategically)
