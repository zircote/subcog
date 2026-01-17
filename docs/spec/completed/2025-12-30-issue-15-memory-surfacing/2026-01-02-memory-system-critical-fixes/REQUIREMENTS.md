# Requirements: Subcog Memory System Critical Fixes

## Document Control

| Field | Value |
|-------|-------|
| Version | 1.0 |
| Status | Draft |
| Last Updated | 2026-01-02 |
| Author | Claude (Architect-Reviewer) |
| Reviewers | - |

## 1. Executive Summary

This specification addresses **5 critical architectural gaps** that prevent the Subcog memory system from functioning correctly. Users are experiencing:

1. **Memories saved but recall returns nothing** - Vector search returns empty results
2. **Extremely low relevance scores (0.01-0.02)** - RRF formula produces low scores by design when vector results are missing

### Root Cause Analysis

| Issue ID | Severity | Component | Root Cause |
|----------|----------|-----------|------------|
| MEM-001 | CRITICAL | Embedding | Placeholder hash-based embeddings, not semantic |
| MEM-002 | CRITICAL | Recall | Vector search stub always returns empty |
| MEM-003 | HIGH | Capture | Storage layers not synchronized |
| MEM-004 | HIGH | Recall | RecallService missing embedder/vector backends |
| MEM-005 | MEDIUM | Recall | RRF K=60 produces confusing low scores |

## 2. Problem Statement

### 2.1 Current Behavior

When a user captures a memory:
```
subcog capture --namespace decisions "Use PostgreSQL for primary storage"
```

And later recalls:
```
subcog recall "database storage"
```

**Expected**: Relevant memories with scores 0.5-0.9
**Actual**: Either no results or scores of 0.01-0.02

### 2.2 Technical Root Causes

#### MEM-001: Placeholder Embeddings

**Location**: `src/embedding/fastembed.rs:46-74`

The `FastEmbedEmbedder` generates pseudo-embeddings using hash-based word tokenization:

```rust
fn pseudo_embed(&self, text: &str) -> Vec<f32> {
 let mut embedding = vec![0.0f32; self.dimensions];
 for (i, word) in text.split_whitespace().enumerate() {
 let mut hasher = DefaultHasher::new();
 word.hash(&mut hasher);
 let hash = hasher.finish();
 // Distributes hash across dimensions - NOT SEMANTIC
 }
}
```

**Impact**: Semantic similarity is impossible. "PostgreSQL database" and "database storage" share no semantic relationship in hash space.

#### MEM-002: Vector Search Stub

**Location**: `src/services/recall.rs:241-250`

```rust
const fn vector_search(&self, _query: &str, _filter: &SearchFilter, _limit: usize)
 -> Result<Vec<SearchHit>> {
 Ok(Vec::new()) // ALWAYS RETURNS EMPTY
}
```

**Impact**: Hybrid search degrades to text-only (BM25), missing semantic matches.

#### MEM-003: Storage Layers Not Synchronized

**Location**: `src/services/capture.rs:119-130`

```rust
let memory = Memory {
 embedding: None, // HARDCODED TO NONE
 //...
};
// Missing: index.index(&memory)?;
// Missing: vector.upsert(&memory)?;
```

**Impact**: Memories exist in Git Notes but are not searchable via index or vector.

#### MEM-004: RecallService Missing Backends

**Location**: `src/services/recall.rs:19-22`

```rust
pub struct RecallService {
 index: Option<SqliteBackend>,
 // Missing: embedder: Option<Arc<dyn Embedder>>,
 // Missing: vector: Option<Arc<dyn VectorBackend>>,
}
```

**Impact**: Even if `vector_search` were implemented, there's no way to embed queries or search vectors.

#### MEM-005: RRF Low Scores

**Location**: `src/services/recall.rs:310-365`

```rust
// RRF formula: score = 1 / (K + rank)
// With K=60 and rank=1, max score = 1/61 ≈ 0.0164
```

**Impact**: Scores appear "broken" to users but are mathematically correct. With missing vector results, final scores are halved again.

## 3. Requirements

### 3.1 Functional Requirements

#### FR-001: Real Semantic Embeddings

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-001.1 | Integrate fastembed-rs with all-MiniLM-L6-v2 model | P0 | Embedding generation completes in <100ms |
| FR-001.2 | Lazy load model on first use | P0 | Cold start remains <10ms, warm embedding <50ms |
| FR-001.3 | Thread-safe singleton pattern | P0 | Concurrent embedding requests don't crash |
| FR-001.4 | Graceful fallback when model unavailable | P1 | BM25-only search with warning |

#### FR-002: Working Vector Search

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-002.1 | Implement `vector_search()` using embedder + vector backend | P0 | Returns top-K results by cosine similarity |
| FR-002.2 | Add embedder field to RecallService | P0 | Query embedding generated for each search |
| FR-002.3 | Add vector backend field to RecallService | P0 | usearch or pgvector backend connected |
| FR-002.4 | Filter by namespace before vector search | P1 | Namespace filtering works correctly |

#### FR-003: Synchronized Storage Layers

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-003.1 | Generate embedding during capture | P0 | All captured memories have embeddings |
| FR-003.2 | Index memory in SQLite FTS5 | P0 | Text search finds all memories |
| FR-003.3 | Upsert embedding to vector backend | P0 | Vector search finds all memories |
| FR-003.4 | Transaction-like behavior | P1 | All three layers succeed or none do |
| FR-003.5 | Backfill existing memories | P2 | Migration tool for existing Git Notes |

#### FR-004: RecallService Backend Integration

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-004.1 | Add `embedder: Option<Arc<dyn Embedder>>` field | P0 | Field exists with proper type |
| FR-004.2 | Add `vector: Option<Arc<dyn VectorBackend>>` field | P0 | Field exists with proper type |
| FR-004.3 | Constructor accepts embedder and vector | P0 | DI pattern works |
| FR-004.4 | Implement hybrid search with both backends | P0 | Text + vector results combined |

#### FR-005: Score Normalization

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-005.1 | Normalize RRF scores to 0.0-1.0 range | P1 | Users see intuitive scores |
| FR-005.2 | Weight text vs vector by intent | P2 | Different intents prioritize differently |
| FR-005.3 | Return raw and normalized scores | P2 | Debug mode shows both |

### 3.2 Non-Functional Requirements

#### NFR-001: Performance

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-001.1 | Cold start (binary) | <10ms | Time to main() |
| NFR-001.2 | Model load (first embedding) | <2s | Time to first embed |
| NFR-001.3 | Warm embedding | <50ms | Subsequent embeds |
| NFR-001.4 | Capture latency (with embedding) | <100ms | End-to-end capture |
| NFR-001.5 | Search latency | <100ms | End-to-end recall |
| NFR-001.6 | Memory (idle) | <50MB | RSS without model |
| NFR-001.7 | Memory (with model) | <150MB | RSS with model loaded |

#### NFR-002: Reliability

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-002.1 | Graceful degradation when embedder fails | BM25-only with warning |
| NFR-002.2 | Graceful degradation when vector fails | Text-only search |
| NFR-002.3 | No data loss on partial failure | Git Notes always written |
| NFR-002.4 | Idempotent capture | Duplicate detection works |

#### NFR-003: Compatibility

| ID | Requirement |
|----|-------------|
| NFR-003.1 | Backward compatible with existing Git Notes |
| NFR-003.2 | Migration path for memories without embeddings |
| NFR-003.3 | Existing CLI commands unchanged |
| NFR-003.4 | Existing MCP tools unchanged (behavior improved) |

### 3.3 Testing Requirements

| ID | Requirement | Type | Coverage Target |
|----|-------------|------|-----------------|
| TR-001 | Unit tests for FastEmbedEmbedder | Unit | 100% |
| TR-002 | Unit tests for vector_search() | Unit | 100% |
| TR-003 | Integration test: capture -> recall round-trip | Integration | Core flow |
| TR-004 | Integration test: semantic similarity | Integration | Similar text finds similar |
| TR-005 | Property test: score range 0.0-1.0 | Property | All inputs |
| TR-006 | Performance benchmark: embedding latency | Benchmark | <50ms p99 |
| TR-007 | Performance benchmark: search latency | Benchmark | <100ms p99 |

## 4. User Stories

### US-001: Semantic Memory Recall

**As a** developer using Subcog
**I want** to recall memories by semantic meaning
**So that** I can find relevant context even with different wording

**Acceptance Criteria**:
- Given I captured "Use PostgreSQL for the primary database"
- When I search "database storage decision"
- Then I receive the PostgreSQL memory with score > 0.5

### US-002: Intuitive Relevance Scores

**As a** developer reviewing recall results
**I want** scores that reflect actual relevance (0.0-1.0)
**So that** I can trust high scores mean high relevance

**Acceptance Criteria**:
- Given semantically identical text, score > 0.9
- Given semantically related text, score 0.5-0.9
- Given unrelated text, score < 0.3

### US-003: Reliable Capture Pipeline

**As a** developer capturing decisions
**I want** memories to be immediately searchable
**So that** I don't lose important context

**Acceptance Criteria**:
- Given I capture a memory
- When I immediately recall with same text
- Then I receive that memory in results

### US-004: Performance Under Load

**As a** developer with 10,000+ memories
**I want** search to remain fast (<100ms)
**So that** Subcog doesn't slow down my workflow

**Acceptance Criteria**:
- Given 10,000 memories in the index
- When I perform a search
- Then results return in <100ms

## 5. Constraints

### 5.1 Technical Constraints

| Constraint | Rationale |
|------------|-----------|
| Must use fastembed-rs (not API calls) | Offline-first, no network dependency |
| Model: all-MiniLM-L6-v2 | Balanced size (22MB) and quality |
| 384 dimensions | Fixed by model choice |
| MSRV Rust 1.85 | Project requirement |
| No unsafe code | Project policy |

### 5.2 Timeline Constraints

| Milestone | Target |
|-----------|--------|
| Spec approval | 2026-01-03 |
| Phase 1 complete (embeddings) | 2026-01-05 |
| Phase 2 complete (recall) | 2026-01-07 |
| Phase 3 complete (capture) | 2026-01-09 |
| Phase 4 complete (testing) | 2026-01-12 |
| Production ready | 2026-01-15 |

## 6. Out of Scope

The following are explicitly **not** in scope for this specification:

1. Multi-model support (e.g., OpenAI embeddings)
2. GPU acceleration for embeddings
3. Distributed vector search
4. Real-time embedding updates on memory edit
5. Embedding model fine-tuning
6. Cross-repository memory sharing

## 7. Dependencies

### 7.1 Crate Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| fastembed | 4.x | ONNX embeddings |
| usearch | 2.x | HNSW vector index |
| ort | (via fastembed) | ONNX runtime |

### 7.2 Internal Dependencies

| Component | Status | Dependency Type |
|-----------|--------|-----------------|
| Embedder trait | Exists | Interface |
| VectorBackend trait | Exists | Interface |
| UsearchBackend | Exists | Implementation |
| SqliteBackend | Exists | Implementation |
| CompositeStorage | Exists | Orchestration |

## 8. Success Metrics

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| Recall success rate | ~10% | >90% | Relevant memory in top-5 |
| Average relevance score | 0.01 | >0.5 | For semantic matches |
| Capture-to-searchable latency | N/A | <100ms | End-to-end |
| User satisfaction | Low | High | Anecdotal feedback |

## 9. Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| fastembed-rs model size bloats binary | Medium | Medium | Lazy loading, separate download |
| ONNX runtime conflicts | Low | High | Pin versions, test matrix |
| Performance regression | Medium | Medium | Benchmarks in CI |
| Breaking changes to existing data | Low | Critical | Migration tool, backward compat |

## 10. Appendix

### A. Related Specifications

- [2025-12-28-subcog-rust-rewrite](../2025-12-28-subcog-rust-rewrite/) - Parent specification
- [2026-01-01-pre-compact-deduplication](../../completed/2026-01-01-pre-compact-deduplication/) - Deduplication (completed)

### B. Research References

- fastembed-rs documentation: https://github.com/Anush008/fastembed-rs
- all-MiniLM-L6-v2 model card: https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2
- RRF paper: https://plg.uwaterloo.ca/~gvcormac/cormacksigir09-rrf.pdf
