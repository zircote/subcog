//! PostgreSQL-based prompt storage.
//!
//! Stores prompts in PostgreSQL with full-text search support.
//! Includes embedded migrations that auto-upgrade the schema on startup.

#[cfg(feature = "postgres")]
#[allow(clippy::excessive_nesting)]
mod implementation {
    use crate::models::PromptTemplate;
    use crate::storage::prompt::PromptStorage;
    use crate::{Error, Result};
    use deadpool_postgres::{Config, ManagerConfig, Pool, RecyclingMethod, Runtime};
    use std::sync::Arc;
    use tokio::runtime::Runtime as TokioRuntime;
    use tokio_postgres::NoTls;

    /// A single migration with version and SQL.
    struct Migration {
        /// Migration version (sequential, starting at 1).
        version: i32,
        /// Human-readable description.
        description: &'static str,
        /// SQL to apply (may contain multiple statements separated by semicolons).
        sql: &'static str,
    }

    /// Embedded migrations compiled into the binary.
    /// Add new migrations here as the schema evolves.
    const MIGRATIONS: &[Migration] = &[
        Migration {
            version: 1,
            description: "Initial prompts table",
            sql: r"
                CREATE TABLE IF NOT EXISTS {table} (
                    name TEXT PRIMARY KEY,
                    description TEXT NOT NULL DEFAULT '',
                    content TEXT NOT NULL,
                    variables JSONB NOT NULL DEFAULT '[]'::jsonb,
                    tags TEXT[] NOT NULL DEFAULT '{}',
                    author TEXT,
                    usage_count BIGINT NOT NULL DEFAULT 0,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_{table}_tags ON {table} USING GIN(tags);
            ",
        },
        Migration {
            version: 2,
            description: "Add full-text search index",
            sql: r"
                CREATE INDEX IF NOT EXISTS idx_{table}_content_fts
                ON {table} USING GIN(to_tsvector('english', content));
                CREATE INDEX IF NOT EXISTS idx_{table}_description_fts
                ON {table} USING GIN(to_tsvector('english', description));
            ",
        },
        // Add future migrations here:
        // Migration {
        //     version: 3,
        //     description: "Add category column",
        //     sql: r"ALTER TABLE {table} ADD COLUMN IF NOT EXISTS category TEXT;",
        // },
    ];

    /// PostgreSQL-based prompt storage with auto-migration support.
    pub struct PostgresPromptStorage {
        /// Connection pool.
        pool: Pool,
        /// Table name for prompts.
        table_name: String,
        /// Tokio runtime for blocking operations.
        runtime: Arc<TokioRuntime>,
    }

    impl PostgresPromptStorage {
        /// Creates a new PostgreSQL prompt storage.
        ///
        /// Automatically runs any pending migrations on startup.
        ///
        /// # Arguments
        ///
        /// * `connection_url` - PostgreSQL connection URL
        /// * `table_name` - Table name for prompts
        ///
        /// # Errors
        ///
        /// Returns an error if the connection pool cannot be created or migrations fail.
        pub fn new(connection_url: &str, table_name: impl Into<String>) -> Result<Self> {
            let table_name = table_name.into();

            // Create tokio runtime for blocking
            let runtime = TokioRuntime::new().map_err(|e| Error::OperationFailed {
                operation: "create_tokio_runtime".to_string(),
                cause: e.to_string(),
            })?;

            // Parse connection URL and create pool config
            let mut cfg = Config::new();
            cfg.url = Some(connection_url.to_string());
            cfg.manager = Some(ManagerConfig {
                recycling_method: RecyclingMethod::Fast,
            });

            let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).map_err(|e| {
                Error::OperationFailed {
                    operation: "create_postgres_pool".to_string(),
                    cause: e.to_string(),
                }
            })?;

            let storage = Self {
                pool,
                table_name,
                runtime: Arc::new(runtime),
            };

            // Run migrations
            storage.run_migrations()?;

            Ok(storage)
        }

        /// Creates a storage with default settings.
        ///
        /// # Errors
        ///
        /// Returns an error if the connection cannot be established.
        pub fn with_defaults() -> Result<Self> {
            Self::new("postgresql://localhost/subcog", "prompts")
        }

        /// Returns the table name.
        #[must_use]
        pub fn table_name(&self) -> &str {
            &self.table_name
        }

        /// Returns the current schema version.
        ///
        /// # Errors
        ///
        /// Returns an error if the database cannot be queried.
        pub fn schema_version(&self) -> Result<i32> {
            self.runtime.block_on(async {
                let client = self.pool.get().await.map_err(|e| Error::OperationFailed {
                    operation: "get_postgres_connection".to_string(),
                    cause: e.to_string(),
                })?;

                self.get_current_version(&client).await
            })
        }

        /// Runs all pending migrations.
        fn run_migrations(&self) -> Result<()> {
            let table_name = self.table_name.clone();
            self.runtime.block_on(async {
                let client = self.pool.get().await.map_err(|e| Error::OperationFailed {
                    operation: "get_postgres_connection".to_string(),
                    cause: e.to_string(),
                })?;

                // Ensure migrations table exists
                self.ensure_migrations_table(&client).await?;

                // Get current version
                let current_version = self.get_current_version(&client).await?;

                // Apply pending migrations
                for migration in MIGRATIONS {
                    if migration.version > current_version {
                        self.apply_migration(&client, migration, &table_name)
                            .await?;
                    }
                }

                Ok(())
            })
        }

        /// Ensures the `schema_migrations` table exists.
        async fn ensure_migrations_table(&self, client: &deadpool_postgres::Object) -> Result<()> {
            let table_name = &self.table_name;
            let migrations_table = format!("{table_name}_schema_migrations");

            let sql = format!(
                r"
                CREATE TABLE IF NOT EXISTS {migrations_table} (
                    version INTEGER PRIMARY KEY,
                    description TEXT NOT NULL,
                    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                )
                "
            );

            client
                .execute(&sql, &[])
                .await
                .map_err(|e| Error::OperationFailed {
                    operation: "create_migrations_table".to_string(),
                    cause: e.to_string(),
                })?;

            Ok(())
        }

        /// Gets the current schema version.
        async fn get_current_version(&self, client: &deadpool_postgres::Object) -> Result<i32> {
            let table_name = &self.table_name;
            let migrations_table = format!("{table_name}_schema_migrations");

            // Check if migrations table exists
            let exists_sql = r"
                SELECT EXISTS (
                    SELECT FROM information_schema.tables
                    WHERE table_name = $1
                )
            ";

            let exists: bool = client
                .query_one(exists_sql, &[&migrations_table])
                .await
                .map(|row| row.get(0))
                .unwrap_or(false);

            if !exists {
                return Ok(0);
            }

            let sql = format!("SELECT COALESCE(MAX(version), 0) FROM {migrations_table}");

            let version: i32 = client
                .query_one(&sql, &[])
                .await
                .map(|row| row.get(0))
                .unwrap_or(0);

            Ok(version)
        }

        /// Applies a single migration.
        async fn apply_migration(
            &self,
            client: &deadpool_postgres::Object,
            migration: &Migration,
            table_name: &str,
        ) -> Result<()> {
            let migrations_table = format!("{table_name}_schema_migrations");

            // Replace {table} placeholder with actual table name
            let sql = migration.sql.replace("{table}", table_name);

            // Split by semicolons and execute each statement
            for statement in sql.split(';') {
                let statement = statement.trim();
                if statement.is_empty() {
                    continue;
                }

                client
                    .execute(statement, &[])
                    .await
                    .map_err(|e| Error::OperationFailed {
                        operation: format!(
                            "migration_v{}: {}",
                            migration.version, migration.description
                        ),
                        cause: e.to_string(),
                    })?;
            }

            // Record the migration
            let record_sql =
                format!("INSERT INTO {migrations_table} (version, description) VALUES ($1, $2)");

            client
                .execute(&record_sql, &[&migration.version, &migration.description])
                .await
                .map_err(|e| Error::OperationFailed {
                    operation: "record_migration".to_string(),
                    cause: e.to_string(),
                })?;

            tracing::info!(
                version = migration.version,
                description = migration.description,
                table = table_name,
                "Applied migration"
            );

            Ok(())
        }

        /// Gets current Unix timestamp.
        #[allow(clippy::cast_possible_wrap)]
        fn current_timestamp() -> i64 {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        }
    }

    #[allow(clippy::cast_sign_loss)]
    impl PromptStorage for PostgresPromptStorage {
        fn save(&self, template: &PromptTemplate) -> Result<String> {
            let table_name = self.table_name.clone();
            let name = template.name.clone();
            let description = template.description.clone();
            let content = template.content.clone();
            let variables = serde_json::to_value(&template.variables).unwrap_or_default();
            let tags: Vec<String> = template.tags.clone();
            let author = template.author.clone();
            let now = Self::current_timestamp();

            self.runtime.block_on(async {
                let client = self.pool.get().await.map_err(|e| Error::OperationFailed {
                    operation: "get_postgres_connection".to_string(),
                    cause: e.to_string(),
                })?;

                // Upsert: insert or update on conflict
                let query = format!(
                    r"
                    INSERT INTO {table_name} (name, description, content, variables, tags, author, usage_count, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, $5, $6, 0, $7, $7)
                    ON CONFLICT (name) DO UPDATE SET
                        description = EXCLUDED.description,
                        content = EXCLUDED.content,
                        variables = EXCLUDED.variables,
                        tags = EXCLUDED.tags,
                        author = EXCLUDED.author,
                        updated_at = EXCLUDED.updated_at
                    "
                );

                client
                    .execute(
                        &query,
                        &[&name, &description, &content, &variables, &tags, &author, &now],
                    )
                    .await
                    .map_err(|e| Error::OperationFailed {
                        operation: "save_prompt".to_string(),
                        cause: e.to_string(),
                    })?;

                Ok(format!("prompt_pg_{name}"))
            })
        }

        fn get(&self, name: &str) -> Result<Option<PromptTemplate>> {
            let table_name = self.table_name.clone();
            let name = name.to_string();

            self.runtime.block_on(async {
                let client = self.pool.get().await.map_err(|e| Error::OperationFailed {
                    operation: "get_postgres_connection".to_string(),
                    cause: e.to_string(),
                })?;

                let query = format!(
                    r"
                    SELECT name, description, content, variables, tags, author, usage_count, created_at, updated_at
                    FROM {table_name}
                    WHERE name = $1
                    "
                );

                let row = client
                    .query_opt(&query, &[&name])
                    .await
                    .map_err(|e| Error::OperationFailed {
                        operation: "get_prompt".to_string(),
                        cause: e.to_string(),
                    })?;

                row.map_or(Ok(None), |row| {
                    let variables_json: serde_json::Value = row.get("variables");
                    let variables = serde_json::from_value(variables_json).unwrap_or_default();
                    let tags: Vec<String> = row.get("tags");
                    let usage_count: i64 = row.get("usage_count");
                    let created_at: i64 = row.get("created_at");
                    let updated_at: i64 = row.get("updated_at");

                    Ok(Some(PromptTemplate {
                        name: row.get("name"),
                        description: row.get("description"),
                        content: row.get("content"),
                        variables,
                        tags,
                        author: row.get("author"),
                        usage_count: usage_count as u64,
                        created_at: created_at as u64,
                        updated_at: updated_at as u64,
                    }))
                })
            })
        }

        fn list(
            &self,
            tags: Option<&[String]>,
            name_pattern: Option<&str>,
        ) -> Result<Vec<PromptTemplate>> {
            let table_name = self.table_name.clone();
            let tags_filter = tags.map(<[String]>::to_vec);
            let pattern = name_pattern.map(|p| {
                // Convert glob to SQL LIKE pattern
                p.replace('*', "%").replace('?', "_")
            });

            self.runtime.block_on(async {
                let client = self.pool.get().await.map_err(|e| Error::OperationFailed {
                    operation: "get_postgres_connection".to_string(),
                    cause: e.to_string(),
                })?;

                // Build dynamic query
                let mut conditions = Vec::new();
                let mut param_idx = 1;

                if tags_filter.is_some() {
                    conditions.push(format!("tags @> ${param_idx}"));
                    param_idx += 1;
                }

                if pattern.is_some() {
                    conditions.push(format!("name LIKE ${param_idx}"));
                }

                let where_clause = if conditions.is_empty() {
                    String::new()
                } else {
                    format!("WHERE {}", conditions.join(" AND "))
                };

                let query = format!(
                    r"
                    SELECT name, description, content, variables, tags, author, usage_count, created_at, updated_at
                    FROM {table_name}
                    {where_clause}
                    ORDER BY usage_count DESC, name ASC
                    "
                );

                // Build params dynamically
                let rows = match (&tags_filter, &pattern) {
                    (Some(t), Some(p)) => {
                        client
                            .query(&query, &[t, p])
                            .await
                            .map_err(|e| Error::OperationFailed {
                                operation: "list_prompts".to_string(),
                                cause: e.to_string(),
                            })?
                    }
                    (Some(t), None) => {
                        client
                            .query(&query, &[t])
                            .await
                            .map_err(|e| Error::OperationFailed {
                                operation: "list_prompts".to_string(),
                                cause: e.to_string(),
                            })?
                    }
                    (None, Some(p)) => {
                        client
                            .query(&query, &[p])
                            .await
                            .map_err(|e| Error::OperationFailed {
                                operation: "list_prompts".to_string(),
                                cause: e.to_string(),
                            })?
                    }
                    (None, None) => {
                        client
                            .query(&query, &[])
                            .await
                            .map_err(|e| Error::OperationFailed {
                                operation: "list_prompts".to_string(),
                                cause: e.to_string(),
                            })?
                    }
                };

                let mut results = Vec::new();
                for row in rows {
                    let variables_json: serde_json::Value = row.get("variables");
                    let variables = serde_json::from_value(variables_json).unwrap_or_default();
                    let tags: Vec<String> = row.get("tags");
                    let usage_count: i64 = row.get("usage_count");
                    let created_at: i64 = row.get("created_at");
                    let updated_at: i64 = row.get("updated_at");

                    results.push(PromptTemplate {
                        name: row.get("name"),
                        description: row.get("description"),
                        content: row.get("content"),
                        variables,
                        tags,
                        author: row.get("author"),
                        usage_count: usage_count as u64,
                        created_at: created_at as u64,
                        updated_at: updated_at as u64,
                    });
                }

                Ok(results)
            })
        }

        fn delete(&self, name: &str) -> Result<bool> {
            let table_name = self.table_name.clone();
            let name = name.to_string();

            self.runtime.block_on(async {
                let client = self.pool.get().await.map_err(|e| Error::OperationFailed {
                    operation: "get_postgres_connection".to_string(),
                    cause: e.to_string(),
                })?;

                let query = format!("DELETE FROM {table_name} WHERE name = $1");

                let rows_affected =
                    client
                        .execute(&query, &[&name])
                        .await
                        .map_err(|e| Error::OperationFailed {
                            operation: "delete_prompt".to_string(),
                            cause: e.to_string(),
                        })?;

                Ok(rows_affected > 0)
            })
        }

        fn increment_usage(&self, name: &str) -> Result<u64> {
            let table_name = self.table_name.clone();
            let name = name.to_string();
            let now = Self::current_timestamp();

            self.runtime.block_on(async {
                let client = self.pool.get().await.map_err(|e| Error::OperationFailed {
                    operation: "get_postgres_connection".to_string(),
                    cause: e.to_string(),
                })?;

                let query = format!(
                    r"
                    UPDATE {table_name}
                    SET usage_count = usage_count + 1, updated_at = $2
                    WHERE name = $1
                    RETURNING usage_count
                    "
                );

                let row = client
                    .query_one(&query, &[&name, &now])
                    .await
                    .map_err(|e| Error::OperationFailed {
                        operation: "increment_usage".to_string(),
                        cause: e.to_string(),
                    })?;

                let count: i64 = row.get("usage_count");
                Ok(count as u64)
            })
        }
    }
}

#[cfg(feature = "postgres")]
pub use implementation::PostgresPromptStorage;

#[cfg(not(feature = "postgres"))]
mod stub {
    use crate::models::PromptTemplate;
    use crate::storage::prompt::PromptStorage;
    use crate::{Error, Result};

    /// Stub PostgreSQL prompt storage when feature is not enabled.
    pub struct PostgresPromptStorage {
        connection_url: String,
        table_name: String,
    }

    impl PostgresPromptStorage {
        /// Creates a new PostgreSQL prompt storage (stub).
        ///
        /// # Errors
        ///
        /// Always returns an error because the feature is not enabled.
        pub fn new(
            connection_url: impl Into<String>,
            table_name: impl Into<String>,
        ) -> Result<Self> {
            // Return the struct for API compatibility, but operations will fail
            Ok(Self {
                connection_url: connection_url.into(),
                table_name: table_name.into(),
            })
        }

        /// Creates a storage with default settings (stub).
        ///
        /// # Errors
        ///
        /// Always returns an error because the feature is not enabled.
        pub fn with_defaults() -> Result<Self> {
            Self::new("postgresql://localhost/subcog", "prompts")
        }

        /// Returns the table name.
        #[must_use]
        pub fn table_name(&self) -> &str {
            &self.table_name
        }

        /// Returns the connection URL.
        #[must_use]
        pub fn connection_url(&self) -> &str {
            &self.connection_url
        }

        /// Returns the current schema version (stub always returns 0).
        ///
        /// # Errors
        ///
        /// Always returns an error because the feature is not enabled.
        pub fn schema_version(&self) -> Result<i32> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }
    }

    impl PromptStorage for PostgresPromptStorage {
        fn save(&self, _template: &PromptTemplate) -> Result<String> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }

        fn get(&self, _name: &str) -> Result<Option<PromptTemplate>> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }

        fn list(
            &self,
            _tags: Option<&[String]>,
            _name_pattern: Option<&str>,
        ) -> Result<Vec<PromptTemplate>> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }

        fn delete(&self, _name: &str) -> Result<bool> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }

        fn increment_usage(&self, _name: &str) -> Result<u64> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }
    }
}

#[cfg(not(feature = "postgres"))]
pub use stub::PostgresPromptStorage;

#[cfg(all(test, not(feature = "postgres")))]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::Error;
    use crate::models::PromptTemplate;
    use crate::storage::prompt::PromptStorage;

    #[test]
    fn test_postgres_prompt_storage_creation() {
        let storage = PostgresPromptStorage::new("postgresql://localhost/test", "test_prompts");
        assert!(storage.is_ok());
        let storage = storage.unwrap();
        assert_eq!(storage.table_name(), "test_prompts");
    }

    #[test]
    fn test_postgres_prompt_storage_defaults() {
        let storage = PostgresPromptStorage::with_defaults();
        assert!(storage.is_ok());
        let storage = storage.unwrap();
        assert_eq!(storage.table_name(), "prompts");
    }

    #[test]
    fn test_postgres_prompt_storage_feature_not_enabled() {
        let storage = PostgresPromptStorage::with_defaults().unwrap();

        // All operations should return FeatureNotEnabled
        assert!(matches!(
            storage.save(&PromptTemplate::new("test", "content")),
            Err(Error::FeatureNotEnabled(_))
        ));

        assert!(matches!(
            storage.get("test"),
            Err(Error::FeatureNotEnabled(_))
        ));

        assert!(matches!(
            storage.list(None, None),
            Err(Error::FeatureNotEnabled(_))
        ));

        assert!(matches!(
            storage.delete("test"),
            Err(Error::FeatureNotEnabled(_))
        ));

        assert!(matches!(
            storage.increment_usage("test"),
            Err(Error::FeatureNotEnabled(_))
        ));
    }

    #[test]
    fn test_postgres_schema_version_stub() {
        let storage = PostgresPromptStorage::with_defaults().unwrap();
        assert!(matches!(
            storage.schema_version(),
            Err(Error::FeatureNotEnabled(_))
        ));
    }
}
