# Changelog

All notable changes to this specification will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

## [1.0.1] - 2025-12-30

### Approved
- Spec approved by Robert Allen <zircote@gmail.com>
- Status changed from draft to approved
- Ready for implementation via /claude-spec:implement

## [1.0.0] - 2025-12-30

### Added

- Initial specification created from GitHub Issue #15
- Requirements document (REQUIREMENTS.md) with 9 P0, 6 P1, and 4 P2 requirements
- Technical architecture (ARCHITECTURE.md) with 5 major components:
  - SearchIntentDetector for keyword and LLM-based intent detection
  - SearchContextBuilder for adaptive memory injection
  - TopicIndexService for topic-based memory lookup
  - Enhanced ResourceHandler for new MCP resources
  - Enhanced PromptRegistry for new MCP prompts
- Implementation plan (IMPLEMENTATION_PLAN.md) with 6 phases and 66 tasks:
  - Phase 1: Foundation - Search Intent Detection (11 tasks)
  - Phase 2: Adaptive Memory Injection (12 tasks)
  - Phase 3: MCP Resources - Query & Topic (16 tasks)
  - Phase 4: Enhanced MCP Prompts (10 tasks)
  - Phase 5: LLM Intent Classification (12 tasks)
  - Phase 6: Hook Guidance & Polish (16 tasks)
- 6 Architecture Decision Records (DECISIONS.md):
  - ADR-001: Hybrid Detection Strategy
  - ADR-002: Namespace Weighting Over Query Rewriting
  - ADR-003: In-Memory Topic Index
  - ADR-004: 200ms LLM Timeout
  - ADR-005: Token Budget for Injected Memories
  - ADR-006: Confidence Threshold for Injection

### Context

This specification implements proactive memory surfacing for subcog, transforming the system from reactive (explicit recall) to proactive (automatic intent-based injection). The design draws from GitHub Issue #15 and subissues #16-#21.

### Key Design Decisions

1. Hybrid detection (keyword + LLM) for best accuracy/latency tradeoff
2. Namespace weights per intent type instead of query rewriting
3. In-memory topic index for fast lookups
4. 200ms LLM timeout with keyword fallback
5. Token budget to prevent context bloat
6. 0.5 confidence threshold for injection

### References

- [Parent Issue #15](https://github.com/zircote/subcog/issues/15)
- [Phase 1 (#16)](https://github.com/zircote/subcog/issues/16) - Foundation
- [Phase 2 (#17)](https://github.com/zircote/subcog/issues/17) - Adaptive Injection
- [Phase 3 (#18)](https://github.com/zircote/subcog/issues/18) - MCP Resources
- [Phase 4 (#19)](https://github.com/zircote/subcog/issues/19) - MCP Prompts
- [Phase 5 (#20)](https://github.com/zircote/subcog/issues/20) - LLM Classification
- [Phase 6 (#21)](https://github.com/zircote/subcog/issues/21) - Polish
