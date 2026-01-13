# Graph Memory / Knowledge Graph Feature

**Status**: Active
**Started**: 2026-01-12
**Target Completion**: 2026-02-23 (6 weeks)
**GitHub Issue**: TBD

## Overview

This specification describes the implementation of a Graph Memory / Knowledge Graph feature for Subcog. The feature adds entity extraction, temporal knowledge graphs, and Graph RAG retrieval capabilities.

## Key Features

- **Entity Extraction**: LLM-powered extraction of People, Organizations, Concepts, Technologies, and Files from memories
- **Temporal Knowledge Graph**: Bitemporal tracking (valid_time + transaction_time) for all entities and relationships
- **Graph RAG**: Hybrid search combining vector/BM25 with graph traversal
- **7 MCP Tools**: entities, relationships, graph_query, extract_entities, entity_merge, relationship_infer, graph_visualize
- **Domain Scoping**: Project-scoped entities for project memories, global for user/org

## Architecture Approach

**Pragmatic Balance**: Clean architecture foundations with phased delivery.

- `GraphBackend` trait for backend independence (SQLite now, Neo4j future)
- Separate graph tables (`graph_entities`, `graph_relationships`) from memory tables
- First-class temporal types (`ValidTimeRange`, `TransactionTime`)
- Graceful degradation when LLM unavailable

## Documents

| Document | Description |
|----------|-------------|
| [REQUIREMENTS.md](REQUIREMENTS.md) | Product requirements and user stories |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Technical architecture and design decisions |
| [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) | Phased implementation plan with tasks |
| [DECISIONS.md](DECISIONS.md) | Architecture Decision Records (ADRs) |
| [PROGRESS.md](PROGRESS.md) | Implementation progress tracking |

## Timeline

| Phase | Duration | Status |
|-------|----------|--------|
| Phase 1: Foundation | Week 1-2 | Not Started |
| Phase 2: Services | Week 3 | Not Started |
| Phase 3: MCP Tools | Week 4 | Not Started |
| Phase 4: Graph RAG | Week 5 | Not Started |
| Phase 5: Integration | Week 6 | Not Started |
| Phase 6: Polish | Week 7 | Not Started |

## Success Criteria

- [ ] All graph operations <100ms (p95)
- [ ] Entity extraction accuracy >85% (with LLM)
- [ ] Graph traversal supports depth=3
- [ ] Full bitemporal query support
- [ ] 200+ tests passing
- [ ] `make ci` passes
- [ ] Documentation complete
