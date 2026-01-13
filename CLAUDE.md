# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Subcog is a persistent memory system for AI coding assistants, written in Rust. It captures decisions, learnings, and context from coding sessions and surfaces them when relevant. This is a Rust rewrite of the [git-notes-memory](https://github.com/zircote/git-notes-memory) Python system.

### Key Capabilities

- **Single-binary distribution** (<100MB, <10ms cold start)
- **Three-layer storage architecture**: Persistence (SQLite), Index (FTS5), Vector (usearch HNSW)
- **MCP server integration** for AI agent interoperability
- **Claude Code hooks** for seamless IDE integration
- **Semantic search** with hybrid vector + BM25 ranking (RRF fusion)

## Build Commands

**Use the Makefile** for all common operations. Run `make help` for the full list.

```bash
# Primary development workflow
make ci              # Run ALL CI gates before committing (format, lint, test, doc, deny, msrv, bench)
make dev             # Full check + install to ~/.cargo/bin
make quick           # Build + install (skip tests)

# Individual operations
make build           # Debug build
make release         # Optimized build
make test            # Run all tests
make test-verbose    # Tests with output (--nocapture)
make lint            # Clippy (warnings allowed)
make lint-strict     # Clippy with warnings as errors
make format          # Auto-format code
make format-check    # Check formatting without changes
make deny            # Supply chain security audit
make bench           # Quick benchmark validation
make bench-full      # Full performance benchmarks
```

### Running Specific Tests

```bash
# Run tests matching a pattern
cargo test capture              # Tests with "capture" in name
cargo test services::recall     # Tests in recall service module
cargo test --test integration   # Integration tests only
cargo test --lib                # Library tests only (faster)

# Run with feature flags
cargo test --all-features       # All features enabled
cargo test --features postgres  # PostgreSQL backend tests
cargo test --no-default-features # Core only

# Debug test output
cargo test test_name -- --nocapture --test-threads=1
```

### Running Benchmarks

```bash
cargo bench                           # All benchmarks
cargo bench --bench search_intent     # Specific benchmark
cargo bench -- "semantic"             # Filter by name pattern
```

## Architecture

### Three-Layer Storage

```
┌─────────────────────────────────────────────────────────────────┐
│                      Service Layer                              │
│  CaptureService │ RecallService │ SyncService │ GraphService    │
└─────────────────────────┬───────────────────────────────────────┘
                          │
    ┌─────────────────────┼─────────────────────┐
    │                     │                     │
┌───▼───────────┐  ┌──────▼──────────┐  ┌───────▼──────────┐
│  Persistence  │  │     Index       │  │     Vector       │
│    Layer      │  │     Layer       │  │     Layer        │
├───────────────┤  ├─────────────────┤  ├──────────────────┤
│ - Authoritative│  │ - BM25 search   │  │ - Embeddings     │
│ - ACID storage │  │ - FTS5          │  │   (384-dim)      │
│ - SQLite/PG    │  │ - Faceted       │  │ - HNSW ANN       │
└───────────────┘  └─────────────────┘  └──────────────────┘
```

### Key Modules

| Directory | Purpose |
|-----------|---------|
| `src/models/` | Data structures: `Memory`, `Namespace`, `Domain`, `CaptureRequest` |
| `src/storage/` | Backend traits and implementations (SQLite, PostgreSQL, usearch) |
| `src/services/` | Business logic: `CaptureService`, `RecallService`, `ConsolidationService` |
| `src/mcp/` | MCP server: JSON-RPC tools, resources, prompts |
| `src/hooks/` | Claude Code hooks: session-start, user-prompt, pre-compact, stop |
| `src/embedding/` | Vector embeddings via FastEmbed (all-MiniLM-L6-v2) |
| `src/llm/` | LLM providers: Anthropic, OpenAI, Ollama |

### Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `fastembed-embeddings` | Real semantic embeddings via ONNX | Off |
| `usearch-hnsw` | Native HNSW vector search | Off |
| `postgres` | PostgreSQL backend | Off |
| `postgres-tls` | PostgreSQL with TLS | Off |
| `redis` | Redis caching | Off |
| `http` | HTTP transport for MCP | Off |
| `encryption` | AES-256-GCM at rest | Off |
| `full` | All features | Off |

## Code Style

- **Edition**: 2024
- **MSRV**: 1.88
- **Line length**: 100 characters
- **Linting**: clippy with pedantic + nursery lints

### Strict Requirements

1. **No panics in library code**: Use `Result` types. `unwrap`, `expect`, `panic!` are denied by clippy.
2. **Use `thiserror`** for custom error types with `#[error(...)]` attributes.
3. **Prefer `const fn`** where possible (clippy enforces this for simple methods).
4. **Prefer borrowing** over ownership: `&str` over `String`, `&[T]` over `Vec<T>`.
5. **Document public items** with `///` including `# Errors` and `# Examples` sections.

### Example Error Type

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Memory not found: {0}")]
    NotFound(String),

    #[error("Capture failed: {0}")]
    Capture(#[source] CaptureError),

    #[error(transparent)]
    Storage(#[from] StorageError),
}
```

### Example Result Handling

```rust
// Good - propagate errors with ?
pub fn process(id: &str) -> Result<Memory, MemoryError> {
    let memory = storage.get(id)?;
    Ok(memory)
}

// Bad - panics
pub fn process(id: &str) -> Memory {
    storage.get(id).unwrap() // DENIED by clippy
}
```

## Testing

- **Unit tests**: Inside source files with `#[cfg(test)]` modules
- **Integration tests**: `tests/integration_test.rs`
- **Property tests**: Use `proptest` for invariant testing
- **Benchmarks**: `benches/` directory with Criterion

### Test Patterns

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success() {
        let result = function(valid_input);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_error() {
        let result = function(invalid_input);
        assert!(matches!(result, Err(Error::NotFound(_))));
    }
}
```

### Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn roundtrip(input in any::<String>()) {
        let encoded = encode(&input);
        let decoded = decode(&encoded)?;
        prop_assert_eq!(input, decoded);
    }
}
```

## CLI Usage

```bash
# Capture a memory
subcog capture --namespace decisions "Use PostgreSQL for primary storage"

# Search memories
subcog recall "database storage decision"

# Check status
subcog status

# Run as MCP server
subcog serve

# Hook commands (called by Claude Code)
subcog hook session-start
subcog hook user-prompt-submit
subcog hook post-tool-use
subcog hook pre-compact
subcog hook stop
```

## MCP Server

The MCP server exposes tools for memory operations:

| Tool | Description |
|------|-------------|
| `subcog_capture` | Store a new memory with namespace and tags |
| `subcog_recall` | Search memories semantically |
| `subcog_status` | Get system statistics |
| `subcog_consolidate` | Run memory consolidation |
| `prompt_save/get/run/delete` | Prompt template management |

## Performance Targets

| Metric | Target |
|--------|--------|
| Cold start | <10ms |
| Capture latency | <30ms |
| Search latency | <50ms |
| Binary size | <100MB |
| Memory (idle) | <50MB |

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` | CLI argument parsing |
| `rmcp` | MCP server (JSON-RPC) |
| `fastembed` | Embedding generation (all-MiniLM-L6-v2) |
| `usearch` | HNSW vector search |
| `rusqlite` | SQLite + FTS5 indexing |
| `tokio` | Async runtime |
| `tracing` | Observability |
| `thiserror` | Error types |

## Supply Chain Security

Uses `cargo-deny` to audit dependencies:

- **Advisories**: Deny crates with known vulnerabilities
- **Licenses**: MIT, Apache-2.0, BSD only
- **Sources**: crates.io only
- **Bans**: openssl (use rustls), atty (use std)

Run `cargo deny check` or `make deny` to audit.

## Active Specifications

Specification documents live in `docs/spec/`:

- **Active work**: `docs/spec/active/` - Current implementation tasks
- **Completed**: `docs/spec/completed/` - Historical reference with retrospectives

When working on a spec, always run `make ci` before declaring success.
