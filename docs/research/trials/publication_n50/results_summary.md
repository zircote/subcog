# Memory Benchmark Results (n=50)

**Model**: gpt-5.2  
**Date**: 2025-12-31  
**Questions per benchmark**: 50

## Summary Results

| Benchmark | With Memory | Without Memory | Δ Accuracy |
|-----------|-------------|----------------|------------|
| locomo | 50% | 0% | +50% |
| longmemeval | 98% | 0% | +98% |
| contextbench | 28% | 0% | +28% |
| memoryagentbench | 27% | 21% | +6% |
| **Mean** | **51%** | **5%** | **+46%** |

## Key Findings

1. **LongMemEval**: 98% accuracy with memory - near-perfect factual recall
2. **LoCoMo**: 50% with memory (+50% improvement) - personal conversation understanding  
3. **ContextBench**: 28% with memory (+28%) - multi-hop reasoning across documents
4. **MemoryAgentBench**: 27% vs 21% (+6%) - marginal improvement on session consistency
