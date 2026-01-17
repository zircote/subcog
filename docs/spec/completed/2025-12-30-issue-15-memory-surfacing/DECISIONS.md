---
document_type: decisions
project_id: SPEC-2025-12-30-001
---

# Proactive Memory Surfacing - Architecture Decision Records

## ADR-001: Hybrid Detection Strategy (Keyword + LLM)

**Date**: 2025-12-30
**Status**: Accepted
**Deciders**: Project team

### Context

We need to detect search intent in user prompts to trigger proactive memory surfacing. Two approaches are available:
1. Keyword-based pattern matching (fast, deterministic, limited accuracy)
2. LLM-based classification (slower, more accurate, requires LLM provider)

### Decision

Implement a **hybrid detection strategy** that combines both approaches:
- Run keyword detection first (always, <10ms)
- Optionally run LLM classification in parallel when configured
- Merge results: use LLM topics when available, keyword confidence as baseline
- Fall back to keyword-only when LLM unavailable or times out

### Consequences

**Positive:**
- Best of both worlds: fast baseline + optional accuracy boost
- Graceful degradation when LLM unavailable
- Keyword detection provides consistent, predictable behavior
- LLM classification improves topic extraction significantly

**Negative:**
- More complex implementation than either approach alone
- Two code paths to test and maintain
- Potential for confusion about which detection source was used

**Neutral:**
- Users can disable LLM classification via configuration
- Detection source tracked in SearchIntent for debugging

---

## ADR-002: Namespace Weighting Over Query Rewriting

**Date**: 2025-12-30
**Status**: Accepted
**Deciders**: Project team

### Context

To improve recall relevance based on detected intent type, we could either:
1. Rewrite the search query to include namespace-specific terms
2. Apply post-retrieval weight multipliers to specific namespaces

### Decision

Use **namespace weight multipliers** applied after retrieval:
- HowTo: Patterns 1.5x, Learnings 1.3x
- Troubleshoot: Blockers 1.5x, Learnings 1.3x
- Location/Explanation: Decisions 1.5x, Context 1.3x

### Consequences

**Positive:**
- Clean separation of concerns (retrieval vs ranking)
- Easier to understand and debug
- Works with existing RecallService search methods
- Weights can be tuned without changing queries

**Negative:**
- May retrieve irrelevant memories before weighting
- Requires fetching more memories than needed (2x limit)

**Neutral:**
- Weights are static per intent type (no learning)

### Alternatives Considered

1. **Query rewriting**: Add namespace terms to query (e.g., "auth patterns")
 - Rejected: Changes semantics of search, harder to predict results

2. **Filtered search per namespace**: Run separate searches per weighted namespace
 - Rejected: Multiple queries = higher latency

---

## ADR-003: In-Memory Topic Index

**Date**: 2025-12-30
**Status**: Accepted
**Deciders**: Project team

### Context

To support topic-based memory surfacing (`subcog://topics/{topic}`), we need to maintain a topic -> memory mapping. Options:
1. Build index on every request
2. Store index in SQLite alongside memories
3. Maintain in-memory HashMap, rebuild on startup

### Decision

Use an **in-memory HashMap with RwLock**, rebuilt at MCP server startup and updated on capture:
```rust
topics: Arc<RwLock<HashMap<String, Vec<MemoryId>>>>
```

### Consequences

**Positive:**
- Very fast lookups (<5ms)
- No additional database schema changes
- Simple implementation
- Automatic refresh on capture

**Negative:**
- Lost on server restart (rebuilt from memories)
- Memory overhead for large projects
- RwLock contention on updates

**Neutral:**
- Index rebuild is async at startup
- Typical project has <100 topics

### Alternatives Considered

1. **On-demand building**: Build index on first topic request
 - Rejected: Unpredictable first-request latency

2. **SQLite persistence**: Store topic index in database
 - Rejected: Overkill for volatile derived data

---

## ADR-004: 200ms LLM Timeout

**Date**: 2025-12-30
**Status**: Accepted
**Deciders**: Project team

### Context

LLM classification can vary widely in latency (50ms to 2s+). We need a timeout that balances accuracy (waiting for LLM) vs responsiveness (not blocking the hook).

### Decision

Use a **200ms timeout** for LLM classification with automatic fallback to keyword detection:
- 200ms chosen based on P95 LLM latency for classification tasks
- Timeout triggers keyword-only path, not an error
- Timeout events are logged for monitoring

### Consequences

**Positive:**
- Guarantees <200ms LLM contribution to total latency
- Graceful fallback to working (keyword) detection
- Predictable worst-case response time

**Negative:**
- May miss LLM classification in ~10% of cases (slow responses)
- Short timeout may waste LLM API calls

**Neutral:**
- Timeout configurable via `llm_timeout_ms`

### Alternatives Considered

1. **500ms timeout**: More LLM results, but too slow for UX
 - Rejected: Total hook latency would exceed 200ms target

2. **No timeout, async**: Return immediately, inject LLM results on next message
 - Rejected: Complex state management, confusing UX

---

## ADR-005: Token Budget for Injected Memories

**Date**: 2025-12-30
**Status**: Accepted
**Deciders**: Project team

### Context

Injecting too many memories or too much content can overwhelm the assistant's context and slow processing. We need to limit injected content.

### Decision

Implement a **token budget** with content truncation:
- Default: 4000 tokens for injected memories
- Memory content truncated to ~200 chars for preview
- Full content available via memory URN if needed

### Consequences

**Positive:**
- Prevents context bloat
- Consistent injection size
- Memory URN provides access to full content

**Negative:**
- Truncation may hide important details
- Fixed budget may not be optimal for all cases

**Neutral:**
- Budget configurable via `max_tokens`

---

## ADR-006: Confidence Threshold for Injection

**Date**: 2025-12-30
**Status**: Accepted
**Deciders**: Project team

### Context

Not all detected intents should trigger memory injection. Very low confidence detections may inject irrelevant memories.

### Decision

Use a **configurable confidence threshold** (default 0.5):
- Confidence < 0.5: No memory injection, signal still logged
- Confidence >= 0.5: Inject memories, include reminder
- Confidence >= 0.8: Inject more memories (max_count)

### Consequences

**Positive:**
- Reduces false positive injections
- Threshold can be tuned per environment
- Graduated response based on confidence

**Negative:**
- May miss valid intents at threshold boundary
- Default may not be optimal for all users

**Neutral:**
- Threshold exposed in configuration

### Alternatives Considered

1. **Always inject on any detection**: Simple, but noisy
 - Rejected: Low-confidence detections add noise

2. **User-configurable per session**: Too complex for MVP
 - Deferred: Consider for future enhancement
