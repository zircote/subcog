# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Subcog is a persistent memory system for AI coding assistants, written in Rust. It captures decisions, learnings, and context from coding sessions and surfaces them when relevant. This is a Rust rewrite of the [git-notes-memory](https://github.com/zircote/git-notes-memory) Python system.

### Key Features

- **Single-binary distribution** (<100MB, <10ms cold start)
- **Three-layer storage architecture** (Persistence, Index, Vector)
- **Pluggable backends** (Git Notes, SQLite+usearch, PostgreSQL+pgvector)
- **MCP server integration** for AI agent interoperability
- **Claude Code hooks** for seamless IDE integration
- **Semantic search** with hybrid vector + BM25 ranking (RRF fusion)

## Project Structure

```
src/
├── lib.rs                    # Library entry point
├── main.rs                   # CLI entry point
│
├── models/                   # Data structures
│   ├── mod.rs
│   ├── memory.rs            # Memory, MemoryId, MemoryResult
│   ├── capture.rs           # CaptureResult, CaptureRequest
│   ├── search.rs            # SearchResult, SearchFilter, SearchMode
│   ├── consolidation.rs     # MemoryTier, EdgeType, RetentionScore
│   ├── domain.rs            # Domain, Namespace (10 variants), MemoryStatus
│   └── events.rs            # MemoryEvent variants
│
├── storage/                  # Three-layer storage abstraction
│   ├── mod.rs               # CompositeStorage, layer trait re-exports
│   ├── traits/
│   │   ├── persistence.rs   # PersistenceBackend trait
│   │   ├── index.rs         # IndexBackend trait
│   │   └── vector.rs        # VectorBackend trait
│   ├── persistence/
│   │   ├── git_notes.rs     # Git notes implementation (primary)
│   │   ├── postgresql.rs    # PostgreSQL implementation
│   │   └── filesystem.rs    # Fallback filesystem storage
│   ├── index/
│   │   ├── sqlite.rs        # SQLite + FTS5 implementation
│   │   ├── postgresql.rs    # PostgreSQL full-text
│   │   └── redis.rs         # RediSearch implementation
│   └── vector/
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
│   ├── context.rs           # ContextBuilderService
│   └── topic_index.rs       # TopicIndexService (topic → memory map)
│
├── git/                      # Git operations
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
│   ├── session_start.rs     # Context injection
│   ├── user_prompt.rs       # Signal detection + search intent
│   ├── search_intent.rs     # Search intent detection (6 types)
│   ├── search_context.rs    # Adaptive context building
│   ├── post_tool_use.rs     # Related memory surfacing
│   ├── pre_compact.rs       # Auto-capture before compaction
│   └── stop.rs              # Session analysis, sync
│
├── mcp/                      # MCP server
│   ├── server.rs            # MCP server setup (rmcp)
│   ├── tools.rs             # Tool implementations
│   ├── resources.rs         # Resource handlers (URN scheme)
│   └── prompts.rs           # Pre-defined prompts
│
├── security/                 # Security features
│   ├── secrets.rs           # Secret detection patterns
│   ├── pii.rs               # PII detection
│   ├── redactor.rs          # Content redaction/masking
│   └── audit.rs             # SOC2/GDPR audit logging
│
├── config/                   # Configuration
│   ├── mod.rs               # Config struct, loading
│   └── features.rs          # FeatureFlags
│
├── cli/                      # CLI commands
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
    ├── metrics.rs           # Prometheus metrics
    ├── tracing.rs           # Distributed tracing
    ├── logging.rs           # Structured logging
    └── otlp.rs              # OTLP export

tests/
└── integration_test.rs      # Integration tests

benches/
└── search_intent.rs         # Performance benchmarks

docs/spec/active/2025-12-28-subcog-rust-rewrite/
├── README.md                # Spec overview
├── REQUIREMENTS.md          # Product requirements
├── ARCHITECTURE.md          # Technical architecture
├── IMPLEMENTATION_PLAN.md   # Phased implementation
├── DECISIONS.md             # Architecture decision records
└── PROGRESS.md              # Implementation progress
```

## Build Commands

This project uses [Cargo](https://doc.rust-lang.org/cargo/) as the build system.

```bash
# Build the project
cargo build

# Build with optimizations
cargo build --release

# Run tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run benchmarks
cargo bench

# Run linting
cargo clippy --all-targets --all-features

# Format code
cargo fmt

# Check formatting
cargo fmt -- --check

# Generate documentation
cargo doc --open

# Check supply chain security
cargo deny check

# Run with MIRI (undefined behavior detection)
cargo +nightly miri test

# Run all checks (lint + format + test + doc + deny)
cargo fmt -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test && cargo doc --no-deps && cargo deny check
```

## CLI Usage

```bash
# Capture a memory
subcog capture --namespace decisions "Use PostgreSQL for primary storage"

# Search memories
subcog recall "database storage decision"

# Check status
subcog status

# Sync with git remote
subcog sync

# Run as MCP server
subcog serve

# Hook commands (called by Claude Code)
subcog hook session-start
subcog hook user-prompt-submit
subcog hook post-tool-use
subcog hook pre-compact
subcog hook stop
```

## Proactive Memory Surfacing

The proactive memory surfacing system automatically detects search intent in user prompts and injects relevant memories into the context. This enables the AI assistant to leverage prior decisions, patterns, and learnings without explicit recall commands.

### Search Intent Detection

When a user prompt is processed, the system detects one of six intent types:

| Intent Type | Trigger Patterns | Example |
|-------------|-----------------|---------|
| **HowTo** | "how do I...", "how to...", "implement...", "create..." | "How do I implement authentication?" |
| **Location** | "where is...", "find...", "locate..." | "Where is the database config?" |
| **Explanation** | "what is...", "explain...", "describe..." | "What is the ServiceContainer?" |
| **Comparison** | "difference between...", "vs", "compare..." | "PostgreSQL vs SQLite?" |
| **Troubleshoot** | "error...", "fix...", "not working...", "debug..." | "Why is this test failing?" |
| **General** | "search...", "show me..." | "Search for recent decisions" |

Detection uses a hybrid approach:
1. **Keyword detection**: Fast pattern matching (<10ms)
2. **LLM classification**: Enhanced accuracy with 200ms timeout
3. **Hybrid mode**: Combines both for best results

### Adaptive Memory Injection

Based on detected intent confidence:

| Confidence | Memory Count | Behavior |
|------------|--------------|----------|
| ≥ 0.8 (high) | 15 memories | Full context injection |
| ≥ 0.5 (medium) | 10 memories | Standard injection |
| < 0.5 (low) | 5 memories | Minimal injection |

Namespace weights are applied based on intent type:
- **HowTo**: Prioritizes `patterns`, `learnings`
- **Troubleshoot**: Prioritizes `blockers`, `learnings`
- **Explanation/Location**: Prioritizes `decisions`, `context`
- **Comparison**: Prioritizes `decisions`, `patterns`

### MCP Resources

New MCP resources for topic-based access:

```
subcog://topics              # List all topics with memory counts
subcog://topics/{topic}      # Get memories for a specific topic
subcog://namespaces          # List all namespaces
subcog://namespaces/{ns}     # Get memories in a namespace
```

### MCP Prompts

New prompts for context-aware operations:

| Prompt | Description |
|--------|-------------|
| `search_with_context` | Search with intent-aware weighting |
| `research_topic` | Deep-dive into a topic with related memories |
| `capture_decision` | Capture with guided namespace selection |

### Configuration

Environment variables for search intent:

```bash
SUBCOG_SEARCH_INTENT_ENABLED=true       # Enable/disable detection
SUBCOG_SEARCH_INTENT_USE_LLM=true       # Enable LLM classification
SUBCOG_SEARCH_INTENT_LLM_TIMEOUT_MS=200 # LLM timeout in milliseconds
SUBCOG_SEARCH_INTENT_MIN_CONFIDENCE=0.5 # Minimum confidence threshold
```

### Graceful Degradation

The system degrades gracefully when components are unavailable:

- **LLM unavailable**: Falls back to keyword-only detection
- **Embeddings down**: Falls back to text search (BM25)
- **Index down**: Skips memory injection, continues processing
- **Low confidence**: Reduces memory count, may skip injection

## Code Style Requirements

This project uses **clippy** with pedantic and nursery lints, and **rustfmt** for formatting.

### Key Rules

- **Line length**: 100 characters
- **Edition**: 2024
- **MSRV**: 1.85
- **Unsafe code**: Forbidden unless explicitly justified
- **Panics**: Not allowed in library code (`unwrap`, `expect`, `panic!`)

### Error Handling

Always use `Result` types for fallible operations. Never panic in library code:

```rust
// Good - Returns Result
pub fn parse(input: &str) -> Result<Value, ParseError> {
    if input.is_empty() {
        return Err(ParseError::EmptyInput);
    }
    // parsing logic
    Ok(value)
}

// Bad - Panics
pub fn parse(input: &str) -> Value {
    input.parse().unwrap() // Never do this in library code
}
```

Use `thiserror` for custom error types:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Capture failed: {0}")]
    Capture(#[source] CaptureError),

    #[error("Memory not found: {0}")]
    NotFound(String),

    #[error("Security: content blocked")]
    ContentBlocked,

    #[error("Feature not enabled: {0}")]
    FeatureNotEnabled(String),
}
```

### Documentation

All public items must have documentation with examples:

```rust
/// Captures a memory to persistent storage.
///
/// # Arguments
///
/// * `request` - The capture request containing content and metadata.
///
/// # Returns
///
/// A [`CaptureResult`] with the memory ID and URN.
///
/// # Errors
///
/// Returns [`MemoryError::ContentBlocked`] if secrets are detected.
///
/// # Examples
///
/// ```rust
/// use subcog::{CaptureService, CaptureRequest, Namespace};
///
/// let service = CaptureService::new(config)?;
/// let result = service.capture(CaptureRequest {
///     namespace: Namespace::Decisions,
///     content: "Use PostgreSQL".to_string(),
///     ..Default::default()
/// })?;
/// assert!(!result.memory_id.is_empty());
/// # Ok::<(), subcog::MemoryError>(())
/// ```
pub async fn capture(&self, request: CaptureRequest) -> Result<CaptureResult, MemoryError> {
    // implementation
}
```

### Ownership and Borrowing

Prefer borrowing over ownership:

```rust
// Good - borrows
pub fn process(data: &[u8]) -> Vec<u8> { ... }

// Avoid - takes ownership unnecessarily
pub fn process(data: Vec<u8>) -> Vec<u8> { ... }
```

Use `Cow` for flexible string handling:

```rust
use std::borrow::Cow;

pub fn normalize(s: &str) -> Cow<'_, str> {
    if s.contains(' ') {
        Cow::Owned(s.replace(' ', "_"))
    } else {
        Cow::Borrowed(s)
    }
}
```

### Builder Pattern

Use builder pattern for complex configuration:

```rust
#[derive(Debug, Clone, Default)]
pub struct Config {
    timeout: Duration,
    retries: u32,
}

impl Config {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            retries: 3,
        }
    }

    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    #[must_use]
    pub const fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }
}
```

## Testing Conventions

- **Unit tests**: Inside `src/*.rs` with `#[cfg(test)]` modules
- **Integration tests**: `tests/` directory
- **Doc tests**: Examples in documentation
- **Property tests**: Use `proptest` for property-based testing

### Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_case() {
        let result = function_under_test(valid_input);
        assert_eq!(result, expected_output);
    }

    #[test]
    fn test_error_case() {
        let result = function_under_test(invalid_input);
        assert!(matches!(result, Err(MemoryError::NotFound(_))));
    }
}
```

### Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn property_holds(input in any::<i64>()) {
        prop_assert!(predicate(input));
    }
}
```

## Linting Configuration

Clippy is configured to deny:
- `unwrap_used`, `expect_used`, `panic` - Use Result instead
- `todo`, `unimplemented` - Complete implementation
- `dbg_macro`, `print_stdout`, `print_stderr` - Use proper logging

## Supply Chain Security

This project uses `cargo-deny` to audit dependencies:
- **Advisories**: Deny crates with known vulnerabilities
- **Licenses**: Only allow permissive licenses (MIT, Apache-2.0, BSD)
- **Bans**: Block specific problematic crates
- **Sources**: Only allow crates.io

## Architecture Guidelines

### Three-Layer Storage

1. **Persistence Layer** (Authoritative): Git Notes (primary), PostgreSQL, Filesystem
2. **Index Layer** (Searchable): SQLite + FTS5, PostgreSQL full-text, RediSearch
3. **Vector Layer** (Embeddings): usearch HNSW, pgvector, Redis vector

### Feature Tiers

| Tier | Features | Requirements |
|------|----------|--------------|
| **Core** | Capture, search, git notes, CLI | None |
| **Enhanced** | Secrets filtering, multi-domain, audit | Configuration |
| **LLM-Powered** | Auto-capture, consolidation, temporal | LLM provider |

### Design Principles

1. **Zero-cost abstractions**: Prefer compile-time over runtime overhead
2. **Explicit over implicit**: No hidden allocations or side effects
3. **Error propagation**: Use `?` operator, avoid `.unwrap()`
4. **Const by default**: Use `const fn` where possible
5. **Minimal dependencies**: Only add what's truly needed
6. **Graceful degradation**: Features fail open with fallbacks

## Performance Targets

| Metric | Target |
|--------|--------|
| Cold start | <10ms |
| Capture latency | <30ms |
| Search latency | <50ms |
| Binary size | <100MB |
| Memory (idle) | <50MB |

## CI/CD

The CI pipeline includes:
1. **Format check**: `cargo fmt -- --check`
2. **Lint**: `cargo clippy --all-targets --all-features`
3. **Test**: `cargo test --all-features` (ubuntu, macos, windows)
4. **Documentation**: `cargo doc --no-deps`
5. **Supply chain**: `cargo deny check`
6. **MSRV check**: Rust 1.85
7. **Coverage**: Generate code coverage reports

## LSP Integration

This project is configured with rust-analyzer LSP for enhanced code intelligence.

### Available LSP Operations

Use the LSP tool for semantic code navigation:

| Operation | Use Case |
|-----------|----------|
| `goToDefinition` | Jump to where a symbol is defined |
| `findReferences` | Find all usages of a symbol |
| `hover` | Get type info and documentation |
| `documentSymbol` | List all symbols in a file |
| `workspaceSymbol` | Search symbols across the project |
| `goToImplementation` | Find trait implementations |

### LSP Workflow

When working with Rust code:

1. **Before modifying**: Use `findReferences` to understand impact
2. **For refactoring**: Use `goToDefinition` to trace dependencies
3. **For traits**: Use `goToImplementation` to find all implementors
4. **For exploration**: Use `documentSymbol` to understand file structure

### Hooks Configured

The following hooks run automatically on file save:
- `rustfmt` - Auto-formats code to project standards
- `cargo check` - Fast compilation checking
- `cargo clippy` - Lint warnings and suggestions

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` | CLI argument parsing |
| `rmcp` | MCP server implementation |
| `fastembed` | Embedding generation (all-MiniLM-L6-v2) |
| `usearch` | HNSW vector similarity search |
| `rusqlite` | SQLite + FTS5 for indexing |
| `git2` | Git operations |
| `serde` / `serde_json` / `serde_yml` | Serialization |
| `tokio` | Async runtime |
| `tracing` | Observability |
| `thiserror` / `anyhow` | Error handling |

## Specification Documents

### Active Specifications

Full specification in `docs/spec/active/2025-12-28-subcog-rust-rewrite/`:

- [REQUIREMENTS.md](docs/spec/active/2025-12-28-subcog-rust-rewrite/REQUIREMENTS.md) - Product requirements
- [ARCHITECTURE.md](docs/spec/active/2025-12-28-subcog-rust-rewrite/ARCHITECTURE.md) - Technical architecture
- [IMPLEMENTATION_PLAN.md](docs/spec/active/2025-12-28-subcog-rust-rewrite/IMPLEMENTATION_PLAN.md) - Phased implementation
- [DECISIONS.md](docs/spec/active/2025-12-28-subcog-rust-rewrite/DECISIONS.md) - Architecture decision records
- [PROGRESS.md](docs/spec/active/2025-12-28-subcog-rust-rewrite/PROGRESS.md) - Implementation progress
- always run `make ci` before commiting or declaring success ensuring all gates pass

### Completed Specifications

- **[Proactive Memory Surfacing](docs/spec/completed/2025-12-30-issue-15-memory-surfacing/)** (2025-12-30)
  - **Issues**: [#15](https://github.com/zircote/subcog/issues/15), [#24](https://github.com/zircote/subcog/issues/24)
  - **PR**: [#23](https://github.com/zircote/subcog/pull/23)
  - **Outcome**: Success - All 77 tasks delivered, 388 tests passing
  - **Features**:
    - Search intent detection (6 types: HowTo, Location, Explanation, Comparison, Troubleshoot, General)
    - Hybrid detection (keyword <10ms + optional LLM <200ms)
    - Namespace weighting for intent-specific prioritization
    - 3 new MCP resources (search, topics, topics/{topic})
    - 6 new MCP prompts (intent_search, query_suggest, discover, generate_decision, generate_tutorial, context_capture)
    - Hook response format fixes (all 5 hooks now compliant with Claude Code spec)
  - **Key Learnings**:
    - Rust `map_or_else` preferred over `if let Some(...)` per clippy::option_if_let_else
    - Claude Code hook format: `{hookSpecificOutput: {hookEventName, additionalContext}}`
    - HashSet deduplication requires `mut` AND `.insert()` call
    - Graceful degradation testing critical for production readiness