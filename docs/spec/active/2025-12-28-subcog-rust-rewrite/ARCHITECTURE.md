# Architecture Design

This document defines the technical architecture for Subcog, the Rust rewrite of the git-notes-memory system.

---

## 1. System Overview

### 1.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              SUBCOG ARCHITECTURE                                 │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                 │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐  ┌─────────────────┐   │
│  │    CLI        │  │  MCP Server   │  │  Streaming    │  │  Hook Handlers  │   │
│  │   (clap)      │  │   (rmcp)      │  │     API       │  │  (JSON output)  │   │
│  └───────┬───────┘  └───────┬───────┘  └───────┬───────┘  └───────┬─────────┘   │
│          │                  │                  │                  │             │
│          └──────────────────┼──────────────────┼──────────────────┘             │
│                             │                  │                                │
│                             ▼                  ▼                                │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │                         ACCESS INTERFACE LAYER                          │    │
│  │  Unified request handling, authentication, rate limiting                │    │
│  └─────────────────────────────────┬───────────────────────────────────────┘    │
│                                    │                                            │
│                                    ▼                                            │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │                           EVENT BUS (tokio broadcast)                   │    │
│  │  MemoryCaptured │ MemoryUpdated │ SyncCompleted │ ConsolidationRan      │    │
│  └────────┬────────────────┬────────────────┬────────────────┬─────────────┘    │
│           │                │                │                │                  │
│           ▼                ▼                ▼                ▼                  │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │                          SERVICE LAYER                                  │    │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐ │    │
│  │  │ Capture     │  │ Recall      │  │ Sync        │  │ Consolidation   │ │    │
│  │  │ Service     │  │ Service     │  │ Service     │  │ Service         │ │    │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └───────┬─────────┘ │    │
│  └─────────┼────────────────┼────────────────┼─────────────────┼───────────┘    │
│            │                │                │                 │                │
│            ▼                ▼                ▼                 ▼                │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │                        CORE COMPONENTS                                  │    │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐ │    │
│  │  │ Embedding   │  │ Secrets     │  │ LLM Client  │  │ Config          │ │    │
│  │  │ (fastembed) │  │ Filter      │  │ (multi)     │  │ Manager         │ │    │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────────┘ │    │
│  └─────────────────────────────────────────────────────────────────────────┘    │
│                                    │                                            │
│                                    ▼                                            │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │                     THREE-LAYER STORAGE ARCHITECTURE                    │    │
│  │                                                                         │    │
│  │  ┌─────────────────────────────────────────────────────────────────┐   │    │
│  │  │ PERSISTENCE LAYER (Authoritative Source)                       │   │    │
│  │  │  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────┐ │   │    │
│  │  │  │  Git Notes   │  │  PostgreSQL  │  │    File System        │ │   │    │
│  │  │  │  (primary)   │  │  (optional)  │  │    (fallback)         │ │   │    │
│  │  │  └──────────────┘  └──────────────┘  └───────────────────────┘ │   │    │
│  │  └─────────────────────────────────────────────────────────────────┘   │    │
│  │                                                                         │    │
│  │  ┌─────────────────────────────────────────────────────────────────┐   │    │
│  │  │ INDEX LAYER (Searchable Metadata + Full-Text)                  │   │    │
│  │  │  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────┐ │   │    │
│  │  │  │    SQLite    │  │  PostgreSQL  │  │        Redis          │ │   │    │
│  │  │  │   (FTS5)     │  │  (full-text) │  │    (RediSearch)       │ │   │    │
│  │  │  └──────────────┘  └──────────────┘  └───────────────────────┘ │   │    │
│  │  └─────────────────────────────────────────────────────────────────┘   │    │
│  │                                                                         │    │
│  │  ┌─────────────────────────────────────────────────────────────────┐   │    │
│  │  │ VECTOR LAYER (Embeddings + KNN Search)                         │   │    │
│  │  │  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────┐ │   │    │
│  │  │  │   usearch    │  │   pgvector   │  │        Redis          │ │   │    │
│  │  │  │  (local)     │  │  (postgres)  │  │    (vector search)    │ │   │    │
│  │  │  └──────────────┘  └──────────────┘  └───────────────────────┘ │   │    │
│  │  └─────────────────────────────────────────────────────────────────┘   │    │
│  │                                                                         │    │
│  └─────────────────────────────────────────────────────────────────────────┘    │
│                                    │                                            │
│                                    ▼                                            │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │                        OBSERVABILITY LAYER                              │    │
│  │  Traces ──────► Metrics ──────► Logs ──────► Audit ──────► OTLP        │    │
│  └─────────────────────────────────────────────────────────────────────────┘    │
│                                                                                 │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Design Principles

| Principle | Description | Implementation |
|-----------|-------------|----------------|
| **Three-Layer Storage** | Separation of persistence, index, and vector | Trait-based abstraction per layer |
| **Feature Tiers** | Core → Enhanced → LLM-powered | Runtime feature flags |
| **Event-Driven** | State changes propagate via events | tokio broadcast channel |
| **Graceful Degradation** | Features fail open | Fallback chains |
| **Configuration-Driven** | All backends/features via config | TOML + env vars |
| **Full Observability** | Every operation traceable | OpenTelemetry spans |

---

## 2. Module Structure

```
src/
├── lib.rs                    # Library entry point
├── main.rs                   # CLI entry point
│
├── models/                   # Data structures
│   ├── mod.rs
│   ├── memory.rs            # Memory, MemoryResult, MemoryId
│   ├── capture.rs           # CaptureResult, CaptureRequest
│   ├── search.rs            # SearchResult, SearchFilter, SearchMode
│   ├── consolidation.rs     # MemoryTier, EdgeType, RetentionScore
│   ├── domain.rs            # Domain, Namespace, MemoryStatus
│   └── events.rs            # MemoryEvent variants
│
├── storage/                  # Three-layer storage abstraction
│   ├── mod.rs               # CompositeStorage, layer trait re-exports
│   ├── traits/
│   │   ├── mod.rs
│   │   ├── persistence.rs   # PersistenceBackend trait
│   │   ├── index.rs         # IndexBackend trait
│   │   └── vector.rs        # VectorBackend trait
│   ├── persistence/
│   │   ├── mod.rs
│   │   ├── git_notes.rs     # Git notes implementation
│   │   ├── postgresql.rs    # PostgreSQL implementation
│   │   └── filesystem.rs    # Fallback filesystem storage
│   ├── index/
│   │   ├── mod.rs
│   │   ├── sqlite.rs        # SQLite + FTS5 implementation
│   │   ├── postgresql.rs    # PostgreSQL full-text
│   │   └── redis.rs         # RediSearch implementation
│   └── vector/
│       ├── mod.rs
│       ├── usearch.rs       # usearch HNSW implementation
│       ├── pgvector.rs      # pgvector implementation
│       └── redis.rs         # Redis vector search
│
├── services/                 # Business logic
│   ├── mod.rs               # ServiceContainer
│   ├── capture.rs           # CaptureService
│   ├── recall.rs            # RecallService (search)
│   ├── sync.rs              # SyncService
│   ├── consolidation.rs     # ConsolidationService
│   └── context.rs           # ContextBuilderService
│
├── git/                      # Git operations
│   ├── mod.rs
│   ├── notes.rs             # Git notes CRUD
│   ├── remote.rs            # Fetch/push operations
│   └── parser.rs            # YAML front matter parsing
│
├── embedding/                # Embedding generation
│   ├── mod.rs               # Embedder trait
│   ├── fastembed.rs         # FastEmbed implementation
│   └── fallback.rs          # Fallback to BM25-only
│
├── llm/                      # LLM client abstraction
│   ├── mod.rs               # LLMProvider trait
│   ├── anthropic.rs         # Anthropic Claude implementation
│   ├── openai.rs            # OpenAI implementation
│   ├── ollama.rs            # Ollama (local) implementation
│   └── lmstudio.rs          # LM Studio implementation
│
├── hooks/                    # Claude Code hooks
│   ├── mod.rs               # HookHandler trait
│   ├── session_start.rs     # Context injection, fetch remote
│   ├── user_prompt.rs       # Signal detection, capture markers
│   ├── post_tool_use.rs     # Related memory surfacing
│   ├── pre_compact.rs       # Auto-capture before compaction
│   └── stop.rs              # Session analysis, sync/push
│
├── mcp/                      # MCP server
│   ├── mod.rs
│   ├── server.rs            # MCP server setup (rmcp)
│   ├── tools.rs             # Tool implementations
│   ├── resources.rs         # Resource handlers (URN scheme)
│   └── prompts.rs           # Pre-defined prompts
│
├── security/                 # Security features
│   ├── mod.rs
│   ├── secrets.rs           # Secret detection patterns
│   ├── pii.rs               # PII detection
│   ├── redactor.rs          # Content redaction/masking
│   └── audit.rs             # SOC2/GDPR audit logging
│
├── config/                   # Configuration
│   ├── mod.rs               # Config struct, loading
│   ├── features.rs          # FeatureFlags
│   └── validation.rs        # Configuration validation
│
├── events/                   # Event bus
│   ├── mod.rs               # EventBus
│   └── handlers.rs          # EventHandler implementations
│
├── cli/                      # CLI commands
│   ├── mod.rs               # Cli struct (clap)
│   ├── capture.rs           # capture subcommand
│   ├── recall.rs            # recall subcommand
│   ├── status.rs            # status subcommand
│   ├── sync.rs              # sync subcommand
│   ├── consolidate.rs       # consolidate subcommand
│   ├── config.rs            # config subcommand
│   ├── serve.rs             # serve subcommand (MCP)
│   └── hook.rs              # hook subcommand
│
└── observability/            # Telemetry
    ├── mod.rs               # Initialization
    ├── metrics.rs           # Prometheus metrics
    ├── tracing.rs           # Distributed tracing
    ├── logging.rs           # Structured logging
    └── otlp.rs              # OTLP export
```

---

## 3. Storage Architecture

### 3.1 Three-Layer Separation

Storage is separated into three independent, pluggable layers:

| Layer | Responsibility | Backends |
|-------|----------------|----------|
| **Persistence** | Authoritative storage of memories | Git Notes, PostgreSQL, Filesystem |
| **Index** | Searchable metadata + full-text (BM25) | SQLite (FTS5), PostgreSQL, Redis |
| **Vector** | Embeddings + KNN similarity search | usearch, pgvector, Redis |

### 3.2 Trait Definitions

#### 3.2.1 Persistence Trait

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

#### 3.2.2 Index Trait

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

#### 3.2.3 Vector Trait

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

### 3.3 Composite Storage

```rust
/// Unified storage that orchestrates all three layers
pub struct CompositeStorage<P, I, V>
where
    P: PersistenceBackend,
    I: IndexBackend,
    V: VectorBackend,
{
    persistence: P,
    index: I,
    vector: V,
    event_bus: Arc<EventBus>,
    audit: Arc<AuditLogger>,
}

impl<P, I, V> CompositeStorage<P, I, V>
where
    P: PersistenceBackend,
    I: IndexBackend,
    V: VectorBackend,
{
    /// Write to all three layers atomically
    #[tracing::instrument(skip(self, memory, embedding))]
    pub async fn write(&self, memory: &Memory, embedding: &[f32]) -> Result<WriteResult> {
        // 1. Persist (authoritative)
        let persist_result = self.persistence.persist(memory).await?;

        // 2. Index metadata
        self.index.index(memory).await?;

        // 3. Store embedding
        self.vector.store_embedding(&memory.id, embedding).await?;

        // 4. Emit event
        self.event_bus.publish(MemoryEvent::MemoryCaptured {
            memory_id: memory.id.clone(),
            namespace: memory.namespace,
            domain: memory.domain.clone(),
            uri: memory.uri(),
        });

        // 5. Audit log
        self.audit.log(AuditEvent::memory_created(memory)).await;

        Ok(WriteResult { /* ... */ })
    }

    /// Hybrid search across index and vector layers
    pub async fn search_hybrid(
        &self,
        query: &str,
        query_embedding: &[f32],
        filter: &SearchFilter,
        limit: usize,
    ) -> Result<Vec<MemoryResult>> {
        // Parallel search
        let (text_results, vector_results) = tokio::join!(
            self.index.search_text(query, filter, limit * 2),
            self.vector.search_knn(query_embedding, limit * 2, None)
        );

        // RRF fusion
        let fused = rrf_fusion(text_results?, vector_results?, limit);

        // Hydrate from persistence
        self.hydrate_results(fused).await
    }
}
```

### 3.4 Backend Comparison Matrix

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

## 4. Event Bus Architecture

### 4.1 Event Types

```rust
/// All events that propagate through the system
#[derive(Debug, Clone)]
pub enum MemoryEvent {
    // Capture events
    MemoryCaptured {
        memory_id: MemoryId,
        namespace: Namespace,
        domain: Domain,
        uri: String,
    },
    CaptureBlocked {
        reason: String,
        detection: SecretDetection,
    },

    // Recall events
    SearchCompleted {
        query: String,
        result_count: usize,
        latency_ms: u64,
    },

    // Sync events
    SyncStarted { direction: SyncDirection, domain: Domain },
    SyncCompleted {
        direction: SyncDirection,
        memories_synced: usize,
        conflicts_resolved: usize,
    },

    // Consolidation events
    ConsolidationStarted { full: bool },
    ClusterCreated { cluster_id: String, memory_count: usize },
    SummaryGenerated { cluster_id: String, summary_id: MemoryId },
    TierAssigned { memory_id: MemoryId, old_tier: MemoryTier, new_tier: MemoryTier },
    ConsolidationCompleted {
        clusters_created: usize,
        summaries_generated: usize,
        tiers_updated: usize,
    },

    // Security events
    SecretDetected { detection: SecretDetection, action: FilterAction },
    AuditLogWritten { event_type: String, memory_id: Option<MemoryId> },

    // Hook events
    HookTriggered { hook_type: HookType, session_id: String },
    HookCompleted { hook_type: HookType, latency_ms: u64 },

    // Storage events
    IndexRebuilt { memories_indexed: usize, duration_ms: u64 },
    StorageError { backend: String, error: String },

    // LLM events
    LlmRequestSent { provider: String, purpose: String },
    LlmResponseReceived { provider: String, tokens_used: u32, latency_ms: u64 },
}
```

### 4.2 Event Bus Implementation

```rust
use tokio::sync::broadcast;

/// Central event bus for cross-component communication
pub struct EventBus {
    sender: broadcast::Sender<MemoryEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn publish(&self, event: MemoryEvent) {
        // Ignore send errors (no receivers)
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<MemoryEvent> {
        self.sender.subscribe()
    }
}

/// Trait for components that handle events
#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: &MemoryEvent);
    fn filter(&self, event: &MemoryEvent) -> bool;
}
```

### 4.3 Feature Interaction Matrix

| Feature → | Capture | Recall | Sync | Consolidate | Secrets | Hooks |
|-----------|---------|--------|------|-------------|---------|-------|
| **Capture** | - | triggers recall | updates git | triggers background | filters content | emits signals |
| **Recall** | uses captured | - | syncs before | tier filtering | redacts output | context injection |
| **Sync** | imports memories | rebuilds index | - | updates tiers | re-validates | triggers on stop |
| **Consolidate** | creates summaries | tier-aware results | syncs after | - | excludes secrets | background run |
| **Secrets** | blocks/redacts | redacts results | filters sync | excludes from | - | filters in hooks |
| **Hooks** | marker detection | injects context | fetch/push | triggers | applies filter | - |

---

## 5. Service Architecture

### 5.1 Service Container

```rust
/// Container for all services, shared across interfaces
pub struct ServiceContainer {
    capture: Arc<CaptureService>,
    recall: Arc<RecallService>,
    sync: Arc<SyncService>,
    consolidation: Option<Arc<ConsolidationService>>,
    config: Arc<Config>,
    features: FeatureFlags,
    event_bus: Arc<EventBus>,
}

impl ServiceContainer {
    pub fn new(config: Config) -> Result<Self> {
        let features = FeatureFlags::from_config(&config)?;
        let event_bus = Arc::new(EventBus::new(1000));
        let storage = create_storage(&config, event_bus.clone())?;
        let embedder = create_embedder(&config)?;

        let capture = Arc::new(CaptureService::new(
            storage.clone(),
            embedder.clone(),
            &features,
            event_bus.clone(),
        )?);

        let recall = Arc::new(RecallService::new(
            storage.clone(),
            embedder.clone(),
            &features,
        )?);

        let sync = Arc::new(SyncService::new(storage.clone())?);

        let consolidation = if features.consolidation {
            let llm = create_llm_provider(&config)?;
            Some(Arc::new(ConsolidationService::new(storage, llm)?))
        } else {
            None
        };

        Ok(Self {
            capture,
            recall,
            sync,
            consolidation,
            config: Arc::new(config),
            features,
            event_bus,
        })
    }

    pub fn capture(&self) -> &Arc<CaptureService> { &self.capture }
    pub fn recall(&self) -> &Arc<RecallService> { &self.recall }
    pub fn sync(&self) -> &Arc<SyncService> { &self.sync }
    pub fn consolidation(&self) -> Option<&Arc<ConsolidationService>> { self.consolidation.as_ref() }
}
```

### 5.2 Capture Pipeline

```rust
impl CaptureService {
    pub async fn capture(&self, request: CaptureRequest) -> Result<CaptureResult> {
        let span = tracing::info_span!("capture", namespace = %request.namespace);
        let _guard = span.enter();

        // 1. Validate input (Core - always runs)
        self.validate(&request)?;

        // 2. Secrets filtering (Enhanced - if enabled)
        let filtered_content = if self.features.secrets_filtering {
            let filter_result = self.secrets_filter.filter(&request.content).await?;
            if filter_result.blocked {
                self.event_bus.publish(MemoryEvent::CaptureBlocked {
                    reason: "secrets_detected".into(),
                    detection: filter_result.detections.first().cloned().unwrap(),
                });
                return Err(MemoryError::ContentBlocked);
            }
            filter_result.filtered_content
        } else {
            request.content.clone()
        };

        // 3. Generate embedding (Core - always runs)
        let embedding = self.embedder.embed(&filtered_content).await?;

        // 4. Create memory object
        let memory = Memory {
            id: self.generate_id(&request.namespace),
            namespace: request.namespace,
            domain: request.domain,
            summary: request.summary.clone(),
            content: filtered_content,
            timestamp: Utc::now(),
            spec: request.spec,
            tags: request.tags,
            status: MemoryStatus::Active,
            relates_to: request.relates_to,
        };

        // 5. Write to all storage layers
        self.storage.write(&memory, &embedding).await?;

        // 6. Build URN
        let uri = memory.uri();

        Ok(CaptureResult {
            success: true,
            memory_id: memory.id.0.clone(),
            uri,
            indexed: true,
            warning: None,
        })
    }
}
```

### 5.3 Recall Pipeline

```rust
impl RecallService {
    pub async fn search(&self, request: SearchRequest) -> Result<SearchResult> {
        let span = tracing::info_span!("recall", query = %request.query);
        let _guard = span.enter();

        // 1. Query expansion (LLM Tier - if enabled)
        let expanded_query = if self.features.query_expansion {
            self.query_expander.expand(&request.query).await?
        } else {
            request.query.clone()
        };

        // 2. Generate query embedding (Core - always runs)
        let query_embedding = self.embedder.embed(&expanded_query).await?;

        // 3. Hybrid search (Core - always runs)
        let raw_results = match request.mode {
            SearchMode::Hybrid => {
                self.storage.search_hybrid(
                    &expanded_query,
                    &query_embedding,
                    &request.filter,
                    request.limit,
                ).await?
            }
            SearchMode::Vector => {
                self.storage.search_vector(&query_embedding, request.limit).await?
            }
            SearchMode::Bm25 => {
                self.storage.search_text(&expanded_query, request.limit).await?
            }
        };

        // 4. Tier filtering (Enhanced - if enabled)
        let filtered_results = if self.features.tier_filtering {
            raw_results.into_iter()
                .filter(|r| self.should_include_tier(r.memory.tier, &request.tier_filter))
                .collect()
        } else {
            raw_results
        };

        // 5. Secrets redaction (Enhanced - if enabled)
        let redacted_results = if self.features.secrets_filtering {
            filtered_results.into_iter()
                .map(|mut r| {
                    r.memory.content = self.secrets_filter.redact(&r.memory.content);
                    r
                })
                .collect()
        } else {
            filtered_results
        };

        Ok(SearchResult {
            results: redacted_results,
            total: redacted_results.len(),
            resource_template: "subcog://mem/{domain}/{namespace}/{id}".into(),
        })
    }
}
```

---

## 6. Access Interface Layer

### 6.1 Interface Overview

| Interface | Transport | Use Case | Latency Target |
|-----------|-----------|----------|----------------|
| **CLI** | stdio | Interactive, scripts, automation | <50ms |
| **MCP Server** | stdio / SSE | AI agent integration | <100ms |
| **Streaming API** | HTTP/SSE/WS | Long operations, real-time | N/A |
| **Hooks** | Process spawn | Claude Code integration | <100ms |

### 6.2 MCP Server Architecture

```rust
use rmcp::{Server, ServerConfig, Tool, Resource};

pub struct MemoryMcpServer {
    services: Arc<ServiceContainer>,
    config: McpConfig,
}

impl MemoryMcpServer {
    pub async fn serve_stdio(self) -> Result<()> {
        let server = Server::builder()
            .config(ServerConfig {
                name: "memory".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                capabilities: self.capabilities(),
            })
            .tools(self.tools())
            .resources(self.resources())
            .resource_templates(self.resource_templates())
            .build()?;

        server.serve_stdio().await
    }
}
```

### 6.3 URN Scheme

All memories are addressable via URN:

```
subcog://mem/{domain}/{namespace}/{id}

Examples:
- subcog://mem/project:my-app/decisions/abc1234:0
- subcog://mem/user/learnings/def5678:1
- subcog://mem/org:acme-corp/patterns/ghi9012:0
```

### 6.4 Hook System

```
┌─────────────────────────────────────────────────────────────────────┐
│                      CLAUDE CODE HOOKS                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  SessionStart ──► subcog hook session-start                        │
│  └── Context injection, fetch remote, memory recall                 │
│                                                                     │
│  UserPromptSubmit ──► subcog hook user-prompt                       │
│  └── Detect [decision], [learned], etc. markers                    │
│                                                                     │
│  PostToolUse ──► subcog hook post-tool                              │
│  └── Surface related memories after file operations                 │
│                                                                     │
│  PreCompact ──► subcog hook pre-compact                             │
│  └── Auto-capture important content before context loss             │
│                                                                     │
│  Stop ──► subcog hook stop                                          │
│  └── Session analysis, sync, push remote                            │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 7. Observability Architecture

### 7.1 Observability Pipeline

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

### 7.2 Required Metrics

```rust
// Counters
memory_operations_total{operation, namespace, domain, status}
memory_search_total{mode, domain, status}
memory_sync_total{direction, domain, status}
embedding_operations_total{status}
hook_executions_total{hook_type, status}

// Histograms (buckets: 1, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000 ms)
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

### 7.3 Audit Events (SOC2/GDPR Compliance)

| Event | Required Fields |
|-------|-----------------|
| `memory.created` | memory_id, namespace, domain, timestamp, user_context |
| `memory.deleted` | memory_id, namespace, domain, timestamp, user_context, reason |
| `memory.accessed` | memory_id, namespace, domain, timestamp, user_context, access_type |
| `secrets.detected` | secret_type, action, source, timestamp, content_hash |
| `secrets.redacted` | secret_type, action, source, timestamp |
| `sync.remote` | direction, remote_url, domain, timestamp, result |
| `config.changed` | key, old_value_hash, new_value_hash, timestamp |

---

## 8. Error Handling

### 8.1 Error Chain Design

```rust
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("Capture failed: {0}")]
    Capture(#[source] CaptureError),

    #[error("Recall failed: {0}")]
    Recall(#[source] RecallError),

    #[error("Sync failed: {0}")]
    Sync(#[source] SyncError),

    #[error("Consolidation failed: {0}")]
    Consolidation(#[source] ConsolidationError),

    #[error("Memory not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Security: content blocked")]
    ContentBlocked,

    #[error("Feature not enabled: {0}")]
    FeatureNotEnabled(String),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
}

impl MemoryError {
    /// Determine if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Validation(_) | Self::ContentBlocked | Self::NotFound(_) => false,
            Self::Config(_) | Self::FeatureNotEnabled(_) => false,
            _ => true,
        }
    }

    /// Get CLI exit code
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NotFound(_) => 10,
            Self::Validation(_) => 11,
            Self::ContentBlocked => 12,
            Self::Config(_) | Self::FeatureNotEnabled(_) => 3,
            _ => 1,
        }
    }
}
```

### 8.2 Graceful Degradation Matrix

| Error | Affected Features | Degraded Behavior | User Experience |
|-------|-------------------|-------------------|-----------------|
| Embedding fails | Vector search | BM25-only search | "Search may be less accurate" |
| LLM unavailable | Tier 3 features | Disabled | "Enhanced features unavailable" |
| Git remote unreachable | Remote sync | Local only | "Changes not synced to remote" |
| OTLP endpoint down | Telemetry export | Buffer locally | No user impact |
| Secrets filter error | Capture | Block with warning | "Security check failed, content blocked" |
| Storage full | All writes | Queue for retry | "Storage full, queuing..." |

---

## 9. Security Architecture

### 9.1 Secret Detection Patterns

| Type | Pattern | Action |
|------|---------|--------|
| API Keys | `sk-[a-zA-Z0-9]{32,}` | REDACT |
| AWS Keys | `AKIA[A-Z0-9]{16}` | REDACT |
| Private Keys | `-----BEGIN.*PRIVATE KEY-----` | BLOCK |
| Passwords | `password\s*[:=]\s*['"][^'"]+['"]` | REDACT |
| JWT Tokens | `eyJ[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+` | MASK |
| Database URLs | `(postgres\|mysql\|mongodb)://[^@]+@` | MASK |

### 9.2 PII Detection

| Type | Validation | Action |
|------|------------|--------|
| SSN | Format + checksum | REDACT |
| Credit Cards | Luhn algorithm | REDACT |
| Phone Numbers | E.164 format | MASK |
| Email | RFC 5322 | WARN |

### 9.3 Filter Strategies

```rust
#[derive(Debug, Clone, Copy)]
pub enum FilterStrategy {
    /// Replace with [REDACTED:type]
    Redact,
    /// Show partial content (abc...xyz)
    Mask,
    /// Reject content entirely
    Block,
    /// Log warning but pass through
    Warn,
}
```

---

## 10. Configuration Schema

```toml
# config.toml - Complete configuration

# ===== PERSISTENCE LAYER =====
[persistence]
backend = "git-notes"  # git-notes | postgresql

[persistence.git]
# Uses repo from current directory
# User memories stored in ~/.local/share/subcog/user-memories

[persistence.postgresql]
url = "postgresql://user:pass@host:5432/subcog"

# ===== INDEX LAYER =====
[index]
backend = "sqlite"  # sqlite | postgresql | redis

[index.sqlite]
path = "~/.local/share/subcog/index.db"

[index.redis]
url = "redis://localhost:6379"
prefix = "subcog"

# ===== VECTOR LAYER =====
[vector]
backend = "usearch"  # usearch | pgvector | redis

[vector.usearch]
path = "~/.local/share/subcog/vectors.usearch"

# ===== EMBEDDING =====
[embedding]
model = "sentence-transformers/all-MiniLM-L6-v2"
dimensions = 384
cache_dir = "~/.local/share/subcog/models"
fallback_to_bm25 = true

# ===== SEARCH =====
[search]
default_mode = "hybrid"
rrf_k = 60
default_limit = 10

# ===== FEATURE FLAGS =====
[features.enhanced]
entity_extraction = true
temporal_extraction = true
secrets_filtering = true
advanced_observability = true

[features.llm]
enabled = false
implicit_capture = false
consolidation = false
supersession_detection = false
temporal_reasoning = false
query_expansion = false

# ===== LLM PROVIDERS =====
[llm]
provider = "anthropic"  # anthropic | openai | ollama | lmstudio
model = "claude-3-5-sonnet-20241022"
api_key_env = "ANTHROPIC_API_KEY"
temperature = 0.3
max_tokens = 1000
timeout_seconds = 30

# ===== HOOKS =====
[hooks]
enabled = true

[hooks.session_start]
enabled = true
fetch_remote = false
context_token_budget = 2000
include_guidance = true

[hooks.user_prompt]
enabled = true
signal_patterns = ["[decision]", "[learned]", "[blocker]", "[progress]"]

[hooks.post_tool_use]
enabled = true
trigger_tools = ["Read", "Edit", "Write"]

[hooks.pre_compact]
enabled = true
confidence_threshold = 0.8

[hooks.stop]
enabled = true
push_remote = false

# ===== OBSERVABILITY =====
[observability]
enabled = true

[observability.tracing]
enabled = true
sample_rate = 1.0
trace_prompts = false

[observability.metrics]
enabled = true
prometheus_enabled = true
prometheus_port = 9090

[observability.logging]
level = "info"
format = "json"
output = "stderr"

[observability.audit]
enabled = true
audit_dir = "~/.local/share/subcog/audit"
retention_days = 90

[observability.export]
otlp_endpoint = ""
otlp_protocol = "http"
```

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-28 | Claude Opus 4.5 | Initial architecture from research documents |
