---
project_id: SPEC-2026-01-01-001
project_name: "Pre-Compact Deduplication"
slug: pre-compact-deduplication
status: completed
created: 2026-01-01T00:00:00Z
approved: null
started: 2026-01-01T00:00:00Z
completed: 2026-01-02T00:00:00Z
final_effort: 8 hours
outcome: success
expires: 2026-04-01T00:00:00Z
superseded_by: null
tags: [hooks, deduplication, pre-compact, semantic-similarity]
stakeholders: []
worktree:
  branch: plan/pre-compact-deduplication
  base_branch: main
---

# Pre-Compact Deduplication

Implement the documented deduplication logic for the pre-compact hook that checks for existing similar memories before capture.

## Overview

The documentation in `docs/hooks/pre-compact.md` lines 140-143 describes deduplication logic that is not currently implemented:

1. **Exact match** - Skips if identical content exists (SHA256 hash comparison)
2. **Semantic similarity** - Skips if >90% similar memory exists (FastEmbed cosine similarity)
3. **Recent capture** - Skips if captured in last 5 minutes (LRU cache with TTL)

The current implementation only performs within-batch prefix deduplication (first 50 chars).

## Status

- **Phase**: Specification complete, awaiting approval
- **Estimated Effort**: 3-4 days
- **Blockers**: None

## Summary

| Metric | Target |
|--------|--------|
| Duplicate reduction | >80% |
| Performance overhead | <50ms |
| False positive rate | <5% |

## Key Features

- **DeduplicationService**: New service orchestrating three-tier dedup
- **Per-namespace thresholds**: Configurable via `SUBCOG_DEDUP_THRESHOLD_{NAMESPACE}`
- **Graceful degradation**: Falls back to text-only when embeddings unavailable
- **Full observability**: Metrics, traces, and debug logging

## Key Documents

- [REQUIREMENTS.md](./REQUIREMENTS.md) - Product requirements (26 functional requirements)
- [ARCHITECTURE.md](./ARCHITECTURE.md) - Technical design with component diagrams
- [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md) - 7 phases, 26 tasks
- [DECISIONS.md](./DECISIONS.md) - 8 architecture decision records
