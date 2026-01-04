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
    }

    impl MigrationRunner {
        /// Creates a new migration runner.
        #[must_use]
        pub fn new(pool: Pool, table_name: impl Into<String>) -> Self {
            Self {
                pool,
                table_name: table_name.into(),
            }
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
                cause: e.to_string(),
            })?;

            // Ensure migrations tracking table exists
            self.ensure_migrations_table(&client).await?;

            // Get current version
            let current_version = self.get_current_version(&client).await?;

            // Apply pending migrations
            for migration in migrations {
                if migration.version > current_version {
                    self.apply_migration(&mut client, migration).await?;
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
            let exists = self.table_exists(&client, &migrations_table).await?;

            if !exists {
                return Ok(0);
            }

            self.get_current_version(&client).await
        }

        /// Returns the name of the migrations tracking table.
        fn migrations_table_name(&self) -> String {
            format!("{}_schema_migrations", self.table_name)
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
                    WHERE table_name = $1
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
            let sql = migration.sql.replace("{table}", &self.table_name);

            // Start transaction for atomic migration application
            let tx = client
                .transaction()
                .await
                .map_err(|e| Error::OperationFailed {
                    operation: format!("migration_v{}_begin_tx", migration.version),
                    cause: e.to_string(),
                })?;

            // Execute all statements within the transaction
            for statement in sql.split(';') {
                let statement = statement.trim();
                if statement.is_empty() {
                    continue;
                }

                tx.execute(statement, &[])
                    .await
                    .map_err(|e| Error::OperationFailed {
                        operation: format!(
                            "migration_v{}: {}",
                            migration.version, migration.description
                        ),
                        cause: e.to_string(),
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
