# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-01-02

### Added

#### Real Semantic Embeddings (MEM-001)
- Replaced placeholder hash-based embeddings with real semantic embeddings via fastembed-rs
- Uses all-MiniLM-L6-v2 model (384 dimensions)
- Thread-safe singleton for model loading with lazy initialization
- Model loads on first embed() call to preserve cold start time

#### RecallService Vector Search (MEM-002)
- Added embedder and vector backend fields to RecallService
- Implemented real `vector_search()` with query embedding
- Hybrid search now uses both text (BM25) and vector results
- Graceful degradation when embedder/vector unavailable

#### CaptureService Integration (MEM-003)
- CaptureService now generates embeddings during capture
- Writes to all three storage layers: Git Notes, SQLite FTS5, usearch HNSW
- Non-blocking index/vector operations (capture succeeds even if they fail)

#### Score Normalization (MEM-005)
- All search results now return normalized scores in 0.0-1.0 range
- `--raw` flag in CLI to display original RRF scores
- MCP tools return both normalized `score` and `raw_score` fields
- Score proportions preserved (relative ordering unchanged)

#### Migration Tooling
- New `subcog migrate embeddings` command
- Options: `--dry-run`, `--force`, `--repo`
- MigrationService with progress tracking
- Scans all memories, generates embeddings for those lacking them

#### Performance Benchmarks
- New benchmark suite in `benches/search.rs`
- Benchmarks for 100, 1,000, and 10,000 memories
- All search modes tested (text, vector, hybrid)
- Results far exceed targets:
  - 100 memories: ~82µs (target <20ms)
  - 1,000 memories: ~413µs (target <50ms)
  - 10,000 memories: ~3.7ms (target <100ms)

### Changed

- ServiceContainer now supports `with_embedder()` and `with_vector()` builders
- RecallService constructor signature updated to accept embedder/vector
- CaptureService constructor signature updated to accept all three backends

### Fixed

- RecallService no longer returns empty results when embedder unavailable
- Hybrid search properly combines text and vector results with RRF fusion
- Score normalization handles edge cases (empty results, zero scores)

### Performance

- Search latency: <5ms at 10,000 memories (target was <100ms)
- Capture latency: ~25ms with embedding generation
- Cold start: ~5ms (target was <10ms)
- Binary size: ~50MB (target was <100MB)

## [0.1.0] - 2025-12-28

### Added

- Initial release of Subcog (Rust rewrite)
- Three-layer storage: Git Notes, SQLite FTS5, usearch HNSW
- MCP server integration
- Claude Code hooks (all 5 hooks)
- 10 memory namespaces
- Multi-domain support (project, user, organization)
- Proactive memory surfacing with search intent detection
- Prompt template management
- Deduplication service
