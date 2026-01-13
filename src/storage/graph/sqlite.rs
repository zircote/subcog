//! `SQLite` graph backend for knowledge graph storage.
//!
//! Provides entity, relationship, and entity mention storage using `SQLite`
//! with support for bitemporal queries and graph traversal via recursive CTEs.

// Allow cast_possible_truncation and cast_sign_loss for SQLite i64 to usize/u32 conversions.
// SQLite returns i64, but entity counts and offsets are inherently non-negative and small.
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
// Allow missing_const_for_fn - some methods use f32 operations not yet const-stable.
#![allow(clippy::missing_const_for_fn)]
// Allow redundant_closure_for_method_calls - closures needed due to rusqlite's Error type.
#![allow(clippy::redundant_closure_for_method_calls)]
// Allow cast_lossless - explicit casts are clearer than Into::into for integer conversions.
#![allow(clippy::cast_lossless)]
// Allow cast_possible_wrap - usize to i64 casts for SQLite parameters won't wrap for text offsets.
#![allow(clippy::cast_possible_wrap)]

use crate::models::graph::{
    Entity, EntityId, EntityMention, EntityQuery, EntityType, Relationship, RelationshipQuery,
    RelationshipType, TraversalResult,
};
use crate::models::temporal::{BitemporalPoint, TransactionTime, ValidTimeRange};
use crate::models::{Domain, MemoryId};
use crate::storage::traits::graph::{GraphBackend, GraphStats};
use crate::{Error, Result};
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use tracing::instrument;

/// Helper to acquire mutex lock with poison recovery.
fn acquire_lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::warn!("Graph SQLite mutex was poisoned, recovering");
            metrics::counter!("graph_sqlite_mutex_poison_recovery_total").increment(1);
            poisoned.into_inner()
        },
    }
}

/// `SQLite`-based graph backend.
///
/// # Concurrency Model
///
/// Uses a `Mutex<Connection>` for thread-safe access. WAL mode and `busy_timeout`
/// handle concurrent access gracefully.
///
/// # Schema
///
/// Three tables store the knowledge graph:
/// - `graph_entities`: Entity nodes with temporal metadata
/// - `graph_relationships`: Directed edges between entities
/// - `graph_entity_mentions`: Links between entities and memories
pub struct SqliteGraphBackend {
    /// Connection to the `SQLite` database.
    conn: Mutex<Connection>,
    /// Path to the database (None for in-memory).
    db_path: Option<PathBuf>,
}

impl SqliteGraphBackend {
    /// Creates a new `SQLite` graph backend.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or initialized.
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        let conn = Connection::open(&db_path).map_err(|e| Error::OperationFailed {
            operation: "open_graph_sqlite".to_string(),
            cause: e.to_string(),
        })?;

        let backend = Self {
            conn: Mutex::new(conn),
            db_path: Some(db_path),
        };

        backend.initialize()?;
        Ok(backend)
    }

    /// Creates an in-memory `SQLite` graph backend (useful for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be initialized.
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| Error::OperationFailed {
            operation: "open_graph_sqlite_memory".to_string(),
            cause: e.to_string(),
        })?;

        let backend = Self {
            conn: Mutex::new(conn),
            db_path: None,
        };

        backend.initialize()?;
        Ok(backend)
    }

    /// Returns the database path.
    #[must_use]
    pub fn db_path(&self) -> Option<&Path> {
        self.db_path.as_deref()
    }

    /// Initializes the database schema.
    fn initialize(&self) -> Result<()> {
        let conn = acquire_lock(&self.conn);

        // Enable WAL mode for better concurrent read performance
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "synchronous", "NORMAL");
        let _ = conn.pragma_update(None, "busy_timeout", "5000");
        // Enable foreign keys for referential integrity
        let _ = conn.pragma_update(None, "foreign_keys", "ON");

        // Create entities table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS graph_entities (
                id TEXT PRIMARY KEY,
                entity_type TEXT NOT NULL,
                name TEXT NOT NULL,
                aliases TEXT,
                domain_org TEXT,
                domain_project TEXT,
                domain_repo TEXT,
                confidence REAL NOT NULL DEFAULT 1.0,
                valid_time_start INTEGER,
                valid_time_end INTEGER,
                transaction_time INTEGER NOT NULL,
                properties TEXT,
                mention_count INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "create_graph_entities_table".to_string(),
            cause: e.to_string(),
        })?;

        // Create relationships table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS graph_relationships (
                from_entity_id TEXT NOT NULL,
                to_entity_id TEXT NOT NULL,
                relationship_type TEXT NOT NULL,
                confidence REAL NOT NULL DEFAULT 1.0,
                valid_time_start INTEGER,
                valid_time_end INTEGER,
                transaction_time INTEGER NOT NULL,
                properties TEXT,
                PRIMARY KEY (from_entity_id, to_entity_id, relationship_type),
                FOREIGN KEY (from_entity_id) REFERENCES graph_entities(id) ON DELETE CASCADE,
                FOREIGN KEY (to_entity_id) REFERENCES graph_entities(id) ON DELETE CASCADE
            )",
            [],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "create_graph_relationships_table".to_string(),
            cause: e.to_string(),
        })?;

        // Create entity mentions table (links entities to memories)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS graph_entity_mentions (
                entity_id TEXT NOT NULL,
                memory_id TEXT NOT NULL,
                confidence REAL NOT NULL DEFAULT 1.0,
                start_offset INTEGER,
                end_offset INTEGER,
                matched_text TEXT,
                transaction_time INTEGER NOT NULL,
                PRIMARY KEY (entity_id, memory_id),
                FOREIGN KEY (entity_id) REFERENCES graph_entities(id) ON DELETE CASCADE
            )",
            [],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "create_graph_entity_mentions_table".to_string(),
            cause: e.to_string(),
        })?;

        // Create indexes
        Self::create_indexes(&conn);

        Ok(())
    }

    /// Creates indexes for optimized queries.
    fn create_indexes(conn: &Connection) {
        // Entity indexes
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_graph_entities_type ON graph_entities(entity_type)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_graph_entities_name ON graph_entities(name)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_graph_entities_domain ON graph_entities(domain_org, domain_project, domain_repo)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_graph_entities_confidence ON graph_entities(confidence DESC)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_graph_entities_mention_count ON graph_entities(mention_count DESC)",
            [],
        );

        // Relationship indexes
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_graph_relationships_from ON graph_relationships(from_entity_id)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_graph_relationships_to ON graph_relationships(to_entity_id)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_graph_relationships_type ON graph_relationships(relationship_type)",
            [],
        );

        // Entity mention indexes
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_graph_entity_mentions_entity ON graph_entity_mentions(entity_id)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_graph_entity_mentions_memory ON graph_entity_mentions(memory_id)",
            [],
        );
    }

    /// Parses an entity from a database row.
    fn parse_entity_row(row: &Row<'_>) -> rusqlite::Result<Entity> {
        let id: String = row.get("id")?;
        let entity_type_str: String = row.get("entity_type")?;
        let name: String = row.get("name")?;
        let aliases_json: Option<String> = row.get("aliases")?;
        let domain_org: Option<String> = row.get("domain_org")?;
        let domain_project: Option<String> = row.get("domain_project")?;
        let domain_repo: Option<String> = row.get("domain_repo")?;
        let confidence: f32 = row.get("confidence")?;
        let valid_time_start: Option<i64> = row.get("valid_time_start")?;
        let valid_time_end: Option<i64> = row.get("valid_time_end")?;
        let transaction_time: i64 = row.get("transaction_time")?;
        let properties_json: Option<String> = row.get("properties")?;
        let mention_count: i64 = row.get("mention_count")?;

        let entity_type = EntityType::parse(&entity_type_str).unwrap_or(EntityType::Concept);
        let aliases: Vec<String> = aliases_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let properties: HashMap<String, String> = properties_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let domain = Domain {
            organization: domain_org,
            project: domain_project,
            repository: domain_repo,
        };

        let valid_time = ValidTimeRange {
            start: valid_time_start,
            end: valid_time_end,
        };

        Ok(Entity {
            id: EntityId::new(id),
            entity_type,
            name,
            aliases,
            domain,
            confidence,
            valid_time,
            transaction_time: TransactionTime::at(transaction_time),
            properties,
            mention_count: mention_count as u32,
        })
    }

    /// Parses a relationship from a database row.
    fn parse_relationship_row(row: &Row<'_>) -> rusqlite::Result<Relationship> {
        let from_entity_id: String = row.get("from_entity_id")?;
        let to_entity_id: String = row.get("to_entity_id")?;
        let relationship_type_str: String = row.get("relationship_type")?;
        let confidence: f32 = row.get("confidence")?;
        let valid_time_start: Option<i64> = row.get("valid_time_start")?;
        let valid_time_end: Option<i64> = row.get("valid_time_end")?;
        let _transaction_time: i64 = row.get("transaction_time")?;
        let _properties_json: Option<String> = row.get("properties")?;

        let relationship_type =
            RelationshipType::parse(&relationship_type_str).unwrap_or(RelationshipType::RelatesTo);

        let valid_time = ValidTimeRange {
            start: valid_time_start,
            end: valid_time_end,
        };

        Ok(Relationship::new(
            EntityId::new(from_entity_id),
            EntityId::new(to_entity_id),
            relationship_type,
        )
        .with_confidence(confidence)
        .with_valid_time(valid_time))
    }

    /// Builds WHERE clause conditions for entity queries.
    fn build_entity_where_clause(query: &EntityQuery) -> (String, Vec<Box<dyn rusqlite::ToSql>>) {
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref entity_type) = query.entity_type {
            conditions.push("entity_type = ?".to_string());
            params.push(Box::new(entity_type.as_str().to_string()));
        }

        if let Some(ref name) = query.name {
            conditions.push("name LIKE ?".to_string());
            params.push(Box::new(format!("%{name}%")));
        }

        if let Some(ref domain) = query.domain {
            if let Some(ref org) = domain.organization {
                conditions.push("domain_org = ?".to_string());
                params.push(Box::new(org.clone()));
            }
            if let Some(ref project) = domain.project {
                conditions.push("domain_project = ?".to_string());
                params.push(Box::new(project.clone()));
            }
            if let Some(ref repo) = domain.repository {
                conditions.push("domain_repo = ?".to_string());
                params.push(Box::new(repo.clone()));
            }
        }

        if let Some(min_confidence) = query.min_confidence {
            conditions.push("confidence >= ?".to_string());
            params.push(Box::new(f64::from(min_confidence)));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        (where_clause, params)
    }
}

impl GraphBackend for SqliteGraphBackend {
    // ========================================================================
    // Entity CRUD Operations
    // ========================================================================

    #[instrument(skip(self, entity), fields(entity_id = %entity.id))]
    fn store_entity(&self, entity: &Entity) -> Result<()> {
        let conn = acquire_lock(&self.conn);

        let aliases_json =
            serde_json::to_string(&entity.aliases).unwrap_or_else(|_| "[]".to_string());
        let properties_json =
            serde_json::to_string(&entity.properties).unwrap_or_else(|_| "{}".to_string());

        conn.execute(
            "INSERT INTO graph_entities (
                id, entity_type, name, aliases, domain_org, domain_project, domain_repo,
                confidence, valid_time_start, valid_time_end, transaction_time, properties, mention_count
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
                entity_type = excluded.entity_type,
                name = excluded.name,
                aliases = excluded.aliases,
                domain_org = excluded.domain_org,
                domain_project = excluded.domain_project,
                domain_repo = excluded.domain_repo,
                confidence = excluded.confidence,
                valid_time_start = excluded.valid_time_start,
                valid_time_end = excluded.valid_time_end,
                properties = excluded.properties,
                mention_count = excluded.mention_count",
            params![
                entity.id.as_str(),
                entity.entity_type.as_str(),
                entity.name,
                aliases_json,
                entity.domain.organization,
                entity.domain.project,
                entity.domain.repository,
                f64::from(entity.confidence),
                entity.valid_time.start,
                entity.valid_time.end,
                entity.transaction_time.timestamp(),
                properties_json,
                entity.mention_count,
            ],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "store_entity".to_string(),
            cause: e.to_string(),
        })?;

        metrics::counter!("graph_entities_stored_total").increment(1);
        Ok(())
    }

    #[instrument(skip(self), fields(entity_id = %id))]
    fn get_entity(&self, id: &EntityId) -> Result<Option<Entity>> {
        let conn = acquire_lock(&self.conn);

        let result = conn
            .query_row(
                "SELECT * FROM graph_entities WHERE id = ?1",
                params![id.as_str()],
                Self::parse_entity_row,
            )
            .optional()
            .map_err(|e| Error::OperationFailed {
                operation: "get_entity".to_string(),
                cause: e.to_string(),
            })?;

        Ok(result)
    }

    #[instrument(skip(self, query))]
    fn query_entities(&self, query: &EntityQuery) -> Result<Vec<Entity>> {
        let conn = acquire_lock(&self.conn);

        let (where_clause, params) = Self::build_entity_where_clause(query);
        let limit = query.limit.unwrap_or(100);

        let sql = format!(
            "SELECT * FROM graph_entities {where_clause} ORDER BY mention_count DESC, confidence DESC LIMIT {limit}"
        );

        let mut stmt = conn.prepare(&sql).map_err(|e| Error::OperationFailed {
            operation: "query_entities_prepare".to_string(),
            cause: e.to_string(),
        })?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let entities = stmt
            .query_map(param_refs.as_slice(), Self::parse_entity_row)
            .map_err(|e| Error::OperationFailed {
                operation: "query_entities".to_string(),
                cause: e.to_string(),
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entities)
    }

    #[instrument(skip(self), fields(entity_id = %id))]
    fn delete_entity(&self, id: &EntityId) -> Result<bool> {
        let conn = acquire_lock(&self.conn);

        // Foreign key cascades handle mentions and relationships
        let rows = conn
            .execute(
                "DELETE FROM graph_entities WHERE id = ?1",
                params![id.as_str()],
            )
            .map_err(|e| Error::OperationFailed {
                operation: "delete_entity".to_string(),
                cause: e.to_string(),
            })?;

        if rows > 0 {
            metrics::counter!("graph_entities_deleted_total").increment(1);
        }

        Ok(rows > 0)
    }

    #[instrument(skip(self, entity_ids))]
    fn merge_entities(&self, entity_ids: &[EntityId], canonical_name: &str) -> Result<Entity> {
        if entity_ids.is_empty() {
            return Err(Error::OperationFailed {
                operation: "merge_entities".to_string(),
                cause: "No entity IDs provided".to_string(),
            });
        }

        let conn = acquire_lock(&self.conn);

        // Get the canonical entity (first in list)
        let canonical_id = &entity_ids[0];
        let canonical_entity: Entity = conn
            .query_row(
                "SELECT * FROM graph_entities WHERE id = ?1",
                params![canonical_id.as_str()],
                Self::parse_entity_row,
            )
            .map_err(|e| Error::OperationFailed {
                operation: "merge_entities_get_canonical".to_string(),
                cause: e.to_string(),
            })?;

        // Collect all aliases from merged entities
        let mut all_aliases = canonical_entity.aliases.clone();
        all_aliases.push(canonical_entity.name.clone());

        for other_id in entity_ids.iter().skip(1) {
            if let Ok(other) = conn.query_row(
                "SELECT * FROM graph_entities WHERE id = ?1",
                params![other_id.as_str()],
                Self::parse_entity_row,
            ) {
                all_aliases.push(other.name);
                all_aliases.extend(other.aliases);

                // Re-point relationships from other entity to canonical
                conn.execute(
                    "UPDATE graph_relationships SET from_entity_id = ?1 WHERE from_entity_id = ?2",
                    params![canonical_id.as_str(), other_id.as_str()],
                )
                .ok();
                conn.execute(
                    "UPDATE graph_relationships SET to_entity_id = ?1 WHERE to_entity_id = ?2",
                    params![canonical_id.as_str(), other_id.as_str()],
                )
                .ok();

                // Re-point mentions
                conn.execute(
                    "UPDATE OR IGNORE graph_entity_mentions SET entity_id = ?1 WHERE entity_id = ?2",
                    params![canonical_id.as_str(), other_id.as_str()],
                )
                .ok();

                // Delete the merged entity
                conn.execute(
                    "DELETE FROM graph_entities WHERE id = ?1",
                    params![other_id.as_str()],
                )
                .ok();
            }
        }

        // Remove duplicates from aliases
        all_aliases.sort();
        all_aliases.dedup();
        // Remove canonical name from aliases
        all_aliases.retain(|a| a != canonical_name);

        // Update canonical entity with new name and merged aliases
        let aliases_json = serde_json::to_string(&all_aliases).unwrap_or_else(|_| "[]".to_string());

        conn.execute(
            "UPDATE graph_entities SET name = ?1, aliases = ?2 WHERE id = ?3",
            params![canonical_name, aliases_json, canonical_id.as_str()],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "merge_entities_update".to_string(),
            cause: e.to_string(),
        })?;

        // Return the updated entity
        let merged = Entity::new(
            canonical_entity.entity_type,
            canonical_name,
            canonical_entity.domain.clone(),
        )
        .with_id(canonical_entity.id)
        .with_confidence(canonical_entity.confidence)
        .with_aliases(all_aliases);

        metrics::counter!("graph_entities_merged_total").increment(1);
        Ok(merged)
    }

    #[instrument(skip(self))]
    fn find_entities_by_name(
        &self,
        name: &str,
        entity_type: Option<EntityType>,
        domain: Option<&Domain>,
        limit: usize,
    ) -> Result<Vec<Entity>> {
        let conn = acquire_lock(&self.conn);

        let mut conditions = vec!["(name LIKE ?1 OR aliases LIKE ?1)".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(format!("%{name}%"))];

        if let Some(ref et) = entity_type {
            conditions.push(format!("entity_type = ?{}", params.len() + 1));
            params.push(Box::new(et.as_str().to_string()));
        }

        if let Some(d) = domain {
            if let Some(ref org) = d.organization {
                conditions.push(format!("domain_org = ?{}", params.len() + 1));
                params.push(Box::new(org.clone()));
            }
            if let Some(ref project) = d.project {
                conditions.push(format!("domain_project = ?{}", params.len() + 1));
                params.push(Box::new(project.clone()));
            }
            if let Some(ref repo) = d.repository {
                conditions.push(format!("domain_repo = ?{}", params.len() + 1));
                params.push(Box::new(repo.clone()));
            }
        }

        let sql = format!(
            "SELECT * FROM graph_entities WHERE {} ORDER BY confidence DESC LIMIT {}",
            conditions.join(" AND "),
            limit
        );

        let mut stmt = conn.prepare(&sql).map_err(|e| Error::OperationFailed {
            operation: "find_entities_by_name_prepare".to_string(),
            cause: e.to_string(),
        })?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let entities = stmt
            .query_map(param_refs.as_slice(), Self::parse_entity_row)
            .map_err(|e| Error::OperationFailed {
                operation: "find_entities_by_name".to_string(),
                cause: e.to_string(),
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entities)
    }

    // ========================================================================
    // Relationship CRUD Operations
    // ========================================================================

    #[instrument(skip(self, relationship))]
    fn store_relationship(&self, relationship: &Relationship) -> Result<()> {
        let conn = acquire_lock(&self.conn);

        let properties_json =
            serde_json::to_string(&relationship.properties).unwrap_or_else(|_| "{}".to_string());

        conn.execute(
            "INSERT INTO graph_relationships (
                from_entity_id, to_entity_id, relationship_type, confidence,
                valid_time_start, valid_time_end, transaction_time, properties
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(from_entity_id, to_entity_id, relationship_type) DO UPDATE SET
                confidence = excluded.confidence,
                valid_time_start = excluded.valid_time_start,
                valid_time_end = excluded.valid_time_end,
                properties = excluded.properties",
            params![
                relationship.from_entity.as_str(),
                relationship.to_entity.as_str(),
                relationship.relationship_type.as_str(),
                f64::from(relationship.confidence),
                relationship.valid_time.start,
                relationship.valid_time.end,
                relationship.transaction_time.timestamp(),
                properties_json,
            ],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "store_relationship".to_string(),
            cause: e.to_string(),
        })?;

        metrics::counter!("graph_relationships_stored_total").increment(1);
        Ok(())
    }

    #[instrument(skip(self, query))]
    fn query_relationships(&self, query: &RelationshipQuery) -> Result<Vec<Relationship>> {
        let conn = acquire_lock(&self.conn);

        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref from_entity) = query.from_entity {
            conditions.push(format!("from_entity_id = ?{}", params.len() + 1));
            params.push(Box::new(from_entity.as_str().to_string()));
        }

        if let Some(ref to_entity) = query.to_entity {
            conditions.push(format!("to_entity_id = ?{}", params.len() + 1));
            params.push(Box::new(to_entity.as_str().to_string()));
        }

        if let Some(ref relationship_type) = query.relationship_type {
            conditions.push(format!("relationship_type = ?{}", params.len() + 1));
            params.push(Box::new(relationship_type.as_str().to_string()));
        }

        if let Some(min_confidence) = query.min_confidence {
            conditions.push(format!("confidence >= ?{}", params.len() + 1));
            params.push(Box::new(f64::from(min_confidence)));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let limit = query.limit.unwrap_or(100);
        let sql = format!(
            "SELECT * FROM graph_relationships {where_clause} ORDER BY confidence DESC LIMIT {limit}"
        );

        let mut stmt = conn.prepare(&sql).map_err(|e| Error::OperationFailed {
            operation: "query_relationships_prepare".to_string(),
            cause: e.to_string(),
        })?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let relationships = stmt
            .query_map(param_refs.as_slice(), Self::parse_relationship_row)
            .map_err(|e| Error::OperationFailed {
                operation: "query_relationships".to_string(),
                cause: e.to_string(),
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(relationships)
    }

    #[instrument(skip(self, query))]
    fn delete_relationships(&self, query: &RelationshipQuery) -> Result<usize> {
        let conn = acquire_lock(&self.conn);

        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref from_entity) = query.from_entity {
            conditions.push(format!("from_entity_id = ?{}", params.len() + 1));
            params.push(Box::new(from_entity.as_str().to_string()));
        }

        if let Some(ref to_entity) = query.to_entity {
            conditions.push(format!("to_entity_id = ?{}", params.len() + 1));
            params.push(Box::new(to_entity.as_str().to_string()));
        }

        if let Some(ref relationship_type) = query.relationship_type {
            conditions.push(format!("relationship_type = ?{}", params.len() + 1));
            params.push(Box::new(relationship_type.as_str().to_string()));
        }

        if conditions.is_empty() {
            return Ok(0); // Don't delete everything if no conditions
        }

        let sql = format!(
            "DELETE FROM graph_relationships WHERE {}",
            conditions.join(" AND ")
        );

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let rows =
            conn.execute(&sql, param_refs.as_slice())
                .map_err(|e| Error::OperationFailed {
                    operation: "delete_relationships".to_string(),
                    cause: e.to_string(),
                })?;

        if rows > 0 {
            metrics::counter!("graph_relationships_deleted_total").increment(rows as u64);
        }

        Ok(rows)
    }

    #[instrument(skip(self))]
    fn get_relationship_types(
        &self,
        from_entity: &EntityId,
        to_entity: &EntityId,
    ) -> Result<Vec<RelationshipType>> {
        let conn = acquire_lock(&self.conn);

        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT relationship_type FROM graph_relationships
                 WHERE from_entity_id = ?1 AND to_entity_id = ?2",
            )
            .map_err(|e| Error::OperationFailed {
                operation: "get_relationship_types_prepare".to_string(),
                cause: e.to_string(),
            })?;

        let types = stmt
            .query_map(params![from_entity.as_str(), to_entity.as_str()], |row| {
                let type_str: String = row.get(0)?;
                Ok(RelationshipType::parse(&type_str).unwrap_or(RelationshipType::RelatesTo))
            })
            .map_err(|e| Error::OperationFailed {
                operation: "get_relationship_types".to_string(),
                cause: e.to_string(),
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(types)
    }

    // ========================================================================
    // Entity Mention Operations
    // ========================================================================

    #[instrument(skip(self, mention))]
    fn store_mention(&self, mention: &EntityMention) -> Result<()> {
        let conn = acquire_lock(&self.conn);

        conn.execute(
            "INSERT INTO graph_entity_mentions (entity_id, memory_id, confidence, start_offset, end_offset, matched_text, transaction_time)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(entity_id, memory_id) DO UPDATE SET
                confidence = excluded.confidence,
                start_offset = excluded.start_offset,
                end_offset = excluded.end_offset,
                matched_text = excluded.matched_text",
            params![
                mention.entity_id.as_str(),
                mention.memory_id.as_str(),
                f64::from(mention.confidence),
                mention.start_offset.map(|v| v as i64),
                mention.end_offset.map(|v| v as i64),
                mention.matched_text,
                mention.transaction_time.timestamp(),
            ],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "store_mention".to_string(),
            cause: e.to_string(),
        })?;

        // Increment mention count on the entity
        conn.execute(
            "UPDATE graph_entities SET mention_count = mention_count + 1 WHERE id = ?1",
            params![mention.entity_id.as_str()],
        )
        .ok();

        metrics::counter!("graph_mentions_stored_total").increment(1);
        Ok(())
    }

    #[instrument(skip(self))]
    fn get_mentions_for_entity(&self, entity_id: &EntityId) -> Result<Vec<EntityMention>> {
        let conn = acquire_lock(&self.conn);

        let mut stmt = conn
            .prepare(
                "SELECT entity_id, memory_id, confidence, start_offset, end_offset, matched_text, transaction_time
                 FROM graph_entity_mentions WHERE entity_id = ?1 ORDER BY transaction_time DESC",
            )
            .map_err(|e| Error::OperationFailed {
                operation: "get_mentions_for_entity_prepare".to_string(),
                cause: e.to_string(),
            })?;

        let mentions = stmt
            .query_map(params![entity_id.as_str()], |row| {
                let start_offset: Option<i64> = row.get("start_offset")?;
                let end_offset: Option<i64> = row.get("end_offset")?;
                let tx_time: i64 = row.get("transaction_time")?;

                Ok(EntityMention {
                    entity_id: EntityId::new(row.get::<_, String>("entity_id")?),
                    memory_id: MemoryId::new(row.get::<_, String>("memory_id")?),
                    confidence: row.get("confidence")?,
                    start_offset: start_offset.map(|v| v as usize),
                    end_offset: end_offset.map(|v| v as usize),
                    matched_text: row.get("matched_text")?,
                    transaction_time: TransactionTime::at(tx_time),
                })
            })
            .map_err(|e| Error::OperationFailed {
                operation: "get_mentions_for_entity".to_string(),
                cause: e.to_string(),
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(mentions)
    }

    #[instrument(skip(self))]
    fn get_entities_in_memory(&self, memory_id: &MemoryId) -> Result<Vec<Entity>> {
        let conn = acquire_lock(&self.conn);

        let mut stmt = conn
            .prepare(
                "SELECT e.* FROM graph_entities e
                 INNER JOIN graph_entity_mentions m ON e.id = m.entity_id
                 WHERE m.memory_id = ?1
                 ORDER BY m.confidence DESC",
            )
            .map_err(|e| Error::OperationFailed {
                operation: "get_entities_in_memory_prepare".to_string(),
                cause: e.to_string(),
            })?;

        let entities = stmt
            .query_map(params![memory_id.as_str()], Self::parse_entity_row)
            .map_err(|e| Error::OperationFailed {
                operation: "get_entities_in_memory".to_string(),
                cause: e.to_string(),
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entities)
    }

    #[instrument(skip(self))]
    fn delete_mentions_for_entity(&self, entity_id: &EntityId) -> Result<usize> {
        let conn = acquire_lock(&self.conn);

        let rows = conn
            .execute(
                "DELETE FROM graph_entity_mentions WHERE entity_id = ?1",
                params![entity_id.as_str()],
            )
            .map_err(|e| Error::OperationFailed {
                operation: "delete_mentions_for_entity".to_string(),
                cause: e.to_string(),
            })?;

        // Reset mention count
        conn.execute(
            "UPDATE graph_entities SET mention_count = 0 WHERE id = ?1",
            params![entity_id.as_str()],
        )
        .ok();

        Ok(rows)
    }

    #[instrument(skip(self))]
    fn delete_mentions_for_memory(&self, memory_id: &MemoryId) -> Result<usize> {
        let conn = acquire_lock(&self.conn);

        // First, get affected entity IDs to decrement their mention counts
        let mut stmt = conn
            .prepare("SELECT entity_id FROM graph_entity_mentions WHERE memory_id = ?1")
            .map_err(|e| Error::OperationFailed {
                operation: "delete_mentions_for_memory_prepare".to_string(),
                cause: e.to_string(),
            })?;

        let entity_ids: Vec<String> = stmt
            .query_map(params![memory_id.as_str()], |row| row.get(0))
            .map_err(|e| Error::OperationFailed {
                operation: "delete_mentions_for_memory_get_entities".to_string(),
                cause: e.to_string(),
            })?
            .filter_map(|r| r.ok())
            .collect();

        // Delete the mentions
        let rows = conn
            .execute(
                "DELETE FROM graph_entity_mentions WHERE memory_id = ?1",
                params![memory_id.as_str()],
            )
            .map_err(|e| Error::OperationFailed {
                operation: "delete_mentions_for_memory".to_string(),
                cause: e.to_string(),
            })?;

        // Decrement mention counts
        for entity_id in entity_ids {
            conn.execute(
                "UPDATE graph_entities SET mention_count = MAX(0, mention_count - 1) WHERE id = ?1",
                params![entity_id],
            )
            .ok();
        }

        Ok(rows)
    }

    // ========================================================================
    // Graph Traversal Operations
    // ========================================================================

    #[instrument(skip(self))]
    fn traverse(
        &self,
        start: &EntityId,
        max_depth: u32,
        relationship_types: Option<&[RelationshipType]>,
        min_confidence: Option<f32>,
    ) -> Result<TraversalResult> {
        let conn = acquire_lock(&self.conn);

        // Build relationship type filter
        let type_filter = relationship_types
            .map(|types| {
                let type_strs: Vec<String> =
                    types.iter().map(|t| format!("'{}'", t.as_str())).collect();
                format!("AND r.relationship_type IN ({})", type_strs.join(", "))
            })
            .unwrap_or_default();

        let confidence_filter = min_confidence
            .map(|c| format!("AND r.confidence >= {c}"))
            .unwrap_or_default();

        // Use recursive CTE for graph traversal
        let sql = format!(
            "WITH RECURSIVE reachable(entity_id, depth, path) AS (
                -- Base case: start node
                SELECT ?1, 0, ?1
                UNION ALL
                -- Recursive case: follow relationships
                SELECT r.to_entity_id, reachable.depth + 1, reachable.path || ',' || r.to_entity_id
                FROM reachable
                JOIN graph_relationships r ON r.from_entity_id = reachable.entity_id
                WHERE reachable.depth < ?2
                  AND instr(reachable.path, r.to_entity_id) = 0
                  {type_filter}
                  {confidence_filter}
            )
            SELECT DISTINCT e.*, reachable.depth
            FROM reachable
            JOIN graph_entities e ON e.id = reachable.entity_id
            ORDER BY reachable.depth, e.mention_count DESC"
        );

        let mut stmt = conn.prepare(&sql).map_err(|e| Error::OperationFailed {
            operation: "traverse_prepare".to_string(),
            cause: e.to_string(),
        })?;

        let entities: Vec<Entity> = stmt
            .query_map(params![start.as_str(), max_depth], |row| {
                Self::parse_entity_row(row)
            })
            .map_err(|e| Error::OperationFailed {
                operation: "traverse".to_string(),
                cause: e.to_string(),
            })?
            .filter_map(|r| r.ok())
            .collect();

        let total_count = entities.len();

        // Get relationships between the found entities
        let entity_ids: Vec<String> = entities.iter().map(|e| e.id.as_str().to_string()).collect();
        let relationships = if entity_ids.is_empty() {
            Vec::new()
        } else {
            // Create separate placeholder indices for each IN clause
            let from_placeholders: Vec<String> =
                (1..=entity_ids.len()).map(|i| format!("?{i}")).collect();
            let to_placeholders: Vec<String> = (entity_ids.len() + 1..=entity_ids.len() * 2)
                .map(|i| format!("?{i}"))
                .collect();

            let from_clause = from_placeholders.join(", ");
            let to_clause = to_placeholders.join(", ");

            let rel_sql = format!(
                "SELECT * FROM graph_relationships
                 WHERE from_entity_id IN ({from_clause}) AND to_entity_id IN ({to_clause})"
            );

            let mut rel_stmt = conn.prepare(&rel_sql).map_err(|e| Error::OperationFailed {
                operation: "traverse_relationships_prepare".to_string(),
                cause: e.to_string(),
            })?;

            // Double the params for both IN clauses
            let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::new();
            for id in &entity_ids {
                params_vec.push(id);
            }
            for id in &entity_ids {
                params_vec.push(id);
            }

            rel_stmt
                .query_map(params_vec.as_slice(), Self::parse_relationship_row)
                .map_err(|e| Error::OperationFailed {
                    operation: "traverse_relationships".to_string(),
                    cause: e.to_string(),
                })?
                .filter_map(|r| r.ok())
                .collect()
        };

        Ok(TraversalResult {
            entities,
            relationships,
            total_count,
        })
    }

    #[instrument(skip(self))]
    fn find_path(
        &self,
        from: &EntityId,
        to: &EntityId,
        max_depth: u32,
    ) -> Result<Option<TraversalResult>> {
        let conn = acquire_lock(&self.conn);

        // Use recursive CTE to find shortest path
        let sql = "WITH RECURSIVE path_finder(entity_id, depth, path) AS (
                SELECT ?1, 0, ?1
                UNION ALL
                SELECT r.to_entity_id, path_finder.depth + 1, path_finder.path || ',' || r.to_entity_id
                FROM path_finder
                JOIN graph_relationships r ON r.from_entity_id = path_finder.entity_id
                WHERE path_finder.depth < ?3
                  AND instr(path_finder.path, r.to_entity_id) = 0
            )
            SELECT path FROM path_finder WHERE entity_id = ?2 ORDER BY depth LIMIT 1";

        let path: Option<String> = conn
            .query_row(sql, params![from.as_str(), to.as_str(), max_depth], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|e| Error::OperationFailed {
                operation: "find_path".to_string(),
                cause: e.to_string(),
            })?;

        if let Some(path_str) = path {
            let entity_ids: Vec<&str> = path_str.split(',').collect();

            // Get entities along the path
            let placeholders: Vec<String> =
                (1..=entity_ids.len()).map(|i| format!("?{i}")).collect();
            let in_clause = placeholders.join(", ");

            let entity_sql = format!("SELECT * FROM graph_entities WHERE id IN ({in_clause})");
            let mut stmt = conn
                .prepare(&entity_sql)
                .map_err(|e| Error::OperationFailed {
                    operation: "find_path_entities_prepare".to_string(),
                    cause: e.to_string(),
                })?;

            let params: Vec<&dyn rusqlite::ToSql> = entity_ids
                .iter()
                .map(|s| s as &dyn rusqlite::ToSql)
                .collect();

            let entities: Vec<Entity> = stmt
                .query_map(params.as_slice(), Self::parse_entity_row)
                .map_err(|e| Error::OperationFailed {
                    operation: "find_path_entities".to_string(),
                    cause: e.to_string(),
                })?
                .filter_map(|r| r.ok())
                .collect();

            let total_count = entities.len();

            // Get relationships along the path
            let mut relationships = Vec::new();
            for window in entity_ids.windows(2) {
                if let [from_id, to_id] = window {
                    let rel: Option<Relationship> = conn
                        .query_row(
                            "SELECT * FROM graph_relationships WHERE from_entity_id = ?1 AND to_entity_id = ?2 LIMIT 1",
                            params![from_id, to_id],
                            Self::parse_relationship_row,
                        )
                        .optional()
                        .ok()
                        .flatten();

                    if let Some(r) = rel {
                        relationships.push(r);
                    }
                }
            }

            Ok(Some(TraversalResult {
                entities,
                relationships,
                total_count,
            }))
        } else {
            Ok(None)
        }
    }

    // ========================================================================
    // Temporal Query Operations
    // ========================================================================

    #[instrument(skip(self, query, point))]
    fn query_entities_at(
        &self,
        query: &EntityQuery,
        point: &BitemporalPoint,
    ) -> Result<Vec<Entity>> {
        let conn = acquire_lock(&self.conn);

        let (base_where, mut params) = Self::build_entity_where_clause(query);

        // Add temporal conditions
        let temporal_conditions = format!(
            "AND (valid_time_start IS NULL OR valid_time_start <= ?{}) \
             AND (valid_time_end IS NULL OR valid_time_end > ?{}) \
             AND transaction_time <= ?{}",
            params.len() + 1,
            params.len() + 2,
            params.len() + 3
        );

        params.push(Box::new(point.valid_at));
        params.push(Box::new(point.valid_at));
        params.push(Box::new(point.as_of));

        let where_clause = if base_where.is_empty() {
            format!("WHERE 1=1 {temporal_conditions}")
        } else {
            format!("{base_where} {temporal_conditions}")
        };

        let limit = query.limit.unwrap_or(100);
        let sql = format!(
            "SELECT * FROM graph_entities {where_clause} ORDER BY mention_count DESC LIMIT {limit}"
        );

        let mut stmt = conn.prepare(&sql).map_err(|e| Error::OperationFailed {
            operation: "query_entities_at_prepare".to_string(),
            cause: e.to_string(),
        })?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let entities = stmt
            .query_map(param_refs.as_slice(), Self::parse_entity_row)
            .map_err(|e| Error::OperationFailed {
                operation: "query_entities_at".to_string(),
                cause: e.to_string(),
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entities)
    }

    #[instrument(skip(self, query, point))]
    fn query_relationships_at(
        &self,
        query: &RelationshipQuery,
        point: &BitemporalPoint,
    ) -> Result<Vec<Relationship>> {
        let conn = acquire_lock(&self.conn);

        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref from_entity) = query.from_entity {
            conditions.push(format!("from_entity_id = ?{}", params.len() + 1));
            params.push(Box::new(from_entity.as_str().to_string()));
        }

        if let Some(ref to_entity) = query.to_entity {
            conditions.push(format!("to_entity_id = ?{}", params.len() + 1));
            params.push(Box::new(to_entity.as_str().to_string()));
        }

        if let Some(ref relationship_type) = query.relationship_type {
            conditions.push(format!("relationship_type = ?{}", params.len() + 1));
            params.push(Box::new(relationship_type.as_str().to_string()));
        }

        // Add temporal conditions
        conditions.push(format!(
            "(valid_time_start IS NULL OR valid_time_start <= ?{})",
            params.len() + 1
        ));
        params.push(Box::new(point.valid_at));

        conditions.push(format!(
            "(valid_time_end IS NULL OR valid_time_end > ?{})",
            params.len() + 1
        ));
        params.push(Box::new(point.valid_at));

        conditions.push(format!("transaction_time <= ?{}", params.len() + 1));
        params.push(Box::new(point.as_of));

        let where_clause = format!("WHERE {}", conditions.join(" AND "));
        let limit = query.limit.unwrap_or(100);

        let sql = format!(
            "SELECT * FROM graph_relationships {where_clause} ORDER BY confidence DESC LIMIT {limit}"
        );

        let mut stmt = conn.prepare(&sql).map_err(|e| Error::OperationFailed {
            operation: "query_relationships_at_prepare".to_string(),
            cause: e.to_string(),
        })?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let relationships = stmt
            .query_map(param_refs.as_slice(), Self::parse_relationship_row)
            .map_err(|e| Error::OperationFailed {
                operation: "query_relationships_at".to_string(),
                cause: e.to_string(),
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(relationships)
    }

    #[instrument(skip(self))]
    fn close_entity_valid_time(&self, id: &EntityId, end_time: i64) -> Result<()> {
        let conn = acquire_lock(&self.conn);

        let rows = conn
            .execute(
                "UPDATE graph_entities SET valid_time_end = ?1 WHERE id = ?2",
                params![end_time, id.as_str()],
            )
            .map_err(|e| Error::OperationFailed {
                operation: "close_entity_valid_time".to_string(),
                cause: e.to_string(),
            })?;

        if rows == 0 {
            return Err(Error::OperationFailed {
                operation: "close_entity_valid_time".to_string(),
                cause: format!("Entity not found: {}", id.as_str()),
            });
        }

        Ok(())
    }

    #[instrument(skip(self))]
    fn close_relationship_valid_time(
        &self,
        from_entity: &EntityId,
        to_entity: &EntityId,
        relationship_type: RelationshipType,
        end_time: i64,
    ) -> Result<()> {
        let conn = acquire_lock(&self.conn);

        let rows = conn
            .execute(
                "UPDATE graph_relationships SET valid_time_end = ?1
                 WHERE from_entity_id = ?2 AND to_entity_id = ?3 AND relationship_type = ?4",
                params![
                    end_time,
                    from_entity.as_str(),
                    to_entity.as_str(),
                    relationship_type.as_str()
                ],
            )
            .map_err(|e| Error::OperationFailed {
                operation: "close_relationship_valid_time".to_string(),
                cause: e.to_string(),
            })?;

        if rows == 0 {
            return Err(Error::OperationFailed {
                operation: "close_relationship_valid_time".to_string(),
                cause: "Relationship not found".to_string(),
            });
        }

        Ok(())
    }

    // ========================================================================
    // Utility Operations
    // ========================================================================

    #[instrument(skip(self))]
    fn get_stats(&self) -> Result<GraphStats> {
        let conn = acquire_lock(&self.conn);

        let entity_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM graph_entities", [], |row| row.get(0))
            .unwrap_or(0);

        let relationship_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM graph_relationships", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);

        let mention_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM graph_entity_mentions", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);

        // Get counts by entity type
        let mut entities_by_type = HashMap::new();
        let mut stmt = conn
            .prepare("SELECT entity_type, COUNT(*) FROM graph_entities GROUP BY entity_type")
            .map_err(|e| Error::OperationFailed {
                operation: "get_stats_entities_by_type".to_string(),
                cause: e.to_string(),
            })?;

        let type_counts = stmt
            .query_map([], |row| {
                let type_str: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok((type_str, count))
            })
            .map_err(|e| Error::OperationFailed {
                operation: "get_stats_entities_by_type_query".to_string(),
                cause: e.to_string(),
            })?;

        for result in type_counts.flatten() {
            if let Some(entity_type) = EntityType::parse(&result.0) {
                entities_by_type.insert(entity_type, result.1 as usize);
            }
        }

        // Get counts by relationship type
        let mut relationships_by_type = HashMap::new();
        let mut stmt = conn
            .prepare(
                "SELECT relationship_type, COUNT(*) FROM graph_relationships GROUP BY relationship_type",
            )
            .map_err(|e| Error::OperationFailed {
                operation: "get_stats_relationships_by_type".to_string(),
                cause: e.to_string(),
            })?;

        let rel_counts = stmt
            .query_map([], |row| {
                let type_str: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok((type_str, count))
            })
            .map_err(|e| Error::OperationFailed {
                operation: "get_stats_relationships_by_type_query".to_string(),
                cause: e.to_string(),
            })?;

        for result in rel_counts.flatten() {
            if let Some(rel_type) = RelationshipType::parse(&result.0) {
                relationships_by_type.insert(rel_type, result.1 as usize);
            }
        }

        let avg_relationships_per_entity = if entity_count > 0 {
            relationship_count as f32 / entity_count as f32
        } else {
            0.0
        };

        Ok(GraphStats {
            entity_count: entity_count as usize,
            entities_by_type,
            relationship_count: relationship_count as usize,
            relationships_by_type,
            mention_count: mention_count as usize,
            avg_relationships_per_entity,
        })
    }

    #[instrument(skip(self))]
    fn clear(&self) -> Result<()> {
        let conn = acquire_lock(&self.conn);

        conn.execute("DELETE FROM graph_entity_mentions", [])
            .map_err(|e| Error::OperationFailed {
                operation: "clear_mentions".to_string(),
                cause: e.to_string(),
            })?;

        conn.execute("DELETE FROM graph_relationships", [])
            .map_err(|e| Error::OperationFailed {
                operation: "clear_relationships".to_string(),
                cause: e.to_string(),
            })?;

        conn.execute("DELETE FROM graph_entities", [])
            .map_err(|e| Error::OperationFailed {
                operation: "clear_entities".to_string(),
                cause: e.to_string(),
            })?;

        metrics::counter!("graph_cleared_total").increment(1);
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;

    fn create_test_entity(name: &str, entity_type: EntityType) -> Entity {
        Entity::new(entity_type, name, Domain::for_user())
    }

    #[test]
    fn test_store_and_get_entity() {
        let backend = SqliteGraphBackend::in_memory().unwrap();
        let entity = create_test_entity("Alice", EntityType::Person);

        backend.store_entity(&entity).unwrap();

        let retrieved = backend.get_entity(&entity.id).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.name, "Alice");
        assert_eq!(retrieved.entity_type, EntityType::Person);
    }

    #[test]
    fn test_query_entities_by_type() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        backend
            .store_entity(&create_test_entity("Alice", EntityType::Person))
            .unwrap();
        backend
            .store_entity(&create_test_entity("Bob", EntityType::Person))
            .unwrap();
        backend
            .store_entity(&create_test_entity("Acme Inc", EntityType::Organization))
            .unwrap();

        let query = EntityQuery::new().with_type(EntityType::Person);
        let results = backend.query_entities(&query).unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|e| e.entity_type == EntityType::Person));
    }

    #[test]
    fn test_delete_entity() {
        let backend = SqliteGraphBackend::in_memory().unwrap();
        let entity = create_test_entity("Alice", EntityType::Person);
        let entity_id = entity.id.clone();

        backend.store_entity(&entity).unwrap();
        assert!(backend.get_entity(&entity_id).unwrap().is_some());

        let deleted = backend.delete_entity(&entity_id).unwrap();
        assert!(deleted);

        assert!(backend.get_entity(&entity_id).unwrap().is_none());
    }

    #[test]
    fn test_store_and_query_relationships() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        let alice = create_test_entity("Alice", EntityType::Person);
        let acme = create_test_entity("Acme Inc", EntityType::Organization);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&acme).unwrap();

        let relationship =
            Relationship::new(alice.id.clone(), acme.id.clone(), RelationshipType::WorksAt);

        backend.store_relationship(&relationship).unwrap();

        let query = RelationshipQuery::new().from(alice.id.clone());
        let results = backend.query_relationships(&query).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].relationship_type, RelationshipType::WorksAt);
    }

    #[test]
    fn test_entity_mentions() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        let entity = create_test_entity("Alice", EntityType::Person);
        backend.store_entity(&entity).unwrap();

        let mention = EntityMention::new(entity.id.clone(), MemoryId::new("mem_123"))
            .with_confidence(0.9)
            .with_span(0, 5, "Alice");

        backend.store_mention(&mention).unwrap();

        let mentions = backend.get_mentions_for_entity(&entity.id).unwrap();
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].memory_id.as_str(), "mem_123");

        // Check mention count was incremented
        let updated_entity = backend.get_entity(&entity.id).unwrap().unwrap();
        assert_eq!(updated_entity.mention_count, 2); // 1 from new() + 1 from store_mention
    }

    #[test]
    fn test_graph_traversal() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        // Create a small graph: Alice -> Acme -> Project
        let alice = create_test_entity("Alice", EntityType::Person);
        let acme = create_test_entity("Acme Inc", EntityType::Organization);
        let project = create_test_entity("Secret Project", EntityType::Concept);

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

        // Traverse from Alice with depth 2
        let result = backend.traverse(&alice.id, 2, None, None).unwrap();

        // Should find all three entities
        assert_eq!(result.entities.len(), 3);
        assert!(result.entities.iter().any(|e| e.name == "Alice"));
        assert!(result.entities.iter().any(|e| e.name == "Acme Inc"));
        assert!(result.entities.iter().any(|e| e.name == "Secret Project"));
    }

    #[test]
    fn test_find_path() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        let alice = create_test_entity("Alice", EntityType::Person);
        let bob = create_test_entity("Bob", EntityType::Person);
        let acme = create_test_entity("Acme Inc", EntityType::Organization);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&bob).unwrap();
        backend.store_entity(&acme).unwrap();

        // Alice -> Acme -> Bob (via relationships)
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
        let path = path.unwrap();
        assert_eq!(path.entities.len(), 3);
    }

    #[test]
    fn test_temporal_queries() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        // Create entity with specific valid time
        let entity = Entity::new(EntityType::Person, "Alice", Domain::for_user())
            .with_valid_time(ValidTimeRange::between(100, 200));

        backend.store_entity(&entity).unwrap();

        // Query at point where entity is valid
        let point = BitemporalPoint::new(150, i64::MAX);
        let query = EntityQuery::new();
        let results = backend.query_entities_at(&query, &point).unwrap();
        assert_eq!(results.len(), 1);

        // Query at point before entity was valid
        let point_before = BitemporalPoint::new(50, i64::MAX);
        let results_before = backend.query_entities_at(&query, &point_before).unwrap();
        assert_eq!(results_before.len(), 0);

        // Query at point after entity validity ended
        let point_after = BitemporalPoint::new(250, i64::MAX);
        let results_after = backend.query_entities_at(&query, &point_after).unwrap();
        assert_eq!(results_after.len(), 0);
    }

    #[test]
    fn test_merge_entities() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        let alice1 = create_test_entity("Alice Smith", EntityType::Person);
        let alice2 = create_test_entity("A. Smith", EntityType::Person);

        backend.store_entity(&alice1).unwrap();
        backend.store_entity(&alice2).unwrap();

        let merged = backend
            .merge_entities(&[alice1.id.clone(), alice2.id.clone()], "Alice Smith")
            .unwrap();

        assert_eq!(merged.name, "Alice Smith");
        assert!(merged.aliases.contains(&"A. Smith".to_string()));

        // Original ID should still exist
        assert!(backend.get_entity(&alice1.id).unwrap().is_some());
        // Merged ID should be deleted
        assert!(backend.get_entity(&alice2.id).unwrap().is_none());
    }

    #[test]
    fn test_get_stats() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        backend
            .store_entity(&create_test_entity("Alice", EntityType::Person))
            .unwrap();
        backend
            .store_entity(&create_test_entity("Bob", EntityType::Person))
            .unwrap();
        backend
            .store_entity(&create_test_entity("Acme", EntityType::Organization))
            .unwrap();

        let stats = backend.get_stats().unwrap();

        assert_eq!(stats.entity_count, 3);
        assert_eq!(stats.entities_by_type.get(&EntityType::Person), Some(&2));
        assert_eq!(
            stats.entities_by_type.get(&EntityType::Organization),
            Some(&1)
        );
    }

    #[test]
    fn test_clear() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        backend
            .store_entity(&create_test_entity("Alice", EntityType::Person))
            .unwrap();
        backend
            .store_entity(&create_test_entity("Bob", EntityType::Person))
            .unwrap();

        backend.clear().unwrap();

        let stats = backend.get_stats().unwrap();
        assert_eq!(stats.entity_count, 0);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Additional Entity CRUD Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_entity_upsert() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        let mut entity = create_test_entity("Alice", EntityType::Person);
        backend.store_entity(&entity).unwrap();

        // Update the same entity
        entity.confidence = 0.95;
        backend.store_entity(&entity).unwrap();

        let retrieved = backend.get_entity(&entity.id).unwrap().unwrap();
        assert!((retrieved.confidence - 0.95).abs() < f32::EPSILON);

        // Should still be only one entity
        let stats = backend.get_stats().unwrap();
        assert_eq!(stats.entity_count, 1);
    }

    #[test]
    fn test_entity_aliases() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        let entity = Entity::new(EntityType::Person, "Robert", Domain::for_user())
            .with_aliases(vec!["Bob".to_string(), "Rob".to_string()]);

        backend.store_entity(&entity).unwrap();

        // Should find by alias
        let results = backend
            .find_entities_by_name("Bob", Some(EntityType::Person), None, 10)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Robert");
    }

    #[test]
    fn test_find_entities_case_insensitive() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        backend
            .store_entity(&create_test_entity("Alice", EntityType::Person))
            .unwrap();

        // Should find with different case
        let results = backend
            .find_entities_by_name("alice", Some(EntityType::Person), None, 10)
            .unwrap();
        assert_eq!(results.len(), 1);

        let results_upper = backend
            .find_entities_by_name("ALICE", Some(EntityType::Person), None, 10)
            .unwrap();
        assert_eq!(results_upper.len(), 1);
    }

    #[test]
    fn test_query_entities_with_offset() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        // Create multiple entities
        for i in 0..5 {
            backend
                .store_entity(&create_test_entity(
                    &format!("Person{i}"),
                    EntityType::Person,
                ))
                .unwrap();
        }

        let query = EntityQuery::new()
            .with_type(EntityType::Person)
            .with_limit(2)
            .with_offset(2);

        let results = backend.query_entities(&query).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_query_entities_by_domain() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        let domain1 = Domain {
            organization: Some("org1".to_string()),
            project: None,
            repository: None,
        };
        let domain2 = Domain {
            organization: Some("org2".to_string()),
            project: None,
            repository: None,
        };

        let entity1 = Entity::new(EntityType::Person, "Alice", domain1.clone());
        let entity2 = Entity::new(EntityType::Person, "Bob", domain2);

        backend.store_entity(&entity1).unwrap();
        backend.store_entity(&entity2).unwrap();

        let query = EntityQuery::new().with_domain(domain1);
        let results = backend.query_entities(&query).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Alice");
    }

    #[test]
    fn test_query_entities_by_confidence() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        let high_conf =
            Entity::new(EntityType::Person, "Alice", Domain::for_user()).with_confidence(0.9);
        let low_conf =
            Entity::new(EntityType::Person, "Bob", Domain::for_user()).with_confidence(0.3);

        backend.store_entity(&high_conf).unwrap();
        backend.store_entity(&low_conf).unwrap();

        let query = EntityQuery::new().with_min_confidence(0.5);
        let results = backend.query_entities(&query).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Alice");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Additional Relationship Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_relationship_upsert() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        let alice = create_test_entity("Alice", EntityType::Person);
        let acme = create_test_entity("Acme", EntityType::Organization);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&acme).unwrap();

        let mut rel =
            Relationship::new(alice.id.clone(), acme.id.clone(), RelationshipType::WorksAt);
        backend.store_relationship(&rel).unwrap();

        // Update with higher confidence
        rel.confidence = 0.95;
        backend.store_relationship(&rel).unwrap();

        let query = RelationshipQuery::new().from(alice.id.clone());
        let results = backend.query_relationships(&query).unwrap();

        assert_eq!(results.len(), 1);
        assert!((results[0].confidence - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn test_query_relationships_by_type() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

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
        let backend = SqliteGraphBackend::in_memory().unwrap();

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

        let remaining = backend.query_relationships(&query).unwrap();
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_get_relationship_types() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        let alice = create_test_entity("Alice", EntityType::Person);
        let acme = create_test_entity("Acme", EntityType::Organization);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&acme).unwrap();

        // Multiple relationship types between same entities
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
        let backend = SqliteGraphBackend::in_memory().unwrap();

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
        let backend = SqliteGraphBackend::in_memory().unwrap();

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
        assert_eq!(remaining[0].memory_id, mem2);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Additional Traversal Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_traverse_with_relationship_filter() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

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

        // Only follow WorksAt relationships
        let result = backend
            .traverse(&alice.id, 2, Some(&[RelationshipType::WorksAt]), None)
            .unwrap();

        assert_eq!(result.entities.len(), 2);
        assert!(result.entities.iter().any(|e| e.name == "Alice"));
        assert!(result.entities.iter().any(|e| e.name == "Acme"));
        assert!(!result.entities.iter().any(|e| e.name == "Bob"));
    }

    #[test]
    fn test_traverse_with_confidence_filter() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        let alice = create_test_entity("Alice", EntityType::Person);
        let bob = create_test_entity("Bob", EntityType::Person);
        let charlie = create_test_entity("Charlie", EntityType::Person);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&bob).unwrap();
        backend.store_entity(&charlie).unwrap();

        backend
            .store_relationship(
                &Relationship::new(
                    alice.id.clone(),
                    bob.id.clone(),
                    RelationshipType::RelatesTo,
                )
                .with_confidence(0.9),
            )
            .unwrap();
        backend
            .store_relationship(
                &Relationship::new(
                    alice.id.clone(),
                    charlie.id.clone(),
                    RelationshipType::RelatesTo,
                )
                .with_confidence(0.3),
            )
            .unwrap();

        // Only follow high-confidence relationships
        let result = backend.traverse(&alice.id, 2, None, Some(0.5)).unwrap();

        assert_eq!(result.entities.len(), 2);
        assert!(result.entities.iter().any(|e| e.name == "Bob"));
        assert!(!result.entities.iter().any(|e| e.name == "Charlie"));
    }

    #[test]
    fn test_traverse_depth_limit() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

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

        // Depth 1: should only find A and B
        let result = backend.traverse(&a.id, 1, None, None).unwrap();
        assert_eq!(result.entities.len(), 2);

        // Depth 2: should find A, B, C
        let result = backend.traverse(&a.id, 2, None, None).unwrap();
        assert_eq!(result.entities.len(), 3);
    }

    #[test]
    fn test_find_path_no_path() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        let alice = create_test_entity("Alice", EntityType::Person);
        let bob = create_test_entity("Bob", EntityType::Person);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&bob).unwrap();

        // No relationship between them
        let path = backend.find_path(&alice.id, &bob.id, 5).unwrap();
        assert!(path.is_none());
    }

    #[test]
    fn test_find_path_direct() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

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

        let path = backend.find_path(&alice.id, &bob.id, 5).unwrap();
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(path.entities.len(), 2);
        assert_eq!(path.relationships.len(), 1);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Additional Temporal Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_close_entity_valid_time() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        let entity = Entity::new(EntityType::Person, "Alice", Domain::for_user())
            .with_valid_time(ValidTimeRange::from(100));

        backend.store_entity(&entity).unwrap();

        // Close the valid time at 200
        backend.close_entity_valid_time(&entity.id, 200).unwrap();

        // Query at 150 (should find)
        let point = BitemporalPoint::new(150, i64::MAX);
        let results = backend
            .query_entities_at(&EntityQuery::new(), &point)
            .unwrap();
        assert_eq!(results.len(), 1);

        // Query at 250 (should not find)
        let point = BitemporalPoint::new(250, i64::MAX);
        let results = backend
            .query_entities_at(&EntityQuery::new(), &point)
            .unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_close_relationship_valid_time() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        let alice = create_test_entity("Alice", EntityType::Person);
        let acme = create_test_entity("Acme", EntityType::Organization);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&acme).unwrap();

        let rel = Relationship::new(alice.id.clone(), acme.id.clone(), RelationshipType::WorksAt)
            .with_valid_time(ValidTimeRange::from(100));

        backend.store_relationship(&rel).unwrap();

        // Close the relationship
        backend
            .close_relationship_valid_time(&alice.id, &acme.id, RelationshipType::WorksAt, 200)
            .unwrap();

        // Query at 150 (should find)
        let point = BitemporalPoint::new(150, i64::MAX);
        let results = backend
            .query_relationships_at(&RelationshipQuery::new(), &point)
            .unwrap();
        assert_eq!(results.len(), 1);

        // Query at 250 (should not find)
        let point = BitemporalPoint::new(250, i64::MAX);
        let results = backend
            .query_relationships_at(&RelationshipQuery::new(), &point)
            .unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_relationship_temporal_queries() {
        let backend = SqliteGraphBackend::in_memory().unwrap();

        let alice = create_test_entity("Alice", EntityType::Person);
        let acme = create_test_entity("Acme", EntityType::Organization);

        backend.store_entity(&alice).unwrap();
        backend.store_entity(&acme).unwrap();

        let rel = Relationship::new(alice.id.clone(), acme.id.clone(), RelationshipType::WorksAt)
            .with_valid_time(ValidTimeRange::between(100, 200));

        backend.store_relationship(&rel).unwrap();

        // Query during valid period
        let point = BitemporalPoint::new(150, i64::MAX);
        let results = backend
            .query_relationships_at(&RelationshipQuery::new(), &point)
            .unwrap();
        assert_eq!(results.len(), 1);

        // Query before valid period
        let point = BitemporalPoint::new(50, i64::MAX);
        let results = backend
            .query_relationships_at(&RelationshipQuery::new(), &point)
            .unwrap();
        assert_eq!(results.len(), 0);
    }
}
