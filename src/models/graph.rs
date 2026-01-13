// Allow non-const functions that use f32::clamp (not const-stable yet)
#![allow(clippy::missing_const_for_fn)]

//! Graph memory types for knowledge graph construction.
//!
//! This module provides types for representing entities extracted from memories
//! and relationships between them, forming a temporal knowledge graph.
//!
//! # Entity Types
//!
//! Entities are categorized into five types:
//!
//! | Type | Description | Examples |
//! |------|-------------|----------|
//! | `Person` | Named individuals | "Alice Johnson", "@username" |
//! | `Organization` | Companies, teams, groups | "Anthropic", "Backend Team" |
//! | `Concept` | Abstract ideas, patterns | "REST API", "Event Sourcing" |
//! | `Technology` | Tools, frameworks, languages | "Rust", "`SQLite`", "Docker" |
//! | `File` | Code files, documents | "src/main.rs", "README.md" |
//!
//! # Relationship Types
//!
//! Relationships between entities include:
//!
//! - `WorksAt` - Person → Organization
//! - `Created` - Entity → Entity (authorship)
//! - `Uses` - Entity → Entity (dependency)
//! - `Implements` - Entity → Entity (realization)
//! - `PartOf` - Entity → Entity (composition)
//! - `RelatesTo` - Entity → Entity (general association)
//! - `MentionedIn` - Entity → Memory (provenance)
//! - `Supersedes` - Entity → Entity (versioning)
//! - `ConflictsWith` - Entity → Entity (contradiction)
//!
//! # Example
//!
//! ```rust
//! use subcog::models::graph::{Entity, EntityType, Relationship, RelationshipType, EntityId};
//! use subcog::models::Domain;
//!
//! // Create an entity for a technology
//! let rust_entity = Entity::new(
//!     EntityType::Technology,
//!     "Rust",
//!     Domain::for_user(),
//! );
//!
//! // Create a relationship
//! let relationship = Relationship::new(
//!     EntityId::new("person_alice"),
//!     EntityId::new("tech_rust"),
//!     RelationshipType::Uses,
//! );
//! ```

use crate::models::temporal::{TransactionTime, ValidTimeRange};
use crate::models::{Domain, MemoryId};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a graph entity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(String);

impl EntityId {
    /// Creates a new entity ID from a string.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generates a new unique entity ID.
    #[must_use]
    pub fn generate() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        // Use a simple hash-like format: ent_<timestamp_hex>_<random>
        let random: u32 = rand_simple();
        Self(format!("ent_{timestamp:x}_{random:08x}"))
    }

    /// Returns the entity ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for EntityId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Simple pseudo-random number generator for ID generation.
/// Uses thread-local state with system time seeding.
#[allow(clippy::cast_possible_truncation)]
fn rand_simple() -> u32 {
    use std::cell::Cell;
    use std::time::{SystemTime, UNIX_EPOCH};

    thread_local! {
        static STATE: Cell<u64> = Cell::new(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                // Truncation is intentional - we only need lower bits for randomness
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(12345)
        );
    }

    STATE.with(|state| {
        // Simple xorshift64 PRNG
        let mut s = state.get();
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        state.set(s);
        // Truncation is intentional - we only need 32 bits for the ID
        s as u32
    })
}

/// Type of entity in the knowledge graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    /// Named individual (people, users, contributors).
    Person,
    /// Company, team, group, or collective.
    Organization,
    /// Abstract idea, pattern, or methodology.
    Concept,
    /// Tool, framework, language, or library.
    Technology,
    /// Code file, document, or artifact.
    File,
}

impl EntityType {
    /// Returns all entity type variants.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Person,
            Self::Organization,
            Self::Concept,
            Self::Technology,
            Self::File,
        ]
    }

    /// Returns the entity type as a string slice.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Organization => "organization",
            Self::Concept => "concept",
            Self::Technology => "technology",
            Self::File => "file",
        }
    }

    /// Parses an entity type from a string.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "person" | "people" | "user" => Some(Self::Person),
            "organization" | "org" | "company" | "team" => Some(Self::Organization),
            "concept" | "idea" | "pattern" => Some(Self::Concept),
            "technology" | "tech" | "tool" | "framework" | "language" => Some(Self::Technology),
            "file" | "document" | "artifact" => Some(Self::File),
            _ => None,
        }
    }
}

impl fmt::Display for EntityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for EntityType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("unknown entity type: {s}"))
    }
}

/// Type of relationship between entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipType {
    /// Person works at an organization.
    WorksAt,
    /// Entity created another entity.
    Created,
    /// Entity uses/depends on another entity.
    Uses,
    /// Entity implements a concept or interface.
    Implements,
    /// Entity is part of another entity.
    PartOf,
    /// General association between entities.
    RelatesTo,
    /// Entity is mentioned in a memory.
    MentionedIn,
    /// Entity supersedes/replaces another entity.
    Supersedes,
    /// Entity conflicts with another entity.
    ConflictsWith,
}

impl RelationshipType {
    /// Returns all relationship type variants.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::WorksAt,
            Self::Created,
            Self::Uses,
            Self::Implements,
            Self::PartOf,
            Self::RelatesTo,
            Self::MentionedIn,
            Self::Supersedes,
            Self::ConflictsWith,
        ]
    }

    /// Returns the relationship type as a string slice.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::WorksAt => "works_at",
            Self::Created => "created",
            Self::Uses => "uses",
            Self::Implements => "implements",
            Self::PartOf => "part_of",
            Self::RelatesTo => "relates_to",
            Self::MentionedIn => "mentioned_in",
            Self::Supersedes => "supersedes",
            Self::ConflictsWith => "conflicts_with",
        }
    }

    /// Returns the inverse relationship type, if defined.
    ///
    /// Some relationships have natural inverses (e.g., `PartOf` ↔ `HasPart`),
    /// while others are symmetric (e.g., `RelatesTo`) or asymmetric without
    /// a defined inverse (e.g., `Created`).
    #[must_use]
    pub const fn inverse(&self) -> Option<Self> {
        match self {
            // Symmetric relationships
            Self::RelatesTo => Some(Self::RelatesTo),
            Self::ConflictsWith => Some(Self::ConflictsWith),
            // Asymmetric relationships without defined inverses
            Self::PartOf
            | Self::WorksAt
            | Self::Created
            | Self::Uses
            | Self::Implements
            | Self::MentionedIn
            | Self::Supersedes => None,
        }
    }

    /// Parses a relationship type from a string.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().replace('-', "_").as_str() {
            "works_at" | "worksat" | "employed_by" => Some(Self::WorksAt),
            "created" | "authored" | "wrote" => Some(Self::Created),
            "uses" | "depends_on" | "requires" => Some(Self::Uses),
            "implements" | "realizes" | "extends" => Some(Self::Implements),
            "part_of" | "partof" | "belongs_to" | "member_of" => Some(Self::PartOf),
            "relates_to" | "relatesto" | "related" | "associated" => Some(Self::RelatesTo),
            "mentioned_in" | "mentionedin" | "referenced_in" => Some(Self::MentionedIn),
            "supersedes" | "replaces" | "upgrades" => Some(Self::Supersedes),
            "conflicts_with" | "conflictswith" | "contradicts" => Some(Self::ConflictsWith),
            _ => None,
        }
    }
}

impl fmt::Display for RelationshipType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for RelationshipType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("unknown relationship type: {s}"))
    }
}

/// An entity in the knowledge graph.
///
/// Entities represent real-world concepts extracted from memories, such as
/// people, organizations, technologies, and files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    /// Unique identifier for this entity.
    pub id: EntityId,
    /// Type of entity.
    pub entity_type: EntityType,
    /// Canonical name for the entity.
    pub name: String,
    /// Alternative names or aliases.
    pub aliases: Vec<String>,
    /// Domain scope for the entity.
    pub domain: Domain,
    /// Confidence score from extraction (0.0 to 1.0).
    pub confidence: f32,
    /// Bitemporal: when this entity was valid in the real world.
    pub valid_time: ValidTimeRange,
    /// Bitemporal: when this entity was recorded in the system.
    pub transaction_time: TransactionTime,
    /// Optional properties as key-value pairs.
    pub properties: std::collections::HashMap<String, String>,
    /// Number of times this entity has been mentioned.
    pub mention_count: u32,
}

impl Entity {
    /// Creates a new entity with default temporal values.
    #[must_use]
    pub fn new(entity_type: EntityType, name: impl Into<String>, domain: Domain) -> Self {
        Self {
            id: EntityId::generate(),
            entity_type,
            name: name.into(),
            aliases: Vec::new(),
            domain,
            confidence: 1.0,
            valid_time: ValidTimeRange::unbounded(),
            transaction_time: TransactionTime::now(),
            properties: std::collections::HashMap::new(),
            mention_count: 1,
        }
    }

    /// Creates an entity with a specific ID.
    #[must_use]
    pub fn with_id(mut self, id: EntityId) -> Self {
        self.id = id;
        self
    }

    /// Sets the confidence score.
    #[must_use]
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Adds an alias to the entity.
    #[must_use]
    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }

    /// Adds multiple aliases to the entity.
    #[must_use]
    pub fn with_aliases(mut self, aliases: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.aliases.extend(aliases.into_iter().map(Into::into));
        self
    }

    /// Sets the valid time range.
    #[must_use]
    pub fn with_valid_time(mut self, valid_time: ValidTimeRange) -> Self {
        self.valid_time = valid_time;
        self
    }

    /// Adds a property to the entity.
    #[must_use]
    pub fn with_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }

    /// Returns true if this entity matches a name (canonical or alias).
    #[must_use]
    pub fn matches_name(&self, name: &str) -> bool {
        let name_lower = name.to_lowercase();
        self.name.to_lowercase() == name_lower
            || self.aliases.iter().any(|a| a.to_lowercase() == name_lower)
    }

    /// Returns true if this entity is valid at the given time.
    #[must_use]
    pub fn is_valid_at(&self, timestamp: i64) -> bool {
        self.valid_time.contains(timestamp)
    }
}

/// A relationship between two entities in the knowledge graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relationship {
    /// Source entity ID.
    pub from_entity: EntityId,
    /// Target entity ID.
    pub to_entity: EntityId,
    /// Type of relationship.
    pub relationship_type: RelationshipType,
    /// Confidence score (0.0 to 1.0).
    pub confidence: f32,
    /// Bitemporal: when this relationship was valid.
    pub valid_time: ValidTimeRange,
    /// Bitemporal: when this relationship was recorded.
    pub transaction_time: TransactionTime,
    /// Optional properties as key-value pairs.
    pub properties: std::collections::HashMap<String, String>,
}

impl Relationship {
    /// Creates a new relationship with default temporal values.
    #[must_use]
    pub fn new(
        from_entity: EntityId,
        to_entity: EntityId,
        relationship_type: RelationshipType,
    ) -> Self {
        Self {
            from_entity,
            to_entity,
            relationship_type,
            confidence: 1.0,
            valid_time: ValidTimeRange::unbounded(),
            transaction_time: TransactionTime::now(),
            properties: std::collections::HashMap::new(),
        }
    }

    /// Sets the confidence score.
    #[must_use]
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Sets the valid time range.
    #[must_use]
    pub fn with_valid_time(mut self, valid_time: ValidTimeRange) -> Self {
        self.valid_time = valid_time;
        self
    }

    /// Adds a property to the relationship.
    #[must_use]
    pub fn with_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }

    /// Returns true if this relationship is valid at the given time.
    #[must_use]
    pub fn is_valid_at(&self, timestamp: i64) -> bool {
        self.valid_time.contains(timestamp)
    }
}

/// A mention of an entity in a memory.
///
/// This links entities to their source memories, providing provenance tracking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityMention {
    /// Entity that was mentioned.
    pub entity_id: EntityId,
    /// Memory where the entity was mentioned.
    pub memory_id: MemoryId,
    /// Confidence score of this specific mention.
    pub confidence: f32,
    /// Character offset where the mention starts (if available).
    pub start_offset: Option<usize>,
    /// Character offset where the mention ends (if available).
    pub end_offset: Option<usize>,
    /// The exact text that was matched.
    pub matched_text: Option<String>,
    /// When this mention was recorded.
    pub transaction_time: TransactionTime,
}

impl EntityMention {
    /// Creates a new entity mention.
    #[must_use]
    pub fn new(entity_id: EntityId, memory_id: MemoryId) -> Self {
        Self {
            entity_id,
            memory_id,
            confidence: 1.0,
            start_offset: None,
            end_offset: None,
            matched_text: None,
            transaction_time: TransactionTime::now(),
        }
    }

    /// Sets the confidence score.
    #[must_use]
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Sets the text span for this mention.
    #[must_use]
    pub fn with_span(mut self, start: usize, end: usize, text: impl Into<String>) -> Self {
        self.start_offset = Some(start);
        self.end_offset = Some(end);
        self.matched_text = Some(text.into());
        self
    }
}

/// Query parameters for searching entities.
#[derive(Debug, Clone, Default)]
pub struct EntityQuery {
    /// Filter by entity type.
    pub entity_type: Option<EntityType>,
    /// Search by name (fuzzy match).
    pub name: Option<String>,
    /// Filter by domain.
    pub domain: Option<Domain>,
    /// Minimum confidence threshold.
    pub min_confidence: Option<f32>,
    /// Point-in-time query for temporal filtering.
    pub valid_at: Option<i64>,
    /// Maximum results to return.
    pub limit: Option<usize>,
    /// Offset for pagination.
    pub offset: Option<usize>,
}

impl EntityQuery {
    /// Creates a new empty query.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entity_type: None,
            name: None,
            domain: None,
            min_confidence: None,
            valid_at: None,
            limit: None,
            offset: None,
        }
    }

    /// Filters by entity type.
    #[must_use]
    pub fn with_type(mut self, entity_type: EntityType) -> Self {
        self.entity_type = Some(entity_type);
        self
    }

    /// Searches by name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Filters by domain.
    #[must_use]
    pub fn with_domain(mut self, domain: Domain) -> Self {
        self.domain = Some(domain);
        self
    }

    /// Sets minimum confidence threshold.
    #[must_use]
    pub fn with_min_confidence(mut self, confidence: f32) -> Self {
        self.min_confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    /// Sets point-in-time for temporal query.
    #[must_use]
    pub const fn valid_at(mut self, timestamp: i64) -> Self {
        self.valid_at = Some(timestamp);
        self
    }

    /// Sets maximum results.
    #[must_use]
    pub const fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets offset for pagination.
    #[must_use]
    pub const fn with_offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }
}

/// Query parameters for traversing relationships.
#[derive(Debug, Clone, Default)]
pub struct RelationshipQuery {
    /// Starting entity for traversal.
    pub from_entity: Option<EntityId>,
    /// Target entity for filtering.
    pub to_entity: Option<EntityId>,
    /// Filter by relationship type.
    pub relationship_type: Option<RelationshipType>,
    /// Maximum traversal depth.
    pub max_depth: Option<u32>,
    /// Minimum confidence threshold.
    pub min_confidence: Option<f32>,
    /// Point-in-time query for temporal filtering.
    pub valid_at: Option<i64>,
    /// Maximum results to return.
    pub limit: Option<usize>,
}

impl RelationshipQuery {
    /// Creates a new empty query.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            from_entity: None,
            to_entity: None,
            relationship_type: None,
            max_depth: None,
            min_confidence: None,
            valid_at: None,
            limit: None,
        }
    }

    /// Sets the starting entity.
    #[must_use]
    pub fn from(mut self, entity_id: EntityId) -> Self {
        self.from_entity = Some(entity_id);
        self
    }

    /// Sets the target entity.
    #[must_use]
    pub fn to(mut self, entity_id: EntityId) -> Self {
        self.to_entity = Some(entity_id);
        self
    }

    /// Filters by relationship type.
    #[must_use]
    pub fn with_type(mut self, relationship_type: RelationshipType) -> Self {
        self.relationship_type = Some(relationship_type);
        self
    }

    /// Sets maximum traversal depth.
    #[must_use]
    pub const fn with_max_depth(mut self, depth: u32) -> Self {
        self.max_depth = Some(depth);
        self
    }

    /// Sets minimum confidence threshold.
    #[must_use]
    pub fn with_min_confidence(mut self, confidence: f32) -> Self {
        self.min_confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    /// Sets point-in-time for temporal query.
    #[must_use]
    pub const fn valid_at(mut self, timestamp: i64) -> Self {
        self.valid_at = Some(timestamp);
        self
    }

    /// Sets maximum results.
    #[must_use]
    pub const fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Result of a graph traversal operation.
#[derive(Debug, Clone, Default)]
pub struct TraversalResult {
    /// Entities found during traversal.
    pub entities: Vec<Entity>,
    /// Relationships traversed.
    pub relationships: Vec<Relationship>,
    /// Total count before limit was applied.
    pub total_count: usize,
}

impl TraversalResult {
    /// Creates a new empty traversal result.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entities: Vec::new(),
            relationships: Vec::new(),
            total_count: 0,
        }
    }

    /// Returns true if the result is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty() && self.relationships.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_id_generate() {
        let id1 = EntityId::generate();
        let id2 = EntityId::generate();
        assert_ne!(id1, id2);
        assert!(id1.as_str().starts_with("ent_"));
    }

    #[test]
    fn test_entity_type_parse() {
        assert_eq!(EntityType::parse("person"), Some(EntityType::Person));
        assert_eq!(EntityType::parse("PERSON"), Some(EntityType::Person));
        assert_eq!(EntityType::parse("org"), Some(EntityType::Organization));
        assert_eq!(EntityType::parse("tech"), Some(EntityType::Technology));
        assert_eq!(EntityType::parse("unknown"), None);
    }

    #[test]
    fn test_relationship_type_parse() {
        assert_eq!(
            RelationshipType::parse("works_at"),
            Some(RelationshipType::WorksAt)
        );
        assert_eq!(
            RelationshipType::parse("uses"),
            Some(RelationshipType::Uses)
        );
        assert_eq!(
            RelationshipType::parse("part-of"),
            Some(RelationshipType::PartOf)
        );
        assert_eq!(RelationshipType::parse("unknown"), None);
    }

    #[test]
    fn test_entity_creation() {
        let entity = Entity::new(EntityType::Technology, "Rust", Domain::for_user())
            .with_confidence(0.95)
            .with_alias("rust-lang")
            .with_property("version", "1.85");

        assert_eq!(entity.entity_type, EntityType::Technology);
        assert_eq!(entity.name, "Rust");
        assert_eq!(entity.confidence, 0.95);
        assert!(entity.aliases.contains(&"rust-lang".to_string()));
        assert_eq!(entity.properties.get("version"), Some(&"1.85".to_string()));
    }

    #[test]
    fn test_entity_matches_name() {
        let entity = Entity::new(EntityType::Person, "Alice Johnson", Domain::for_user())
            .with_alias("alice")
            .with_alias("AJ");

        assert!(entity.matches_name("Alice Johnson"));
        assert!(entity.matches_name("alice johnson"));
        assert!(entity.matches_name("alice"));
        assert!(entity.matches_name("AJ"));
        assert!(!entity.matches_name("Bob"));
    }

    #[test]
    fn test_relationship_creation() {
        let rel = Relationship::new(
            EntityId::new("person_1"),
            EntityId::new("org_1"),
            RelationshipType::WorksAt,
        )
        .with_confidence(0.9)
        .with_property("role", "Engineer");

        assert_eq!(rel.from_entity.as_str(), "person_1");
        assert_eq!(rel.to_entity.as_str(), "org_1");
        assert_eq!(rel.relationship_type, RelationshipType::WorksAt);
        assert_eq!(rel.confidence, 0.9);
    }

    #[test]
    fn test_entity_query_builder() {
        let query = EntityQuery::new()
            .with_type(EntityType::Person)
            .with_name("Alice")
            .with_min_confidence(0.8)
            .with_limit(10);

        assert_eq!(query.entity_type, Some(EntityType::Person));
        assert_eq!(query.name, Some("Alice".to_string()));
        assert_eq!(query.min_confidence, Some(0.8));
        assert_eq!(query.limit, Some(10));
    }

    #[test]
    fn test_relationship_query_builder() {
        let query = RelationshipQuery::new()
            .from(EntityId::new("ent_1"))
            .with_type(RelationshipType::Uses)
            .with_max_depth(2)
            .with_limit(20);

        assert_eq!(query.from_entity, Some(EntityId::new("ent_1")));
        assert_eq!(query.relationship_type, Some(RelationshipType::Uses));
        assert_eq!(query.max_depth, Some(2));
        assert_eq!(query.limit, Some(20));
    }

    #[test]
    fn test_confidence_clamping() {
        let entity =
            Entity::new(EntityType::Concept, "Test", Domain::for_user()).with_confidence(1.5); // Should clamp to 1.0
        assert_eq!(entity.confidence, 1.0);

        let entity2 =
            Entity::new(EntityType::Concept, "Test", Domain::for_user()).with_confidence(-0.5); // Should clamp to 0.0
        assert_eq!(entity2.confidence, 0.0);
    }

    #[test]
    fn test_entity_mention() {
        let mention = EntityMention::new(EntityId::new("ent_1"), MemoryId::new("mem_1"))
            .with_confidence(0.95)
            .with_span(10, 20, "example text");

        assert_eq!(mention.entity_id.as_str(), "ent_1");
        assert_eq!(mention.memory_id.as_str(), "mem_1");
        assert_eq!(mention.confidence, 0.95);
        assert_eq!(mention.start_offset, Some(10));
        assert_eq!(mention.end_offset, Some(20));
        assert_eq!(mention.matched_text, Some("example text".to_string()));
    }

    #[test]
    fn test_traversal_result() {
        let result = TraversalResult::new();
        assert!(result.is_empty());

        let mut result = TraversalResult::new();
        result
            .entities
            .push(Entity::new(EntityType::Person, "Test", Domain::for_user()));
        assert!(!result.is_empty());
    }
}
