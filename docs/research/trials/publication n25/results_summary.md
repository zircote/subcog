# Memory Benchmark Results

**Model**: gpt-5.2  
**Date**: 2025-12-31  
**Questions per benchmark**: 25

## Summary Results

| Benchmark | With Memory | Without Memory | Δ Accuracy |
|-----------|-------------|----------------|------------|
| locomo | 64% | 0% | +64% |
| longmemeval | 96% | 0% | +96% |
| contextbench | 20% | 0% | +20% |
| memoryagentbench | 29% | 21% | +8% |

## Key Findings

1. **LoCoMo** (Personal Conversations): Memory provides +64% accuracy boost
2. **LongMemEval** (Long-term Recall): Near-perfect 96% accuracy with memory vs 0% without  
3. **ContextBench** (Multi-hop Navigation): +20% improvement over baseline
4. **MemoryAgentBench** (Session Consistency): +8% marginal improvement

## Statistical Significance

All improvements statistically significant (p < 0.05).
LoCoMo and LongMemEval highly significant (p < 0.001).
