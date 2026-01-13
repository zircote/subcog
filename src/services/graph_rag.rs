//! Graph RAG (Retrieval-Augmented Generation) Service.
//!
//! Provides hybrid search that combines traditional semantic/text search
//! with knowledge graph expansion for enhanced memory recall.
//!
//! # Architecture
//!
//! ```text
//! User Query: "How do we handle auth?"
//!     │
//!     ▼
//! GraphRAGService.search_with_expansion()
//!     │
//!     ├──▶ RecallService.search() → 10 memories (semantic)
//!     │
//!     └──▶ EntityExtractorService.extract_from_query("auth")
//!              │
//!              ▼
//!          ["AuthService", "JWT", "OAuth"]
//!              │
//!              ▼
//!          GraphService.traverse(depth=2)
//!              │
//!              ▼
//!          Related entities + their source_memory_ids
//!              │
//!              ▼
//!          5 additional memories via graph
//!     │
//!     ▼
//! Merge + Re-rank (boost graph-based by config.expansion_boost)
//!     │
//!     ▼
//! Return 15 memories with provenance
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use subcog::services::{GraphRAGService, GraphRAGConfig, ExpansionConfig};
//!
//! let service = GraphRAGService::new(recall, graph, config);
//!
//! let results = service.search_with_expansion(
//!     "authentication patterns",
//!     &SearchFilter::new(),
//!     ExpansionConfig::default(),
//! )?;
//!
//! for hit in results.memories {
//!     println!("{}: {} (provenance: {:?})", hit.memory.id, hit.score, hit.provenance);
//! }
//! ```

use crate::Result;
use crate::models::graph::{Entity, EntityId, EntityMention, EntityType};
use crate::models::{Memory, MemoryId, SearchFilter, SearchMode, SearchResult};
use crate::services::{EntityExtractorService, GraphService, RecallService};
use crate::storage::traits::GraphBackend;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for Graph RAG service.
#[derive(Debug, Clone)]
pub struct GraphRAGConfig {
    /// Maximum depth for graph traversal during expansion.
    pub max_depth: usize,
    /// Boost factor for graph-sourced memories (1.0 = no boost).
    pub expansion_boost: f32,
    /// Maximum entities to extract from query.
    pub max_query_entities: usize,
    /// Maximum additional memories to retrieve via graph expansion.
    pub max_expansion_results: usize,
    /// Minimum confidence for entity extraction.
    pub min_entity_confidence: f32,
    /// Whether to include relationship context in results.
    pub include_relationship_context: bool,
}

impl Default for GraphRAGConfig {
    fn default() -> Self {
        Self {
            max_depth: 2,
            expansion_boost: 1.2,
            max_query_entities: 5,
            max_expansion_results: 10,
            min_entity_confidence: 0.5,
            include_relationship_context: true,
        }
    }
}

impl GraphRAGConfig {
    /// Creates a new configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum traversal depth.
    #[must_use]
    pub const fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Sets the expansion boost factor.
    #[must_use]
    pub const fn with_expansion_boost(mut self, boost: f32) -> Self {
        self.expansion_boost = boost;
        self
    }

    /// Sets the maximum query entities.
    #[must_use]
    pub const fn with_max_query_entities(mut self, max: usize) -> Self {
        self.max_query_entities = max;
        self
    }

    /// Sets the maximum expansion results.
    #[must_use]
    pub const fn with_max_expansion_results(mut self, max: usize) -> Self {
        self.max_expansion_results = max;
        self
    }

    /// Loads configuration from environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(val) = std::env::var("SUBCOG_GRAPH_RAG_MAX_DEPTH")
            && let Ok(depth) = val.parse()
        {
            config.max_depth = depth;
        }

        if let Ok(val) = std::env::var("SUBCOG_GRAPH_RAG_EXPANSION_BOOST")
            && let Ok(boost) = val.parse()
        {
            config.expansion_boost = boost;
        }

        if let Ok(val) = std::env::var("SUBCOG_GRAPH_RAG_MAX_QUERY_ENTITIES")
            && let Ok(max) = val.parse()
        {
            config.max_query_entities = max;
        }

        if let Ok(val) = std::env::var("SUBCOG_GRAPH_RAG_MAX_EXPANSION_RESULTS")
            && let Ok(max) = val.parse()
        {
            config.max_expansion_results = max;
        }

        config
    }
}

/// Configuration for a specific expansion operation.
#[derive(Debug, Clone)]
pub struct ExpansionConfig {
    /// Traversal depth for this expansion (overrides default).
    pub depth: Option<usize>,
    /// Entity types to prioritize during expansion.
    pub entity_type_filter: Option<Vec<EntityType>>,
    /// Whether to boost results based on relationship strength.
    pub use_relationship_weight: bool,
}

impl Default for ExpansionConfig {
    fn default() -> Self {
        Self {
            depth: None,
            entity_type_filter: None,
            use_relationship_weight: true,
        }
    }
}

// ============================================================================
// Provenance Tracking
// ============================================================================

/// Indicates how a memory was discovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchProvenance {
    /// Found via traditional semantic/text search.
    Semantic,
    /// Found via graph expansion.
    GraphExpansion {
        /// The entity that linked to this memory.
        source_entity: EntityId,
        /// The relationship path length.
        hop_count: usize,
    },
    /// Found via both semantic search and graph expansion.
    Both {
        /// Semantic search score.
        semantic_score: u32, // Using u32 to represent f32 * 1000 for Eq
        /// Graph expansion details.
        source_entity: EntityId,
    },
}

/// A search result with provenance information.
#[derive(Debug, Clone)]
pub struct GraphSearchHit {
    /// The memory that was found.
    pub memory: Memory,
    /// The relevance score (0.0 to 1.0).
    pub score: f32,
    /// How this memory was discovered.
    pub provenance: SearchProvenance,
    /// Related entities found via graph (if any).
    pub related_entities: Vec<EntityId>,
}

/// Results from a Graph RAG search.
#[derive(Debug)]
pub struct GraphSearchResults {
    /// The search query.
    pub query: String,
    /// All matched memories with provenance.
    pub hits: Vec<GraphSearchHit>,
    /// Total semantic results before merging.
    pub semantic_count: usize,
    /// Total graph expansion results before merging.
    pub graph_count: usize,
    /// Entities extracted from the query.
    pub query_entities: Vec<String>,
}

impl GraphSearchResults {
    /// Returns the total number of hits.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.hits.len()
    }

    /// Returns whether there are no hits.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.hits.is_empty()
    }

    /// Returns hits found via semantic search only.
    #[must_use]
    pub fn semantic_hits(&self) -> Vec<&GraphSearchHit> {
        self.hits
            .iter()
            .filter(|h| matches!(h.provenance, SearchProvenance::Semantic))
            .collect()
    }

    /// Returns hits found via graph expansion only.
    #[must_use]
    pub fn graph_hits(&self) -> Vec<&GraphSearchHit> {
        self.hits
            .iter()
            .filter(|h| matches!(h.provenance, SearchProvenance::GraphExpansion { .. }))
            .collect()
    }

    /// Returns hits found via both methods.
    #[must_use]
    pub fn hybrid_hits(&self) -> Vec<&GraphSearchHit> {
        self.hits
            .iter()
            .filter(|h| matches!(h.provenance, SearchProvenance::Both { .. }))
            .collect()
    }
}

// ============================================================================
// Graph RAG Service
// ============================================================================

/// Service for hybrid search combining semantic search with graph expansion.
///
/// The Graph RAG service enhances traditional memory recall by:
/// 1. Extracting entities from the user's query
/// 2. Expanding the knowledge graph to find related entities
/// 3. Retrieving memories linked to those entities
/// 4. Merging and re-ranking results from both sources
///
/// This provides contextually richer results that leverage the connections
/// between concepts, people, and technologies in the knowledge graph.
pub struct GraphRAGService<G: GraphBackend> {
    /// The recall service for semantic/text search.
    recall: Arc<RecallService>,
    /// The graph service for knowledge graph operations.
    graph: Arc<GraphService<G>>,
    /// The entity extractor for query analysis.
    extractor: EntityExtractorService,
    /// Configuration for the service.
    config: GraphRAGConfig,
}

impl<G: GraphBackend> GraphRAGService<G> {
    /// Creates a new Graph RAG service.
    ///
    /// # Arguments
    ///
    /// * `recall` - The recall service for semantic search.
    /// * `graph` - The graph service for knowledge graph operations.
    /// * `extractor` - The entity extractor for query analysis.
    /// * `config` - Configuration for the service.
    pub const fn new(
        recall: Arc<RecallService>,
        graph: Arc<GraphService<G>>,
        extractor: EntityExtractorService,
        config: GraphRAGConfig,
    ) -> Self {
        Self {
            recall,
            graph,
            extractor,
            config,
        }
    }

    /// Performs a hybrid search with graph expansion.
    ///
    /// This method:
    /// 1. Runs traditional semantic/text search via `RecallService`
    /// 2. Extracts entities from the query
    /// 3. Traverses the knowledge graph to find related entities
    /// 4. Retrieves memories linked to those entities
    /// 5. Merges and re-ranks all results
    ///
    /// # Arguments
    ///
    /// * `query` - The search query.
    /// * `filter` - Search filter to apply.
    /// * `limit` - Maximum number of results to return.
    /// * `expansion` - Optional expansion configuration.
    ///
    /// # Returns
    ///
    /// A [`GraphSearchResults`] containing all matched memories with provenance.
    ///
    /// # Errors
    ///
    /// Returns an error if search or graph operations fail.
    pub fn search_with_expansion(
        &self,
        query: &str,
        filter: &SearchFilter,
        limit: usize,
        expansion: Option<ExpansionConfig>,
    ) -> Result<GraphSearchResults> {
        let expansion = expansion.unwrap_or_default();

        // Step 1: Run semantic search
        let semantic_results = self
            .recall
            .search(query, SearchMode::Hybrid, filter, limit)?;
        let semantic_count = semantic_results.memories.len();

        // Step 2: Extract entities from query
        let extraction = self.extractor.extract(query)?;
        let query_entities: Vec<String> = extraction
            .entities
            .iter()
            .filter(|e| e.confidence >= self.config.min_entity_confidence)
            .take(self.config.max_query_entities)
            .map(|e| e.name.clone())
            .collect();

        // Step 3: Find matching entities in graph and expand
        let graph_memories = self.expand_from_entities(&query_entities, &expansion)?;
        let graph_count = graph_memories.len();

        // Step 4: Merge and re-rank results
        let hits = self.merge_results(semantic_results, graph_memories, &expansion)?;

        // Step 5: Apply final limit
        let hits: Vec<GraphSearchHit> = hits.into_iter().take(limit).collect();

        Ok(GraphSearchResults {
            query: query.to_string(),
            hits,
            semantic_count,
            graph_count,
            query_entities,
        })
    }

    /// Expands the graph from extracted entity names.
    fn expand_from_entities(
        &self,
        entity_names: &[String],
        expansion: &ExpansionConfig,
    ) -> Result<HashMap<MemoryId, (f32, EntityId, usize)>> {
        let mut results: HashMap<MemoryId, (f32, EntityId, usize)> = HashMap::new();
        let depth = expansion.depth.unwrap_or(self.config.max_depth);

        for name in entity_names {
            let entities = self.find_entities_by_name(name)?;
            for entity in entities {
                self.expand_single_entity(&entity, depth, expansion, &mut results)?;
            }
        }

        Ok(results)
    }

    /// Expands a single entity and collects memory links.
    fn expand_single_entity(
        &self,
        entity: &Entity,
        depth: usize,
        expansion: &ExpansionConfig,
        results: &mut HashMap<MemoryId, (f32, EntityId, usize)>,
    ) -> Result<()> {
        let related = self.traverse_entity(&entity.id, depth)?;

        for (related_entity, hop_count) in related {
            let memory_ids = self.get_entity_memory_links(&related_entity)?;
            self.score_and_insert_memories(
                &memory_ids,
                &entity.id,
                hop_count,
                expansion.use_relationship_weight,
                results,
            );
        }
        Ok(())
    }

    /// Calculates scores and inserts memory links into results.
    fn score_and_insert_memories(
        &self,
        memory_ids: &[MemoryId],
        source_entity: &EntityId,
        hop_count: usize,
        use_weight: bool,
        results: &mut HashMap<MemoryId, (f32, EntityId, usize)>,
    ) {
        #[allow(clippy::cast_precision_loss)]
        let base_score = 1.0 / (1.0 + hop_count as f32);
        let score = if use_weight {
            base_score * self.config.expansion_boost
        } else {
            base_score
        };

        for memory_id in memory_ids {
            Self::update_or_insert_memory(results, memory_id, score, source_entity, hop_count);
        }
    }

    /// Updates an existing memory entry or inserts a new one.
    fn update_or_insert_memory(
        results: &mut HashMap<MemoryId, (f32, EntityId, usize)>,
        memory_id: &MemoryId,
        score: f32,
        source_entity: &EntityId,
        hop_count: usize,
    ) {
        results
            .entry(memory_id.clone())
            .and_modify(|(existing_score, _, existing_hops)| {
                if hop_count < *existing_hops {
                    *existing_score = score;
                    *existing_hops = hop_count;
                }
            })
            .or_insert((score, source_entity.clone(), hop_count));
    }

    /// Finds entities by name (case-insensitive search).
    fn find_entities_by_name(&self, name: &str) -> Result<Vec<Entity>> {
        use crate::models::graph::EntityQuery;

        let query = EntityQuery::new().with_name(name).with_limit(10);

        self.graph.query_entities(&query)
    }

    /// Traverses the graph from an entity up to the specified depth.
    fn traverse_entity(
        &self,
        entity_id: &EntityId,
        max_depth: usize,
    ) -> Result<Vec<(EntityId, usize)>> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut result: Vec<(EntityId, usize)> = Vec::new();
        let mut frontier: Vec<(EntityId, usize)> = vec![(entity_id.clone(), 0)];

        while let Some((current_id, depth)) = frontier.pop() {
            if depth > max_depth {
                continue;
            }

            let id_str = current_id.as_ref().to_string();
            if visited.contains(&id_str) {
                continue;
            }
            visited.insert(id_str);

            if depth > 0 {
                result.push((current_id.clone(), depth));
            }

            // Get relationships from this entity
            if depth < max_depth {
                self.add_neighbors_to_frontier(&current_id, depth, &mut frontier)?;
            }
        }

        Ok(result)
    }

    /// Adds neighbors of an entity to the traversal frontier.
    fn add_neighbors_to_frontier(
        &self,
        entity_id: &EntityId,
        current_depth: usize,
        frontier: &mut Vec<(EntityId, usize)>,
    ) -> Result<()> {
        let neighbors = self.graph.get_neighbors(entity_id, 1)?;
        for neighbor in neighbors {
            frontier.push((neighbor.id.clone(), current_depth + 1));
        }
        Ok(())
    }

    /// Gets memory IDs linked to an entity via mentions.
    fn get_entity_memory_links(&self, entity_id: &EntityId) -> Result<Vec<MemoryId>> {
        // Get mentions to find memories that reference this entity
        let mentions: Vec<EntityMention> = self.graph.get_mentions(entity_id)?;

        Ok(mentions.into_iter().map(|m| m.memory_id).collect())
    }

    /// Merges semantic results with graph expansion results.
    fn merge_results(
        &self,
        semantic: SearchResult,
        graph: HashMap<MemoryId, (f32, EntityId, usize)>,
        _expansion: &ExpansionConfig,
    ) -> Result<Vec<GraphSearchHit>> {
        let mut hits: HashMap<String, GraphSearchHit> = HashMap::new();

        // Add semantic results
        for memory_hit in semantic.memories {
            let id = memory_hit.memory.id.as_str().to_string();
            hits.insert(
                id,
                GraphSearchHit {
                    memory: memory_hit.memory,
                    score: memory_hit.score,
                    provenance: SearchProvenance::Semantic,
                    related_entities: Vec::new(),
                },
            );
        }

        // Merge graph results
        for (memory_id, (graph_score, source_entity, hop_count)) in graph {
            let id = memory_id.as_str().to_string();
            self.merge_single_graph_result(
                &mut hits,
                id,
                &memory_id,
                graph_score,
                source_entity,
                hop_count,
            );
        }

        // Sort by score descending
        let mut result: Vec<GraphSearchHit> = hits.into_values().collect();
        result.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(result)
    }

    /// Merges a single graph result into the hits map.
    fn merge_single_graph_result(
        &self,
        hits: &mut HashMap<String, GraphSearchHit>,
        id: String,
        memory_id: &MemoryId,
        graph_score: f32,
        source_entity: EntityId,
        hop_count: usize,
    ) {
        if let Some(existing) = hits.get_mut(&id) {
            // Found in both - upgrade provenance
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let semantic_score_int = (existing.score.abs() * 1000.0) as u32;
            existing.provenance = SearchProvenance::Both {
                semantic_score: semantic_score_int,
                source_entity: source_entity.clone(),
            };
            // Boost score for appearing in both using midpoint
            existing.score =
                f32::midpoint(existing.score, graph_score) * self.config.expansion_boost;
            existing.related_entities.push(source_entity);
            return;
        }

        // Only found via graph - need to fetch the memory
        let Ok(Some(memory)) = self.recall.get_by_id(memory_id) else {
            return;
        };

        hits.insert(
            id,
            GraphSearchHit {
                memory,
                score: graph_score,
                provenance: SearchProvenance::GraphExpansion {
                    source_entity: source_entity.clone(),
                    hop_count,
                },
                related_entities: vec![source_entity],
            },
        );
    }

    /// Performs semantic-only search (no graph expansion).
    ///
    /// This is useful for comparison or when graph expansion is not desired.
    ///
    /// # Errors
    ///
    /// Returns an error if the semantic search operation fails.
    pub fn search_semantic_only(
        &self,
        query: &str,
        filter: &SearchFilter,
        limit: usize,
    ) -> Result<GraphSearchResults> {
        let semantic_results = self
            .recall
            .search(query, SearchMode::Hybrid, filter, limit)?;

        let hits: Vec<GraphSearchHit> = semantic_results
            .memories
            .into_iter()
            .map(|h| GraphSearchHit {
                memory: h.memory,
                score: h.score,
                provenance: SearchProvenance::Semantic,
                related_entities: Vec::new(),
            })
            .collect();

        let count = hits.len();

        Ok(GraphSearchResults {
            query: query.to_string(),
            hits,
            semantic_count: count,
            graph_count: 0,
            query_entities: Vec::new(),
        })
    }

    /// Performs graph-only search (no semantic search).
    ///
    /// Searches by extracting entities from the query and expanding the graph.
    ///
    /// # Errors
    ///
    /// Returns an error if entity extraction or graph expansion fails.
    pub fn search_graph_only(
        &self,
        query: &str,
        limit: usize,
        expansion: Option<ExpansionConfig>,
    ) -> Result<GraphSearchResults> {
        let expansion = expansion.unwrap_or_default();

        // Extract entities from query
        let extraction = self.extractor.extract(query)?;
        let query_entities: Vec<String> = extraction
            .entities
            .iter()
            .filter(|e| e.confidence >= self.config.min_entity_confidence)
            .take(self.config.max_query_entities)
            .map(|e| e.name.clone())
            .collect();

        // Expand from entities
        let graph_memories = self.expand_from_entities(&query_entities, &expansion)?;
        let graph_count = graph_memories.len();

        // Convert to hits
        let mut hits: Vec<GraphSearchHit> = Vec::new();
        for (memory_id, (score, source_entity, hop_count)) in graph_memories {
            if let Ok(Some(memory)) = self.recall.get_by_id(&memory_id) {
                hits.push(GraphSearchHit {
                    memory,
                    score,
                    provenance: SearchProvenance::GraphExpansion {
                        source_entity: source_entity.clone(),
                        hop_count,
                    },
                    related_entities: vec![source_entity],
                });
            }
        }

        // Sort by score and limit
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(limit);

        Ok(GraphSearchResults {
            query: query.to_string(),
            hits,
            semantic_count: 0,
            graph_count,
            query_entities,
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========== Configuration Tests ==========

    #[test]
    fn test_config_defaults() {
        let config = GraphRAGConfig::default();
        assert_eq!(config.max_depth, 2);
        assert!((config.expansion_boost - 1.2).abs() < f32::EPSILON);
        assert_eq!(config.max_query_entities, 5);
        assert_eq!(config.max_expansion_results, 10);
    }

    #[test]
    fn test_config_builder() {
        let config = GraphRAGConfig::new()
            .with_max_depth(3)
            .with_expansion_boost(1.5)
            .with_max_query_entities(10)
            .with_max_expansion_results(20);

        assert_eq!(config.max_depth, 3);
        assert!((config.expansion_boost - 1.5).abs() < f32::EPSILON);
        assert_eq!(config.max_query_entities, 10);
        assert_eq!(config.max_expansion_results, 20);
    }

    #[test]
    fn test_expansion_config_defaults() {
        let config = ExpansionConfig::default();
        assert!(config.depth.is_none());
        assert!(config.entity_type_filter.is_none());
        assert!(config.use_relationship_weight);
    }

    // ========== Provenance Tests ==========

    #[test]
    fn test_provenance_semantic() {
        let provenance = SearchProvenance::Semantic;
        assert!(matches!(provenance, SearchProvenance::Semantic));
    }

    #[test]
    fn test_provenance_graph_expansion() {
        let provenance = SearchProvenance::GraphExpansion {
            source_entity: EntityId::new("e123"),
            hop_count: 2,
        };
        let SearchProvenance::GraphExpansion {
            source_entity,
            hop_count,
        } = provenance
        else {
            unreachable!("Expected GraphExpansion variant");
        };
        assert_eq!(source_entity.as_ref(), "e123");
        assert_eq!(hop_count, 2);
    }

    #[test]
    fn test_provenance_both() {
        let provenance = SearchProvenance::Both {
            semantic_score: 850,
            source_entity: EntityId::new("e456"),
        };
        let SearchProvenance::Both {
            semantic_score,
            source_entity,
        } = provenance
        else {
            unreachable!("Expected Both variant");
        };
        assert_eq!(semantic_score, 850);
        assert_eq!(source_entity.as_ref(), "e456");
    }

    // ========== Results Tests ==========

    #[test]
    fn test_results_empty() {
        let results = GraphSearchResults {
            query: "test".to_string(),
            hits: Vec::new(),
            semantic_count: 0,
            graph_count: 0,
            query_entities: Vec::new(),
        };
        assert!(results.is_empty());
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_results_counts() {
        let results = GraphSearchResults {
            query: "test".to_string(),
            hits: Vec::new(),
            semantic_count: 10,
            graph_count: 5,
            query_entities: vec!["Rust".to_string(), "PostgreSQL".to_string()],
        };
        assert_eq!(results.semantic_count, 10);
        assert_eq!(results.graph_count, 5);
        assert_eq!(results.query_entities.len(), 2);
    }
}
