# Changelog

All notable changes to this specification will be documented in this file.

## [Unreleased]

### Added

- Initial project creation
- Requirements elicitation completed
- Documented 5 critical architectural gaps from architecture review
- REQUIREMENTS.md with complete PRD
  - 5 functional requirement groups (FR-001 through FR-005)
  - 3 non-functional requirement categories (NFR-001 through NFR-003)
  - 7 testing requirements (TR-001 through TR-007)
  - 4 user stories
  - Success metrics and risk assessment
- ARCHITECTURE.md with technical design
  - Current vs target architecture diagrams
  - Component design for FastEmbedEmbedder, RecallService, CaptureService
  - Score normalization strategy
  - Graceful degradation matrix
  - Data flow diagrams for capture and recall
  - Integration points with ServiceContainer
  - Migration strategy for existing memories
  - Testing strategy with unit, integration, and property tests
  - Performance and security considerations
- IMPLEMENTATION_PLAN.md with 5 phases
  - Phase 1: Real Embeddings (7 tasks)
  - Phase 2: RecallService Integration (8 tasks)
  - Phase 3: CaptureService Integration (9 tasks)
  - Phase 4: Score Normalization (7 tasks)
  - Phase 5: Testing & Migration (9 tasks)
  - Risk mitigation strategies
  - Rollout plan
  - Success criteria
- DECISIONS.md with 8 Architecture Decision Records
  - ADR-001: Use fastembed-rs for embeddings
  - ADR-002: Lazy load embedding model
  - ADR-003: Three-layer storage synchronization
  - ADR-004: Score normalization to 0.0-1.0
  - ADR-005: Graceful degradation strategy
  - ADR-006: Model selection (all-MiniLM-L6-v2)
  - ADR-007: Vector index (usearch)
  - ADR-008: Backward compatibility
- RESEARCH_NOTES.md with investigation findings
  - Complete analysis of embedding infrastructure
  - Vector backend analysis
  - Capture and recall service analysis
  - Test infrastructure gaps
  - fastembed-rs integration research
  - Benchmark targets
  - Risk assessment

### Research Findings

1. **MEM-001**: `FastEmbedEmbedder` uses hash-based pseudo-embeddings (lines 46-74 in `fastembed.rs`)
2. **MEM-002**: `vector_search()` is a const fn stub returning empty (lines 241-250 in `recall.rs`)
3. **MEM-003**: `CaptureService` never calls index or vector backends (lines 119-130 in `capture.rs`)
4. **MEM-004**: `RecallService` lacks embedder and vector fields (lines 19-22 in `recall.rs`)
5. **MEM-005**: RRF with K=60 produces max scores of ~0.016 by design (lines 310-365 in `recall.rs`)

### User Requirements Gathered

- **Scope**: All 5 issues to be addressed
- **Embedding Provider**: fastembed-rs with all-MiniLM-L6-v2
- **Testing Depth**: Comprehensive with integration and property tests
- **Priority**: Production quality - reliability over speed
