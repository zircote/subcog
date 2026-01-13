//! SQLite-based context template storage with versioning.
//!
//! Stores context templates in `~/.config/subcog/memories.db` with
//! automatic version incrementing on save.

use super::ContextTemplateStorage;
use crate::models::{ContextTemplate, OutputFormat, TemplateVariable};
use crate::{Error, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// `SQLite`-based context template storage with versioning support.
pub struct SqliteContextTemplateStorage {
    /// Connection to the `SQLite` database.
    conn: Mutex<Connection>,
    /// Path to the `SQLite` database.
    db_path: PathBuf,
}

impl SqliteContextTemplateStorage {
    /// Creates a new `SQLite` context template storage.
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
                operation: "create_context_template_dir".to_string(),
                cause: e.to_string(),
            })?;
        }

        let conn = Connection::open(&db_path).map_err(|e| Error::OperationFailed {
            operation: "open_context_template_db".to_string(),
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
            operation: "open_context_template_db_memory".to_string(),
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
    /// Returns `~/.config/subcog/memories.db`.
    #[must_use]
    pub fn default_user_path() -> Option<PathBuf> {
        directories::BaseDirs::new().map(|d| {
            d.home_dir()
                .join(".config")
                .join("subcog")
                .join("memories.db")
        })
    }

    /// Returns the default org-scope database path.
    ///
    /// Returns `~/.config/subcog/orgs/{org}/memories.db`.
    #[must_use]
    pub fn default_org_path(org: &str) -> Option<PathBuf> {
        directories::BaseDirs::new().map(|d| {
            d.home_dir()
                .join(".config")
                .join("subcog")
                .join("orgs")
                .join(org)
                .join("memories.db")
        })
    }

    /// Returns the database path.
    #[must_use]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Initializes the database schema and configures pragmas.
    fn initialize(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_context_template_db".to_string(),
            cause: e.to_string(),
        })?;

        // Configure SQLite pragmas for performance and reliability
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "synchronous", "NORMAL");
        let _ = conn.pragma_update(None, "busy_timeout", "5000");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS context_templates (
                name TEXT NOT NULL,
                version INTEGER NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                content TEXT NOT NULL,
                variables TEXT NOT NULL DEFAULT '[]',
                tags TEXT NOT NULL DEFAULT '[]',
                output_format TEXT NOT NULL DEFAULT 'markdown',
                author TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (name, version)
            )",
            [],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "create_context_templates_table".to_string(),
            cause: e.to_string(),
        })?;

        // Create index for efficient version lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_context_templates_name_version
             ON context_templates(name, version DESC)",
            [],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "create_context_templates_index".to_string(),
            cause: e.to_string(),
        })?;

        // Create index on tags for faster filtering
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_context_templates_tags
             ON context_templates(tags)",
            [],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "create_context_templates_tags_index".to_string(),
            cause: e.to_string(),
        })?;

        Ok(())
    }

    /// Locks the connection and returns a guard.
    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_context_template_db".to_string(),
            cause: e.to_string(),
        })
    }

    /// Runs database maintenance (VACUUM and ANALYZE).
    ///
    /// # Errors
    ///
    /// Returns an error if maintenance commands fail.
    pub fn vacuum_and_analyze(&self) -> Result<()> {
        let conn = self.lock_conn()?;

        conn.execute("VACUUM", [])
            .map_err(|e| Error::OperationFailed {
                operation: "context_template_db_vacuum".to_string(),
                cause: e.to_string(),
            })?;

        conn.execute("ANALYZE", [])
            .map_err(|e| Error::OperationFailed {
                operation: "context_template_db_analyze".to_string(),
                cause: e.to_string(),
            })?;

        Ok(())
    }

    /// Returns database statistics for monitoring.
    #[must_use]
    pub fn stats(&self) -> Option<ContextTemplateDbStats> {
        let conn = self.lock_conn().ok()?;

        let template_count: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT name) FROM context_templates",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let version_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM context_templates", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);

        let page_count: i64 = conn
            .pragma_query_value(None, "page_count", |row| row.get(0))
            .unwrap_or(0);

        let page_size: i64 = conn
            .pragma_query_value(None, "page_size", |row| row.get(0))
            .unwrap_or(4096);

        Some(ContextTemplateDbStats {
            template_count: u64::try_from(template_count).unwrap_or(0),
            version_count: u64::try_from(version_count).unwrap_or(0),
            db_size_bytes: u64::try_from(page_count.saturating_mul(page_size)).unwrap_or(0),
        })
    }

    /// Parses output format from string.
    fn parse_output_format(s: &str) -> OutputFormat {
        match s.to_lowercase().as_str() {
            "json" => OutputFormat::Json,
            "xml" => OutputFormat::Xml,
            _ => OutputFormat::Markdown,
        }
    }

    /// Converts output format to string.
    const fn format_to_string(format: OutputFormat) -> &'static str {
        match format {
            OutputFormat::Markdown => "markdown",
            OutputFormat::Json => "json",
            OutputFormat::Xml => "xml",
        }
    }
}

/// Database statistics for context template storage.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContextTemplateDbStats {
    /// Number of unique templates stored.
    pub template_count: u64,
    /// Total number of template versions stored.
    pub version_count: u64,
    /// Total database size in bytes.
    pub db_size_bytes: u64,
}

impl ContextTemplateStorage for SqliteContextTemplateStorage {
    #[allow(clippy::cast_possible_wrap)]
    fn save(&self, template: &ContextTemplate) -> Result<(String, u32)> {
        let conn = self.lock_conn()?;

        // Get the next version number (max + 1, or 1 if new)
        let next_version: u32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) + 1 FROM context_templates WHERE name = ?1",
                params![template.name],
                |row| row.get::<_, u32>(0),
            )
            .map_err(|e| Error::OperationFailed {
                operation: "get_next_version".to_string(),
                cause: e.to_string(),
            })?;

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

        let now = crate::current_timestamp();
        let output_format = Self::format_to_string(template.output_format);

        conn.execute(
            "INSERT INTO context_templates
             (name, version, description, content, variables, tags, output_format, author, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                template.name,
                next_version,
                template.description,
                template.content,
                variables_json,
                tags_json,
                output_format,
                template.author,
                now as i64,
            ],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "save_context_template".to_string(),
            cause: e.to_string(),
        })?;

        Ok((template.name.clone(), next_version))
    }

    #[allow(clippy::cast_sign_loss, clippy::option_if_let_else)]
    fn get(&self, name: &str, version: Option<u32>) -> Result<Option<ContextTemplate>> {
        let conn = self.lock_conn()?;

        let sql = if version.is_some() {
            "SELECT name, version, description, content, variables, tags, output_format, author, created_at, updated_at
             FROM context_templates WHERE name = ?1 AND version = ?2"
        } else {
            "SELECT name, version, description, content, variables, tags, output_format, author, created_at, updated_at
             FROM context_templates WHERE name = ?1 ORDER BY version DESC LIMIT 1"
        };

        let result = if let Some(v) = version {
            conn.query_row(sql, params![name, v], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            })
        } else {
            conn.query_row(sql, params![name], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            })
        }
        .optional()
        .map_err(|e| Error::OperationFailed {
            operation: "get_context_template".to_string(),
            cause: e.to_string(),
        })?;

        match result {
            Some((
                name,
                version,
                description,
                content,
                variables_json,
                tags_json,
                output_format,
                author,
                created_at,
                updated_at,
            )) => {
                let variables: Vec<TemplateVariable> =
                    serde_json::from_str(&variables_json).unwrap_or_default();
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

                Ok(Some(ContextTemplate {
                    name,
                    version,
                    description,
                    content,
                    variables,
                    tags,
                    output_format: Self::parse_output_format(&output_format),
                    author,
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
    ) -> Result<Vec<ContextTemplate>> {
        let conn = self.lock_conn()?;

        // Get latest version of each template
        let mut sql = String::from(
            "SELECT ct.name, ct.version, ct.description, ct.content, ct.variables, ct.tags,
                    ct.output_format, ct.author, ct.created_at, ct.updated_at
             FROM context_templates ct
             INNER JOIN (
                 SELECT name, MAX(version) as max_version
                 FROM context_templates
                 GROUP BY name
             ) latest ON ct.name = latest.name AND ct.version = latest.max_version
             WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        // Add name pattern filter
        if let Some(pattern) = name_pattern {
            let like_pattern = pattern.replace('*', "%").replace('?', "_");
            sql.push_str(" AND ct.name LIKE ?");
            params_vec.push(Box::new(like_pattern));
        }

        sql.push_str(" ORDER BY ct.name ASC");

        let mut stmt = conn.prepare(&sql).map_err(|e| Error::OperationFailed {
            operation: "prepare_list_context_templates".to_string(),
            cause: e.to_string(),
        })?;

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(AsRef::as_ref).collect();

        let rows = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            })
            .map_err(|e| Error::OperationFailed {
                operation: "list_context_templates".to_string(),
                cause: e.to_string(),
            })?;

        let mut results = Vec::new();
        for row in rows {
            let (
                name,
                version,
                description,
                content,
                variables_json,
                tags_json,
                output_format,
                author,
                created_at,
                updated_at,
            ) = row.map_err(|e| Error::OperationFailed {
                operation: "read_context_template_row".to_string(),
                cause: e.to_string(),
            })?;

            let variables: Vec<TemplateVariable> =
                serde_json::from_str(&variables_json).unwrap_or_default();
            let template_tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

            // Filter by tags if specified
            let has_all_required_tags = tags.is_none_or(|required_tags| {
                required_tags.iter().all(|t| template_tags.contains(t))
            });
            if !has_all_required_tags {
                continue;
            }

            results.push(ContextTemplate {
                name,
                version,
                description,
                content,
                variables,
                tags: template_tags,
                output_format: Self::parse_output_format(&output_format),
                author,
                created_at: created_at as u64,
                updated_at: updated_at as u64,
            });
        }

        Ok(results)
    }

    #[allow(clippy::option_if_let_else)]
    fn delete(&self, name: &str, version: Option<u32>) -> Result<bool> {
        let conn = self.lock_conn()?;

        let rows_affected = if let Some(v) = version {
            conn.execute(
                "DELETE FROM context_templates WHERE name = ?1 AND version = ?2",
                params![name, v],
            )
        } else {
            conn.execute(
                "DELETE FROM context_templates WHERE name = ?1",
                params![name],
            )
        }
        .map_err(|e| Error::OperationFailed {
            operation: "delete_context_template".to_string(),
            cause: e.to_string(),
        })?;

        Ok(rows_affected > 0)
    }

    fn get_versions(&self, name: &str) -> Result<Vec<u32>> {
        let conn = self.lock_conn()?;

        let mut stmt = conn
            .prepare("SELECT version FROM context_templates WHERE name = ?1 ORDER BY version DESC")
            .map_err(|e| Error::OperationFailed {
                operation: "prepare_get_versions".to_string(),
                cause: e.to_string(),
            })?;

        let versions = stmt
            .query_map(params![name], |row| row.get::<_, u32>(0))
            .map_err(|e| Error::OperationFailed {
                operation: "get_versions".to_string(),
                cause: e.to_string(),
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::OperationFailed {
                operation: "collect_versions".to_string(),
                cause: e.to_string(),
            })?;

        Ok(versions)
    }

    fn get_latest_version(&self, name: &str) -> Result<Option<u32>> {
        let conn = self.lock_conn()?;

        conn.query_row(
            "SELECT MAX(version) FROM context_templates WHERE name = ?1",
            params![name],
            |row| row.get::<_, Option<u32>>(0),
        )
        .map_err(|e| Error::OperationFailed {
            operation: "get_latest_version".to_string(),
            cause: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::VariableType;

    fn create_test_template(name: &str, content: &str) -> ContextTemplate {
        ContextTemplate {
            name: name.to_string(),
            version: 0, // Will be set by storage
            description: format!("Test template: {name}"),
            content: content.to_string(),
            variables: vec![],
            tags: vec![],
            output_format: OutputFormat::Markdown,
            author: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn test_sqlite_context_template_storage_creation() {
        let storage = SqliteContextTemplateStorage::in_memory().unwrap();
        assert_eq!(storage.db_path().to_str(), Some(":memory:"));
    }

    #[test]
    fn test_save_and_get_template() {
        let storage = SqliteContextTemplateStorage::in_memory().unwrap();

        let template = create_test_template("test-template", "Hello {{name}}!");

        let (name, version) = storage.save(&template).unwrap();
        assert_eq!(name, "test-template");
        assert_eq!(version, 1);

        let retrieved = storage.get("test-template", None).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.name, "test-template");
        assert_eq!(retrieved.version, 1);
        assert_eq!(retrieved.content, "Hello {{name}}!");
    }

    #[test]
    fn test_auto_increment_version() {
        let storage = SqliteContextTemplateStorage::in_memory().unwrap();

        // Save first version
        let template = create_test_template("versioned", "Version 1");
        let (_, v1) = storage.save(&template).unwrap();
        assert_eq!(v1, 1);

        // Save second version
        let template2 = create_test_template("versioned", "Version 2");
        let (_, v2) = storage.save(&template2).unwrap();
        assert_eq!(v2, 2);

        // Save third version
        let template3 = create_test_template("versioned", "Version 3");
        let (_, v3) = storage.save(&template3).unwrap();
        assert_eq!(v3, 3);

        // Get latest should return version 3
        let latest = storage.get("versioned", None).unwrap().unwrap();
        assert_eq!(latest.version, 3);
        assert_eq!(latest.content, "Version 3");

        // Get specific version should work
        let v1_retrieved = storage.get("versioned", Some(1)).unwrap().unwrap();
        assert_eq!(v1_retrieved.version, 1);
        assert_eq!(v1_retrieved.content, "Version 1");
    }

    #[test]
    fn test_get_versions() {
        let storage = SqliteContextTemplateStorage::in_memory().unwrap();

        // Save multiple versions
        for i in 1..=5 {
            let template = create_test_template("multi-version", &format!("Content {i}"));
            storage.save(&template).unwrap();
        }

        let versions = storage.get_versions("multi-version").unwrap();
        assert_eq!(versions, vec![5, 4, 3, 2, 1]);
    }

    #[test]
    fn test_get_latest_version() {
        let storage = SqliteContextTemplateStorage::in_memory().unwrap();

        // No template exists
        assert_eq!(storage.get_latest_version("nonexistent").unwrap(), None);

        // Save some versions
        for _ in 0..3 {
            let template = create_test_template("latest-test", "Content");
            storage.save(&template).unwrap();
        }

        assert_eq!(storage.get_latest_version("latest-test").unwrap(), Some(3));
    }

    #[test]
    fn test_list_templates() {
        let storage = SqliteContextTemplateStorage::in_memory().unwrap();

        // Save templates with multiple versions
        for i in 1..=3 {
            let template = create_test_template("alpha", &format!("Alpha v{i}"));
            storage.save(&template).unwrap();
        }

        let mut beta = create_test_template("beta", "Beta content");
        beta.tags = vec!["tag1".to_string(), "tag2".to_string()];
        storage.save(&beta).unwrap();

        let gamma = create_test_template("gamma", "Gamma content");
        storage.save(&gamma).unwrap();

        // List all - should return latest versions only
        let all = storage.list(None, None).unwrap();
        assert_eq!(all.len(), 3);

        // Verify we got the latest version of alpha
        let alpha = all.iter().find(|t| t.name == "alpha").unwrap();
        assert_eq!(alpha.version, 3);

        // Filter by tag
        let with_tag1 = storage.list(Some(&["tag1".to_string()]), None).unwrap();
        assert_eq!(with_tag1.len(), 1);
        assert_eq!(with_tag1[0].name, "beta");

        // Filter by name pattern
        let alpha_pattern = storage.list(None, Some("a*")).unwrap();
        assert_eq!(alpha_pattern.len(), 1);
        assert_eq!(alpha_pattern[0].name, "alpha");
    }

    #[test]
    fn test_delete_specific_version() {
        let storage = SqliteContextTemplateStorage::in_memory().unwrap();

        // Save multiple versions
        for _ in 0..3 {
            let template = create_test_template("to-delete", "Content");
            storage.save(&template).unwrap();
        }

        // Delete version 2
        assert!(storage.delete("to-delete", Some(2)).unwrap());

        // Version 2 should be gone
        assert!(storage.get("to-delete", Some(2)).unwrap().is_none());

        // Versions 1 and 3 should still exist
        assert!(storage.get("to-delete", Some(1)).unwrap().is_some());
        assert!(storage.get("to-delete", Some(3)).unwrap().is_some());

        // Latest should still be version 3
        let latest = storage.get("to-delete", None).unwrap().unwrap();
        assert_eq!(latest.version, 3);
    }

    #[test]
    fn test_delete_all_versions() {
        let storage = SqliteContextTemplateStorage::in_memory().unwrap();

        // Save multiple versions
        for _ in 0..3 {
            let template = create_test_template("delete-all", "Content");
            storage.save(&template).unwrap();
        }

        // Delete all versions
        assert!(storage.delete("delete-all", None).unwrap());

        // All versions should be gone
        assert!(storage.get("delete-all", None).unwrap().is_none());
        assert!(storage.get_versions("delete-all").unwrap().is_empty());
    }

    #[test]
    fn test_template_with_variables() {
        let storage = SqliteContextTemplateStorage::in_memory().unwrap();

        let template = ContextTemplate {
            name: "with-vars".to_string(),
            version: 0,
            description: "Template with variables".to_string(),
            content: "Hello {{name}}, you have {{count}} items".to_string(),
            variables: vec![
                TemplateVariable {
                    name: "name".to_string(),
                    var_type: VariableType::User,
                    description: Some("User's name".to_string()),
                    default: None,
                    required: true,
                },
                TemplateVariable {
                    name: "count".to_string(),
                    var_type: VariableType::User,
                    description: None,
                    default: Some("0".to_string()),
                    required: false,
                },
            ],
            tags: vec!["greeting".to_string()],
            output_format: OutputFormat::Markdown,
            author: Some("test-author".to_string()),
            created_at: 0,
            updated_at: 0,
        };

        storage.save(&template).unwrap();

        let retrieved = storage.get("with-vars", None).unwrap().unwrap();
        assert_eq!(retrieved.variables.len(), 2);
        assert_eq!(retrieved.variables[0].name, "name");
        assert!(retrieved.variables[0].required);
        assert_eq!(retrieved.author, Some("test-author".to_string()));
    }

    #[test]
    fn test_output_format_persistence() {
        let storage = SqliteContextTemplateStorage::in_memory().unwrap();

        // Save with JSON format
        let mut template = create_test_template("json-format", "{{data}}");
        template.output_format = OutputFormat::Json;
        storage.save(&template).unwrap();

        let retrieved = storage.get("json-format", None).unwrap().unwrap();
        assert!(matches!(retrieved.output_format, OutputFormat::Json));

        // Save with XML format
        let mut template2 = create_test_template("xml-format", "{{data}}");
        template2.output_format = OutputFormat::Xml;
        storage.save(&template2).unwrap();

        let retrieved2 = storage.get("xml-format", None).unwrap().unwrap();
        assert!(matches!(retrieved2.output_format, OutputFormat::Xml));
    }

    #[test]
    fn test_stats() {
        let storage = SqliteContextTemplateStorage::in_memory().unwrap();

        // Initially empty
        let stats = storage.stats().unwrap();
        assert_eq!(stats.template_count, 0);
        assert_eq!(stats.version_count, 0);

        // Add some templates with versions
        for _ in 0..3 {
            let template = create_test_template("stats-test-1", "Content");
            storage.save(&template).unwrap();
        }
        storage
            .save(&create_test_template("stats-test-2", "Content"))
            .unwrap();

        let stats = storage.stats().unwrap();
        assert_eq!(stats.template_count, 2); // 2 unique templates
        assert_eq!(stats.version_count, 4); // 4 total versions
        assert!(stats.db_size_bytes > 0);
    }

    #[test]
    fn test_vacuum_and_analyze() {
        let storage = SqliteContextTemplateStorage::in_memory().unwrap();

        // Add and delete some templates
        for i in 0..10 {
            storage
                .save(&create_test_template(&format!("temp-{i}"), "Content"))
                .unwrap();
        }
        for i in 0..10 {
            storage.delete(&format!("temp-{i}"), None).unwrap();
        }

        assert!(storage.vacuum_and_analyze().is_ok());
    }

    #[test]
    fn test_default_user_path() {
        let path = SqliteContextTemplateStorage::default_user_path();
        if let Some(p) = path {
            assert!(p.to_string_lossy().contains("subcog"));
            assert!(p.to_string_lossy().ends_with("memories.db"));
        }
    }

    #[test]
    fn test_nonexistent_template() {
        let storage = SqliteContextTemplateStorage::in_memory().unwrap();

        assert!(storage.get("nonexistent", None).unwrap().is_none());
        assert!(storage.get("nonexistent", Some(1)).unwrap().is_none());
        assert!(!storage.delete("nonexistent", None).unwrap());
    }
}
