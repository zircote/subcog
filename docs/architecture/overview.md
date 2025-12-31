# Architecture Overview

Detailed overview of Subcog's system architecture.

> **Note**: This documentation describes the target architecture from the specification.
> Current implementation uses simplified, synchronous patterns. Where significant differences
> exist, they are noted inline. See [ARCHITECTURE.md](../../docs/spec/active/2025-12-28-subcog-rust-rewrite/ARCHITECTURE.md) for the full specification.

## System Layers

### 1. Access Layer

Entry points for interacting with the system.

#### CLI (`src/cli/`)

Command-line interface for direct user interaction:
- `capture` - Store memories
- `recall` - Search memories
- `status` - System status
- `sync` - Git synchronization
- `prompt` - Template management
- `serve` - MCP server
- `hook` - Claude Code hooks

#### MCP Server (`src/mcp/`)

Model Context Protocol server for AI assistant integration:
- JSON-RPC 2.0 protocol
- Tools, Resources, Prompts
- stdio and HTTP transports

#### Hooks (`src/hooks/`)

Claude Code integration hooks:
- SessionStart - Context injection
- UserPromptSubmit - Memory surfacing
- PostToolUse - Related memories
- PreCompact - Auto-capture
- Stop - Session finalization

### 2. Service Layer

Business logic and orchestration.

#### ServiceContainer

Dependency injection container:

**Target Design (Spec):**
```rust
pub struct ServiceContainer<P, I, V>
where
    P: PersistenceBackend,
    I: IndexBackend,
    V: VectorBackend,
{
    capture: CaptureService<P, I, V>,
    recall: RecallService<I, V>,
    prompt: PromptService<P>,
    sync: SyncService<P>,
    consolidation: ConsolidationService<P, I, V>,
    context: ContextBuilderService<I, V>,
}
```

**Current Implementation:**
```rust
pub struct ServiceContainer {
    capture: CaptureService,
    sync: SyncService,
    index_manager: Mutex<DomainIndexManager>,
    repo_path: Option<PathBuf>,
}
```

#### Services

| Service | Responsibility |
|---------|----------------|
| `CaptureService` | Memory capture with validation |
| `RecallService` | Search with RRF fusion |
| `PromptService` | Template CRUD and execution |
| `SyncService` | Git remote synchronization |
| `ConsolidationService` | LLM-powered memory merging |
| `ContextBuilderService` | Adaptive context building |
| `TopicIndexService` | Topic → memory mapping |

### 3. Storage Layer

Three-tier storage architecture.

#### CompositeStorage

Generic storage facade:

```rust
pub struct CompositeStorage<P, I, V>
where
    P: PersistenceBackend,
    I: IndexBackend,
    V: VectorBackend,
{
    persistence: P,
    index: I,
    vector: V,
}
```

#### Layer Traits

**Target Design (Spec) - Async:**
```rust
#[async_trait]
pub trait PersistenceBackend: Send + Sync {
    async fn store(&self, memory: &Memory) -> Result<MemoryId>;
    async fn retrieve(&self, id: &MemoryId) -> Result<Option<Memory>>;
    async fn delete(&self, id: &MemoryId) -> Result<bool>;
    async fn list(&self, filter: &PersistenceFilter) -> Result<Vec<Memory>>;
}
```

**Current Implementation - Sync:**
```rust
pub trait PersistenceBackend {
    fn store(&mut self, memory: &Memory) -> Result<()>;
    fn get(&self, id: &MemoryId) -> Result<Option<Memory>>;
    fn delete(&mut self, id: &MemoryId) -> Result<bool>;
    fn list(&self, filter: Option<&SearchFilter>) -> Result<Vec<Memory>>;
}

pub trait IndexBackend {
    fn index(&mut self, memory: &Memory) -> Result<()>;
    fn search(&self, filter: &SearchFilter) -> Result<Vec<MemoryResult>>;
    fn reindex(&mut self, memories: &[Memory]) -> Result<()>;
}

pub trait VectorBackend {
    fn upsert(&mut self, id: &MemoryId, embedding: &[f32]) -> Result<()>;
    fn search(&self, embedding: &[f32], limit: usize) -> Result<Vec<(MemoryId, f32)>>;
    fn rebuild(&mut self, items: &[(MemoryId, Vec<f32>)]) -> Result<()>;
}
```

## Data Flow

### Capture Flow

**Target Design (Spec):**
```
User Input → Security → Embedding → Persistence → Index + Vector (parallel)
```

**Current Implementation:**
```
User Input
    │
    ▼
┌─────────────────┐
│ Security Check  │ ─── Block if secrets detected
│   (secrets)     │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Create Memory   │ ─── Generate MemoryId, add metadata
│    Object       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Git Notes     │ ─── Store to refs/notes/subcog
│     Store       │
└────────┬────────┘
         │
         ▼
    Return URN
```

> **Note**: Embedding generation and index/vector updates are deferred to a separate
> `reindex` operation in the current implementation.

### Search Flow

```
Query
    │
    ├────────────────────────┬────────────────────┐
    ▼                        ▼                    ▼
┌─────────────┐      ┌─────────────┐      ┌─────────────┐
│  Embedding  │      │  BM25 Text  │      │   Filter    │
│   Search    │      │   Search    │      │  (ns, tag)  │
└──────┬──────┘      └──────┬──────┘      └──────┬──────┘
       │                    │                    │
       ▼                    ▼                    ▼
   Vector Results      Text Results        Filtered IDs
       │                    │                    │
       └────────────────────┴────────────────────┘
                            │
                            ▼
                    ┌─────────────────┐
                    │   RRF Fusion    │
                    │ (k=60 constant) │
                    └────────┬────────┘
                             │
                             ▼
                    Ranked Results
```

### Hook Flow

```
Claude Code Event
        │
        ▼
┌─────────────────────┐
│    Hook Handler     │
│  (event dispatch)   │
└──────────┬──────────┘
           │
    ┌──────┴──────┬──────────────┬──────────────┐
    ▼             ▼              ▼              ▼
Session      UserPrompt      PostTool        Stop
Start        Submit          Use             │
│            │               │               │
▼            ▼               ▼               ▼
Context      Intent          Related         Sync +
Injection    Detection       Memories        Summary
             │
             ▼
         Memory
         Surfacing
```

## Configuration

### Config Loading

```
Command Line → Environment → Project → User → System → Defaults
    (highest)                                        (lowest)
```

### Feature Flags

Tier-based feature organization:
- **Core**: Always available
- **Enhanced**: Configuration required
- **LLM-Powered**: LLM provider required

## Error Handling

### Error Types

```rust
#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Capture failed: {0}")]
    Capture(#[source] CaptureError),

    #[error("Memory not found: {0}")]
    NotFound(String),

    #[error("Content blocked: security")]
    ContentBlocked,

    #[error("Storage error: {0}")]
    Storage(#[source] StorageError),

    #[error("LLM error: {0}")]
    Llm(#[source] LlmError),
}
```

### Error Propagation

- Use `Result<T, E>` everywhere
- No panics in library code
- Graceful degradation on failures

## Concurrency

### Async Runtime

**Target Design (Spec):**
Tokio-based async runtime:
- Non-blocking I/O
- Work-stealing scheduler
- Configurable thread pool

**Current Implementation:**
Synchronous execution:
- Blocking I/O for simplicity
- Single-threaded CLI operations
- MCP server uses tokio for transport only

### Synchronization

- Read-heavy workloads: `Mutex` (current) / `RwLock` (target)
- Write coordination: Direct calls (current) / Channel-based (target)
- Atomic operations for counters

## Observability

### Metrics (Prometheus)

- Request latency
- Memory counts
- Search hit rates
- Error rates

### Tracing (OpenTelemetry)

- Distributed trace propagation
- Span-based instrumentation
- OTLP export

### Logging (tracing)

- Structured JSON logging
- Configurable levels
- Context propagation

## See Also

- [Models](models.md) - Data model details
- [Services](services.md) - Service layer details
- [Search](search.md) - Search architecture
- [Security](security.md) - Security features
