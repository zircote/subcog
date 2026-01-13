//! Graph backend trait for knowledge graph storage.
//!
//! The graph layer provides entity and relationship storage with temporal tracking,
//! enabling knowledge graph construction and traversal.
//!
//! # Available Implementations
//!
//! | Backend | Use Case | Features |
//! |---------|----------|----------|
//! | `SqliteGraphBackend` | Default; embedded | Recursive CTEs for traversal |
//! | `InMemoryGraphBackend` | Testing | Fast, no persistence |
//! | `Neo4jGraphBackend` | Future | Native graph operations |
//!
//! # Error Modes and Guarantees
//!
//! All backends return `Result<T>` with errors propagated via [`crate::Error`].
//!
//! ## Entity Operations
//!
//! | Operation | Complexity | Notes |
//! |-----------|------------|-------|
//! | `store_entity` | O(1) | Insert or update |
//! | `get_entity` | O(1) | By ID lookup |
//! | `query_entities` | O(log n) | With filters |
//! | `delete_entity` | O(k) | k = relationship count |
//!
//! ## Relationship Operations
//!
//! | Operation | Complexity | Notes |
//! |-----------|------------|-------|
//! | `store_relationship` | O(1) | Insert or update |
//! | `query_relationships` | O(log n) | With filters |
//! | `traverse` | O(b^d) | b = branching, d = depth |
//!
//! ## Temporal Queries
//!
//! Bitemporal queries filter by both `valid_time` and `transaction_time`:
//! - `valid_at`: Filter entities/relationships valid at a point in time
//! - `as_of`: Filter by when records were known to the system
//!
//! # Example
//!
//! ```rust,ignore
//! use subcog::storage::traits::GraphBackend;
//! use subcog::models::graph::{Entity, EntityType, EntityQuery};
//!
//! // Query all Person entities with confidence > 0.8
//! let query = EntityQuery::new()
//!     .with_type(EntityType::Person)
//!     .with_min_confidence(0.8)
//!     .with_limit(20);
//!
//! let people = backend.query_entities(&query)?;
//! ```

use crate::Result;
use crate::models::graph::{
    Entity, EntityId, EntityMention, EntityQuery, EntityType, Relationship, RelationshipQuery,
    RelationshipType, TraversalResult,
};
use crate::models::temporal::BitemporalPoint;
use crate::models::{Domain, MemoryId};

/// Trait for graph layer backends.
///
/// Graph backends provide entity and relationship storage for knowledge graph
/// construction, with support for temporal queries and graph traversal.
///
/// # Implementor Notes
///
/// - Methods use `&self` to enable sharing via `Arc<dyn GraphBackend>`
/// - Use interior mutability (e.g., `Mutex<Connection>`) for mutable state
/// - Implement `traverse()` with recursive CTEs or similar for efficient multi-hop queries
/// - Support bitemporal filtering on all query methods
/// - Entity deletion should cascade to relationships (or return error if referenced)
pub trait GraphBackend: Send + Sync {
    // ========================================================================
    // Entity CRUD Operations
    // ========================================================================

    /// Stores an entity in the graph.
    ///
    /// If an entity with the same ID exists, it is updated.
    /// Uses interior mutability for thread-safe concurrent access.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage operation fails.
    fn store_entity(&self, entity: &Entity) -> Result<()>;

    /// Retrieves an entity by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the lookup operation fails.
    fn get_entity(&self, id: &EntityId) -> Result<Option<Entity>>;

    /// Queries entities with optional filters.
    ///
    /// Returns entities matching the query criteria, ordered by relevance
    /// (mention count, confidence, or recency depending on query).
    ///
    /// # Errors
    ///
    /// Returns an error if the query operation fails.
    fn query_entities(&self, query: &EntityQuery) -> Result<Vec<Entity>>;

    /// Deletes an entity by ID.
    ///
    /// This also removes all relationships involving the entity and
    /// all entity mentions.
    ///
    /// Returns `true` if the entity was deleted, `false` if not found.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion operation fails.
    fn delete_entity(&self, id: &EntityId) -> Result<bool>;

    /// Merges multiple entities into a canonical entity.
    ///
    /// The first entity ID becomes the canonical entity. All relationships
    /// from the other entities are re-pointed to the canonical entity,
    /// and the other entities are deleted.
    ///
    /// # Arguments
    ///
    /// * `entity_ids` - Entity IDs to merge (first is canonical)
    /// * `canonical_name` - New canonical name for the merged entity
    ///
    /// # Errors
    ///
    /// Returns an error if any entity is not found or the merge fails.
    fn merge_entities(&self, entity_ids: &[EntityId], canonical_name: &str) -> Result<Entity>;

    /// Finds entities by name using fuzzy matching.
    ///
    /// Searches both canonical names and aliases.
    ///
    /// # Errors
    ///
    /// Returns an error if the search operation fails.
    fn find_entities_by_name(
        &self,
        name: &str,
        entity_type: Option<EntityType>,
        domain: Option<&Domain>,
        limit: usize,
    ) -> Result<Vec<Entity>>;

    // ========================================================================
    // Relationship CRUD Operations
    // ========================================================================

    /// Stores a relationship in the graph.
    ///
    /// If a relationship between the same entities with the same type exists,
    /// it may be updated or a new version created (depending on temporal settings).
    ///
    /// # Errors
    ///
    /// Returns an error if the storage operation fails or if either entity
    /// referenced by the relationship does not exist.
    fn store_relationship(&self, relationship: &Relationship) -> Result<()>;

    /// Queries relationships with optional filters.
    ///
    /// Returns relationships matching the query criteria.
    ///
    /// # Errors
    ///
    /// Returns an error if the query operation fails.
    fn query_relationships(&self, query: &RelationshipQuery) -> Result<Vec<Relationship>>;

    /// Deletes relationships matching the query.
    ///
    /// Returns the number of relationships deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion operation fails.
    fn delete_relationships(&self, query: &RelationshipQuery) -> Result<usize>;

    /// Gets all relationship types between two entities.
    ///
    /// # Errors
    ///
    /// Returns an error if the query operation fails.
    fn get_relationship_types(
        &self,
        from_entity: &EntityId,
        to_entity: &EntityId,
    ) -> Result<Vec<RelationshipType>>;

    // ========================================================================
    // Entity Mention Operations
    // ========================================================================

    /// Stores an entity mention (link between entity and memory).
    ///
    /// # Errors
    ///
    /// Returns an error if the storage operation fails.
    fn store_mention(&self, mention: &EntityMention) -> Result<()>;

    /// Gets all mentions of an entity.
    ///
    /// Returns memory IDs where the entity was mentioned, with confidence scores.
    ///
    /// # Errors
    ///
    /// Returns an error if the query operation fails.
    fn get_mentions_for_entity(&self, entity_id: &EntityId) -> Result<Vec<EntityMention>>;

    /// Gets all entities mentioned in a memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the query operation fails.
    fn get_entities_in_memory(&self, memory_id: &MemoryId) -> Result<Vec<Entity>>;

    /// Deletes all mentions of an entity.
    ///
    /// Returns the number of mentions deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion operation fails.
    fn delete_mentions_for_entity(&self, entity_id: &EntityId) -> Result<usize>;

    /// Deletes all entity mentions for a memory.
    ///
    /// Returns the number of mentions deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion operation fails.
    fn delete_mentions_for_memory(&self, memory_id: &MemoryId) -> Result<usize>;

    // ========================================================================
    // Graph Traversal Operations
    // ========================================================================

    /// Traverses the graph from a starting entity.
    ///
    /// Performs breadth-first traversal up to `max_depth` hops, collecting
    /// all reachable entities and the relationships used to reach them.
    ///
    /// # Arguments
    ///
    /// * `start` - Starting entity ID
    /// * `max_depth` - Maximum traversal depth (1 = immediate neighbors)
    /// * `relationship_types` - Optional filter for relationship types
    /// * `min_confidence` - Minimum confidence threshold for relationships
    ///
    /// # Errors
    ///
    /// Returns an error if the traversal operation fails.
    fn traverse(
        &self,
        start: &EntityId,
        max_depth: u32,
        relationship_types: Option<&[RelationshipType]>,
        min_confidence: Option<f32>,
    ) -> Result<TraversalResult>;

    /// Finds the shortest path between two entities.
    ///
    /// Returns `None` if no path exists within `max_depth`.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn find_path(
        &self,
        from: &EntityId,
        to: &EntityId,
        max_depth: u32,
    ) -> Result<Option<TraversalResult>>;

    /// Gets entities related to a given entity within N hops.
    ///
    /// Convenience method combining traversal with entity extraction.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn get_related_entities(
        &self,
        entity_id: &EntityId,
        max_depth: u32,
        limit: usize,
    ) -> Result<Vec<Entity>> {
        let result = self.traverse(entity_id, max_depth, None, None)?;
        let entities = result
            .entities
            .into_iter()
            .filter(|e| e.id != *entity_id) // Exclude starting entity
            .take(limit)
            .collect();
        Ok(entities)
    }

    // ========================================================================
    // Temporal Query Operations
    // ========================================================================

    /// Queries entities at a specific point in bitemporal space.
    ///
    /// Returns entities that were valid at `point.valid_at` and were known
    /// to the system as of `point.as_of`.
    ///
    /// # Errors
    ///
    /// Returns an error if the query operation fails.
    fn query_entities_at(
        &self,
        query: &EntityQuery,
        point: &BitemporalPoint,
    ) -> Result<Vec<Entity>>;

    /// Queries relationships at a specific point in bitemporal space.
    ///
    /// # Errors
    ///
    /// Returns an error if the query operation fails.
    fn query_relationships_at(
        &self,
        query: &RelationshipQuery,
        point: &BitemporalPoint,
    ) -> Result<Vec<Relationship>>;

    /// Closes (ends) an entity's valid time at the given timestamp.
    ///
    /// This marks the entity as no longer valid from the given time forward,
    /// without deleting historical data.
    ///
    /// # Errors
    ///
    /// Returns an error if the entity is not found or the operation fails.
    fn close_entity_valid_time(&self, id: &EntityId, end_time: i64) -> Result<()>;

    /// Closes (ends) a relationship's valid time at the given timestamp.
    ///
    /// # Errors
    ///
    /// Returns an error if the relationship is not found or the operation fails.
    fn close_relationship_valid_time(
        &self,
        from_entity: &EntityId,
        to_entity: &EntityId,
        relationship_type: RelationshipType,
        end_time: i64,
    ) -> Result<()>;

    // ========================================================================
    // Utility Operations
    // ========================================================================

    /// Returns statistics about the graph.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn get_stats(&self) -> Result<GraphStats>;

    /// Clears all graph data.
    ///
    /// Use with caution - this removes all entities, relationships, and mentions.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn clear(&self) -> Result<()>;
}

/// Statistics about the knowledge graph.
#[derive(Debug, Clone, Default)]
pub struct GraphStats {
    /// Total number of entities.
    pub entity_count: usize,
    /// Number of entities by type.
    pub entities_by_type: std::collections::HashMap<EntityType, usize>,
    /// Total number of relationships.
    pub relationship_count: usize,
    /// Number of relationships by type.
    pub relationships_by_type: std::collections::HashMap<RelationshipType, usize>,
    /// Total number of entity mentions.
    pub mention_count: usize,
    /// Average relationships per entity.
    pub avg_relationships_per_entity: f32,
}

impl GraphStats {
    /// Creates empty stats.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_stats_default() {
        let stats = GraphStats::default();
        assert_eq!(stats.entity_count, 0);
        assert_eq!(stats.relationship_count, 0);
        assert_eq!(stats.mention_count, 0);
    }
}
