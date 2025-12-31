# Vector Layer

The vector layer provides semantic similarity search using embeddings.

## Backends

| Backend | Description | Use Case |
|---------|-------------|----------|
| usearch | Embedded HNSW | Default, local development |
| pgvector | PostgreSQL extension | Enterprise, shared access |
| Redis | Redis vector search | High-performance, distributed |

## usearch (Default)

Embedded high-performance vector index using HNSW algorithm.

### Configuration

```yaml
storage:
  vector: usearch
  vector_path: ~/.subcog/vectors.usearch
```

### Parameters

```yaml
storage:
  # Embedding model
  embedding_model: all-MiniLM-L6-v2
  embedding_dimensions: 384

  # HNSW parameters
  usearch_connectivity: 16  # M parameter
  usearch_expansion_add: 128
  usearch_expansion_search: 64
```

### How It Works

1. Content is embedded using FastEmbed (all-MiniLM-L6-v2)
2. 384-dimensional vector is stored in HNSW index
3. Approximate nearest neighbor search for queries

### Advantages

- Zero external dependencies
- Very fast search (<10ms)
- Low memory footprint
- Embedded in process

### Limitations

- Single-process access
- Local only
- File-based persistence

---

## pgvector

PostgreSQL extension for vector similarity search.

### Configuration

```yaml
storage:
  vector: pgvector

postgresql:
  host: localhost
  port: 5432
  database: subcog
```

### Setup

```sql
-- Enable extension
CREATE EXTENSION vector;

-- Schema
CREATE TABLE memory_vectors (
    id UUID PRIMARY KEY,
    embedding vector(384) NOT NULL,
    FOREIGN KEY (id) REFERENCES memories(id)
);

-- HNSW index
CREATE INDEX ON memory_vectors
USING hnsw (embedding vector_cosine_ops);
```

### Search

```sql
SELECT id, embedding <=> query_vector AS distance
FROM memory_vectors
ORDER BY embedding <=> query_vector
LIMIT 10;
```

### Advantages

- Integrates with PostgreSQL
- ACID transactions
- Shared access
- Familiar SQL interface

### Limitations

- Requires pgvector extension
- PostgreSQL setup needed

---

## Redis Vector

Redis with vector search capabilities.

### Configuration

```yaml
storage:
  vector: redis

redis:
  url: redis://localhost:6379
  prefix: subcog:vec:
```

### Setup

```
FT.CREATE subcog:vectors
  ON HASH
  PREFIX 1 subcog:vec:
  SCHEMA
    id TEXT
    embedding VECTOR HNSW 6
      TYPE FLOAT32
      DIM 384
      DISTANCE_METRIC COSINE
```

### Search

```
FT.SEARCH subcog:vectors
  "*=>[KNN 10 @embedding $query_vec AS score]"
  PARAMS 2 query_vec <binary_vector>
  SORTBY score
  RETURN 2 id score
```

### Advantages

- Very fast
- Distributed
- Real-time updates
- Combined with text search

### Limitations

- Requires Redis Stack
- Memory-intensive
- Complex setup

---

## Embedding Model

Default model: `all-MiniLM-L6-v2`

### Specifications

| Property | Value |
|----------|-------|
| Dimensions | 384 |
| Max sequence | 256 tokens |
| Model size | ~90MB |
| Embedding time | ~20ms |

### Configuration

```yaml
storage:
  embedding_model: all-MiniLM-L6-v2
  embedding_dimensions: 384
  embedding_cache_size: 1000
```

### Caching

Embeddings are cached to avoid recomputation:

| Platform | Cache Path |
|----------|------------|
| macOS | `~/Library/Caches/subcog/embeddings/` |
| Linux | `~/.cache/subcog/embeddings/` |
| Windows | `%LOCALAPPDATA%\subcog\cache\embeddings\` |

---

## VectorBackend Trait

All backends implement:

```rust
#[async_trait]
pub trait VectorBackend: Send + Sync {
    async fn store(&self, id: &MemoryId, vector: &[f32]) -> Result<()>;
    async fn remove(&self, id: &MemoryId) -> Result<bool>;
    async fn search(&self, vector: &[f32], limit: usize) -> Result<Vec<VectorResult>>;
    async fn rebuild(&self, items: &[(MemoryId, Vec<f32>)]) -> Result<usize>;
}
```

## Similarity Metrics

| Metric | Description | Use |
|--------|-------------|-----|
| Cosine | Angle between vectors | Default, normalized |
| L2 | Euclidean distance | Absolute distance |
| Inner Product | Dot product | Magnitude matters |

Default: Cosine similarity (normalized for text embeddings)

## Hybrid Search (RRF)

Vector results are combined with text search using Reciprocal Rank Fusion:

```
RRF(d) = Σ 1 / (k + rank(d))
```

Where k = 60 (constant)

This balances:
- Semantic similarity (vector)
- Keyword matching (text)

## Rebuilding Vectors

If the vector index becomes corrupted:

```bash
subcog reindex
```

This:
1. Reads all memories from persistence
2. Re-embeds all content
3. Rebuilds vector index

## Choosing a Backend

| Criteria | usearch | pgvector | Redis |
|----------|---------|----------|-------|
| Setup | Zero | Medium | Medium |
| Speed | Very Fast | Fast | Very Fast |
| Memory | Low | Medium | High |
| Concurrency | Limited | High | High |
| Distribution | No | Yes | Yes |

## See Also

- [Persistence Layer](persistence.md) - Source of truth
- [Index Layer](index.md) - Full-text search
- [Search Architecture](../architecture/search.md) - Hybrid search
