---
document_type: decisions
project_id: SPEC-2026-01-01-001
---

# Pre-Compact Deduplication - Architecture Decision Records

## ADR-001: Short-Circuit Evaluation Order

**Date**: 2026-01-01
**Status**: Accepted
**Deciders**: User (via elicitation)

### Context

The deduplication service needs to perform three checks: exact match, semantic similarity, and recent capture. These checks have different performance characteristics and accuracy levels. We need to determine the order of evaluation.

### Decision

Evaluate in order: **Exact Match → Semantic Similarity → Recent Capture**, with short-circuit on first match.

### Consequences

**Positive:**
- Exact match is fastest (<10ms) and most accurate - handles common case efficiently
- Semantic check runs only when exact match fails (reduces embedding cost)
- Recent capture is last since it's a fallback for rapid duplicates

**Negative:**
- Recent capture may catch something semantic would have caught at higher confidence
- Order is fixed (not configurable)

**Neutral:**
- All three checks are independent and could theoretically run in parallel

---

## ADR-002: Content Hash Storage as Tags

**Date**: 2026-01-01
**Status**: Accepted
**Deciders**: Implementation analysis

### Context

To enable fast exact-match lookup, we need to store a content hash with each memory. Options include: new schema column, metadata field, or tags.

### Decision

Store SHA256 hash as a tag with format `hash:sha256:<16-char-prefix>`.

### Consequences

**Positive:**
- No schema migration required
- Works with existing tag search infrastructure
- SQLite FTS5 index already optimized for tag queries
- Tag prefix allows for multiple hash algorithms in future

**Negative:**
- Tag storage slightly less efficient than dedicated column
- Truncated to 16 chars (collision unlikely but possible)
- Pollutes tag namespace

### Alternatives Considered

1. **New schema column**: Would require migration, but more efficient lookup
2. **Metadata JSON field**: Flexible but no index support
3. **Separate hash table**: More complex, additional storage layer

---

## ADR-003: Per-Namespace Similarity Thresholds

**Date**: 2026-01-01
**Status**: Accepted
**Deciders**: User (via elicitation)

### Context

Different namespace types may have different tolerance for similarity. Decisions might need stricter matching than learnings.

### Decision

Support per-namespace thresholds via environment variables (`SUBCOG_DEDUP_THRESHOLD_{NAMESPACE}`) with a default fallback.

### Consequences

**Positive:**
- Fine-grained control for tuning accuracy per content type
- Decisions (stricter) vs learnings (looser) can be configured independently
- Operators can tune based on observed false positive rates

**Negative:**
- More configuration complexity
- Harder to document all permutations
- May confuse users with too many options

### Default Values

| Namespace | Default Threshold | Rationale |
|-----------|-------------------|-----------|
| Decisions | 0.92 | High value, avoid losing unique decisions |
| Patterns | 0.90 | Standard threshold |
| Learnings | 0.88 | Learnings often phrased differently |
| Default | 0.90 | Match documented behavior |

---

## ADR-004: In-Memory LRU Cache for Recent Captures

**Date**: 2026-01-01
**Status**: Accepted
**Deciders**: User (via elicitation)

### Context

We need to track recently captured content to prevent rapid duplicate captures within a 5-minute window. Options include: in-memory cache, SQLite query, Redis.

### Decision

Use in-memory LRU cache with TTL eviction.

### Consequences

**Positive:**
- Fastest option (<1ms lookup)
- No external dependencies
- Bounded memory usage (1000 entries max)
- Simple implementation with `lru` crate

**Negative:**
- State lost on process restart (acceptable - window is only 5 minutes)
- Not shared across multiple subcog instances
- TTL handling requires custom logic

### Alternatives Considered

1. **SQLite query**: Would add ~10-30ms latency per check
2. **Redis**: Adds operational complexity, overkill for single-instance use
3. **Both with fallback**: Complexity not justified for this use case

---

## ADR-005: Fail-Open on Deduplication Errors

**Date**: 2026-01-01
**Status**: Accepted
**Deciders**: Design principle (graceful degradation)

### Context

What should happen if a deduplication check fails? Options: fail closed (block capture), fail open (allow capture), or skip with warning.

### Decision

Fail open - if deduplication checks fail, proceed with capture and log a warning.

### Consequences

**Positive:**
- Prioritizes data capture over duplicate prevention
- Aligns with existing hook error handling philosophy
- Temporary failures don't block important captures
- Consolidation service can clean up duplicates later

**Negative:**
- May capture duplicates during service degradation
- Errors might go unnoticed if only logged at warn level

**Neutral:**
- Metrics will show elevated capture counts during failures

---

## ADR-006: Semantic Check Minimum Length

**Date**: 2026-01-01
**Status**: Accepted
**Deciders**: Performance analysis

### Context

Very short content (<50 chars) produces poor embeddings and is cheap to store. Should we run semantic checks on short content?

### Decision

Skip semantic similarity check for content shorter than 50 characters. Only use exact match and recent capture for short content.

### Consequences

**Positive:**
- Avoids noisy embeddings from short phrases
- Saves embedding computation cost
- Short duplicates caught by exact match anyway

**Negative:**
- Very short content with slight variations won't be caught by semantic check
- Threshold is somewhat arbitrary

---

## ADR-007: RecallService for Deduplication Lookups

**Date**: 2026-01-01
**Status**: Accepted
**Deciders**: User (via elicitation)

### Context

We need to query existing memories to check for duplicates. Options: direct index query, direct vector query, or use RecallService.

### Decision

Use `RecallService.search()` with appropriate filters for deduplication lookups.

### Consequences

**Positive:**
- Consistent with existing search patterns
- Automatic handling of hybrid search modes
- Benefits from any future RecallService optimizations
- Single source of truth for search logic

**Negative:**
- Slightly more overhead than direct backend queries
- RecallService may do more work than strictly needed

### Alternatives Considered

1. **Direct IndexBackend query**: Faster but bypasses abstractions
2. **Direct VectorBackend query**: Misses text-based fallback
3. **New DeduplicationBackend**: Overkill, duplicates functionality

---

## ADR-008: Hook Output Format for Skip Reporting

**Date**: 2026-01-01
**Status**: Accepted
**Deciders**: Observability requirements

### Context

How should skipped duplicates be reported in the hook output? Options: silent skip, count only, or detailed listing.

### Decision

Include detailed skip information in `additionalContext` with reason, matched ID, and similarity score.

### Consequences

**Positive:**
- Full transparency for debugging
- Users can verify deduplication is working
- Similarity scores help tune thresholds

**Negative:**
- Longer output in hook response
- May be verbose during high-dedup scenarios

### Output Format

```
Skipped 3 duplicates:
- Exact match: "Use PostgreSQL for..." (matches subcog://global/decisions/abc123)
- Semantic 94%: "We chose Postgres..." (similar to subcog://global/decisions/abc123)
- Recent capture: "PostgreSQL decision..." (captured 2 min ago, subcog://global/decisions/def456)
```

**Note**: All memory references use full URN format: `subcog://{domain}/{namespace}/{id}`
