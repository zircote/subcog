//! Graph service for high-level knowledge graph operations.
//!
//! Provides a service layer wrapping [`GraphBackend`] with business logic,
//! entity deduplication, and relationship inference.
//!
//! # Example
//!
//! ```rust,ignore
//! use subcog::services::GraphService;
//! use subcog::storage::graph::SqliteGraphBackend;
//! use subcog::models::graph::{Entity, EntityType, EntityQuery};
//!
//! let backend = SqliteGraphBackend::new("graph.db")?;
//! let service = GraphService::new(backend);
//!
//! // Store an entity
//! let entity = Entity::new(EntityType::Person, "Alice", Domain::for_user());
//! service.store_entity(&entity)?;
//!
//! // Query entities
//! let people = service.find_by_type(EntityType::Person, 10)?;
//! ```

use crate::models::graph::{
    Entity, EntityId, EntityMention, EntityQuery, EntityType, Relationship, RelationshipQuery,
    RelationshipType, TraversalResult,
};
use crate::models::temporal::BitemporalPoint;
use crate::models::{Domain, MemoryId};
use crate::storage::traits::graph::{GraphBackend, GraphStats};
use crate::{Error, Result};
use std::sync::Arc;

/// High-level service for knowledge graph operations.
///
/// Wraps a [`GraphBackend`] and provides:
/// - Entity CRUD with deduplication hints
/// - Relationship management
/// - Graph traversal and path finding
/// - Integration with memory system
///
/// # Thread Safety
///
/// The service is thread-safe when the underlying backend is thread-safe.
/// Both [`SqliteGraphBackend`](crate::storage::graph::SqliteGraphBackend) and
/// [`InMemoryGraphBackend`](crate::storage::graph::InMemoryGraphBackend) are thread-safe.
pub struct GraphService<B: GraphBackend> {
    backend: Arc<B>,
}

impl<B: GraphBackend> GraphService<B> {
    /// Creates a new graph service with the given backend.
    pub fn new(backend: B) -> Self {
        Self {
            backend: Arc::new(backend),
        }
    }

    /// Creates a new graph service with a shared backend.
    #[must_use]
    pub const fn with_shared_backend(backend: Arc<B>) -> Self {
        Self { backend }
    }

    /// Returns a reference to the underlying backend.
    #[must_use]
    pub fn backend(&self) -> &B {
        &self.backend
    }

    // =========================================================================
    // Entity Operations
    // =========================================================================

    /// Stores an entity in the graph.
    ///
    /// If an entity with the same ID exists, it will be updated.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage operation fails.
    pub fn store_entity(&self, entity: &Entity) -> Result<()> {
        self.backend.store_entity(entity)
    }

    /// Stores an entity with automatic deduplication.
    ///
    /// Checks for an existing entity with the same name and type (case-insensitive).
    /// If found:
    /// - Updates confidence if the new entity has higher confidence
    /// - Merges aliases from both entities
    /// - Returns the existing entity's ID
    ///
    /// If not found, stores the new entity and returns its ID.
    ///
    /// # Arguments
    ///
    /// * `entity` - The entity to store or merge
    ///
    /// # Returns
    ///
    /// The ID of the stored or existing entity.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage or lookup operation fails.
    pub fn store_entity_deduped(&self, entity: &Entity) -> Result<EntityId> {
        // Look for existing entity with exact name+type match (case-insensitive)
        let existing = self.backend.find_entities_by_name(
            &entity.name,
            Some(entity.entity_type),
            Some(&entity.domain),
            10, // Small limit since we're looking for exact matches
        )?;

        // Find exact case-insensitive match
        let name_lower = entity.name.to_lowercase();
        let exact_match = existing
            .into_iter()
            .find(|e| e.name.to_lowercase() == name_lower);

        if let Some(mut existing_entity) = exact_match {
            // Update confidence if new extraction has higher confidence
            if entity.confidence > existing_entity.confidence {
                existing_entity.confidence = entity.confidence;
            }

            // Merge aliases (add new aliases that don't already exist)
            // Build set of existing aliases (lowercased) for efficient lookup
            let existing_lower: std::collections::HashSet<String> = existing_entity
                .aliases
                .iter()
                .map(|a| a.to_lowercase())
                .chain(std::iter::once(existing_entity.name.to_lowercase()))
                .collect();

            let new_aliases: Vec<String> = entity
                .aliases
                .iter()
                .filter(|alias| !existing_lower.contains(&alias.to_lowercase()))
                .cloned()
                .collect();
            existing_entity.aliases.extend(new_aliases);

            // Increment mention count
            existing_entity.mention_count = existing_entity.mention_count.saturating_add(1);

            // Store the updated entity
            self.backend.store_entity(&existing_entity)?;

            Ok(existing_entity.id)
        } else {
            // No duplicate found, store the new entity
            self.backend.store_entity(entity)?;
            Ok(entity.id.clone())
        }
    }

    /// Retrieves an entity by ID.
    ///
    /// # Returns
    ///
    /// `Some(entity)` if found, `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the retrieval fails.
    pub fn get_entity(&self, id: &EntityId) -> Result<Option<Entity>> {
        self.backend.get_entity(id)
    }

    /// Queries entities matching the given criteria.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn query_entities(&self, query: &EntityQuery) -> Result<Vec<Entity>> {
        self.backend.query_entities(query)
    }

    /// Finds entities by type with a limit.
    ///
    /// Convenience method for common entity type queries.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn find_by_type(&self, entity_type: EntityType, limit: usize) -> Result<Vec<Entity>> {
        let query = EntityQuery::new().with_type(entity_type).with_limit(limit);
        self.backend.query_entities(&query)
    }

    /// Finds entities by name with optional type and domain filtering.
    ///
    /// Performs case-insensitive partial matching on entity names and aliases.
    ///
    /// # Errors
    ///
    /// Returns an error if the search fails.
    pub fn find_by_name(
        &self,
        name: &str,
        entity_type: Option<EntityType>,
        domain: Option<&Domain>,
        limit: usize,
    ) -> Result<Vec<Entity>> {
        self.backend
            .find_entities_by_name(name, entity_type, domain, limit)
    }

    /// Deletes an entity and its relationships/mentions.
    ///
    /// # Returns
    ///
    /// `true` if the entity existed and was deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion fails.
    pub fn delete_entity(&self, id: &EntityId) -> Result<bool> {
        self.backend.delete_entity(id)
    }

    /// Merges multiple entities into one canonical entity.
    ///
    /// The first entity ID becomes the canonical entity. All relationships
    /// and mentions from other entities are redirected to the canonical entity.
    ///
    /// # Arguments
    ///
    /// * `entity_ids` - IDs of entities to merge (first is canonical)
    /// * `canonical_name` - The name for the merged entity
    ///
    /// # Returns
    ///
    /// The merged entity.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No entity IDs provided
    /// - Canonical entity not found
    /// - Merge operation fails
    pub fn merge_entities(&self, entity_ids: &[EntityId], canonical_name: &str) -> Result<Entity> {
        if entity_ids.is_empty() {
            return Err(Error::OperationFailed {
                operation: "merge_entities".to_string(),
                cause: "No entity IDs provided".to_string(),
            });
        }

        self.backend.merge_entities(entity_ids, canonical_name)
    }

    /// Finds potential duplicate entities based on name similarity.
    ///
    /// Returns entities that may be duplicates of the given entity,
    /// useful for deduplication workflows.
    ///
    /// # Errors
    ///
    /// Returns an error if the search fails.
    pub fn find_duplicates(&self, entity: &Entity, threshold: f32) -> Result<Vec<Entity>> {
        // Find entities with similar names
        let candidates = self.backend.find_entities_by_name(
            &entity.name,
            Some(entity.entity_type),
            Some(&entity.domain),
            20,
        )?;

        // Filter by confidence threshold (simple heuristic)
        let duplicates: Vec<Entity> = candidates
            .into_iter()
            .filter(|e| e.id != entity.id && name_similarity(&e.name, &entity.name) >= threshold)
            .collect();

        Ok(duplicates)
    }

    // =========================================================================
    // Relationship Operations
    // =========================================================================

    /// Stores a relationship between entities.
    ///
    /// If a relationship with the same (from, to, type) exists, it will be updated.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage operation fails.
    pub fn store_relationship(&self, relationship: &Relationship) -> Result<()> {
        self.backend.store_relationship(relationship)
    }

    /// Creates a relationship between two entities.
    ///
    /// Convenience method that creates and stores a relationship.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Either entity doesn't exist
    /// - Storage operation fails
    pub fn relate(
        &self,
        from: &EntityId,
        to: &EntityId,
        relationship_type: RelationshipType,
    ) -> Result<Relationship> {
        // Verify entities exist
        if self.backend.get_entity(from)?.is_none() {
            return Err(Error::OperationFailed {
                operation: "relate".to_string(),
                cause: format!("From entity not found: {}", from.as_str()),
            });
        }
        if self.backend.get_entity(to)?.is_none() {
            return Err(Error::OperationFailed {
                operation: "relate".to_string(),
                cause: format!("To entity not found: {}", to.as_str()),
            });
        }

        let relationship = Relationship::new(from.clone(), to.clone(), relationship_type);
        self.backend.store_relationship(&relationship)?;
        Ok(relationship)
    }

    /// Queries relationships matching the given criteria.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn query_relationships(&self, query: &RelationshipQuery) -> Result<Vec<Relationship>> {
        self.backend.query_relationships(query)
    }

    /// Gets all relationships from an entity.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_outgoing_relationships(&self, entity_id: &EntityId) -> Result<Vec<Relationship>> {
        let query = RelationshipQuery::new().from(entity_id.clone());
        self.backend.query_relationships(&query)
    }

    /// Gets all relationships to an entity.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_incoming_relationships(&self, entity_id: &EntityId) -> Result<Vec<Relationship>> {
        let query = RelationshipQuery::new().to(entity_id.clone());
        self.backend.query_relationships(&query)
    }

    /// Deletes relationships matching the query.
    ///
    /// # Returns
    ///
    /// Number of relationships deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion fails.
    pub fn delete_relationships(&self, query: &RelationshipQuery) -> Result<usize> {
        self.backend.delete_relationships(query)
    }

    /// Gets all relationship types between two entities.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_relationship_types(
        &self,
        from: &EntityId,
        to: &EntityId,
    ) -> Result<Vec<RelationshipType>> {
        self.backend.get_relationship_types(from, to)
    }

    // =========================================================================
    // Mention Operations
    // =========================================================================

    /// Records an entity mention in a memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage operation fails.
    pub fn record_mention(&self, entity_id: &EntityId, memory_id: &MemoryId) -> Result<()> {
        let mention = EntityMention::new(entity_id.clone(), memory_id.clone());
        self.backend.store_mention(&mention)
    }

    /// Gets all mentions of an entity.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_mentions(&self, entity_id: &EntityId) -> Result<Vec<EntityMention>> {
        self.backend.get_mentions_for_entity(entity_id)
    }

    /// Gets all entities mentioned in a memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_entities_in_memory(&self, memory_id: &MemoryId) -> Result<Vec<Entity>> {
        self.backend.get_entities_in_memory(memory_id)
    }

    /// Removes entity mentions when a memory is deleted.
    ///
    /// # Returns
    ///
    /// Number of mentions removed.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion fails.
    pub fn remove_mentions_for_memory(&self, memory_id: &MemoryId) -> Result<usize> {
        self.backend.delete_mentions_for_memory(memory_id)
    }

    // =========================================================================
    // Graph Traversal
    // =========================================================================

    /// Traverses the graph from a starting entity.
    ///
    /// Performs breadth-first search up to the specified depth.
    ///
    /// # Arguments
    ///
    /// * `start` - Starting entity ID
    /// * `max_depth` - Maximum traversal depth
    /// * `relationship_types` - Optional filter for relationship types
    /// * `min_confidence` - Optional minimum confidence threshold
    ///
    /// # Returns
    ///
    /// Entities and relationships discovered during traversal.
    ///
    /// # Errors
    ///
    /// Returns an error if the traversal fails.
    pub fn traverse(
        &self,
        start: &EntityId,
        max_depth: u32,
        relationship_types: Option<&[RelationshipType]>,
        min_confidence: Option<f32>,
    ) -> Result<TraversalResult> {
        self.backend
            .traverse(start, max_depth, relationship_types, min_confidence)
    }

    /// Finds the shortest path between two entities.
    ///
    /// # Arguments
    ///
    /// * `from` - Starting entity ID
    /// * `to` - Target entity ID
    /// * `max_depth` - Maximum path length
    ///
    /// # Returns
    ///
    /// `Some(result)` with the path if found, `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the search fails.
    pub fn find_path(
        &self,
        from: &EntityId,
        to: &EntityId,
        max_depth: u32,
    ) -> Result<Option<TraversalResult>> {
        self.backend.find_path(from, to, max_depth)
    }

    /// Gets neighbors of an entity within a given depth.
    ///
    /// Convenience method for single-depth traversal.
    ///
    /// # Errors
    ///
    /// Returns an error if the traversal fails.
    pub fn get_neighbors(&self, entity_id: &EntityId, depth: u32) -> Result<Vec<Entity>> {
        let result = self.backend.traverse(entity_id, depth, None, None)?;
        // Exclude the starting entity
        Ok(result
            .entities
            .into_iter()
            .filter(|e| e.id != *entity_id)
            .collect())
    }

    // =========================================================================
    // Temporal Queries
    // =========================================================================

    /// Queries entities at a specific point in time.
    ///
    /// Uses bitemporal filtering to find entities that were:
    /// - Valid at the specified time (`valid_at`)
    /// - Known in the system at the specified time (`as_of`)
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn query_entities_at(
        &self,
        query: &EntityQuery,
        point: &BitemporalPoint,
    ) -> Result<Vec<Entity>> {
        self.backend.query_entities_at(query, point)
    }

    /// Queries relationships at a specific point in time.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn query_relationships_at(
        &self,
        query: &RelationshipQuery,
        point: &BitemporalPoint,
    ) -> Result<Vec<Relationship>> {
        self.backend.query_relationships_at(query, point)
    }

    /// Closes an entity's valid time (marks it as no longer valid).
    ///
    /// Used when an entity is superseded or becomes invalid.
    ///
    /// # Errors
    ///
    /// Returns an error if the entity doesn't exist or update fails.
    pub fn close_entity_valid_time(&self, id: &EntityId, end_time: i64) -> Result<()> {
        self.backend.close_entity_valid_time(id, end_time)
    }

    /// Closes a relationship's valid time.
    ///
    /// # Errors
    ///
    /// Returns an error if the relationship doesn't exist or update fails.
    pub fn close_relationship_valid_time(
        &self,
        from: &EntityId,
        to: &EntityId,
        relationship_type: RelationshipType,
        end_time: i64,
    ) -> Result<()> {
        self.backend
            .close_relationship_valid_time(from, to, relationship_type, end_time)
    }

    // =========================================================================
    // Statistics
    // =========================================================================

    /// Gets graph statistics.
    ///
    /// # Errors
    ///
    /// Returns an error if the statistics cannot be retrieved.
    pub fn get_stats(&self) -> Result<GraphStats> {
        self.backend.get_stats()
    }

    /// Clears all graph data.
    ///
    /// **Warning**: This permanently deletes all entities, relationships, and mentions.
    ///
    /// # Errors
    ///
    /// Returns an error if the clear operation fails.
    pub fn clear(&self) -> Result<()> {
        self.backend.clear()
    }
}

/// Simple name similarity using Jaccard index on character bigrams.
fn name_similarity(a: &str, b: &str) -> f32 {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();

    if a_lower == b_lower {
        return 1.0;
    }

    let a_bigrams: std::collections::HashSet<_> = a_lower
        .chars()
        .collect::<Vec<_>>()
        .windows(2)
        .map(|w| (w[0], w[1]))
        .collect();

    let b_bigrams: std::collections::HashSet<_> = b_lower
        .chars()
        .collect::<Vec<_>>()
        .windows(2)
        .map(|w| (w[0], w[1]))
        .collect();

    if a_bigrams.is_empty() || b_bigrams.is_empty() {
        return 0.0;
    }

    let intersection = a_bigrams.intersection(&b_bigrams).count();
    let union = a_bigrams.union(&b_bigrams).count();

    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::graph::InMemoryGraphBackend;

    fn create_service() -> GraphService<InMemoryGraphBackend> {
        GraphService::new(InMemoryGraphBackend::new())
    }

    fn create_entity(name: &str, entity_type: EntityType) -> Entity {
        Entity::new(entity_type, name, Domain::for_user())
    }

    #[test]
    fn test_store_and_get_entity() {
        let service = create_service();
        let entity = create_entity("Alice", EntityType::Person);

        service.store_entity(&entity).unwrap();
        let retrieved = service.get_entity(&entity.id).unwrap();

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Alice");
    }

    #[test]
    fn test_find_by_type() {
        let service = create_service();

        service
            .store_entity(&create_entity("Alice", EntityType::Person))
            .unwrap();
        service
            .store_entity(&create_entity("Bob", EntityType::Person))
            .unwrap();
        service
            .store_entity(&create_entity("Acme", EntityType::Organization))
            .unwrap();

        let people = service.find_by_type(EntityType::Person, 10).unwrap();
        assert_eq!(people.len(), 2);

        let orgs = service.find_by_type(EntityType::Organization, 10).unwrap();
        assert_eq!(orgs.len(), 1);
    }

    #[test]
    fn test_relate_entities() {
        let service = create_service();

        let alice = create_entity("Alice", EntityType::Person);
        let acme = create_entity("Acme", EntityType::Organization);

        service.store_entity(&alice).unwrap();
        service.store_entity(&acme).unwrap();

        let rel = service
            .relate(&alice.id, &acme.id, RelationshipType::WorksAt)
            .unwrap();

        assert_eq!(rel.from_entity, alice.id);
        assert_eq!(rel.to_entity, acme.id);
        assert_eq!(rel.relationship_type, RelationshipType::WorksAt);
    }

    #[test]
    fn test_relate_nonexistent_entity() {
        let service = create_service();

        let alice = create_entity("Alice", EntityType::Person);
        service.store_entity(&alice).unwrap();

        let fake_id = EntityId::generate();
        let result = service.relate(&alice.id, &fake_id, RelationshipType::WorksAt);

        assert!(result.is_err());
    }

    #[test]
    fn test_get_neighbors() {
        let service = create_service();

        let alice = create_entity("Alice", EntityType::Person);
        let bob = create_entity("Bob", EntityType::Person);
        let acme = create_entity("Acme", EntityType::Organization);

        service.store_entity(&alice).unwrap();
        service.store_entity(&bob).unwrap();
        service.store_entity(&acme).unwrap();

        service
            .relate(&alice.id, &bob.id, RelationshipType::RelatesTo)
            .unwrap();
        service
            .relate(&alice.id, &acme.id, RelationshipType::WorksAt)
            .unwrap();

        let neighbors = service.get_neighbors(&alice.id, 1).unwrap();
        assert_eq!(neighbors.len(), 2);
    }

    #[test]
    fn test_find_path() {
        let service = create_service();

        let a = create_entity("A", EntityType::Concept);
        let b = create_entity("B", EntityType::Concept);
        let c = create_entity("C", EntityType::Concept);

        service.store_entity(&a).unwrap();
        service.store_entity(&b).unwrap();
        service.store_entity(&c).unwrap();

        service
            .relate(&a.id, &b.id, RelationshipType::RelatesTo)
            .unwrap();
        service
            .relate(&b.id, &c.id, RelationshipType::RelatesTo)
            .unwrap();

        let path = service.find_path(&a.id, &c.id, 5).unwrap();
        assert!(path.is_some());
        assert_eq!(path.unwrap().entities.len(), 3);
    }

    #[test]
    fn test_record_and_get_mentions() {
        let service = create_service();

        let alice = create_entity("Alice", EntityType::Person);
        service.store_entity(&alice).unwrap();

        let mem1 = MemoryId::new("mem_1");
        let mem2 = MemoryId::new("mem_2");

        service.record_mention(&alice.id, &mem1).unwrap();
        service.record_mention(&alice.id, &mem2).unwrap();

        let mentions = service.get_mentions(&alice.id).unwrap();
        assert_eq!(mentions.len(), 2);
    }

    #[test]
    fn test_get_stats() {
        let service = create_service();

        service
            .store_entity(&create_entity("Alice", EntityType::Person))
            .unwrap();
        service
            .store_entity(&create_entity("Bob", EntityType::Person))
            .unwrap();

        let stats = service.get_stats().unwrap();
        assert_eq!(stats.entity_count, 2);
    }

    #[test]
    fn test_name_similarity() {
        assert!((name_similarity("Alice", "Alice") - 1.0).abs() < f32::EPSILON);
        assert!((name_similarity("alice", "ALICE") - 1.0).abs() < f32::EPSILON);
        // "Alice" vs "Alicia" share 3 bigrams out of 6 unique = 0.5 Jaccard
        assert!(name_similarity("Alice", "Alicia") >= 0.5);
        assert!(name_similarity("Alice", "Bob") < 0.3);
    }

    #[test]
    fn test_find_duplicates() {
        let service = create_service();

        let alice1 = create_entity("Alice Smith", EntityType::Person);
        let alice2 = create_entity("Alice Smithson", EntityType::Person);
        let bob = create_entity("Bob Jones", EntityType::Person);

        service.store_entity(&alice1).unwrap();
        service.store_entity(&alice2).unwrap();
        service.store_entity(&bob).unwrap();

        let duplicates = service.find_duplicates(&alice1, 0.5).unwrap();
        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates[0].name, "Alice Smithson");
    }

    #[test]
    fn test_store_entity_upsert() {
        let service = create_service();
        let mut entity = create_entity("Alice", EntityType::Person);
        service.store_entity(&entity).unwrap();

        // Update and store again (upsert behavior)
        entity.aliases = vec!["Ali".to_string()];
        entity.confidence = 0.95;
        service.store_entity(&entity).unwrap();

        let retrieved = service.get_entity(&entity.id).unwrap().unwrap();
        assert_eq!(retrieved.aliases, vec!["Ali".to_string()]);
        assert!((retrieved.confidence - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn test_delete_entity() {
        let service = create_service();
        let entity = create_entity("Alice", EntityType::Person);
        service.store_entity(&entity).unwrap();

        service.delete_entity(&entity.id).unwrap();
        let retrieved = service.get_entity(&entity.id).unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_relationship_with_properties() {
        let service = create_service();

        let rust = create_entity("Rust", EntityType::Technology);
        let cargo = create_entity("Cargo", EntityType::Technology);

        service.store_entity(&rust).unwrap();
        service.store_entity(&cargo).unwrap();

        let rel = service
            .relate(&rust.id, &cargo.id, RelationshipType::Uses)
            .unwrap();

        // Relationship should have the correct type
        assert_eq!(rel.relationship_type, RelationshipType::Uses);
        assert_eq!(rel.from_entity, rust.id);
        assert_eq!(rel.to_entity, cargo.id);
    }

    #[test]
    fn test_get_outgoing_relationships() {
        let service = create_service();

        let alice = create_entity("Alice", EntityType::Person);
        let bob = create_entity("Bob", EntityType::Person);
        let acme = create_entity("Acme", EntityType::Organization);

        service.store_entity(&alice).unwrap();
        service.store_entity(&bob).unwrap();
        service.store_entity(&acme).unwrap();

        service
            .relate(&alice.id, &bob.id, RelationshipType::RelatesTo)
            .unwrap();
        service
            .relate(&alice.id, &acme.id, RelationshipType::WorksAt)
            .unwrap();

        let rels = service.get_outgoing_relationships(&alice.id).unwrap();
        assert_eq!(rels.len(), 2);
    }

    #[test]
    fn test_no_path_found() {
        let service = create_service();

        let a = create_entity("A", EntityType::Concept);
        let b = create_entity("B", EntityType::Concept);

        service.store_entity(&a).unwrap();
        service.store_entity(&b).unwrap();

        // No relationship between A and B
        let path = service.find_path(&a.id, &b.id, 5).unwrap();
        assert!(path.is_none());
    }

    #[test]
    fn test_traversal_depth_limit() {
        let service = create_service();

        // Create chain: A -> B -> C -> D
        let a = create_entity("A", EntityType::Concept);
        let b = create_entity("B", EntityType::Concept);
        let c = create_entity("C", EntityType::Concept);
        let d = create_entity("D", EntityType::Concept);

        service.store_entity(&a).unwrap();
        service.store_entity(&b).unwrap();
        service.store_entity(&c).unwrap();
        service.store_entity(&d).unwrap();

        service
            .relate(&a.id, &b.id, RelationshipType::RelatesTo)
            .unwrap();
        service
            .relate(&b.id, &c.id, RelationshipType::RelatesTo)
            .unwrap();
        service
            .relate(&c.id, &d.id, RelationshipType::RelatesTo)
            .unwrap();

        // Depth 1 should only find B
        let neighbors = service.get_neighbors(&a.id, 1).unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].name, "B");

        // Depth 2 should find B and C
        let neighbors = service.get_neighbors(&a.id, 2).unwrap();
        assert_eq!(neighbors.len(), 2);
    }

    #[test]
    fn test_merge_entities() {
        let service = create_service();

        let alice1 = create_entity("Alice Smith", EntityType::Person);
        let alice2 = create_entity("A. Smith", EntityType::Person);
        let bob = create_entity("Bob", EntityType::Person);

        service.store_entity(&alice1).unwrap();
        service.store_entity(&alice2).unwrap();
        service.store_entity(&bob).unwrap();

        // Create relationship with alice2
        service
            .relate(&alice2.id, &bob.id, RelationshipType::RelatesTo)
            .unwrap();

        // Merge alice1 and alice2 into canonical "Alice Smith"
        let merged = service
            .merge_entities(&[alice1.id, alice2.id.clone()], "Alice Smith")
            .unwrap();

        // Merged entity should have the canonical name
        assert_eq!(merged.name, "Alice Smith");

        // alice2 should be gone (merged)
        let retrieved = service.get_entity(&alice2.id).unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_find_by_name() {
        let service = create_service();

        let alice = create_entity("Alice", EntityType::Person);
        let bob = create_entity("Bob", EntityType::Person);

        service.store_entity(&alice).unwrap();
        service.store_entity(&bob).unwrap();

        let found = service.find_by_name("Alice", None, None, 10).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Alice");
    }

    #[test]
    fn test_find_by_name_with_type_filter() {
        let service = create_service();

        let alice_person = create_entity("Alice", EntityType::Person);
        let alice_tech = create_entity("Alice", EntityType::Technology);

        service.store_entity(&alice_person).unwrap();
        service.store_entity(&alice_tech).unwrap();

        let found = service
            .find_by_name("Alice", Some(EntityType::Person), None, 10)
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].entity_type, EntityType::Person);
    }

    #[test]
    fn test_empty_graph_stats() {
        let service = create_service();
        let stats = service.get_stats().unwrap();

        assert_eq!(stats.entity_count, 0);
        assert_eq!(stats.relationship_count, 0);
    }

    #[test]
    fn test_record_duplicate_mention() {
        let service = create_service();

        let alice = create_entity("Alice", EntityType::Person);
        service.store_entity(&alice).unwrap();

        let mem = MemoryId::new("mem_1");

        // Record same mention twice
        service.record_mention(&alice.id, &mem).unwrap();
        service.record_mention(&alice.id, &mem).unwrap();

        // Should not create duplicates (implementation may dedupe)
        let mentions = service.get_mentions(&alice.id).unwrap();
        assert!(!mentions.is_empty());
    }
}
