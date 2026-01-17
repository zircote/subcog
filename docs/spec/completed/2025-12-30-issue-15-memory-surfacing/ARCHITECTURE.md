---
document_type: architecture
project_id: SPEC-2025-12-30-001
version: 1.0.0
last_updated: 2025-12-30T12:00:00Z
status: draft
---

# Proactive Memory Surfacing - Technical Architecture

## System Overview

This document describes the technical architecture for proactive memory surfacing in subcog. The system enhances the existing UserPromptSubmit hook to detect search intent in user prompts and automatically inject relevant memories, while also exposing new MCP resources for topic-based and query-based memory access.

### Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Claude Code                                     │
│  ┌─────────────────┐                                                        │
│  │   User Prompt   │                                                        │
│  └────────┬────────┘                                                        │
│           │                                                                  │
│           ▼                                                                  │
│  ┌─────────────────┐     ┌──────────────────────────────────────────────┐   │
│  │  UserPromptSubmit│────▶│              Hook Handler                    │   │
│  │      Hook        │     │  ┌────────────────────────────────────────┐ │   │
│  └─────────────────┘     │  │        Search Intent Detection         │ │   │
│                          │  │  ┌────────────┐  ┌─────────────────┐   │ │   │
│                          │  │  │  Keyword   │  │  LLM Classifier │   │ │   │
│                          │  │  │  Detector  │  │  (with timeout) │   │ │   │
│                          │  │  └─────┬──────┘  └────────┬────────┘   │ │   │
│                          │  │        │                   │           │ │   │
│                          │  │        └─────────┬─────────┘           │ │   │
│                          │  │                  ▼                     │ │   │
│                          │  │         ┌───────────────┐              │ │   │
│                          │  │         │ SearchIntent  │              │ │   │
│                          │  │         │ (type,topics, │              │ │   │
│                          │  │         │  confidence)  │              │ │   │
│                          │  │         └───────┬───────┘              │ │   │
│                          │  └─────────────────┼──────────────────────┘ │   │
│                          │                    ▼                        │   │
│                          │  ┌────────────────────────────────────────┐ │   │
│                          │  │      Adaptive Memory Injection         │ │   │
│                          │  │  ┌─────────────┐  ┌─────────────────┐  │ │   │
│                          │  │  │  Namespace  │  │ SearchContext   │  │ │   │
│                          │  │  │  Weighting  │  │    Builder      │  │ │   │
│                          │  │  └──────┬──────┘  └────────┬────────┘  │ │   │
│                          │  │         │                   │          │ │   │
│                          │  │         └─────────┬─────────┘          │ │   │
│                          │  │                   ▼                    │ │   │
│                          │  │          ┌──────────────┐              │ │   │
│                          │  │          │RecallService │              │ │   │
│                          │  │          │(with weights)│              │ │   │
│                          │  │          └──────┬───────┘              │ │   │
│                          │  └─────────────────┼──────────────────────┘ │   │
│                          │                    ▼                        │   │
│                          │  ┌────────────────────────────────────────┐ │   │
│                          │  │           Hook Response                 │ │   │
│                          │  │  { continue: true,                     │ │   │
│                          │  │    context: "...",                     │ │   │
│                          │  │    metadata: { memory_context: {...} } │ │   │
│                          │  │  }                                     │ │   │
│                          │  └────────────────────────────────────────┘ │   │
│                          └──────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│                              MCP Server                                      │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                        Resource Handler                               │   │
│  │  ┌──────────────────┐  ┌──────────────────┐  ┌────────────────────┐  │   │
│  │  │ subcog://search/ │  │ subcog://topics  │  │ subcog://topics/   │  │   │
│  │  │     {query}      │  │     (list)       │  │     {topic}        │  │   │
│  │  └────────┬─────────┘  └────────┬─────────┘  └─────────┬──────────┘  │   │
│  │           │                      │                      │            │   │
│  │           ▼                      ▼                      ▼            │   │
│  │  ┌─────────────────────────────────────────────────────────────────┐ │   │
│  │  │                    TopicIndexService                            │ │   │
│  │  │  ┌────────────────┐  ┌────────────────────────────────────────┐ │ │   │
│  │  │  │  Topic Index   │  │ Methods:                               │ │ │   │
│  │  │  │ HashMap<Topic, │  │  - build_index()                       │ │ │   │
│  │  │  │  Vec<MemoryId>>│  │  - get_topic_memories(topic)           │ │ │   │
│  │  │  └────────────────┘  │  - list_topics()                       │ │ │   │
│  │  │                      └────────────────────────────────────────┘ │ │   │
│  │  └─────────────────────────────────────────────────────────────────┘ │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                        Prompt Registry                                │   │
│  │  ┌────────────────────┐  ┌────────────────────┐  ┌────────────────┐  │   │
│  │  │ search_with_context│  │   research_topic   │  │ recall_context │  │   │
│  │  │  (query, scope)    │  │  (topic, depth)    │  │(topic, intent) │  │   │
│  │  └────────────────────┘  └────────────────────┘  └────────────────┘  │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Design Decisions

1. **Hybrid Detection**: Combines fast keyword matching with optional LLM classification for best accuracy/latency tradeoff
2. **Namespace Weighting**: Intent-specific namespace boosts improve recall relevance without changing the underlying ranking algorithm
3. **Topic Pre-indexing**: Topics are indexed at server startup for fast lookup, refreshed on memory capture
4. **Graceful Degradation**: Each component has a fallback path (LLM → keyword, vector → text, injection → skip)

## Component Design

### Component 1: SearchIntentDetector

**Purpose**: Detect search intent in user prompts using keyword patterns and optional LLM classification

**Responsibilities**:
- Parse user prompts for search signal keywords
- Classify intent type (HowTo, Location, Explanation, Comparison, Troubleshoot, General)
- Extract topics from prompt content
- Calculate confidence score
- Optionally invoke LLM for higher-accuracy classification

**Interfaces**:
```rust
pub struct SearchIntent {
    pub keywords: Vec<String>,
    pub topics: Vec<String>,
    pub confidence: f32,
    pub intent_type: SearchIntentType,
    pub source: DetectionSource,  // Keyword, Llm, Hybrid
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchIntentType {
    HowTo,          // "how do I...", "how to..."
    Location,       // "where is...", "find..."
    Explanation,    // "what is...", "explain..."
    Comparison,     // "difference between...", "vs"
    Troubleshoot,   // "why is...", "error...", "fix..."
    General,        // Other search-like queries
}

impl SearchIntentDetector {
    pub fn detect(&self, prompt: &str) -> Option<SearchIntent>;
    pub async fn detect_with_llm(&self, prompt: &str) -> Option<SearchIntent>;
}
```

**Dependencies**: LLM provider (optional), regex patterns

**Technology**: Rust standard library regex, LazyLock for pattern compilation

**Location**: `src/hooks/search_intent.rs`

### Component 2: SearchContextBuilder

**Purpose**: Orchestrate memory injection based on detected search intent

**Responsibilities**:
- Calculate memory count based on confidence
- Determine namespace weights based on intent type
- Invoke RecallService with weighted search
- Format memories for hook response
- Respect token budget

**Interfaces**:
```rust
pub struct AdaptiveContextConfig {
    pub base_count: usize,      // 5
    pub max_count: usize,       // 15
    pub max_tokens: usize,      // 4000
    pub confidence_thresholds: ConfidenceThresholds,
}

pub struct ConfidenceThresholds {
    pub low: f32,     // 0.3
    pub medium: f32,  // 0.5
    pub high: f32,    // 0.8
}

impl SearchContextBuilder {
    pub fn new(config: AdaptiveContextConfig, recall: RecallService) -> Self;
    pub fn memories_for_intent(&self, intent: &SearchIntent) -> usize;
    pub fn namespace_weights_for_intent(&self, intent_type: SearchIntentType) -> HashMap<Namespace, f32>;
    pub async fn build_context(&self, intent: &SearchIntent) -> Result<MemoryContext>;
}
```

**Dependencies**: RecallService, SearchIntent

**Location**: `src/hooks/search_context.rs`

### Component 3: TopicIndexService

**Purpose**: Maintain pre-indexed topic-to-memory mappings for fast topic-based lookup

**Responsibilities**:
- Extract topics from existing memories at startup
- Map topics to memory IDs
- Refresh index on new memory capture
- Provide topic listing and lookup

**Interfaces**:
```rust
pub struct TopicInfo {
    pub name: String,
    pub memory_count: usize,
    pub namespaces: Vec<Namespace>,
}

pub struct TopicIndexService {
    topics: Arc<RwLock<HashMap<String, Vec<MemoryId>>>>,
    last_refresh: Arc<RwLock<Instant>>,
}

impl TopicIndexService {
    pub fn new() -> Self;
    pub fn build_index(&self, memories: &[Memory]) -> Result<()>;
    pub fn add_memory(&self, memory: &Memory) -> Result<()>;
    pub fn get_topic_memories(&self, topic: &str) -> Vec<MemoryId>;
    pub fn list_topics(&self) -> Vec<TopicInfo>;
    pub fn search_topics(&self, query: &str) -> Vec<TopicInfo>;
}
```

**Dependencies**: Memory storage, namespace definitions

**Location**: `src/services/topic_index.rs`

### Component 4: Enhanced ResourceHandler

**Purpose**: Handle new MCP resource types for query and topic-based memory access

**Responsibilities**:
- Parse resource URIs (`subcog://search/{query}`, `subcog://topics`, `subcog://topics/{topic}`)
- Invoke appropriate services
- Format JSON responses

**Interfaces**:
```rust
// New URN patterns in ParsedUrn enum
pub enum ParsedUrn {
    // ... existing variants ...
    SearchQuery { query: String },
    TopicList,
    TopicDetail { topic: String },
}

impl ResourceHandler {
    pub fn handle_search_resource(&self, query: &str) -> Result<String>;
    pub fn handle_topics_list_resource(&self) -> Result<String>;
    pub fn handle_topic_resource(&self, topic: &str) -> Result<String>;
}
```

**Dependencies**: RecallService, TopicIndexService

**Location**: `src/mcp/resources.rs` (modifications)

### Component 5: Enhanced PromptRegistry

**Purpose**: Provide new MCP prompts for memory-aware search and research

**Responsibilities**:
- Define new prompt templates
- Generate prompt content with contextual memories
- Handle prompt arguments

**Interfaces**:
```rust
// New prompts added to registry
pub fn search_with_context_prompt() -> PromptInfo;
pub fn research_topic_prompt() -> PromptInfo;

// Enhanced existing prompt
pub fn recall_context_prompt() -> PromptInfo;  // Now with intent arg
```

**Dependencies**: RecallService, TopicIndexService

**Location**: `src/mcp/prompts.rs` (modifications)

## Data Design

### Data Models

```rust
// Search Intent Detection
pub struct SearchIntent {
    pub keywords: Vec<String>,
    pub topics: Vec<String>,
    pub confidence: f32,
    pub intent_type: SearchIntentType,
    pub source: DetectionSource,
}

pub enum SearchIntentType {
    HowTo,
    Location,
    Explanation,
    Comparison,
    Troubleshoot,
    General,
}

pub enum DetectionSource {
    Keyword,
    Llm,
    Hybrid,
}

// Memory Context for Hook Response
pub struct MemoryContext {
    pub search_intent_detected: bool,
    pub intent_type: Option<String>,
    pub topics: Vec<String>,
    pub injected_memories: Vec<InjectedMemory>,
    pub reminder: Option<String>,
    pub suggested_resources: Vec<String>,
}

pub struct InjectedMemory {
    pub id: String,
    pub namespace: String,
    pub content_preview: String,  // Truncated to ~200 chars
    pub score: f32,
}

// Topic Index
pub struct TopicInfo {
    pub name: String,
    pub memory_count: usize,
    pub namespaces: Vec<Namespace>,
}

// Configuration
pub struct SearchIntentConfig {
    pub enabled: bool,
    pub use_llm: bool,
    pub llm_timeout_ms: u64,
    pub min_confidence: f32,
    pub context: AdaptiveContextConfig,
}

pub struct AdaptiveContextConfig {
    pub base_count: usize,
    pub max_count: usize,
    pub max_tokens: usize,
}
```

### Data Flow

```
User Prompt
    │
    ▼
┌───────────────────────┐
│  UserPromptHandler    │
│  (existing)           │
│  - detect_signals()   │  ──▶  CaptureSignals (existing)
│  + detect_search()    │  ──▶  SearchIntent (new)
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│  SearchContextBuilder │
│  - memories_for_intent│
│  - namespace_weights  │
│  - build_context()    │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│  RecallService        │
│  (with weight override)│
│  - search(query,      │
│    mode, filter,      │
│    weights, limit)    │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│  MemoryContext        │
│  - injected_memories  │
│  - suggested_resources│
│  - reminder text      │
└───────────┬───────────┘
            │
            ▼
   Hook Response JSON
```

### Storage Strategy

- **Primary Store**: Existing SQLite + FTS5 index (no changes)
- **Topic Index**: In-memory HashMap with RwLock (rebuilt on startup, updated on capture)
- **Configuration**: TOML file at standard config locations (existing pattern)

## API Design

### MCP Resources

#### `subcog://search/{query}`

**Purpose**: Semantic search for memories matching a query

**Request**: Resource read with URI `subcog://search/authentication`

**Response**:
```json
{
  "query": "authentication",
  "mode": "hybrid",
  "memories": [
    {
      "id": "mem_abc123",
      "urn": "subcog://memory/mem_abc123",
      "namespace": "decisions",
      "content": "Use JWT tokens for API authentication...",
      "score": 0.85,
      "created_at": "2025-12-28T10:00:00Z",
      "tags": ["auth", "jwt", "security"]
    }
  ],
  "topics": ["auth", "security"],
  "total_count": 3,
  "execution_time_ms": 45
}
```

#### `subcog://topics`

**Purpose**: List all pre-indexed topics

**Request**: Resource read with URI `subcog://topics`

**Response**:
```json
{
  "topics": [
    {
      "name": "authentication",
      "memory_count": 5,
      "namespaces": ["decisions", "patterns"]
    },
    {
      "name": "database",
      "memory_count": 8,
      "namespaces": ["decisions", "learnings", "tech-debt"]
    }
  ],
  "total_topics": 15,
  "last_indexed": "2025-12-30T10:00:00Z"
}
```

#### `subcog://topics/{topic}`

**Purpose**: Get memories for a specific topic

**Request**: Resource read with URI `subcog://topics/authentication`

**Response**:
```json
{
  "topic": "authentication",
  "memories": [
    {
      "id": "mem_abc123",
      "urn": "subcog://memory/mem_abc123",
      "namespace": "decisions",
      "content": "Use JWT tokens for API authentication...",
      "created_at": "2025-12-28T10:00:00Z"
    }
  ],
  "related_topics": ["security", "jwt", "oauth"],
  "total_count": 5
}
```

### MCP Prompts

#### `search_with_context`

**Arguments**:
- `query` (required): What you are searching for
- `scope` (optional): Limit to specific namespace

**Generated Content**:
```markdown
# Search with Memory Context

You're searching for: {query}

## Relevant Memories

{memories formatted as markdown list}

## Search Strategy

Based on your query, consider:
1. The memories above may contain prior decisions or patterns
2. Use `subcog://search/{query}` for additional context
3. Check related topics: {suggested_topics}

## Proceed with Search

Now search the codebase, keeping these memories in mind...
```

#### `research_topic`

**Arguments**:
- `topic` (required): Topic to research
- `depth` (optional): quick | standard | comprehensive

**Generated Content**:
```markdown
# Research Topic: {topic}

## Existing Knowledge

{memories formatted based on depth}

## Research Approach ({depth})

{depth-specific guidance}

## Available Resources

- subcog://topics/{topic}
- subcog://search/{topic}
```

### Hook Response Enhancement

**UserPromptSubmit Response** (when search intent detected):
```json
{
  "continue": true,
  "context": "Consider these related memories:\n- ...",
  "metadata": {
    "signals": [...],  // existing capture signals
    "memory_context": {
      "search_intent_detected": true,
      "intent_type": "howto",
      "topics": ["authentication", "jwt"],
      "injected_memories": [
        {
          "id": "mem_abc123",
          "namespace": "patterns",
          "content_preview": "For authentication, use JWT tokens with...",
          "score": 0.85
        }
      ],
      "reminder": "These memories may be relevant to the user's question.",
      "suggested_resources": [
        "subcog://search/authentication",
        "subcog://topics/security"
      ]
    }
  }
}
```

## Integration Points

### Internal Integrations

| System | Integration Type | Purpose |
|--------|-----------------|---------|
| UserPromptHandler | Extension | Add search intent detection |
| RecallService | Enhancement | Add namespace weight override |
| ResourceHandler | Extension | Handle new resource URIs |
| PromptRegistry | Extension | Add new prompts |
| ServiceContainer | Consumption | Access services |
| CaptureService | Event subscription | Refresh topic index on capture |

### External Integrations

| Service | Integration Type | Purpose |
|---------|-----------------|---------|
| LLM Provider | API call | Intent classification |
| Claude Code | Hook protocol | Deliver context |
| MCP Client | Protocol | Resource/prompt access |

## Security Design

### Authentication

- No new authentication required
- LLM calls use existing provider configuration
- Hook calls are internal to Claude Code process

### Authorization

- All memory access respects existing domain scoping
- No cross-project memory leakage
- Topic index is project-scoped

### Data Protection

- Memories already filtered for secrets at capture time
- No additional PII handling required
- Audit logging inherits from existing hooks

### Security Considerations

| Threat | Mitigation |
|--------|------------|
| Prompt injection via search intent | LLM classification has timeout, falls back to keyword |
| Memory content in error messages | Use content preview truncation, no full content in logs |
| Topic index enumeration | Topics derived from existing memories, no new exposure |

## Performance Considerations

### Expected Load

- ~100 UserPromptSubmit calls per hour (typical session)
- ~10-50 memories in index per project
- ~5-20 topics per project

### Performance Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| Keyword detection | <10ms | Near-instant, no perceived delay |
| LLM classification | <200ms | Acceptable for async operation |
| Memory retrieval | <50ms | Existing RecallService target |
| Topic index lookup | <5ms | In-memory HashMap |
| Total UserPromptSubmit | <200ms | Responsive UX |

### Optimization Strategies

1. **Lazy LLM invocation**: Only call LLM if keyword detection confidence < 0.8
2. **Parallel execution**: Run keyword and LLM detection in parallel
3. **Token budget**: Cap injected memory content to prevent response bloat
4. **Connection pooling**: Reuse LLM connections (existing)
5. **Index caching**: Topic index persists across requests

## Reliability & Operations

### Availability Target

- Hook handlers: 99.9% (matches Claude Code)
- MCP resources: 99.9% (matches MCP server)

### Failure Modes

| Failure | Impact | Recovery |
|---------|--------|----------|
| LLM timeout | Medium - reduced accuracy | Fall back to keyword detection |
| LLM error | Medium - reduced accuracy | Fall back to keyword detection |
| Index unavailable | Low - no memory injection | Skip injection, log warning |
| RecallService error | Medium - no memory injection | Skip injection, return error in metadata |
| Topic index stale | Low - outdated topics | Rebuild on next server start |

### Graceful Degradation Chain

```
Full Functionality
       │
       ▼ (LLM timeout/error)
Keyword Detection Only
       │
       ▼ (embedding error)
Text Search Only
       │
       ▼ (index error)
Skip Memory Injection
       │
       ▼ (handler error)
Pass-through (continue: true)
```

### Monitoring & Alerting

| Metric | Threshold | Action |
|--------|-----------|--------|
| LLM timeout rate | >10% | Log warning, review LLM config |
| Detection latency p95 | >200ms | Log warning, check LLM health |
| Memory injection count | 0 for 1 hour | Check index health |
| Topic index size | 0 | Rebuild index |

### Backup & Recovery

- Topic index: Rebuilt from memories on server start
- Configuration: Standard file backup
- Memories: Protected by git notes (existing)

## Testing Strategy

### Unit Testing

| Component | Test Focus | Coverage Target |
|-----------|-----------|-----------------|
| SearchIntentDetector | Keyword patterns, confidence calculation | >95% |
| SearchContextBuilder | Memory count, namespace weights | >95% |
| TopicIndexService | Index building, lookup, refresh | >95% |
| New prompts | Argument validation, content generation | >90% |

### Integration Testing

| Flow | Test Cases |
|------|-----------|
| UserPromptSubmit → RecallService | Intent detected, memories injected |
| Resource read → TopicIndexService | Topics listed, topic details returned |
| Prompt execution | Content generated with memories |
| LLM timeout → fallback | Graceful degradation verified |

### End-to-End Testing

| Scenario | Verification |
|----------|-------------|
| "how do I implement auth?" | Correct intent type, relevant memories |
| "why is the test failing?" | Troubleshoot intent, blocker memories |
| MCP search resource | JSON response with scored memories |
| MCP topic browse | Complete topic list with counts |

### Performance Testing

| Benchmark | Target |
|-----------|--------|
| keyword_detection | <10ms |
| llm_classification | <200ms |
| memory_retrieval | <50ms |
| topic_index_build | <100ms for 1000 memories |

## Deployment Considerations

### Environment Requirements

- Rust 1.85+ (existing)
- SQLite 3.x (existing)
- LLM provider API access (optional)

### Configuration Management

New configuration section in `config.toml`:
```toml
[search_intent]
enabled = true
use_llm = true
llm_timeout_ms = 200
min_confidence = 0.5

[search_intent.context]
base_count = 5
max_count = 15
max_tokens = 4000
```

Environment variable overrides:
- `SUBCOG_SEARCH_INTENT_ENABLED`
- `SUBCOG_SEARCH_INTENT_USE_LLM`
- `SUBCOG_SEARCH_INTENT_LLM_TIMEOUT_MS`
- `SUBCOG_SEARCH_INTENT_MIN_CONFIDENCE`

### Rollout Strategy

1. **Phase 1**: Ship with `enabled = true`, `use_llm = false` (keyword only)
2. **Phase 2**: Enable LLM classification after stability confirmed
3. **Phase 3**: Default LLM on when provider configured

### Rollback Plan

1. Set `enabled = false` in config or via environment variable
2. Restart MCP server
3. Hook passes through without memory injection

## Future Considerations

1. **PostToolUse integration**: Surface memories before specific tool types (e.g., before Read operations)
2. **Streaming injection**: Inject memories progressively as response streams
3. **User feedback loop**: Track which injected memories were useful
4. **Cross-project memory**: Surface memories from related projects
5. **Memory ranking ML**: Learn optimal namespace weights from usage patterns
