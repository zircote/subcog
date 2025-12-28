# Storage and Observability Requirements Addendum

**CRITICAL**: This document supersedes sections 8-11 of PRD.md. All specifications herein are **mandatory** - there are no "future" features.

---

## 1. Storage Architecture Principles

### 1.1 Three-Layer Separation

Storage is separated into three independent, pluggable layers:

| Layer | Responsibility | Backends |
|-------|----------------|----------|
| **Persistence** | Authoritative storage of memories | Git Notes, PostgreSQL |
| **Index** | Searchable metadata + full-text (BM25) | SQLite, PostgreSQL, Redis |
| **Vector** | Embeddings + KNN similarity search | usearch, pgvector, Redis |

### 1.2 Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              ACCESS INTERFACES                                  │
│  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────────────┐ │
│  │   stdio   │ │    MCP    │ │ Streaming │ │   HTTP    │ │  Hook System      │ │
│  │   (CLI)   │ │  (Tools)  │ │    API    │ │   REST    │ │  (Claude Code)    │ │
│  └─────┬─────┘ └─────┬─────┘ └─────┬─────┘ └─────┬─────┘ └─────────┬─────────┘ │
│        └─────────────┴─────────────┴─────────────┴─────────────────┘           │
│                                     │                                          │
├─────────────────────────────────────▼──────────────────────────────────────────┤
│                              MEMORY SYSTEM CORE                                │
│  ┌─────────────────────────────────────────────────────────────────────────┐   │
│  │                        Service Layer                                     │  │
│  │  CaptureService │ RecallService │ SyncService │ ConsolidationService   │  │
│  └───────────────────────────────────┬─────────────────────────────────────┘   │
│                                      │                                         │
├──────────────────────────────────────▼─────────────────────────────────────────┤
│                           PLUGGABLE STORAGE                                    │
│                                                                                │
│  ┌─────────────────────────────────────────────────────────────────────────┐  │
│  │ PERSISTENCE LAYER (Authoritative Source)                               │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────────────┐ │  │
│  │  │  Git Notes   │  │  PostgreSQL  │  │        File System           │ │  │
│  │  │  (primary)   │  │  (optional)  │  │        (fallback)            │ │  │
│  │  └──────────────┘  └──────────────┘  └───────────────────────────────┘ │  │
│  └─────────────────────────────────────────────────────────────────────────┘  │
│                                                                                │
│  ┌─────────────────────────────────────────────────────────────────────────┐  │
│  │ INDEX LAYER (Searchable Metadata + Full-Text)                          │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────────────┐ │  │
│  │  │    SQLite    │  │  PostgreSQL  │  │         Redis                │ │  │
│  │  │   (FTS5)     │  │  (full-text) │  │    (RediSearch)              │ │  │
│  │  └──────────────┘  └──────────────┘  └───────────────────────────────┘ │  │
│  └─────────────────────────────────────────────────────────────────────────┘  │
│                                                                                │
│  ┌─────────────────────────────────────────────────────────────────────────┐  │
│  │ VECTOR LAYER (Embeddings + KNN Search)                                 │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────────────┐ │  │
│  │  │   usearch    │  │   pgvector   │  │         Redis                │ │  │
│  │  │  (local)     │  │  (postgres)  │  │    (vector search)           │ │  │
│  │  └──────────────┘  └──────────────┘  └───────────────────────────────┘ │  │
│  └─────────────────────────────────────────────────────────────────────────┘  │
│                                                                                │
└────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Trait Definitions

### 2.1 Persistence Trait

```rust
/// Persistence layer for authoritative memory storage
/// Git notes is the primary implementation
#[async_trait]
pub trait PersistenceBackend: Send + Sync {
    /// Persist a memory (create or update)
    async fn persist(&self, memory: &Memory) -> Result<PersistenceResult>;

    /// Load a memory by ID
    async fn load(&self, id: &MemoryId) -> Result<Option<Memory>>;

    /// Load all memories for a namespace
    async fn load_namespace(&self, namespace: Namespace, domain: Domain) -> Result<Vec<Memory>>;

    /// Load all memories (for full reindex)
    async fn load_all(&self, domain: Option<Domain>) -> Result<Vec<Memory>>;

    /// Delete a memory
    async fn delete(&self, id: &MemoryId) -> Result<()>;

    /// Sync with remote (if applicable)
    async fn sync_remote(&self, direction: SyncDirection) -> Result<SyncResult>;

    /// Get persistence statistics
    async fn stats(&self) -> Result<PersistenceStats>;

    /// Backend identifier for logging/metrics
    fn backend_name(&self) -> &'static str;
}
```

### 2.2 Index Trait

```rust
/// Index layer for searchable metadata and BM25 full-text search
#[async_trait]
pub trait IndexBackend: Send + Sync {
    /// Index a memory's metadata
    async fn index(&self, memory: &Memory) -> Result<()>;

    /// Remove a memory from the index
    async fn remove(&self, id: &MemoryId) -> Result<()>;

    /// Full-text search (BM25)
    async fn search_text(
        &self,
        query: &str,
        filter: &SearchFilter,
        limit: usize,
    ) -> Result<Vec<IndexResult>>;

    /// Filter search by metadata only
    async fn search_filter(
        &self,
        filter: &SearchFilter,
        limit: usize,
    ) -> Result<Vec<IndexResult>>;

    /// Get index statistics
    async fn stats(&self) -> Result<IndexStats>;

    /// Initialize index (create tables/indices)
    async fn initialize(&self) -> Result<()>;

    /// Run migrations
    async fn migrate(&self) -> Result<MigrationResult>;

    /// Backend identifier
    fn backend_name(&self) -> &'static str;
}
```

### 2.3 Vector Trait

```rust
/// Vector layer for embedding storage and KNN similarity search
#[async_trait]
pub trait VectorBackend: Send + Sync {
    /// Store an embedding for a memory
    async fn store_embedding(&self, id: &MemoryId, embedding: &[f32]) -> Result<()>;

    /// Remove an embedding
    async fn remove_embedding(&self, id: &MemoryId) -> Result<()>;

    /// KNN similarity search
    async fn search_knn(
        &self,
        query_embedding: &[f32],
        k: usize,
        filter: Option<&VectorFilter>,
    ) -> Result<Vec<VectorResult>>;

    /// Batch KNN search (for multiple queries)
    async fn search_knn_batch(
        &self,
        queries: &[Vec<f32>],
        k: usize,
    ) -> Result<Vec<Vec<VectorResult>>>;

    /// Get vector statistics
    async fn stats(&self) -> Result<VectorStats>;

    /// Initialize vector storage
    async fn initialize(&self, dimensions: usize) -> Result<()>;

    /// Backend identifier
    fn backend_name(&self) -> &'static str;

    /// Supported distance metrics
    fn supported_metrics(&self) -> Vec<DistanceMetric>;
}
```

---

## 3. Backend Implementations

### 3.1 Git Notes Persistence

**Use Case**: Primary authoritative storage
**Storage Format**: YAML front matter + markdown body in `refs/notes/mem/{namespace}`

```rust
pub struct GitNotesPersistence {
    repo_path: PathBuf,
    user_repo_path: PathBuf,  // Bare repo for user domain
}
```

### 3.2 SQLite + usearch

**Use Case**: Local single-user deployment, CLI usage, offline-first
**Latency**: <10ms
**Memory**: ~50MB

```rust
pub struct SqliteBackend {
    conn: Arc<Mutex<Connection>>,
    vector_index: Arc<Mutex<usearch::Index>>,
    db_path: PathBuf,
}
```

Features:
- FTS5 for BM25 full-text search
- usearch HNSW for vector KNN
- WAL mode for performance
- Single-file deployment

### 3.3 PostgreSQL + pgvector

**Use Case**: Team environments, production deployments, multi-user access
**Latency**: <50ms
**Memory**: ~500MB

```rust
pub struct PostgresBackend {
    pool: sqlx::PgPool,
    dimensions: usize,
}
```

Features:
- pgvector extension for vector search
- PostgreSQL full-text search
- ACID guarantees
- Connection pooling
- Horizontal scaling with read replicas

### 3.4 Redis

**Use Case**: Distributed caching, high-throughput, real-time applications
**Latency**: <5ms
**Memory**: In-memory (configurable persistence)

```rust
pub struct RedisBackend {
    client: redis::Client,
    conn: Arc<Mutex<redis::aio::MultiplexedConnection>>,
    prefix: String,
    dimensions: usize,
}
```

Features:
- RediSearch for full-text search
- Redis vector search (HNSW)
- Cluster support
- Pub/sub for real-time updates

---

## 4. Backend Comparison Matrix

| Aspect | SQLite + usearch | PostgreSQL + pgvector | Redis |
|--------|------------------|----------------------|-------|
| **Deployment** | Single file, local | Server required | Server required |
| **Scaling** | Single user | Multi-user, sharding | Cluster, replicas |
| **Latency** | <10ms | <50ms | <5ms |
| **Persistence** | File-based | ACID, WAL | Optional (AOF/RDB) |
| **Full-text** | FTS5 (good) | PostgreSQL FTS (excellent) | RediSearch (good) |
| **Vector** | HNSW (excellent) | pgvector HNSW (good) | HNSW (good) |
| **Memory** | Low (~50MB) | Medium (~500MB) | High (in-memory) |
| **Best for** | CLI, single dev | Teams, production | Caching, real-time |

---

## 5. Configuration Schema

```toml
# memory.toml - Complete storage configuration

[persistence]
# Authoritative storage backend
backend = "git-notes"  # git-notes | postgresql

# Git notes specific (default)
# Uses repo from current directory
# User memories stored in ~/.local/share/memory/user-memories

# PostgreSQL specific
# postgres_url = "postgresql://user:pass@host:5432/memories"

[index]
# Metadata and full-text search backend
backend = "sqlite"  # sqlite | postgresql | redis

# SQLite specific (default)
# Path auto-configured to data_dir/index.db

# PostgreSQL specific
# postgres_url = "postgresql://user:pass@host:5432/memories"

# Redis specific
# redis_url = "redis://localhost:6379"
# redis_prefix = "memory"

[vector]
# Vector embedding and KNN search backend
backend = "usearch"  # usearch | pgvector | redis

# usearch specific (default)
# Path auto-configured to data_dir/index.usearch

# pgvector specific (uses same connection as index if postgresql)
# postgres_url = "postgresql://user:pass@host:5432/memories"

# Redis specific (uses same connection as index if redis)
# redis_url = "redis://localhost:6379"
# redis_prefix = "memory"

[embedding]
# Embedding model configuration
model = "sentence-transformers/all-MiniLM-L6-v2"
dimensions = 384
cache_dir = "~/.local/share/memory/models"
```

---

## 6. Observability Requirements

**CRITICAL**: Observability is NOT optional. Every operation MUST be traceable, measurable, and auditable.

### 6.1 Observability Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                           OBSERVABILITY PIPELINE                               │
│                                                                                │
│  ┌───────────────────────────────────────────────────────────────────────┐    │
│  │                        INSTRUMENTATION                                 │    │
│  │                                                                        │    │
│  │  Every Operation                                                       │    │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────────┐  │    │
│  │  │   Spans     │ │  Metrics    │ │    Logs     │ │   Audit Events  │  │    │
│  │  │ (tracing)   │ │ (counters/  │ │ (structured)│ │ (compliance)    │  │    │
│  │  │             │ │ histograms) │ │             │ │                 │  │    │
│  │  └──────┬──────┘ └──────┬──────┘ └──────┬──────┘ └───────┬─────────┘  │    │
│  │         │               │               │                │            │    │
│  └─────────┴───────────────┴───────────────┴────────────────┴────────────┘    │
│                                    │                                          │
│  ┌─────────────────────────────────▼────────────────────────────────────────┐ │
│  │                          COLLECTION                                       │ │
│  │  ┌─────────────────────────────────────────────────────────────────────┐ │ │
│  │  │                    OpenTelemetry SDK                                 │ │ │
│  │  │  TracerProvider │ MeterProvider │ LoggerProvider                   │ │ │
│  │  └─────────────────────────────────────────────────────────────────────┘ │ │
│  └─────────────────────────────────┬────────────────────────────────────────┘ │
│                                    │                                          │
│  ┌─────────────────────────────────▼────────────────────────────────────────┐ │
│  │                           EXPORT                                          │ │
│  │  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌─────────────────────────┐│ │
│  │  │   OTLP     │ │ Prometheus │ │   File     │ │   Console (stderr)     ││ │
│  │  │ (gRPC/HTTP)│ │ (scrape)   │ │  (JSON)    │ │   (structured/pretty)  ││ │
│  │  └────────────┘ └────────────┘ └────────────┘ └─────────────────────────┘│ │
│  └──────────────────────────────────────────────────────────────────────────┘ │
│                                                                                │
└────────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Tracing Requirements

**Every operation MUST be traced with:**

```rust
#[tracing::instrument(
    name = "memory.capture",
    skip(content),
    fields(
        namespace = %namespace,
        domain = %domain,
        memory.id = tracing::field::Empty,
        memory.summary_len = content.len(),
    )
)]
pub async fn capture(
    &self,
    namespace: Namespace,
    summary: &str,
    content: &str,
    domain: Domain,
) -> Result<CaptureResult> {
    let span = tracing::Span::current();

    // Record timing for each sub-operation
    let _validate_span = tracing::info_span!("validate").entered();
    self.validate(summary, content)?;
    drop(_validate_span);

    let _persist_span = tracing::info_span!("persist").entered();
    let result = self.persistence.persist(&memory).await?;
    drop(_persist_span);

    let _embed_span = tracing::info_span!("embed").entered();
    let embedding = self.embedder.embed(&content)?;
    drop(_embed_span);

    let _index_span = tracing::info_span!("index").entered();
    self.storage.index(&memory, &embedding).await?;
    drop(_index_span);

    // Record final fields
    span.record("memory.id", &result.id.0);

    Ok(result)
}
```

**Required span attributes:**

| Attribute | Type | Description |
|-----------|------|-------------|
| `operation` | string | Operation name (capture, recall, search, etc.) |
| `namespace` | string | Memory namespace |
| `domain` | string | project or user |
| `memory.id` | string | Memory ID (when applicable) |
| `backend.persistence` | string | Persistence backend name |
| `backend.index` | string | Index backend name |
| `backend.vector` | string | Vector backend name |
| `error` | bool | Whether operation failed |
| `error.message` | string | Error message (if failed) |

### 6.3 Metrics Requirements

**Required metrics (all with standard labels):**

```rust
// Counters
memory_operations_total{operation, namespace, domain, status}
memory_search_total{mode, domain, status}
memory_sync_total{direction, domain, status}
embedding_operations_total{status}
hook_executions_total{hook_type, status}

// Histograms (with buckets: 1, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000 ms)
memory_operation_duration_ms{operation, namespace}
memory_search_duration_ms{mode, backend}
embedding_duration_ms{}
hook_duration_ms{hook_type}
persistence_duration_ms{backend, operation}
index_duration_ms{backend, operation}
vector_duration_ms{backend, operation}

// Gauges
memory_count{namespace, domain, status}
index_size_bytes{backend}
vector_count{backend}
active_sessions{}
```

### 6.4 Logging Requirements

**Log levels and when to use:**

| Level | Use Case |
|-------|----------|
| ERROR | Operation failures, data corruption, unrecoverable errors |
| WARN | Degraded operation, retries, timeouts, deprecated usage |
| INFO | Operation completion, significant state changes |
| DEBUG | Request/response details, cache hits/misses |
| TRACE | Full payloads, intermediate steps, profiling data |

**Structured log format:**

```json
{
  "timestamp": "2025-01-15T10:30:00.123Z",
  "level": "INFO",
  "target": "memory::capture",
  "message": "Memory captured successfully",
  "span": {
    "name": "memory.capture",
    "trace_id": "abc123",
    "span_id": "def456"
  },
  "fields": {
    "namespace": "decisions",
    "domain": "project",
    "memory_id": "decisions:abc1234:0",
    "duration_ms": 45
  }
}
```

### 6.5 Audit Requirements

**Auditable events (SOC2/GDPR compliance):**

| Event | Required Fields |
|-------|-----------------|
| `memory.created` | memory_id, namespace, domain, timestamp, user_context |
| `memory.deleted` | memory_id, namespace, domain, timestamp, user_context, reason |
| `memory.accessed` | memory_id, namespace, domain, timestamp, user_context, access_type |
| `secrets.detected` | secret_type, action, source, timestamp, content_hash |
| `secrets.redacted` | secret_type, action, source, timestamp |
| `sync.remote` | direction, remote_url, domain, timestamp, result |
| `config.changed` | key, old_value_hash, new_value_hash, timestamp |

**Audit log format:**

```rust
#[derive(Debug, Serialize)]
pub struct AuditEvent {
    pub event_type: AuditEventType,
    pub timestamp: DateTime<Utc>,
    pub trace_id: Option<String>,
    pub user_context: Option<UserContext>,
    pub resource: AuditResource,
    pub action: String,
    pub outcome: AuditOutcome,
    pub metadata: HashMap<String, Value>,
}

#[derive(Debug, Serialize)]
pub struct AuditResource {
    pub resource_type: String,  // memory, secret, config
    pub resource_id: Option<String>,
    pub namespace: Option<String>,
    pub domain: Option<String>,
}
```

### 6.6 Profiling Requirements

**Full execution profiling for:**

1. **Request profiling**: Every MCP tool call, CLI command, hook execution
2. **Backend profiling**: Persistence, index, and vector operations
3. **Embedding profiling**: Model inference time, batch sizes
4. **Memory profiling**: Heap usage during search, embedding generation

**Profiling output (when enabled):**

```json
{
  "profile_id": "prof_123",
  "operation": "memory.recall",
  "total_duration_ms": 85,
  "breakdown": {
    "embed_query": 12,
    "vector_search": 25,
    "text_search": 18,
    "rrf_fusion": 5,
    "hydration": 20,
    "serialization": 5
  },
  "memory": {
    "peak_heap_mb": 45,
    "allocated_mb": 32
  },
  "backend_calls": [
    {"backend": "usearch", "operation": "search_knn", "duration_ms": 25},
    {"backend": "sqlite", "operation": "search_text", "duration_ms": 18}
  ]
}
```

### 6.7 Observability Configuration

```toml
# memory.toml - Observability configuration

[observability]
# Master switch
enabled = true

[observability.tracing]
enabled = true
# Sampling rate (0.0 - 1.0, 1.0 = all traces)
sample_rate = 1.0
# Include prompts/responses in spans (verbose, disable in production)
trace_prompts = false
# Max length for prompt/response in spans
trace_prompt_max_length = 2000

[observability.metrics]
enabled = true
# Prometheus scrape endpoint
prometheus_enabled = true
prometheus_port = 9090

[observability.logging]
# Level: quiet | info | debug | trace
level = "info"
# Format: json | text
format = "json"
# Output: stderr | file | both
output = "stderr"
# Log file path (if output includes file)
# file_path = "/var/log/memory/memory.log"

[observability.audit]
enabled = true
# Audit log directory
audit_dir = "~/.local/share/memory/audit"
# Retention days
retention_days = 90
# Events to audit (empty = all)
# events = ["memory.created", "memory.deleted", "secrets.detected"]

[observability.profiling]
# Enable detailed profiling
enabled = false
# Profile output directory
profile_dir = "~/.local/share/memory/profiles"
# Profile slow operations (ms threshold)
slow_operation_threshold_ms = 100

[observability.export]
# OTLP endpoint for traces and metrics
otlp_endpoint = ""  # e.g., "http://localhost:4318"
# OTLP protocol: grpc | http
otlp_protocol = "http"
# Allow localhost/internal endpoints (disable SSRF protection)
otlp_allow_internal = false
```

---

## 7. Integration Points

### 7.1 Feature Integration Matrix

Every feature MUST integrate with observability:

| Feature | Tracing | Metrics | Logging | Audit |
|---------|---------|---------|---------|-------|
| Capture | ✓ | ✓ | ✓ | ✓ |
| Recall | ✓ | ✓ | ✓ | ✓ |
| Search | ✓ | ✓ | ✓ | ✓ |
| Sync | ✓ | ✓ | ✓ | ✓ |
| Hooks | ✓ | ✓ | ✓ | - |
| Secrets Filtering | ✓ | ✓ | ✓ | ✓ |
| Consolidation | ✓ | ✓ | ✓ | ✓ |
| LLM Operations | ✓ | ✓ | ✓ | - |

### 7.2 Storage ↔ Observability Integration

```rust
/// All storage operations are instrumented
impl<P, I, V> CompositeStorage<P, I, V>
where
    P: PersistenceBackend,
    I: IndexBackend,
    V: VectorBackend,
{
    #[tracing::instrument(skip(self, memory, embedding))]
    pub async fn insert(&self, memory: &Memory, embedding: &[f32]) -> Result<InsertResult> {
        // Metrics
        let timer = metrics::histogram!("memory_operation_duration_ms", "operation" => "insert");

        // Persist
        let persist_result = {
            let _span = tracing::info_span!("persist", backend = self.persistence.backend_name()).entered();
            self.persistence.persist(memory).await?
        };

        // Index
        {
            let _span = tracing::info_span!("index", backend = self.index.backend_name()).entered();
            self.index.index(memory).await?;
        }

        // Vector
        {
            let _span = tracing::info_span!("vector", backend = self.vector.backend_name()).entered();
            self.vector.store_embedding(&memory.id, embedding).await?;
        }

        // Audit
        audit::log(AuditEvent::memory_created(&memory));

        timer.record(start.elapsed());
        metrics::counter!("memory_operations_total", "operation" => "insert", "status" => "success").increment(1);

        Ok(InsertResult { /* ... */ })
    }
}
```

---

## 8. Validation Requirements

### 8.1 Storage Validation

Every storage backend MUST pass:

1. **CRUD tests**: Create, read, update, delete operations
2. **Search tests**: Full-text and vector search correctness
3. **Concurrency tests**: Parallel read/write operations
4. **Failure tests**: Network failures, timeouts, reconnection
5. **Migration tests**: Schema migration up/down

### 8.2 Observability Validation

1. **Trace completeness**: Every operation has traces
2. **Metric accuracy**: Metrics match actual operations
3. **Log correlation**: Logs correlate with traces
4. **Audit completeness**: All auditable events logged

---

## 9. Non-Negotiable Requirements

| Requirement | Description |
|-------------|-------------|
| **No "future" backends** | All specified backends are implemented in v1.0 |
| **Full observability** | Every operation is traced, metered, logged |
| **Configuration-driven** | All backends selectable via config |
| **Seamless integration** | Features compose without special cases |
| **Audit compliance** | SOC2/GDPR audit trail for all data operations |
