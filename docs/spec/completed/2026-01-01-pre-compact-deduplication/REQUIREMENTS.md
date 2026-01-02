---
document_type: requirements
project_id: SPEC-2026-01-01-001
version: 1.0.0
last_updated: 2026-01-01T00:00:00Z
status: draft
---

# Pre-Compact Deduplication - Product Requirements Document

## Executive Summary

The pre-compact hook documentation (`docs/hooks/pre-compact.md` lines 140-143) describes a three-tier deduplication system that checks for existing similar memories before auto-capture. The current implementation only performs within-batch prefix deduplication. This project implements the documented behavior: exact match checking, semantic similarity (>90%), and recent capture (5-minute window) deduplication against the memory store.

## Problem Statement

### The Problem

Auto-capture during pre-compact creates duplicate memories when:
1. The same decision/learning is mentioned multiple times across conversation
2. Similar phrasings of the same concept appear in different sections
3. Content is captured within minutes of identical prior captures

### Impact

- **Storage bloat**: Redundant memories consume storage and slow searches
- **Search noise**: Duplicates pollute search results with repetitive content
- **User confusion**: Recalling memories shows the same insight multiple times
- **Documentation mismatch**: Users expect documented behavior but get different results

### Current State

The `PreCompactHandler::deduplicate_candidates()` method (lines 152-184) only:
- Compares candidates within the same batch using 50-char prefix matching
- Does NOT query existing memories in storage
- Does NOT use embeddings for semantic similarity
- Does NOT track recent captures across invocations

## Goals and Success Criteria

### Primary Goal

Implement the documented deduplication logic that checks for existing similar memories before capture.

### Success Metrics

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Duplicate reduction | >80% reduction in duplicate auto-captures | Compare duplicate rate before/after |
| Performance | <100ms total hook latency | Metrics: `hook_duration_ms` histogram |
| Accuracy | <5% false positive rate (skipping unique content) | Manual review of 100 skipped captures |
| Semantic match quality | >90% similarity threshold properly applied | Unit tests with known similar pairs |

### Non-Goals (Explicit Exclusions)

- Retroactive deduplication of existing memories (use consolidation service)
- Cross-namespace deduplication (decisions vs learnings may be semantically similar but both valuable)
- User-facing deduplication controls in CLI (configuration only)
- Distributed deduplication across multiple subcog instances

## User Analysis

### Primary Users

- **Who**: AI coding assistants (Claude Code) using subcog hooks
- **Needs**: Clean, non-redundant memory capture with minimal overhead
- **Context**: Pre-compact hook runs before conversation compaction

### User Stories

1. As an AI assistant, I want auto-capture to skip memories that are identical to existing ones so that I don't store redundant content.
2. As an AI assistant, I want auto-capture to skip memories that are >90% semantically similar so that minor rephrasing doesn't create duplicates.
3. As an AI assistant, I want auto-capture to skip content captured in the last 5 minutes so that repeated mentions don't create duplicates.
4. As a developer, I want deduplication metrics so that I can monitor duplicate reduction effectiveness.
5. As a developer, I want configurable similarity thresholds per namespace so that I can tune accuracy.

## Functional Requirements

### Must Have (P0)

| ID | Requirement | Rationale | Acceptance Criteria |
|----|-------------|-----------|---------------------|
| FR-001 | Exact match deduplication | Skip identical content | Content hash comparison returns true for identical strings |
| FR-002 | Semantic similarity deduplication | Skip >90% similar content | Uses FastEmbed embeddings with cosine similarity threshold |
| FR-003 | Recent capture deduplication | Skip if captured <5 minutes ago | In-memory LRU cache with TTL tracks recent captures |
| FR-004 | RecallService integration | Query existing memories | Uses `RecallService.search()` with appropriate filters |
| FR-005 | Hook output includes skip reason | Observability | `additionalContext` lists skipped candidates with reasons |
| FR-006 | Metrics for deduplication events | Monitoring | `dedup_skipped_total{reason}` counter incremented |

### Should Have (P1)

| ID | Requirement | Rationale | Acceptance Criteria |
|----|-------------|-----------|---------------------|
| FR-101 | Per-namespace similarity thresholds | Different content types need different thresholds | Config: `SUBCOG_DEDUP_THRESHOLD_{NAMESPACE}` environment variables |
| FR-102 | Trace spans for each dedup check | Debugging | `tracing::span!` for exact/semantic/recent checks |
| FR-103 | Debug logging with fingerprints | Troubleshooting | Log at debug level with content hash and similarity scores |
| FR-104 | Graceful degradation when embeddings unavailable | Reliability | Falls back to text-only (exact match + BM25 similarity) |

### Nice to Have (P2)

| ID | Requirement | Rationale | Acceptance Criteria |
|----|-------------|-----------|---------------------|
| FR-201 | Dry-run deduplication | Testing | `--dry-run` shows what would be skipped without querying store |
| FR-202 | Cache hit rate metrics | Performance tuning | `dedup_cache_hits_total` and `dedup_cache_misses_total` |
| FR-203 | Configurable time window | Flexibility | `SUBCOG_DEDUP_TIME_WINDOW_SECS` (default 300) |

## Non-Functional Requirements

### Performance

- **Hook latency**: <100ms total (matches existing performance target)
- **Deduplication overhead**: <50ms added latency for all three checks combined
- **Memory footprint**: <10MB for recent capture cache (1000 entries max)
- **Index query**: <30ms for similarity search (5-10 results limit)

### Reliability

- **Graceful degradation**: If vector search fails, fall back to text-only
- **Cache failure tolerance**: If cache unavailable, proceed without recent-capture check
- **Error isolation**: Deduplication failures should not block capture

### Observability

- **Metrics**: Counters for each dedup type (exact/semantic/recent), histograms for latency
- **Tracing**: Spans for each check with attributes for scores and decisions
- **Logging**: Debug-level logs with content fingerprints (first 50 chars hash)

### Security

- **No content leakage**: Debug logs use hashes, not raw content
- **Memory safety**: Recent capture cache uses bounded LRU

## Technical Constraints

- **Rust 2024 edition**, MSRV 1.85
- **No panics**: Use `Result` types, never `unwrap()` in library code
- **Existing dependencies**: Use `fastembed` for embeddings, existing storage traits
- **Hook format**: Must return Claude Code hook JSON format

## Dependencies

### Internal Dependencies

- `RecallService` - For querying existing memories
- `FastEmbedEmbedder` - For generating embeddings
- `CaptureService` - For actual memory capture
- `SearchFilter`, `SearchMode` - For query construction

### External Dependencies

- `lru` crate - For TTL-based recent capture cache
- Existing: `serde`, `tracing`, `metrics`

## Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Semantic similarity too slow | Medium | Medium | Use vector search with limit=5, fall back to BM25 |
| False positives skip unique content | Medium | High | Start with 92% threshold, tune based on metrics |
| LRU cache memory growth | Low | Medium | Bounded 1000 entries, TTL eviction |
| Embedding model unavailable | Low | Medium | Graceful degradation to text-only |

## Open Questions

- [x] Should exact match use SHA256 hash or full content comparison? → **SHA256 for performance**
- [x] Should semantic search be namespace-scoped? → **Yes, search within same namespace**
- [x] How to handle embeddings for very short content (<50 chars)? → **Skip semantic check, use exact match only**

## Appendix

### Glossary

| Term | Definition |
|------|------------|
| Pre-compact hook | Claude Code hook that runs before conversation compaction |
| Semantic similarity | Cosine similarity of embedding vectors |
| BM25 | Probabilistic text relevance scoring algorithm |
| RRF | Reciprocal Rank Fusion for combining search scores |

### References

- [docs/hooks/pre-compact.md](../../hooks/pre-compact.md) - Pre-compact hook documentation
- [src/hooks/pre_compact.rs](../../../../src/hooks/pre_compact.rs) - Current implementation
- [src/services/recall.rs](../../../../src/services/recall.rs) - RecallService implementation
