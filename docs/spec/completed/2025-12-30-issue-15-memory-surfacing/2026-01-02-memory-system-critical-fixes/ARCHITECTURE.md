# Architecture: Subcog Memory System Critical Fixes

## Document Control

| Field | Value |
|-------|-------|
| Version | 1.0 |
| Status | Draft |
| Last Updated | 2026-01-02 |
| Author | Claude (Architect-Reviewer) |

## 1. Overview

This document describes the technical architecture for fixing the 5 critical gaps in Subcog's memory system. The fixes integrate real semantic embeddings, wire up the three-layer storage architecture, and ensure memories are immediately searchable after capture.

### 1.1 Current Architecture (Broken)

```
┌─────────────────────────────────────────────────────────────────┐
│ CaptureService │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ capture() -> Memory { embedding: None } -> Git Notes ONLY │ │
│ └─────────────────────────────────────────────────────────┘ │
│ │
│ No Index Update │
│ No Vector Upsert │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ RecallService │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Fields: index: Option<SqliteBackend> │ │
│ │ Missing: embedder │ │
│ │ Missing: vector │ │
│ └─────────────────────────────────────────────────────────┘ │
│ │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ vector_search() -> Ok(Vec::new()) // ALWAYS EMPTY │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ FastEmbedEmbedder │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ pseudo_embed() -> hash-based, NOT SEMANTIC │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 Target Architecture (Fixed)

```
┌─────────────────────────────────────────────────────────────────┐
│ CaptureService │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ embedder: Arc<dyn Embedder> │ │
│ │ persistence: Arc<dyn PersistenceBackend> │ │
│ │ index: Arc<dyn IndexBackend> │ │
│ │ vector: Arc<dyn VectorBackend> │ │
│ └─────────────────────────────────────────────────────────┘ │
│ │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ 1. Generate embedding via embedder │ │
│ │ 2. Store to Git Notes (persistence) │ │
│ │ 3. Index in SQLite FTS5 (index) │ │
│ │ 4. Upsert to usearch (vector) │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ RecallService │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ embedder: Arc<dyn Embedder> │ │
│ │ index: Arc<dyn IndexBackend> │ │
│ │ vector: Arc<dyn VectorBackend> │ │
│ └─────────────────────────────────────────────────────────┘ │
│ │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ 1. Embed query via embedder │ │
│ │ 2. Text search via index (BM25) │ │
│ │ 3. Vector search via vector (cosine) │ │
│ │ 4. Fuse results via RRF │ │
│ │ 5. Normalize scores to 0.0-1.0 │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ FastEmbedEmbedder │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ model: OnceLock<TextEmbedding> // Lazy loaded │ │
│ │ embed() -> real semantic vectors via all-MiniLM-L6-v2 │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## 2. Component Design

### 2.1 FastEmbedEmbedder (Real Implementation)

**File**: `src/embedding/fastembed.rs`

```rust
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::OnceLock;

/// Thread-safe singleton for embedding model.
static EMBEDDING_MODEL: OnceLock<TextEmbedding> = OnceLock::new();

/// FastEmbed-based embedder using all-MiniLM-L6-v2.
pub struct FastEmbedEmbedder {
 /// Model name for logging/debugging.
 model_name: String,
}

impl FastEmbedEmbedder {
 /// Embedding dimensions for all-MiniLM-L6-v2.
 pub const DIMENSIONS: usize = 384;

 /// Creates a new FastEmbed embedder.
 ///
 /// Note: Model is lazily loaded on first embed() call.
 #[must_use]
 pub fn new() -> Self {
 Self {
 model_name: "all-MiniLM-L6-v2".to_string(),
 }
 }

 /// Gets or initializes the embedding model (thread-safe).
 fn get_model() -> Result<&'static TextEmbedding, Error> {
 EMBEDDING_MODEL.get_or_try_init(|| {
 tracing::info!("Loading embedding model (first use)...");
 let start = std::time::Instant::now();

 let model = TextEmbedding::try_new(InitOptions {
 model_name: EmbeddingModel::AllMiniLML6V2,
 show_download_progress: false,
..Default::default()
 })?;

 tracing::info!(
 elapsed_ms = start.elapsed().as_millis(),
 "Embedding model loaded"
 );
 Ok(model)
 })
 }
}

impl Embedder for FastEmbedEmbedder {
 fn dimensions(&self) -> usize {
 Self::DIMENSIONS
 }

 fn embed(&self, text: &str) -> Result<Vec<f32>> {
 if text.is_empty() {
 return Err(Error::InvalidInput("Cannot embed empty text".into()));
 }

 let model = Self::get_model()?;
 let embeddings = model.embed(vec![text], None)?;

 embeddings
.into_iter()
.next()
.ok_or_else(|| Error::OperationFailed {
 operation: "embed".into(),
 cause: "No embedding returned".into(),
 })
 }

 fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
 if texts.is_empty() {
 return Ok(Vec::new());
 }

 if texts.iter().any(|t| t.is_empty()) {
 return Err(Error::InvalidInput("Cannot embed empty text".into()));
 }

 let model = Self::get_model()?;
 let texts_owned: Vec<String> = texts.iter().map(|s| (*s).to_string()).collect();
 model.embed(texts_owned, None).map_err(Into::into)
 }
}
```

**Key Design Decisions**:

1. **Lazy Loading**: Model loaded on first `embed()` call, not at construction
2. **Thread-Safe Singleton**: `OnceLock` ensures model is initialized exactly once
3. **Owned Strings**: fastembed-rs requires `Vec<String>`, so we convert
4. **Error Handling**: Proper `Result` types, no panics

### 2.2 RecallService (With Embedder + Vector)

**File**: `src/services/recall.rs`

```rust
use std::sync::Arc;

/// Service for recalling memories via hybrid search.
pub struct RecallService {
 /// Text search backend (SQLite FTS5 or PostgreSQL).
 index: Option<Arc<dyn IndexBackend>>,

 /// Embedding generator for query embedding.
 embedder: Option<Arc<dyn Embedder>>,

 /// Vector search backend (usearch or pgvector).
 vector: Option<Arc<dyn VectorBackend>>,
}

impl RecallService {
 /// Creates a new RecallService with all backends.
 pub fn new(
 index: Option<Arc<dyn IndexBackend>>,
 embedder: Option<Arc<dyn Embedder>>,
 vector: Option<Arc<dyn VectorBackend>>,
 ) -> Self {
 Self { index, embedder, vector }
 }

 /// Performs hybrid search combining text and vector results.
 pub async fn search(
 &self,
 query: &str,
 filter: &SearchFilter,
 limit: usize,
 ) -> Result<Vec<SearchResult>> {
 // 1. Text search (BM25)
 let text_results = self.text_search(query, filter, limit).await?;

 // 2. Vector search (cosine similarity)
 let vector_results = self.vector_search(query, filter, limit).await?;

 // 3. Fuse with RRF
 let fused = self.rrf_fuse(&text_results, &vector_results);

 // 4. Normalize scores
 let normalized = self.normalize_scores(fused);

 Ok(normalized.into_iter().take(limit).collect())
 }

 /// Vector search using embedder + vector backend.
 async fn vector_search(
 &self,
 query: &str,
 filter: &SearchFilter,
 limit: usize,
 ) -> Result<Vec<SearchHit>> {
 // Check if embedder and vector are available
 let (embedder, vector) = match (&self.embedder, &self.vector) {
 (Some(e), Some(v)) => (e, v),
 _ => {
 tracing::debug!("Vector search unavailable, falling back to text-only");
 return Ok(Vec::new());
 }
 };

 // Generate query embedding
 let query_embedding = embedder.embed(query)?;

 // Search vector index
 let results = vector.search(&query_embedding, limit)?;

 // Apply namespace filter if specified
 let filtered = if let Some(ns) = &filter.namespace {
 results
.into_iter()
.filter(|r| &r.namespace == ns)
.collect()
 } else {
 results
 };

 Ok(filtered)
 }

 /// Normalizes RRF scores to 0.0-1.0 range.
 fn normalize_scores(&self, results: Vec<SearchHit>) -> Vec<SearchResult> {
 if results.is_empty() {
 return Vec::new();
 }

 // Find max score for normalization
 let max_score = results
.iter()
.map(|r| r.score)
.fold(f32::NEG_INFINITY, f32::max);

 results
.into_iter()
.map(|hit| SearchResult {
 memory_id: hit.memory_id,
 score: if max_score > 0.0 {
 hit.score / max_score
 } else {
 0.0
 },
 raw_score: hit.score,
 //... other fields
 })
.collect()
 }
}
```

**Key Design Decisions**:

1. **Optional Backends**: All backends are `Option<Arc<dyn...>>` for graceful degradation
2. **Async Methods**: Search is async to support network backends (PostgreSQL, Redis)
3. **Score Normalization**: Raw RRF scores mapped to intuitive 0.0-1.0 range
4. **Namespace Filtering**: Applied post-search to vector results

### 2.3 CaptureService (With All Three Layers)

**File**: `src/services/capture.rs`

```rust
/// Service for capturing memories to all storage layers.
pub struct CaptureService {
 /// Embedding generator.
 embedder: Arc<dyn Embedder>,

 /// Persistence layer (Git Notes).
 persistence: Arc<dyn PersistenceBackend>,

 /// Index layer (SQLite FTS5).
 index: Arc<dyn IndexBackend>,

 /// Vector layer (usearch).
 vector: Arc<dyn VectorBackend>,
}

impl CaptureService {
 /// Captures a memory to all storage layers.
 pub async fn capture(&self, request: CaptureRequest) -> Result<CaptureResult> {
 // 1. Generate embedding
 let embedding = self.embedder.embed(&request.content)?;

 // 2. Create memory with embedding
 let memory = Memory {
 id: MemoryId::new(),
 namespace: request.namespace,
 content: request.content.clone(),
 embedding: Some(embedding.clone()),
 created_at: Utc::now(),
 //... other fields
 };

 // 3. Store to Git Notes (authoritative)
 self.persistence.store(&memory).await?;

 // 4. Index in SQLite FTS5
 if let Err(e) = self.index.index(&memory).await {
 tracing::warn!(error = %e, "Failed to index memory, continuing");
 // Don't fail - Git Notes is authoritative
 }

 // 5. Upsert to vector store
 if let Err(e) = self.vector.upsert(&memory.id, &embedding).await {
 tracing::warn!(error = %e, "Failed to upsert vector, continuing");
 // Don't fail - Git Notes is authoritative
 }

 Ok(CaptureResult {
 memory_id: memory.id.to_string(),
 urn: format!("subcog://{}/{}/{}",
 request.domain.unwrap_or_default(),
 request.namespace,
 memory.id
 ),
 })
 }
}
```

**Key Design Decisions**:

1. **Embedding First**: Generate embedding before any storage to fail fast
2. **Git Notes Authoritative**: Persistence layer is the source of truth
3. **Best-Effort Secondary Layers**: Index and vector failures logged but don't fail capture
4. **Sync Later**: If index/vector fail, can be rebuilt from Git Notes

### 2.4 Score Normalization Strategy

**Current RRF Formula** (K=60):
```
score = 1 / (K + rank)
```

With rank starting at 1:
- Rank 1: 1/61 ≈ 0.0164
- Rank 2: 1/62 ≈ 0.0161
- Rank 10: 1/70 ≈ 0.0143

**Normalized Formula**:
```
normalized_score = raw_score / max_raw_score
```

Or alternatively, map to intuitive range:
```
// If both text and vector return rank-1 match:
// Combined RRF = 1/61 + 1/61 = 0.0328
// Normalized to 0.0-1.0: 0.0328 / 0.0328 = 1.0

// If only text returns rank-1:
// Combined RRF = 1/61 + 0 = 0.0164
// Normalized: 0.0164 / 0.0328 = 0.5
```

### 2.5 Graceful Degradation Matrix

| Embedder | Vector | Index | Behavior |
|----------|--------|-------|----------|
| | | | Full hybrid search |
| | | | Vector-only search |
| | | | Text-only search (BM25) |
| | | | Text-only search (BM25) |
| | | | Error: no search available |

## 3. Data Flow

### 3.1 Capture Flow

```
User: subcog capture --namespace decisions "Use PostgreSQL"
 
 CaptureService.capture()
 
 ┌─────────────────┴─────────────────┐
 
 1. embedder.embed("Use PostgreSQL") 2. Create Memory
 
 Vec<f32> [0.12, -0.34,...] Memory { embedding: Some(...) }
 
 ┌─────────────────┼─────────────────┐
 
 3. persistence.store() 4. index.index() 5. vector.upsert()
 
 Git Notes SQLite FTS5 usearch HNSW
 
 CaptureResult { urn }
```

### 3.2 Recall Flow

```
User: subcog recall "database storage"
 
 RecallService.search()
 
 ┌─────────────────┴─────────────────┐
 
 1. embedder.embed("database storage") 2. index.search()
 
 query_embedding text_results (BM25)
 
 3. vector.search(query_embedding)
 
 vector_results (cosine)
 
 4. rrf_fuse()
 
 5. normalize_scores()
 
 Vec<SearchResult> { score: 0.85,... }
```

## 4. Integration Points

### 4.1 ServiceContainer Changes

**File**: `src/services/mod.rs`

```rust
pub struct ServiceContainer {
 embedder: Arc<dyn Embedder>,
 persistence: Arc<dyn PersistenceBackend>,
 index: Arc<dyn IndexBackend>,
 vector: Arc<dyn VectorBackend>,
 //... existing fields
}

impl ServiceContainer {
 pub fn new(config: &Config) -> Result<Self> {
 // Initialize embedder (lazy loading inside)
 let embedder: Arc<dyn Embedder> = Arc::new(FastEmbedEmbedder::new());

 // Initialize persistence (Git Notes)
 let persistence: Arc<dyn PersistenceBackend> =
 Arc::new(GitNotesBackend::new(&config.repo_path)?);

 // Initialize index (SQLite)
 let index: Arc<dyn IndexBackend> =
 Arc::new(SqliteBackend::new(&config.db_path)?);

 // Initialize vector (usearch)
 let vector: Arc<dyn VectorBackend> =
 Arc::new(UsearchBackend::new(&config.index_path, 384)?);

 Ok(Self {
 embedder,
 persistence,
 index,
 vector,
 //...
 })
 }

 pub fn capture_service(&self) -> CaptureService {
 CaptureService::new(
 Arc::clone(&self.embedder),
 Arc::clone(&self.persistence),
 Arc::clone(&self.index),
 Arc::clone(&self.vector),
 )
 }

 pub fn recall_service(&self) -> RecallService {
 RecallService::new(
 Some(Arc::clone(&self.index)),
 Some(Arc::clone(&self.embedder)),
 Some(Arc::clone(&self.vector)),
 )
 }
}
```

### 4.2 Feature Flags

**File**: `src/config/features.rs`

```rust
pub struct FeatureFlags {
 /// Enable real embeddings (vs placeholder).
 pub real_embeddings: bool,

 /// Enable vector search.
 pub vector_search: bool,

 /// Enable score normalization.
 pub normalize_scores: bool,
}

impl Default for FeatureFlags {
 fn default() -> Self {
 Self {
 real_embeddings: true, // ON by default after fix
 vector_search: true, // ON by default after fix
 normalize_scores: true, // ON by default after fix
 }
 }
}
```

## 5. Migration Strategy

### 5.1 Existing Memories Without Embeddings

For memories captured before this fix (no embeddings in Git Notes):

```rust
/// Migration: Add embeddings to existing memories.
pub async fn migrate_embeddings(
 embedder: &dyn Embedder,
 persistence: &dyn PersistenceBackend,
 vector: &dyn VectorBackend,
) -> Result<MigrationStats> {
 let memories = persistence.list_all().await?;
 let mut stats = MigrationStats::default();

 for memory in memories {
 if memory.embedding.is_some() {
 stats.skipped += 1;
 continue;
 }

 // Generate embedding
 match embedder.embed(&memory.content) {
 Ok(embedding) => {
 // Update Git Notes with embedding
 let mut updated = memory.clone();
 updated.embedding = Some(embedding.clone());
 persistence.update(&updated).await?;

 // Upsert to vector store
 vector.upsert(&memory.id, &embedding).await?;

 stats.migrated += 1;
 }
 Err(e) => {
 tracing::warn!(id = %memory.id, error = %e, "Failed to embed");
 stats.failed += 1;
 }
 }
 }

 Ok(stats)
}
```

### 5.2 CLI Command

```bash
# Migrate existing memories to add embeddings
subcog migrate embeddings

# Dry run (show what would be migrated)
subcog migrate embeddings --dry-run

# Force re-embed all (even if embedding exists)
subcog migrate embeddings --force
```

## 6. Testing Strategy

### 6.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn test_fastembed_dimensions() {
 let embedder = FastEmbedEmbedder::new();
 assert_eq!(embedder.dimensions(), 384);
 }

 #[test]
 fn test_fastembed_embed() {
 let embedder = FastEmbedEmbedder::new();
 let result = embedder.embed("Hello, world!");

 assert!(result.is_ok());
 let embedding = result.unwrap();
 assert_eq!(embedding.len(), 384);
 }

 #[test]
 fn test_fastembed_semantic_similarity() {
 let embedder = FastEmbedEmbedder::new();

 let emb1 = embedder.embed("database storage").unwrap();
 let emb2 = embedder.embed("PostgreSQL database").unwrap();
 let emb3 = embedder.embed("cat dog pet").unwrap();

 let sim_related = cosine_similarity(&emb1, &emb2);
 let sim_unrelated = cosine_similarity(&emb1, &emb3);

 assert!(sim_related > sim_unrelated,
 "Related text should be more similar");
 assert!(sim_related > 0.5,
 "Related text should have high similarity");
 }

 #[test]
 fn test_score_normalization() {
 let service = RecallService::new(None, None, None);

 let results = vec![
 SearchHit { score: 0.0328,.. },
 SearchHit { score: 0.0164,.. },
 ];

 let normalized = service.normalize_scores(results);

 assert!((normalized[0].score - 1.0).abs() < 0.01);
 assert!((normalized[1].score - 0.5).abs() < 0.01);
 }
}
```

### 6.2 Integration Tests

```rust
#[tokio::test]
async fn test_capture_recall_roundtrip() {
 let container = ServiceContainer::new(&test_config()).unwrap();
 let capture = container.capture_service();
 let recall = container.recall_service();

 // Capture
 let result = capture.capture(CaptureRequest {
 namespace: Namespace::Decisions,
 content: "Use PostgreSQL for primary storage".into(),
..Default::default()
 }).await.unwrap();

 // Recall
 let results = recall.search(
 "database storage decision",
 &SearchFilter::default(),
 10,
 ).await.unwrap();

 assert!(!results.is_empty(), "Should find the captured memory");
 assert!(results[0].score > 0.5, "Score should be high for semantic match");
}
```

### 6.3 Property Tests

```rust
use proptest::prelude::*;

proptest! {
 #[test]
 fn prop_normalized_scores_in_range(scores in prop::collection::vec(0.0f32..1.0f32, 1..100)) {
 let service = RecallService::new(None, None, None);
 let hits: Vec<_> = scores.iter().map(|&s| SearchHit { score: s,.. }).collect();

 let normalized = service.normalize_scores(hits);

 for result in normalized {
 prop_assert!(result.score >= 0.0);
 prop_assert!(result.score <= 1.0);
 }
 }
}
```

## 7. Performance Considerations

### 7.1 Model Loading

The all-MiniLM-L6-v2 model is ~22MB (quantized). Loading takes 1-2 seconds.

**Mitigation**: Lazy loading via `OnceLock` - only loaded on first embed call.

### 7.2 Memory Footprint

| State | Memory |
|-------|--------|
| Idle (no model) | ~20MB |
| Model loaded | ~70MB |
| 10k vectors in usearch | ~15MB |
| **Total (typical)** | **~105MB** |

### 7.3 Latency Budget

| Operation | Budget | Typical |
|-----------|--------|---------|
| First embed (cold) | 2000ms | ~1500ms |
| Subsequent embed | 50ms | ~30ms |
| Vector search (10k items) | 20ms | ~5ms |
| Text search (10k items) | 30ms | ~10ms |
| RRF fusion | 5ms | ~1ms |
| **Total search (warm)** | **100ms** | **~50ms** |

## 8. Security Considerations

### 8.1 Model Files

- Downloaded from Hugging Face on first use
- Stored in `~/.cache/huggingface/` (default)
- Configurable via `HF_HOME` environment variable
- No sensitive data in model files

### 8.2 Embedding Data

- Embeddings are dense vectors, not reversible to original text
- Stored in Git Notes (version controlled)
- No PII in embeddings themselves

## 9. Appendix

### A. fastembed-rs Integration Details

**Cargo.toml**:
```toml
[dependencies]
fastembed = { version = "4", default-features = false, features = ["ort"] }
```

**Model Options**:
| Model | Dimensions | Size | Quality |
|-------|------------|------|---------|
| all-MiniLM-L6-v2 | 384 | 22MB | Good |
| all-MiniLM-L12-v2 | 384 | 33MB | Better |
| BGE-small-en | 384 | 33MB | Best (small) |

### B. usearch Integration Details

**Cargo.toml**:
```toml
[dependencies]
usearch = { version = "2", optional = true }

[features]
default = ["usearch"]
```

**Index Parameters**:
```rust
let index = usearch::Index::new(&usearch::IndexOptions {
 dimensions: 384,
 metric: usearch::MetricKind::Cos, // Cosine similarity
 quantization: usearch::ScalarKind::F32,
 connectivity: 16, // M parameter for HNSW
 expansion_add: 128, // efConstruction
 expansion_search: 64, // ef
})?;
```
