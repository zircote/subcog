# Search Architecture

Subcog uses hybrid search combining semantic and keyword matching.

## Hybrid Search

Combines two search methods for best results:
1. **Vector Search** - Semantic similarity using embeddings
2. **Text Search** - BM25 keyword matching

### Architecture

```
Query
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│                     Query Processing                     │
│  ┌─────────────────────────────────────────────────────┐│
│  │  Query Text → Embedding → 384-dim vector            ││
│  │                                                      ││
│  │  Query Text → Tokenization → Keywords               ││
│  └─────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────┘
                    │
        ┌───────────┴───────────┐
        ▼                       ▼
┌───────────────┐       ┌───────────────┐
│ Vector Search │       │  Text Search  │
│   (usearch)   │       │ (SQLite FTS5) │
│               │       │               │
│ HNSW ANN      │       │ BM25 ranking  │
│ Cosine sim    │       │ Token match   │
└───────┬───────┘       └───────┬───────┘
        │                       │
        │   Vector Results      │   Text Results
        │   (id, distance)      │   (id, score)
        │                       │
        └───────────┬───────────┘
                    ▼
        ┌───────────────────────┐
        │    RRF Fusion         │
        │  (k=60 constant)      │
        │                       │
        │ score = Σ 1/(k+rank)  │
        └───────────┬───────────┘
                    │
                    ▼
            Fused Rankings
```

## Embedding Generation

### Model

- **Model**: all-MiniLM-L6-v2
- **Provider**: FastEmbed (Rust)
- **Dimensions**: 384
- **Max Tokens**: 256

### Process

```rust
pub struct Embedder {
    model: FastEmbed,
    cache: LruCache<String, Vec<f32>>,
}

impl Embedder {
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Check cache
        if let Some(cached) = self.cache.get(text) {
            return Ok(cached.clone());
        }

        // Generate embedding
        let embedding = self.model.embed(vec![text])?
            .into_iter()
            .next()
            .ok_or(Error::EmbeddingFailed)?;

        // Cache result
        self.cache.put(text.to_string(), embedding.clone());

        Ok(embedding)
    }
}
```

### Performance

| Operation | Latency |
|-----------|---------|
| Cache hit | <1ms |
| Cache miss | ~20ms |
| Batch (10) | ~50ms |

## Vector Search

### HNSW Index

usearch implementation of Hierarchical Navigable Small World graphs.

```rust
pub struct UsearchBackend {
    index: Index,
    id_map: HashMap<u64, MemoryId>,
}

impl VectorBackend for UsearchBackend {
    async fn search(
        &self,
        vector: &[f32],
        limit: usize
    ) -> Result<Vec<VectorResult>> {
        let results = self.index.search(vector, limit)?;

        results.iter().map(|r| {
            VectorResult {
                id: self.id_map.get(&r.key).cloned()?,
                distance: r.distance,
            }
        }).collect()
    }
}
```

### Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| M | 16 | Connections per layer |
| ef_construction | 128 | Build-time search width |
| ef_search | 64 | Query-time search width |
| Metric | Cosine | Similarity measure |

## Text Search

### BM25 Implementation

SQLite FTS5 with BM25 ranking.

```sql
CREATE VIRTUAL TABLE memories_fts USING fts5(
    id,
    content,
    tags,
    content='memories',
    content_rowid='rowid'
);

-- Search query
SELECT id, bm25(memories_fts) AS score
FROM memories_fts
WHERE memories_fts MATCH ?
ORDER BY score
LIMIT ?;
```

### Tokenization

- Standard SQLite FTS5 tokenizer
- Case-insensitive matching
- Prefix matching supported

## RRF Fusion

Reciprocal Rank Fusion combines rankings from different sources.

### Algorithm

```rust
fn rrf_fusion(
    vector_results: Vec<VectorResult>,
    text_results: Vec<TextResult>,
    k: u32,
) -> Vec<FusedResult> {
    let mut scores: HashMap<MemoryId, f32> = HashMap::new();

    // Add vector scores
    for (rank, result) in vector_results.iter().enumerate() {
        let score = 1.0 / (k as f32 + rank as f32 + 1.0);
        *scores.entry(result.id.clone()).or_default() += score;
    }

    // Add text scores
    for (rank, result) in text_results.iter().enumerate() {
        let score = 1.0 / (k as f32 + rank as f32 + 1.0);
        *scores.entry(result.id.clone()).or_default() += score;
    }

    // Sort by combined score
    let mut results: Vec<_> = scores.into_iter().collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    results.into_iter().map(|(id, score)| {
        FusedResult { id, score }
    }).collect()
}
```

### Why k=60?

The constant k=60 is a well-established default that:
- Balances contributions from each ranking
- Reduces impact of outlier rankings
- Works well for most use cases

## Search Modes

### Hybrid (Default)

Best for general search:
```rust
SearchMode::Hybrid => {
    let (vector, text) = tokio::join!(
        self.vector_search(query),
        self.text_search(query)
    );
    rrf_fusion(vector?, text?, 60)
}
```

### Vector Only

Best for conceptual/semantic search:
```rust
SearchMode::Vector => {
    let embedding = self.embedder.embed(&query.text)?;
    self.vector.search(&embedding, query.limit).await
}
```

### Text Only

Best for exact term matching:
```rust
SearchMode::Text => {
    self.index.search(&IndexQuery::from(query)).await
}
```

## Namespace Weighting

For search intent, namespaces are weighted:

```rust
fn apply_namespace_weights(
    results: &mut [SearchResult],
    weights: &HashMap<Namespace, f32>,
) {
    for result in results {
        if let Some(&weight) = weights.get(&result.namespace) {
            result.score *= weight;
        }
    }
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
}
```

### Intent Weights

| Intent | High Weight | Medium Weight |
|--------|-------------|---------------|
| HowTo | patterns, learnings | apis |
| Location | apis, config | context |
| Explanation | decisions, context | patterns |
| Troubleshoot | blockers, learnings | testing |

## Performance

| Operation | Target | Typical |
|-----------|--------|---------|
| Vector search | <30ms | ~20ms |
| Text search | <30ms | ~15ms |
| RRF fusion | <5ms | ~2ms |
| Total hybrid | <50ms | ~35ms |

## Caching

### Query Cache

Recently executed queries are cached:
```rust
cache: LruCache<QueryHash, Vec<SearchResult>>
```

### Embedding Cache

Embeddings are cached to avoid recomputation.

## See Also

- [RecallService](services.md#recallservice) - Service implementation
- [Vector Layer](../storage/vector.md) - Vector storage
- [Index Layer](../storage/index.md) - Text search storage
