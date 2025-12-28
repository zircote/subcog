# Architectural Decision Records

This document captures key architectural decisions for the Subcog Rust rewrite project.

---

## ADR Index

| ID | Title | Status | Date |
|----|-------|--------|------|
| ADR-001 | Rust as Implementation Language | Accepted | 2025-12-28 |
| ADR-002 | Three-Layer Storage Architecture | Accepted | 2025-12-28 |
| ADR-003 | Feature Tier System | Accepted | 2025-12-28 |
| ADR-004 | Event Bus for Cross-Component Communication | Accepted | 2025-12-28 |
| ADR-005 | URN Scheme for Memory Addressing | Accepted | 2025-12-28 |
| ADR-006 | Git Notes as Primary Persistence | Accepted | 2025-12-28 |
| ADR-007 | fastembed for Embedding Generation | Accepted | 2025-12-28 |
| ADR-008 | usearch for Vector Search | Accepted | 2025-12-28 |
| ADR-009 | rmcp for MCP Server Implementation | Accepted | 2025-12-28 |
| ADR-010 | OpenTelemetry for Observability | Accepted | 2025-12-28 |

---

## ADR-001: Rust as Implementation Language

### Status
Accepted

### Context
The Python implementation of git-notes-memory served as a successful proof-of-concept, achieving 90%+ test coverage and validating the core architecture. However, the Python version has inherent limitations:

- Requires Python runtime, pip, and virtualenv for distribution
- ~500ms+ startup time with model loading
- GIL limitations for true parallelism
- Runtime type errors vs compile-time guarantees

### Decision
Rewrite the entire system in Rust.

### Consequences

**Positive:**
- Single static binary distribution (<100MB)
- ~10ms cold start (50x improvement)
- True parallelism with async/await (tokio)
- Memory safety via compiler guarantees
- Predictable memory usage (no GC pauses)
- Type safety at compile time

**Negative:**
- Longer initial development time
- Steeper learning curve
- Smaller ecosystem for some AI/ML libraries
- Compilation time for development iteration

### Rationale
The performance and distribution benefits outweigh the development overhead. The Python POC validated the architecture, so the Rust rewrite can focus on implementation quality rather than architectural exploration.

---

## ADR-002: Three-Layer Storage Architecture

### Status
Accepted

### Context
Memory storage requires three distinct capabilities:
1. **Persistence**: Authoritative, durable storage
2. **Index**: Fast metadata and full-text search
3. **Vector**: Embedding storage and KNN similarity search

A monolithic storage backend would couple these concerns and limit flexibility.

### Decision
Implement storage as three independent, pluggable layers with trait-based abstraction:

```rust
trait PersistenceBackend { /* ... */ }
trait IndexBackend { /* ... */ }
trait VectorBackend { /* ... */ }
```

Each layer can have multiple implementations:
- **Persistence**: Git Notes, PostgreSQL, Filesystem
- **Index**: SQLite (FTS5), PostgreSQL, Redis (RediSearch)
- **Vector**: usearch, pgvector, Redis

### Consequences

**Positive:**
- Backend selection via configuration
- Mix-and-match deployment options
- Independent scaling of each layer
- Easier testing with mock implementations
- Future backends without code changes

**Negative:**
- More complex architecture
- Cross-layer coordination overhead
- Potential consistency challenges
- Configuration complexity

### Rationale
The flexibility to support git notes for local development while allowing PostgreSQL+pgvector for team deployments is essential. The complexity is justified by the deployment flexibility.

---

## ADR-003: Feature Tier System

### Status
Accepted

### Context
The memory system has features with varying dependencies:
- Core features work standalone
- Enhanced features need local processing
- LLM features require external API access

Users must be able to run with minimal dependencies.

### Decision
Implement a three-tier feature architecture:

**Tier 1: Core (always available, zero external dependencies)**
- Memory capture and storage
- Semantic search (vector + BM25)
- Git notes persistence
- Multi-domain memories
- MCP tools

**Tier 2: Enhanced (opt-in, no external services)**
- Entity extraction
- Temporal extraction
- Secrets filtering
- Hook system
- Advanced observability

**Tier 3: LLM-Powered (opt-in, requires LLM provider)**
- Implicit capture
- Consolidation
- Supersession detection
- Temporal reasoning
- Query expansion

### Consequences

**Positive:**
- Minimal dependency for basic usage
- Explicit opt-in for advanced features
- Graceful degradation when services unavailable
- Clear documentation of requirements

**Negative:**
- Feature flag management complexity
- Conditional code paths
- Testing all tier combinations

### Rationale
Users should not be forced to configure an LLM provider just to capture and search memories. The tier system makes this explicit.

---

## ADR-004: Event Bus for Cross-Component Communication

### Status
Accepted

### Context
Features need to communicate state changes:
- Capture should trigger index updates
- Search should notify metrics
- Consolidation should update tiers

Direct coupling between components creates tight dependencies.

### Decision
Implement a central event bus using tokio broadcast channels:

```rust
pub enum MemoryEvent {
    MemoryCaptured { /* ... */ },
    SearchCompleted { /* ... */ },
    SyncCompleted { /* ... */ },
    // ...
}

pub struct EventBus {
    sender: broadcast::Sender<MemoryEvent>,
}
```

Components subscribe to events they care about and react asynchronously.

### Consequences

**Positive:**
- Loose coupling between components
- Easy addition of new handlers
- Centralized event logging
- Testable in isolation

**Negative:**
- Eventual consistency (async)
- Debugging event chains
- Potential event storms

### Rationale
The event bus pattern is well-established for this type of integration. tokio broadcast provides the right semantics (multiple receivers, async).

---

## ADR-005: URN Scheme for Memory Addressing

### Status
Accepted

### Context
Memories need unique, stable identifiers that:
- Work across domains (project, user, org)
- Are human-readable
- Support MCP resource addressing
- Enable linking between memories

### Decision
Adopt the URN scheme: `subcog://mem/{domain}/{namespace}/{id}`

Examples:
- `subcog://mem/project:my-app/decisions/abc1234:0`
- `subcog://mem/user/learnings/def5678:1`
- `subcog://mem/org:acme-corp/patterns/ghi9012:0`

### Consequences

**Positive:**
- Consistent addressing across interfaces
- MCP resource compatibility
- Human-readable format
- Domain hierarchy encoded
- Supports URI parsing libraries

**Negative:**
- Longer than simple IDs
- Encoding required for special characters
- Version not encoded (may need extension)

### Rationale
The URN scheme provides a foundation for MCP resources and future API extensions. The format is intuitive and parseable.

---

## ADR-006: Git Notes as Primary Persistence

### Status
Accepted

### Context
Memory storage needs:
- Durability
- Version history
- Remote synchronization
- Merge capability

Options considered:
1. SQLite with custom sync
2. Git notes
3. PostgreSQL
4. File-based with custom format

### Decision
Use git notes as the primary persistence layer:
- Notes attached to commits in `refs/notes/mem/{namespace}`
- YAML front matter for metadata
- Markdown body for content
- Cat-sort-uniq merge strategy

### Consequences

**Positive:**
- Built-in version history
- Free remote sync via git push/pull
- Works with existing git workflows
- No additional infrastructure

**Negative:**
- Git dependency
- Performance at scale (10K+ memories)
- Complex merge conflicts
- Not suitable for multi-user concurrent writes

### Rationale
Git notes provide free versioning and sync while staying local-first. The scaling limitations are acceptable for the target use case (individual developers, small teams).

---

## ADR-007: fastembed for Embedding Generation

### Status
Accepted

### Context
The system needs to generate text embeddings for semantic search. Options:
1. Call external API (OpenAI, Cohere)
2. Use Python bridge (sentence-transformers)
3. Use native Rust library (fastembed)

### Decision
Use fastembed crate with all-MiniLM-L6-v2 model (384 dimensions).

### Consequences

**Positive:**
- No external API dependency for core features
- Works offline
- Single binary (model embedded or cached)
- Fast inference

**Negative:**
- Model download on first use
- Fixed model (no custom training)
- Larger binary size

### Rationale
Local embedding generation aligns with the local-first philosophy. fastembed provides the best native Rust experience.

---

## ADR-008: usearch for Vector Search

### Status
Accepted

### Context
Vector similarity search options:
1. sqlite-vec (SQLite extension)
2. usearch (standalone HNSW)
3. lance (columnar + vector)
4. qdrant (server)

### Decision
Use usearch for the default SQLite+usearch backend:
- HNSW algorithm for approximate KNN
- Single file persistence
- 384 dimensions (MiniLM)
- Cosine similarity

### Consequences

**Positive:**
- Excellent performance (<10ms for 10K vectors)
- Small memory footprint
- No external server
- Well-maintained

**Negative:**
- Separate file from SQLite
- No ACID guarantees
- Requires manual sync

### Rationale
usearch provides the best performance/simplicity tradeoff for local usage. The lack of ACID is acceptable given git notes as authoritative.

---

## ADR-009: rmcp for MCP Server Implementation

### Status
Accepted

### Context
The system needs to implement an MCP (Model Context Protocol) server. Options:
1. Custom JSON-RPC implementation
2. Use rmcp crate (Rust MCP SDK)

### Decision
Use rmcp crate for MCP server implementation:
- Built-in JSON-RPC handling
- stdio transport
- Tool, resource, and prompt support
- Subscription support

### Consequences

**Positive:**
- Standards-compliant implementation
- Reduced boilerplate
- Active development
- Feature complete

**Negative:**
- External dependency
- API may change
- Less control over internals

### Rationale
rmcp provides a clean SDK that handles protocol details, allowing focus on business logic.

---

## ADR-010: OpenTelemetry for Observability

### Status
Accepted

### Context
The system requires comprehensive observability:
- Distributed tracing
- Metrics collection
- Structured logging
- OTLP export

### Decision
Use the OpenTelemetry ecosystem:
- `tracing` crate for instrumentation
- `opentelemetry` for OTLP export
- `tracing-subscriber` for formatting
- Prometheus endpoint for metrics

### Consequences

**Positive:**
- Industry standard
- Vendor-neutral
- Excellent Rust support
- Unified tracing/metrics

**Negative:**
- Configuration complexity
- Runtime overhead
- Large dependency tree

### Rationale
OpenTelemetry is the industry standard for observability. The Rust ecosystem has mature support through the tracing family of crates.

---

## Pending Decisions

### PDR-001: Streaming API Implementation
**Status**: Pending Phase 3

**Options**:
1. axum with SSE
2. axum with WebSocket
3. Both SSE and WebSocket

**Considerations**:
- SSE simpler for one-way streaming
- WebSocket for bidirectional
- Browser compatibility

---

### PDR-002: LM Studio vs Ollama Priority
**Status**: Pending Phase 5

**Options**:
1. Implement Ollama first
2. Implement LM Studio first
3. Implement both simultaneously

**Considerations**:
- Ollama more popular
- LM Studio better UX
- Both OpenAI-compatible

---

## Decision Template

```markdown
## ADR-XXX: [Title]

### Status
[Proposed | Accepted | Deprecated | Superseded by ADR-XXX]

### Context
[What is the issue that we're seeing that is motivating this decision?]

### Decision
[What is the change that we're actually making?]

### Consequences
**Positive:**
- [List benefits]

**Negative:**
- [List drawbacks]

### Rationale
[Why is this the best choice among alternatives?]
```

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-28 | Claude Opus 4.5 | Initial ADRs from research documents |
