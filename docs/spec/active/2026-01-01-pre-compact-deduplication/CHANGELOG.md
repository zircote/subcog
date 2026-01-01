# Changelog

All notable changes to this specification will be documented in this file.

## [1.0.0] - 2026-01-01

### Added

- Complete requirements specification (REQUIREMENTS.md)
  - 6 P0 (must have) requirements
  - 4 P1 (should have) requirements
  - 3 P2 (nice to have) requirements
  - Performance targets: <100ms hook latency, <50ms dedup overhead
  - Accuracy targets: >80% duplicate reduction, <5% false positive rate

- Technical architecture design (ARCHITECTURE.md)
  - DeduplicationService with three checkers
  - ExactMatchChecker using SHA256 hash tags
  - SemanticSimilarityChecker using FastEmbed embeddings
  - RecentCaptureChecker using LRU cache with TTL
  - Graceful degradation strategy
  - Full API design with configuration

- Implementation plan (IMPLEMENTATION_PLAN.md)
  - 7 phases, 26 tasks
  - Estimated effort: 3-4 days
  - Comprehensive testing strategy
  - Observability plan (metrics, traces, logs)

- Architecture decision records (DECISIONS.md)
  - ADR-001: Short-circuit evaluation order
  - ADR-002: Content hash storage as tags
  - ADR-003: Per-namespace similarity thresholds
  - ADR-004: In-memory LRU cache for recent captures
  - ADR-005: Fail-open on deduplication errors
  - ADR-006: Semantic check minimum length
  - ADR-007: RecallService for deduplication lookups
  - ADR-008: Hook output format for skip reporting

### Research Conducted

- Analyzed existing codebase patterns:
  - RecallService search implementation
  - FastEmbed embedder interface
  - VectorBackend cosine similarity
  - PreCompactHandler current deduplication

- Identified gap between documentation and implementation:
  - Documentation claims 3 dedup checks
  - Implementation only has prefix-based batch dedup
