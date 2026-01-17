---
document_type: implementation_plan
project_id: SPEC-2026-01-01-001
version: 1.0.0
last_updated: 2026-01-01T00:00:00Z
status: draft
estimated_effort: 3-4 days
---

# Pre-Compact Deduplication - Implementation Plan

## Overview

This plan implements the three-tier deduplication system for the pre-compact hook: exact match, semantic similarity, and recent capture detection. The implementation follows a bottom-up approach, building core components first and integrating them into the hook.

## Phase Summary

| Phase | Description | Tasks | Est. Effort |
|-------|-------------|-------|-------------|
| Phase 1: Foundation | Data types, configuration, content hashing | 5 tasks | 0.5 days |
| Phase 2: Checkers | Individual dedup checker implementations | 4 tasks | 1 day |
| Phase 3: Service | DeduplicationService orchestration | 3 tasks | 0.5 days |
| Phase 4: Integration | Hook integration and output formatting | 4 tasks | 0.5 days |
| Phase 5: Testing | Unit, integration, property, and benchmark tests | 5 tasks | 1 day |
| Phase 6: Observability | Metrics, tracing, logging | 3 tasks | 0.25 days |
| Phase 7: Documentation | Update docs, CLAUDE.md | 2 tasks | 0.25 days |

---

## Phase 1: Foundation

**Goal**: Establish core data types and utilities for deduplication.
**Prerequisites**: None

### Task 1.1: Create deduplication module structure

- **Description**: Create `src/services/deduplication/` module with mod.rs
- **Estimated Effort**: 0.5 hours
- **Dependencies**: None
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] `src/services/deduplication/mod.rs` created with submodule declarations
 - [ ] Re-exports from `src/services/mod.rs`
 - [ ] Compiles with `cargo check`

### Task 1.2: Define DuplicateCheckResult and DuplicateReason types

- **Description**: Create result types in `src/services/deduplication/types.rs`
- **Estimated Effort**: 0.5 hours
- **Dependencies**: None
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] `DuplicateCheckResult` struct with all fields from ARCHITECTURE.md
 - [ ] `DuplicateReason` enum (ExactMatch, SemanticSimilar, RecentCapture)
 - [ ] Implements `Debug`, `Clone`, `Serialize`, `Deserialize`
 - [ ] Doc comments with examples

### Task 1.3: Define DeduplicationConfig struct

- **Description**: Create configuration struct in `src/services/deduplication/config.rs`
- **Estimated Effort**: 0.5 hours
- **Dependencies**: None
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] `DeduplicationConfig` with all fields from ARCHITECTURE.md
 - [ ] `Default` implementation with documented defaults
 - [ ] `from_env()` method to load from environment variables
 - [ ] Per-namespace threshold loading: `SUBCOG_DEDUP_THRESHOLD_{NAMESPACE}`

### Task 1.4: Implement content hash utility

- **Description**: Create `ContentHasher` in `src/services/deduplication/hasher.rs`
- **Estimated Effort**: 1 hour
- **Dependencies**: `sha2` crate (add to Cargo.toml)
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] `ContentHasher::hash(content: &str) -> String` returns SHA256 hex
 - [ ] Content normalized: trim, lowercase, collapse whitespace
 - [ ] `hash_to_tag(hash: &str) -> String` returns `hash:sha256:<16-char-prefix>`
 - [ ] Unit tests for normalization edge cases

### Task 1.5: Add sha2 dependency

- **Description**: Add `sha2` crate to Cargo.toml
- **Estimated Effort**: 0.25 hours
- **Dependencies**: None
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] `sha2 = "0.10"` added to dependencies
 - [ ] `cargo build` succeeds

### Phase 1 Deliverables

- [ ] `src/services/deduplication/mod.rs`
- [ ] `src/services/deduplication/types.rs`
- [ ] `src/services/deduplication/config.rs`
- [ ] `src/services/deduplication/hasher.rs`

### Phase 1 Exit Criteria

- [ ] All types compile and have doc comments
- [ ] `cargo clippy` passes with no warnings
- [ ] Unit tests for ContentHasher pass

---

## Phase 2: Checkers

**Goal**: Implement the three individual deduplication checker components.
**Prerequisites**: Phase 1 complete

### Task 2.1: Implement ExactMatchChecker

- **Description**: Create `src/services/deduplication/exact_match.rs`
- **Estimated Effort**: 2 hours
- **Dependencies**: Task 1.4 (ContentHasher)
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] `ExactMatchChecker::new(recall: Arc<RecallService>)`
 - [ ] `check(&self, content: &str, namespace: Namespace) -> Result<Option<MemoryId>>`
 - [ ] Uses `SearchFilter::new().with_namespace(ns).with_tag(hash_tag)`
 - [ ] Returns `Some(id)` if exact match found, `None` otherwise
 - [ ] Unit tests with mock RecallService

### Task 2.2: Implement SemanticSimilarityChecker

- **Description**: Create `src/services/deduplication/semantic.rs`
- **Estimated Effort**: 3 hours
- **Dependencies**: Task 1.3 (DeduplicationConfig)
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] `SemanticSimilarityChecker::new(recall, embedder, thresholds)`
 - [ ] `check(&self, content: &str, namespace: Namespace) -> Result<Option<(MemoryId, f32)>>`
 - [ ] Generates embedding for content
 - [ ] Uses `SearchMode::Vector` with namespace filter
 - [ ] Returns highest similarity match above threshold
 - [ ] Skips if content length < `min_semantic_length`
 - [ ] Unit tests with mock embedder and recall

### Task 2.3: Implement RecentCaptureChecker

- **Description**: Create `src/services/deduplication/recent.rs`
- **Estimated Effort**: 2 hours
- **Dependencies**: `lru` crate, Task 1.4 (ContentHasher)
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] `RecentCaptureChecker::new(ttl: Duration, capacity: usize)`
 - [ ] `check(&self, content_hash: &str) -> Option<(MemoryId, Instant)>`
 - [ ] `record(&self, content_hash: &str, memory_id: &MemoryId)`
 - [ ] LRU cache with TTL-based eviction
 - [ ] Thread-safe with `RwLock` or `Mutex`
 - [ ] Unit tests for TTL expiration, capacity limits

### Task 2.4: Add lru dependency

- **Description**: Add `lru` crate to Cargo.toml
- **Estimated Effort**: 0.25 hours
- **Dependencies**: None
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] `lru = "0.12"` added to dependencies
 - [ ] `cargo build` succeeds

### Phase 2 Deliverables

- [ ] `src/services/deduplication/exact_match.rs`
- [ ] `src/services/deduplication/semantic.rs`
- [ ] `src/services/deduplication/recent.rs`

### Phase 2 Exit Criteria

- [ ] Each checker has unit tests
- [ ] `cargo test` passes for deduplication module
- [ ] `cargo clippy` passes

---

## Phase 3: Service

**Goal**: Create the DeduplicationService that orchestrates the three checkers.
**Prerequisites**: Phase 2 complete

### Task 3.1: Implement DeduplicationService core

- **Description**: Create `src/services/deduplication/service.rs`
- **Estimated Effort**: 2 hours
- **Dependencies**: All Phase 2 tasks
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] `DeduplicationService::new(recall, embedder, config)`
 - [ ] `without_embeddings(recall, config)` constructor
 - [ ] `check_duplicate(&self, content: &str, namespace: Namespace) -> Result<DuplicateCheckResult>`
 - [ ] Short-circuit: exact -> semantic -> recent
 - [ ] `record_capture(&self, content: &str, memory_id: &MemoryId)`
 - [ ] Graceful degradation on checker failures

### Task 3.2: Implement Deduplicator trait

- **Description**: Define trait for testability and future extension
- **Estimated Effort**: 0.5 hours
- **Dependencies**: Task 3.1
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] `pub trait Deduplicator: Send + Sync` in `types.rs`
 - [ ] `check_duplicate` and `record_capture` methods
 - [ ] `DeduplicationService` implements `Deduplicator`

### Task 3.3: Add DeduplicationService to ServiceContainer

- **Description**: Wire up service creation in `src/services/mod.rs`
- **Estimated Effort**: 0.5 hours
- **Dependencies**: Task 3.1
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] `ServiceContainer::deduplication() -> Option<&DeduplicationService>`
 - [ ] Created with appropriate dependencies from container
 - [ ] Feature-flagged based on `SUBCOG_DEDUP_ENABLED`

### Phase 3 Deliverables

- [ ] `src/services/deduplication/service.rs`
- [ ] Updated `src/services/mod.rs`

### Phase 3 Exit Criteria

- [ ] Service orchestrates all three checkers
- [ ] Short-circuit behavior verified in tests
- [ ] Graceful degradation tested

---

## Phase 4: Integration

**Goal**: Integrate DeduplicationService into PreCompactHandler.
**Prerequisites**: Phase 3 complete

### Task 4.1: Update PreCompactHandler to accept DeduplicationService

- **Description**: Add optional deduplication service field
- **Estimated Effort**: 1 hour
- **Dependencies**: Task 3.1
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] `PreCompactHandler::with_deduplication(dedup: DeduplicationService)`
 - [ ] Falls back to legacy prefix dedup if service not provided
 - [ ] Handler construction updated in main/CLI

### Task 4.2: Refactor deduplicate_candidates to use service

- **Description**: Replace prefix-based dedup with service call
- **Estimated Effort**: 1.5 hours
- **Dependencies**: Task 4.1
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] `deduplicate_candidates()` calls `dedup.check_duplicate()` for each candidate
 - [ ] Skips candidates where `result.is_duplicate == true`
 - [ ] Records skips with reason for output
 - [ ] Fail-open on service errors

### Task 4.3: Update hook output format to include skipped duplicates

- **Description**: Extend `additionalContext` with skip information
- **Estimated Effort**: 1 hour
- **Dependencies**: Task 4.2
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] Output includes "Skipped N duplicates" section
 - [ ] Each skip shows reason (Exact/Semantic/Recent) and matched ID
 - [ ] Semantic matches show similarity percentage
 - [ ] Format matches ARCHITECTURE.md example

### Task 4.4: Record captures in deduplication service

- **Description**: Call `record_capture` after successful capture
- **Estimated Effort**: 0.5 hours
- **Dependencies**: Task 4.2
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] `dedup.record_capture()` called after `capture_service.capture()` succeeds
 - [ ] Hash computed once and reused for efficiency

### Phase 4 Deliverables

- [ ] Updated `src/hooks/pre_compact.rs`

### Phase 4 Exit Criteria

- [ ] Hook uses deduplication service when available
- [ ] Output format includes skip information
- [ ] Legacy behavior preserved when service unavailable

---

## Phase 5: Testing

**Goal**: Comprehensive test coverage for all deduplication components.
**Prerequisites**: Phase 4 complete

### Task 5.1: Unit tests for all checkers

- **Description**: Expand unit tests in each checker module
- **Estimated Effort**: 2 hours
- **Dependencies**: Phase 2 complete
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] ExactMatchChecker: hash generation, tag format, namespace filtering
 - [ ] SemanticSimilarityChecker: threshold comparison, min length skip
 - [ ] RecentCaptureChecker: TTL expiration, capacity eviction
 - [ ] >80% coverage for deduplication module

### Task 5.2: Integration tests with real backends

- **Description**: Create `tests/deduplication_integration.rs`
- **Estimated Effort**: 3 hours
- **Dependencies**: Phase 3 complete
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] Test with real SQLite index backend
 - [ ] Test with real usearch vector backend
 - [ ] End-to-end hook invocation with deduplication
 - [ ] Verify skipped duplicates in output

### Task 5.3: Property-based tests for similarity

- **Description**: Add proptest for fuzzy similarity edge cases
- **Estimated Effort**: 2 hours
- **Dependencies**: `proptest` crate (already in devDependencies)
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] Identical content always detected as duplicate
 - [ ] Slight variations not detected as exact match
 - [ ] Threshold boundary behavior correct

### Task 5.4: Benchmark tests for performance

- **Description**: Add `benches/deduplication.rs`
- **Estimated Effort**: 1.5 hours
- **Dependencies**: Phase 3 complete
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] Benchmark `check_duplicate` cold path (<50ms)
 - [ ] Benchmark `check_duplicate` with cached embedding (<30ms)
 - [ ] Benchmark recent capture lookup (<1ms)
 - [ ] CI fails if performance regresses >20%

### Task 5.5: Graceful degradation tests

- **Description**: Test behavior when components unavailable
- **Estimated Effort**: 1.5 hours
- **Dependencies**: Task 3.1
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] Test with no embedder (falls back to exact + recent)
 - [ ] Test with index error (proceeds with capture)
 - [ ] Test with cache disabled (skips recent check)

### Phase 5 Deliverables

- [ ] Unit tests in `src/services/deduplication/*.rs`
- [ ] `tests/deduplication_integration.rs`
- [ ] `benches/deduplication.rs`

### Phase 5 Exit Criteria

- [ ] >80% code coverage for deduplication
- [ ] All property tests pass
- [ ] Benchmarks meet performance targets
- [ ] CI pipeline passes

---

## Phase 6: Observability

**Goal**: Add metrics, tracing, and logging for deduplication operations.
**Prerequisites**: Phase 4 complete

### Task 6.1: Add deduplication metrics

- **Description**: Emit Prometheus-style metrics for dedup operations
- **Estimated Effort**: 1 hour
- **Dependencies**: Task 3.1
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] `dedup_checks_total{namespace, result}` counter
 - [ ] `dedup_skipped_total{namespace, reason}` counter
 - [ ] `dedup_check_duration_ms{check_type}` histogram
 - [ ] `dedup_similarity_score{namespace}` histogram
 - [ ] `dedup_cache_size` gauge

### Task 6.2: Add tracing spans

- **Description**: Instrument each check with tracing spans
- **Estimated Effort**: 0.75 hours
- **Dependencies**: Task 3.1
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] `#[instrument]` on `check_duplicate` with fields
 - [ ] Child spans for exact/semantic/recent checks
 - [ ] Span attributes: content_length, namespace, is_duplicate, reason

### Task 6.3: Add debug logging

- **Description**: Log dedup decisions at debug level
- **Estimated Effort**: 0.5 hours
- **Dependencies**: Task 3.1
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] Log content hash (not content) for each check
 - [ ] Log similarity scores for semantic matches
 - [ ] Log cache hits/misses for recent captures
 - [ ] No content leakage in logs

### Phase 6 Deliverables

- [ ] Metrics in `src/services/deduplication/service.rs`
- [ ] Tracing spans in checker modules

### Phase 6 Exit Criteria

- [ ] Grafana dashboard can display dedup metrics
- [ ] Jaeger shows dedup spans in traces
- [ ] Debug logs show hash fingerprints only

---

## Phase 7: Documentation

**Goal**: Update documentation to reflect implementation.
**Prerequisites**: Phase 6 complete

### Task 7.1: Update docs/hooks/pre-compact.md

- **Description**: Expand deduplication section with implementation details
- **Estimated Effort**: 0.75 hours
- **Dependencies**: All implementation complete
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] Document exact match (SHA256 hash tag)
 - [ ] Document semantic similarity (configurable thresholds)
 - [ ] Document recent capture (LRU cache with TTL)
 - [ ] Add configuration section with env vars
 - [ ] Update performance targets

### Task 7.2: Update CLAUDE.md with new service

- **Description**: Add deduplication service to project structure
- **Estimated Effort**: 0.25 hours
- **Dependencies**: Task 7.1
- **Assignee**: TBD
- **Acceptance Criteria**:
 - [ ] Add `deduplication/` to services directory listing
 - [ ] Document new environment variables
 - [ ] Link to spec documents

### Phase 7 Deliverables

- [ ] Updated `docs/hooks/pre-compact.md`
- [ ] Updated `CLAUDE.md`

### Phase 7 Exit Criteria

- [ ] Documentation accurately reflects implementation
- [ ] Configuration options documented
- [ ] No stale information

---

## Dependency Graph

```
Phase 1: Foundation
 Task 1.1 (module structure) ──┐
 Task 1.2 (types) ─┼──▶ Phase 2: Checkers
 Task 1.3 (config) ─┤
 Task 1.4 (hasher) ◀──────────┤
 Task 1.5 (sha2) ───────────┘
 │
 ▼
Phase 2: Checkers
 Task 2.1 (exact match) ───────┐
 Task 2.2 (semantic) ──────────┼──▶ Phase 3: Service
 Task 2.3 (recent) ────────────┤
 Task 2.4 (lru) ───────────────┘
 │
 ▼
Phase 3: Service
 Task 3.1 (core service) ──────┐
 Task 3.2 (trait) ─────────────┼──▶ Phase 4: Integration
 Task 3.3 (container) ─────────┘
 │
 ├──▶ Phase 5: Testing (parallel start possible)
 │
 ▼
Phase 4: Integration
 Task 4.1 (handler update) ────┐
 Task 4.2 (dedup refactor) ────┤
 Task 4.3 (output format) ─────┼──▶ Phase 6: Observability
 Task 4.4 (record captures) ───┘
 │
 ▼
Phase 6: Observability
 Task 6.1 (metrics) ───────────┐
 Task 6.2 (tracing) ───────────┼──▶ Phase 7: Documentation
 Task 6.3 (logging) ───────────┘
```

## Risk Mitigation Tasks

| Risk | Mitigation Task | Phase |
|------|-----------------|-------|
| Semantic similarity too slow | Task 5.4 benchmarks, optimize vector search limit | 5 |
| False positives skip unique content | Task 5.3 property tests, conservative 92% threshold | 5 |
| LRU cache memory growth | Task 2.3 bounded capacity, Task 5.1 capacity tests | 2, 5 |
| Embedding model unavailable | Task 3.1 graceful degradation, Task 5.5 degradation tests | 3, 5 |

## Testing Checklist

- [ ] Unit tests for ContentHasher
- [ ] Unit tests for ExactMatchChecker
- [ ] Unit tests for SemanticSimilarityChecker
- [ ] Unit tests for RecentCaptureChecker
- [ ] Unit tests for DeduplicationService
- [ ] Integration tests with SQLite
- [ ] Integration tests with usearch
- [ ] Integration tests for hook output
- [ ] Property tests for similarity edge cases
- [ ] Benchmark tests for performance targets
- [ ] Graceful degradation tests

## Documentation Tasks

- [ ] Update docs/hooks/pre-compact.md
- [ ] Update CLAUDE.md
- [ ] Add inline doc comments
- [ ] Update ARCHITECTURE.md if design changes

## Launch Checklist

- [ ] All tests passing (`cargo test`)
- [ ] Clippy clean (`cargo clippy --all-targets --all-features`)
- [ ] Format check (`cargo fmt -- --check`)
- [ ] Documentation complete (`cargo doc --no-deps`)
- [ ] Benchmarks meet targets (`cargo bench`)
- [ ] Supply chain audit (`cargo deny check`)
- [ ] Feature flag tested (enable/disable)
- [ ] Metrics visible in Grafana
- [ ] Traces visible in Jaeger

## Post-Launch

- [ ] Monitor `dedup_skipped_total` metrics for 48 hours
- [ ] Review false positive rate from logs
- [ ] Tune thresholds if needed
- [ ] Archive planning documents to completed/
