# Memory Consolidation Benchmark

## Overview

This benchmark verifies that the memory consolidation service preserves all factual details from source memories during the summarization process. It measures **detail loss** - the percentage of specific facts that cannot be retrieved after consolidation.

**Target**: 0% detail loss on Information Extraction queries

## What It Tests

The benchmark validates the core acceptance criterion from the spec:

> "Benchmark shows no detail loss on Information Extraction queries"

### Test Methodology

1. **Capture Phase**: Captures specific, verifiable facts as individual memories
2. **Consolidation Phase**: Runs consolidation to create summary nodes
3. **Verification Phase**: Searches for each fact and verifies it's still retrievable
4. **Measurement**: Calculates detail loss percentage and reports extraction accuracy

### Test Data Sets

The benchmark includes three test data sets with specific, verifiable facts:

#### 1. Redis Architecture Facts (5 facts)
- Session storage configuration (TTL: 30 minutes)
- AOF persistence with fsync frequency
- Memory eviction policy (allkeys-lru, 2GB limit)
- Replication configuration (2 read replicas)
- Monitoring setup (Prometheus on port 9121)

#### 2. Database Migration Facts (3 facts)
- Migration tool selection (Flyway with versioned SQL)
- Rollback strategy (down migrations for emergency revert)
- Testing requirements (staging before production)

#### 3. Combined Facts (8 facts)
- All Redis and Database facts together

### Mock LLM Provider

The benchmark uses `DetailPreservingMockLlm` - a mock LLM that:
- Extracts all content from source memories
- Creates summaries that preserve all key facts
- Simulates realistic LLM behavior
- Supports configurable latency for performance testing

This mock ensures the benchmark measures the consolidation service's ability to preserve details, not the LLM's summarization quality.

## Running the Benchmark

### Basic Usage

```bash
# Run all consolidation benchmarks
cargo bench --bench consolidation

# Run specific benchmark group
cargo bench --bench consolidation -- "detail_preservation"
cargo bench --bench consolidation -- "scalability"
cargo bench --bench consolidation -- "llm_latency"

# Save baseline for comparison
cargo bench --bench consolidation -- --save-baseline main

# Compare against baseline
cargo bench --bench consolidation -- --baseline main
```

### Generating Reports

Criterion generates HTML reports in `target/criterion/`:

```bash
# Run benchmark and view report
cargo bench --bench consolidation
open target/criterion/consolidation_detail_preservation/report/index.html
```

## Benchmark Groups

### 1. Detail Preservation (`consolidation_detail_preservation`)

Measures detail loss across different fact sets:

- `redis_facts_5`: 5 facts about Redis architecture
- `db_migration_facts_3`: 3 facts about database migrations
- `combined_facts_8`: All 8 facts together
- `throughput_fact_verification`: Facts verified per second

**Metrics Reported**:
- Time per consolidation + verification cycle
- Detail loss percentage (custom metric, calculated manually)
- Facts lost count (custom metric, calculated manually)

**Expected Results**:
- Detail loss: **0%**
- All facts should be retrievable through search

### 2. Scalability (`consolidation_scalability`)

Tests consolidation performance with varying memory counts:

- 5 memories
- 10 memories
- 20 memories
- 50 memories

**Metrics Reported**:
- Time per consolidation
- Throughput (memories processed per second)
- Scaling behavior (should be roughly linear)

**Expected Results**:
- Performance scales linearly with memory count
- No degradation at higher memory counts

### 3. LLM Latency Impact (`llm_latency_impact`)

Tests consolidation performance with varying LLM latencies:

- 0ms (instant mock)
- 50ms (fast LLM)
- 100ms (typical LLM)
- 200ms (slow LLM)

**Metrics Reported**:
- Total consolidation time
- Impact of LLM latency on overall performance

**Expected Results**:
- Consolidation time includes LLM latency
- Other operations (grouping, storage) are fast relative to LLM calls

## Interpreting Results

### Detail Loss Calculation

The benchmark calculates detail loss as:

```
detail_loss = (facts_lost / total_facts) × 100%
```

Where:
- `facts_lost`: Number of facts not retrievable after consolidation
- `total_facts`: Total number of captured facts

### Success Criteria

✅ **PASS**: Detail loss = 0% (all facts retrievable)
⚠️  **WARNING**: Detail loss < 5% (minor information loss)
❌ **FAIL**: Detail loss ≥ 5% (significant information loss)

### Example Output

```
consolidation_detail_preservation/redis_facts_5
                        time:   [142.35 ms 145.82 ms 149.67 ms]
                        thrpt:  [33.402 elem/s 34.287 elem/s 35.125 elem/s]

consolidation_detail_preservation/db_migration_facts_3
                        time:   [87.234 ms 89.156 ms 91.342 ms]
                        thrpt:  [32.837 elem/s 33.653 elem/s 34.385 elem/s]

consolidation_detail_preservation/combined_facts_8
                        time:   [234.12 ms 238.45 ms 243.21 ms]
                        thrpt:  [32.884 elem/s 33.551 elem/s 34.175 elem/s]
```

## Manual Verification

To manually verify detail preservation:

```bash
# 1. Run benchmark in verbose mode
RUST_LOG=debug cargo bench --bench consolidation -- redis_facts_5 --verbose

# 2. Check the generated summaries contain all facts
# Look for lines like:
#   - Use Redis for session storage with TTL of 30 minutes
#   - Configure Redis with AOF persistence, fsync every second
#   - Set maxmemory-policy to allkeys-lru with 2GB limit
#   - Enable Redis replication with 2 read replicas for scaling
#   - Monitor Redis with Prometheus metrics exported on port 9121

# 3. Verify search results include all facts
# The benchmark logs search queries and results
```

## Troubleshooting

### Benchmark Fails with "fact not found"

**Cause**: Search is not finding consolidated memories

**Solution**:
1. Check if consolidation created summary nodes
2. Verify embeddings are generated for summaries
3. Ensure search index includes summary content
4. Check similarity thresholds in ConsolidationConfig

### High Detail Loss Percentage

**Cause**: Facts being lost during summarization or not indexed properly

**Solution**:
1. Review mock LLM summary generation logic
2. Check if search terms match summary content
3. Verify tags are preserved from source memories
4. Ensure summary nodes have embeddings

### Slow Performance

**Cause**: Multiple factors can slow consolidation

**Solution**:
1. Check LLM latency (use 0ms mock for baseline)
2. Verify index backend performance
3. Review memory grouping algorithm complexity
4. Check if embeddings are being generated efficiently

## Implementation Details

### Files

- `benches/consolidation.rs`: Main benchmark implementation
- `Cargo.toml`: Benchmark registration
- `CONSOLIDATION_BENCHMARK.md`: This documentation

### Key Functions

- `bench_consolidation_detail_preservation()`: Main detail loss benchmark
- `bench_consolidation_scalability()`: Scalability testing
- `bench_llm_latency_impact()`: LLM performance impact
- `consolidate_and_verify_facts()`: Core verification logic
- `DetailPreservingMockLlm`: Mock LLM for testing

### Dependencies

- `criterion`: Benchmarking framework
- `tempfile`: Temporary directories for test data
- `subcog`: Core consolidation service

## Future Improvements

Potential enhancements for the benchmark:

1. **Real LLM Testing**: Option to test with actual LLM providers
2. **Fuzzy Matching**: Allow approximate fact matching (semantic similarity)
3. **Fact Complexity**: Test with more complex, multi-part facts
4. **Cross-Namespace**: Test consolidation across multiple namespaces
5. **Temporal Decay**: Test with time-based memory filtering
6. **Contradiction Detection**: Verify contradictory facts are flagged

## Related Documentation

- [Consolidation Service](./src/services/consolidation.rs)
- [Integration Tests](./tests/integration_test.rs)
- [Unit Tests Summary](./CONSOLIDATION_TESTS_SUMMARY.md)
