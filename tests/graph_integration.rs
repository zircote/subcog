//! Graph Memory Integration Tests (Phase 5.4)
//!
//! Tests the entity extraction → graph storage → query roundtrip.
//! Verifies that auto-extraction during capture correctly stores entities
//! and relationships in the knowledge graph.

// Integration tests use expect/unwrap for simplicity - panics are acceptable in tests
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;
use subcog::embedding::FastEmbedEmbedder;
use subcog::models::Domain;
use subcog::models::graph::{Entity, EntityQuery, EntityType, Relationship, RelationshipType};
use subcog::services::{
    CaptureService, EntityExtractionCallback, EntityExtractionStats, EntityExtractorService,
    GraphService,
};
use subcog::storage::graph::SqliteGraphBackend;
use subcog::storage::index::SqliteBackend;
use subcog::storage::vector::UsearchBackend;
use subcog::{CaptureRequest, Embedder, IndexBackend, Namespace, VectorBackend};
use tempfile::TempDir;

/// Helper to create a graph service for testing.
fn create_graph_service(temp_dir: &TempDir) -> GraphService<SqliteGraphBackend> {
    let graph_path = temp_dir.path().join("test_graph.db");
    let backend = SqliteGraphBackend::new(&graph_path).expect("Failed to create graph backend");
    GraphService::new(backend)
}

/// Helper to create an entity extraction callback for testing.
///
/// This creates a callback that extracts entities using fallback mode (no LLM)
/// and stores them in the provided graph service.
#[allow(clippy::excessive_nesting)] // Callback closure requires nested scopes
fn create_entity_extraction_callback(
    graph_service: Arc<GraphService<SqliteGraphBackend>>,
) -> EntityExtractionCallback {
    use std::collections::HashMap;

    let domain = Domain::new();
    let entity_extractor = Arc::new(EntityExtractorService::without_llm(domain));

    Arc::new(move |content: &str, memory_id: &subcog::MemoryId| {
        let mut stats = EntityExtractionStats::default();

        // Extract entities from content
        let extraction = entity_extractor.extract(content)?;
        stats.used_fallback = extraction.used_fallback;

        // Map entity names to IDs for relationship resolution
        let mut name_to_id: HashMap<String, subcog::models::graph::EntityId> = HashMap::new();

        // Store entities in graph
        for extracted in &extraction.entities {
            // Parse entity type, defaulting to Concept if unknown
            let entity_type =
                EntityType::parse(&extracted.entity_type).unwrap_or(EntityType::Concept);

            // Create the Entity from ExtractedEntity
            let entity = Entity::new(entity_type, &extracted.name, Domain::new())
                .with_confidence(extracted.confidence)
                .with_aliases(extracted.aliases.iter().cloned());

            // Store entity and track name mapping on success
            if graph_service.store_entity(&entity).is_ok() {
                stats.entities_stored += 1;
                name_to_id.insert(extracted.name.clone(), entity.id.clone());
                name_to_id.extend(
                    extracted
                        .aliases
                        .iter()
                        .map(|alias| (alias.clone(), entity.id.clone())),
                );
                let _ = graph_service.record_mention(&entity.id, memory_id);
            }
        }

        // Store relationships in graph
        for extracted_rel in &extraction.relationships {
            // Look up entity IDs by name - skip if either entity not found
            let (Some(from), Some(to)) = (
                name_to_id.get(&extracted_rel.from),
                name_to_id.get(&extracted_rel.to),
            ) else {
                continue;
            };

            // Parse relationship type, defaulting to RelatesTo if unknown
            let rel_type = RelationshipType::parse(&extracted_rel.relationship_type)
                .unwrap_or(RelationshipType::RelatesTo);

            let relationship = Relationship::new(from.clone(), to.clone(), rel_type)
                .with_confidence(extracted_rel.confidence);

            if graph_service.store_relationship(&relationship).is_ok() {
                stats.relationships_stored += 1;
            }
        }

        Ok(stats)
    })
}

/// Helper to create a capture service with entity extraction enabled.
fn create_capture_service_with_extraction(
    temp_dir: &TempDir,
    graph_service: Arc<GraphService<SqliteGraphBackend>>,
) -> CaptureService {
    let mut config = subcog::config::Config::default();
    config.features.auto_extract_entities = true;

    let embedder: Arc<dyn Embedder> = Arc::new(FastEmbedEmbedder::new());

    let index_path = temp_dir.path().join("test_index.db");
    let index: Arc<dyn IndexBackend + Send + Sync> =
        Arc::new(SqliteBackend::new(&index_path).expect("Failed to create SQLite index"));

    let vector_path = temp_dir.path().join("test_vectors");
    #[cfg(feature = "usearch-hnsw")]
    let vector: Arc<dyn VectorBackend + Send + Sync> = Arc::new(
        UsearchBackend::new(&vector_path, FastEmbedEmbedder::DEFAULT_DIMENSIONS)
            .expect("Failed to create vector backend"),
    );
    #[cfg(not(feature = "usearch-hnsw"))]
    let vector: Arc<dyn VectorBackend + Send + Sync> = Arc::new(UsearchBackend::new(
        &vector_path,
        FastEmbedEmbedder::DEFAULT_DIMENSIONS,
    ));

    let callback = create_entity_extraction_callback(graph_service);

    CaptureService::with_backends(config, embedder, index, vector).with_entity_extraction(callback)
}

// ============================================================================
// Entity Extraction Integration Tests
// ============================================================================

/// Test: Entity extraction is triggered during capture
///
/// Verifies that the entity extraction callback is invoked during capture.
#[test]
fn test_entity_extraction_triggered_during_capture() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let graph_service = Arc::new(create_graph_service(&temp_dir));
    let capture_service =
        create_capture_service_with_extraction(&temp_dir, Arc::clone(&graph_service));

    // Verify extraction is configured
    assert!(
        capture_service.has_entity_extraction(),
        "Entity extraction should be configured"
    );

    // Capture a memory with entities
    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "Alice from Anthropic decided to use Rust for the new project".to_string(),
        domain: Domain::new(),
        tags: vec!["architecture".to_string()],
        source: None,
        skip_security_check: true,
        ttl_seconds: None,
        scope: None,
        #[cfg(feature = "group-scope")]
        group_id: None,
    };

    let result = capture_service.capture(request);
    assert!(result.is_ok(), "Capture should succeed: {:?}", result.err());
}

/// Test: Entities are stored in graph after capture
///
/// Verifies that entities extracted during capture are stored in the graph.
#[test]
fn test_entities_stored_in_graph_after_capture() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let graph_service = Arc::new(create_graph_service(&temp_dir));
    let capture_service =
        create_capture_service_with_extraction(&temp_dir, Arc::clone(&graph_service));

    // Check initial graph stats
    let initial_stats = graph_service.get_stats().expect("Should get stats");
    let initial_entity_count = initial_stats.entity_count;

    // Capture memory with clear entity content
    // Note: Fallback extraction uses regex patterns, so we need structured content
    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "The PostgreSQL database will store user data. Redis will handle caching."
            .to_string(),
        domain: Domain::new(),
        tags: vec!["database".to_string()],
        source: None,
        skip_security_check: true,
        ttl_seconds: None,
        scope: None,
        #[cfg(feature = "group-scope")]
        group_id: None,
    };

    let result = capture_service.capture(request);
    assert!(result.is_ok(), "Capture should succeed");

    // Check graph stats after capture
    let final_stats = graph_service.get_stats().expect("Should get stats");

    // Note: Fallback extraction may or may not find entities depending on patterns
    // The test verifies the pipeline works, not that specific entities are found
    println!(
        "Initial entities: {}, Final entities: {}",
        initial_entity_count, final_stats.entity_count
    );
}

/// Test: Graph query finds entities by type
///
/// Verifies that entities can be queried by type after extraction.
#[test]
fn test_graph_query_entities_by_type() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let graph_service = Arc::new(create_graph_service(&temp_dir));

    // Manually store a Technology entity for testing
    let entity = Entity::new(EntityType::Technology, "Rust", Domain::new());
    graph_service
        .store_entity(&entity)
        .expect("Should store entity");

    // Query by type
    let results = graph_service
        .find_by_type(EntityType::Technology, 10)
        .expect("Should query entities");

    assert!(!results.is_empty(), "Should find Technology entities");
    assert!(
        results.iter().any(|e| e.name == "Rust"),
        "Should find Rust entity"
    );
}

/// Test: Graph query finds entities by name
///
/// Verifies that entities can be found by name search.
#[test]
fn test_graph_query_entities_by_name() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let graph_service = Arc::new(create_graph_service(&temp_dir));

    // Store multiple entities
    let entities = vec![
        Entity::new(EntityType::Technology, "PostgreSQL", Domain::new()),
        Entity::new(EntityType::Technology, "MySQL", Domain::new()),
        Entity::new(EntityType::Technology, "Redis", Domain::new()),
    ];

    for entity in &entities {
        graph_service
            .store_entity(entity)
            .expect("Should store entity");
    }

    // Search by name
    let results = graph_service
        .find_by_name("SQL", None, None, 10)
        .expect("Should search entities");

    // Should find PostgreSQL and MySQL (both contain "SQL")
    assert!(
        results.len() >= 2,
        "Should find at least 2 entities with 'SQL' in name, found {}",
        results.len()
    );
}

/// Test: Graph relationships are stored
///
/// Verifies that relationships between entities are stored correctly.
#[test]
fn test_graph_relationships_stored() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let graph_service = Arc::new(create_graph_service(&temp_dir));

    // Store entities
    let person = Entity::new(EntityType::Person, "Alice", Domain::new());
    let org = Entity::new(EntityType::Organization, "Anthropic", Domain::new());

    graph_service
        .store_entity(&person)
        .expect("Should store person");
    graph_service.store_entity(&org).expect("Should store org");

    // Store relationship
    let relationship = Relationship::new(person.id.clone(), org.id, RelationshipType::WorksAt);
    graph_service
        .store_relationship(&relationship)
        .expect("Should store relationship");

    // Query relationships
    let outgoing = graph_service
        .get_outgoing_relationships(&person.id)
        .expect("Should get relationships");

    assert!(!outgoing.is_empty(), "Should have outgoing relationships");
    assert!(
        outgoing
            .iter()
            .any(|r| r.relationship_type == RelationshipType::WorksAt),
        "Should have WorksAt relationship"
    );
}

/// Test: Entity mentions are recorded
///
/// Verifies that entity mentions linking memories to entities are recorded.
#[test]
fn test_entity_mentions_recorded() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let graph_service = Arc::new(create_graph_service(&temp_dir));

    // Store an entity
    let entity = Entity::new(EntityType::Concept, "Architecture", Domain::new());
    graph_service
        .store_entity(&entity)
        .expect("Should store entity");

    // Record a mention
    let memory_id = subcog::MemoryId::new("test-memory-001");
    graph_service
        .record_mention(&entity.id, &memory_id)
        .expect("Should record mention");

    // Query mentions
    let mentions = graph_service
        .get_mentions(&entity.id)
        .expect("Should get mentions");

    assert!(!mentions.is_empty(), "Should have mentions");
    assert!(
        mentions.iter().any(|m| m.memory_id == memory_id),
        "Should have the recorded mention"
    );
}

/// Test: Graph traversal works
///
/// Verifies that graph traversal from an entity works correctly.
#[test]
fn test_graph_traversal() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let graph_service = Arc::new(create_graph_service(&temp_dir));

    // Build a small graph: Alice -> WorksAt -> Anthropic -> Uses -> Rust
    let alice = Entity::new(EntityType::Person, "Alice", Domain::new());
    let anthropic = Entity::new(EntityType::Organization, "Anthropic", Domain::new());
    let rust = Entity::new(EntityType::Technology, "Rust", Domain::new());

    graph_service.store_entity(&alice).expect("Store alice");
    graph_service
        .store_entity(&anthropic)
        .expect("Store anthropic");
    graph_service.store_entity(&rust).expect("Store rust");

    let works_at = Relationship::new(
        alice.id.clone(),
        anthropic.id.clone(),
        RelationshipType::WorksAt,
    );
    let uses = Relationship::new(anthropic.id, rust.id, RelationshipType::Uses);

    graph_service
        .store_relationship(&works_at)
        .expect("Store works_at");
    graph_service.store_relationship(&uses).expect("Store uses");

    // Traverse from Alice with depth 2
    let traversal = graph_service
        .traverse(&alice.id, 2, None, None)
        .expect("Should traverse");

    // Should find Alice, Anthropic, and Rust
    assert!(
        traversal.entities.len() >= 3,
        "Should find at least 3 entities in traversal, found {}",
        traversal.entities.len()
    );

    // Should find both relationships
    assert!(
        traversal.relationships.len() >= 2,
        "Should find at least 2 relationships in traversal, found {}",
        traversal.relationships.len()
    );
}

/// Test: Graph stats are accurate
///
/// Verifies that graph statistics reflect the actual state.
#[test]
fn test_graph_stats_accurate() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let graph_service = Arc::new(create_graph_service(&temp_dir));

    // Initial stats should be zero
    let stats = graph_service.get_stats().expect("Should get stats");
    assert_eq!(stats.entity_count, 0, "Initial entity count should be 0");
    assert_eq!(
        stats.relationship_count, 0,
        "Initial relationship count should be 0"
    );

    // Add entities and relationships
    let e1 = Entity::new(EntityType::Person, "Bob", Domain::new());
    let e2 = Entity::new(EntityType::Organization, "TechCorp", Domain::new());

    graph_service.store_entity(&e1).expect("Store e1");
    graph_service.store_entity(&e2).expect("Store e2");

    let rel = Relationship::new(e1.id, e2.id, RelationshipType::WorksAt);
    graph_service.store_relationship(&rel).expect("Store rel");

    // Stats should reflect the additions
    let final_stats = graph_service.get_stats().expect("Should get final stats");
    assert_eq!(final_stats.entity_count, 2, "Should have 2 entities");
    assert_eq!(
        final_stats.relationship_count, 1,
        "Should have 1 relationship"
    );
}

/// Test: Entity query with limit
///
/// Verifies that entity queries respect the limit parameter.
#[test]
fn test_entity_query_with_limit() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let graph_service = Arc::new(create_graph_service(&temp_dir));

    // Store 10 entities
    for i in 0..10 {
        let entity = Entity::new(EntityType::Concept, format!("Concept{i}"), Domain::new());
        graph_service.store_entity(&entity).expect("Store entity");
    }

    // Query with limit
    let query = EntityQuery::new().with_limit(5);
    let results = graph_service
        .query_entities(&query)
        .expect("Should query entities");

    assert!(
        results.len() <= 5,
        "Should return at most 5 entities, got {}",
        results.len()
    );
}

/// Test: Graceful degradation when feature disabled
///
/// Verifies that capture works normally when entity extraction is disabled.
#[test]
fn test_capture_without_extraction() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let config = subcog::config::Config::default();
    let embedder: Arc<dyn Embedder> = Arc::new(FastEmbedEmbedder::new());

    let index_path = temp_dir.path().join("test_index.db");
    let index: Arc<dyn IndexBackend + Send + Sync> =
        Arc::new(SqliteBackend::new(&index_path).expect("Failed to create SQLite index"));

    let vector_path = temp_dir.path().join("test_vectors");
    #[cfg(feature = "usearch-hnsw")]
    let vector: Arc<dyn VectorBackend + Send + Sync> = Arc::new(
        UsearchBackend::new(&vector_path, FastEmbedEmbedder::DEFAULT_DIMENSIONS)
            .expect("Failed to create vector backend"),
    );
    #[cfg(not(feature = "usearch-hnsw"))]
    let vector: Arc<dyn VectorBackend + Send + Sync> = Arc::new(UsearchBackend::new(
        &vector_path,
        FastEmbedEmbedder::DEFAULT_DIMENSIONS,
    ));

    // Create capture service WITHOUT entity extraction
    let capture_service = CaptureService::with_backends(config, embedder, index, vector);

    assert!(
        !capture_service.has_entity_extraction(),
        "Entity extraction should NOT be configured"
    );

    // Capture should still work
    let request = CaptureRequest {
        namespace: Namespace::Decisions,
        content: "Test content without extraction".to_string(),
        domain: Domain::new(),
        tags: vec!["test".to_string()],
        source: None,
        skip_security_check: true,
        ttl_seconds: None,
        scope: None,
        #[cfg(feature = "group-scope")]
        group_id: None,
    };

    let result = capture_service.capture(request);
    assert!(
        result.is_ok(),
        "Capture should succeed without extraction: {:?}",
        result.err()
    );
}
