//! In-memory graph backend for testing.
//!
//! Provides a fast, non-persistent implementation of [`GraphBackend`] for use
//! in unit tests and development scenarios.

// Allow collapsible_if for clearer nested conditional logic in query matching.
#![allow(clippy::collapsible_if)]
// Allow cognitive_complexity for graph traversal algorithms.
#![allow(clippy::cognitive_complexity)]
// Allow excessive_nesting for domain filtering logic in queries.
#![allow(clippy::excessive_nesting)]

use crate::models::graph::{
    Entity, EntityId, EntityMention, EntityQuery, EntityType, Relationship, RelationshipQuery,
    RelationshipType, TraversalResult,
};
use crate::models::temporal::BitemporalPoint;
use crate::models::{Domain, MemoryId};
use crate::storage::traits::graph::{GraphBackend, GraphStats};
use crate::{Error, Result};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::RwLock;

/// In-memory graph backend for testing.
///
/// Uses `RwLock` for thread-safe access with reader-writer semantics.
/// Data is not persisted between runs.
///
/// # Example
///
/// ```rust,ignore
/// use subcog::storage::graph::InMemoryGraphBackend;
/// use subcog::storage::traits::GraphBackend;
///
/// let backend = InMemoryGraphBackend::new();
/// // Use for testing...
/// ```
#[derive(Debug, Default)]
pub struct InMemoryGraphBackend {
    entities: RwLock<HashMap<EntityId, Entity>>,
    relationships: RwLock<Vec<Relationship>>,
    mentions: RwLock<Vec<EntityMention>>,
}

impl InMemoryGraphBackend {
    /// Creates a new empty in-memory graph backend.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of entities stored.
    #[must_use]
    pub fn entity_count(&self) -> usize {
        self.entities.read().map(|e| e.len()).unwrap_or(0)
    }

    /// Returns the number of relationships stored.
    #[must_use]
    pub fn relationship_count(&self) -> usize {
        self.relationships.read().map(|r| r.len()).unwrap_or(0)
    }

    /// Returns the number of mentions stored.
    #[must_use]
    pub fn mention_count(&self) -> usize {
        self.mentions.read().map(|m| m.len()).unwrap_or(0)
    }

    /// Checks if an entity matches the query criteria.
    fn entity_matches_query(entity: &Entity, query: &EntityQuery) -> bool {
        if let Some(ref et) = query.entity_type {
            if entity.entity_type != *et {
                return false;
            }
        }

        if let Some(ref name) = query.name {
            let name_lower = name.to_lowercase();
            if !entity.name.to_lowercase().contains(&name_lower)
                && !entity
                    .aliases
                    .iter()
                    .any(|a| a.to_lowercase().contains(&name_lower))
            {
                return false;
            }
        }

        if let Some(ref domain) = query.domain {
            if let Some(ref org) = domain.organization {
                if entity.domain.organization.as_ref() != Some(org) {
                    return false;
                }
            }
            if let Some(ref project) = domain.project {
                if entity.domain.project.as_ref() != Some(project) {
                    return false;
                }
            }
            if let Some(ref repo) = domain.repository {
                if entity.domain.repository.as_ref() != Some(repo) {
                    return false;
                }
            }
        }

        if let Some(min_conf) = query.min_confidence {
            if entity.confidence < min_conf {
                return false;
            }
        }

        if let Some(valid_at) = query.valid_at {
            if !entity.valid_time.contains(valid_at) {
                return false;
            }
        }

        true
    }

    /// Checks if a relationship matches the query criteria.
    fn relationship_matches_query(rel: &Relationship, query: &RelationshipQuery) -> bool {
        if let Some(ref from) = query.from_entity {
            if rel.from_entity != *from {
                return false;
            }
        }

        if let Some(ref to) = query.to_entity {
            if rel.to_entity != *to {
                return false;
            }
        }

        if let Some(ref rt) = query.relationship_type {
            if rel.relationship_type != *rt {
                return false;
            }
        }

        if let Some(min_conf) = query.min_confidence {
            if rel.confidence < min_conf {
                return false;
            }
        }

        if let Some(valid_at) = query.valid_at {
            if !rel.valid_time.contains(valid_at) {
                return false;
            }
        }

        true
    }
}

impl GraphBackend for InMemoryGraphBackend {
    fn store_entity(&self, entity: &Entity) -> Result<()> {
        let mut entities = self.entities.write().map_err(|_| Error::OperationFailed {
            operation: "store_entity".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        entities.insert(entity.id.clone(), entity.clone());
        Ok(())
    }

    fn get_entity(&self, id: &EntityId) -> Result<Option<Entity>> {
        let entities = self.entities.read().map_err(|_| Error::OperationFailed {
            operation: "get_entity".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        Ok(entities.get(id).cloned())
    }

    fn query_entities(&self, query: &EntityQuery) -> Result<Vec<Entity>> {
        let entities = self.entities.read().map_err(|_| Error::OperationFailed {
            operation: "query_entities".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        let limit = query.limit.unwrap_or(100);
        let offset = query.offset.unwrap_or(0);

        let mut results: Vec<Entity> = entities
            .values()
            .filter(|e| Self::entity_matches_query(e, query))
            .cloned()
            .collect();

        // Sort by mention_count desc, confidence desc
        results.sort_by(|a, b| {
            b.mention_count.cmp(&a.mention_count).then_with(|| {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });

        Ok(results.into_iter().skip(offset).take(limit).collect())
    }

    fn delete_entity(&self, id: &EntityId) -> Result<bool> {
        let mut entities = self.entities.write().map_err(|_| Error::OperationFailed {
            operation: "delete_entity".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        let existed = entities.remove(id).is_some();

        if existed {
            // Cascade delete relationships
            if let Ok(mut rels) = self.relationships.write() {
                rels.retain(|r| r.from_entity != *id && r.to_entity != *id);
            }

            // Cascade delete mentions
            if let Ok(mut mentions) = self.mentions.write() {
                mentions.retain(|m| m.entity_id != *id);
            }
        }

        Ok(existed)
    }

    fn merge_entities(&self, entity_ids: &[EntityId], canonical_name: &str) -> Result<Entity> {
        if entity_ids.is_empty() {
            return Err(Error::OperationFailed {
                operation: "merge_entities".to_string(),
                cause: "No entity IDs provided".to_string(),
            });
        }

        let mut entities = self.entities.write().map_err(|_| Error::OperationFailed {
            operation: "merge_entities".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        let canonical_id = &entity_ids[0];
        let canonical_entity =
            entities
                .get(canonical_id)
                .cloned()
                .ok_or_else(|| Error::OperationFailed {
                    operation: "merge_entities".to_string(),
                    cause: format!("Canonical entity not found: {}", canonical_id.as_str()),
                })?;

        // Collect aliases from all entities
        let mut all_aliases = canonical_entity.aliases.clone();
        all_aliases.push(canonical_entity.name.clone());

        for other_id in entity_ids.iter().skip(1) {
            if let Some(other) = entities.get(other_id) {
                all_aliases.push(other.name.clone());
                all_aliases.extend(other.aliases.clone());
            }
        }

        // Remove duplicates and canonical name
        all_aliases.sort();
        all_aliases.dedup();
        all_aliases.retain(|a| a != canonical_name);

        // Update relationships to point to canonical entity
        if let Ok(mut rels) = self.relationships.write() {
            for rel in rels.iter_mut() {
                for other_id in entity_ids.iter().skip(1) {
                    if rel.from_entity == *other_id {
                        rel.from_entity = canonical_id.clone();
                    }
                    if rel.to_entity == *other_id {
                        rel.to_entity = canonical_id.clone();
                    }
                }
            }
        }

        // Update mentions to point to canonical entity
        if let Ok(mut mentions) = self.mentions.write() {
            for mention in mentions.iter_mut() {
                for other_id in entity_ids.iter().skip(1) {
                    if mention.entity_id == *other_id {
                        mention.entity_id = canonical_id.clone();
                    }
                }
            }
        }

        // Remove merged entities
        for other_id in entity_ids.iter().skip(1) {
            entities.remove(other_id);
        }

        // Create merged entity
        let merged = Entity::new(
            canonical_entity.entity_type,
            canonical_name,
            canonical_entity.domain.clone(),
        )
        .with_id(canonical_entity.id.clone())
        .with_confidence(canonical_entity.confidence)
        .with_aliases(all_aliases.clone());

        entities.insert(canonical_id.clone(), merged.clone());

        Ok(merged)
    }

    fn find_entities_by_name(
        &self,
        name: &str,
        entity_type: Option<EntityType>,
        domain: Option<&Domain>,
        limit: usize,
    ) -> Result<Vec<Entity>> {
        let query = EntityQuery {
            entity_type,
            name: Some(name.to_string()),
            domain: domain.cloned(),
            min_confidence: None,
            valid_at: None,
            limit: Some(limit),
            offset: None,
        };

        self.query_entities(&query)
    }

    fn store_relationship(&self, relationship: &Relationship) -> Result<()> {
        let mut rels = self
            .relationships
            .write()
            .map_err(|_| Error::OperationFailed {
                operation: "store_relationship".to_string(),
                cause: "Lock poisoned".to_string(),
            })?;

        // Check if relationship already exists (upsert)
        if let Some(existing) = rels.iter_mut().find(|r| {
            r.from_entity == relationship.from_entity
                && r.to_entity == relationship.to_entity
                && r.relationship_type == relationship.relationship_type
        }) {
            *existing = relationship.clone();
        } else {
            rels.push(relationship.clone());
        }

        Ok(())
    }

    fn query_relationships(&self, query: &RelationshipQuery) -> Result<Vec<Relationship>> {
        let rels = self
            .relationships
            .read()
            .map_err(|_| Error::OperationFailed {
                operation: "query_relationships".to_string(),
                cause: "Lock poisoned".to_string(),
            })?;

        let limit = query.limit.unwrap_or(100);

        let mut results: Vec<Relationship> = rels
            .iter()
            .filter(|r| Self::relationship_matches_query(r, query))
            .cloned()
            .collect();

        // Sort by confidence desc
        results.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results.into_iter().take(limit).collect())
    }

    fn delete_relationships(&self, query: &RelationshipQuery) -> Result<usize> {
        let mut rels = self
            .relationships
            .write()
            .map_err(|_| Error::OperationFailed {
                operation: "delete_relationships".to_string(),
                cause: "Lock poisoned".to_string(),
            })?;

        let before = rels.len();
        rels.retain(|r| !Self::relationship_matches_query(r, query));
        let after = rels.len();

        Ok(before - after)
    }

    fn get_relationship_types(
        &self,
        from_entity: &EntityId,
        to_entity: &EntityId,
    ) -> Result<Vec<RelationshipType>> {
        let rels = self
            .relationships
            .read()
            .map_err(|_| Error::OperationFailed {
                operation: "get_relationship_types".to_string(),
                cause: "Lock poisoned".to_string(),
            })?;

        let types: Vec<RelationshipType> = rels
            .iter()
            .filter(|r| r.from_entity == *from_entity && r.to_entity == *to_entity)
            .map(|r| r.relationship_type)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        Ok(types)
    }

    fn store_mention(&self, mention: &EntityMention) -> Result<()> {
        let mut mentions = self.mentions.write().map_err(|_| Error::OperationFailed {
            operation: "store_mention".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        // Check if mention already exists (upsert)
        if let Some(existing) = mentions
            .iter_mut()
            .find(|m| m.entity_id == mention.entity_id && m.memory_id == mention.memory_id)
        {
            *existing = mention.clone();
        } else {
            mentions.push(mention.clone());

            // Increment mention count on entity
            if let Ok(mut entities) = self.entities.write() {
                if let Some(entity) = entities.get_mut(&mention.entity_id) {
                    entity.mention_count += 1;
                }
            }
        }

        Ok(())
    }

    fn get_mentions_for_entity(&self, entity_id: &EntityId) -> Result<Vec<EntityMention>> {
        let mentions = self.mentions.read().map_err(|_| Error::OperationFailed {
            operation: "get_mentions_for_entity".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        Ok(mentions
            .iter()
            .filter(|m| m.entity_id == *entity_id)
            .cloned()
            .collect())
    }

    fn get_entities_in_memory(&self, memory_id: &MemoryId) -> Result<Vec<Entity>> {
        let mentions = self.mentions.read().map_err(|_| Error::OperationFailed {
            operation: "get_entities_in_memory".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        let entities = self.entities.read().map_err(|_| Error::OperationFailed {
            operation: "get_entities_in_memory".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        let entity_ids: HashSet<_> = mentions
            .iter()
            .filter(|m| m.memory_id == *memory_id)
            .map(|m| &m.entity_id)
            .collect();

        Ok(entities
            .values()
            .filter(|e| entity_ids.contains(&e.id))
            .cloned()
            .collect())
    }

    fn delete_mentions_for_entity(&self, entity_id: &EntityId) -> Result<usize> {
        let mut mentions = self.mentions.write().map_err(|_| Error::OperationFailed {
            operation: "delete_mentions_for_entity".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        let before = mentions.len();
        mentions.retain(|m| m.entity_id != *entity_id);
        let after = mentions.len();
        let deleted = before - after;

        // Reset mention count
        if let Ok(mut entities) = self.entities.write() {
            if let Some(entity) = entities.get_mut(entity_id) {
                entity.mention_count = 0;
            }
        }

        Ok(deleted)
    }

    fn delete_mentions_for_memory(&self, memory_id: &MemoryId) -> Result<usize> {
        let mut mentions = self.mentions.write().map_err(|_| Error::OperationFailed {
            operation: "delete_mentions_for_memory".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        // Get affected entity IDs before deletion
        let affected_entities: Vec<EntityId> = mentions
            .iter()
            .filter(|m| m.memory_id == *memory_id)
            .map(|m| m.entity_id.clone())
            .collect();

        let before = mentions.len();
        mentions.retain(|m| m.memory_id != *memory_id);
        let after = mentions.len();
        let deleted = before - after;

        // Decrement mention counts
        if let Ok(mut entities) = self.entities.write() {
            for entity_id in affected_entities {
                if let Some(entity) = entities.get_mut(&entity_id) {
                    entity.mention_count = entity.mention_count.saturating_sub(1);
                }
            }
        }

        Ok(deleted)
    }

    fn traverse(
        &self,
        start: &EntityId,
        max_depth: u32,
        relationship_types: Option<&[RelationshipType]>,
        min_confidence: Option<f32>,
    ) -> Result<TraversalResult> {
        let entities = self.entities.read().map_err(|_| Error::OperationFailed {
            operation: "traverse".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        let rels = self
            .relationships
            .read()
            .map_err(|_| Error::OperationFailed {
                operation: "traverse".to_string(),
                cause: "Lock poisoned".to_string(),
            })?;

        let mut visited: HashSet<EntityId> = HashSet::new();
        let mut queue: VecDeque<(EntityId, u32)> = VecDeque::new();
        let mut result_entities: Vec<Entity> = Vec::new();
        let mut result_relationships: Vec<Relationship> = Vec::new();

        // Start BFS
        queue.push_back((start.clone(), 0));
        visited.insert(start.clone());

        while let Some((current_id, depth)) = queue.pop_front() {
            // Add entity to results
            if let Some(entity) = entities.get(&current_id) {
                result_entities.push(entity.clone());
            }

            if depth >= max_depth {
                continue;
            }

            // Find outgoing relationships
            for rel in rels.iter() {
                if rel.from_entity != current_id {
                    continue;
                }

                // Filter by relationship type
                if let Some(types) = relationship_types {
                    if !types.contains(&rel.relationship_type) {
                        continue;
                    }
                }

                // Filter by confidence
                if let Some(min_conf) = min_confidence {
                    if rel.confidence < min_conf {
                        continue;
                    }
                }

                // Add relationship to results
                if !result_relationships.iter().any(|r| {
                    r.from_entity == rel.from_entity
                        && r.to_entity == rel.to_entity
                        && r.relationship_type == rel.relationship_type
                }) {
                    result_relationships.push(rel.clone());
                }

                // Enqueue next entity if not visited
                if !visited.contains(&rel.to_entity) {
                    visited.insert(rel.to_entity.clone());
                    queue.push_back((rel.to_entity.clone(), depth + 1));
                }
            }
        }

        let total_count = result_entities.len();

        Ok(TraversalResult {
            entities: result_entities,
            relationships: result_relationships,
            total_count,
        })
    }

    fn find_path(
        &self,
        from: &EntityId,
        to: &EntityId,
        max_depth: u32,
    ) -> Result<Option<TraversalResult>> {
        let entities = self.entities.read().map_err(|_| Error::OperationFailed {
            operation: "find_path".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        let rels = self
            .relationships
            .read()
            .map_err(|_| Error::OperationFailed {
                operation: "find_path".to_string(),
                cause: "Lock poisoned".to_string(),
            })?;

        // BFS to find shortest path
        let mut visited: HashMap<EntityId, (EntityId, Relationship)> = HashMap::new();
        let mut queue: VecDeque<(EntityId, u32)> = VecDeque::new();

        queue.push_back((from.clone(), 0));

        while let Some((current_id, depth)) = queue.pop_front() {
            if current_id == *to {
                // Reconstruct path
                let mut path_entities: Vec<Entity> = Vec::new();
                let mut path_relationships: Vec<Relationship> = Vec::new();
                let mut current = to.clone();

                while current != *from {
                    if let Some(entity) = entities.get(&current) {
                        path_entities.push(entity.clone());
                    }
                    if let Some((prev, rel)) = visited.get(&current) {
                        path_relationships.push(rel.clone());
                        current = prev.clone();
                    } else {
                        break;
                    }
                }

                // Add start entity
                if let Some(entity) = entities.get(from) {
                    path_entities.push(entity.clone());
                }

                path_entities.reverse();
                path_relationships.reverse();

                let total_count = path_entities.len();

                return Ok(Some(TraversalResult {
                    entities: path_entities,
                    relationships: path_relationships,
                    total_count,
                }));
            }

            if depth >= max_depth {
                continue;
            }

            // Find outgoing relationships
            for rel in rels.iter() {
                if rel.from_entity != current_id {
                    continue;
                }

                if !visited.contains_key(&rel.to_entity) && rel.to_entity != *from {
                    visited.insert(rel.to_entity.clone(), (current_id.clone(), rel.clone()));
                    queue.push_back((rel.to_entity.clone(), depth + 1));
                }
            }
        }

        Ok(None)
    }

    fn query_entities_at(
        &self,
        query: &EntityQuery,
        point: &BitemporalPoint,
    ) -> Result<Vec<Entity>> {
        let entities = self.entities.read().map_err(|_| Error::OperationFailed {
            operation: "query_entities_at".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        let limit = query.limit.unwrap_or(100);

        let mut results: Vec<Entity> = entities
            .values()
            .filter(|e| {
                Self::entity_matches_query(e, query)
                    && e.valid_time.contains(point.valid_at)
                    && e.transaction_time.was_known_at(point.as_of)
            })
            .cloned()
            .collect();

        results.sort_by(|a, b| {
            b.mention_count.cmp(&a.mention_count).then_with(|| {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });

        Ok(results.into_iter().take(limit).collect())
    }

    fn query_relationships_at(
        &self,
        query: &RelationshipQuery,
        point: &BitemporalPoint,
    ) -> Result<Vec<Relationship>> {
        let rels = self
            .relationships
            .read()
            .map_err(|_| Error::OperationFailed {
                operation: "query_relationships_at".to_string(),
                cause: "Lock poisoned".to_string(),
            })?;

        let limit = query.limit.unwrap_or(100);

        let mut results: Vec<Relationship> = rels
            .iter()
            .filter(|r| {
                Self::relationship_matches_query(r, query)
                    && r.valid_time.contains(point.valid_at)
                    && r.transaction_time.was_known_at(point.as_of)
            })
            .cloned()
            .collect();

        results.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results.into_iter().take(limit).collect())
    }

    fn close_entity_valid_time(&self, id: &EntityId, end_time: i64) -> Result<()> {
        let mut entities = self.entities.write().map_err(|_| Error::OperationFailed {
            operation: "close_entity_valid_time".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        let entity = entities.get_mut(id).ok_or_else(|| Error::OperationFailed {
            operation: "close_entity_valid_time".to_string(),
            cause: format!("Entity not found: {}", id.as_str()),
        })?;

        entity.valid_time = entity.valid_time.close_at(end_time);
        Ok(())
    }

    fn close_relationship_valid_time(
        &self,
        from_entity: &EntityId,
        to_entity: &EntityId,
        relationship_type: RelationshipType,
        end_time: i64,
    ) -> Result<()> {
        let mut rels = self
            .relationships
            .write()
            .map_err(|_| Error::OperationFailed {
                operation: "close_relationship_valid_time".to_string(),
                cause: "Lock poisoned".to_string(),
            })?;

        let rel = rels
            .iter_mut()
            .find(|r| {
                r.from_entity == *from_entity
                    && r.to_entity == *to_entity
                    && r.relationship_type == relationship_type
            })
            .ok_or_else(|| Error::OperationFailed {
                operation: "close_relationship_valid_time".to_string(),
                cause: "Relationship not found".to_string(),
            })?;

        rel.valid_time = rel.valid_time.close_at(end_time);
        Ok(())
    }

    fn get_stats(&self) -> Result<GraphStats> {
        let entities = self.entities.read().map_err(|_| Error::OperationFailed {
            operation: "get_stats".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        let rels = self
            .relationships
            .read()
            .map_err(|_| Error::OperationFailed {
                operation: "get_stats".to_string(),
                cause: "Lock poisoned".to_string(),
            })?;

        let mentions = self.mentions.read().map_err(|_| Error::OperationFailed {
            operation: "get_stats".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        let entity_count = entities.len();
        let relationship_count = rels.len();
        let mention_count = mentions.len();

        // Count by entity type
        let mut entities_by_type: HashMap<EntityType, usize> = HashMap::new();
        for entity in entities.values() {
            *entities_by_type.entry(entity.entity_type).or_insert(0) += 1;
        }

        // Count by relationship type
        let mut relationships_by_type: HashMap<RelationshipType, usize> = HashMap::new();
        for rel in rels.iter() {
            *relationships_by_type
                .entry(rel.relationship_type)
                .or_insert(0) += 1;
        }

        let avg_relationships_per_entity = if entity_count > 0 {
            relationship_count as f32 / entity_count as f32
        } else {
            0.0
        };

        Ok(GraphStats {
            entity_count,
            entities_by_type,
            relationship_count,
            relationships_by_type,
            mention_count,
            avg_relationships_per_entity,
        })
    }

    fn clear(&self) -> Result<()> {
        if let Ok(mut entities) = self.entities.write() {
            entities.clear();
        }
        if let Ok(mut rels) = self.relationships.write() {
            rels.clear();
        }
        if let Ok(mut mentions) = self.mentions.write() {
            mentions.clear();
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;
    use crate::models::temporal::ValidTimeRange;

    fn create_test_entity(name: &str, entity_type: EntityType) -> Entity {
        Entity::new(entity_type, name, Domain::for_user())
    }

    #[test]
    fn test_store_and_get_entity() {
        let backend = InMemoryGraphBackend::new();
        let entity = create_test_entity("Alice", EntityType::Person);

        backend.store_entity(&entity).unwrap();

        let retrieved = backend.get_entity(&entity.id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Alice");
    }

    #[test]
    fn test_query_entities() {
        let backend = InMemoryGraphBackend::new();

        backend
            .store_entity(&create_test_entity("Alice", EntityType::Person))
            .unwrap();
        backend
            .store_entity(&create_test_entity("Bob", EntityType::Person))
            .unwrap();
        backend
            .store_entity(&create_test_entity("Acme", EntityType::Organization))
            .unwrap();

        let query = EntityQuery::new().with_type(EntityType::Person);
        let results = backend.query_entities(&query).unwrap();

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_delete_entity_cascades() {
        let backend = InMemoryGraphBackend::new();

        let alice = create_test_entity("Alice", EntityType::Person);
        let bob = create_test_entity("Bob", EntityType::Person);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&bob).unwrap();

        // Create relationship
        backend
            .store_relationship(&Relationship::new(
                alice.id.clone(),
                bob.id.clone(),
                RelationshipType::RelatesTo,
            ))
            .unwrap();

        // Create mention
        backend
            .store_mention(&EntityMention::new(
                alice.id.clone(),
                MemoryId::new("mem_1"),
            ))
            .unwrap();

        // Delete Alice
        backend.delete_entity(&alice.id).unwrap();

        // Relationships should be gone
        assert_eq!(backend.relationship_count(), 0);

        // Mentions should be gone
        assert_eq!(backend.mention_count(), 0);
    }

    #[test]
    fn test_traverse() {
        let backend = InMemoryGraphBackend::new();

        let alice = create_test_entity("Alice", EntityType::Person);
        let acme = create_test_entity("Acme", EntityType::Organization);
        let project = create_test_entity("Project", EntityType::Concept);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&acme).unwrap();
        backend.store_entity(&project).unwrap();

        backend
            .store_relationship(&Relationship::new(
                alice.id.clone(),
                acme.id.clone(),
                RelationshipType::WorksAt,
            ))
            .unwrap();

        backend
            .store_relationship(&Relationship::new(
                acme.id.clone(),
                project.id.clone(),
                RelationshipType::Created,
            ))
            .unwrap();

        let result = backend.traverse(&alice.id, 2, None, None).unwrap();

        assert_eq!(result.entities.len(), 3);
        assert_eq!(result.relationships.len(), 2);
    }

    #[test]
    fn test_find_path() {
        let backend = InMemoryGraphBackend::new();

        let alice = create_test_entity("Alice", EntityType::Person);
        let bob = create_test_entity("Bob", EntityType::Person);
        let acme = create_test_entity("Acme", EntityType::Organization);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&bob).unwrap();
        backend.store_entity(&acme).unwrap();

        backend
            .store_relationship(&Relationship::new(
                alice.id.clone(),
                acme.id.clone(),
                RelationshipType::WorksAt,
            ))
            .unwrap();

        backend
            .store_relationship(&Relationship::new(
                acme.id.clone(),
                bob.id.clone(),
                RelationshipType::RelatesTo,
            ))
            .unwrap();

        let path = backend.find_path(&alice.id, &bob.id, 3).unwrap();
        assert!(path.is_some());
        assert_eq!(path.unwrap().entities.len(), 3);
    }

    #[test]
    fn test_temporal_query() {
        let backend = InMemoryGraphBackend::new();

        let entity = Entity::new(EntityType::Person, "Alice", Domain::for_user())
            .with_valid_time(ValidTimeRange::between(100, 200));

        backend.store_entity(&entity).unwrap();

        // Query at valid point
        let point = BitemporalPoint::new(150, i64::MAX);
        let results = backend
            .query_entities_at(&EntityQuery::new(), &point)
            .unwrap();
        assert_eq!(results.len(), 1);

        // Query at invalid point
        let point_before = BitemporalPoint::new(50, i64::MAX);
        let results_before = backend
            .query_entities_at(&EntityQuery::new(), &point_before)
            .unwrap();
        assert_eq!(results_before.len(), 0);
    }

    #[test]
    fn test_get_stats() {
        let backend = InMemoryGraphBackend::new();

        backend
            .store_entity(&create_test_entity("Alice", EntityType::Person))
            .unwrap();
        backend
            .store_entity(&create_test_entity("Acme", EntityType::Organization))
            .unwrap();

        let stats = backend.get_stats().unwrap();
        assert_eq!(stats.entity_count, 2);
        assert_eq!(stats.entities_by_type.get(&EntityType::Person), Some(&1));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Additional Entity Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_entity_upsert() {
        let backend = InMemoryGraphBackend::new();

        let mut entity = create_test_entity("Alice", EntityType::Person);
        backend.store_entity(&entity).unwrap();

        entity.confidence = 0.95;
        backend.store_entity(&entity).unwrap();

        let retrieved = backend.get_entity(&entity.id).unwrap().unwrap();
        assert!((retrieved.confidence - 0.95).abs() < f32::EPSILON);
        assert_eq!(backend.entity_count(), 1);
    }

    #[test]
    fn test_entity_aliases() {
        let backend = InMemoryGraphBackend::new();

        let entity = Entity::new(EntityType::Person, "Robert", Domain::for_user())
            .with_aliases(vec!["Bob".to_string(), "Rob".to_string()]);

        backend.store_entity(&entity).unwrap();

        let results = backend
            .find_entities_by_name("Bob", Some(EntityType::Person), None, 10)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Robert");
    }

    #[test]
    fn test_query_by_confidence() {
        let backend = InMemoryGraphBackend::new();

        let high =
            Entity::new(EntityType::Person, "Alice", Domain::for_user()).with_confidence(0.9);
        let low = Entity::new(EntityType::Person, "Bob", Domain::for_user()).with_confidence(0.3);

        backend.store_entity(&high).unwrap();
        backend.store_entity(&low).unwrap();

        let query = EntityQuery::new().with_min_confidence(0.5);
        let results = backend.query_entities(&query).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Alice");
    }

    #[test]
    fn test_merge_entities() {
        let backend = InMemoryGraphBackend::new();

        let alice1 = create_test_entity("Alice Smith", EntityType::Person);
        let alice2 = create_test_entity("A. Smith", EntityType::Person);

        backend.store_entity(&alice1).unwrap();
        backend.store_entity(&alice2).unwrap();

        let merged = backend
            .merge_entities(&[alice1.id.clone(), alice2.id.clone()], "Alice Smith")
            .unwrap();

        assert_eq!(merged.name, "Alice Smith");
        assert!(merged.aliases.contains(&"A. Smith".to_string()));
        assert!(backend.get_entity(&alice1.id).unwrap().is_some());
        assert!(backend.get_entity(&alice2.id).unwrap().is_none());
    }

    #[test]
    fn test_clear() {
        let backend = InMemoryGraphBackend::new();

        backend
            .store_entity(&create_test_entity("Alice", EntityType::Person))
            .unwrap();
        backend
            .store_entity(&create_test_entity("Bob", EntityType::Person))
            .unwrap();

        backend.clear().unwrap();
        assert_eq!(backend.entity_count(), 0);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Additional Relationship Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_relationship_upsert() {
        let backend = InMemoryGraphBackend::new();

        let alice = create_test_entity("Alice", EntityType::Person);
        let acme = create_test_entity("Acme", EntityType::Organization);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&acme).unwrap();

        let mut rel =
            Relationship::new(alice.id.clone(), acme.id.clone(), RelationshipType::WorksAt);
        backend.store_relationship(&rel).unwrap();

        rel.confidence = 0.95;
        backend.store_relationship(&rel).unwrap();

        let query = RelationshipQuery::new().from(alice.id.clone());
        let results = backend.query_relationships(&query).unwrap();

        assert_eq!(results.len(), 1);
        assert!((results[0].confidence - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn test_query_relationships_by_type() {
        let backend = InMemoryGraphBackend::new();

        let alice = create_test_entity("Alice", EntityType::Person);
        let bob = create_test_entity("Bob", EntityType::Person);
        let acme = create_test_entity("Acme", EntityType::Organization);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&bob).unwrap();
        backend.store_entity(&acme).unwrap();

        backend
            .store_relationship(&Relationship::new(
                alice.id.clone(),
                acme.id.clone(),
                RelationshipType::WorksAt,
            ))
            .unwrap();
        backend
            .store_relationship(&Relationship::new(
                alice.id.clone(),
                bob.id.clone(),
                RelationshipType::RelatesTo,
            ))
            .unwrap();

        let query = RelationshipQuery::new().with_type(RelationshipType::WorksAt);
        let results = backend.query_relationships(&query).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].to_entity, acme.id);
    }

    #[test]
    fn test_delete_relationships() {
        let backend = InMemoryGraphBackend::new();

        let alice = create_test_entity("Alice", EntityType::Person);
        let bob = create_test_entity("Bob", EntityType::Person);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&bob).unwrap();

        backend
            .store_relationship(&Relationship::new(
                alice.id.clone(),
                bob.id.clone(),
                RelationshipType::RelatesTo,
            ))
            .unwrap();

        let query = RelationshipQuery::new().from(alice.id.clone());
        let deleted = backend.delete_relationships(&query).unwrap();

        assert_eq!(deleted, 1);
        assert_eq!(backend.relationship_count(), 0);
    }

    #[test]
    fn test_get_relationship_types() {
        let backend = InMemoryGraphBackend::new();

        let alice = create_test_entity("Alice", EntityType::Person);
        let acme = create_test_entity("Acme", EntityType::Organization);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&acme).unwrap();

        backend
            .store_relationship(&Relationship::new(
                alice.id.clone(),
                acme.id.clone(),
                RelationshipType::WorksAt,
            ))
            .unwrap();
        backend
            .store_relationship(&Relationship::new(
                alice.id.clone(),
                acme.id.clone(),
                RelationshipType::Created,
            ))
            .unwrap();

        let types = backend.get_relationship_types(&alice.id, &acme.id).unwrap();

        assert_eq!(types.len(), 2);
        assert!(types.contains(&RelationshipType::WorksAt));
        assert!(types.contains(&RelationshipType::Created));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Additional Mention Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_get_entities_in_memory() {
        let backend = InMemoryGraphBackend::new();

        let alice = create_test_entity("Alice", EntityType::Person);
        let bob = create_test_entity("Bob", EntityType::Person);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&bob).unwrap();

        let memory_id = MemoryId::new("mem_123");

        backend
            .store_mention(&EntityMention::new(alice.id.clone(), memory_id.clone()))
            .unwrap();
        backend
            .store_mention(&EntityMention::new(bob.id.clone(), memory_id.clone()))
            .unwrap();

        let entities = backend.get_entities_in_memory(&memory_id).unwrap();
        assert_eq!(entities.len(), 2);
    }

    #[test]
    fn test_delete_mentions_for_memory() {
        let backend = InMemoryGraphBackend::new();

        let alice = create_test_entity("Alice", EntityType::Person);
        backend.store_entity(&alice).unwrap();

        let mem1 = MemoryId::new("mem_1");
        let mem2 = MemoryId::new("mem_2");

        backend
            .store_mention(&EntityMention::new(alice.id.clone(), mem1.clone()))
            .unwrap();
        backend
            .store_mention(&EntityMention::new(alice.id.clone(), mem2.clone()))
            .unwrap();

        let deleted = backend.delete_mentions_for_memory(&mem1).unwrap();
        assert_eq!(deleted, 1);

        let remaining = backend.get_mentions_for_entity(&alice.id).unwrap();
        assert_eq!(remaining.len(), 1);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Additional Traversal Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_traverse_with_relationship_filter() {
        let backend = InMemoryGraphBackend::new();

        let alice = create_test_entity("Alice", EntityType::Person);
        let acme = create_test_entity("Acme", EntityType::Organization);
        let bob = create_test_entity("Bob", EntityType::Person);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&acme).unwrap();
        backend.store_entity(&bob).unwrap();

        backend
            .store_relationship(&Relationship::new(
                alice.id.clone(),
                acme.id.clone(),
                RelationshipType::WorksAt,
            ))
            .unwrap();
        backend
            .store_relationship(&Relationship::new(
                alice.id.clone(),
                bob.id.clone(),
                RelationshipType::RelatesTo,
            ))
            .unwrap();

        let result = backend
            .traverse(&alice.id, 2, Some(&[RelationshipType::WorksAt]), None)
            .unwrap();

        assert_eq!(result.entities.len(), 2);
        assert!(result.entities.iter().any(|e| e.name == "Alice"));
        assert!(result.entities.iter().any(|e| e.name == "Acme"));
        assert!(!result.entities.iter().any(|e| e.name == "Bob"));
    }

    #[test]
    fn test_traverse_depth_limit() {
        let backend = InMemoryGraphBackend::new();

        let a = create_test_entity("A", EntityType::Concept);
        let b = create_test_entity("B", EntityType::Concept);
        let c = create_test_entity("C", EntityType::Concept);
        let d = create_test_entity("D", EntityType::Concept);

        backend.store_entity(&a).unwrap();
        backend.store_entity(&b).unwrap();
        backend.store_entity(&c).unwrap();
        backend.store_entity(&d).unwrap();

        // A -> B -> C -> D
        backend
            .store_relationship(&Relationship::new(
                a.id.clone(),
                b.id.clone(),
                RelationshipType::RelatesTo,
            ))
            .unwrap();
        backend
            .store_relationship(&Relationship::new(
                b.id.clone(),
                c.id.clone(),
                RelationshipType::RelatesTo,
            ))
            .unwrap();
        backend
            .store_relationship(&Relationship::new(
                c.id.clone(),
                d.id.clone(),
                RelationshipType::RelatesTo,
            ))
            .unwrap();

        let result = backend.traverse(&a.id, 1, None, None).unwrap();
        assert_eq!(result.entities.len(), 2);

        let result = backend.traverse(&a.id, 2, None, None).unwrap();
        assert_eq!(result.entities.len(), 3);
    }

    #[test]
    fn test_find_path_no_path() {
        let backend = InMemoryGraphBackend::new();

        let alice = create_test_entity("Alice", EntityType::Person);
        let bob = create_test_entity("Bob", EntityType::Person);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&bob).unwrap();

        let path = backend.find_path(&alice.id, &bob.id, 5).unwrap();
        assert!(path.is_none());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Additional Temporal Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_close_entity_valid_time() {
        let backend = InMemoryGraphBackend::new();

        let entity = Entity::new(EntityType::Person, "Alice", Domain::for_user())
            .with_valid_time(ValidTimeRange::from(100));

        backend.store_entity(&entity).unwrap();
        backend.close_entity_valid_time(&entity.id, 200).unwrap();

        let point = BitemporalPoint::new(150, i64::MAX);
        let results = backend
            .query_entities_at(&EntityQuery::new(), &point)
            .unwrap();
        assert_eq!(results.len(), 1);

        let point = BitemporalPoint::new(250, i64::MAX);
        let results = backend
            .query_entities_at(&EntityQuery::new(), &point)
            .unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_close_relationship_valid_time() {
        let backend = InMemoryGraphBackend::new();

        let alice = create_test_entity("Alice", EntityType::Person);
        let acme = create_test_entity("Acme", EntityType::Organization);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&acme).unwrap();

        let rel = Relationship::new(alice.id.clone(), acme.id.clone(), RelationshipType::WorksAt)
            .with_valid_time(ValidTimeRange::from(100));

        backend.store_relationship(&rel).unwrap();
        backend
            .close_relationship_valid_time(&alice.id, &acme.id, RelationshipType::WorksAt, 200)
            .unwrap();

        let point = BitemporalPoint::new(150, i64::MAX);
        let results = backend
            .query_relationships_at(&RelationshipQuery::new(), &point)
            .unwrap();
        assert_eq!(results.len(), 1);

        let point = BitemporalPoint::new(250, i64::MAX);
        let results = backend
            .query_relationships_at(&RelationshipQuery::new(), &point)
            .unwrap();
        assert_eq!(results.len(), 0);
    }
}
