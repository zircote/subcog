# Graph Memory Implementation Plan

## Overview

This document outlines the phased implementation of the Graph Memory feature.

**Total Estimated Effort**: 180-250 hours (5-6 weeks)
**Approach**: Pragmatic Balance - Clean architecture foundations with phased delivery

## Phase 1: Foundation (Week 1-2)

**Goal**: Core data structures, traits, and SQLite backend

### Tasks

| ID | Task | Est. Hours | Status |
|----|------|------------|--------|
| 1.1 | Create `src/models/graph.rs` with Entity, Relationship, EntityType, RelationshipType | 4 | Not Started |
| 1.2 | Create `src/models/temporal.rs` with ValidTimeRange, TransactionTime | 2 | Not Started |
| 1.3 | Update `src/models/mod.rs` to export graph and temporal modules | 0.5 | Not Started |
| 1.4 | Create `src/storage/traits/graph.rs` with GraphBackend trait | 4 | Not Started |
| 1.5 | Update `src/storage/traits/mod.rs` to export GraphBackend | 0.5 | Not Started |
| 1.6 | Create database migration for graph tables | 3 | Not Started |
| 1.7 | Create `src/storage/graph/mod.rs` module | 0.5 | Not Started |
| 1.8 | Implement `SqliteGraphBackend` - entity CRUD | 8 | Not Started |
| 1.9 | Implement `SqliteGraphBackend` - relationship CRUD | 6 | Not Started |
| 1.10 | Implement `SqliteGraphBackend` - mention CRUD | 4 | Not Started |
| 1.11 | Implement `SqliteGraphBackend` - graph traversal | 8 | Not Started |
| 1.12 | Implement `SqliteGraphBackend` - temporal queries | 4 | Not Started |
| 1.13 | Create `InMemoryGraphBackend` for testing | 4 | Not Started |
| 1.14 | Write unit tests for entity CRUD (20+ tests) | 4 | Not Started |
| 1.15 | Write unit tests for relationship CRUD (20+ tests) | 4 | Not Started |
| 1.16 | Write unit tests for graph traversal (15+ tests) | 3 | Not Started |
| 1.17 | Write unit tests for temporal queries (10+ tests) | 2 | Not Started |

**Phase 1 Total**: 61.5 hours

### Deliverables

- [ ] `Entity`, `Relationship`, `EntityType`, `RelationshipType` types
- [ ] `ValidTimeRange`, `TransactionTime` temporal types
- [ ] `GraphBackend` trait with full interface
- [ ] `SqliteGraphBackend` implementation
- [ ] `InMemoryGraphBackend` for testing
- [ ] Database migrations for `graph_entities`, `graph_relationships`, `graph_entity_mentions`
- [ ] 65+ unit tests passing

### Files to Create

| File | LOC | Description |
|------|-----|-------------|
| `src/models/graph.rs` | 400 | Entity, Relationship, query types |
| `src/models/temporal.rs` | 150 | Bitemporal time types |
| `src/storage/traits/graph.rs` | 200 | GraphBackend trait |
| `src/storage/graph/mod.rs` | 30 | Module exports |
| `src/storage/graph/sqlite.rs` | 800 | SQLite implementation |
| `src/storage/graph/memory.rs` | 300 | In-memory implementation |
| `src/storage/migrations/009_graph_schema.sql` | 80 | Schema migration |

### Files to Modify

| File | Changes |
|------|---------|
| `src/models/mod.rs` | Add `pub mod graph; pub mod temporal;` |
| `src/storage/traits/mod.rs` | Add `pub mod graph;` |
| `src/storage/mod.rs` | Add graph backend exports |

---

## Phase 2: Services (Week 3)

**Goal**: High-level service layer with LLM integration

### Tasks

| ID | Task | Est. Hours | Status |
|----|------|------------|--------|
| 2.1 | Create `src/services/graph.rs` - GraphService | 6 | Not Started |
| 2.2 | Create `src/services/entity_extraction.rs` - EntityExtractorService | 8 | Not Started |
| 2.3 | Add `ENTITY_EXTRACTION_PROMPT` to `src/llm/system_prompt.rs` | 3 | Not Started |
| 2.4 | Implement LLM response parsing for entities | 4 | Not Started |
| 2.5 | Implement entity deduplication logic | 4 | Not Started |
| 2.6 | Implement relationship inference with LLM | 6 | Not Started |
| 2.7 | Integrate with ServiceContainer | 2 | Not Started |
| 2.8 | Add graceful degradation (LLM unavailable) | 3 | Not Started |
| 2.9 | Write integration tests for GraphService (15+ tests) | 4 | Not Started |
| 2.10 | Write integration tests for EntityExtractor (15+ tests) | 4 | Not Started |

**Phase 2 Total**: 44 hours

### Deliverables

- [ ] `GraphService` with CRUD and traversal operations
- [ ] `EntityExtractorService` with LLM extraction
- [ ] `ENTITY_EXTRACTION_PROMPT` system prompt
- [ ] Graceful degradation when LLM unavailable
- [ ] ServiceContainer integration
- [ ] 30+ integration tests passing

### Files to Create

| File | LOC | Description |
|------|-----|-------------|
| `src/services/graph.rs` | 400 | GraphService |
| `src/services/entity_extraction.rs` | 500 | EntityExtractorService |

### Files to Modify

| File | Changes |
|------|---------|
| `src/llm/system_prompt.rs` | Add ENTITY_EXTRACTION_PROMPT |
| `src/services/mod.rs` | Export graph services |
| `src/services/backend_factory.rs` | Add graph backend factory |

---

## Phase 3: MCP Tools (Week 4)

**Goal**: Expose all 7 MCP tools

### Tasks

| ID | Task | Est. Hours | Status |
|----|------|------------|--------|
| 3.1 | Add graph tool definitions to `definitions.rs` | 3 | Not Started |
| 3.2 | Create `src/mcp/tools/handlers/graph.rs` module | 2 | Not Started |
| 3.3 | Implement `subcog_entities` handler | 3 | Not Started |
| 3.4 | Implement `subcog_relationships` handler | 3 | Not Started |
| 3.5 | Implement `subcog_graph_query` handler | 4 | Not Started |
| 3.6 | Implement `subcog_extract_entities` handler | 4 | Not Started |
| 3.7 | Implement `subcog_entity_merge` handler | 3 | Not Started |
| 3.8 | Implement `subcog_relationship_infer` handler | 4 | Not Started |
| 3.9 | Implement `subcog_graph_visualize` handler | 4 | Not Started |
| 3.10 | Register tools in MCP server | 2 | Not Started |
| 3.11 | Write MCP tool tests (20+ tests) | 5 | Not Started |

**Phase 3 Total**: 37 hours

### Deliverables

- [ ] 7 MCP tools implemented and registered
- [ ] Tool input validation
- [ ] Error handling with meaningful messages
- [ ] 20+ MCP tool tests passing

### Files to Create

| File | LOC | Description |
|------|-----|-------------|
| `src/mcp/tools/handlers/graph.rs` | 600 | Tool handlers |

### Files to Modify

| File | Changes |
|------|---------|
| `src/mcp/tools/definitions.rs` | Add 7 tool definitions |
| `src/mcp/tools/handlers/mod.rs` | Export graph handlers |
| `src/mcp/tools/mod.rs` | Register graph tools |
| `src/mcp/tool_types.rs` | Add argument structs |

---

## Phase 4: Graph RAG (Week 5)

**Goal**: Hybrid search with graph expansion

### Tasks

| ID | Task | Est. Hours | Status |
|----|------|------------|--------|
| 4.1 | Create `src/services/graph_rag.rs` | 8 | Not Started |
| 4.2 | Implement entity extraction from search queries | 4 | Not Started |
| 4.3 | Implement graph expansion algorithm | 6 | Not Started |
| 4.4 | Implement result merging and re-ranking | 4 | Not Started |
| 4.5 | Integrate with RecallService | 3 | Not Started |
| 4.6 | Add configuration for expansion parameters | 2 | Not Started |
| 4.7 | Write benchmarks for Graph RAG (5+ benchmarks) | 4 | Not Started |
| 4.8 | Write integration tests (15+ tests) | 4 | Not Started |

**Phase 4 Total**: 35 hours

### Deliverables

- [ ] `GraphRAGService` with hybrid search
- [ ] Graph expansion algorithm (configurable depth)
- [ ] Result merging with provenance tracking
- [ ] Performance benchmarks
- [ ] 15+ integration tests

### Files to Create

| File | LOC | Description |
|------|-----|-------------|
| `src/services/graph_rag.rs` | 500 | GraphRAGService |
| `benches/graph_rag.rs` | 150 | Performance benchmarks |

### Files to Modify

| File | Changes |
|------|---------|
| `src/services/recall.rs` | Add Graph RAG integration point |
| `src/services/mod.rs` | Export GraphRAGService |

---

## Phase 5: Integration (Week 6)

**Goal**: Auto-extraction hook and CLI commands

### Tasks

| ID | Task | Est. Hours | Status |
|----|------|------------|--------|
| 5.1 | Add auto-extraction to CaptureService | 4 | Not Started |
| 5.2 | Add `SUBCOG_GRAPH_AUTO_EXTRACT` config flag | 2 | Not Started |
| 5.3 | Create `src/cli/graph.rs` module | 2 | Not Started |
| 5.4 | Implement `subcog graph entities` command | 3 | Not Started |
| 5.5 | Implement `subcog graph query` command | 3 | Not Started |
| 5.6 | Implement `subcog graph extract` command | 3 | Not Started |
| 5.7 | Implement `subcog graph visualize` command | 3 | Not Started |
| 5.8 | Add graph metrics (Prometheus) | 3 | Not Started |
| 5.9 | Write CLI tests (10+ tests) | 3 | Not Started |
| 5.10 | Write end-to-end integration tests (10+ tests) | 4 | Not Started |

**Phase 5 Total**: 30 hours

### Deliverables

- [ ] Auto-extraction on capture (opt-in)
- [ ] CLI commands for graph operations
- [ ] Prometheus metrics for observability
- [ ] 20+ tests (CLI + e2e)

### Files to Create

| File | LOC | Description |
|------|-----|-------------|
| `src/cli/graph.rs` | 350 | CLI commands |

### Files to Modify

| File | Changes |
|------|---------|
| `src/services/capture.rs` | Add auto-extraction hook |
| `src/config/mod.rs` | Add GraphConfig |
| `src/cli/mod.rs` | Register graph subcommand |
| `src/observability/metrics.rs` | Add graph metrics |

---

## Phase 6: Polish (Week 7)

**Goal**: Documentation, optimization, and specification completion

### Tasks

| ID | Task | Est. Hours | Status |
|----|------|------------|--------|
| 6.1 | Update CLAUDE.md with graph features section | 4 | Not Started |
| 6.2 | Add graph examples to CLI help | 2 | Not Started |
| 6.3 | Write troubleshooting guide | 2 | Not Started |
| 6.4 | Performance optimization based on benchmarks | 6 | Not Started |
| 6.5 | Add property-based tests (10+ tests) | 4 | Not Started |
| 6.6 | Complete PROGRESS.md with final status | 2 | Not Started |
| 6.7 | Write DECISIONS.md ADRs | 3 | Not Started |
| 6.8 | Code review and cleanup | 4 | Not Started |
| 6.9 | Final `make ci` verification | 2 | Not Started |

**Phase 6 Total**: 29 hours

### Deliverables

- [ ] CLAUDE.md updated with graph features
- [ ] CLI help text with examples
- [ ] Troubleshooting guide
- [ ] Performance optimizations applied
- [ ] Property-based tests passing
- [ ] Specification documents complete
- [ ] `make ci` passes

### Files to Modify

| File | Changes |
|------|---------|
| `CLAUDE.md` | Add Graph Memory section |
| `docs/spec/active/2026-01-12-graph-memory/PROGRESS.md` | Final status |
| `docs/spec/active/2026-01-12-graph-memory/DECISIONS.md` | Complete ADRs |

---

## Summary

| Phase | Duration | Hours | Deliverables |
|-------|----------|-------|--------------|
| Phase 1: Foundation | Week 1-2 | 61.5 | Models, traits, SQLite backend |
| Phase 2: Services | Week 3 | 44 | GraphService, EntityExtractor |
| Phase 3: MCP Tools | Week 4 | 37 | 7 MCP tools |
| Phase 4: Graph RAG | Week 5 | 35 | Hybrid search |
| Phase 5: Integration | Week 6 | 30 | Auto-extract, CLI |
| Phase 6: Polish | Week 7 | 29 | Docs, optimization |
| **Total** | **7 weeks** | **236.5** | Complete feature |

## Dependencies

### External Dependencies

- `chrono` (already in Cargo.toml) - Temporal types
- `serde_json` (already in Cargo.toml) - JSON serialization
- No new crates required

### Internal Dependencies

```
Phase 1 (Foundation)
    │
    ▼
Phase 2 (Services) ◄─── requires graph backend
    │
    ▼
Phase 3 (MCP Tools) ◄─── requires services
    │
    ▼
Phase 4 (Graph RAG) ◄─── requires services + MCP
    │
    ▼
Phase 5 (Integration) ◄─── requires all above
    │
    ▼
Phase 6 (Polish)
```

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| LLM extraction accuracy <85% | Medium | Medium | Iterative prompt tuning, fallback to regex |
| Graph traversal performance | Low | High | Recursive CTE optimization, depth limits |
| Schema migration complexity | Low | Medium | Incremental migrations, rollback support |
| Neo4j migration blockers | Low | Low | Trait abstraction tested early |
| Scope creep | Medium | Medium | Strict phase boundaries, defer to future |

## Success Criteria

- [ ] All 65+ Phase 1 tests passing
- [ ] All 7 MCP tools functional
- [ ] Entity extraction accuracy >85% (manual evaluation)
- [ ] Graph traversal <100ms for depth=2
- [ ] Graph RAG latency overhead <100ms
- [ ] `make ci` passes
- [ ] CLAUDE.md documentation complete
- [ ] Specification documents complete
