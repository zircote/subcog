//! Graph storage backends for knowledge graph operations.
//!
//! This module provides implementations of the [`GraphBackend`] trait for
//! storing and querying entities, relationships, and entity mentions.
//!
//! # Available Backends
//!
//! | Backend | Use Case | Features |
//! |---------|----------|----------|
//! | [`SqliteGraphBackend`] | Default; embedded | Recursive CTEs for traversal |
//! | [`InMemoryGraphBackend`] | Testing | Fast, no persistence |
//!
//! # Example
//!
//! ```rust,ignore
//! use subcog::storage::graph::SqliteGraphBackend;
//! use subcog::storage::traits::GraphBackend;
//! use subcog::models::graph::{Entity, EntityType};
//!
//! let backend = SqliteGraphBackend::new("graph.db")?;
//!
//! // Store an entity
//! let entity = Entity::builder()
//!     .name("Alice")
//!     .entity_type(EntityType::Person)
//!     .build();
//! backend.store_entity(&entity)?;
//!
//! // Query entities
//! let people = backend.query_entities(&EntityQuery::new().with_type(EntityType::Person))?;
//! ```

mod memory;
mod sqlite;

pub use memory::InMemoryGraphBackend;
pub use sqlite::SqliteGraphBackend;

// Re-export trait for convenience
pub use crate::storage::traits::graph::{GraphBackend, GraphStats};
