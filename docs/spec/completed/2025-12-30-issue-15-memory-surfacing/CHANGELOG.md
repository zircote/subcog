# Changelog

All notable changes to this specification will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

## [2.0.0] - 2025-12-30

### Completed

- **Status**: Project completed successfully
- **Outcome**: All 77 tasks across 6 phases delivered
- **Test Coverage**: 388 tests passing (361 unit + 22 integration + 5 doc)
- **Performance**: All benchmarks pass (keyword <10ms, LLM <200ms, injection <50ms)
- **CI/CD**: All gates pass (fmt, clippy, test, doc, deny, bench)
- **Pull Request**: [#23](https://github.com/zircote/subcog/pull/23) merged
- **Issues Closed**: #15, #24

### Delivered Features

- **6 SearchIntentType variants**: HowTo, Location, Explanation, Comparison, Troubleshoot, General
- **Hybrid detection**: Keyword (<10ms) + optional LLM (<200ms) with timeout fallback
- **Namespace weighting**: Intent-specific memory prioritization (HowTo → Patterns 1.5x)
- **3 new MCP resources**:
  - `subcog://search/{query}` - Query-based memory search
  - `subcog://topics` - List all indexed topics
  - `subcog://topics/{topic}` - Memories for specific topic
- **6 new MCP prompts**: intent_search, query_suggest, discover, generate_decision, generate_tutorial, context_capture
- **5 hook response format fixes**: SessionStart, UserPromptSubmit, Stop, PostToolUse, PreCompact (Issue #24)

### Implementation Phases

1. **Phase 1 - Foundation**: Search intent detection with keyword patterns and confidence scoring
2. **Phase 2 - Adaptive Injection**: Memory context builder with namespace weighting and token budget
3. **Phase 3 - MCP Resources**: Topic index service and resource handlers for query/topic access
4. **Phase 4 - MCP Prompts**: Enhanced prompts for intent-based search and context capture
5. **Phase 5 - LLM Classification**: Anthropic Claude integration with timeout and fallback
6. **Phase 6 - Polish**: Hook guidance, graceful degradation tests, benchmarks

### Additional Work

- **Issue #24 fix**: All 5 hooks brought into compliance with Claude Code specification
  - Response format changed from `{continue, context, metadata}` to `{hookSpecificOutput: {hookEventName, additionalContext}}`
  - Metadata embedded as XML comments for debugging
- **PR review feedback**: Fixed duplicate topic filtering bug in `extract_topics()` (HashSet wasn't being mutated)
- **35 benchmark tests**: Added comprehensive performance benchmarks beyond original plan
- **11 graceful degradation tests**: Added integration tests for LLM fallback, timeout, and component unavailability

### Key Learnings

1. **Rust ownership patterns**: `map_or_else` preferred over `if let Some(...)` per clippy::option_if_let_else
2. **Hook response formats**: Claude Code spec requires `hookSpecificOutput.additionalContext`, not `continue + context` format
3. **HashSet mutation**: Must declare `mut` AND call `.insert()` for deduplication to work
4. **Criterion benchmarking**: Excellent baseline for tracking performance targets over time
5. **Graceful degradation testing**: Testing fallback paths is critical for production readiness

### References

- [Retrospective](./RETROSPECTIVE.md) - Full project retrospective with metrics and learnings
- [Pull Request #23](https://github.com/zircote/subcog/pull/23) - Implementation PR
- [Issue #15](https://github.com/zircote/subcog/issues/15) - Parent issue
- [Issue #24](https://github.com/zircote/subcog/issues/24) - Hook response format fixes

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
