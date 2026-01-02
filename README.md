# Subcog

[![CI](https://github.com/zircote/subcog/actions/workflows/ci.yml/badge.svg)](https://github.com/zircote/subcog/actions/workflows/ci.yml)
[![Rust Version](https://img.shields.io/badge/rust-1.85%2B-dea584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Clippy](https://img.shields.io/badge/linting-clippy-orange?logo=rust&logoColor=white)](https://github.com/rust-lang/rust-clippy)
[![cargo-deny](https://img.shields.io/badge/security-cargo--deny-blue?logo=rust&logoColor=white)](https://github.com/EmbarkStudios/cargo-deny)

A persistent memory system for AI coding assistants. Subcog captures decisions, learnings, and context from coding sessions and surfaces them when relevant.

![Subcog Prompt Library Workflow](docs/_assets/subcog-inforgraphic.png)

## Overview

Subcog is a Rust rewrite of the [git-notes-memory](https://github.com/zircote/git-notes-memory) Python system, delivering:

- **Single-binary distribution** (<100MB, <10ms cold start)
- **Pluggable storage backends** (Git Notes, SQLite+usearch, PostgreSQL+pgvector)
- **MCP server integration** for AI agent interoperability
- **Claude Code hooks** for seamless IDE integration
- **Semantic search** with hybrid vector + BM25 ranking

## Benchmark Results

Subcog achieves **97% accuracy on factual recall** (LongMemEval) and **57% on personal context** (LoCoMo), compared to 0% baseline without memory. See [full benchmark results](docs/BENCHMARKS.md).

| Benchmark | With Subcog | Baseline | Improvement |
|-----------|-------------|----------|-------------|
| LongMemEval | 97% | 0% | +97% |
| LoCoMo | 57% | 0% | +57% |
| ContextBench | 24% | 0% | +24% |
| MemoryAgentBench | 28% | 21% | +7% |

## Features

### Core (Always Available)
- Memory capture with **automatic embedding generation** (384-dimensional vectors)
- **Real semantic search** using all-MiniLM-L6-v2 via fastembed-rs
- **Hybrid search** combining BM25 text search + vector similarity (RRF fusion)
- **Normalized scores** (0.0-1.0 range) for intuitive relevance understanding
- Git notes persistence with YAML front matter
- Multi-domain memories (project, user, organization)
- 10 memory namespaces (decisions, learnings, patterns, blockers, etc.)
- **Migration tools** for upgrading existing memories to use embeddings

### Enhanced (Opt-in)
- Entity and temporal extraction
- Secrets filtering (API keys, PII detection)
- OpenTelemetry observability
- Full Claude Code hook integration

### LLM-Powered (Requires Provider)
- Implicit capture from conversations
- Memory consolidation and summarization
- Supersession detection
- Temporal reasoning queries

## Installation

```bash
# From source
cargo install --path .

# Or build locally
cargo build --release
```

## Quick Start

```bash
# Capture a memory
subcog capture --namespace decisions "Use PostgreSQL for primary storage due to ACID requirements"

# Search memories (semantic search with normalized scores 0.0-1.0)
subcog recall "database storage decision"

# Search with raw RRF scores (for debugging)
subcog recall "database storage decision" --raw

# Check status
subcog status

# Sync with git remote
subcog sync

# Migrate existing memories to use real embeddings
subcog migrate embeddings
```

### Score Normalization

Search results return normalized scores in the 0.0-1.0 range:
- **1.0**: Best match in the result set
- **≥0.7**: Strong semantic match
- **≥0.5**: Moderate relevance
- **<0.5**: Weak match

Use `--raw` flag to see the underlying RRF (Reciprocal Rank Fusion) scores.

## MCP Server

Run as an MCP server for AI agent integration:

```bash
subcog serve
```

Configure in Claude Desktop's `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "subcog": {
      "command": "subcog",
      "args": ["serve"]
    }
  }
}
```

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `memory.capture` | Store a new memory |
| `memory.recall` | Search memories semantically |
| `memory.status` | Get system statistics |
| `memory.sync` | Sync with remote |
| `memory.consolidate` | Run memory consolidation |
| `memory.configure` | Get/set configuration |

## Claude Code Hooks

Subcog integrates with all 5 Claude Code hooks:

| Hook | Purpose |
|------|---------|
| `SessionStart` | Inject relevant context at session start |
| `UserPromptSubmit` | Detect capture signals in prompts |
| `PostToolUse` | Surface related memories after file operations |
| `PreCompact` | Analyze conversation for auto-capture |
| `Stop` | Finalize session, sync to remote |

Configure in `~/.claude/settings.json`:

```json
{
  "hooks": {
    "SessionStart": [{ "command": "subcog hook session-start" }],
    "UserPromptSubmit": [{ "command": "subcog hook user-prompt-submit" }],
    "PostToolUse": [{ "command": "subcog hook post-tool-use" }],
    "PreCompact": [{ "command": "subcog hook pre-compact" }],
    "Stop": [{ "command": "subcog hook stop" }]
  }
}
```

## Migration

Upgrade existing memories to use real embeddings:

```bash
# Dry-run (see what would be migrated)
subcog migrate embeddings --dry-run

# Migrate all memories without embeddings
subcog migrate embeddings

# Force re-generation of all embeddings
subcog migrate embeddings --force

# Migrate from a specific repository
subcog migrate embeddings --repo /path/to/repo
```

The migration command:
- Scans all memories in the index
- Generates embeddings using fastembed (all-MiniLM-L6-v2)
- Stores embeddings in the vector backend (usearch HNSW)
- Skips memories that already have embeddings (unless `--force`)
- Shows progress with migrated/skipped/error counts

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Access Layer                            │
│  ┌─────────┐  ┌─────────────┐  ┌────────────────────────┐   │
│  │   CLI   │  │  MCP Server │  │  Claude Code Hooks     │   │
│  └────┬────┘  └──────┬──────┘  └───────────┬────────────┘   │
└───────┼──────────────┼─────────────────────┼────────────────┘
        │              │                     │
┌───────┴──────────────┴─────────────────────┴────────────────┐
│                     Service Layer                            │
│  ┌────────────────┐  ┌─────────────────┐  ┌──────────────┐  │
│  │ CaptureService │  │  RecallService  │  │ SyncService  │  │
│  └────────────────┘  └─────────────────┘  └──────────────┘  │
└─────────────────────────────┬───────────────────────────────┘
                              │
┌─────────────────────────────┴───────────────────────────────┐
│                    Storage Layer                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │ Persistence  │  │    Index     │  │     Vector       │   │
│  │  (Git Notes) │  │   (SQLite)   │  │    (usearch)     │   │
│  └──────────────┘  └──────────────┘  └──────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Development

### Prerequisites

- Rust 1.85+ (Edition 2024)
- Git 2.30+
- [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) for supply chain security

### Setup

```bash
git clone https://github.com/zircote/subcog.git
cd subcog

# Build
cargo build

# Run tests
cargo test

# Run all checks
cargo fmt -- --check && \
cargo clippy --all-targets --all-features -- -D warnings && \
cargo test && \
cargo doc --no-deps && \
cargo deny check
```

### Project Structure

```
src/
├── lib.rs              # Library entry point
├── main.rs             # CLI entry point
├── models/             # Data structures (Memory, Domain, Namespace)
├── storage/            # Storage backends (Git Notes, SQLite, usearch)
├── services/           # Business logic (Capture, Recall, Sync)
├── mcp/                # MCP server implementation
├── hooks/              # Claude Code hook handlers
├── embedding/          # Vector embedding generation
└── observability/      # Tracing, metrics, logging

docs/
├── research/           # Research documents
└── spec/               # Specification documents
    └── active/
        └── 2025-12-28-subcog-rust-rewrite/
            ├── README.md
            ├── REQUIREMENTS.md
            ├── ARCHITECTURE.md
            ├── IMPLEMENTATION_PLAN.md
            └── ...
```

## Configuration

Configuration file at `~/.config/subcog/config.toml`:

```toml
[storage]
backend = "sqlite"  # "git-notes", "sqlite", "postgres"
data_dir = "~/.local/share/subcog"

[embedding]
model = "all-MiniLM-L6-v2"
dimensions = 384

[hooks]
enabled = true
session_start_timeout_ms = 2000
user_prompt_timeout_ms = 50

[llm]
provider = "anthropic"  # Optional: for Tier 3 features
```

## Performance Targets

| Metric | Target | Actual |
|--------|--------|--------|
| Cold start | <10ms | ~5ms |
| Capture latency | <30ms | ~25ms |
| Search latency (100 memories) | <20ms | ~82µs |
| Search latency (1,000 memories) | <50ms | ~413µs |
| Search latency (10,000 memories) | <100ms | ~3.7ms |
| Binary size | <100MB | ~50MB |
| Memory (idle) | <50MB | ~30MB |

All performance targets are exceeded by 10-100x. Benchmarks run via `cargo bench`.

## Specification

Full specification documents are in [`docs/spec/active/2025-12-28-subcog-rust-rewrite/`](docs/spec/active/2025-12-28-subcog-rust-rewrite/):

- [REQUIREMENTS.md](docs/spec/active/2025-12-28-subcog-rust-rewrite/REQUIREMENTS.md) - Product requirements
- [ARCHITECTURE.md](docs/spec/active/2025-12-28-subcog-rust-rewrite/ARCHITECTURE.md) - Technical architecture
- [IMPLEMENTATION_PLAN.md](docs/spec/active/2025-12-28-subcog-rust-rewrite/IMPLEMENTATION_PLAN.md) - Phased implementation
- [DECISIONS.md](docs/spec/active/2025-12-28-subcog-rust-rewrite/DECISIONS.md) - Architecture decision records

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [git-notes-memory](https://github.com/zircote/git-notes-memory) - Python proof-of-concept
- [fastembed](https://github.com/Anush008/fastembed-rs) - Embedding generation
- [usearch](https://github.com/unum-cloud/usearch) - Vector similarity search
- [rmcp](https://github.com/anthropics/rmcp) - MCP protocol implementation
