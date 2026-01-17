# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Subcog is a persistent memory system for AI coding assistants, written in Rust. It captures decisions, learnings, and context from coding sessions and surfaces them when relevant.

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
```

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

