//! pgvector-based vector backend.
//!
//! Provides vector similarity search using PostgreSQL with pgvector extension.

#[cfg(feature = "postgres")]
mod implementation {
    use crate::models::{MemoryId, SearchFilter};
    use crate::storage::migrations::{Migration, MigrationRunner};
    use crate::storage::traits::VectorBackend;
    use crate::{Error, Result};
    use deadpool_postgres::{Config, Pool, Runtime};
    use tokio::runtime::Handle;
    use tokio_postgres::NoTls;

    /// Default embedding dimensions for all-MiniLM-L6-v2.
    pub const DEFAULT_DIMENSIONS: usize = 384;

    /// Embedded migrations compiled into the binary.
    /// Note: Migration 1 assumes pgvector extension is already installed.
    /// Run `CREATE EXTENSION IF NOT EXISTS vector;` before using this backend.
    const MIGRATIONS: &[Migration] = &[
        Migration {
            version: 1,
            description: "Initial vectors table",
            sql: r"
                CREATE TABLE IF NOT EXISTS {table} (
                    id TEXT PRIMARY KEY,
                    embedding vector(384),
                    namespace TEXT,
                    created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT
                );
            ",
        },
        Migration {
            version: 2,
            description: "Add HNSW index for cosine similarity",
            sql: r"
                CREATE INDEX IF NOT EXISTS {table}_embedding_idx
                ON {table} USING hnsw (embedding vector_cosine_ops)
                WITH (m = 16, ef_construction = 64);
            ",
        },
        Migration {
            version: 3,
            description: "Add namespace index for filtering",
            sql: r"
                CREATE INDEX IF NOT EXISTS {table}_namespace_idx ON {table} (namespace);
            ",
        },
    ];

    /// pgvector-based vector backend.
    pub struct PgvectorBackend {
        /// Connection pool.
        pool: Pool,
        /// Table name for vectors.
        table_name: String,
        /// Embedding dimensions.
        dimensions: usize,
    }

    /// Helper to map pool errors.
    fn pool_error(e: impl std::fmt::Display) -> Error {
        Error::OperationFailed {
            operation: "pgvector_get_client".to_string(),
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

    impl PgvectorBackend {
        /// Creates a new pgvector backend.
        ///
        /// # Errors
        ///
        /// Returns an error if the connection pool fails to initialize or
        /// if migrations fail (which can happen if pgvector extension is not installed).
        pub fn new(
            connection_url: &str,
            table_name: impl Into<String>,
            dimensions: usize,
        ) -> Result<Self> {
            let table_name = table_name.into();
            let config = Self::parse_connection_url(connection_url)?;
            let cfg = Self::build_pool_config(&config);

            let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).map_err(|e| {
                Error::OperationFailed {
                    operation: "pgvector_create_pool".to_string(),
                    cause: e.to_string(),
                }
            })?;

            let backend = Self {
                pool,
                table_name,
                dimensions,
            };
            backend.run_migrations()?;
            Ok(backend)
        }

        /// Parses the connection URL into a tokio-postgres config.
        fn parse_connection_url(url: &str) -> Result<tokio_postgres::Config> {
            url.parse::<tokio_postgres::Config>()
                .map_err(|e| Error::OperationFailed {
                    operation: "pgvector_parse_url".to_string(),
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

        /// Builds a deadpool config from tokio-postgres config.
        fn build_pool_config(config: &tokio_postgres::Config) -> Config {
            let mut cfg = Config::new();
            cfg.host = config.get_hosts().first().map(Self::host_to_string);
            cfg.port = config.get_ports().first().copied();
            cfg.user = config.get_user().map(String::from);
            cfg.password = config
                .get_password()
                .map(|p| String::from_utf8_lossy(p).to_string());
            cfg.dbname = config.get_dbname().map(String::from);
            cfg
        }

        /// Creates a backend with default settings.
        ///
        /// # Errors
        ///
        /// Returns an error if the connection fails.
        pub fn with_defaults() -> Result<Self> {
            Self::new(
                "postgresql://localhost/subcog",
                "memory_vectors",
                DEFAULT_DIMENSIONS,
            )
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
                        operation: "pgvector_create_runtime".to_string(),
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

        /// Formats an embedding as a pgvector string: `'[1.0,2.0,3.0]'`.
        fn format_embedding(embedding: &[f32]) -> String {
            let values: Vec<String> = embedding
                .iter()
                .map(std::string::ToString::to_string)
                .collect();
            format!("[{}]", values.join(","))
        }

        /// Async implementation of upsert operation.
        async fn upsert_async(&self, id: &MemoryId, embedding: &[f32]) -> Result<()> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let embedding_str = Self::format_embedding(embedding);

            let upsert = format!(
                r"INSERT INTO {} (id, embedding)
                VALUES ($1, $2::vector)
                ON CONFLICT (id) DO UPDATE SET
                    embedding = EXCLUDED.embedding",
                self.table_name
            );

            client
                .execute(&upsert, &[&id.as_str(), &embedding_str])
                .await
                .map_err(|e| query_error("pgvector_upsert", e))?;

            Ok(())
        }

        /// Async implementation of remove operation.
        async fn remove_async(&self, id: &MemoryId) -> Result<bool> {
            let client = self.pool.get().await.map_err(pool_error)?;
            let delete = format!("DELETE FROM {} WHERE id = $1", self.table_name);
            let rows = client
                .execute(&delete, &[&id.as_str()])
                .await
                .map_err(|e| query_error("pgvector_remove", e))?;
            Ok(rows > 0)
        }

        /// Async implementation of search operation.
        /// Returns cosine similarity (1 - `cosine_distance`).
        async fn search_async(
            &self,
            query_embedding: &[f32],
            filter: &SearchFilter,
            limit: usize,
        ) -> Result<Vec<(MemoryId, f32)>> {
            let client = self.pool.get().await.map_err(pool_error)?;
            let embedding_str = Self::format_embedding(query_embedding);

            // Build namespace filter if present
            let (namespace_clause, namespace_params) = Self::build_namespace_filter(filter);

            let search_query = format!(
                r"SELECT id, 1 - (embedding <=> $1::vector) as similarity
                FROM {}
                {}
                ORDER BY embedding <=> $1::vector
                LIMIT {}",
                self.table_name, namespace_clause, limit
            );

            let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = Vec::new();
            params.push(&embedding_str);
            for p in &namespace_params {
                params.push(p);
            }

            let rows = client
                .query(&search_query, &params)
                .await
                .map_err(|e| query_error("pgvector_search", e))?;

            Ok(rows
                .iter()
                .map(|row| {
                    let id: String = row.get(0);
                    let similarity: f64 = row.get(1);
                    #[allow(clippy::cast_possible_truncation)]
                    (MemoryId::new(&id), similarity as f32)
                })
                .collect())
        }

        /// Builds namespace filter clause.
        fn build_namespace_filter(filter: &SearchFilter) -> (String, Vec<String>) {
            if filter.namespaces.is_empty() {
                return (String::new(), Vec::new());
            }

            let placeholders: Vec<String> = filter
                .namespaces
                .iter()
                .enumerate()
                .map(|(i, _)| format!("${}", i + 2))
                .collect();

            let clause = format!("WHERE namespace IN ({})", placeholders.join(", "));
            let params: Vec<String> = filter
                .namespaces
                .iter()
                .map(|ns| ns.as_str().to_string())
                .collect();

            (clause, params)
        }

        /// Async implementation of count operation.
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        async fn count_async(&self) -> Result<usize> {
            let client = self.pool.get().await.map_err(pool_error)?;
            let query = format!("SELECT COUNT(*) FROM {}", self.table_name);
            let row = client
                .query_one(&query, &[])
                .await
                .map_err(|e| query_error("pgvector_count", e))?;
            let count: i64 = row.get(0);
            Ok(count as usize)
        }

        /// Async implementation of clear operation.
        async fn clear_async(&self) -> Result<()> {
            let client = self.pool.get().await.map_err(pool_error)?;
            let truncate = format!("TRUNCATE TABLE {}", self.table_name);
            client
                .execute(&truncate, &[])
                .await
                .map_err(|e| query_error("pgvector_clear", e))?;
            Ok(())
        }
    }

    impl VectorBackend for PgvectorBackend {
        fn dimensions(&self) -> usize {
            self.dimensions
        }

        fn upsert(&self, id: &MemoryId, embedding: &[f32]) -> Result<()> {
            self.block_on(self.upsert_async(id, embedding))
        }

        fn remove(&self, id: &MemoryId) -> Result<bool> {
            self.block_on(self.remove_async(id))
        }

        fn search(
            &self,
            query_embedding: &[f32],
            filter: &SearchFilter,
            limit: usize,
        ) -> Result<Vec<(MemoryId, f32)>> {
            self.block_on(self.search_async(query_embedding, filter, limit))
        }

        fn count(&self) -> Result<usize> {
            self.block_on(self.count_async())
        }

        fn clear(&self) -> Result<()> {
            self.block_on(self.clear_async())
        }
    }
}

#[cfg(feature = "postgres")]
pub use implementation::{DEFAULT_DIMENSIONS, PgvectorBackend};

#[cfg(not(feature = "postgres"))]
mod stub {
    use crate::models::{MemoryId, SearchFilter};
    use crate::storage::traits::VectorBackend;
    use crate::{Error, Result};

    /// Default embedding dimensions for all-MiniLM-L6-v2.
    pub const DEFAULT_DIMENSIONS: usize = 384;

    /// pgvector-based vector backend (stub).
    pub struct PgvectorBackend {
        /// PostgreSQL connection URL.
        connection_url: String,
        /// Table name for vectors.
        table_name: String,
        /// Embedding dimensions.
        dimensions: usize,
    }

    impl PgvectorBackend {
        /// Creates a new pgvector backend (stub).
        #[must_use]
        pub fn new(
            connection_url: impl Into<String>,
            table_name: impl Into<String>,
            dimensions: usize,
        ) -> Self {
            Self {
                connection_url: connection_url.into(),
                table_name: table_name.into(),
                dimensions,
            }
        }

        /// Creates a backend with default settings (stub).
        #[must_use]
        pub fn with_defaults() -> Self {
            Self::new(
                "postgresql://localhost/subcog",
                "memory_vectors",
                DEFAULT_DIMENSIONS,
            )
        }
    }

    impl VectorBackend for PgvectorBackend {
        fn dimensions(&self) -> usize {
            self.dimensions
        }

        fn upsert(&self, _id: &MemoryId, _embedding: &[f32]) -> Result<()> {
            Err(Error::NotImplemented(format!(
                "PgvectorBackend::upsert for {} on {}",
                self.table_name, self.connection_url
            )))
        }

        fn remove(&self, _id: &MemoryId) -> Result<bool> {
            Err(Error::NotImplemented(format!(
                "PgvectorBackend::remove for {} on {}",
                self.table_name, self.connection_url
            )))
        }

        fn search(
            &self,
            _query_embedding: &[f32],
            _filter: &SearchFilter,
            _limit: usize,
        ) -> Result<Vec<(MemoryId, f32)>> {
            Err(Error::NotImplemented(format!(
                "PgvectorBackend::search for {} on {}",
                self.table_name, self.connection_url
            )))
        }

        fn count(&self) -> Result<usize> {
            Err(Error::NotImplemented(format!(
                "PgvectorBackend::count for {} on {}",
                self.table_name, self.connection_url
            )))
        }

        fn clear(&self) -> Result<()> {
            Err(Error::NotImplemented(format!(
                "PgvectorBackend::clear for {} on {}",
                self.table_name, self.connection_url
            )))
        }
    }
}

#[cfg(not(feature = "postgres"))]
pub use stub::{DEFAULT_DIMENSIONS, PgvectorBackend};
