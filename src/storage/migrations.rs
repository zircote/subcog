//! PostgreSQL migration system for schema management.
//!
//! Provides a compile-time embedded migration system that automatically
//! upgrades database schemas when the application starts.
//!
//! # Usage
//!
//! ```rust,ignore
//! use subcog::storage::migrations::{Migration, MigrationRunner};
//!
//! const MIGRATIONS: &[Migration] = &[
//!     Migration {
//!         version: 1,
//!         description: "Initial table",
//!         sql: "CREATE TABLE IF NOT EXISTS {table} (id SERIAL PRIMARY KEY);",
//!     },
//! ];
//!
//! let runner = MigrationRunner::new(pool, "my_table");
//! runner.run(MIGRATIONS).await?;
//! ```

#[cfg(feature = "postgres")]
#[allow(clippy::excessive_nesting)]
mod implementation {
    use crate::{Error, Result};
    use deadpool_postgres::Pool;

    /// A single migration with version and SQL.
    #[derive(Debug, Clone, Copy)]
    pub struct Migration {
        /// Migration version (sequential, starting at 1).
        pub version: i32,
        /// Human-readable description.
        pub description: &'static str,
        /// SQL to apply (may contain multiple statements separated by semicolons).
        /// Use `{table}` as a placeholder for the table name.
        pub sql: &'static str,
    }

    /// Runs migrations for a PostgreSQL table.
    pub struct MigrationRunner {
        pool: Pool,
        table_name: String,
        /// Additional placeholder replacements applied to migration SQL.
        /// Each entry is `("{placeholder}", "replacement_value")`.
        extra_replacements: Vec<(String, String)>,
    }

    impl MigrationRunner {
        /// Creates a new migration runner.
        #[must_use]
        pub fn new(pool: Pool, table_name: impl Into<String>) -> Self {
            Self {
                pool,
                table_name: table_name.into(),
                extra_replacements: Vec::new(),
            }
        }

        /// Registers an additional placeholder replacement for migration SQL.
        ///
        /// The `{table}` placeholder is always replaced with the table name.
        /// Use this for additional placeholders like `{vector_table}`.
        #[must_use]
        pub fn with_replacement(
            mut self,
            placeholder: impl Into<String>,
            value: impl Into<String>,
        ) -> Self {
            self.extra_replacements
                .push((placeholder.into(), value.into()));
            self
        }

        /// Returns the table name.
        #[must_use]
        pub fn table_name(&self) -> &str {
            &self.table_name
        }

        /// Runs all pending migrations.
        ///
        /// # Errors
        ///
        /// Returns an error if a migration fails.
        pub async fn run(&self, migrations: &[Migration]) -> Result<()> {
            let mut client = self.pool.get().await.map_err(|e| Error::OperationFailed {
                operation: "migration_get_connection".to_string(),
                cause: format!("{e:?}"),
            })?;

            // Acquire advisory lock to prevent concurrent migration runs.
            // hashtext('subcog_migrations') produces a stable int4 lock key.
            client
                .execute(
                    "SELECT pg_advisory_lock(hashtext('subcog_migrations'))",
                    &[],
                )
                .await
                .map_err(|e| Error::OperationFailed {
                    operation: "migration_advisory_lock".to_string(),
                    cause: e.to_string(),
                })?;

            let result = self.run_inner(&mut client, migrations).await;

            // Always release the advisory lock, even on failure
            let _ = client
                .execute(
                    "SELECT pg_advisory_unlock(hashtext('subcog_migrations'))",
                    &[],
                )
                .await;

            result
        }

        /// Inner migration logic, called while holding the advisory lock.
        async fn run_inner(
            &self,
            client: &mut deadpool_postgres::Object,
            migrations: &[Migration],
        ) -> Result<()> {
            // Ensure migrations tracking table exists
            self.ensure_migrations_table(client).await?;

            // Get current version
            let current_version = self.get_current_version(client).await?;

            // Apply pending migrations
            for migration in migrations {
                if migration.version > current_version {
                    self.apply_migration(client, migration).await?;
                }
            }

            Ok(())
        }

        /// Returns the current schema version.
        ///
        /// # Errors
        ///
        /// Returns an error if the database cannot be queried.
        pub async fn current_version(&self) -> Result<i32> {
            let client = self.pool.get().await.map_err(|e| Error::OperationFailed {
                operation: "migration_get_connection".to_string(),
                cause: e.to_string(),
            })?;

            // Check if migrations table exists first
            let migrations_table = self.migrations_table_name();
            let exists = self.table_exists(&client, migrations_table).await?;

            if !exists {
                return Ok(0);
            }

            self.get_current_version(&client).await
        }

        /// Returns the name of the migrations tracking table.
        const fn migrations_table_name(&self) -> &'static str {
            "migrations"
        }

        /// Ensures the `schema_migrations` table exists.
        async fn ensure_migrations_table(&self, client: &deadpool_postgres::Object) -> Result<()> {
            let migrations_table = self.migrations_table_name();

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

        /// Checks if a table exists.
        async fn table_exists(
            &self,
            client: &deadpool_postgres::Object,
            table_name: &str,
        ) -> Result<bool> {
            let sql = r"
                SELECT EXISTS (
                    SELECT FROM information_schema.tables
                    WHERE table_schema = 'public' AND table_name = $1
                )
            ";

            let exists: bool = client
                .query_one(sql, &[&table_name])
                .await
                .map(|row| row.get(0))
                .unwrap_or(false);

            Ok(exists)
        }

        /// Gets the current schema version.
        async fn get_current_version(&self, client: &deadpool_postgres::Object) -> Result<i32> {
            let migrations_table = self.migrations_table_name();
            let sql = format!("SELECT COALESCE(MAX(version), 0) FROM {migrations_table}");

            let version: i32 = client
                .query_one(&sql, &[])
                .await
                .map(|row| row.get(0))
                .unwrap_or(0);

            Ok(version)
        }

        /// Applies a single migration within a transaction.
        ///
        /// # Transaction Safety (CRIT-001)
        ///
        /// All migration statements and the version record are executed within
        /// a single transaction. If any statement fails, the entire migration
        /// is rolled back, preventing partial schema updates that could leave
        /// the database in an inconsistent state.
        async fn apply_migration(
            &self,
            client: &mut deadpool_postgres::Object,
            migration: &Migration,
        ) -> Result<()> {
            let migrations_table = self.migrations_table_name();

            // Replace {table} placeholder with actual table name
            let mut sql = migration.sql.replace("{table}", &self.table_name);

            // Apply any additional placeholder replacements
            for (placeholder, value) in &self.extra_replacements {
                sql = sql.replace(placeholder, value);
            }

            // Start transaction for atomic migration application
            let tx = client
                .transaction()
                .await
                .map_err(|e| Error::OperationFailed {
                    operation: format!("migration_v{}_begin_tx", migration.version),
                    cause: e.to_string(),
                })?;

            // Execute all statements within the transaction.
            // Uses dollar-quote-aware splitting so PL/pgSQL function
            // bodies containing semicolons are not split incorrectly.
            for statement in split_sql_statements(&sql) {
                tx.execute(statement.as_str(), &[]).await.map_err(|e| {
                    // tokio_postgres::Error Display impl only shows "db error"
                    // for database errors — use Debug format to include the
                    // actual message, detail, and hint from PostgreSQL.
                    let cause = e.as_db_error().map_or_else(
                        || e.to_string(),
                        |db_err| {
                            format!(
                                "{}: {} (detail: {:?}, hint: {:?})",
                                db_err.severity(),
                                db_err.message(),
                                db_err.detail(),
                                db_err.hint(),
                            )
                        },
                    );
                    Error::OperationFailed {
                        operation: format!(
                            "migration_v{}: {}",
                            migration.version, migration.description
                        ),
                        cause,
                    }
                })?;
            }

            // Record the migration within the same transaction
            let record_sql =
                format!("INSERT INTO {migrations_table} (version, description) VALUES ($1, $2)");

            tx.execute(&record_sql, &[&migration.version, &migration.description])
                .await
                .map_err(|e| Error::OperationFailed {
                    operation: "record_migration".to_string(),
                    cause: e.to_string(),
                })?;

            // Commit the transaction - all statements succeed or none do
            tx.commit().await.map_err(|e| Error::OperationFailed {
                operation: format!("migration_v{}_commit", migration.version),
                cause: e.to_string(),
            })?;

            tracing::info!(
                version = migration.version,
                description = migration.description,
                table = self.table_name,
                "Applied migration"
            );

            Ok(())
        }
    }

    /// Splits SQL text into individual statements, respecting dollar-quoted blocks.
    ///
    /// PL/pgSQL function bodies use dollar-quote delimiters (`$$`, `$body$`, `$func$`)
    /// and contain semicolons that should NOT be treated as statement separators.
    /// This function tracks whether we're inside a dollar-quoted block and only
    /// splits on semicolons that are outside such blocks.
    fn split_sql_statements(sql: &str) -> Vec<String> {
        let mut statements = Vec::new();
        let mut current = String::new();
        let mut dollar_quote_tag: Option<String> = None;
        let bytes = sql.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            if bytes[i] == b'$' {
                // Try to match a dollar-quote tag: $tag$ where tag is [a-zA-Z0-9_]*
                if let Some(tag) = try_parse_dollar_tag(bytes, i) {
                    let full_tag = &sql[i..i + tag.len()];
                    current.push_str(full_tag);
                    i += tag.len();

                    match &dollar_quote_tag {
                        Some(open_tag) if open_tag == full_tag => {
                            // Closing tag matches — exit dollar-quoted block
                            dollar_quote_tag = None;
                        },
                        None => {
                            // Opening tag — enter dollar-quoted block
                            dollar_quote_tag = Some(full_tag.to_string());
                        },
                        _ => {
                            // Inside a different dollar-quoted block, treat as content
                        },
                    }
                    continue;
                }
            }

            let Some(ch) = sql[i..].chars().next() else {
                break;
            };
            if ch == ';' && dollar_quote_tag.is_none() {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    statements.push(trimmed);
                }
                current.clear();
            } else {
                current.push(ch);
            }
            i += ch.len_utf8();
        }

        let trimmed = current.trim().to_string();
        if !trimmed.is_empty() {
            statements.push(trimmed);
        }

        statements
    }

    /// Tries to parse a dollar-quote tag starting at position `i`.
    ///
    /// Returns the full tag (e.g., `$$`, `$body$`, `$func$`) if found,
    /// or `None` if the `$` is not the start of a valid dollar-quote tag.
    fn try_parse_dollar_tag(bytes: &[u8], i: usize) -> Option<String> {
        if i >= bytes.len() || bytes[i] != b'$' {
            return None;
        }

        let mut j = i + 1;
        // Scan tag name: [a-zA-Z0-9_]*
        while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
            j += 1;
        }

        // Must end with another $
        if j < bytes.len() && bytes[j] == b'$' {
            let tag = std::str::from_utf8(&bytes[i..=j]).ok()?;
            Some(tag.to_string())
        } else {
            None
        }
    }

    /// Maximum version across a set of migrations.
    #[must_use]
    pub fn max_version(migrations: &[Migration]) -> i32 {
        migrations.iter().map(|m| m.version).max().unwrap_or(0)
    }
}

#[cfg(feature = "postgres")]
pub use implementation::{Migration, MigrationRunner, max_version};

#[cfg(not(feature = "postgres"))]
mod stub {
    /// A single migration with version and SQL (stub).
    #[derive(Debug, Clone, Copy)]
    pub struct Migration {
        /// Migration version.
        pub version: i32,
        /// Human-readable description.
        pub description: &'static str,
        /// SQL to apply.
        pub sql: &'static str,
    }

    /// Maximum version across a set of migrations.
    #[must_use]
    pub const fn max_version(migrations: &[Migration]) -> i32 {
        let mut max = 0;
        let mut i = 0;
        while i < migrations.len() {
            if migrations[i].version > max {
                max = migrations[i].version;
            }
            i += 1;
        }
        max
    }
}

#[cfg(not(feature = "postgres"))]
pub use stub::{Migration, max_version};
