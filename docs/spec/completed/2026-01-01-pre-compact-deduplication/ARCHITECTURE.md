---
document_type: architecture
project_id: SPEC-2026-01-01-001
version: 1.0.0
last_updated: 2026-01-01T00:00:00Z
status: draft
---

# Pre-Compact Deduplication - Technical Architecture

## System Overview

This architecture introduces a `DeduplicationService` that integrates with `PreCompactHandler` to check for existing similar memories before auto-capture. The service implements three deduplication strategies: exact match, semantic similarity, and recent capture detection.

### Architecture Diagram

```
┌──────────────────────────────────────────────────────────────────────────┐
│                         PreCompactHandler                                │
│  ┌──────────────┐    ┌───────────────────┐    ┌───────────────────────┐  │
│  │ analyze_     │───▶│ deduplicate_      │───▶│ capture_              │  │
│  │ content()    │    │ candidates()      │    │ candidates()          │  │
│  └──────────────┘    └─────────┬─────────┘    └───────────────────────┘  │
│                                │                                         │
└────────────────────────────────┼─────────────────────────────────────────┘
                                 │
                                 ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                       DeduplicationService                               │
│  ┌───────────────────────────────────────────────────────────────────┐   │
│  │                    check_duplicate()                              │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐   │   │
│  │  │ ExactMatch   │  │ Semantic     │  │ RecentCapture          │   │   │
│  │  │ Checker      │  │ Checker      │  │ Checker                │   │   │
│  │  │              │  │              │  │                        │   │   │
│  │  │ SHA256 hash  │  │ Embedding    │  │ LRU Cache with TTL     │   │   │
│  │  │ comparison   │  │ similarity   │  │ (5 min window)         │   │   │
│  │  └──────┬───────┘  └──────┬───────┘  └────────────┬───────────┘   │   │
│  │         │                 │                       │               │   │
│  │         ▼                 ▼                       ▼               │   │
│  │  ┌─────────────────────────────────────────────────────────────┐  │   │
│  │  │              DuplicateCheckResult                           │  │   │
│  │  │  is_duplicate: bool                                         │  │   │
│  │  │  reason: ExactMatch | SemanticSimilar | RecentCapture       │  │   │
│  │  │  similarity_score: Option<f32>                              │  │   │
│  │  │  matched_memory_id: Option<MemoryId>                        │  │   │
│  │  └─────────────────────────────────────────────────────────────┘  │   │
│  └───────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│  Dependencies:                                                           │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────────────────┐  │
│  │ RecallService  │  │ FastEmbed      │  │ RecentCaptureCache         │  │
│  │ (search)       │  │ Embedder       │  │ (in-memory LRU)            │  │
│  └────────────────┘  └────────────────┘  └────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────┘
```

### Key Design Decisions

1. **Separate DeduplicationService**: Encapsulates all dedup logic, injectable into PreCompactHandler
2. **Short-circuit evaluation**: Exit early on first duplicate match (exact → semantic → recent)
3. **Namespace-scoped search**: Only compare within same namespace to avoid cross-category false positives
4. **Configurable thresholds**: Per-namespace similarity thresholds via environment variables

## Component Design

### Component 1: DeduplicationService

- **Purpose**: Orchestrates the three-tier deduplication check
- **Responsibilities**:
  - Check if content is a duplicate using exact, semantic, or recent capture detection
  - Provide detailed results for observability
  - Handle graceful degradation when backends unavailable
- **Interfaces**:
  ```rust
  pub trait Deduplicator: Send + Sync {
      fn check_duplicate(
          &self,
          content: &str,
          namespace: Namespace,
      ) -> Result<DuplicateCheckResult>;

      fn record_capture(&self, content_hash: &str, memory_id: &MemoryId);
  }
  ```
- **Dependencies**: RecallService, Embedder, RecentCaptureCache
- **Technology**: Rust, SHA256 from `sha2` crate

### Component 2: ExactMatchChecker

- **Purpose**: Fast hash-based exact duplicate detection
- **Responsibilities**:
  - Generate SHA256 hash of normalized content
  - Query index for memories with matching hash
- **Interfaces**:
  ```rust
  impl ExactMatchChecker {
      pub fn new(recall: Arc<RecallService>) -> Self;
      pub fn check(&self, content: &str, namespace: Namespace) -> Result<Option<MemoryId>>;
  }
  ```
- **Implementation Notes**:
  - Normalize content: trim, lowercase, collapse whitespace
  - Store hash as tag: `hash:sha256:<first-16-chars-of-hash>`
  - Search using tag filter for O(1) lookup

### Component 3: SemanticSimilarityChecker

- **Purpose**: Embedding-based similarity detection for paraphrased content
- **Responsibilities**:
  - Generate embedding for candidate content
  - Search vector store for similar memories
  - Apply namespace-specific similarity threshold
- **Interfaces**:
  ```rust
  impl SemanticSimilarityChecker {
      pub fn new(
          recall: Arc<RecallService>,
          embedder: Arc<dyn Embedder>,
          thresholds: HashMap<Namespace, f32>,
      ) -> Self;

      pub fn check(
          &self,
          content: &str,
          namespace: Namespace,
      ) -> Result<Option<(MemoryId, f32)>>;
  }
  ```
- **Threshold Configuration**:
  ```
  SUBCOG_DEDUP_THRESHOLD_DECISIONS=0.92
  SUBCOG_DEDUP_THRESHOLD_PATTERNS=0.90
  SUBCOG_DEDUP_THRESHOLD_LEARNINGS=0.88
  SUBCOG_DEDUP_THRESHOLD_DEFAULT=0.90
  ```

### Component 4: RecentCaptureChecker

- **Purpose**: Time-based deduplication for rapid repeated captures
- **Responsibilities**:
  - Track recently captured content hashes with timestamps
  - Evict entries older than TTL (5 minutes default)
  - Check if content was captured within window
- **Interfaces**:
  ```rust
  impl RecentCaptureChecker {
      pub fn new(ttl: Duration, capacity: usize) -> Self;
      pub fn check(&self, content_hash: &str) -> Option<(MemoryId, Instant)>;
      pub fn record(&self, content_hash: &str, memory_id: &MemoryId);
  }
  ```
- **Implementation**: LRU cache with TTL using `lru` crate with time-based eviction

### Component 5: DuplicateCheckResult

- **Purpose**: Structured result of deduplication check
- **Definition**:
  ```rust
  #[derive(Debug, Clone)]
  pub struct DuplicateCheckResult {
      pub is_duplicate: bool,
      pub reason: Option<DuplicateReason>,
      pub similarity_score: Option<f32>,
      pub matched_memory_id: Option<MemoryId>,
      /// Full URN of matched memory: subcog://{domain}/{namespace}/{id}
      pub matched_urn: Option<String>,
      pub check_duration_ms: u64,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum DuplicateReason {
      ExactMatch,
      SemanticSimilar,
      RecentCapture,
  }
  ```

- **URN Requirement**: The `matched_urn` field MUST be populated when `is_duplicate == true`. All external outputs (logs, metrics labels, hook responses) MUST reference memories by URN, not bare ID.

## Data Design

### Data Flow

```
┌─────────────┐     ┌───────────────────┐     ┌─────────────────────┐
│ Candidate   │────▶│ Content Hash      │────▶│ Exact Match Check   │
│ Content     │     │ (SHA256)          │     │ (tag search)        │
└─────────────┘     └───────────────────┘     └──────────┬──────────┘
                                                         │
                                                         │ Not found
                                                         ▼
                    ┌───────────────────┐     ┌─────────────────────┐
                    │ Content Embedding │────▶│ Semantic Check      │
                    │ (FastEmbed 384d)  │     │ (vector search)     │
                    └───────────────────┘     └──────────┬──────────┘
                                                         │
                                                         │ Below threshold
                                                         ▼
                    ┌───────────────────┐     ┌─────────────────────┐
                    │ Content Hash      │────▶│ Recent Capture      │
                    │ (same as step 1)  │     │ (LRU cache lookup)  │
                    └───────────────────┘     └──────────┬──────────┘
                                                         │
                                                         │ Not in cache
                                                         ▼
                                              ┌─────────────────────┐
                                              │ Proceed with        │
                                              │ Capture             │
                                              └─────────────────────┘
```

### Storage Strategy

- **Content Hash Tag**: Stored as tag `hash:sha256:<16-char-prefix>` on each memory
- **Embeddings**: Already stored via VectorBackend during normal capture
- **Recent Captures**: In-memory LRU cache, not persisted (intentional - clears on restart)

### Index Schema Changes

No schema changes required. The existing tag system supports hash-based lookup:

```sql
-- Existing FTS5 table already supports tag search
SELECT * FROM memories_fts WHERE tags MATCH '"hash:sha256:a1b2c3d4e5f6"'
```

## API Design

### DeduplicationService API

```rust
impl DeduplicationService {
    /// Creates a new deduplication service with all checkers.
    pub fn new(
        recall: Arc<RecallService>,
        embedder: Arc<dyn Embedder>,
        config: DeduplicationConfig,
    ) -> Self;

    /// Creates a service with only exact match and recent capture (no embeddings).
    pub fn without_embeddings(
        recall: Arc<RecallService>,
        config: DeduplicationConfig,
    ) -> Self;

    /// Checks if content is a duplicate.
    ///
    /// Returns early on first match. Check order: exact → semantic → recent.
    pub fn check_duplicate(
        &self,
        content: &str,
        namespace: Namespace,
    ) -> Result<DuplicateCheckResult>;

    /// Records a successful capture for recent-capture tracking.
    pub fn record_capture(&self, content: &str, memory_id: &MemoryId);
}
```

### Configuration

```rust
#[derive(Debug, Clone)]
pub struct DeduplicationConfig {
    /// Enable/disable entire deduplication.
    pub enabled: bool,

    /// Per-namespace similarity thresholds.
    pub similarity_thresholds: HashMap<Namespace, f32>,

    /// Default threshold when namespace not configured.
    pub default_threshold: f32,

    /// Recent capture time window.
    pub recent_window: Duration,

    /// Recent capture cache capacity.
    pub cache_capacity: usize,

    /// Minimum content length for semantic check.
    pub min_semantic_length: usize,
}

impl Default for DeduplicationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            similarity_thresholds: HashMap::new(),
            default_threshold: 0.90,
            recent_window: Duration::from_secs(300), // 5 minutes
            cache_capacity: 1000,
            min_semantic_length: 50,
        }
    }
}
```

### Environment Variables

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SUBCOG_DEDUP_ENABLED` | bool | `true` | Enable deduplication |
| `SUBCOG_DEDUP_THRESHOLD_DECISIONS` | f32 | `0.92` | Threshold for decisions namespace |
| `SUBCOG_DEDUP_THRESHOLD_PATTERNS` | f32 | `0.90` | Threshold for patterns namespace |
| `SUBCOG_DEDUP_THRESHOLD_LEARNINGS` | f32 | `0.88` | Threshold for learnings namespace |
| `SUBCOG_DEDUP_THRESHOLD_DEFAULT` | f32 | `0.90` | Default threshold |
| `SUBCOG_DEDUP_TIME_WINDOW_SECS` | u64 | `300` | Recent capture window |
| `SUBCOG_DEDUP_CACHE_CAPACITY` | usize | `1000` | LRU cache size |

## Integration Points

### Integration with PreCompactHandler

```rust
impl PreCompactHandler {
    pub fn with_deduplication(mut self, dedup: DeduplicationService) -> Self {
        self.dedup = Some(dedup);
        self
    }

    fn deduplicate_candidates(&self, candidates: Vec<CaptureCandidate>) -> Vec<CaptureCandidate> {
        let Some(dedup) = &self.dedup else {
            // Fall back to existing prefix-based dedup
            return Self::legacy_deduplicate(candidates);
        };

        candidates
            .into_iter()
            .filter_map(|c| {
                match dedup.check_duplicate(&c.content, c.namespace) {
                    Ok(result) if result.is_duplicate => {
                        self.record_skip(&c, &result);
                        None
                    }
                    Ok(_) => Some(c),
                    Err(e) => {
                        tracing::warn!("Dedup check failed, proceeding: {}", e);
                        Some(c) // Fail open
                    }
                }
            })
            .collect()
    }
}
```

### Hook Output Format

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreCompact",
    "additionalContext": "Auto-captured 2 memories before compaction:\n\n1. **decisions**: Use PostgreSQL\n   URN: subcog://global/decisions/abc123\n\nSkipped 3 duplicates:\n- Exact match: \"Use PostgreSQL for...\" (matches subcog://global/decisions/abc123)\n- Semantic 94%: \"We chose Postgres...\" (similar to subcog://global/decisions/abc123)\n- Recent capture: \"PostgreSQL decision...\" (captured 2 min ago, subcog://global/decisions/def456)"
  }
}
```

**URN Format**: All memory references MUST use the full URN scheme: `subcog://{domain}/{namespace}/{id}`

## Security Design

### Content Protection

- Debug logs use SHA256 hash fingerprints, never raw content
- Recent capture cache stores hashes, not content
- No new secrets or credentials required

### Memory Safety

- LRU cache is bounded to prevent unbounded growth
- All operations use `Result` types, never panic

## Performance Considerations

### Expected Load

- Typical: 1-5 candidates per pre-compact invocation
- Peak: 10-20 candidates during long sessions
- Frequency: Every 10-30 minutes during active development

### Performance Targets

| Operation | Target | Rationale |
|-----------|--------|-----------|
| SHA256 hash | <1ms | In-memory, CPU-bound |
| Tag search | <10ms | SQLite index lookup |
| Embedding generation | <20ms | FastEmbed cached model |
| Vector search | <30ms | usearch with limit=5 |
| LRU cache lookup | <1ms | In-memory hashmap |
| Total dedup overhead | <50ms | Sum of above, short-circuit |

### Optimization Strategies

1. **Short-circuit**: Stop on first match (exact is fastest)
2. **Batch queries**: If multiple candidates, batch tag searches
3. **Embed once**: Generate embedding only if exact match fails
4. **Limit results**: Vector search with limit=5 is sufficient

## Reliability & Operations

### Graceful Degradation

| Failure Mode | Behavior |
|--------------|----------|
| Embeddings unavailable | Skip semantic check, use exact + recent only |
| Index unavailable | Skip exact match, use semantic + recent only |
| Vector search fails | Skip semantic check, use exact + recent only |
| Cache unavailable | Skip recent check, use exact + semantic only |
| All checks fail | Proceed with capture (fail open) |

### Monitoring & Alerting

Metrics to emit:

```rust
// Counters
dedup_checks_total{namespace, result=duplicate|unique}
dedup_skipped_total{namespace, reason=exact|semantic|recent}

// Histograms
dedup_check_duration_ms{check_type=exact|semantic|recent|total}
dedup_similarity_score{namespace}  // For semantic matches

// Gauges
dedup_cache_size
dedup_cache_hit_rate
```

## Testing Strategy

### Unit Testing

- `ExactMatchChecker`: Test hash generation, normalization, tag format
- `SemanticSimilarityChecker`: Test threshold comparison, namespace filtering
- `RecentCaptureChecker`: Test TTL eviction, capacity limits

### Integration Testing

- End-to-end with real SQLite and usearch backends
- Test with actual FastEmbed embeddings
- Verify hook output format includes skip reasons

### Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn identical_content_always_duplicates(content in ".{50,500}") {
        let dedup = DeduplicationService::new(...);
        dedup.record_capture(&content, &MemoryId::new("test"));

        let result = dedup.check_duplicate(&content, Namespace::Decisions)?;
        prop_assert!(result.is_duplicate);
    }

    #[test]
    fn slightly_different_content_not_exact_match(
        base in ".{100,300}",
        suffix in ".{10,50}"
    ) {
        let modified = format!("{} {}", base, suffix);
        // Should not be exact match (but may be semantic match)
        let result = dedup.check_duplicate(&modified, Namespace::Decisions)?;
        prop_assert!(result.reason != Some(DuplicateReason::ExactMatch));
    }
}
```

### Benchmark Testing

```rust
#[bench]
fn bench_dedup_check_cold(b: &mut Bencher) {
    let dedup = setup_dedup_service();
    let content = "We decided to use PostgreSQL for the primary database.";

    b.iter(|| {
        dedup.check_duplicate(content, Namespace::Decisions)
    });
}

#[bench]
fn bench_dedup_check_hot_cache(b: &mut Bencher) {
    let dedup = setup_dedup_service();
    let content = "We decided to use PostgreSQL for the primary database.";
    dedup.record_capture(content, &MemoryId::new("test"));

    b.iter(|| {
        dedup.check_duplicate(content, Namespace::Decisions)
    });
}
```

## Deployment Considerations

### Feature Flag

```rust
// In config/features.rs
pub struct FeatureFlags {
    pub deduplication_enabled: bool,
    // ... other flags
}
```

### Rollout Strategy

1. Deploy with `SUBCOG_DEDUP_ENABLED=false` (disabled)
2. Enable in dev/staging, verify metrics
3. Enable in production with 92% threshold (conservative)
4. Tune thresholds based on false positive rate

### Rollback Plan

Set `SUBCOG_DEDUP_ENABLED=false` to disable entirely without code change.

## Future Considerations

1. **Distributed deduplication**: Use Redis for cross-instance recent capture cache
2. **Learned thresholds**: ML-based threshold tuning based on user feedback
3. **Async deduplication**: Background job for post-capture consolidation
4. **Cross-namespace detection**: Semantic similarity across namespaces with user confirmation
