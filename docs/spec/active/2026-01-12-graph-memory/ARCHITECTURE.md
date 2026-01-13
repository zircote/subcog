# Graph Memory Architecture

## 1. Architecture Overview

The Graph Memory feature follows a **Pragmatic Balance** approach: clean architecture foundations with phased delivery. This enables future Neo4j migration while delivering value incrementally.

### Layer Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      MCP Tools Layer                         │
│  subcog_entities, subcog_relationships, subcog_graph_query   │
│  subcog_extract_entities, subcog_entity_merge                │
│  subcog_relationship_infer, subcog_graph_visualize           │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│                   Service Layer                              │
│  ┌──────────────────┐  ┌─────────────────┐  ┌────────────┐ │
│  │ GraphService     │  │ EntityExtractor │  │ GraphRAG   │ │
│  │ - query()        │  │ - extract()     │  │ - search() │ │
│  │ - traverse()     │  │ - infer()       │  │ - expand() │ │
│  │ - merge()        │  │                 │  │            │ │
│  └──────────────────┘  └─────────────────┘  └────────────┘ │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│                  Storage Trait Layer                         │
│  ┌────────────────────────────────────────────────────────┐ │
│  │            GraphBackend Trait                          │ │
│  │  - store_entity(), get_entity(), search_entities()    │ │
│  │  - store_relationship(), query_relationships()         │ │
│  │  - traverse(), query_graph()                          │ │
│  └────────────────────────────────────────────────────────┘ │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│              Backend Implementations                         │
│  ┌──────────────────┐  ┌──────────────────┐  ┌───────────┐ │
│  │ SqliteGraphBack  │  │ Neo4jGraphBack   │  │ InMemory  │ │
│  │ (primary)        │  │ (future)         │  │ (testing) │ │
│  └──────────────────┘  └──────────────────┘  └───────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### Integration with Existing Subcog Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    ServiceContainer                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ Capture      │──│ Recall       │  │ Graph (NEW)      │  │
│  │ Service      │  │ Service      │──│ Service          │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
│         │                 │                   │             │
│  ┌──────▼─────────────────▼───────────────────▼──────────┐ │
│  │           Existing Storage Layers                      │ │
│  │  Persistence (SQLite) │ Index (FTS5) │ Vector (usearch)│ │
│  └───────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │           NEW: Graph Storage Layer                    │  │
│  │  GraphBackend (separate tables: graph_entities, etc.) │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## 2. Module Structure

### File Organization

```
src/
├── models/
│   ├── mod.rs                    # Add graph exports
│   ├── graph.rs                  # NEW: Entity, Relationship, GraphQuery
│   └── temporal.rs               # NEW: ValidTimeRange, TransactionTime
│
├── storage/
│   ├── mod.rs                    # Add GraphBackend export
│   ├── traits/
│   │   ├── mod.rs               # Add graph trait export
│   │   └── graph.rs             # NEW: GraphBackend trait
│   └── graph/
│       ├── mod.rs               # NEW: Graph backend module
│       ├── sqlite.rs            # NEW: SqliteGraphBackend
│       └── memory.rs            # NEW: InMemoryGraphBackend (testing)
│
├── services/
│   ├── mod.rs                   # Add graph service exports
│   ├── graph.rs                 # NEW: GraphService
│   ├── entity_extraction.rs     # NEW: EntityExtractorService
│   └── graph_rag.rs             # NEW: GraphRAGService
│
├── mcp/
│   └── tools/
│       ├── handlers/
│       │   └── graph.rs         # NEW: Graph MCP tool handlers
│       └── definitions.rs       # Add graph tool definitions
│
├── llm/
│   └── system_prompt.rs         # Add ENTITY_EXTRACTION_PROMPT
│
└── cli/
    └── graph.rs                 # NEW: Graph CLI commands
```

## 3. Data Model

### Entity Structure

```rust
/// Unique identifier for an entity.
pub struct EntityId(String);

/// Entity type classification.
pub enum EntityType {
    Person,
    Organization,
    Concept,
    Technology,
    File,
}

/// An entity in the knowledge graph.
pub struct Entity {
    pub id: EntityId,
    pub entity_type: EntityType,
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub description: Option<String>,
    pub domain: Domain,
    pub project_id: Option<String>,
    pub properties: HashMap<String, String>,
    pub source_memory_ids: Vec<MemoryId>,
    pub valid_time: ValidTimeRange,
    pub transaction_time: TransactionTime,
    pub confidence: f32,
}
```

### Relationship Structure

```rust
/// Relationship type between entities.
pub enum RelationshipType {
    WorksAt,
    Created,
    Uses,
    Implements,
    PartOf,
    RelatesTo,
    MentionedIn,
    Supersedes,
    ConflictsWith,
}

/// A directed relationship between two entities.
pub struct Relationship {
    pub from_entity: EntityId,
    pub to_entity: EntityId,
    pub relationship_type: RelationshipType,
    pub properties: HashMap<String, String>,
    pub source_memory_ids: Vec<MemoryId>,
    pub valid_time: ValidTimeRange,
    pub transaction_time: TransactionTime,
    pub confidence: f32,
}
```

### Temporal Types

```rust
/// Valid time: when the fact was true in the real world.
pub struct ValidTimeRange {
    pub start: DateTime<Utc>,
    pub end: Option<DateTime<Utc>>,  // None = ongoing
}

/// Transaction time: when the fact was recorded.
pub struct TransactionTime {
    pub created_at: DateTime<Utc>,
}
```

## 4. Database Schema

### graph_entities Table

```sql
CREATE TABLE IF NOT EXISTS graph_entities (
    id TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,
    canonical_name TEXT NOT NULL,
    aliases TEXT,                           -- JSON array
    description TEXT,
    domain TEXT NOT NULL,
    project_id TEXT,
    properties TEXT,                        -- JSON object
    source_memory_ids TEXT NOT NULL,        -- JSON array
    valid_time_start INTEGER NOT NULL,      -- Unix timestamp
    valid_time_end INTEGER,                 -- NULL = ongoing
    transaction_time INTEGER NOT NULL,
    confidence REAL NOT NULL DEFAULT 1.0
        CHECK(confidence >= 0.0 AND confidence <= 1.0)
);

-- Indexes
CREATE INDEX idx_graph_entities_type ON graph_entities(entity_type);
CREATE INDEX idx_graph_entities_name ON graph_entities(canonical_name);
CREATE INDEX idx_graph_entities_domain ON graph_entities(domain, project_id);
CREATE INDEX idx_graph_entities_confidence ON graph_entities(confidence);
CREATE INDEX idx_graph_entities_valid_time
    ON graph_entities(valid_time_start, valid_time_end);

-- FTS5 for name search
CREATE VIRTUAL TABLE IF NOT EXISTS graph_entities_fts USING fts5(
    id,
    canonical_name,
    aliases,
    description
);
```

### graph_relationships Table

```sql
CREATE TABLE IF NOT EXISTS graph_relationships (
    from_entity TEXT NOT NULL,
    to_entity TEXT NOT NULL,
    relationship_type TEXT NOT NULL,
    properties TEXT,                        -- JSON object
    source_memory_ids TEXT NOT NULL,        -- JSON array
    valid_time_start INTEGER NOT NULL,
    valid_time_end INTEGER,
    transaction_time INTEGER NOT NULL,
    confidence REAL NOT NULL DEFAULT 1.0
        CHECK(confidence >= 0.0 AND confidence <= 1.0),
    PRIMARY KEY (from_entity, to_entity, relationship_type, valid_time_start),
    FOREIGN KEY (from_entity) REFERENCES graph_entities(id) ON DELETE CASCADE,
    FOREIGN KEY (to_entity) REFERENCES graph_entities(id) ON DELETE CASCADE
);

-- Indexes
CREATE INDEX idx_graph_rel_from ON graph_relationships(from_entity, relationship_type);
CREATE INDEX idx_graph_rel_to ON graph_relationships(to_entity, relationship_type);
CREATE INDEX idx_graph_rel_confidence ON graph_relationships(confidence);
CREATE INDEX idx_graph_rel_valid_time
    ON graph_relationships(valid_time_start, valid_time_end);
```

### graph_entity_mentions Table

```sql
CREATE TABLE IF NOT EXISTS graph_entity_mentions (
    entity_id TEXT NOT NULL,
    memory_id TEXT NOT NULL,
    mention_text TEXT,
    context TEXT,
    confidence REAL NOT NULL DEFAULT 1.0,
    first_mentioned_at INTEGER NOT NULL,
    last_mentioned_at INTEGER NOT NULL,
    mention_count INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (entity_id, memory_id),
    FOREIGN KEY (entity_id) REFERENCES graph_entities(id) ON DELETE CASCADE,
    FOREIGN KEY (memory_id) REFERENCES memories(id) ON DELETE CASCADE
);

CREATE INDEX idx_graph_mentions_entity ON graph_entity_mentions(entity_id);
CREATE INDEX idx_graph_mentions_memory ON graph_entity_mentions(memory_id);
```

## 5. GraphBackend Trait

```rust
/// Trait for graph storage backends.
pub trait GraphBackend: Send + Sync {
    // Entity Operations
    fn store_entity(&self, entity: &Entity) -> Result<()>;
    fn get_entity(&self, id: &EntityId) -> Result<Option<Entity>>;
    fn search_entities(
        &self,
        filter: &EntityFilter,
        limit: usize,
    ) -> Result<Vec<Entity>>;
    fn find_by_name(&self, name: &str, fuzzy: bool) -> Result<Vec<Entity>>;
    fn delete_entity(&self, id: &EntityId) -> Result<bool>;

    // Relationship Operations
    fn store_relationship(&self, relationship: &Relationship) -> Result<()>;
    fn query_relationships(
        &self,
        from_entity: &EntityId,
        relationship_type: Option<RelationshipType>,
    ) -> Result<Vec<Relationship>>;
    fn query_incoming(
        &self,
        to_entity: &EntityId,
        relationship_type: Option<RelationshipType>,
    ) -> Result<Vec<Relationship>>;
    fn delete_relationship(
        &self,
        from: &EntityId,
        to: &EntityId,
        rel_type: RelationshipType,
    ) -> Result<bool>;

    // Graph Traversal
    fn traverse(
        &self,
        start: &EntityId,
        max_depth: usize,
        filter: Option<RelationshipType>,
    ) -> Result<GraphQueryResult>;

    fn shortest_path(
        &self,
        from: &EntityId,
        to: &EntityId,
        max_depth: usize,
    ) -> Result<Option<Vec<EntityId>>>;

    // Mention Operations
    fn store_mention(&self, mention: &EntityMention) -> Result<()>;
    fn get_mentions_for_entity(&self, id: &EntityId) -> Result<Vec<EntityMention>>;
    fn get_entities_for_memory(&self, id: &MemoryId) -> Result<Vec<Entity>>;
}
```

## 6. Service Design

### GraphService

```rust
pub struct GraphService {
    backend: Arc<dyn GraphBackend>,
}

impl GraphService {
    pub fn new(backend: Arc<dyn GraphBackend>) -> Self;

    // CRUD operations delegate to backend
    pub fn store_entity(&self, entity: Entity) -> Result<()>;
    pub fn get_entity(&self, id: &EntityId) -> Result<Option<Entity>>;
    pub fn search_entities(&self, filter: &EntityFilter) -> Result<Vec<Entity>>;

    // Graph operations
    pub fn traverse(&self, start: &EntityId, depth: usize) -> Result<GraphQueryResult>;
    pub fn merge_entities(&self, ids: &[EntityId], canonical: &str) -> Result<Entity>;

    // Visualization
    pub fn visualize(&self, start: &EntityId, format: VisFormat) -> Result<String>;
}
```

### EntityExtractorService

```rust
pub struct EntityExtractorService {
    llm: Option<Arc<dyn LlmProvider>>,
    graph: Arc<GraphService>,
}

impl EntityExtractorService {
    pub fn new(
        llm: Option<Arc<dyn LlmProvider>>,
        graph: Arc<GraphService>,
    ) -> Self;

    /// Extract entities from a single memory.
    pub fn extract_from_memory(&self, memory: &Memory) -> Result<ExtractionResult>;

    /// Batch extract from multiple memories.
    pub fn extract_batch(
        &self,
        memories: &[Memory],
        progress: Option<ProgressCallback>,
    ) -> Result<BatchExtractionResult>;

    /// Infer relationships between entities.
    pub fn infer_relationships(
        &self,
        entities: &[Entity],
    ) -> Result<Vec<Relationship>>;
}
```

### GraphRAGService

```rust
pub struct GraphRAGService {
    recall: Arc<RecallService>,
    graph: Arc<GraphService>,
    config: GraphRAGConfig,
}

impl GraphRAGService {
    /// Hybrid search with graph expansion.
    pub fn search_with_expansion(
        &self,
        query: &str,
        filter: &SearchFilter,
        expansion_config: ExpansionConfig,
    ) -> Result<SearchResults>;
}
```

## 7. LLM Integration

### Entity Extraction Prompt

```rust
pub const ENTITY_EXTRACTION_PROMPT: &str = r#"
<operation_mode>entity_extraction</operation_mode>

<task>
Extract entities and relationships from the following memory content.
</task>

<entity_types>
- person: Named individuals (e.g., "Alice", "@username")
- organization: Companies, teams (e.g., "Anthropic", "Backend Team")
- concept: Abstract ideas (e.g., "REST API", "Event Sourcing")
- technology: Tools, frameworks (e.g., "PostgreSQL", "Rust")
- file: Code files, paths (e.g., "src/main.rs", "Dockerfile")
</entity_types>

<relationship_types>
- works_at: Person works at organization
- created: Entity created entity
- uses: Entity uses/depends on entity
- implements: Entity implements entity
- part_of: Entity is part of entity
- relates_to: Generic relationship
</relationship_types>

<output_format>
{
  "entities": [
    {
      "type": "technology",
      "name": "PostgreSQL",
      "aliases": ["Postgres", "pg"],
      "description": "Relational database",
      "confidence": 0.95
    }
  ],
  "relationships": [
    {
      "from": "AuthService",
      "to": "PostgreSQL",
      "type": "uses",
      "confidence": 0.9
    }
  ]
}
</output_format>
"#;
```

## 8. Data Flow

### Entity Extraction Flow

```
Memory Capture
    │
    ▼
CaptureService.capture()
    │
    ▼ (if SUBCOG_GRAPH_AUTO_EXTRACT=true)
EntityExtractorService.extract_from_memory()
    │
    ▼
LLM.complete_with_system(ENTITY_EXTRACTION_PROMPT)
    │
    ▼
Parse JSON → Vec<ExtractedEntity>, Vec<ExtractedRelationship>
    │
    ▼
GraphService.store_entity() for each entity
    │
    ▼
GraphService.store_relationship() for each relationship
    │
    ▼
GraphBackend.store_mention() (link to memory)
```

### Graph RAG Flow

```
User Query: "How do we handle auth?"
    │
    ▼
GraphRAGService.search_with_expansion()
    │
    ├──▶ RecallService.search() → 10 memories (semantic)
    │
    └──▶ EntityExtractorService.extract_from_query("auth")
             │
             ▼
         ["AuthService", "JWT", "OAuth"]
             │
             ▼
         GraphService.traverse(depth=2)
             │
             ▼
         Related entities + their source_memory_ids
             │
             ▼
         5 additional memories via graph
    │
    ▼
Merge + Re-rank (boost graph-based by config.expansion_boost)
    │
    ▼
Return 15 memories with provenance
```

## 9. Configuration

### GraphConfig

```rust
pub struct GraphConfig {
    /// Enable graph features
    pub enabled: bool,
    /// Auto-extract on capture
    pub auto_extract: bool,
    /// Minimum extraction confidence
    pub extract_confidence: f32,
    /// Maximum traversal depth
    pub max_depth: usize,
    /// Enable Graph RAG expansion
    pub expansion_enabled: bool,
    /// Boost factor for graph results
    pub expansion_boost: f32,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_extract: false,
            extract_confidence: 0.7,
            max_depth: 3,
            expansion_enabled: true,
            expansion_boost: 1.2,
        }
    }
}
```

## 10. Error Handling

| Error | Recovery |
|-------|----------|
| LLM timeout | Log warning, skip extraction, continue capture |
| Entity not found | Return None/empty, suggest extraction |
| Duplicate entity | Use existing or merge based on confidence |
| Circular graph | Detect cycles, limit depth |
| Parse error | Log error, return partial results |
| Storage error | Propagate error, rollback transaction |

## 11. Observability

### Metrics

```rust
// Counters
graph_entities_stored_total{entity_type}
graph_relationships_stored_total{relationship_type}
graph_extractions_total{status="success"|"failure"|"skipped"}
graph_merges_total

// Histograms
graph_extraction_duration_ms
graph_traversal_duration_ms{depth}
graph_search_duration_ms

// Gauges
graph_entities_count{entity_type, domain}
graph_relationships_count{relationship_type}
```

### Tracing

```rust
#[tracing::instrument(skip(self), fields(entity_id = %id))]
pub fn get_entity(&self, id: &EntityId) -> Result<Option<Entity>> {
    // ...
}
```

## 12. Security

### Domain Isolation

- Entities scoped by `domain` and `project_id`
- Queries filter by domain automatically
- Cross-domain access requires explicit flag

### Injection Prevention

- All SQL queries use parameterized statements
- Entity names sanitized for FTS5 queries
- Graph query DSL whitelist-validated

### PII Handling

- Person entities may contain PII
- ContentRedactor applied to descriptions (names exempt)
- GDPR export includes entity data

## 13. Future: Neo4j Migration

The `GraphBackend` trait enables clean migration:

```rust
pub struct Neo4jGraphBackend {
    graph: neo4rs::Graph,
}

impl GraphBackend for Neo4jGraphBackend {
    fn store_entity(&self, entity: &Entity) -> Result<()> {
        // MERGE (e:Person {id: $id}) SET e += $properties
    }
    // ...
}
```

Migration path:
1. **Dual-write**: Write to both SQLite and Neo4j
2. **Shadow mode**: Compare query results
3. **Cutover**: Switch reads to Neo4j
4. **Cleanup**: Remove SQLite graph tables
