# Graph Memory Requirements

## 1. Problem Statement

Subcog currently stores memories as isolated units with limited relationship tracking. The existing `memory_edges` table supports consolidation relationships (`SummarizedBy`, `SourceOf`, `RelatedTo`) but lacks:

- **Entity extraction**: No automatic identification of people, organizations, concepts, technologies, or files
- **Knowledge graph construction**: No structured representation of "who did what, when, and with whom"
- **Temporal tracking**: No bitemporal support for historical queries ("what did we know at time T?")
- **Graph-based retrieval**: No ability to expand search results by following entity relationships

Industry leaders (Mem0, Zep) implement temporal knowledge graphs as a core differentiator.

## 2. Goals

### Primary Goals

1. **Entity Extraction**: Automatically extract entities from memories using LLM analysis
2. **Knowledge Graph**: Build and maintain a graph of entities and their relationships
3. **Temporal Tracking**: Support bitemporal queries (valid_time + transaction_time)
4. **Graph RAG**: Enhance retrieval by combining vector/BM25 search with graph traversal
5. **MCP Integration**: Expose graph capabilities via 7 MCP tools

### Secondary Goals

1. **Domain Scoping**: Support project-scoped and global entities appropriately
2. **Confidence Scoring**: Track extraction confidence for filtering/ranking
3. **Entity Deduplication**: Merge duplicate entities with canonical names
4. **Visualization**: Generate Mermaid/DOT diagrams of subgraphs
5. **Neo4j Migration Path**: Enable future migration to dedicated graph database

## 3. Non-Goals

1. **Real-time streaming**: Initial implementation is batch/request-based
2. **Graph algorithms**: No PageRank, community detection, or centrality metrics (future)
3. **Cross-user graphs**: No shared entities between different users
4. **Interactive visualization**: Static diagram generation only (Mermaid/DOT)
5. **Inference engine**: No logical reasoning or rule-based inference

## 4. User Stories

### US-1: Auto-Extract Entities on Capture

**As a** developer using Subcog
**I want** entities automatically extracted when I capture memories
**So that** my knowledge graph stays up-to-date without manual effort

**Acceptance Criteria**:
- [ ] When `SUBCOG_GRAPH_AUTO_EXTRACT=true`, entities are extracted on capture
- [ ] Extraction failures don't block memory capture (graceful degradation)
- [ ] Extracted entities include confidence scores
- [ ] Extraction completes within 3 seconds

### US-2: Batch Extract from Existing Memories

**As a** user with existing memories
**I want** to extract entities from my historical memories
**So that** I can build a complete knowledge graph

**Acceptance Criteria**:
- [ ] `subcog_extract_entities` tool accepts optional `memory_ids` filter
- [ ] Without filter, extracts from all memories (with progress reporting)
- [ ] Supports `namespace` and `since_days` filters
- [ ] Handles rate limiting gracefully

### US-3: Query Entities

**As a** user exploring my knowledge graph
**I want** to list and search entities
**So that** I can discover what concepts/people/technologies I've captured

**Acceptance Criteria**:
- [ ] `subcog_entities` tool lists entities with filters (type, domain, name)
- [ ] Supports fuzzy name matching
- [ ] Returns mention count for ranking
- [ ] Respects domain scoping (project vs user)

### US-4: Explore Relationships

**As a** user understanding connections
**I want** to query relationships between entities
**So that** I can understand "who uses what" and "what relates to what"

**Acceptance Criteria**:
- [ ] `subcog_relationships` tool queries from a starting entity
- [ ] Supports relationship type filtering
- [ ] Supports multi-hop traversal (max_depth parameter)
- [ ] Returns confidence scores on relationships

### US-5: Graph-Enhanced Search

**As a** user searching my memories
**I want** search results expanded by entity relationships
**So that** I find relevant memories I wouldn't find with keywords alone

**Acceptance Criteria**:
- [ ] Graph expansion is optional (configurable)
- [ ] Expands top-k results by 1-2 hops via entity relationships
- [ ] Results include provenance (semantic vs graph-based)
- [ ] Latency overhead <100ms for expansion

### US-6: Merge Duplicate Entities

**As a** user with duplicate entities
**I want** to merge them into a canonical form
**So that** my knowledge graph is clean and accurate

**Acceptance Criteria**:
- [ ] `subcog_entity_merge` accepts list of entity IDs and canonical name
- [ ] Preserves all relationships (re-pointed to canonical)
- [ ] Preserves all aliases from merged entities
- [ ] Audit log records the merge operation

### US-7: Infer Relationships

**As a** user building my knowledge graph
**I want** to infer relationships between entities
**So that** connections are discovered automatically

**Acceptance Criteria**:
- [ ] `subcog_relationship_infer` analyzes entities for likely relationships
- [ ] Uses LLM analysis with context from memories
- [ ] Requires minimum confidence threshold (default 0.7)
- [ ] Supports dry-run mode to preview inferences

### US-8: Visualize Knowledge Graph

**As a** user understanding my knowledge
**I want** to visualize entity relationships
**So that** I can see the structure of my captured knowledge

**Acceptance Criteria**:
- [ ] `subcog_graph_visualize` generates Mermaid or DOT format
- [ ] Supports subgraph visualization from a starting entity
- [ ] Configurable depth and entity type filters
- [ ] Output is valid Mermaid/DOT syntax

### US-9: Temporal Queries

**As a** user tracking knowledge evolution
**I want** to query "what did I know at time T"
**So that** I can understand how my knowledge has changed

**Acceptance Criteria**:
- [ ] Entities store valid_time_start and valid_time_end
- [ ] Relationships store temporal validity
- [ ] `subcog_graph_query` supports `valid_at` filter
- [ ] Historical versions are preserved (not overwritten)

### US-10: Domain-Scoped Entities

**As a** user with multiple projects
**I want** project entities separate from global entities
**So that** my per-project knowledge doesn't pollute global scope

**Acceptance Criteria**:
- [ ] Project-scoped memories create project-scoped entities
- [ ] User/org-scoped memories create global entities
- [ ] Queries respect domain scoping by default
- [ ] Cross-domain queries are opt-in

## 5. Entity Types

| Type | Description | Examples |
|------|-------------|----------|
| `person` | Named individuals | "Alice Johnson", "@username", "the architect" |
| `organization` | Companies, teams, groups | "Anthropic", "Backend Team", "PostgreSQL Foundation" |
| `concept` | Abstract ideas, patterns | "REST API", "Event Sourcing", "Memory Consolidation" |
| `technology` | Tools, frameworks, languages | "Rust", "SQLite", "FastEmbed", "Docker" |
| `file` | Code files, documents | "src/main.rs", "README.md", "Dockerfile" |

## 6. Relationship Types

| Type | Description | Example |
|------|-------------|---------|
| `works_at` | Person -> Organization | Alice works_at Anthropic |
| `created` | Entity -> Entity | Alice created AuthService |
| `uses` | Entity -> Entity | AuthService uses PostgreSQL |
| `implements` | Entity -> Entity | auth.rs implements JWT |
| `part_of` | Entity -> Entity | AuthService part_of Backend |
| `relates_to` | Entity -> Entity | Caching relates_to Performance |
| `mentioned_in` | Entity -> Memory | PostgreSQL mentioned_in mem_123 |
| `supersedes` | Entity -> Entity | v2.0 supersedes v1.0 |
| `conflicts_with` | Entity -> Entity | Decision A conflicts_with Decision B |

## 7. MCP Tools Specification

### 7.1 subcog_entities

**Purpose**: List/search entities

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `entity_type` | string | No | Filter by type: person, organization, concept, technology, file |
| `name` | string | No | Search by name (fuzzy match) |
| `domain` | string | No | Filter by domain: user, project, org |
| `min_confidence` | number | No | Minimum confidence threshold (0.0-1.0) |
| `limit` | number | No | Maximum results (default: 20) |

### 7.2 subcog_relationships

**Purpose**: Query entity relationships

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `from_entity` | string | Yes | Starting entity ID |
| `relationship_type` | string | No | Filter by relationship type |
| `max_depth` | number | No | Traversal depth (default: 1) |
| `min_confidence` | number | No | Minimum confidence (default: 0.5) |

### 7.3 subcog_graph_query

**Purpose**: Execute graph query (DSL)

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `query` | string | Yes | Graph query string |
| `valid_at` | string | No | ISO timestamp for temporal query |

### 7.4 subcog_extract_entities

**Purpose**: Extract entities from memories

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `memory_ids` | array | No | Specific memories (omit for all) |
| `namespace` | string | No | Filter by namespace |
| `since_days` | number | No | Extract from last N days |

### 7.5 subcog_entity_merge

**Purpose**: Merge duplicate entities

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `entity_ids` | array | Yes | Entity IDs to merge |
| `canonical_name` | string | Yes | Canonical name for merged entity |

### 7.6 subcog_relationship_infer

**Purpose**: Infer new relationships

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `entity_ids` | array | No | Entities to analyze (omit for all) |
| `confidence_threshold` | number | No | Minimum confidence (default: 0.7) |
| `dry_run` | boolean | No | Preview without storing |

### 7.7 subcog_graph_visualize

**Purpose**: Generate visualization

**Parameters**:
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `start_entity` | string | Yes | Starting entity for subgraph |
| `max_depth` | number | No | Traversal depth (default: 2) |
| `format` | string | No | Output format: mermaid, dot, json (default: mermaid) |

## 8. Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_GRAPH_ENABLED` | Enable graph features | `true` |
| `SUBCOG_GRAPH_AUTO_EXTRACT` | Auto-extract on capture | `false` |
| `SUBCOG_GRAPH_EXTRACT_CONFIDENCE` | Min extraction confidence | `0.7` |
| `SUBCOG_GRAPH_MAX_DEPTH` | Max traversal depth | `3` |
| `SUBCOG_GRAPH_EXPANSION_ENABLED` | Enable Graph RAG expansion | `true` |

### Config File

```toml
[graph]
enabled = true
auto_extract = false
extract_confidence = 0.7
max_traversal_depth = 3
expansion_enabled = true
expansion_boost = 1.2 # Boost factor for graph-based results

[graph.extraction]
batch_size = 10
timeout_ms = 5000
retry_count = 3
```

## 9. Performance Requirements

| Metric | Target | Notes |
|--------|--------|-------|
| Entity storage | <5ms | Single INSERT |
| Entity lookup | <2ms | Indexed by ID |
| Entity search | <50ms | FTS5 + limit 20 |
| Relationship query | <20ms | Depth=1 |
| Graph traversal (depth=2) | <100ms | Recursive CTE |
| Graph traversal (depth=3) | <500ms | May need optimization |
| Entity extraction | <3s | LLM latency dominant |
| Graph RAG expansion | <100ms | Additional latency |

## 10. Security Requirements

### SEC-1: Domain Isolation

- Project-scoped entities are only visible to queries within that project
- No cross-domain entity access without explicit opt-in
- Domain scoping enforced at storage layer

### SEC-2: Confidence Thresholds

- Low-confidence extractions (<0.5) flagged for review
- Configurable threshold for auto-storage
- Confidence visible in all query results

### SEC-3: PII Handling

- Person entities may contain PII
- Optional redaction before storage
- GDPR export includes entity mentions

### SEC-4: Injection Prevention

- All SQL queries parameterized
- Graph query DSL validated and sanitized
- No dynamic SQL from user input

## 11. Graceful Degradation

| Scenario | Behavior |
|----------|----------|
| LLM unavailable | Skip entity extraction, log warning, continue capture |
| Embedding service down | Skip similarity-based entity matching |
| Graph storage error | Log error, return partial results |
| Low confidence extraction | Store with flag, exclude from default queries |

## 12. Testing Requirements

| Test Type | Target | Description |
|-----------|--------|-------------|
| Unit tests | 150+ | Entity/relationship CRUD, temporal logic |
| Integration tests | 50+ | End-to-end extraction, Graph RAG |
| Property tests | 20+ | Graph invariants, temporal consistency |
| Benchmark tests | 10+ | Performance targets verification |

## 13. Documentation Requirements

- [ ] Update CLAUDE.md with graph features section
- [ ] Add graph examples to CLI help text
- [ ] Document MCP tools in MCP server docs
- [ ] Add troubleshooting guide for extraction issues
- [ ] Document temporal query patterns
