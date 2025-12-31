//! SQLite-based prompt storage for user scope.
//!
//! Stores prompts in `~/.config/subcog/_prompts/prompts.db`.

use super::PromptStorage;
use crate::models::PromptTemplate;
use crate::{Error, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// `SQLite`-based prompt storage.
pub struct SqlitePromptStorage {
    /// Connection to the `SQLite` database.
    conn: Mutex<Connection>,
    /// Path to the `SQLite` database.
    db_path: PathBuf,
}

impl SqlitePromptStorage {
    /// Creates a new `SQLite` prompt storage.
    ///
    /// # Arguments
    ///
    /// * `db_path` - Path to the `SQLite` database file
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or initialized.
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::OperationFailed {
                operation: "create_prompt_dir".to_string(),
                cause: e.to_string(),
            })?;
        }

        let conn = Connection::open(&db_path).map_err(|e| Error::OperationFailed {
            operation: "open_prompt_db".to_string(),
            cause: e.to_string(),
        })?;

        let storage = Self {
            conn: Mutex::new(conn),
            db_path,
        };

        storage.initialize()?;
        Ok(storage)
    }

    /// Creates an in-memory `SQLite` storage (useful for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be initialized.
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| Error::OperationFailed {
            operation: "open_prompt_db_memory".to_string(),
            cause: e.to_string(),
        })?;

        let storage = Self {
            conn: Mutex::new(conn),
            db_path: PathBuf::from(":memory:"),
        };

        storage.initialize()?;
        Ok(storage)
    }

    /// Returns the default user-scope database path.
    ///
    /// Returns `~/.config/subcog/_prompts/prompts.db` on Unix systems,
    /// or the platform-specific config directory.
    #[must_use]
    pub fn default_user_path() -> Option<PathBuf> {
        directories::BaseDirs::new().map(|d| {
            d.config_dir()
                .join("subcog")
                .join("_prompts")
                .join("prompts.db")
        })
    }

    /// Returns the database path.
    #[must_use]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Initializes the database schema.
    fn initialize(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_prompt_db".to_string(),
            cause: e.to_string(),
        })?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS prompts (
                name TEXT PRIMARY KEY,
                description TEXT NOT NULL DEFAULT '',
                content TEXT NOT NULL,
                variables TEXT NOT NULL DEFAULT '[]',
                tags TEXT NOT NULL DEFAULT '[]',
                author TEXT,
                usage_count INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            [],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "create_prompts_table".to_string(),
            cause: e.to_string(),
        })?;

        // Create index on tags for faster filtering
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_prompts_tags ON prompts(tags)",
            [],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "create_prompts_tags_index".to_string(),
            cause: e.to_string(),
        })?;

        Ok(())
    }

    /// Locks the connection and returns a guard.
    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_prompt_db".to_string(),
            cause: e.to_string(),
        })
    }
}

impl PromptStorage for SqlitePromptStorage {
    #[allow(clippy::cast_possible_wrap)]
    fn save(&self, template: &PromptTemplate) -> Result<String> {
        let conn = self.lock_conn()?;

        let variables_json =
            serde_json::to_string(&template.variables).map_err(|e| Error::OperationFailed {
                operation: "serialize_variables".to_string(),
                cause: e.to_string(),
            })?;

        let tags_json =
            serde_json::to_string(&template.tags).map_err(|e| Error::OperationFailed {
                operation: "serialize_tags".to_string(),
                cause: e.to_string(),
            })?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Use INSERT OR REPLACE to handle updates
        conn.execute(
            "INSERT OR REPLACE INTO prompts
             (name, description, content, variables, tags, author, usage_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6,
                     COALESCE((SELECT usage_count FROM prompts WHERE name = ?1), 0),
                     COALESCE((SELECT created_at FROM prompts WHERE name = ?1), ?7),
                     ?7)",
            params![
                template.name,
                template.description,
                template.content,
                variables_json,
                tags_json,
                template.author,
                now as i64,
            ],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "save_prompt".to_string(),
            cause: e.to_string(),
        })?;

        Ok(format!("prompt_user_{}", template.name))
    }

    #[allow(clippy::cast_sign_loss)]
    fn get(&self, name: &str) -> Result<Option<PromptTemplate>> {
        let conn = self.lock_conn()?;

        let result = conn
            .query_row(
                "SELECT name, description, content, variables, tags, author, usage_count, created_at, updated_at
                 FROM prompts WHERE name = ?1",
                params![name],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, i64>(8)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| Error::OperationFailed {
                operation: "get_prompt".to_string(),
                cause: e.to_string(),
            })?;

        match result {
            Some((
                name,
                description,
                content,
                variables_json,
                tags_json,
                author,
                usage_count,
                created_at,
                updated_at,
            )) => {
                let variables = serde_json::from_str(&variables_json).unwrap_or_default();
                let tags = serde_json::from_str(&tags_json).unwrap_or_default();

                Ok(Some(PromptTemplate {
                    name,
                    description,
                    content,
                    variables,
                    tags,
                    author,
                    usage_count: usage_count as u64,
                    created_at: created_at as u64,
                    updated_at: updated_at as u64,
                }))
            },
            None => Ok(None),
        }
    }

    #[allow(clippy::cast_sign_loss)]
    fn list(
        &self,
        tags: Option<&[String]>,
        name_pattern: Option<&str>,
    ) -> Result<Vec<PromptTemplate>> {
        let conn = self.lock_conn()?;

        let mut sql = String::from(
            "SELECT name, description, content, variables, tags, author, usage_count, created_at, updated_at
             FROM prompts WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        // Add name pattern filter (convert glob to SQL LIKE)
        if let Some(pattern) = name_pattern {
            let like_pattern = pattern.replace('*', "%").replace('?', "_");
            sql.push_str(" AND name LIKE ?");
            params_vec.push(Box::new(like_pattern));
        }

        sql.push_str(" ORDER BY usage_count DESC, name ASC");

        let mut stmt = conn.prepare(&sql).map_err(|e| Error::OperationFailed {
            operation: "prepare_list_prompts".to_string(),
            cause: e.to_string(),
        })?;

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(AsRef::as_ref).collect();

        let rows = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            })
            .map_err(|e| Error::OperationFailed {
                operation: "list_prompts".to_string(),
                cause: e.to_string(),
            })?;

        let mut results = Vec::new();
        for row in rows {
            let (
                name,
                description,
                content,
                variables_json,
                tags_json,
                author,
                usage_count,
                created_at,
                updated_at,
            ) = row.map_err(|e| Error::OperationFailed {
                operation: "read_prompt_row".to_string(),
                cause: e.to_string(),
            })?;

            let variables = serde_json::from_str(&variables_json).unwrap_or_default();
            let prompt_tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

            // Filter by tags if specified
            let has_all_required_tags = tags
                .is_none_or(|required_tags| required_tags.iter().all(|t| prompt_tags.contains(t)));
            if !has_all_required_tags {
                continue;
            }

            results.push(PromptTemplate {
                name,
                description,
                content,
                variables,
                tags: prompt_tags,
                author,
                usage_count: usage_count as u64,
                created_at: created_at as u64,
                updated_at: updated_at as u64,
            });
        }

        Ok(results)
    }

    fn delete(&self, name: &str) -> Result<bool> {
        let conn = self.lock_conn()?;

        let rows_affected = conn
            .execute("DELETE FROM prompts WHERE name = ?1", params![name])
            .map_err(|e| Error::OperationFailed {
                operation: "delete_prompt".to_string(),
                cause: e.to_string(),
            })?;

        Ok(rows_affected > 0)
    }

    #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
    fn increment_usage(&self, name: &str) -> Result<u64> {
        let conn = self.lock_conn()?;

        conn.execute(
            "UPDATE prompts SET usage_count = usage_count + 1, updated_at = ?1 WHERE name = ?2",
            params![
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0),
                name
            ],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "increment_usage".to_string(),
            cause: e.to_string(),
        })?;

        // Get the new count
        let count: i64 = conn
            .query_row(
                "SELECT usage_count FROM prompts WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .map_err(|e| Error::OperationFailed {
                operation: "get_usage_count".to_string(),
                cause: e.to_string(),
            })?;

        Ok(count as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_prompt_storage_creation() {
        let storage = SqlitePromptStorage::in_memory().unwrap();
        assert_eq!(storage.db_path().to_str(), Some(":memory:"));
    }

    #[test]
    fn test_save_and_get_prompt() {
        let storage = SqlitePromptStorage::in_memory().unwrap();

        let template =
            PromptTemplate::new("test-prompt", "Hello {{name}}!").with_description("A test prompt");

        let id = storage.save(&template).unwrap();
        assert!(id.contains("test-prompt"));

        let retrieved = storage.get("test-prompt").unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.name, "test-prompt");
        assert_eq!(retrieved.content, "Hello {{name}}!");
        assert_eq!(retrieved.description, "A test prompt");
    }

    #[test]
    fn test_list_prompts() {
        let storage = SqlitePromptStorage::in_memory().unwrap();

        storage
            .save(&PromptTemplate::new("alpha", "A").with_tags(vec!["tag1".to_string()]))
            .unwrap();
        storage
            .save(
                &PromptTemplate::new("beta", "B")
                    .with_tags(vec!["tag1".to_string(), "tag2".to_string()]),
            )
            .unwrap();
        storage.save(&PromptTemplate::new("gamma", "C")).unwrap();

        // List all
        let all = storage.list(None, None).unwrap();
        assert_eq!(all.len(), 3);

        // Filter by tag
        let with_tag1 = storage.list(Some(&["tag1".to_string()]), None).unwrap();
        assert_eq!(with_tag1.len(), 2);

        // Filter by name pattern
        let alpha_pattern = storage.list(None, Some("a*")).unwrap();
        assert_eq!(alpha_pattern.len(), 1);
        assert_eq!(alpha_pattern[0].name, "alpha");
    }

    #[test]
    fn test_delete_prompt() {
        let storage = SqlitePromptStorage::in_memory().unwrap();

        storage
            .save(&PromptTemplate::new("to-delete", "Content"))
            .unwrap();

        assert!(storage.get("to-delete").unwrap().is_some());
        assert!(storage.delete("to-delete").unwrap());
        assert!(storage.get("to-delete").unwrap().is_none());
        assert!(!storage.delete("to-delete").unwrap()); // Already deleted
    }

    #[test]
    fn test_increment_usage() {
        let storage = SqlitePromptStorage::in_memory().unwrap();

        storage
            .save(&PromptTemplate::new("used-prompt", "Content"))
            .unwrap();

        let count1 = storage.increment_usage("used-prompt").unwrap();
        assert_eq!(count1, 1);

        let count2 = storage.increment_usage("used-prompt").unwrap();
        assert_eq!(count2, 2);

        let prompt = storage.get("used-prompt").unwrap().unwrap();
        assert_eq!(prompt.usage_count, 2);
    }

    #[test]
    fn test_update_existing_prompt() {
        let storage = SqlitePromptStorage::in_memory().unwrap();

        // Save initial version
        storage
            .save(&PromptTemplate::new("update-me", "Version 1"))
            .unwrap();

        // Increment usage
        storage.increment_usage("update-me").unwrap();

        // Update content
        storage
            .save(&PromptTemplate::new("update-me", "Version 2").with_description("Updated"))
            .unwrap();

        // Verify update preserved usage count
        let prompt = storage.get("update-me").unwrap().unwrap();
        assert_eq!(prompt.content, "Version 2");
        assert_eq!(prompt.description, "Updated");
        assert_eq!(prompt.usage_count, 1); // Preserved from before update
    }

    #[test]
    fn test_default_user_path() {
        let path = SqlitePromptStorage::default_user_path();
        // Should return Some on most systems
        if let Some(p) = path {
            assert!(p.to_string_lossy().contains("_prompts"));
            assert!(p.to_string_lossy().ends_with("prompts.db"));
        }
    }
}
