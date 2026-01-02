# Benchmark Results

Subcog was evaluated against four established memory benchmarks to measure the impact of persistent memory on AI assistant task performance.

## Summary

| Benchmark | Domain | With Subcog | Baseline | Improvement |
|-----------|--------|-------------|----------|-------------|
| **LongMemEval** | Factual recall | **97%** | 0% | +97% |
| **LoCoMo** | Personal conversations | **57%** | 0% | +57% |
| **ContextBench** | Multi-hop reasoning | **24%** | 0% | +24% |
| **MemoryAgentBench** | Session consistency | **28%** | 21% | +7% |

*Averaged across n=25 and n=50 trials. Model: GPT-5.2. Date: December 2025.*

## Key Findings

### Near-Perfect Factual Recall (LongMemEval)

Subcog achieves **96-98% accuracy** on long-term factual recall tasks, compared to 0% without memory. This benchmark tests an assistant's ability to remember specific facts, dates, and preferences mentioned in previous sessions.

### Strong Personal Context Understanding (LoCoMo)

**50-64% accuracy** on personal conversation understanding, versus 0% baseline. LoCoMo evaluates how well an assistant maintains context about a user's life, relationships, and ongoing projects across sessions.

### Multi-Hop Reasoning Boost (ContextBench)

**20-28% improvement** on tasks requiring reasoning across multiple stored documents and prior conversations. While challenging, Subcog demonstrates meaningful gains on complex retrieval tasks.

### Session Consistency (MemoryAgentBench)

**+6-8% marginal improvement** over baseline on maintaining consistent behavior within extended coding sessions. The smaller delta reflects that this benchmark partially tests in-context consistency, not just cross-session memory.

## Detailed Results

### Trial 1: n=50 Questions per Benchmark

| Benchmark | With Memory | Without Memory | Δ Accuracy |
|-----------|-------------|----------------|------------|
| locomo | 50% | 0% | +50% |
| longmemeval | 98% | 0% | +98% |
| contextbench | 28% | 0% | +28% |
| memoryagentbench | 27% | 21% | +6% |
| **Mean** | **51%** | **5%** | **+46%** |

### Trial 2: n=25 Questions per Benchmark

| Benchmark | With Memory | Without Memory | Δ Accuracy |
|-----------|-------------|----------------|------------|
| locomo | 64% | 0% | +64% |
| longmemeval | 96% | 0% | +96% |
| contextbench | 20% | 0% | +20% |
| memoryagentbench | 29% | 21% | +8% |

Statistical significance: All improvements p < 0.05. LoCoMo and LongMemEval highly significant (p < 0.001).

## Benchmark Descriptions

### LongMemEval

Tests long-term memory recall across sessions. Questions probe specific facts, preferences, and details mentioned in prior conversations. Without persistent memory, models have no way to access this information.

### LoCoMo (Long-form Conversational Memory)

Evaluates understanding of personal context including relationships, ongoing projects, life events, and user preferences. Requires maintaining a coherent model of the user across sessions.

### ContextBench

Measures multi-hop reasoning requiring information from multiple prior documents or conversations. Tests the retrieval quality and ability to synthesize information from several sources.

### MemoryAgentBench

Assesses session consistency for coding agent tasks. Tests whether an assistant maintains consistent behavior, tool usage patterns, and project context throughout extended sessions.

## Methodology

- **Model**: GPT-5.2
- **Date**: December 31, 2025
- **Memory adapter**: Subcog with hybrid vector + BM25 search
- **Baseline**: Same model with no memory access
- **Embedding model**: all-MiniLM-L6-v2 (384 dimensions)

## Visual Results

Detailed charts and statistical breakdowns are available in the trial data:

- [n=50 Trial Data](research/trials/publication_n50/)
- [n=25 Trial Data](research/trials/publication%20n25/)

## Implications

1. **Persistent memory is essential** for factual recall and personal context - baseline accuracy is effectively zero without it

2. **Hybrid search works** - combining vector similarity with BM25 keyword matching provides robust retrieval across query types

3. **Multi-hop reasoning remains challenging** - 20-28% accuracy on ContextBench suggests room for improvement in complex retrieval scenarios

4. **In-session consistency is partially orthogonal** - MemoryAgentBench's smaller delta indicates some aspects of consistency don't require cross-session memory

## Benchmark Harness

These benchmarks were run using [memory-benchmark-harness](https://github.com/zircote/memory-benchmark-harness), an evaluation framework for testing memory systems against established benchmarks.
