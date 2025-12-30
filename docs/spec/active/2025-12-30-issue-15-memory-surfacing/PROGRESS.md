---
document_type: progress
project_id: SPEC-2025-12-30-001
initialized: 2025-12-30T17:30:00Z
last_updated: 2025-12-30T17:30:00Z
---

# Proactive Memory Surfacing - Implementation Progress

## Summary

| Phase | Tasks | Done | Progress |
|-------|-------|------|----------|
| Phase 1: Foundation | 11 | 0 | 0% |
| Phase 2: Adaptive Injection | 12 | 0 | 0% |
| Phase 3: MCP Resources | 16 | 0 | 0% |
| Phase 4: MCP Prompts | 10 | 0 | 0% |
| Phase 5: LLM Classification | 12 | 0 | 0% |
| Phase 6: Polish | 16 | 0 | 0% |
| **Total** | **77** | **0** | **0%** |

## Phase 1: Foundation - Search Intent Detection

**GitHub Issue**: #16
**Status**: pending

| Task | Description | Status | Started | Completed |
|------|-------------|--------|---------|-----------|
| 1.1 | Define SearchIntentType Enum | pending | - | - |
| 1.2 | Define SearchIntent Struct | pending | - | - |
| 1.3 | Implement SEARCH_SIGNALS Constant | pending | - | - |
| 1.4 | Implement detect_search_intent Method | pending | - | - |
| 1.5 | Implement Confidence Calculation | pending | - | - |
| 1.6 | Implement Basic Topic Extraction | pending | - | - |
| 1.7 | Create search_intent.rs Module | pending | - | - |
| 1.8 | Integrate with UserPromptHandler | pending | - | - |
| 1.9 | Unit Tests for Intent Type Detection | pending | - | - |
| 1.10 | Unit Tests for Confidence Calculation | pending | - | - |
| 1.11 | Unit Tests for Topic Extraction | pending | - | - |

### Phase 1 Deliverables

- [ ] `src/hooks/search_intent.rs` with SearchIntent, SearchIntentType, detection logic
- [ ] Modified `src/hooks/user_prompt.rs` with integration
- [ ] Modified `src/hooks/mod.rs` with export
- [ ] >90% test coverage for new code

### Phase 1 Exit Criteria

- [ ] All 6 SearchIntentType variants defined and documented
- [ ] Keyword detection matches all signals in spec
- [ ] Detection completes in <10ms
- [ ] All unit tests pass
- [ ] Clippy passes with no warnings

---

## Phase 2: Adaptive Memory Injection

**GitHub Issue**: #17
**Status**: pending

| Task | Description | Status | Started | Completed |
|------|-------------|--------|---------|-----------|
| 2.1 | Define AdaptiveContextConfig | pending | - | - |
| 2.2 | Implement memories_for_intent Method | pending | - | - |
| 2.3 | Implement namespace_weights_for_intent | pending | - | - |
| 2.4 | Add Namespace Weights to RecallService | pending | - | - |
| 2.5 | Create SearchContextBuilder | pending | - | - |
| 2.6 | Define MemoryContext Struct | pending | - | - |
| 2.7 | Define InjectedMemory Struct | pending | - | - |
| 2.8 | Integrate with UserPromptHandler | pending | - | - |
| 2.9 | Create search_context.rs Module | pending | - | - |
| 2.10 | Integration Tests for Adaptive Count | pending | - | - |
| 2.11 | Integration Tests for Namespace Weighting | pending | - | - |
| 2.12 | Integration Tests for End-to-End Flow | pending | - | - |

### Phase 2 Deliverables

- [ ] `src/hooks/search_context.rs` with SearchContextBuilder, AdaptiveContextConfig
- [ ] Modified `src/services/recall.rs` with namespace weight support
- [ ] Modified `src/hooks/user_prompt.rs` with memory injection
- [ ] All integration tests pass

### Phase 2 Exit Criteria

- [ ] Memory count adapts based on confidence
- [ ] Namespace weights applied correctly per intent type
- [ ] Memory injection adds <50ms to UserPromptSubmit
- [ ] All tests pass

---

## Phase 3: MCP Resources - Query & Topic

**GitHub Issue**: #18
**Status**: pending

| Task | Description | Status | Started | Completed |
|------|-------------|--------|---------|-----------|
| 3.1 | Define TopicInfo Struct | pending | - | - |
| 3.2 | Create TopicIndexService | pending | - | - |
| 3.3 | Implement build_index Method | pending | - | - |
| 3.4 | Implement get_topic_memories Method | pending | - | - |
| 3.5 | Implement list_topics Method | pending | - | - |
| 3.6 | Implement add_memory Method | pending | - | - |
| 3.7 | Update ParsedUrn Enum | pending | - | - |
| 3.8 | Add Resource Templates | pending | - | - |
| 3.9 | Implement handle_search_resource | pending | - | - |
| 3.10 | Implement handle_topics_list_resource | pending | - | - |
| 3.11 | Implement handle_topic_resource | pending | - | - |
| 3.12 | Initialize TopicIndex at Server Startup | pending | - | - |
| 3.13 | Create topic_index.rs Module | pending | - | - |
| 3.14 | Unit Tests for Topic Extraction | pending | - | - |
| 3.15 | Unit Tests for TopicIndexService | pending | - | - |
| 3.16 | Functional Tests for Resources | pending | - | - |

### Phase 3 Deliverables

- [ ] `src/services/topic_index.rs` with TopicIndexService
- [ ] Modified `src/mcp/resources.rs` with new templates and handlers
- [ ] Modified `src/mcp/server.rs` with topic index initialization
- [ ] Modified `src/services/mod.rs` with exports
- [ ] All tests pass with >90% coverage

### Phase 3 Exit Criteria

- [ ] `subcog://search/{query}` returns semantically matched memories
- [ ] `subcog://topics` lists all indexed topics with counts
- [ ] `subcog://topics/{topic}` returns memories for specific topic
- [ ] Topic index builds at startup in <100ms
- [ ] All tests pass

---

## Phase 4: Enhanced MCP Prompts

**GitHub Issue**: #19
**Status**: pending

| Task | Description | Status | Started | Completed |
|------|-------------|--------|---------|-----------|
| 4.1 | Add search_with_context Prompt | pending | - | - |
| 4.2 | Add research_topic Prompt | pending | - | - |
| 4.3 | Enhance recall_context Prompt | pending | - | - |
| 4.4 | Implement generate_search_with_context_content | pending | - | - |
| 4.5 | Implement generate_research_topic_content | pending | - | - |
| 4.6 | Update generate_recall_context_content | pending | - | - |
| 4.7 | Define Depth Options | pending | - | - |
| 4.8 | Unit Tests for Prompt Definitions | pending | - | - |
| 4.9 | Unit Tests for Argument Validation | pending | - | - |
| 4.10 | Functional Tests for Prompt Execution | pending | - | - |

### Phase 4 Deliverables

- [ ] Modified `src/mcp/prompts.rs` with new prompts
- [ ] Modified `src/mcp/server.rs` with content generation
- [ ] All tests pass

### Phase 4 Exit Criteria

- [ ] search_with_context prompt listed and functional
- [ ] research_topic prompt listed and functional
- [ ] recall_context enhanced with intent argument
- [ ] All tests pass

---

## Phase 5: LLM Intent Classification

**GitHub Issue**: #20
**Status**: pending

| Task | Description | Status | Started | Completed |
|------|-------------|--------|---------|-----------|
| 5.1 | Define Intent Classification Prompt | pending | - | - |
| 5.2 | Implement classify_intent_with_llm | pending | - | - |
| 5.3 | Implement LLM Response Parsing | pending | - | - |
| 5.4 | Implement 200ms Timeout | pending | - | - |
| 5.5 | Implement Keyword Fallback | pending | - | - |
| 5.6 | Implement Hybrid Detection | pending | - | - |
| 5.7 | Add use_llm Feature Flag | pending | - | - |
| 5.8 | Update UserPromptHandler for Async | pending | - | - |
| 5.9 | Unit Tests for LLM Prompt | pending | - | - |
| 5.10 | Unit Tests for Response Parsing | pending | - | - |
| 5.11 | Integration Tests for Timeout | pending | - | - |
| 5.12 | Integration Tests for Fallback Chain | pending | - | - |

### Phase 5 Deliverables

- [ ] Modified `src/hooks/search_intent.rs` with LLM classification
- [ ] Modified `src/hooks/user_prompt.rs` with async hybrid detection
- [ ] Modified `src/config/mod.rs` with LLM config options
- [ ] All tests pass

### Phase 5 Exit Criteria

- [ ] LLM classification completes in <200ms or times out
- [ ] Fallback chain works correctly
- [ ] Hybrid detection merges results appropriately
- [ ] Feature can be disabled via configuration
- [ ] All tests pass

---

## Phase 6: Hook Guidance & Polish

**GitHub Issue**: #21
**Status**: pending

| Task | Description | Status | Started | Completed |
|------|-------------|--------|---------|-----------|
| 6.1 | Add memory_context to Hook Response | pending | - | - |
| 6.2 | Add Conditional Reminder Text | pending | - | - |
| 6.3 | Add suggested_resources Array | pending | - | - |
| 6.4 | Define SearchIntentConfig | pending | - | - |
| 6.5 | Add Environment Variable Support | pending | - | - |
| 6.6 | Create Keyword Detection Benchmark | pending | - | - |
| 6.7 | Create LLM Classification Benchmark | pending | - | - |
| 6.8 | Create Memory Retrieval Benchmark | pending | - | - |
| 6.9 | Create Topic Index Benchmark | pending | - | - |
| 6.10 | Add Benchmarks to CI | pending | - | - |
| 6.11 | Verify Graceful Degradation | pending | - | - |
| 6.12 | Update CLAUDE.md | pending | - | - |
| 6.13 | Add Help Content | pending | - | - |
| 6.14 | End-to-End Functional Tests | pending | - | - |
| 6.15 | Performance Regression Tests | pending | - | - |
| 6.16 | Documentation Accuracy Verification | pending | - | - |

### Phase 6 Deliverables

- [ ] `benches/search_intent.rs` with performance benchmarks
- [ ] Modified `src/hooks/user_prompt.rs` with memory_context
- [ ] Modified `src/config/mod.rs` with SearchIntentConfig
- [ ] Updated `CLAUDE.md` with documentation
- [ ] All tests pass

### Phase 6 Exit Criteria

- [ ] Hook response includes memory context when intent detected
- [ ] All configuration options work as documented
- [ ] Keyword detection <10ms in benchmarks
- [ ] Total UserPromptSubmit <200ms in benchmarks
- [ ] All degradation paths tested and working
- [ ] Documentation complete and accurate
- [ ] All tests pass
- [ ] No performance regressions

---

## Divergences from Plan

_None yet_

---

## Notes

- Implementation started: 2025-12-30
- Last activity: 2025-12-30T17:30:00Z
