//! Row conversion utilities for `SQLite` memory storage.
//!
//! This module provides the `MemoryRow` struct and conversion functions for
//! transforming between database rows and `Memory` objects. This shared code
//! is used by both the index and persistence backends to ensure consistent
//! behavior across the storage layer.

use crate::models::{Domain, Memory, MemoryId, MemoryStatus, Namespace};
use crate::{Error, Result};
use chrono::{TimeZone, Utc};
use rusqlite::{Connection, OptionalExtension, params};

/// Internal representation of a memory row from the database.
///
/// This struct maps directly to the database schema with all fields as their
/// primitive types. Use [`build_memory_from_row`] to convert to a [`Memory`]
/// object with proper type conversions.
#[derive(Debug)]
pub struct MemoryRow {
    /// Unique identifier for the memory.
    pub id: String,
    /// Namespace category (e.g., "decisions", "patterns", "learnings").
    pub namespace: String,
    /// Domain scope (organization/repository).
    pub domain: Option<String>,
    /// Project identifier within the domain.
    pub project_id: Option<String>,
    /// Git branch associated with this memory.
    pub branch: Option<String>,
    /// File path context where the memory was captured.
    pub file_path: Option<String>,
    /// Current status (active, archived, superseded, etc.).
    pub status: String,
    /// Unix timestamp (seconds since epoch) when the memory was created.
    pub created_at: i64,
    /// Optional unix timestamp when the memory was tombstoned.
    pub tombstoned_at: Option<i64>,
    /// Comma-separated list of tags.
    pub tags: Option<String>,
    /// Source/origin of the memory.
    pub source: Option<String>,
    /// The actual memory content.
    pub content: String,
}

/// Fetches a memory row from the database by ID.
///
/// Performs a join between the `memories` table and `memories_fts` table to
/// retrieve both metadata and content. Returns `None` if the memory does not exist.
///
/// # Arguments
///
/// * `conn` - Active `SQLite` connection.
/// * `id` - Memory identifier to fetch.
///
/// # Returns
///
/// * `Ok(Some(MemoryRow))` - Memory found and retrieved successfully.
/// * `Ok(None)` - Memory not found.
/// * `Err(_)` - Database error occurred.
///
/// # Errors
///
/// Returns [`Error::OperationFailed`] if the query preparation or execution fails.
///
/// # Examples
///
/// ```ignore
/// use subcog::models::MemoryId;
/// use subcog::storage::sqlite::fetch_memory_row;
///
/// let memory_id = MemoryId::new("abc123");
/// match fetch_memory_row(&conn, &memory_id)? {
///     Some(row) => println!("Found: {}", row.content),
///     None => println!("Memory not found"),
/// }
/// # Ok::<(), subcog::Error>(())
/// ```
pub fn fetch_memory_row(conn: &Connection, id: &MemoryId) -> Result<Option<MemoryRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.namespace, m.domain, m.project_id, m.branch, m.file_path, m.status, m.created_at,
                    m.tombstoned_at, m.tags, m.source, f.content
             FROM memories m
             JOIN memories_fts f ON m.id = f.id
             WHERE m.id = ?1",
        )
        .map_err(|e| Error::OperationFailed {
            operation: "prepare_get_memory".to_string(),
            cause: e.to_string(),
        })?;

    let result: std::result::Result<Option<_>, _> = stmt
        .query_row(params![id.as_str()], |row| {
            Ok(MemoryRow {
                id: row.get(0)?,
                namespace: row.get(1)?,
                domain: row.get(2)?,
                project_id: row.get(3)?,
                branch: row.get(4)?,
                file_path: row.get(5)?,
                status: row.get(6)?,
                created_at: row.get(7)?,
                tombstoned_at: row.get(8)?,
                tags: row.get(9)?,
                source: row.get(10)?,
                content: row.get(11)?,
            })
        })
        .optional();

    result.map_err(|e| Error::OperationFailed {
        operation: "get_memory".to_string(),
        cause: e.to_string(),
    })
}

/// Converts a `MemoryRow` to a `Memory` object with proper type conversions.
///
/// This function handles:
/// - Parsing namespace strings to [`Namespace`] enum
/// - Converting domain strings to [`Domain`] structs (organization/project/repository)
/// - Parsing status strings to [`MemoryStatus`] enum
/// - Splitting comma-separated tags into a `Vec<String>`
/// - Converting unix timestamps to proper datetime types
///
/// # Parsing Rules
///
/// ## Namespace
/// - Parsed using `Namespace::parse()` with fallback to default
///
/// ## Domain
/// - Empty or "project" → Default domain
/// - "org" → Organization only
/// - "org/repo" → Organization and repository
/// - Invalid formats → Default domain
///
/// ## Status
/// - Case-insensitive matching: "active", "archived", "superseded", "pending", "deleted", "tombstoned"
/// - Unknown values → Defaults to `MemoryStatus::Active`
///
/// ## Tags
/// - Split on commas, trimmed, empty strings filtered out
/// - `None` or empty string → Empty vector
///
/// # Arguments
///
/// * `row` - Database row to convert.
///
/// # Returns
///
/// A fully-constructed [`Memory`] object with all fields properly typed.
///
/// # Examples
///
/// ```ignore
/// use subcog::storage::sqlite::{fetch_memory_row, build_memory_from_row};
/// use subcog::models::MemoryId;
///
/// let memory_id = MemoryId::new("abc123");
/// if let Some(row) = fetch_memory_row(&conn, &memory_id)? {
///     let memory = build_memory_from_row(row);
///     println!("Namespace: {}", memory.namespace.as_str());
///     println!("Tags: {:?}", memory.tags);
/// }
/// # Ok::<(), subcog::Error>(())
/// ```
#[must_use]
pub fn build_memory_from_row(row: MemoryRow) -> Memory {
    let namespace = Namespace::parse(&row.namespace).unwrap_or_default();
    let domain = row.domain.map_or_else(Domain::new, |d: String| {
        if d.is_empty() || d == "project" {
            Domain::new()
        } else {
            let parts: Vec<&str> = d.split('/').collect();
            match parts.len() {
                1 => Domain {
                    organization: Some(parts[0].to_string()),
                    project: None,
                    repository: None,
                },
                2 => Domain {
                    organization: Some(parts[0].to_string()),
                    project: None,
                    repository: Some(parts[1].to_string()),
                },
                _ => Domain::new(),
            }
        }
    });

    let status = match row.status.to_lowercase().as_str() {
        "active" => MemoryStatus::Active,
        "archived" => MemoryStatus::Archived,
        "superseded" => MemoryStatus::Superseded,
        "pending" => MemoryStatus::Pending,
        "deleted" => MemoryStatus::Deleted,
        "tombstoned" => MemoryStatus::Tombstoned,
        _ => MemoryStatus::Active,
    };

    let tags: Vec<String> = row
        .tags
        .map(|t| {
            t.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    #[allow(clippy::cast_sign_loss)]
    let created_at_u64 = row.created_at as u64;
    let tombstoned_at = row
        .tombstoned_at
        .and_then(|ts| Utc.timestamp_opt(ts, 0).single());

    Memory {
        id: MemoryId::new(row.id),
        content: row.content,
        namespace,
        domain,
        project_id: row.project_id,
        branch: row.branch,
        file_path: row.file_path,
        status,
        created_at: created_at_u64,
        updated_at: created_at_u64,
        tombstoned_at,
        embedding: None,
        tags,
        source: row.source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_memory_from_row_basic() {
        let row = MemoryRow {
            id: "test123".to_string(),
            namespace: "decisions".to_string(),
            domain: None,
            project_id: None,
            branch: None,
            file_path: None,
            status: "active".to_string(),
            created_at: 1_000_000_000,
            tombstoned_at: None,
            tags: Some("foo,bar,baz".to_string()),
            source: Some("cli".to_string()),
            content: "Test content".to_string(),
        };

        let memory = build_memory_from_row(row);

        assert_eq!(memory.id.as_str(), "test123");
        assert_eq!(memory.namespace.as_str(), "decisions");
        assert_eq!(memory.content, "Test content");
        assert_eq!(memory.tags, vec!["foo", "bar", "baz"]);
        assert_eq!(memory.created_at, 1_000_000_000);
        assert!(matches!(memory.status, MemoryStatus::Active));
        assert!(memory.domain.organization.is_none());
    }

    #[test]
    fn test_build_memory_from_row_with_domain_org_only() {
        let row = MemoryRow {
            id: "test123".to_string(),
            namespace: "patterns".to_string(),
            domain: Some("myorg".to_string()),
            project_id: None,
            branch: None,
            file_path: None,
            status: "active".to_string(),
            created_at: 1_000_000_000,
            tombstoned_at: None,
            tags: None,
            source: None,
            content: "Test".to_string(),
        };

        let memory = build_memory_from_row(row);

        assert_eq!(memory.domain.organization, Some("myorg".to_string()));
        assert_eq!(memory.domain.repository, None);
    }

    #[test]
    fn test_build_memory_from_row_with_domain_org_and_repo() {
        let row = MemoryRow {
            id: "test123".to_string(),
            namespace: "learnings".to_string(),
            domain: Some("myorg/myrepo".to_string()),
            project_id: None,
            branch: None,
            file_path: None,
            status: "active".to_string(),
            created_at: 1_000_000_000,
            tombstoned_at: None,
            tags: None,
            source: None,
            content: "Test".to_string(),
        };

        let memory = build_memory_from_row(row);

        assert_eq!(memory.domain.organization, Some("myorg".to_string()));
        assert_eq!(memory.domain.repository, Some("myrepo".to_string()));
    }

    #[test]
    fn test_build_memory_from_row_domain_project_defaults() {
        let row = MemoryRow {
            id: "test123".to_string(),
            namespace: "decisions".to_string(),
            domain: Some("project".to_string()),
            project_id: None,
            branch: None,
            file_path: None,
            status: "active".to_string(),
            created_at: 1_000_000_000,
            tombstoned_at: None,
            tags: None,
            source: None,
            content: "Test".to_string(),
        };

        let memory = build_memory_from_row(row);

        assert!(memory.domain.organization.is_none());
        assert!(memory.domain.repository.is_none());
    }

    #[test]
    fn test_build_memory_from_row_all_statuses() {
        let statuses = vec![
            ("active", MemoryStatus::Active),
            ("ACTIVE", MemoryStatus::Active),
            ("archived", MemoryStatus::Archived),
            ("superseded", MemoryStatus::Superseded),
            ("pending", MemoryStatus::Pending),
            ("deleted", MemoryStatus::Deleted),
            ("tombstoned", MemoryStatus::Tombstoned),
            ("unknown", MemoryStatus::Active), // Default
        ];

        for (status_str, expected_status) in statuses {
            let row = MemoryRow {
                id: "test123".to_string(),
                namespace: "decisions".to_string(),
                domain: None,
                project_id: None,
                branch: None,
                file_path: None,
                status: status_str.to_string(),
                created_at: 1_000_000_000,
                tombstoned_at: None,
                tags: None,
                source: None,
                content: "Test".to_string(),
            };

            let memory = build_memory_from_row(row);
            assert_eq!(
                memory.status, expected_status,
                "Status '{}' should map to {:?}, got {:?}",
                status_str, expected_status, memory.status
            );
        }
    }

    #[test]
    fn test_build_memory_from_row_tags_parsing() {
        let test_cases: Vec<(Option<String>, Vec<&str>)> = vec![
            (Some("foo,bar,baz".to_string()), vec!["foo", "bar", "baz"]),
            (Some("  foo  ,  bar  ".to_string()), vec!["foo", "bar"]),
            (Some("single".to_string()), vec!["single"]),
            (Some(String::new()), vec![]),
            (None, vec![]),
        ];

        for (tags_input, expected) in test_cases {
            let row = MemoryRow {
                id: "test123".to_string(),
                namespace: "decisions".to_string(),
                domain: None,
                project_id: None,
                branch: None,
                file_path: None,
                status: "active".to_string(),
                created_at: 1_000_000_000,
                tombstoned_at: None,
                tags: tags_input.clone(),
                source: None,
                content: "Test".to_string(),
            };

            let memory = build_memory_from_row(row);
            assert_eq!(
                memory.tags, expected,
                "Tags input {:?} should produce {:?}, got {:?}",
                tags_input, expected, memory.tags
            );
        }
    }

    #[test]
    fn test_build_memory_from_row_with_tombstone() {
        let row = MemoryRow {
            id: "test123".to_string(),
            namespace: "decisions".to_string(),
            domain: None,
            project_id: None,
            branch: None,
            file_path: None,
            status: "tombstoned".to_string(),
            created_at: 1_000_000_000,
            tombstoned_at: Some(1_000_001_000),
            tags: None,
            source: None,
            content: "Test".to_string(),
        };

        let memory = build_memory_from_row(row);

        assert!(memory.tombstoned_at.is_some());
        assert_eq!(memory.tombstoned_at.unwrap().timestamp(), 1_000_001_000);
    }

    #[test]
    fn test_build_memory_from_row_namespace_parsing() {
        let namespaces = vec![
            "decisions",
            "patterns",
            "learnings",
            "blockers",
            "context",
            "tech-debt",
        ];

        for ns in namespaces {
            let row = MemoryRow {
                id: "test123".to_string(),
                namespace: ns.to_string(),
                domain: None,
                project_id: None,
                branch: None,
                file_path: None,
                status: "active".to_string(),
                created_at: 1_000_000_000,
                tombstoned_at: None,
                tags: None,
                source: None,
                content: "Test".to_string(),
            };

            let memory = build_memory_from_row(row);
            assert_eq!(memory.namespace.as_str(), ns);
        }
    }

    #[test]
    fn test_build_memory_from_row_invalid_namespace_defaults() {
        let row = MemoryRow {
            id: "test123".to_string(),
            namespace: "invalid_namespace".to_string(),
            domain: None,
            project_id: None,
            branch: None,
            file_path: None,
            status: "active".to_string(),
            created_at: 1_000_000_000,
            tombstoned_at: None,
            tags: None,
            source: None,
            content: "Test".to_string(),
        };

        let memory = build_memory_from_row(row);
        // Default namespace behavior
        assert_eq!(memory.namespace.as_str(), Namespace::default().as_str());
    }

    #[test]
    fn test_build_memory_from_row_with_all_optional_fields() {
        let row = MemoryRow {
            id: "test123".to_string(),
            namespace: "decisions".to_string(),
            domain: Some("myorg/myrepo".to_string()),
            project_id: Some("proj123".to_string()),
            branch: Some("main".to_string()),
            file_path: Some("src/main.rs".to_string()),
            status: "archived".to_string(),
            created_at: 1_000_000_000,
            tombstoned_at: Some(1_000_001_000),
            tags: Some("tag1,tag2".to_string()),
            source: Some("mcp".to_string()),
            content: "Full memory".to_string(),
        };

        let memory = build_memory_from_row(row);

        assert_eq!(memory.id.as_str(), "test123");
        assert_eq!(memory.project_id, Some("proj123".to_string()));
        assert_eq!(memory.branch, Some("main".to_string()));
        assert_eq!(memory.file_path, Some("src/main.rs".to_string()));
        assert_eq!(memory.source, Some("mcp".to_string()));
        assert_eq!(memory.domain.organization, Some("myorg".to_string()));
        assert_eq!(memory.domain.repository, Some("myrepo".to_string()));
        assert!(matches!(memory.status, MemoryStatus::Archived));
        assert!(memory.tombstoned_at.is_some());
        assert_eq!(memory.tags, vec!["tag1", "tag2"]);
    }

    // Note: Integration tests for fetch_memory_row are in the backend-specific
    // test modules since they require a database connection and schema setup.
}
