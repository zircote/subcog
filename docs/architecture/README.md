# Architecture

Subcog is a persistent memory system for AI coding assistants, built with a modular, layered architecture.

## Overview

| Topic | Description |
|-------|-------------|
| [Overview](./overview.md) | High-level architecture |
| [Data Models](models.md) | Core data structures |
| [Services](services.md) | Business logic layer |
| [Search](search.md) | Hybrid search system |
| [Security](security.md) | Security and privacy |

## System Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                    Access Layer                              │
│  ┌─────────┐  ┌──────────┐  ┌──────────────────────────────┐ │
│  │   CLI   │  │   MCP    │  │      Claude Code Hooks       │ │
│  │         │  │  Server  │  │ (session, prompt, tool, stop)│ │
│  └────┬────┘  └────┬─────┘  └──────────────┬───────────────┘ │
└───────┼────────────┼────────────────────────┼────────────────┘
        │            │                        │
        ▼            ▼                        ▼
┌──────────────────────────────────────────────────────────────┐
│                    Service Layer                             │
│  ┌───────────┐ ┌────────────┐ ┌────────────┐ ┌───────────┐   │
│  │  Capture  │ │   Recall   │ │   Prompt   │ │   Sync    │   │
│  │  Service  │ │  Service   │ │  Service   │ │  Service  │   │
│  └─────┬─────┘ └─────┬──────┘ └─────┬──────┘ └─────┬─────┘   │
│        │             │              │              │         │
│  ┌─────┴─────────────┴──────────────┴──────────────┴──────┐  │
│  │              ServiceContainer (DI)                     │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────┐
│                    Storage Layer                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐           │
│  │ Persistence │  │    Index    │  │   Vector    │           │
│  │   (Truth)   │  │  (Search)   │  │(Embeddings) │           │
│  ├─────────────┤  ├─────────────┤  ├─────────────┤           │
│  │ • Git Notes │  │ • SQLite    │  │ • usearch   │           │
│  │ • PostgreSQL│  │ • PostgreSQL│  │ • pgvector  │           │
│  │ • Filesystem│  │ • Redis     │  │ • Redis     │           │
│  └─────────────┘  └─────────────┘  └─────────────┘           │
└──────────────────────────────────────────────────────────────┘
```

## Key Design Principles

### 1. Three-Layer Storage

Separation of concerns for durability, searchability, and semantics:
- **Persistence**: Source of truth, durable, syncable
- **Index**: Fast text search, can be rebuilt
- **Vector**: Semantic search, can be rebuilt

### 2. Pluggable Backends

Each layer supports multiple backends:
```rust
CompositeStorage<P: PersistenceBackend, I: IndexBackend, V: VectorBackend>
```

### 3. Progressive Disclosure

Token-efficient responses:
- List endpoints → Minimal data
- Fetch endpoints → Full content
- Search → Configurable detail level

### 4. Graceful Degradation

System continues when components fail:
- LLM unavailable → Keyword search
- Embeddings down → Text search
- Index corrupted → Rebuild from persistence

### 5. Domain Scoping

Multi-level organization:
- Project → Repository-specific
- User → Personal global
- Org → Team-wide

## Performance Targets

| Metric | Target |
|--------|--------|
| Cold start | <10ms |
| Capture latency | <30ms |
| Search latency | <50ms |
| Binary size | <100MB |
| Memory (idle) | <50MB |

## Technology Stack

| Component | Technology |
|-----------|------------|
| Language | Rust (Edition 2024) |
| MCP Server | rmcp crate |
| Embeddings | FastEmbed (all-MiniLM-L6-v2) |
| Vector Index | usearch (HNSW) |
| Text Search | SQLite + FTS5 |
| Serialization | serde (JSON, YAML) |
| Async Runtime | tokio |
| CLI | clap |
| Error Handling | thiserror, anyhow |

## Module Structure

```
src/
├── models/          # Data structures
├── storage/         # Three-layer storage
├── services/        # Business logic
├── mcp/             # MCP server
├── hooks/           # Claude Code hooks
├── cli/             # CLI commands
├── embedding/       # Embedding generation
├── llm/             # LLM provider abstraction
├── security/        # Security features
├── config/          # Configuration
└── observability/   # Metrics, tracing, logging
```

## See Also

- [Overview](./overview.md) - Detailed architecture overview
- [Storage](../storage/README.md) - Storage layer details
- [MCP](../mcp/README.md) - MCP integration
- [Hooks](../hooks/README.md) - Claude Code hooks
