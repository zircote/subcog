---
project_id: SPEC-2026-01-02-001
project_name: "Subcog Memory System Critical Fixes"
slug: memory-system-critical-fixes
status: draft
created: 2026-01-02T00:00:00Z
approved: null
started: null
completed: null
expires: 2026-04-02T00:00:00Z
superseded_by: null
parent_spec: "2025-12-28-subcog-rust-rewrite"
tags: [embeddings, vector-search, recall, storage-sync, critical-bug, fastembed-rs]
stakeholders: []
---

# Subcog Memory System Critical Fixes

## Overview

This specification addresses **5 critical architectural gaps** identified during an architecture review that explain why the memory capture/recall system is not functioning correctly:

1. Memories saved but recall returns nothing
2. Relevance scores are extremely low (0.01-0.02 instead of 0.5-0.9)

## Root Causes Identified

| ID | Issue | Severity | File | Impact |
|----|-------|----------|------|--------|
| MEM-001 | Placeholder embeddings (hash-based, not semantic) | CRITICAL | `src/embedding/fastembed.rs:46-74` | No semantic search works |
| MEM-002 | Vector search stub returns empty | CRITICAL | `src/services/recall.rs:241-250` | Hybrid search = text-only |
| MEM-003 | Storage layers not synchronized | HIGH | `src/services/capture.rs:119-130` | Captured memories not searchable |
| MEM-004 | RecallService missing backends | HIGH | `src/services/recall.rs:19-35` | Cannot implement vector search |
| MEM-005 | RRF produces low scores by design | MEDIUM | `src/services/recall.rs:310-365` | Confusing UX |

## Solution Summary

| Issue | Solution | ADR |
|-------|----------|-----|
| MEM-001 | Replace with real fastembed-rs integration | ADR-001, ADR-006 |
| MEM-002 | Implement vector_search with embedder + backend | ADR-005 |
| MEM-003 | Sync all three storage layers on capture | ADR-003 |
| MEM-004 | Add embedder and vector fields to RecallService | ADR-007 |
| MEM-005 | Normalize scores to 0.0-1.0 range | ADR-004 |

## Quick Links

- [REQUIREMENTS.md](./REQUIREMENTS.md) - Product requirements (5 FR groups, 3 NFR categories, 4 user stories)
- [ARCHITECTURE.md](./ARCHITECTURE.md) - Technical design (component diagrams, data flows, integration points)
- [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md) - 5 phases, 40 tasks, ~2 week timeline
- [DECISIONS.md](./DECISIONS.md) - 8 Architecture Decision Records
- [RESEARCH_NOTES.md](./RESEARCH_NOTES.md) - Investigation findings, fastembed-rs research
- [CHANGELOG.md](./CHANGELOG.md) - Specification changelog

## Implementation Phases

| Phase | Focus | Tasks | Dependencies |
|-------|-------|-------|--------------|
| 1 | Real Embeddings (MEM-001) | 7 | None |
| 2 | RecallService Integration (MEM-002, MEM-004) | 8 | Phase 1 |
| 3 | CaptureService Integration (MEM-003) | 9 | Phase 1 |
| 4 | Score Normalization (MEM-005) | 7 | Phase 2 |
| 5 | Testing & Migration | 9 | Phases 1-4 |

## Key Decisions

| ADR | Decision | Status |
|-----|----------|--------|
| ADR-001 | Use fastembed-rs for embeddings | Accepted |
| ADR-002 | Lazy load embedding model via OnceLock | Accepted |
| ADR-003 | Synchronous best-effort storage sync | Accepted |
| ADR-004 | Linear score normalization to 0.0-1.0 | Accepted |
| ADR-005 | Graceful degradation when components fail | Accepted |
| ADR-006 | all-MiniLM-L6-v2 as default model | Accepted |
| ADR-007 | usearch for vector index (already implemented) | Accepted |
| ADR-008 | Backward compatibility with manual migration | Accepted |

## Success Criteria

| Metric | Before | After |
|--------|--------|-------|
| Recall success rate | ~10% | >90% |
| Average relevance score | 0.01 | >0.5 |
| Recall service tests | 5 | 20+ |
| Total tests | ~820 | ~900 |
| Capture-to-searchable latency | N/A | <100ms |

## Status

**Current Phase**: Draft - Awaiting Approval

**Documents Complete**:
- [x] REQUIREMENTS.md
- [x] ARCHITECTURE.md
- [x] IMPLEMENTATION_PLAN.md
- [x] DECISIONS.md
- [x] RESEARCH_NOTES.md
- [x] CHANGELOG.md

**Next Steps**:
1. Review and approve specification
2. Begin Phase 1: Real Embeddings implementation
