//! PostgreSQL-based persistence backend.
//!
//! Provides reliable persistence using PostgreSQL as the storage backend.

#[cfg(feature = "postgres")]
mod implementation {
    use crate::models::{Domain, Memory, MemoryId, MemoryStatus, Namespace};
    use crate::storage::migrations::{Migration, MigrationRunner};
    use crate::storage::traits::PersistenceBackend;
    use crate::{Error, Result};
    use deadpool_postgres::{Config, Pool, Runtime};
    use tokio::runtime::Handle;
    use tokio_postgres::NoTls;

    /// Embedded migrations compiled into the binary.
    const MIGRATIONS: &[Migration] = &[
        Migration {
            version: 1,
            description: "Initial memories table",
            sql: r"
                CREATE TABLE IF NOT EXISTS {table} (
                    id TEXT PRIMARY KEY,
                    content TEXT NOT NULL,
                    namespace TEXT NOT NULL,
                    domain_org TEXT,
                    domain_project TEXT,
                    domain_repo TEXT,
                    status TEXT NOT NULL DEFAULT 'active',
                    tags TEXT[] NOT NULL DEFAULT '{}',
                    source TEXT,
                    embedding JSONB,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_{table}_namespace ON {table} (namespace);
                CREATE INDEX IF NOT EXISTS idx_{table}_status ON {table} (status);
                CREATE INDEX IF NOT EXISTS idx_{table}_created_at ON {table} (created_at DESC);
                CREATE INDEX IF NOT EXISTS idx_{table}_updated_at ON {table} (updated_at DESC);
            ",
        },
        Migration {
            version: 2,
            description: "Add tags GIN index",
            sql: r"
                CREATE INDEX IF NOT EXISTS idx_{table}_tags ON {table} USING GIN(tags);
            ",
        },
        Migration {
            version: 3,
            description: "Add domain composite index",
            sql: r"
                CREATE INDEX IF NOT EXISTS idx_{table}_domain ON {table} (domain_org, domain_project, domain_repo);
            ",
        },
        Migration {
            version: 4,
            description: "Add tombstoned_at column (ADR-0053)",
            sql: r"
                ALTER TABLE {table} ADD COLUMN IF NOT EXISTS tombstoned_at BIGINT;
                CREATE INDEX IF NOT EXISTS idx_{table}_tombstoned ON {table} (tombstoned_at) WHERE tombstoned_at IS NOT NULL;
            ",
        },
    ];

    /// PostgreSQL-based persistence backend.
    pub struct PostgresBackend {
        /// Connection pool.
        pool: Pool,
        /// Table name for memories.
        table_name: String,
    }

    /// Helper to map pool errors.
    fn pool_error(e: impl std::fmt::Display) -> Error {
        Error::OperationFailed {
            operation: "postgres_persistence_get_client".to_string(),
            cause: e.to_string(),
        }
    }

    /// Helper to map query errors.
    fn query_error(op: &str, e: impl std::fmt::Display) -> Error {
        Error::OperationFailed {
            operation: op.to_string(),
            cause: e.to_string(),
        }
    }

    impl PostgresBackend {
        /// Creates a new PostgreSQL backend.
        ///
        /// # Errors
        ///
        /// Returns an error if the connection pool fails to initialize.
        pub fn new(connection_url: &str, table_name: impl Into<String>) -> Result<Self> {
            Self::with_pool_size(connection_url, table_name, None)
        }

        /// Creates a new PostgreSQL backend with configurable pool size.
        ///
        /// # Arguments
        ///
        /// * `connection_url` - PostgreSQL connection URL
        /// * `table_name` - Name of the table for storing memories
        /// * `pool_max_size` - Maximum connections in pool (defaults to 20)
        ///
        /// # Errors
        ///
        /// Returns an error if the connection pool fails to initialize.
        pub fn with_pool_size(
            connection_url: &str,
            table_name: impl Into<String>,
            pool_max_size: Option<usize>,
        ) -> Result<Self> {
            let table_name = table_name.into();
            let config = Self::parse_connection_url(connection_url)?;
            let cfg = Self::build_pool_config(&config, pool_max_size);

            let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).map_err(|e| {
                Error::OperationFailed {
                    operation: "postgres_persistence_create_pool".to_string(),
                    cause: e.to_string(),
                }
            })?;

            let backend = Self { pool, table_name };
            backend.run_migrations()?;
            Ok(backend)
        }

        /// Parses the connection URL into a tokio-postgres config.
        fn parse_connection_url(url: &str) -> Result<tokio_postgres::Config> {
            url.parse::<tokio_postgres::Config>()
                .map_err(|e| Error::OperationFailed {
                    operation: "postgres_persistence_parse_url".to_string(),
                    cause: e.to_string(),
                })
        }

        /// Extracts host string from tokio-postgres Host.
        #[cfg(unix)]
        fn host_to_string(h: &tokio_postgres::config::Host) -> String {
            match h {
                tokio_postgres::config::Host::Tcp(s) => s.clone(),
                tokio_postgres::config::Host::Unix(p) => p.to_string_lossy().to_string(),
            }
        }

        /// Extracts host string from tokio-postgres Host (Windows: Tcp only).
        #[cfg(not(unix))]
        fn host_to_string(h: &tokio_postgres::config::Host) -> String {
            let tokio_postgres::config::Host::Tcp(s) = h;
            s.clone()
        }

        /// Default maximum connections in pool.
        const DEFAULT_POOL_MAX_SIZE: usize = 20;

        /// Builds a deadpool config from tokio-postgres config.
        ///
        /// # Pool Configuration (HIGH-010, DB-M2)
        ///
        /// Configures connection pool with safety limits:
        /// - Configurable max connections (defaults to 20, prevents pool exhaustion)
        /// - 5 second acquire timeout (prevents hanging on pool exhaustion)
        ///
        /// Pool size can be configured via `StorageBackendConfig.pool_max_size`.
        fn build_pool_config(
            config: &tokio_postgres::Config,
            pool_max_size: Option<usize>,
        ) -> Config {
            let mut cfg = Config::new();
            cfg.host = config.get_hosts().first().map(Self::host_to_string);
            cfg.port = config.get_ports().first().copied();
            cfg.user = config.get_user().map(String::from);
            cfg.password = config
                .get_password()
                .map(|p| String::from_utf8_lossy(p).to_string());
            cfg.dbname = config.get_dbname().map(String::from);

            // Pool configuration with timeout (HIGH-010, DB-M2)
            let max_size = pool_max_size.unwrap_or(Self::DEFAULT_POOL_MAX_SIZE);
            cfg.pool = Some(deadpool_postgres::PoolConfig {
                max_size,
                timeouts: deadpool_postgres::Timeouts {
                    wait: Some(std::time::Duration::from_secs(5)),
                    create: Some(std::time::Duration::from_secs(5)),
                    recycle: Some(std::time::Duration::from_secs(5)),
                },
                ..Default::default()
            });

            // Configure manager with fast recycling for connection reuse
            cfg.manager = Some(deadpool_postgres::ManagerConfig {
                recycling_method: deadpool_postgres::RecyclingMethod::Fast,
            });

            cfg
        }

        /// Creates a backend with default settings.
        ///
        /// # Errors
        ///
        /// Returns an error if the connection fails.
        pub fn with_defaults() -> Result<Self> {
            Self::new("postgresql://localhost/subcog", "memories")
        }

        /// Runs a blocking operation on the async pool.
        fn block_on<F, T>(&self, f: F) -> Result<T>
        where
            F: std::future::Future<Output = Result<T>>,
        {
            if let Ok(handle) = Handle::try_current() {
                handle.block_on(f)
            } else {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| Error::OperationFailed {
                        operation: "postgres_persistence_create_runtime".to_string(),
                        cause: e.to_string(),
                    })?;
                rt.block_on(f)
            }
        }

        /// Runs migrations.
        fn run_migrations(&self) -> Result<()> {
            self.block_on(async {
                let runner = MigrationRunner::new(self.pool.clone(), &self.table_name);
                runner.run(MIGRATIONS).await
            })
        }

        /// Async implementation of store operation.
        #[allow(clippy::cast_possible_wrap)]
        async fn store_async(&self, memory: &Memory) -> Result<()> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let upsert = format!(
                r"INSERT INTO {} (id, content, namespace, domain_org, domain_project, domain_repo,
                    status, tags, source, embedding, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                ON CONFLICT (id) DO UPDATE SET
                    content = EXCLUDED.content,
                    namespace = EXCLUDED.namespace,
                    domain_org = EXCLUDED.domain_org,
                    domain_project = EXCLUDED.domain_project,
                    domain_repo = EXCLUDED.domain_repo,
                    status = EXCLUDED.status,
                    tags = EXCLUDED.tags,
                    source = EXCLUDED.source,
                    embedding = EXCLUDED.embedding,
                    updated_at = EXCLUDED.updated_at",
                self.table_name
            );

            let tags: Vec<&str> = memory.tags.iter().map(String::as_str).collect();
            let embedding_json: Option<serde_json::Value> =
                memory.embedding.as_ref().map(|e| serde_json::json!(e));

            client
                .execute(
                    &upsert,
                    &[
                        &memory.id.as_str(),
                        &memory.content,
                        &memory.namespace.as_str(),
                        &memory.domain.organization,
                        &memory.domain.project,
                        &memory.domain.repository,
                        &memory.status.as_str(),
                        &tags,
                        &memory.source,
                        &embedding_json,
                        &(memory.created_at as i64),
                        &(memory.updated_at as i64),
                    ],
                )
                .await
                .map_err(|e| query_error("postgres_persistence_store", e))?;

            Ok(())
        }

        /// Async implementation of get operation.
        #[allow(clippy::cast_sign_loss)]
        async fn get_async(&self, id: &MemoryId) -> Result<Option<Memory>> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let query = format!(
                r"SELECT id, content, namespace, domain_org, domain_project, domain_repo,
                    status, tags, source, embedding, created_at, updated_at
                FROM {}
                WHERE id = $1",
                self.table_name
            );

            let row = client
                .query_opt(&query, &[&id.as_str()])
                .await
                .map_err(|e| query_error("postgres_persistence_get", e))?;

            Ok(row.map(|r| Self::row_to_memory(&r)))
        }

        /// Converts a database row to a Memory.
        #[allow(clippy::cast_sign_loss)]
        fn row_to_memory(row: &tokio_postgres::Row) -> Memory {
            let id: String = row.get("id");
            let content: String = row.get("content");
            let namespace_str: String = row.get("namespace");
            let domain_org: Option<String> = row.get("domain_org");
            let domain_project: Option<String> = row.get("domain_project");
            let domain_repo: Option<String> = row.get("domain_repo");
            let status_str: String = row.get("status");
            let tags: Vec<String> = row.get("tags");
            let source: Option<String> = row.get("source");
            let embedding_json: Option<serde_json::Value> = row.get("embedding");
            let created_at: i64 = row.get("created_at");
            let updated_at: i64 = row.get("updated_at");

            let namespace = Namespace::parse(&namespace_str).unwrap_or_default();
            let status = match status_str.as_str() {
                "active" => MemoryStatus::Active,
                "archived" => MemoryStatus::Archived,
                "superseded" => MemoryStatus::Superseded,
                "pending" => MemoryStatus::Pending,
                "deleted" => MemoryStatus::Deleted,
                _ => MemoryStatus::Active,
            };

            let embedding: Option<Vec<f32>> =
                embedding_json.and_then(|v| serde_json::from_value(v).ok());

            Memory {
                id: MemoryId::new(id),
                content,
                namespace,
                domain: Domain {
                    organization: domain_org,
                    project: domain_project,
                    repository: domain_repo,
                },
                status,
                tags,
                source,
                embedding,
                created_at: created_at as u64,
                updated_at: updated_at as u64,
                tombstoned_at: None,
            }
        }

        /// Async implementation of delete operation.
        async fn delete_async(&self, id: &MemoryId) -> Result<bool> {
            let client = self.pool.get().await.map_err(pool_error)?;
            let delete = format!("DELETE FROM {} WHERE id = $1", self.table_name);
            let rows = client
                .execute(&delete, &[&id.as_str()])
                .await
                .map_err(|e| query_error("postgres_persistence_delete", e))?;
            Ok(rows > 0)
        }

        /// Async implementation of `list_ids` operation.
        async fn list_ids_async(&self) -> Result<Vec<MemoryId>> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let query = format!(
                "SELECT id FROM {} ORDER BY updated_at DESC",
                self.table_name
            );

            let rows = client
                .query(&query, &[])
                .await
                .map_err(|e| query_error("postgres_persistence_list_ids", e))?;

            Ok(rows
                .iter()
                .map(|row| {
                    let id: String = row.get(0);
                    MemoryId::new(id)
                })
                .collect())
        }

        /// Async implementation of `get_batch` operation using single IN query.
        ///
        /// Avoids N+1 queries by fetching all IDs in a single round-trip.
        #[allow(clippy::cast_sign_loss)]
        async fn get_batch_async(&self, ids: &[MemoryId]) -> Result<Vec<Memory>> {
            if ids.is_empty() {
                return Ok(Vec::new());
            }

            let client = self.pool.get().await.map_err(pool_error)?;

            // Build parameterized IN clause: $1, $2, $3, ...
            let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("${i}")).collect();
            let query = format!(
                r"SELECT id, content, namespace, domain_org, domain_project, domain_repo,
                    status, tags, source, embedding, created_at, updated_at
                FROM {} WHERE id IN ({})",
                self.table_name,
                placeholders.join(", ")
            );

            // Build params array
            let id_strs: Vec<&str> = ids.iter().map(MemoryId::as_str).collect();
            let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
                id_strs.iter().map(|s| s as _).collect();

            let rows = client
                .query(&query, &params)
                .await
                .map_err(|e| query_error("postgres_persistence_get_batch", e))?;

            Ok(rows.iter().map(Self::row_to_memory).collect())
        }
    }

    impl PersistenceBackend for PostgresBackend {
        fn store(&self, memory: &Memory) -> Result<()> {
            self.block_on(self.store_async(memory))
        }

        fn get(&self, id: &MemoryId) -> Result<Option<Memory>> {
            self.block_on(self.get_async(id))
        }

        fn delete(&self, id: &MemoryId) -> Result<bool> {
            self.block_on(self.delete_async(id))
        }

        fn list_ids(&self) -> Result<Vec<MemoryId>> {
            self.block_on(self.list_ids_async())
        }

        /// Optimized batch retrieval using a single IN query (HIGH-PERF-002).
        fn get_batch(&self, ids: &[MemoryId]) -> Result<Vec<Memory>> {
            self.block_on(self.get_batch_async(ids))
        }
    }
}

#[cfg(feature = "postgres")]
pub use implementation::PostgresBackend;

#[cfg(not(feature = "postgres"))]
mod stub {
    use crate::models::{Memory, MemoryId};
    use crate::storage::traits::PersistenceBackend;
    use crate::{Error, Result};

    /// Stub PostgreSQL backend when feature is not enabled.
    pub struct PostgresBackend {
        connection_url: String,
        table_name: String,
    }

    impl PostgresBackend {
        /// Creates a new PostgreSQL backend (stub).
        #[must_use]
        pub fn new(connection_url: impl Into<String>, table_name: impl Into<String>) -> Self {
            Self {
                connection_url: connection_url.into(),
                table_name: table_name.into(),
            }
        }

        /// Creates a new PostgreSQL backend with configurable pool size (stub).
        ///
        /// The pool size is ignored in the stub - requires `postgres` feature.
        #[must_use]
        pub fn with_pool_size(
            connection_url: impl Into<String>,
            table_name: impl Into<String>,
            _pool_max_size: Option<usize>,
        ) -> Self {
            Self::new(connection_url, table_name)
        }

        /// Creates a backend with default settings (stub).
        #[must_use]
        pub fn with_defaults() -> Self {
            Self::new("postgresql://localhost/subcog", "memories")
        }
    }

    impl PersistenceBackend for PostgresBackend {
        fn store(&self, _memory: &Memory) -> Result<()> {
            Err(Error::NotImplemented(format!(
                "PostgresBackend::store to {} on {}",
                self.table_name, self.connection_url
            )))
        }

        fn get(&self, _id: &MemoryId) -> Result<Option<Memory>> {
            Err(Error::NotImplemented(format!(
                "PostgresBackend::get from {} on {}",
                self.table_name, self.connection_url
            )))
        }

        fn delete(&self, _id: &MemoryId) -> Result<bool> {
            Err(Error::NotImplemented(format!(
                "PostgresBackend::delete from {} on {}",
                self.table_name, self.connection_url
            )))
        }

        fn list_ids(&self) -> Result<Vec<MemoryId>> {
            Err(Error::NotImplemented(format!(
                "PostgresBackend::list_ids from {} on {}",
                self.table_name, self.connection_url
            )))
        }
    }
}

#[cfg(not(feature = "postgres"))]
pub use stub::PostgresBackend;

#[cfg(all(test, feature = "postgres"))]
mod tests {
    use super::*;
    use crate::models::{Domain, Memory, MemoryId, MemoryStatus, Namespace};
    use crate::storage::traits::PersistenceBackend;
    use std::env;

    /// Gets test database URL from environment or skips test.
    fn get_test_db_url() -> Option<String> {
        env::var("SUBCOG_TEST_POSTGRES_URL").ok()
    }

    /// Creates a test memory with given ID.
    fn create_test_memory(id: &str) -> Memory {
        Memory {
            id: MemoryId::new(id),
            content: format!("Test content for {id}"),
            namespace: Namespace::Decisions,
            domain: Domain {
                organization: Some("test-org".to_string()),
                project: Some("test-project".to_string()),
                repository: Some("test-repo".to_string()),
            },
            status: MemoryStatus::Active,
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
            tombstoned_at: None,
            embedding: None,
            tags: vec!["test".to_string(), "integration".to_string()],
            source: Some("test.rs".to_string()),
        }
    }

    /// Creates a unique table name for test isolation.
    fn unique_table_name() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        format!("test_memories_{ts}")
    }

    #[test]
    fn test_store_and_retrieve_memory() {
        let Some(url) = get_test_db_url() else {
            eprintln!("Skipping: SUBCOG_TEST_POSTGRES_URL not set");
            return;
        };

        let table = unique_table_name();
        let backend = PostgresBackend::new(&url, &table).expect("Failed to create backend");

        let memory = create_test_memory("test-store-retrieve");
        backend.store(&memory).expect("Failed to store memory");

        let retrieved = backend
            .get(&MemoryId::new("test-store-retrieve"))
            .expect("Failed to get memory");

        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id.as_str(), "test-store-retrieve");
        assert_eq!(retrieved.namespace, Namespace::Decisions);
        assert_eq!(retrieved.status, MemoryStatus::Active);
        assert!(retrieved.content.contains("test-store-retrieve"));
    }

    #[test]
    fn test_get_nonexistent_memory() {
        let Some(url) = get_test_db_url() else {
            eprintln!("Skipping: SUBCOG_TEST_POSTGRES_URL not set");
            return;
        };

        let table = unique_table_name();
        let backend = PostgresBackend::new(&url, &table).expect("Failed to create backend");

        let result = backend
            .get(&MemoryId::new("nonexistent-id"))
            .expect("Failed to query");

        assert!(result.is_none());
    }

    #[test]
    fn test_update_existing_memory() {
        let Some(url) = get_test_db_url() else {
            eprintln!("Skipping: SUBCOG_TEST_POSTGRES_URL not set");
            return;
        };

        let table = unique_table_name();
        let backend = PostgresBackend::new(&url, &table).expect("Failed to create backend");

        let mut memory = create_test_memory("test-update");
        backend.store(&memory).expect("Failed to store initial");

        // Update the memory
        memory.content = "Updated content".to_string();
        memory.status = MemoryStatus::Archived;
        memory.updated_at = 1_700_001_000;
        backend.store(&memory).expect("Failed to store update");

        let retrieved = backend
            .get(&MemoryId::new("test-update"))
            .expect("Failed to get")
            .expect("Memory not found");

        assert_eq!(retrieved.content, "Updated content");
        assert_eq!(retrieved.status, MemoryStatus::Archived);
        assert_eq!(retrieved.updated_at, 1_700_001_000);
    }

    #[test]
    fn test_delete_memory() {
        let Some(url) = get_test_db_url() else {
            eprintln!("Skipping: SUBCOG_TEST_POSTGRES_URL not set");
            return;
        };

        let table = unique_table_name();
        let backend = PostgresBackend::new(&url, &table).expect("Failed to create backend");

        let memory = create_test_memory("test-delete");
        backend.store(&memory).expect("Failed to store");

        let deleted = backend
            .delete(&MemoryId::new("test-delete"))
            .expect("Failed to delete");
        assert!(deleted);

        let retrieved = backend
            .get(&MemoryId::new("test-delete"))
            .expect("Failed to get");
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_delete_nonexistent_memory() {
        let Some(url) = get_test_db_url() else {
            eprintln!("Skipping: SUBCOG_TEST_POSTGRES_URL not set");
            return;
        };

        let table = unique_table_name();
        let backend = PostgresBackend::new(&url, &table).expect("Failed to create backend");

        let deleted = backend
            .delete(&MemoryId::new("never-existed"))
            .expect("Failed to delete");
        assert!(!deleted);
    }

    #[test]
    fn test_list_ids() {
        let Some(url) = get_test_db_url() else {
            eprintln!("Skipping: SUBCOG_TEST_POSTGRES_URL not set");
            return;
        };

        let table = unique_table_name();
        let backend = PostgresBackend::new(&url, &table).expect("Failed to create backend");

        // Initially empty
        let ids = backend.list_ids().expect("Failed to list");
        assert!(ids.is_empty());

        // Add some memories
        for i in 1..=3 {
            let memory = create_test_memory(&format!("list-test-{i}"));
            backend.store(&memory).expect("Failed to store");
        }

        let ids = backend.list_ids().expect("Failed to list");
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn test_memory_with_embedding() {
        let Some(url) = get_test_db_url() else {
            eprintln!("Skipping: SUBCOG_TEST_POSTGRES_URL not set");
            return;
        };

        let table = unique_table_name();
        let backend = PostgresBackend::new(&url, &table).expect("Failed to create backend");

        let mut memory = create_test_memory("test-embedding");
        memory.embedding = Some(vec![0.1, 0.2, 0.3, 0.4, 0.5]);

        backend.store(&memory).expect("Failed to store");

        let retrieved = backend
            .get(&MemoryId::new("test-embedding"))
            .expect("Failed to get")
            .expect("Memory not found");

        assert!(retrieved.embedding.is_some());
        let emb = retrieved.embedding.unwrap();
        assert_eq!(emb.len(), 5);
        assert!((emb[0] - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn test_all_namespaces() {
        let Some(url) = get_test_db_url() else {
            eprintln!("Skipping: SUBCOG_TEST_POSTGRES_URL not set");
            return;
        };

        let table = unique_table_name();
        let backend = PostgresBackend::new(&url, &table).expect("Failed to create backend");

        let namespaces = [
            Namespace::Decisions,
            Namespace::Patterns,
            Namespace::Learnings,
            Namespace::Context,
            Namespace::TechDebt,
            Namespace::Apis,
            Namespace::Config,
            Namespace::Security,
            Namespace::Performance,
            Namespace::Testing,
        ];

        for (i, ns) in namespaces.iter().enumerate() {
            let mut memory = create_test_memory(&format!("ns-test-{i}"));
            memory.namespace = *ns;
            backend.store(&memory).expect("Failed to store");

            let retrieved = backend
                .get(&MemoryId::new(format!("ns-test-{i}")))
                .expect("Failed to get")
                .expect("Memory not found");

            assert_eq!(retrieved.namespace, *ns);
        }
    }
}

#[cfg(all(test, not(feature = "postgres")))]
mod stub_tests {
    use super::*;
    use crate::models::{Domain, Memory, MemoryId, MemoryStatus, Namespace};
    use crate::storage::traits::PersistenceBackend;

    fn create_test_memory() -> Memory {
        Memory {
            id: MemoryId::new("test-id"),
            content: "Test content".to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            status: MemoryStatus::Active,
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
            tombstoned_at: None,
            embedding: None,
            tags: vec![],
            source: None,
        }
    }

    #[test]
    fn test_stub_store_returns_not_implemented() {
        let backend = PostgresBackend::with_defaults();
        let memory = create_test_memory();
        let result = backend.store(&memory);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::Error::NotImplemented(_)
        ));
    }

    #[test]
    fn test_stub_get_returns_not_implemented() {
        let backend = PostgresBackend::with_defaults();
        let result = backend.get(&MemoryId::new("test"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::Error::NotImplemented(_)
        ));
    }

    #[test]
    fn test_stub_delete_returns_not_implemented() {
        let backend = PostgresBackend::with_defaults();
        let result = backend.delete(&MemoryId::new("test"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::Error::NotImplemented(_)
        ));
    }

    #[test]
    fn test_stub_list_ids_returns_not_implemented() {
        let backend = PostgresBackend::with_defaults();
        let result = backend.list_ids();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::Error::NotImplemented(_)
        ));
    }

    #[test]
    fn test_stub_new_creates_instance() {
        // Stub constructor always succeeds (returns stub, not Result)
        let _backend = PostgresBackend::new("postgresql://custom", "custom_table");
    }

    #[test]
    fn test_stub_with_defaults_creates_instance() {
        // with_defaults() always succeeds (returns stub, not Result)
        let _backend = PostgresBackend::with_defaults();
    }
}
