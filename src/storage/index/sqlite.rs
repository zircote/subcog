//! `SQLite` + FTS5 index backend.
//!
//! Provides full-text search using `SQLite`'s FTS5 extension.

use crate::models::{Memory, MemoryId, SearchFilter};
use crate::storage::traits::IndexBackend;
use crate::{Error, Result};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// SQLite-based index backend with FTS5.
pub struct SqliteBackend {
    /// Connection to the `SQLite` database.
    conn: Mutex<Connection>,
    /// Path to the `SQLite` database (None for in-memory).
    db_path: Option<PathBuf>,
}

impl SqliteBackend {
    /// Creates a new `SQLite` backend.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or initialized.
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        let conn = Connection::open(&db_path).map_err(|e| Error::OperationFailed {
            operation: "open_sqlite".to_string(),
            cause: e.to_string(),
        })?;

        let backend = Self {
            conn: Mutex::new(conn),
            db_path: Some(db_path),
        };

        backend.initialize()?;
        Ok(backend)
    }

    /// Creates an in-memory `SQLite` backend (useful for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be initialized.
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| Error::OperationFailed {
            operation: "open_sqlite_memory".to_string(),
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
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        // Create the main table for memory metadata
        conn.execute(
            "CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                namespace TEXT NOT NULL,
                domain TEXT,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                tags TEXT
            )",
            [],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "create_memories_table".to_string(),
            cause: e.to_string(),
        })?;

        // Create FTS5 virtual table for full-text search (standalone, not synced with memories)
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                id,
                content,
                tags
            )",
            [],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "create_fts_table".to_string(),
            cause: e.to_string(),
        })?;

        Ok(())
    }

    /// Builds a WHERE clause from a search filter with numbered parameters.
    /// Returns the clause string, the parameters, and the next parameter index.
    fn build_filter_clause_numbered(
        &self,
        filter: &SearchFilter,
        start_param: usize,
    ) -> (String, Vec<String>, usize) {
        let mut conditions = Vec::new();
        let mut params = Vec::new();
        let mut param_idx = start_param;

        if !filter.namespaces.is_empty() {
            let placeholders: Vec<String> = filter
                .namespaces
                .iter()
                .map(|_| {
                    let p = format!("?{param_idx}");
                    param_idx += 1;
                    p
                })
                .collect();
            conditions.push(format!("m.namespace IN ({})", placeholders.join(",")));
            for ns in &filter.namespaces {
                params.push(ns.as_str().to_string());
            }
        }

        if !filter.statuses.is_empty() {
            let placeholders: Vec<String> = filter
                .statuses
                .iter()
                .map(|_| {
                    let p = format!("?{param_idx}");
                    param_idx += 1;
                    p
                })
                .collect();
            conditions.push(format!("m.status IN ({})", placeholders.join(",")));
            for s in &filter.statuses {
                params.push(s.as_str().to_string());
            }
        }

        if let Some(after) = filter.created_after {
            conditions.push(format!("m.created_at >= ?{param_idx}"));
            param_idx += 1;
            params.push(after.to_string());
        }

        if let Some(before) = filter.created_before {
            conditions.push(format!("m.created_at <= ?{param_idx}"));
            param_idx += 1;
            params.push(before.to_string());
        }

        let clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" AND {}", conditions.join(" AND "))
        };

        (clause, params, param_idx)
    }
}

impl IndexBackend for SqliteBackend {
    fn index(&mut self, memory: &Memory) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let tags_str = memory.tags.join(",");
        let domain_str = memory.domain.to_string();

        // Insert or replace in main table
        conn.execute(
            "INSERT OR REPLACE INTO memories (id, namespace, domain, status, created_at, tags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                memory.id.as_str(),
                memory.namespace.as_str(),
                domain_str,
                memory.status.as_str(),
                memory.created_at,
                tags_str
            ],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "insert_memory".to_string(),
            cause: e.to_string(),
        })?;

        // Delete from FTS if exists (FTS5 uses rowid internally for matching)
        conn.execute(
            "DELETE FROM memories_fts WHERE id = ?1",
            params![memory.id.as_str()],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "delete_fts".to_string(),
            cause: e.to_string(),
        })?;

        // Insert into FTS table
        conn.execute(
            "INSERT INTO memories_fts (id, content, tags) VALUES (?1, ?2, ?3)",
            params![memory.id.as_str(), memory.content, tags_str],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "insert_fts".to_string(),
            cause: e.to_string(),
        })?;

        Ok(())
    }

    fn remove(&mut self, id: &MemoryId) -> Result<bool> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        // Delete from FTS
        conn.execute(
            "DELETE FROM memories_fts WHERE id = ?1",
            params![id.as_str()],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "delete_fts".to_string(),
            cause: e.to_string(),
        })?;

        // Delete from main table
        let deleted = conn
            .execute("DELETE FROM memories WHERE id = ?1", params![id.as_str()])
            .map_err(|e| Error::OperationFailed {
                operation: "delete_memory".to_string(),
                cause: e.to_string(),
            })?;

        Ok(deleted > 0)
    }

    fn search(
        &self,
        query: &str,
        filter: &SearchFilter,
        limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        // Build filter clause with numbered parameters starting from ?2
        // ?1 is the FTS query
        let (filter_clause, filter_params, next_param) =
            self.build_filter_clause_numbered(filter, 2);

        // Use FTS5 MATCH for search with BM25 ranking
        // Limit parameter comes after all filter parameters
        let sql = format!(
            "SELECT f.id, bm25(memories_fts) as score
             FROM memories_fts f
             JOIN memories m ON f.id = m.id
             WHERE memories_fts MATCH ?1 {filter_clause}
             ORDER BY score
             LIMIT ?{next_param}"
        );

        let mut stmt = conn.prepare(&sql).map_err(|e| Error::OperationFailed {
            operation: "prepare_search".to_string(),
            cause: e.to_string(),
        })?;

        // Build parameters: query, filter params, limit
        let mut results = Vec::new();

        // FTS5 query - escape special characters and wrap terms in quotes
        // FTS5 special chars: - (NOT), * (prefix), " (phrase), : (column)
        let fts_query = query
            .split_whitespace()
            .map(|term| {
                // Escape double quotes and wrap each term in quotes for literal matching
                let escaped = term.replace('"', "\"\"");
                format!("\"{escaped}\"")
            })
            .collect::<Vec<_>>()
            .join(" OR ");

        let rows = stmt
            .query_map(
                rusqlite::params_from_iter(
                    std::iter::once(fts_query)
                        .chain(filter_params.into_iter())
                        .chain(std::iter::once(limit.to_string())),
                ),
                |row| {
                    let id: String = row.get(0)?;
                    let score: f64 = row.get(1)?;
                    Ok((id, score))
                },
            )
            .map_err(|e| Error::OperationFailed {
                operation: "execute_search".to_string(),
                cause: e.to_string(),
            })?;

        for row in rows {
            let (id, score) = row.map_err(|e| Error::OperationFailed {
                operation: "read_search_row".to_string(),
                cause: e.to_string(),
            })?;

            // Normalize BM25 score (BM25 returns negative values, lower is better)
            // Convert to 0-1 range where higher is better
            #[allow(clippy::cast_possible_truncation)]
            let normalized_score = (1.0 / (1.0 - score)).min(1.0) as f32;

            results.push((MemoryId::new(id), normalized_score));
        }

        // Apply minimum score filter if specified
        if let Some(min_score) = filter.min_score {
            results.retain(|(_, score)| *score >= min_score);
        }

        Ok(results)
    }

    fn clear(&mut self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        conn.execute("DELETE FROM memories_fts", [])
            .map_err(|e| Error::OperationFailed {
                operation: "clear_fts".to_string(),
                cause: e.to_string(),
            })?;

        conn.execute("DELETE FROM memories", [])
            .map_err(|e| Error::OperationFailed {
                operation: "clear_memories".to_string(),
                cause: e.to_string(),
            })?;

        Ok(())
    }

    fn list_all(&self, filter: &SearchFilter, limit: usize) -> Result<Vec<(MemoryId, f32)>> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        // Build filter clause (starting at parameter 1, no FTS query)
        let (filter_clause, filter_params, next_param) =
            self.build_filter_clause_numbered(filter, 1);

        // Query all memories without FTS MATCH, ordered by created_at desc
        let sql = format!(
            "SELECT m.id, 1.0 as score
             FROM memories m
             WHERE 1=1 {filter_clause}
             ORDER BY m.created_at DESC
             LIMIT ?{next_param}"
        );

        let mut stmt = conn.prepare(&sql).map_err(|e| Error::OperationFailed {
            operation: "prepare_list_all".to_string(),
            cause: e.to_string(),
        })?;

        let mut results = Vec::new();

        let rows = stmt
            .query_map(
                rusqlite::params_from_iter(
                    filter_params
                        .into_iter()
                        .chain(std::iter::once(limit.to_string())),
                ),
                |row| {
                    let id: String = row.get(0)?;
                    let score: f64 = row.get(1)?;
                    Ok((id, score))
                },
            )
            .map_err(|e| Error::OperationFailed {
                operation: "list_all".to_string(),
                cause: e.to_string(),
            })?;

        for row in rows {
            let (id, score) = row.map_err(|e| Error::OperationFailed {
                operation: "read_list_row".to_string(),
                cause: e.to_string(),
            })?;

            #[allow(clippy::cast_possible_truncation)]
            results.push((MemoryId::new(id), score as f32));
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, MemoryStatus, Namespace};

    fn create_test_memory(id: &str, content: &str, namespace: Namespace) -> Memory {
        Memory {
            id: MemoryId::new(id),
            content: content.to_string(),
            namespace,
            domain: Domain::new(),
            status: MemoryStatus::Active,
            created_at: 1_234_567_890,
            updated_at: 1_234_567_890,
            embedding: None,
            tags: vec!["test".to_string()],
            source: None,
        }
    }

    #[test]
    fn test_index_and_search() {
        let mut backend = SqliteBackend::in_memory().unwrap();

        let memory1 = create_test_memory("id1", "Rust programming language", Namespace::Decisions);
        let memory2 = create_test_memory("id2", "Python scripting", Namespace::Learnings);
        let memory3 =
            create_test_memory("id3", "Rust ownership and borrowing", Namespace::Patterns);

        backend.index(&memory1).unwrap();
        backend.index(&memory2).unwrap();
        backend.index(&memory3).unwrap();

        // Search for "Rust"
        let results = backend.search("Rust", &SearchFilter::new(), 10).unwrap();

        assert_eq!(results.len(), 2);
        let ids: Vec<_> = results.iter().map(|(id, _)| id.as_str()).collect();
        assert!(ids.contains(&"id1"));
        assert!(ids.contains(&"id3"));
    }

    #[test]
    fn test_search_with_namespace_filter() {
        let mut backend = SqliteBackend::in_memory().unwrap();

        let memory1 = create_test_memory("id1", "Rust programming", Namespace::Decisions);
        let memory2 = create_test_memory("id2", "Rust patterns", Namespace::Patterns);

        backend.index(&memory1).unwrap();
        backend.index(&memory2).unwrap();

        // Search with namespace filter
        let filter = SearchFilter::new().with_namespace(Namespace::Patterns);
        let results = backend.search("Rust", &filter, 10).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.as_str(), "id2");
    }

    #[test]
    fn test_remove() {
        let mut backend = SqliteBackend::in_memory().unwrap();

        let memory = create_test_memory("to_remove", "Test content", Namespace::Decisions);
        backend.index(&memory).unwrap();

        // Verify it exists
        let results = backend.search("content", &SearchFilter::new(), 10).unwrap();
        assert_eq!(results.len(), 1);

        // Remove it
        let removed = backend.remove(&MemoryId::new("to_remove")).unwrap();
        assert!(removed);

        // Verify it's gone
        let results = backend.search("content", &SearchFilter::new(), 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut backend = SqliteBackend::in_memory().unwrap();

        backend
            .index(&create_test_memory("id1", "content1", Namespace::Decisions))
            .unwrap();
        backend
            .index(&create_test_memory("id2", "content2", Namespace::Decisions))
            .unwrap();

        backend.clear().unwrap();

        let results = backend.search("content", &SearchFilter::new(), 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_reindex() {
        let mut backend = SqliteBackend::in_memory().unwrap();

        let memories = vec![
            create_test_memory("id1", "memory one", Namespace::Decisions),
            create_test_memory("id2", "memory two", Namespace::Learnings),
            create_test_memory("id3", "memory three", Namespace::Patterns),
        ];

        backend.reindex(&memories).unwrap();

        let results = backend.search("memory", &SearchFilter::new(), 10).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_update_index() {
        let mut backend = SqliteBackend::in_memory().unwrap();

        let mut memory =
            create_test_memory("update_test", "original content", Namespace::Decisions);
        backend.index(&memory).unwrap();

        // Update the memory
        memory.content = "updated content completely different".to_string();
        backend.index(&memory).unwrap();

        // Search for old content should not find it
        let old_results = backend
            .search("original", &SearchFilter::new(), 10)
            .unwrap();
        assert!(old_results.is_empty());

        // Search for new content should find it
        let new_results = backend
            .search("different", &SearchFilter::new(), 10)
            .unwrap();
        assert_eq!(new_results.len(), 1);
    }
}
