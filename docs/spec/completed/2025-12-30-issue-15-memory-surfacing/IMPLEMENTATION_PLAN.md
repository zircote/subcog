---
document_type: implementation_plan
project_id: SPEC-2025-12-30-001
version: 1.0.0
last_updated: 2025-12-30T12:00:00Z
status: draft
---

# Proactive Memory Surfacing - Implementation Plan

## Overview

This implementation plan follows the 6-phase structure defined in GitHub Issue #15 and its subissues (#16-#21). Each phase builds on the previous, with clear dependencies and acceptance criteria.

## Team & Resources

| Role | Responsibility | Allocation |
|------|----------------|------------|
| Developer | Implementation, testing | 100% |
| Reviewer | Code review, testing | As needed |

## Phase Summary

| Phase | Title | Key Deliverables | GitHub Issue |
|-------|-------|------------------|--------------|
| Phase 1 | Foundation - Search Intent Detection | SearchIntent struct, keyword patterns, detection logic | #16 |
| Phase 2 | Adaptive Memory Injection | AdaptiveContextConfig, namespace weights, SearchContextBuilder | #17 |
| Phase 3 | MCP Resources - Query & Topic | TopicIndexService, new resource handlers | #18 |
| Phase 4 | Enhanced MCP Prompts | search_with_context, research_topic prompts | #19 |
| Phase 5 | LLM Intent Classification | Async LLM classification, hybrid detection | #20 |
| Phase 6 | Hook Guidance & Polish | Configuration, benchmarks, documentation | #21 |

---

## Phase 1: Foundation - Search Intent Detection

**Goal**: Establish the foundation for search intent detection by implementing core data models and keyword-based detection logic.
**Prerequisites**: None - this is the foundation phase.
**GitHub Issue**: #16

### Tasks

#### Task 1.1: Define SearchIntentType Enum

- **Description**: Create the `SearchIntentType` enum with 6 variants: HowTo, Location, Explanation, Comparison, Troubleshoot, General
- **Dependencies**: None
- **Acceptance Criteria**:
 - [ ] Enum defined with all 6 variants
 - [ ] Each variant has documentation
 - [ ] Implements Debug, Clone, Copy, PartialEq, Eq
 - [ ] `as_str()` method for string conversion

#### Task 1.2: Define SearchIntent Struct

- **Description**: Create the `SearchIntent` struct with keywords, topics, confidence, intent_type, and source fields
- **Dependencies**: Task 1.1
- **Acceptance Criteria**:
 - [ ] Struct defined with all fields
 - [ ] `DetectionSource` enum (Keyword, Llm, Hybrid)
 - [ ] Implements Debug, Clone
 - [ ] Default implementation

#### Task 1.3: Implement SEARCH_SIGNALS Constant

- **Description**: Define the keyword-to-intent-type mapping as a constant array
- **Dependencies**: Task 1.1
- **Acceptance Criteria**:
 - [ ] All signals from spec implemented
 - [ ] Case-insensitive matching considered
 - [ ] Signals grouped by intent type for clarity

#### Task 1.4: Implement detect_search_intent Method

- **Description**: Add `detect_search_intent()` method to detect search intent using keyword patterns
- **Dependencies**: Tasks 1.2, 1.3
- **Acceptance Criteria**:
 - [ ] Iterates through SEARCH_SIGNALS
 - [ ] Returns None if no signals match
 - [ ] Case-insensitive matching
 - [ ] Detects correct intent type based on matched signal

#### Task 1.5: Implement Confidence Calculation

- **Description**: Calculate confidence score based on number of matched signals, prompt length, and structure
- **Dependencies**: Task 1.4
- **Acceptance Criteria**:
 - [ ] Base confidence 0.5 for any match
 - [ ] +0.1 for each additional signal (max +0.15)
 - [ ] +0.1 if prompt > 50 chars
 - [ ] +0.1 if prompt contains multiple sentences
 - [ ] Max confidence 0.95

#### Task 1.6: Implement Basic Topic Extraction

- **Description**: Extract topics from prompt using keyword context and known terms
- **Dependencies**: Task 1.4
- **Acceptance Criteria**:
 - [ ] Extracts nouns after search signal keywords
 - [ ] Filters common stop words
 - [ ] Returns 1-5 topics max
 - [ ] Topics are lowercase, trimmed

#### Task 1.7: Create search_intent.rs Module

- **Description**: Create new module file and export from hooks/mod.rs
- **Dependencies**: Tasks 1.1-1.6
- **Acceptance Criteria**:
 - [ ] File created at src/hooks/search_intent.rs
 - [ ] Module exported in src/hooks/mod.rs
 - [ ] Public types properly exported

#### Task 1.8: Integrate with UserPromptHandler

- **Description**: Add search intent detection to UserPromptHandler::handle() and include in response
- **Dependencies**: Task 1.7
- **Acceptance Criteria**:
 - [ ] Detection called during handle()
 - [ ] SearchIntent included in response metadata
 - [ ] Does not break existing capture signal detection
 - [ ] Detection completes in <10ms

#### Task 1.9: Unit Tests for Intent Type Detection

- **Description**: Test each intent type detection with representative prompts
- **Dependencies**: Task 1.4
- **Acceptance Criteria**:
 - [ ] Test "how do I..." -> HowTo
 - [ ] Test "where is..." -> Location
 - [ ] Test "what is..." -> Explanation
 - [ ] Test "difference between..." -> Comparison
 - [ ] Test "why is...error" -> Troubleshoot
 - [ ] Test generic search -> General

#### Task 1.10: Unit Tests for Confidence Calculation

- **Description**: Test confidence scoring across various scenarios
- **Dependencies**: Task 1.5
- **Acceptance Criteria**:
 - [ ] Test single match -> 0.5
 - [ ] Test multiple matches -> higher confidence
 - [ ] Test long prompts -> length bonus
 - [ ] Test multi-sentence -> sentence bonus
 - [ ] Test max cap at 0.95

#### Task 1.11: Unit Tests for Topic Extraction

- **Description**: Test topic extraction from various prompts
- **Dependencies**: Task 1.6
- **Acceptance Criteria**:
 - [ ] Test "how do I implement authentication?" -> ["authentication"]
 - [ ] Test "where is the database config?" -> ["database", "config"]
 - [ ] Test empty/no topics case

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

**Goal**: Implement adaptive memory injection that adjusts count and namespace weights based on detected search intent.
**Prerequisites**: Phase 1 complete
**GitHub Issue**: #17

### Tasks

#### Task 2.1: Define AdaptiveContextConfig

- **Description**: Create configuration struct for adaptive memory injection
- **Dependencies**: Phase 1
- **Acceptance Criteria**:
 - [ ] `base_count: usize` (default 5)
 - [ ] `max_count: usize` (default 15)
 - [ ] `max_tokens: usize` (default 4000)
 - [ ] Builder pattern methods
 - [ ] Default implementation

#### Task 2.2: Implement memories_for_intent Method

- **Description**: Calculate memory count based on confidence level
- **Dependencies**: Task 2.1
- **Acceptance Criteria**:
 - [ ] confidence >= 0.8 -> max_count (15)
 - [ ] confidence >= 0.5 -> base_count + 5 (10)
 - [ ] confidence < 0.5 -> base_count (5)

#### Task 2.3: Implement namespace_weights_for_intent

- **Description**: Return namespace weight multipliers based on intent type
- **Dependencies**: Phase 1
- **Acceptance Criteria**:
 - [ ] HowTo: Patterns 1.5x, Learnings 1.3x, Decisions 1.0x
 - [ ] Troubleshoot: Blockers 1.5x, Learnings 1.3x, Decisions 1.0x
 - [ ] Location/Explanation: Decisions 1.5x, Context 1.3x, Patterns 1.0x
 - [ ] General: Decisions 1.2x, Patterns 1.2x, Learnings 1.0x

#### Task 2.4: Add Namespace Weights to RecallService

- **Description**: Extend RecallService.search() to accept optional namespace weight overrides
- **Dependencies**: Task 2.3
- **Acceptance Criteria**:
 - [ ] New `with_namespace_weights()` builder method or parameter
 - [ ] Weights applied to scores after retrieval
 - [ ] Default weights are 1.0 (no change)
 - [ ] Existing callers unaffected

#### Task 2.5: Create SearchContextBuilder

- **Description**: Create service to orchestrate intent -> memory injection flow
- **Dependencies**: Tasks 2.2, 2.3, 2.4
- **Acceptance Criteria**:
 - [ ] `new(config, recall_service)` constructor
 - [ ] `build_context(intent) -> Result<MemoryContext>`
 - [ ] Respects token budget
 - [ ] Returns structured MemoryContext

#### Task 2.6: Define MemoryContext Struct

- **Description**: Create struct for memory context in hook response
- **Dependencies**: None
- **Acceptance Criteria**:
 - [ ] `search_intent_detected: bool`
 - [ ] `intent_type: Option<String>`
 - [ ] `topics: Vec<String>`
 - [ ] `injected_memories: Vec<InjectedMemory>`
 - [ ] Serde serialization

#### Task 2.7: Define InjectedMemory Struct

- **Description**: Create struct for individual injected memory in response
- **Dependencies**: None
- **Acceptance Criteria**:
 - [ ] `id: String`
 - [ ] `namespace: String`
 - [ ] `content_preview: String` (truncated)
 - [ ] `score: f32`
 - [ ] Serde serialization

#### Task 2.8: Integrate with UserPromptHandler

- **Description**: Add memory injection to UserPromptHandler when search intent detected
- **Dependencies**: Tasks 2.5, 2.6, 2.7
- **Acceptance Criteria**:
 - [ ] Injects memories when confidence >= min_confidence
 - [ ] MemoryContext in response metadata
 - [ ] Context string includes memory summary
 - [ ] Total latency <150ms

#### Task 2.9: Create search_context.rs Module

- **Description**: Create new module file for SearchContextBuilder and related types
- **Dependencies**: Tasks 2.5-2.7
- **Acceptance Criteria**:
 - [ ] File created at src/hooks/search_context.rs
 - [ ] Module exported in src/hooks/mod.rs
 - [ ] Public types properly exported

#### Task 2.10: Integration Tests for Adaptive Count

- **Description**: Test memory count adapts to confidence levels
- **Dependencies**: Task 2.8
- **Acceptance Criteria**:
 - [ ] Low confidence prompt -> 5 memories
 - [ ] Medium confidence prompt -> 10 memories
 - [ ] High confidence prompt -> 15 memories

#### Task 2.11: Integration Tests for Namespace Weighting

- **Description**: Test that namespace weights affect returned memories
- **Dependencies**: Task 2.8
- **Acceptance Criteria**:
 - [ ] HowTo query -> Patterns ranked higher
 - [ ] Troubleshoot query -> Blockers ranked higher
 - [ ] Weights actually change ordering

#### Task 2.12: Integration Tests for End-to-End Flow

- **Description**: Test complete flow from prompt to injected memories
- **Dependencies**: Task 2.8
- **Acceptance Criteria**:
 - [ ] Search intent detected
 - [ ] Memories retrieved with weights
 - [ ] Response includes memory_context
 - [ ] Content preview is truncated appropriately

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

**Goal**: Implement MCP resource-based memory surfacing with query and topic resources.
**Prerequisites**: Phase 1 (for topic extraction patterns)
**GitHub Issue**: #18

### Tasks

#### Task 3.1: Define TopicInfo Struct

- **Description**: Create struct for topic metadata
- **Dependencies**: None
- **Acceptance Criteria**:
 - [ ] `name: String`
 - [ ] `memory_count: usize`
 - [ ] `namespaces: Vec<Namespace>`
 - [ ] Serde serialization

#### Task 3.2: Create TopicIndexService

- **Description**: Create service for maintaining topic -> memory mappings
- **Dependencies**: Task 3.1
- **Acceptance Criteria**:
 - [ ] `topics: Arc<RwLock<HashMap<String, Vec<MemoryId>>>>`
 - [ ] `last_refresh: Arc<RwLock<Instant>>`
 - [ ] Thread-safe access

#### Task 3.3: Implement build_index Method

- **Description**: Build topic index from existing memories
- **Dependencies**: Task 3.2
- **Acceptance Criteria**:
 - [ ] Extracts topics from namespace names
 - [ ] Extracts topics from memory tags
 - [ ] Extracts keywords from memory content
 - [ ] Builds in <100ms for 1000 memories

#### Task 3.4: Implement get_topic_memories Method

- **Description**: Get memory IDs for a specific topic
- **Dependencies**: Task 3.2
- **Acceptance Criteria**:
 - [ ] Returns Vec<MemoryId>
 - [ ] Case-insensitive lookup
 - [ ] Returns empty vec for unknown topics

#### Task 3.5: Implement list_topics Method

- **Description**: List all indexed topics with metadata
- **Dependencies**: Tasks 3.1, 3.2
- **Acceptance Criteria**:
 - [ ] Returns Vec<TopicInfo>
 - [ ] Sorted by memory_count descending
 - [ ] Includes namespace breakdown

#### Task 3.6: Implement add_memory Method

- **Description**: Update topic index when new memory captured
- **Dependencies**: Task 3.2
- **Acceptance Criteria**:
 - [ ] Extracts topics from new memory
 - [ ] Updates index atomically
 - [ ] Fast (<5ms) for single memory

#### Task 3.7: Update ParsedUrn Enum

- **Description**: Add new URN variants for search and topic resources
- **Dependencies**: None
- **Acceptance Criteria**:
 - [ ] `SearchQuery { query: String }` variant
 - [ ] `TopicList` variant
 - [ ] `TopicDetail { topic: String }` variant
 - [ ] URI parsing implemented

#### Task 3.8: Add Resource Templates

- **Description**: Add resource templates for new resources in resources.rs
- **Dependencies**: Task 3.7
- **Acceptance Criteria**:
 - [ ] `subcog://search/{query}` template
 - [ ] `subcog://topics` template
 - [ ] `subcog://topics/{topic}` template
 - [ ] Templates listed in resources/list

#### Task 3.9: Implement handle_search_resource

- **Description**: Handle `subcog://search/{query}` resource reads
- **Dependencies**: Tasks 3.7, Phase 2 RecallService
- **Acceptance Criteria**:
 - [ ] Parses query from URI
 - [ ] Calls RecallService.search()
 - [ ] Returns JSON with memories, scores
 - [ ] Includes topics extracted from query

#### Task 3.10: Implement handle_topics_list_resource

- **Description**: Handle `subcog://topics` resource reads
- **Dependencies**: Tasks 3.5, 3.7
- **Acceptance Criteria**:
 - [ ] Calls TopicIndexService.list_topics()
 - [ ] Returns JSON with topic list
 - [ ] Includes total count, last indexed timestamp

#### Task 3.11: Implement handle_topic_resource

- **Description**: Handle `subcog://topics/{topic}` resource reads
- **Dependencies**: Tasks 3.4, 3.7
- **Acceptance Criteria**:
 - [ ] Parses topic from URI
 - [ ] Gets memory IDs from index
 - [ ] Fetches full memory details
 - [ ] Returns JSON with memories, related topics

#### Task 3.12: Initialize TopicIndex at Server Startup

- **Description**: Build topic index when MCP server starts
- **Dependencies**: Task 3.3
- **Acceptance Criteria**:
 - [ ] Index built after services initialized
 - [ ] Logs index build time
 - [ ] Handles empty memory case

#### Task 3.13: Create topic_index.rs Module

- **Description**: Create module file for TopicIndexService
- **Dependencies**: Tasks 3.1-3.6
- **Acceptance Criteria**:
 - [ ] File at src/services/topic_index.rs
 - [ ] Exported in src/services/mod.rs

#### Task 3.14: Unit Tests for Topic Extraction

- **Description**: Test topic extraction from memories
- **Dependencies**: Task 3.3
- **Acceptance Criteria**:
 - [ ] Namespace names become topics
 - [ ] Tags become topics
 - [ ] Content keywords extracted

#### Task 3.15: Unit Tests for TopicIndexService

- **Description**: Test all TopicIndexService methods
- **Dependencies**: Tasks 3.3-3.6
- **Acceptance Criteria**:
 - [ ] build_index creates correct mapping
 - [ ] get_topic_memories returns correct IDs
 - [ ] list_topics returns sorted list
 - [ ] add_memory updates index

#### Task 3.16: Functional Tests for Resources

- **Description**: Test MCP resource handlers end-to-end
- **Dependencies**: Tasks 3.9-3.11
- **Acceptance Criteria**:
 - [ ] Search resource returns valid JSON
 - [ ] Topics list resource returns valid JSON
 - [ ] Topic detail resource returns valid JSON
 - [ ] Unknown topic returns empty result

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

**Goal**: Add and enhance MCP prompts for memory-aware search and research.
**Prerequisites**: Phase 3 (for topic-based prompts)
**GitHub Issue**: #19

### Tasks

#### Task 4.1: Add search_with_context Prompt

- **Description**: Add new prompt for search with memory context
- **Dependencies**: Phase 3
- **Acceptance Criteria**:
 - [ ] Name: "search_with_context"
 - [ ] Required arg: query
 - [ ] Optional arg: scope (namespace filter)
 - [ ] Listed in prompts/list

#### Task 4.2: Add research_topic Prompt

- **Description**: Add new prompt for deep topic research
- **Dependencies**: Phase 3
- **Acceptance Criteria**:
 - [ ] Name: "research_topic"
 - [ ] Required arg: topic
 - [ ] Optional arg: depth (quick/standard/comprehensive)
 - [ ] Listed in prompts/list

#### Task 4.3: Enhance recall_context Prompt

- **Description**: Add intent argument to existing recall_context prompt
- **Dependencies**: Phase 1 (for intent types)
- **Acceptance Criteria**:
 - [ ] New optional arg: intent (howto/troubleshoot/explain/locate)
 - [ ] Intent affects content generation
 - [ ] Backward compatible

#### Task 4.4: Implement generate_search_with_context_content

- **Description**: Generate content for search_with_context prompt
- **Dependencies**: Task 4.1
- **Acceptance Criteria**:
 - [ ] Retrieves relevant memories based on query
 - [ ] Formats memories as markdown
 - [ ] Includes search strategy guidance
 - [ ] Suggests related resources

#### Task 4.5: Implement generate_research_topic_content

- **Description**: Generate content for research_topic prompt
- **Dependencies**: Task 4.2
- **Acceptance Criteria**:
 - [ ] Retrieves topic memories from TopicIndexService
 - [ ] Depth affects memory count and detail level
 - [ ] quick: 3 memories, summaries only
 - [ ] standard: 5 memories, with context
 - [ ] comprehensive: 10+ memories, full detail

#### Task 4.6: Update generate_recall_context_content

- **Description**: Update existing prompt to handle intent argument
- **Dependencies**: Task 4.3
- **Acceptance Criteria**:
 - [ ] Intent affects namespace weighting
 - [ ] Guidance text varies by intent type
 - [ ] No intent -> default behavior

#### Task 4.7: Define Depth Options

- **Description**: Define enum or constants for depth parameter
- **Dependencies**: None
- **Acceptance Criteria**:
 - [ ] quick: fast, minimal detail
 - [ ] standard: balanced (default)
 - [ ] comprehensive: thorough, detailed

#### Task 4.8: Unit Tests for Prompt Definitions

- **Description**: Test prompt definitions and arguments
- **Dependencies**: Tasks 4.1-4.3
- **Acceptance Criteria**:
 - [ ] All prompts listed
 - [ ] Arguments validated correctly
 - [ ] Required vs optional enforced

#### Task 4.9: Unit Tests for Argument Validation

- **Description**: Test argument validation for new prompts
- **Dependencies**: Tasks 4.1-4.3
- **Acceptance Criteria**:
 - [ ] Missing required arg -> error
 - [ ] Invalid depth value -> error or default
 - [ ] Invalid scope value -> error

#### Task 4.10: Functional Tests for Prompt Execution

- **Description**: Test prompt content generation end-to-end
- **Dependencies**: Tasks 4.4-4.6
- **Acceptance Criteria**:
 - [ ] search_with_context returns memories
 - [ ] research_topic respects depth
 - [ ] recall_context respects intent

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

**Goal**: Add LLM-based intent classification for higher accuracy, with timeout and fallback.
**Prerequisites**: Phase 1 (keyword detection as fallback)
**GitHub Issue**: #20

### Tasks

#### Task 5.1: Define Intent Classification Prompt

- **Description**: Create prompt template for LLM intent classification
- **Dependencies**: Phase 1 (SearchIntentType)
- **Acceptance Criteria**:
 - [ ] Prompt asks for intent type classification
 - [ ] Prompt asks for topic extraction
 - [ ] Output format is structured JSON
 - [ ] Prompt is concise for fast response

#### Task 5.2: Implement classify_intent_with_llm

- **Description**: Async method to call LLM for intent classification
- **Dependencies**: Task 5.1
- **Acceptance Criteria**:
 - [ ] Calls LLM provider with prompt
 - [ ] Parses JSON response
 - [ ] Returns SearchIntent with source=Llm
 - [ ] Handles parse errors gracefully

#### Task 5.3: Implement LLM Response Parsing

- **Description**: Parse LLM JSON response to SearchIntent
- **Dependencies**: Task 5.2
- **Acceptance Criteria**:
 - [ ] Extracts intent_type
 - [ ] Extracts topics array
 - [ ] Sets confidence based on LLM's indication
 - [ ] Falls back on malformed response

#### Task 5.4: Implement 200ms Timeout

- **Description**: Add timeout for LLM classification
- **Dependencies**: Task 5.2
- **Acceptance Criteria**:
 - [ ] tokio::time::timeout with 200ms
 - [ ] Returns None on timeout
 - [ ] Logs timeout event
 - [ ] Configurable via config

#### Task 5.5: Implement Keyword Fallback

- **Description**: Fall back to keyword detection when LLM fails or times out
- **Dependencies**: Tasks 5.4, Phase 1
- **Acceptance Criteria**:
 - [ ] Timeout -> keyword fallback
 - [ ] Error -> keyword fallback
 - [ ] Logs fallback reason
 - [ ] Sets source=Keyword on fallback

#### Task 5.6: Implement Hybrid Detection

- **Description**: Run keyword and LLM in parallel, merge results
- **Dependencies**: Tasks 5.2, Phase 1
- **Acceptance Criteria**:
 - [ ] Parallel execution via tokio::join!
 - [ ] LLM topics merged with keyword matches
 - [ ] Confidence from keyword, topics from LLM
 - [ ] Sets source=Hybrid when both succeed

#### Task 5.7: Add use_llm Feature Flag

- **Description**: Configuration option to enable/disable LLM classification
- **Dependencies**: None
- **Acceptance Criteria**:
 - [ ] Config field: use_llm (default: true when LLM available)
 - [ ] Environment variable override
 - [ ] Skips LLM if disabled

#### Task 5.8: Update UserPromptHandler for Async

- **Description**: Make detection async to support LLM calls
- **Dependencies**: Tasks 5.6
- **Acceptance Criteria**:
 - [ ] handle() method is async
 - [ ] Calls hybrid detection when LLM enabled
 - [ ] Calls keyword-only when LLM disabled
 - [ ] Maintains <200ms total latency

#### Task 5.9: Unit Tests for LLM Prompt

- **Description**: Test prompt generation
- **Dependencies**: Task 5.1
- **Acceptance Criteria**:
 - [ ] Prompt includes user query
 - [ ] Prompt specifies output format
 - [ ] Prompt is under token limit

#### Task 5.10: Unit Tests for Response Parsing

- **Description**: Test LLM response parsing
- **Dependencies**: Task 5.3
- **Acceptance Criteria**:
 - [ ] Valid JSON parsed correctly
 - [ ] Invalid JSON handled gracefully
 - [ ] Missing fields handled

#### Task 5.11: Integration Tests for Timeout

- **Description**: Test timeout behavior with mock LLM
- **Dependencies**: Task 5.4
- **Acceptance Criteria**:
 - [ ] Slow LLM triggers timeout
 - [ ] Fallback to keyword occurs
 - [ ] Timeout logged

#### Task 5.12: Integration Tests for Fallback Chain

- **Description**: Test complete fallback chain
- **Dependencies**: Task 5.5
- **Acceptance Criteria**:
 - [ ] LLM success -> LLM result used
 - [ ] LLM timeout -> keyword fallback
 - [ ] LLM error -> keyword fallback
 - [ ] LLM disabled -> keyword only

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

**Goal**: Complete the feature with hook guidance, configuration, benchmarks, and documentation.
**Prerequisites**: All previous phases
**GitHub Issue**: #21

### Tasks

#### Task 6.1: Add memory_context to Hook Response

- **Description**: Ensure MemoryContext included in UserPromptSubmit response
- **Dependencies**: Phase 2
- **Acceptance Criteria**:
 - [ ] `memory_context` field in metadata
 - [ ] Contains search_intent_detected, intent_type, topics
 - [ ] Contains injected_memories array
 - [ ] Contains suggested_resources

#### Task 6.2: Add Conditional Reminder Text

- **Description**: Include reminder when confidence >= 0.5
- **Dependencies**: Task 6.1
- **Acceptance Criteria**:
 - [ ] Reminder in memory_context when threshold met
 - [ ] Reminder text guides memory usage
 - [ ] No reminder below threshold

#### Task 6.3: Add suggested_resources Array

- **Description**: Include suggested resource URIs in response
- **Dependencies**: Phase 3
- **Acceptance Criteria**:
 - [ ] Includes subcog://search/{topic} for detected topics
 - [ ] Includes subcog://topics
 - [ ] 2-4 resources suggested max

#### Task 6.4: Define SearchIntentConfig

- **Description**: Create comprehensive configuration struct
- **Dependencies**: All phases
- **Acceptance Criteria**:
 - [ ] enabled: bool (default true)
 - [ ] use_llm: bool (default true when available)
 - [ ] llm_timeout_ms: u64 (default 200)
 - [ ] min_confidence: f32 (default 0.5)
 - [ ] context: AdaptiveContextConfig

#### Task 6.5: Add Environment Variable Support

- **Description**: Environment variable overrides for config
- **Dependencies**: Task 6.4
- **Acceptance Criteria**:
 - [ ] SUBCOG_SEARCH_INTENT_ENABLED
 - [ ] SUBCOG_SEARCH_INTENT_USE_LLM
 - [ ] SUBCOG_SEARCH_INTENT_LLM_TIMEOUT_MS
 - [ ] SUBCOG_SEARCH_INTENT_MIN_CONFIDENCE

#### Task 6.6: Create Keyword Detection Benchmark

- **Description**: Benchmark for keyword detection latency
- **Dependencies**: Phase 1
- **Acceptance Criteria**:
 - [ ] Benchmark in benches/search_intent.rs
 - [ ] Tests various prompt sizes
 - [ ] Target: <10ms

#### Task 6.7: Create LLM Classification Benchmark

- **Description**: Benchmark for LLM classification latency
- **Dependencies**: Phase 5
- **Acceptance Criteria**:
 - [ ] Mock LLM for consistent timing
 - [ ] Tests timeout behavior
 - [ ] Target: <200ms

#### Task 6.8: Create Memory Retrieval Benchmark

- **Description**: Benchmark for memory retrieval with weights
- **Dependencies**: Phase 2
- **Acceptance Criteria**:
 - [ ] Tests various memory counts
 - [ ] Tests with/without weights
 - [ ] Target: <50ms

#### Task 6.9: Create Topic Index Benchmark

- **Description**: Benchmark for topic index building
- **Dependencies**: Phase 3
- **Acceptance Criteria**:
 - [ ] Tests 100, 1000, 10000 memories
 - [ ] Target: <100ms for 1000

#### Task 6.10: Add Benchmarks to CI

- **Description**: Run benchmarks in CI, fail on regression
- **Dependencies**: Tasks 6.6-6.9
- **Acceptance Criteria**:
 - [ ] cargo bench runs in CI
 - [ ] Results compared to baseline
 - [ ] >10% regression fails build

#### Task 6.11: Verify Graceful Degradation

- **Description**: Test all fallback paths work correctly
- **Dependencies**: All phases
- **Acceptance Criteria**:
 - [ ] LLM unavailable -> keyword only
 - [ ] Embeddings down -> text search
 - [ ] Index down -> skip injection
 - [ ] Warning logs for each case

#### Task 6.12: Update CLAUDE.md

- **Description**: Document new features in project CLAUDE.md
- **Dependencies**: All phases
- **Acceptance Criteria**:
 - [ ] Search intent detection documented
 - [ ] New MCP resources documented
 - [ ] New prompts documented
 - [ ] Configuration options documented

#### Task 6.13: Add Help Content

- **Description**: Add help content for new features
- **Dependencies**: All phases
- **Acceptance Criteria**:
 - [ ] Search intent explained in help
 - [ ] Examples provided
 - [ ] Resource URIs documented

#### Task 6.14: End-to-End Functional Tests

- **Description**: Complete integration tests for all scenarios
- **Dependencies**: All phases
- **Acceptance Criteria**:
 - [ ] "how do I..." -> HowTo intent, patterns weighted
 - [ ] "why is...error" -> Troubleshoot, blockers weighted
 - [ ] MCP resource queries work
 - [ ] Prompts generate correct content

#### Task 6.15: Performance Regression Tests

- **Description**: Ensure no performance regressions
- **Dependencies**: Tasks 6.6-6.9
- **Acceptance Criteria**:
 - [ ] All latency targets met
 - [ ] No regression from baseline
 - [ ] 1100+ existing tests still pass

#### Task 6.16: Documentation Accuracy Verification

- **Description**: Verify all documentation matches implementation
- **Dependencies**: Tasks 6.12, 6.13
- **Acceptance Criteria**:
 - [ ] Examples in docs work
 - [ ] Config options accurate
 - [ ] Resource URIs valid

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

## Dependency Graph

```
Phase 1: Foundation ──────────────────────────────────────────────────────┐
 │ │
 ├─────────────────┬─────────────────┬─────────────────┐ │
 ▼ │ │ │ │
Phase 2: Injection │ │ │ │
 │ ▼ │ │ │
 │ Phase 3: Resources │ │ │
 │ │ ▼ │ │
 │ │ Phase 5: LLM │ │
 │ │ │ │ │
 │ ▼ │ │ │
 │ Phase 4: Prompts │ │ │
 │ │ │ │ │
 └─────────────────┴─────────────────┴─────────────────┘ │
 │ │
 ▼ │
 Phase 6: Polish ◄─────────────────────────────────┘
```

**Critical Path**: Phase 1 -> Phase 2 -> Phase 6 (for core functionality)
**Parallel Opportunities**: Phases 3, 4, 5 can proceed in parallel after Phase 1

## Risk Mitigation Tasks

| Risk | Mitigation Task | Phase |
|------|-----------------|-------|
| LLM latency spikes | Implement 200ms timeout with keyword fallback | Phase 5 |
| False positive detection | Configurable confidence threshold, testing | Phase 1, 6 |
| Memory overload | Token budget in AdaptiveContextConfig | Phase 2 |
| Performance regression | Benchmarks in CI | Phase 6 |

## Testing Checklist

- [ ] Unit tests for SearchIntentDetector (Phase 1)
- [ ] Unit tests for SearchContextBuilder (Phase 2)
- [ ] Unit tests for TopicIndexService (Phase 3)
- [ ] Unit tests for new prompts (Phase 4)
- [ ] Unit tests for LLM classification (Phase 5)
- [ ] Integration tests for memory injection (Phase 2)
- [ ] Integration tests for MCP resources (Phase 3)
- [ ] Integration tests for LLM timeout/fallback (Phase 5)
- [ ] End-to-end tests for complete flow (Phase 6)
- [ ] Performance benchmarks (Phase 6)
- [ ] Graceful degradation tests (Phase 6)

## Documentation Tasks

- [ ] Update CLAUDE.md with new features
- [ ] Add help content for search intent
- [ ] Document new MCP resources
- [ ] Document new prompts
- [ ] Add configuration examples

## Launch Checklist

- [ ] All tests passing
- [ ] Documentation complete
- [ ] Benchmarks meet targets
- [ ] CI pipeline updated
- [ ] Graceful degradation verified
- [ ] No clippy warnings
- [ ] No performance regressions

## Post-Launch

- [ ] Monitor for issues (24-48 hours)
- [ ] Gather feedback on detection accuracy
- [ ] Track LLM timeout/fallback rates
- [ ] Update documentation with learnings
- [ ] Consider Phase 7 enhancements
