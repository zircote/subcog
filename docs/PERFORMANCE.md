# Performance Tuning Guide

This guide covers performance optimization for subcog deployments.

## Table of Contents

- [Performance Targets](#performance-targets)
- [Benchmarks](#benchmarks)
- [Storage Tuning](#storage-tuning)
- [Embedding Tuning](#embedding-tuning)
- [Search Tuning](#search-tuning)
- [Memory Management](#memory-management)
- [Hook Optimization](#hook-optimization)
- [Monitoring](#monitoring)

---

## Performance Targets

Subcog is designed to meet these performance targets:

| Metric | Target | Typical |
|--------|--------|---------|
| Cold start | <10ms | ~5ms |
| Capture latency | <30ms | ~25ms |
| Search (100 memories) | <20ms | ~82us |
| Search (1,000 memories) | <50ms | ~413us |
| Search (10,000 memories) | <100ms | ~3.7ms |
| Binary size | <100MB | ~50MB |
| Memory (idle) | <50MB | ~30MB |

These targets are exceeded by 10-100x in typical deployments.

---

## Benchmarks

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench search_intent

# Generate HTML report
cargo bench -- --save-baseline main
```

### Benchmark Results Interpretation

```
search_intent/hybrid_detection
                        time:   [1.2ms 1.3ms 1.4ms]
                        thrpt:  [714.29 ops/s 769.23 ops/s 833.33 ops/s]
```

- **time**: [min, median, max] execution time
- **thrpt**: Operations per second (higher is better)

### Memory Benchmark Results

From LongMemEval benchmark suite:

| Benchmark | With Subcog | Baseline | Notes |
|-----------|-------------|----------|-------|
| LongMemEval | 97% | 0% | Factual recall |
| LoCoMo | 57% | 0% | Personal context |
| ContextBench | 24% | 0% | Multi-turn |
| MemoryAgentBench | 28% | 21% | Agent tasks |

---

## Storage Tuning

### SQLite Optimization

SQLite is the default storage backend. Key tuning parameters:

```bash
# WAL mode (default, best for concurrent reads)
export SUBCOG_SQLITE_JOURNAL_MODE=wal

# Synchronous mode (trade durability for speed)
# - full: safest, slowest
# - normal: good balance (default)
# - off: fastest, risk of corruption on crash
export SUBCOG_SQLITE_SYNCHRONOUS=normal

# Cache size (pages, default 2000 = ~8MB)
export SUBCOG_SQLITE_CACHE_SIZE=4000

# Busy timeout (ms, for lock contention)
export SUBCOG_SQLITE_BUSY_TIMEOUT_MS=5000

# Memory-mapped I/O size (bytes, 0 = disabled)
export SUBCOG_SQLITE_MMAP_SIZE=268435456  # 256MB
```

### SQLite WAL Checkpoint

Large WAL files can slow down reads:

```bash
# Manual checkpoint
sqlite3 .subcog/index.db "PRAGMA wal_checkpoint(TRUNCATE);"

# Auto-checkpoint threshold (pages)
export SUBCOG_SQLITE_WAL_AUTOCHECKPOINT=1000
```

### PostgreSQL Optimization

For multi-user deployments:

```bash
# Connection pool size
export SUBCOG_PG_POOL_SIZE=10
export SUBCOG_PG_POOL_MIN=2

# Statement timeout (ms)
export SUBCOG_PG_STATEMENT_TIMEOUT_MS=30000

# Use prepared statements
export SUBCOG_PG_PREPARED_STATEMENTS=true
```

PostgreSQL server tuning:

```sql
-- Increase work memory for sorting
SET work_mem = '256MB';

-- Increase effective cache size
SET effective_cache_size = '4GB';

-- Optimize for SSDs
SET random_page_cost = 1.1;
```

### Index Optimization

Periodically optimize the FTS index:

```bash
# SQLite FTS5 optimization
sqlite3 .subcog/index.db "INSERT INTO memories_fts(memories_fts) VALUES('optimize');"

# Rebuild index from scratch
subcog migrate embeddings --force
```

---

## Embedding Tuning

### Model Selection

The default model is `all-MiniLM-L6-v2` (384 dimensions):

| Model | Dimensions | Speed | Quality |
|-------|------------|-------|---------|
| all-MiniLM-L6-v2 | 384 | Fast | Good |
| all-MiniLM-L12-v2 | 384 | Medium | Better |
| all-mpnet-base-v2 | 768 | Slow | Best |

```bash
# Use a different model
export SUBCOG_EMBEDDING_MODEL=all-MiniLM-L12-v2
```

### Embedding Cache

Cache embeddings to avoid regeneration:

```bash
# Cache size (number of embeddings)
export SUBCOG_EMBEDDING_CACHE_SIZE=1000

# Cache TTL (seconds, 0 = no expiry)
export SUBCOG_EMBEDDING_CACHE_TTL=3600
```

### Batch Embedding

For large migrations, use batch processing:

```bash
# Batch size for embedding generation
export SUBCOG_EMBEDDING_BATCH_SIZE=32

# Parallel embedding threads
export SUBCOG_EMBEDDING_THREADS=4
```

### GPU Acceleration

FastEmbed supports GPU acceleration on supported hardware:

```bash
# Enable CUDA (if available)
export SUBCOG_EMBEDDING_DEVICE=cuda

# Specific GPU
export SUBCOG_EMBEDDING_DEVICE=cuda:0
```

---

## Search Tuning

### Hybrid Search Parameters

Subcog uses Reciprocal Rank Fusion (RRF) to combine BM25 and vector search:

```bash
# RRF constant k (lower = more weight to top results)
export SUBCOG_RRF_K=60

# Vector weight (0.0-1.0, higher = more semantic)
export SUBCOG_VECTOR_WEIGHT=0.5

# BM25 weight (0.0-1.0, higher = more keyword)
export SUBCOG_BM25_WEIGHT=0.5
```

### Vector Search (HNSW)

usearch HNSW parameters:

```bash
# M parameter (connections per node, higher = better quality, more memory)
export SUBCOG_HNSW_M=16

# ef_construction (index build quality, higher = slower build, better quality)
export SUBCOG_HNSW_EF_CONSTRUCTION=100

# ef_search (search quality, higher = slower search, better recall)
export SUBCOG_HNSW_EF_SEARCH=50
```

### BM25 Tuning

FTS5 BM25 parameters:

```bash
# k1: term frequency saturation (default 1.2)
export SUBCOG_BM25_K1=1.2

# b: length normalization (default 0.75)
export SUBCOG_BM25_B=0.75
```

### Result Limits

Limit results for faster queries:

```bash
# Default result limit
export SUBCOG_DEFAULT_LIMIT=20

# Maximum allowed limit
export SUBCOG_MAX_LIMIT=100
```

---

## Memory Management

### Process Memory

```bash
# Maximum heap size (not strictly enforced)
export SUBCOG_MAX_HEAP_MB=512

# Embedding model memory limit
export SUBCOG_EMBEDDING_MEMORY_MB=200

# Vector index memory limit
export SUBCOG_VECTOR_MEMORY_MB=100
```

### Memory Profiling

```bash
# Enable memory profiling (development only)
RUST_LOG=debug subcog recall "test" 2>&1 | grep -i memory

# Use heaptrack for detailed analysis
heaptrack subcog recall "test query"
heaptrack_gui heaptrack.subcog.*.gz
```

### Reducing Memory Usage

1. **Use lazy initialization**:
   ```bash
   export SUBCOG_LAZY_INIT=true
   ```

2. **Reduce cache sizes**:
   ```bash
   export SUBCOG_EMBEDDING_CACHE_SIZE=100
   export SUBCOG_SQLITE_CACHE_SIZE=1000
   ```

3. **Use disk-backed vector index**:
   ```bash
   export SUBCOG_VECTOR_MMAP=true
   ```

---

## Hook Optimization

### Timeout Settings

Hooks must complete within timeout to avoid blocking Claude Code:

```toml
# ~/.config/subcog/config.toml
[hooks]
# SessionStart can be slower (runs once)
session_start_timeout_ms = 2000

# UserPrompt must be fast (runs on every message)
user_prompt_timeout_ms = 50

# PostToolUse can be moderate
post_tool_use_timeout_ms = 100

# PreCompact can be slower (runs before context compaction)
pre_compact_timeout_ms = 5000

# Stop can be slowest (session end)
stop_timeout_ms = 10000
```

### Hook-Specific Tuning

**SessionStart** (context injection):
```bash
# Limit memories injected
export SUBCOG_SESSION_START_LIMIT=10

# Maximum tokens for context
export SUBCOG_SESSION_START_MAX_TOKENS=2000
```

**UserPromptSubmit** (search intent detection):
```bash
# Use keyword-only detection (faster)
export SUBCOG_SEARCH_INTENT_USE_LLM=false

# Minimum confidence for memory injection
export SUBCOG_SEARCH_INTENT_MIN_CONFIDENCE=0.5
```

**PreCompact** (auto-capture):
```bash
# Enable deduplication (prevents duplicates)
export SUBCOG_DEDUP_ENABLED=true

# Skip semantic similarity check (faster)
export SUBCOG_DEDUP_SEMANTIC_ENABLED=false
```

---

## Monitoring

### Prometheus Metrics

Enable metrics export:

```bash
export SUBCOG_METRICS_ENABLED=true
export SUBCOG_METRICS_PORT=9090
```

Key metrics to monitor:

| Metric | Description | Alert Threshold |
|--------|-------------|-----------------|
| `subcog_capture_duration_ms` | Capture latency | >50ms p99 |
| `subcog_search_duration_ms` | Search latency | >100ms p99 |
| `subcog_embedding_duration_ms` | Embedding generation | >200ms p99 |
| `subcog_memory_count` | Total memories | >50,000 |
| `subcog_index_size_bytes` | Index file size | >1GB |

### OpenTelemetry Tracing

Enable distributed tracing:

```bash
export SUBCOG_OTLP_ENABLED=true
export SUBCOG_OTLP_ENDPOINT=http://localhost:4317

# Service name for traces
export SUBCOG_SERVICE_NAME=subcog-dev
```

### Logging Performance

Reduce log overhead in production:

```bash
# Production logging (errors and warnings only)
export RUST_LOG=warn,subcog=info

# Disable trace-level logs
export RUST_LOG=subcog=info

# Log to file instead of stderr
export SUBCOG_LOG_FILE=/var/log/subcog/subcog.log
```

### Health Checks

```bash
# Quick health check
subcog status --json | jq '.healthy'

# Detailed status
subcog status --verbose

# Check specific components
subcog status --check storage
subcog status --check embedding
subcog status --check vector
```

---

## Production Checklist

Before deploying to production:

- [ ] Set `RUST_LOG=warn,subcog=info`
- [ ] Enable WAL mode for SQLite
- [ ] Configure appropriate timeouts
- [ ] Set memory limits
- [ ] Enable metrics and monitoring
- [ ] Configure log rotation
- [ ] Test with production-like data volume
- [ ] Benchmark hook latencies
- [ ] Set up alerting for key metrics

---

## Troubleshooting Performance

If performance degrades:

1. **Check index size**: Large indexes slow down search
   ```bash
   subcog status | grep -i count
   ```

2. **Check for lock contention**: Multiple processes
   ```bash
   lsof +D .subcog/
   ```

3. **Profile slow queries**:
   ```bash
   RUST_LOG=debug subcog recall "slow query" 2>&1 | grep -i duration
   ```

4. **Check disk I/O**:
   ```bash
   iostat -x 1 5
   ```

5. **Check memory pressure**:
   ```bash
   free -m  # Linux
   vm_stat  # macOS
   ```

See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for more debugging guidance.
