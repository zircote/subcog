# Architecture Decision Records

This document captures key architecture decisions for the Graph Memory feature.

---

## ADR-001: Separate Graph Tables vs Extending Memory Tables

**Status**: Accepted
**Date**: 2026-01-12

### Context

We need to store entities and relationships. Options:
1. Extend existing `memories` table with entity columns
2. Create separate `graph_entities` and `graph_relationships` tables
3. Use a dedicated graph database (Neo4j)

### Decision

Create separate tables (`graph_entities`, `graph_relationships`, `graph_entity_mentions`) alongside existing memory tables.

### Rationale

- **Separation of concerns**: Memories and entities have different lifecycles and query patterns
- **Performance**: Graph queries (traversals, aggregations) benefit from dedicated indexes
- **Migration path**: Easier to migrate graph tables to Neo4j later without touching memory tables
- **Backward compatibility**: Existing memory queries unaffected

### Consequences

- Additional tables to manage
- Need JOIN queries to connect entities to memories
- Cleaner data model overall

---

## ADR-002: GraphBackend Trait for Backend Independence

**Status**: Accepted
**Date**: 2026-01-12

### Context

We want to support SQLite now with future Neo4j migration. Options:
1. Direct SQLite implementation, refactor later
2. Abstract trait from the start
3. Generic over database type

### Decision

Define `GraphBackend` trait with async methods, implement `SqliteGraphBackend` first.

### Rationale

- **Future-proofing**: Neo4j implementation requires only implementing the trait
- **Testing**: `InMemoryGraphBackend` enables fast unit tests
- **Consistency**: Matches existing `PersistenceBackend`, `IndexBackend`, `VectorBackend` patterns
- **Low overhead**: Trait abstraction has minimal runtime cost

### Consequences

- Initial implementation takes slightly longer
- All graph operations go through trait interface
- Enables parallel development of Neo4j backend

---

## ADR-003: Bitemporal Tracking with Valid + Transaction Time

**Status**: Accepted
**Date**: 2026-01-12

### Context

Temporal queries require tracking when facts were true. Options:
1. Single timestamp (created_at only)
2. Valid time only (when fact was true in the world)
3. Full bitemporal (valid_time + transaction_time)

### Decision

Implement full bitemporal tracking with `valid_time_start`, `valid_time_end`, and `transaction_time`.

### Rationale

- **Historical queries**: "What did we know at time T?" requires transaction_time
- **Fact validity**: "When was this relationship active?" requires valid_time
- **Industry standard**: Mem0 and Zep use bitemporal models
- **Audit compliance**: Full temporal tracking aids SOC2/GDPR compliance

### Consequences

- More complex queries (must filter by both time dimensions)
- Larger storage footprint (additional timestamp columns)
- Richer query capabilities

---

## ADR-004: LLM-Powered Entity Extraction with Regex Fallback

**Status**: Accepted
**Date**: 2026-01-12

### Context

Entity extraction requires natural language understanding. Options:
1. Regex/pattern matching only
2. LLM-only extraction
3. Hybrid (LLM primary, regex fallback)

### Decision

Use LLM for primary extraction with graceful degradation to regex patterns.

### Rationale

- **Accuracy**: LLM achieves >85% extraction accuracy vs ~40% for regex
- **Robustness**: Regex handles edge cases when LLM unavailable
- **Performance**: Regex fallback is <10ms vs 1-3s for LLM
- **Cost awareness**: Skips LLM for very short content

### Consequences

- Requires LLM provider configuration for best results
- Two code paths to maintain
- Variable extraction quality depending on availability

---

## ADR-005: Domain-Scoped Entities Following Memory Domains

**Status**: Accepted
**Date**: 2026-01-12

### Context

Entities may be project-specific or global. Options:
1. All entities global
2. All entities project-scoped
3. Scope follows source memory domain

### Decision

Entity scope follows the domain of the memory that mentioned it:
- Project memories -> project-scoped entities
- User/org memories -> global entities

### Rationale

- **Isolation**: Project-specific concepts don't pollute global namespace
- **Consistency**: Matches memory domain model
- **Cross-project**: Allows opt-in cross-domain queries
- **User expectation**: "My project's Foo" vs "Global Foo"

### Consequences

- Same name can exist in multiple domains
- Need domain-aware deduplication
- Cross-domain queries require explicit opt-in

---

## ADR-006: Lazy Entity Migration vs Batch Migration

**Status**: Accepted
**Date**: 2026-01-12

### Context

Existing memories need entity extraction. Options:
1. Batch migration on upgrade (blocking)
2. Lazy migration on access
3. Background migration worker
4. Manual extraction tool only

### Decision

Lazy migration: extract entities when memories are accessed, with manual batch tool available.

### Rationale

- **Non-blocking**: Doesn't require long migration during upgrade
- **Progressive**: Hot memories get entities first
- **User control**: Manual tool for bulk operations
- **Resource efficient**: Spreads LLM costs over time

### Consequences

- Initial queries may trigger extraction
- Inconsistent state until fully migrated
- Manual batch extraction for completeness

---

## ADR-007: Confidence Scoring for Extraction Quality

**Status**: Accepted
**Date**: 2026-01-12

### Context

LLM extraction quality varies. Options:
1. Binary accept/reject
2. Confidence scores (0.0-1.0)
3. Manual verification only

### Decision

Store confidence scores with configurable threshold (default 0.7).

### Rationale

- **Filtering**: Low-confidence entities can be excluded from queries
- **Ranking**: Higher confidence entities rank higher in results
- **Review workflow**: Enables flagging low-confidence for human review
- **Tuning**: Threshold can be adjusted per use case

### Consequences

- Additional storage column
- Threshold configuration required
- More nuanced query filtering

---

## ADR-008: Graph RAG with Configurable Expansion

**Status**: Accepted
**Date**: 2026-01-12

### Context

Graph-enhanced search needs to balance relevance with performance. Options:
1. Always expand
2. Never expand (separate tool)
3. Configurable expansion

### Decision

Opt-in expansion with configurable depth and boost factor.

### Rationale

- **Performance control**: Expansion adds latency (<100ms target)
- **Relevance tuning**: Boost factor balances graph vs vector results
- **Flexibility**: Can disable for speed-sensitive use cases
- **Provenance**: Results include source (semantic vs graph)

### Consequences

- Configuration complexity
- Results may vary with expansion enabled/disabled
- Need performance monitoring

---

## ADR-009: Extended Relationship Types Beyond Consolidation

**Status**: Accepted
**Date**: 2026-01-12

### Context

Existing EdgeType has 8 variants for consolidation. Options:
1. Reuse EdgeType for all relationships
2. Separate RelationshipType for graph
3. Generic string-based types

### Decision

Create separate `RelationshipType` enum for entity relationships.

### Rationale

- **Semantic clarity**: Graph relationships (works_at, uses, implements) differ from consolidation edges (SummarizedBy, SourceOf)
- **Type safety**: Enum prevents invalid relationship types
- **Extensibility**: Can add new types without affecting consolidation
- **Documentation**: Self-documenting code

### Consequences

- Two relationship type systems
- Need mapping for future integration
- Clearer domain model

---

## ADR-010: Mermaid/DOT Visualization Output

**Status**: Accepted
**Date**: 2026-01-12

### Context

Users want to visualize knowledge graphs. Options:
1. Interactive web visualization
2. Static diagram generation (Mermaid/DOT)
3. JSON export only

### Decision

Generate Mermaid and DOT format diagrams; defer interactive visualization.

### Rationale

- **Simplicity**: Text-based output requires no additional infrastructure
- **Portability**: Mermaid renders in markdown, DOT in many tools
- **Scope control**: Interactive viz is significant additional work
- **User workflow**: Most users already use markdown documentation

### Consequences

- No interactive exploration (future feature)
- Limited to static snapshots
- Works in existing documentation workflows

---

## Decision Log

| ADR | Decision | Status |
|-----|----------|--------|
| 001 | Separate graph tables | Accepted |
| 002 | GraphBackend trait | Accepted |
| 003 | Bitemporal tracking | Accepted |
| 004 | LLM extraction + regex fallback | Accepted |
| 005 | Domain-scoped entities | Accepted |
| 006 | Lazy migration | Accepted |
| 007 | Confidence scoring | Accepted |
| 008 | Configurable Graph RAG | Accepted |
| 009 | Separate RelationshipType | Accepted |
| 010 | Mermaid/DOT visualization | Accepted |
