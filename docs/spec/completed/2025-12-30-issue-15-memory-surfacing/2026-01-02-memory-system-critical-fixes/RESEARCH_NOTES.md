# Research Notes: Subcog Memory System Critical Fixes

## Document Control

| Field | Value |
|-------|-------|
| Version | 1.0 |
| Status | Draft |
| Last Updated | 2026-01-02 |
| Author | Claude (Architect-Reviewer) |

---

## 1. Investigation Summary

This document captures findings from the deep investigation into why Subcog's memory capture/recall system is not functioning correctly.

### Initial Symptoms

1. Memories saved but recall returns nothing
2. Relevance scores extremely low (0.01-0.02 instead of 0.5-0.9)

### Root Causes Identified

| Issue | Severity | File | Line(s) | Impact |
|-------|----------|------|---------|--------|
| Placeholder embeddings | CRITICAL | `src/embedding/fastembed.rs` | 46-74 | No semantic search |
| Vector search stub | CRITICAL | `src/services/recall.rs` | 241-250 | Hybrid = text-only |
| Storage not synchronized | HIGH | `src/services/capture.rs` | 119-130 | Memories not searchable |
| RecallService missing backends | HIGH | `src/services/recall.rs` | 19-22 | Cannot implement vector |
| RRF low scores | MEDIUM | `src/services/recall.rs` | 310-365 | Confusing UX |

---

## 2. Embedding Infrastructure Analysis

### 2.1 Embedder Trait

**Location**: `src/embedding/mod.rs`

```rust
pub trait Embedder: Send + Sync {
    fn dimensions(&self) -> usize;
    fn embed(&self, text: &str) -> Result<Vec<f32>>;
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
}
```

**Assessment**: Trait is well-designed with proper bounds (`Send + Sync`). No changes needed.

### 2.2 FastEmbedEmbedder Implementation

**Location**: `src/embedding/fastembed.rs`

```rust
// Line 10-12: Explicit placeholder warning
/// Note: This is a placeholder implementation that generates deterministic
/// pseudo-embeddings based on content hashing. For production use, integrate
/// the actual `fastembed-rs` crate.

// Line 46-74: Hash-based pseudo-embedding
fn pseudo_embed(&self, text: &str) -> Vec<f32> {
    let mut embedding = vec![0.0f32; self.dimensions];
    for (i, word) in text.split_whitespace().enumerate() {
        let mut hasher = DefaultHasher::new();
        word.hash(&mut hasher);
        let hash = hasher.finish();
        // Distributes hash across embedding dimensions
        for j in 0..8 {
            let idx = ((hash >> (j * 8)) as usize + i) % self.dimensions;
            let value = ((hash >> (j * 4)) & 0xFF) as f32 / 255.0 - 0.5;
            embedding[idx] += value;
        }
    }
    // Normalizes to unit vector
}
```

**Assessment**: This is explicitly a placeholder. The implementation:
- Uses `DefaultHasher` which is deterministic but not semantic
- Distributes hash values pseudo-randomly across dimensions
- Produces normalized unit vectors (magnitude ~1.0)
- **Cannot** capture semantic similarity

### 2.3 Fallback Embedder

**Location**: `src/embedding/fallback.rs`

```rust
pub struct FallbackEmbedder {
    dimensions: usize,
}

impl Embedder for FallbackEmbedder {
    fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        // Returns zero vector
        Ok(vec![0.0; self.dimensions])
    }
}
```

**Assessment**: Intentional fallback for when no embedder is available. Returns zero vectors which results in 0.0 cosine similarity with everything.

---

## 3. Vector Backend Analysis

### 3.1 VectorBackend Trait

**Location**: `src/storage/traits/vector.rs`

```rust
pub trait VectorBackend: Send + Sync {
    fn upsert(&self, id: &MemoryId, embedding: &[f32]) -> Result<()>;
    fn remove(&self, id: &MemoryId) -> Result<()>;
    fn search(&self, query: &[f32], limit: usize) -> Result<Vec<VectorHit>>;
    fn count(&self) -> Result<usize>;
    fn clear(&self) -> Result<()>;
}
```

**Assessment**: Trait is well-designed. All necessary methods present.

### 3.2 UsearchBackend Implementation

**Location**: `src/storage/vector/usearch.rs`

Two implementations exist:

1. **Native HNSW** (feature = "usearch"):
```rust
#[cfg(feature = "usearch")]
pub struct UsearchBackend {
    index: usearch::Index,
    id_map: DashMap<String, usize>,
    reverse_map: DashMap<usize, String>,
}
```

2. **Fallback Brute-Force** (no feature):
```rust
#[cfg(not(feature = "usearch"))]
pub struct UsearchBackend {
    vectors: DashMap<String, Vec<f32>>,
    dimensions: usize,
}
```

**Assessment**: Both implementations are functional. The native version uses HNSW for O(log n) search, fallback uses O(n) brute force. Neither is wired to RecallService.

---

## 4. Capture Service Analysis

### 4.1 Current Capture Flow

**Location**: `src/services/capture.rs`

```rust
// Line 119-130: Memory created without embedding
let memory = Memory {
    id: MemoryId::new(),
    namespace: request.namespace.clone(),
    content: request.content.clone(),
    embedding: None,  // HARDCODED TO NONE
    tags: request.tags.clone(),
    source: request.source.clone(),
    created_at: Utc::now(),
    updated_at: Utc::now(),
    status: MemoryStatus::Active,
    metadata: request.metadata.clone().unwrap_or_default(),
};

// After this: ONLY writes to Git Notes
// MISSING: index.index(&memory)
// MISSING: vector.upsert(&memory.id, &embedding)
```

**Assessment**:
- Embedding is never generated
- Index backend never called
- Vector backend never called
- Only Git Notes receives the memory

### 4.2 CaptureService Fields

```rust
pub struct CaptureService {
    persistence: Arc<dyn PersistenceBackend>,
    // MISSING: embedder
    // MISSING: index
    // MISSING: vector
}
```

**Assessment**: Service lacks necessary backends for full capture pipeline.

---

## 5. Recall Service Analysis

### 5.1 RecallService Fields

**Location**: `src/services/recall.rs:19-35`

```rust
pub struct RecallService {
    index: Option<SqliteBackend>,
    // MISSING: embedder: Option<Arc<dyn Embedder>>
    // MISSING: vector: Option<Arc<dyn VectorBackend>>
}
```

**Assessment**: Cannot perform vector search without embedder and vector backend.

### 5.2 Vector Search Implementation

**Location**: `src/services/recall.rs:241-250`

```rust
const fn vector_search(
    &self,
    _query: &str,
    _filter: &SearchFilter,
    _limit: usize,
) -> Result<Vec<SearchHit>> {
    // TODO: Implement vector search using embedder + vector backend
    Ok(Vec::new())  // ALWAYS RETURNS EMPTY
}
```

**Assessment**: This is a stub that always returns empty. The `const fn` qualifier indicates no runtime behavior is even possible.

### 5.3 RRF Implementation

**Location**: `src/services/recall.rs:310-365`

```rust
const K: f32 = 60.0;  // Standard RRF constant

fn rrf_score(rank: usize) -> f32 {
    1.0 / (K + rank as f32)
}
```

With K=60:
- Rank 1: 1/(60+1) = 0.0164
- Rank 2: 1/(60+2) = 0.0161
- Rank 10: 1/(60+10) = 0.0143

When vector search returns empty, only text scores contribute, roughly halving final scores.

**Assessment**: The math is correct per RRF paper, but scores appear "broken" to users expecting 0.0-1.0 range.

---

## 6. Test Infrastructure Analysis

### 6.1 Test Statistics

| Category | Count | Files |
|----------|-------|-------|
| Total tests | 817 | - |
| Recall service | 5 | `src/services/recall.rs` |
| Embedding | 24 | `src/embedding/fastembed.rs` |
| Vector backend | 12 | `src/storage/vector/usearch.rs` |
| Capture service | 8 | `src/services/capture.rs` |

### 6.2 Recall Service Test Gaps

Current tests only verify:
1. Empty query handling
2. Filter construction
3. Result limit
4. Basic text search

Missing tests:
- Vector search with real embeddings
- Hybrid search (text + vector)
- Score normalization
- Semantic similarity

### 6.3 Embedding Tests

All 24 tests pass because they test the placeholder implementation:
- Determinism (same input → same output) ✓
- Normalization (unit vectors) ✓
- Dimensions (384) ✓

**But**: No tests verify semantic similarity because placeholder can't provide it.

---

## 7. fastembed-rs Integration Research

### 7.1 Crate Information

- **Crate**: `fastembed`
- **Version**: 4.x
- **Repository**: https://github.com/Anush008/fastembed-rs
- **License**: Apache-2.0

### 7.2 Supported Models

| Model | Dimensions | Size | MTEB Score |
|-------|------------|------|------------|
| all-MiniLM-L6-v2 | 384 | 22MB | 0.63 |
| all-MiniLM-L12-v2 | 384 | 33MB | 0.65 |
| BGE-small-en-v1.5 | 384 | 33MB | 0.67 |
| BGE-base-en-v1.5 | 768 | 110MB | 0.73 |

### 7.3 Integration Example

```rust
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

// Initialize (lazy recommended)
let model = TextEmbedding::try_new(InitOptions {
    model_name: EmbeddingModel::AllMiniLML6V2,
    show_download_progress: false,
    ..Default::default()
})?;

// Embed single text
let embeddings = model.embed(vec!["Hello, world!"], None)?;

// Embed batch
let embeddings = model.embed(vec!["Text 1", "Text 2"], None)?;
```

### 7.4 Performance Characteristics

| Metric | Value |
|--------|-------|
| Model load time | 1-2 seconds |
| Embed latency (single) | ~30ms |
| Embed latency (batch 10) | ~50ms |
| Memory (model loaded) | ~50MB |

### 7.5 Thread Safety

```rust
// Recommended: OnceLock for thread-safe singleton
use std::sync::OnceLock;

static MODEL: OnceLock<TextEmbedding> = OnceLock::new();

fn get_model() -> &'static TextEmbedding {
    MODEL.get_or_init(|| {
        TextEmbedding::try_new(Default::default()).unwrap()
    })
}
```

### 7.6 Cargo.toml Integration

```toml
[dependencies]
fastembed = { version = "4", default-features = false, features = ["ort"] }

# Optional: enable CUDA for GPU acceleration
# fastembed = { version = "4", features = ["cuda"] }
```

---

## 8. Cosine Similarity Verification

To verify embeddings work correctly, we need semantic similarity tests:

```rust
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (norm_a * norm_b)
}

#[test]
fn test_semantic_similarity() {
    let embedder = FastEmbedEmbedder::new();

    let db = embedder.embed("database storage").unwrap();
    let pg = embedder.embed("PostgreSQL database").unwrap();
    let cat = embedder.embed("cat dog pet").unwrap();

    // Related concepts should be similar
    assert!(cosine_similarity(&db, &pg) > 0.5);

    // Unrelated concepts should be dissimilar
    assert!(cosine_similarity(&db, &cat) < 0.3);

    // Related > unrelated
    assert!(cosine_similarity(&db, &pg) > cosine_similarity(&db, &cat));
}
```

---

## 9. Storage Layer Synchronization

### 9.1 Current State

```
Capture → Git Notes (only)
         ↓
         Memory stored, but:
         - Not indexed in SQLite FTS5
         - Not upserted to usearch
         ↓
Recall → Searches Git Notes (slow, no vector)
```

### 9.2 Target State

```
Capture → Generate embedding
         → Git Notes (authoritative)
         → SQLite FTS5 (text search)
         → usearch (vector search)
         ↓
Recall → SQLite FTS5 (BM25 results)
       → usearch (cosine results)
       → RRF fusion
       → Normalized scores
```

---

## 10. Benchmark Targets

Based on research and project requirements:

| Metric | Target | Measurement |
|--------|--------|-------------|
| Cold start | <10ms | Binary load to main() |
| Model load | <2s | First embed call |
| Warm embed | <50ms | Subsequent embeds |
| Capture (with embed) | <100ms | End-to-end |
| Search (10k items) | <100ms | End-to-end |
| Binary size | <150MB | Release build |
| Memory (idle) | <50MB | No model loaded |
| Memory (active) | <150MB | Model + 10k vectors |

---

## 11. Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| fastembed-rs API changes | Low | Medium | Pin version, test upgrades |
| ONNX runtime conflicts | Low | High | Isolate in feature flag |
| Model download failures | Medium | Medium | Retry logic, offline fallback |
| Binary size bloat | Medium | Medium | Strip symbols, optimize |
| Performance regression | Low | Medium | Benchmarks in CI |
| Breaking changes to data | Low | Critical | Migration tool, compat layer |

---

## 12. References

### Documentation

- fastembed-rs: https://github.com/Anush008/fastembed-rs
- usearch: https://github.com/unum-cloud/usearch
- Hugging Face Models: https://huggingface.co/sentence-transformers
- RRF Paper: https://plg.uwaterloo.ca/~gvcormac/cormacksigir09-rrf.pdf

### Related Subcog Specs

- Parent: [2025-12-28-subcog-rust-rewrite](../2025-12-28-subcog-rust-rewrite/)
- Deduplication: [2026-01-01-pre-compact-deduplication](../../completed/2026-01-01-pre-compact-deduplication/)
- Prompt Management: [2025-12-30-prompt-management](../../completed/2025-12-30-prompt-management/)

### Model Cards

- all-MiniLM-L6-v2: https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2
- BGE-small-en-v1.5: https://huggingface.co/BAAI/bge-small-en-v1.5
