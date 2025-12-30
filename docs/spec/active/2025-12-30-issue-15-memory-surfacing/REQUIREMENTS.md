---
document_type: requirements
project_id: SPEC-2025-12-30-001
version: 1.0.0
last_updated: 2025-12-30T12:00:00Z
status: draft
github_issue: 15
---

# Proactive Memory Surfacing - Product Requirements Document

## Executive Summary

This project implements proactive memory surfacing for subcog, transforming the memory system from reactive (memories surfaced only on explicit recall) to proactive (memories automatically surfaced when relevant based on detected user intent). The solution uses search intent detection in user prompts combined with MCP resource-based surfacing to provide contextual memories exactly when the AI assistant needs them.

## Problem Statement

### The Problem

Currently, subcog's memory system is purely reactive - memories are only surfaced when:
1. The assistant explicitly calls `memory.recall`
2. SessionStart injects generic context at session start

This misses critical opportunities to provide relevant context when:
1. **User prompts suggest information seeking** - Questions like "how do I...", "where is...", "find the..." indicate the assistant is about to search the codebase
2. **The assistant uses search tools** - Grep, Glob, Read operations could benefit from pre-existing memory context
3. **Topics discussed have relevant prior decisions** - Previous decisions/patterns should inform current work

The current `UserPromptSubmit` hook only detects memory *capture* signals, not *recall* opportunities.

### Impact

- **Lost context**: AI assistants repeat investigations that prior sessions already completed
- **Inconsistent decisions**: Without surfaced prior decisions, assistants may make conflicting choices
- **Wasted effort**: Users must manually prompt for memory recall even when the need is obvious
- **Reduced trust**: Users don't see the memory system providing value proactively

### Current State

The existing `UserPromptSubmit` hook (`src/hooks/user_prompt.rs`) has:
- Pattern detection for capture signals (decisions, patterns, learnings, blockers, tech-debt)
- Confidence scoring based on pattern matches, length, and sentence structure
- Explicit command detection (`@subcog capture/remember/save/store`)

The hook currently **does not**:
- Detect search intent signals
- Inject memories proactively
- Provide namespace-weighted recall based on query type

## Goals and Success Criteria

### Primary Goal

Enable subcog to **proactively surface relevant memories** when user prompts indicate information-seeking intent, without requiring explicit recall commands.

### Success Metrics

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Search intent detection accuracy | >80% | Test suite with labeled prompts |
| Keyword detection latency | <10ms | Benchmark suite |
| LLM classification latency | <200ms (with fallback) | Benchmark suite |
| Total UserPromptSubmit latency | <200ms | End-to-end benchmarks |
| Memory injection latency | <50ms | Benchmark suite |
| Topic index build time | <100ms for 1000 memories | Benchmark suite |
| Test coverage (new code) | >90% | cargo tarpaulin |

### Non-Goals (Explicit Exclusions)

- **Real-time streaming** - Memory injection happens before response, not during
- **PostToolUse enhancement** - Focus is on UserPromptSubmit hook (PostToolUse already surfaces related memories)
- **Memory ranking improvements** - Use existing RRF fusion, don't redesign ranking
- **UI changes** - No changes to Claude Code UI, only hook responses
- **New capture mechanisms** - Focus is on recall, not capture

## User Analysis

### Primary Users

- **Who**: AI coding assistants (Claude) operating via Claude Code hooks
- **Needs**: Contextual memories surfaced automatically when working on information-seeking tasks
- **Context**: Running in Claude Code sessions, processing user prompts before generating responses

### Secondary Users

- **Who**: Developers using Claude Code with subcog
- **Needs**: See relevant prior decisions and patterns without manual recall
- **Context**: Asking questions, exploring codebases, making technical decisions

### User Stories

1. As an AI assistant, I want to receive relevant prior decisions when a user asks "how do I implement authentication?", so that I can provide consistent advice based on established patterns.

2. As an AI assistant, I want to receive debugging patterns when a user asks "why is the test failing?", so that I can apply previously learned troubleshooting approaches.

3. As an AI assistant, I want to access topic-based memory resources, so that I can proactively fetch context for specific technical domains.

4. As a developer, I want subcog to automatically surface relevant context when I ask questions, so that I don't need to remember to manually request memory recall.

5. As a developer, I want the memory surfacing to be fast and non-intrusive, so that it doesn't slow down my interactions with the AI assistant.

## Functional Requirements

### Must Have (P0)

| ID | Requirement | Rationale | Acceptance Criteria | Issue |
|----|-------------|-----------|---------------------|-------|
| FR-001 | Detect search intent in user prompts using keyword patterns | Foundation for proactive surfacing | Detects HowTo, Location, Explanation, Comparison, Troubleshoot, General intent types with >80% accuracy | #16 |
| FR-002 | Calculate confidence score for search intent detection | Enables threshold-based injection | Confidence ranges 0.0-1.0, correlates with actual intent presence | #16 |
| FR-003 | Extract topics from user prompts | Enables targeted memory retrieval | Topics extracted match semantic content of prompt | #16 |
| FR-004 | Inject relevant memories into UserPromptSubmit response | Core value delivery | Memories appear in hook response metadata when confidence >= threshold | #17 |
| FR-005 | Adapt memory count based on query complexity/confidence | Prevents noise from irrelevant memories | 5 memories (low), 10 (medium), 15 (high confidence) | #17 |
| FR-006 | Apply namespace weights based on detected intent type | Improves recall relevance | HowTo boosts Patterns, Troubleshoot boosts Blockers, etc. | #17 |
| FR-007 | Implement `subcog://search/{query}` resource | Enables on-demand semantic search via MCP | Returns JSON with matching memories and scores | #18 |
| FR-008 | Implement `subcog://topics` resource | Enables topic discovery | Lists all indexed topics with memory counts | #18 |
| FR-009 | Implement `subcog://topics/{topic}` resource | Enables topic-specific retrieval | Returns memories associated with topic | #18 |

### Should Have (P1)

| ID | Requirement | Rationale | Acceptance Criteria | Issue |
|----|-------------|-----------|---------------------|-------|
| FR-101 | Add `search_with_context` MCP prompt | Guided memory-aware search | Prompt listed, includes query and scope args, generates contextual guidance | #19 |
| FR-102 | Add `research_topic` MCP prompt | Deep topic research workflow | Prompt listed, includes topic and depth args, suggests comprehensive approach | #19 |
| FR-103 | Enhance `recall_context` prompt with intent awareness | Better prompt guidance | Intent arg added, guidance varies by intent type | #19 |
| FR-104 | LLM-based intent classification | Higher accuracy than keywords alone | Classification accuracy >90% on test set, <200ms timeout | #20 |
| FR-105 | Hybrid detection (keyword + LLM) | Best of both approaches | LLM results merged with keyword results, LLM improves topic extraction | #20 |
| FR-106 | Graceful fallback when LLM unavailable | Maintain functionality | Falls back to keyword-only, logs warning, continues operation | #20 |

### Nice to Have (P2)

| ID | Requirement | Rationale | Acceptance Criteria | Issue |
|----|-------------|-----------|---------------------|-------|
| FR-201 | Memory context field in hook response | Rich assistant guidance | `memory_context` object with intent_type, topics, suggested_resources | #21 |
| FR-202 | Conditional reminder text | Guide memory usage | Reminder included when confidence >= 0.5 | #21 |
| FR-203 | Configuration options for search intent | User customization | `SearchIntentConfig` with enabled, use_llm, llm_timeout_ms, min_confidence | #21 |
| FR-204 | Performance benchmarks in CI | Prevent regression | Benchmarks run in CI, fail on >10% regression | #21 |

## Non-Functional Requirements

### Performance

| Metric | Requirement | Rationale |
|--------|-------------|-----------|
| Keyword detection | <10ms | Near-instant response, no perceptible delay |
| LLM classification | <200ms (or timeout) | Reasonable wait with guaranteed fallback |
| Memory retrieval | <50ms | Keep total latency under 200ms |
| Total UserPromptSubmit | <200ms | Maintain responsive UX |
| Topic index build | <100ms for 1000 memories | Fast server startup |

### Reliability

| Requirement | Implementation |
|-------------|----------------|
| LLM timeout handling | 200ms timeout with automatic keyword fallback |
| Embedding service failure | Fall back to text search (BM25 only) |
| Index unavailable | Skip memory injection, log warning |
| Feature toggle | `enabled` flag in config to disable entirely |

### Scalability

| Consideration | Approach |
|---------------|----------|
| Memory count | Tested with 1000+ memories, O(log n) search |
| Topic count | Lazy loading, pagination if >100 topics |
| Concurrent requests | Stateless handlers, thread-safe services |

### Security

| Consideration | Approach |
|---------------|----------|
| Secret detection | Memories already filtered by capture service |
| PII handling | Inherits existing redaction from capture |
| Audit logging | Memory access logged via existing hooks |

### Maintainability

| Requirement | Implementation |
|-------------|----------------|
| Module separation | New `search_intent.rs`, `search_context.rs`, `topic_index.rs` modules |
| Test coverage | >90% for new code |
| Documentation | Inline rustdoc, updated CLAUDE.md |
| Clippy compliance | All pedantic + nursery lints pass |

## Technical Constraints

### Technology Stack Requirements

- **Language**: Rust (Edition 2024, MSRV 1.85)
- **Async runtime**: Tokio
- **JSON handling**: serde_json
- **Regex**: Standard library regex or LazyLock patterns
- **Testing**: cargo test + proptest for property-based tests

### Integration Requirements

- Must integrate with existing `HookHandler` trait
- Must use existing `RecallService` for memory retrieval
- Must extend existing MCP resource/prompt infrastructure
- Must respect existing configuration patterns

### Compatibility Requirements

- Claude Code hook protocol (JSON-RPC format)
- Existing MCP resource URI scheme (`subcog://`)
- Existing filter query syntax (`ns:`, `tag:`, `since:`, etc.)

## Dependencies

### Internal Dependencies

| Component | Dependency Type | Impact |
|-----------|-----------------|--------|
| UserPromptHandler | Enhancement | Add search intent detection |
| RecallService | Enhancement | Add namespace weight support |
| ResourceHandler | Extension | Add new resource types |
| PromptRegistry | Extension | Add new prompts |
| ServiceContainer | Consumption | Access domain-scoped services |

### External Dependencies

| Dependency | Purpose | Risk |
|------------|---------|------|
| LLM Provider | Intent classification | Medium - fallback available |
| Embedding service | Vector search | Low - text search fallback |
| SQLite/FTS5 | Text search | None - required for existing functionality |

## Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| LLM latency spikes | Medium | Medium | 200ms timeout + keyword fallback |
| False positive intent detection | Medium | Low | Confidence threshold (0.5), user can ignore injected context |
| Memory overload in response | Low | Medium | Token budget, truncation, configurable limits |
| Topic index stale | Low | Low | Rebuild on capture, refresh on server start |
| Performance regression | Low | High | Benchmarks in CI, target budgets |

## Open Questions

- [x] Should intent detection run in parallel with keyword detection? **Yes - hybrid approach**
- [x] What confidence threshold for memory injection? **0.5 (configurable)**
- [x] How many topics to pre-index at startup? **All from existing tags + namespaces**
- [ ] Should related topics be shown in `subcog://topics/{topic}` response?

## Appendix

### Glossary

| Term | Definition |
|------|------------|
| Search Intent | User's implied goal when asking a question (HowTo, Troubleshoot, etc.) |
| Namespace Weight | Multiplier applied to specific namespaces based on intent type |
| Topic Index | Pre-built mapping of topics to memory IDs for fast lookup |
| Hybrid Detection | Combining keyword and LLM-based intent classification |
| RRF | Reciprocal Rank Fusion - algorithm for merging ranked lists |

### References

- [Parent Issue #15](https://github.com/zircote/subcog/issues/15)
- [Phase 1: Search Intent Detection (#16)](https://github.com/zircote/subcog/issues/16)
- [Phase 2: Adaptive Memory Injection (#17)](https://github.com/zircote/subcog/issues/17)
- [Phase 3: MCP Resources (#18)](https://github.com/zircote/subcog/issues/18)
- [Phase 4: Enhanced MCP Prompts (#19)](https://github.com/zircote/subcog/issues/19)
- [Phase 5: LLM Intent Classification (#20)](https://github.com/zircote/subcog/issues/20)
- [Phase 6: Hook Guidance & Polish (#21)](https://github.com/zircote/subcog/issues/21)
