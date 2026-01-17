# Architecture Decision Records: Subcog Memory System Critical Fixes

## Document Control

| Field | Value |
|-------|-------|
| Version | 1.0 |
| Status | Draft |
| Last Updated | 2026-01-02 |
| Author | Claude (Architect-Reviewer) |

---

## ADR-001: Use fastembed-rs for Embeddings

### Status

**Accepted**

### Context

The current `FastEmbedEmbedder` uses a placeholder hash-based implementation that produces non-semantic vectors. This breaks all semantic search functionality.

Options considered:
1. **fastembed-rs** - Rust wrapper for ONNX-based embeddings
2. **Anthropic API** - Claude embeddings via API
3. **OpenAI API** - OpenAI embeddings via API
4. **candle** - Rust ML framework for local inference
5. **tokenizers + manual** - Build from scratch

### Decision

Use **fastembed-rs** with the all-MiniLM-L6-v2 model.

### Rationale

| Option | Pros | Cons |
|--------|------|------|
| fastembed-rs | Offline, fast, well-maintained, easy API | Binary size (+~30MB) |
| Anthropic/OpenAI API | High quality | Network dependency, cost, latency |
| candle | Full control | Complex setup, maintenance burden |
| tokenizers | Minimal deps | Huge implementation effort |

fastembed-rs provides the best balance of:
- **Offline-first**: No network calls, works in air-gapped environments
- **Performance**: ~30ms per embed after model load
- **Quality**: all-MiniLM-L6-v2 is proven for semantic search
- **Maintenance**: Active project, Rust-native

### Consequences

**Positive**:
- Real semantic embeddings enable meaningful search
- No API keys or network required
- Deterministic results

**Negative**:
- Binary size increases by ~30MB (ONNX runtime + model)
- First embed has ~1-2s cold start (model loading)
- Model files need to be downloaded on first use

**Mitigations**:
- Lazy load model to preserve cold start
- Cache model in CI to avoid repeated downloads
- Consider model stripping for size optimization

---

## ADR-002: Lazy Load Embedding Model

### Status

**Accepted**

### Context

The all-MiniLM-L6-v2 model takes ~1-2 seconds to load from disk. Loading at application start would violate the <10ms cold start requirement.

Options considered:
1. **Eager loading** - Load model at startup
2. **Lazy loading** - Load on first embed call
3. **Background loading** - Load in separate thread at startup
4. **Pre-compiled binary** - Embed model in binary

### Decision

Use **lazy loading** via `OnceLock<TextEmbedding>` singleton.

### Rationale

```rust
static MODEL: OnceLock<TextEmbedding> = OnceLock::new();

fn get_model() -> Result<&'static TextEmbedding> {
 MODEL.get_or_try_init(|| TextEmbedding::try_new(...))
}
```

| Option | Cold Start | First Embed | Complexity |
|--------|------------|-------------|------------|
| Eager | ~2s | ~30ms | Low |
| Lazy | <10ms | ~2s | Low |
| Background | <10ms | Varies | High |
| Pre-compiled | <10ms | ~30ms | Very High |

### Consequences

**Positive**:
- Cold start remains <10ms
- Simple implementation
- Thread-safe via `OnceLock`

**Negative**:
- First embed call is slow (~2s)
- User may perceive lag on first capture/recall

**Mitigations**:
- Add progress indicator for first embed
- Consider background warm-up in MCP server mode
- Document expected first-use latency

---

## ADR-003: Three-Layer Storage Synchronization Strategy

### Status

**Accepted**

### Context

Subcog has three storage layers:
1. **Persistence** (Git Notes) - Authoritative, version-controlled
2. **Index** (SQLite FTS5) - Text search
3. **Vector** (usearch) - Semantic search

Currently, capture only writes to Git Notes. Index and vector are never updated.

Options considered:
1. **Synchronous all-or-nothing** - Transaction across all three
2. **Synchronous best-effort** - Write all, tolerate failures
3. **Async eventual consistency** - Queue writes, process async
4. **Git Notes only + rebuild** - Only persist, rebuild index/vector on demand

### Decision

Use **synchronous best-effort** with Git Notes as authoritative.

### Rationale

```
Capture Flow:
1. Generate embedding (fail fast)
2. Write Git Notes (authoritative, must succeed)
3. Index in SQLite (best effort)
4. Upsert to vector (best effort)
```

| Option | Consistency | Performance | Complexity |
|--------|-------------|-------------|------------|
| All-or-nothing | Strong | Slow | High |
| Best-effort | Eventual | Fast | Medium |
| Async | Eventual | Fastest | High |
| Rebuild | None until rebuild | Fast capture | Low |

### Consequences

**Positive**:
- Capture never fails due to index/vector issues
- Git Notes remain the source of truth
- Index/vector can be rebuilt from Git Notes
- Simpler error handling

**Negative**:
- Briefly inconsistent (memory in Git but not searchable)
- Need monitoring for index/vector failures
- May need periodic reconciliation

**Mitigations**:
- Log warnings for index/vector failures
- Add `subcog repair` command to reconcile
- Monitor for persistent failures

---

## ADR-004: Score Normalization to 0.0-1.0 Range

### Status

**Accepted**

### Context

The current RRF implementation produces scores in the range 0.0-0.033 (approximately). Users expect scores in 0.0-1.0 range where higher means more relevant.

Options considered:
1. **No normalization** - Return raw RRF scores
2. **Linear normalization** - Scale to 0.0-1.0 based on max
3. **Min-max normalization** - Scale based on min and max
4. **Sigmoid normalization** - Apply sigmoid function
5. **Percentile normalization** - Convert to percentile rank

### Decision

Use **linear normalization** based on maximum score in result set.

### Rationale

```rust
fn normalize(results: &[SearchHit]) -> Vec<SearchResult> {
 let max = results.iter().map(|r| r.score).fold(0.0, f32::max);
 results.iter().map(|r| SearchResult {
 score: if max > 0.0 { r.score / max } else { 0.0 },
 raw_score: r.score,
 }).collect()
}
```

| Option | Intuitive | Comparable | Stable |
|--------|-----------|------------|--------|
| Raw | No | Yes | Yes |
| Linear | Yes | No* | Yes |
| Min-max | Yes | No | No |
| Sigmoid | Sort of | No | Yes |
| Percentile | Yes | No | No |

*Linear normalization means scores aren't comparable across different queries, but within a query they're intuitive.

### Consequences

**Positive**:
- Users see intuitive 0.0-1.0 scores
- Top result always scores 1.0 (if results exist)
- Preserves relative ordering

**Negative**:
- Scores not comparable across queries
- May mask quality differences (if best match is poor)
- Single-result queries always show 1.0

**Mitigations**:
- Return `raw_score` alongside `score` for debugging
- Add `--raw` CLI flag for power users
- Document normalization behavior

---

## ADR-005: Graceful Degradation Strategy

### Status

**Accepted**

### Context

The system has multiple components that may fail independently:
- Embedding service (model load failure)
- Vector backend (index corruption)
- Index backend (SQLite issues)
- Network (for remote backends)

Need to ensure system remains useful even when components fail.

Options considered:
1. **Fail fast** - Any component failure stops operation
2. **Graceful degradation** - Fall back to available components
3. **Queue and retry** - Queue failed operations for later

### Decision

Use **graceful degradation** with clear fallback hierarchy.

### Rationale

Degradation matrix:

| Embedder | Vector | Index | Behavior |
|----------|--------|-------|----------|
| | | | Full hybrid search |
| | | | Vector-only + warning |
| | | | Text-only (BM25) + warning |
| | | | Text-only (BM25) + warning |
| | | | Error: no search available |

For capture:
```
1. Embedding fails -> Capture to Git Notes without embedding + warning
2. Index fails -> Continue, log warning
3. Vector fails -> Continue, log warning
4. Git Notes fails -> Error (authoritative store must succeed)
```

### Consequences

**Positive**:
- System remains functional under partial failure
- Users get best available results
- Git Notes never lost

**Negative**:
- May return incomplete results without user knowing
- Warnings may be ignored
- Degraded performance may be confusing

**Mitigations**:
- Clear messaging about degraded mode
- `subcog status` shows component health
- Structured logging for monitoring

---

## ADR-006: Model Selection - all-MiniLM-L6-v2

### Status

**Accepted**

### Context

fastembed-rs supports multiple embedding models. Need to choose default model balancing quality, size, and performance.

Options considered:
| Model | Dimensions | Size | Quality (MTEB) |
|-------|------------|------|----------------|
| all-MiniLM-L6-v2 | 384 | 22MB | 0.63 |
| all-MiniLM-L12-v2 | 384 | 33MB | 0.65 |
| BGE-small-en-v1.5 | 384 | 33MB | 0.67 |
| BGE-base-en-v1.5 | 768 | 110MB | 0.73 |
| nomic-embed-text-v1 | 768 | 137MB | 0.74 |

### Decision

Use **all-MiniLM-L6-v2** as default.

### Rationale

| Factor | Weight | all-MiniLM-L6-v2 | BGE-base-en-v1.5 |
|--------|--------|------------------|------------------|
| Quality | 30% | Good (0.63) | Excellent (0.73) |
| Size | 25% | Small (22MB) | Large (110MB) |
| Speed | 25% | Fast (~30ms) | Slower (~60ms) |
| Memory | 20% | ~50MB | ~200MB |

For Subcog's use case (coding assistant memories), all-MiniLM-L6-v2 provides sufficient quality with significantly better size/speed characteristics.

### Consequences

**Positive**:
- Smaller binary and memory footprint
- Faster embedding generation
- Proven model with wide adoption

**Negative**:
- Slightly lower quality than larger models
- May miss subtle semantic relationships

**Mitigations**:
- Allow model override via config
- Consider model upgrade in future version
- Hybrid search (text + vector) compensates

---

## ADR-007: Vector Index Implementation - usearch

### Status

**Accepted**

### Context

Need a vector similarity search index for semantic search. The codebase already has `UsearchBackend` implemented but not connected.

Options considered:
1. **usearch** (existing) - HNSW index, Rust bindings
2. **hnswlib** - Original HNSW implementation
3. **faiss** - Facebook's similarity search
4. **pgvector** - PostgreSQL extension
5. **milvus** - Distributed vector DB
6. **brute force** - Linear scan

### Decision

Use **usearch** (already implemented).

### Rationale

usearch is already implemented in the codebase at `src/storage/vector/usearch.rs` with both native and fallback modes. The implementation supports:
- HNSW graph structure
- Cosine similarity
- Namespace filtering
- Persistence to disk

No reason to change - just need to wire it up.

### Consequences

**Positive**:
- No new implementation needed
- Battle-tested HNSW algorithm
- Fast search (<10ms for 10k vectors)

**Negative**:
- Single-node only (no distributed)
- Memory-mapped, needs disk space

**Mitigations**:
- PostgreSQL + pgvector available for scaling
- Can migrate index without data loss (rebuild from Git Notes)

---

## ADR-008: Backward Compatibility with Existing Memories

### Status

**Accepted**

### Context

Existing Subcog installations have memories in Git Notes without embeddings. Need to ensure:
1. Old memories remain accessible
2. New memories get embeddings
3. Migration path exists

Options considered:
1. **Breaking change** - Require all memories to have embeddings
2. **Backward compatible** - Support memories with/without embeddings
3. **Auto-migrate** - Automatically add embeddings on access
4. **Manual migrate** - Provide migration tool

### Decision

Use **backward compatible** with **manual migration** tool.

### Rationale

```rust
// Memory struct supports optional embedding
pub struct Memory {
 pub embedding: Option<Vec<f32>>, // None for old memories
 //...
}

// Search handles missing embeddings
fn search(&self, query: &str) -> Vec<SearchResult> {
 let text_results = self.text_search(query); // Always works
 let vector_results = self.vector_search(query); // Only for embedded
 self.fuse(text_results, vector_results)
}
```

| Approach | User Effort | Risk | Complexity |
|----------|-------------|------|------------|
| Breaking | High | High | Low |
| Backward compat | None | Low | Medium |
| Auto-migrate | None | Medium | High |
| Manual migrate | Low | Low | Medium |

### Consequences

**Positive**:
- Existing users not disrupted
- Gradual migration at user's pace
- Old memories still searchable via text

**Negative**:
- Old memories won't appear in vector search
- Need to maintain backward compatibility code

**Mitigations**:
- `subcog migrate embeddings` command
- Clear documentation
- Status command shows migration status

---

## Decision Log Summary

| ADR | Decision | Status |
|-----|----------|--------|
| ADR-001 | fastembed-rs for embeddings | Accepted |
| ADR-002 | Lazy load embedding model | Accepted |
| ADR-003 | Synchronous best-effort storage | Accepted |
| ADR-004 | Linear score normalization | Accepted |
| ADR-005 | Graceful degradation | Accepted |
| ADR-006 | all-MiniLM-L6-v2 model | Accepted |
| ADR-007 | usearch vector index | Accepted |
| ADR-008 | Backward compatibility | Accepted |
