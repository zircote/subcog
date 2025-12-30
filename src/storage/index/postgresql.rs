//! PostgreSQL-based index backend.
//!
//! Provides full-text search using PostgreSQL's built-in tsvector/tsquery.

#[cfg(feature = "postgres")]
mod implementation {
    use crate::models::{Memory, MemoryId, SearchFilter};
    use crate::storage::traits::IndexBackend;
    use crate::{Error, Result};
    use deadpool_postgres::{Config, Pool, Runtime};
    use tokio::runtime::Handle;
    use tokio_postgres::NoTls;

    /// PostgreSQL-based index backend.
    pub struct PostgresIndexBackend {
        /// Connection pool.
        pool: Pool,
        /// Table name for memories.
        table_name: String,
    }

    /// Helper to map pool errors.
    fn pool_error(e: impl std::fmt::Display) -> Error {
        Error::OperationFailed {
            operation: "postgres_get_client".to_string(),
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

    impl PostgresIndexBackend {
        /// Creates a new PostgreSQL index backend.
        ///
        /// # Errors
        ///
        /// Returns an error if the connection pool fails to initialize.
        pub fn new(connection_url: &str, table_name: impl Into<String>) -> Result<Self> {
            let table_name = table_name.into();
            let config = Self::parse_connection_url(connection_url)?;
            let cfg = Self::build_pool_config(&config);

            let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).map_err(|e| {
                Error::OperationFailed {
                    operation: "postgres_create_pool".to_string(),
                    cause: e.to_string(),
                }
            })?;

            let backend = Self { pool, table_name };
            backend.ensure_table()?;
            Ok(backend)
        }

        /// Parses the connection URL into a tokio-postgres config.
        fn parse_connection_url(url: &str) -> Result<tokio_postgres::Config> {
            url.parse::<tokio_postgres::Config>()
                .map_err(|e| Error::OperationFailed {
                    operation: "postgres_parse_url".to_string(),
                    cause: e.to_string(),
                })
        }

        /// Builds a deadpool config from tokio-postgres config.
        fn build_pool_config(config: &tokio_postgres::Config) -> Config {
            let mut cfg = Config::new();
            cfg.host = config.get_hosts().first().map(|h| match h {
                tokio_postgres::config::Host::Tcp(s) => s.clone(),
                tokio_postgres::config::Host::Unix(p) => p.to_string_lossy().to_string(),
            });
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
                        operation: "postgres_create_runtime".to_string(),
                        cause: e.to_string(),
                    })?;
                rt.block_on(f)
            }
        }

        /// Ensures the memories table and index exist.
        fn ensure_table(&self) -> Result<()> {
            self.block_on(self.ensure_table_async())
        }

        /// Async implementation of table creation.
        async fn ensure_table_async(&self) -> Result<()> {
            let client = self.pool.get().await.map_err(pool_error)?;
            self.create_main_table(&client).await?;
            self.create_search_index(&client).await?;
            self.create_namespace_index(&client).await?;
            self.create_updated_index(&client).await
        }

        /// Creates the main memories table.
        async fn create_main_table(&self, client: &deadpool_postgres::Object) -> Result<()> {
            let sql = format!(
                r"CREATE TABLE IF NOT EXISTS {} (
                    id TEXT PRIMARY KEY,
                    content TEXT NOT NULL,
                    namespace TEXT NOT NULL,
                    domain TEXT NOT NULL,
                    status TEXT NOT NULL,
                    tags TEXT[] DEFAULT '{{}}',
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL,
                    search_vector TSVECTOR GENERATED ALWAYS AS (
                        setweight(to_tsvector('english', coalesce(content, '')), 'A') ||
                        setweight(to_tsvector('english', coalesce(array_to_string(tags, ' '), '')), 'B')
                    ) STORED
                )",
                self.table_name
            );
            client
                .execute(&sql, &[])
                .await
                .map_err(|e| query_error("postgres_create_table", e))?;
            Ok(())
        }

        /// Creates GIN index on `search_vector`.
        async fn create_search_index(&self, client: &deadpool_postgres::Object) -> Result<()> {
            let sql = format!(
                "CREATE INDEX IF NOT EXISTS {}_search_idx ON {} USING GIN (search_vector)",
                self.table_name, self.table_name
            );
            client
                .execute(&sql, &[])
                .await
                .map_err(|e| query_error("postgres_create_index", e))?;
            Ok(())
        }

        /// Creates index on namespace for filtering.
        async fn create_namespace_index(&self, client: &deadpool_postgres::Object) -> Result<()> {
            let sql = format!(
                "CREATE INDEX IF NOT EXISTS {}_namespace_idx ON {} (namespace)",
                self.table_name, self.table_name
            );
            client
                .execute(&sql, &[])
                .await
                .map_err(|e| query_error("postgres_create_ns_index", e))?;
            Ok(())
        }

        /// Creates index on `updated_at` for sorting.
        async fn create_updated_index(&self, client: &deadpool_postgres::Object) -> Result<()> {
            let sql = format!(
                "CREATE INDEX IF NOT EXISTS {}_updated_idx ON {} (updated_at DESC)",
                self.table_name, self.table_name
            );
            client
                .execute(&sql, &[])
                .await
                .map_err(|e| query_error("postgres_create_updated_index", e))?;
            Ok(())
        }

        /// Builds WHERE clause for filters.
        fn build_where_clause(filter: &SearchFilter, start_param: i32) -> (String, Vec<String>) {
            let mut clauses = Vec::new();
            let mut params = Vec::new();
            let mut param_num = start_param;

            Self::add_namespace_filter(filter, &mut clauses, &mut params, &mut param_num);
            Self::add_domain_filter(filter, &mut clauses, &mut params, &mut param_num);
            Self::add_status_filter(filter, &mut clauses, &mut params, &mut param_num);

            let clause = if clauses.is_empty() {
                String::new()
            } else {
                format!(" AND {}", clauses.join(" AND "))
            };

            (clause, params)
        }

        /// Adds namespace filter to WHERE clause.
        fn add_namespace_filter(
            filter: &SearchFilter,
            clauses: &mut Vec<String>,
            params: &mut Vec<String>,
            param_num: &mut i32,
        ) {
            if filter.namespaces.is_empty() {
                return;
            }
            let placeholders: Vec<String> = filter
                .namespaces
                .iter()
                .map(|_| {
                    let p = format!("${param_num}");
                    *param_num += 1;
                    p
                })
                .collect();
            clauses.push(format!("namespace IN ({})", placeholders.join(", ")));
            for ns in &filter.namespaces {
                params.push(ns.as_str().to_string());
            }
        }

        /// Adds domain filter to WHERE clause.
        fn add_domain_filter(
            filter: &SearchFilter,
            clauses: &mut Vec<String>,
            params: &mut Vec<String>,
            param_num: &mut i32,
        ) {
            if filter.domains.is_empty() {
                return;
            }
            let placeholders: Vec<String> = filter
                .domains
                .iter()
                .map(|_| {
                    let p = format!("${param_num}");
                    *param_num += 1;
                    p
                })
                .collect();
            clauses.push(format!("domain IN ({})", placeholders.join(", ")));
            for d in &filter.domains {
                params.push(d.to_string());
            }
        }

        /// Adds status filter to WHERE clause.
        fn add_status_filter(
            filter: &SearchFilter,
            clauses: &mut Vec<String>,
            params: &mut Vec<String>,
            param_num: &mut i32,
        ) {
            if filter.statuses.is_empty() {
                return;
            }
            let placeholders: Vec<String> = filter
                .statuses
                .iter()
                .map(|_| {
                    let p = format!("${param_num}");
                    *param_num += 1;
                    p
                })
                .collect();
            clauses.push(format!("status IN ({})", placeholders.join(", ")));
            for s in &filter.statuses {
                params.push(s.as_str().to_string());
            }
        }

        /// Async implementation of index operation.
        #[allow(clippy::cast_possible_wrap)]
        async fn index_async(&self, memory: &Memory) -> Result<()> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let upsert = format!(
                r"INSERT INTO {} (id, content, namespace, domain, status, tags, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (id) DO UPDATE SET
                    content = EXCLUDED.content,
                    namespace = EXCLUDED.namespace,
                    domain = EXCLUDED.domain,
                    status = EXCLUDED.status,
                    tags = EXCLUDED.tags,
                    updated_at = EXCLUDED.updated_at",
                self.table_name
            );

            let tags: Vec<&str> = memory.tags.iter().map(String::as_str).collect();
            let domain_str = memory.domain.to_string();
            let namespace_str = memory.namespace.as_str();
            let status_str = memory.status.as_str();

            client
                .execute(
                    &upsert,
                    &[
                        &memory.id.as_str(),
                        &memory.content,
                        &namespace_str,
                        &domain_str,
                        &status_str,
                        &tags,
                        &(memory.created_at as i64),
                        &(memory.updated_at as i64),
                    ],
                )
                .await
                .map_err(|e| query_error("postgres_index", e))?;

            Ok(())
        }

        /// Async implementation of remove operation.
        async fn remove_async(&self, id: &MemoryId) -> Result<bool> {
            let client = self.pool.get().await.map_err(pool_error)?;
            let delete = format!("DELETE FROM {} WHERE id = $1", self.table_name);
            let rows = client
                .execute(&delete, &[&id.as_str()])
                .await
                .map_err(|e| query_error("postgres_remove", e))?;
            Ok(rows > 0)
        }

        /// Async implementation of search operation.
        async fn search_async(
            &self,
            query: &str,
            filter: &SearchFilter,
            limit: usize,
        ) -> Result<Vec<(MemoryId, f32)>> {
            let client = self.pool.get().await.map_err(pool_error)?;
            let (filter_clause, filter_params) = Self::build_where_clause(filter, 2);

            let search_query = format!(
                r"SELECT id, ts_rank(search_vector, websearch_to_tsquery('english', $1)) as score
                FROM {}
                WHERE search_vector @@ websearch_to_tsquery('english', $1)
                {}
                ORDER BY score DESC
                LIMIT {}",
                self.table_name, filter_clause, limit
            );

            let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = Vec::new();
            params.push(&query);
            for p in &filter_params {
                params.push(p);
            }

            let rows = client
                .query(&search_query, &params)
                .await
                .map_err(|e| query_error("postgres_search", e))?;

            Ok(rows
                .iter()
                .map(|row| {
                    let id: String = row.get(0);
                    let score: f32 = row.get(1);
                    (MemoryId::new(&id), score)
                })
                .collect())
        }

        /// Async implementation of `list_all` operation.
        async fn list_all_async(
            &self,
            filter: &SearchFilter,
            limit: usize,
        ) -> Result<Vec<(MemoryId, f32)>> {
            let client = self.pool.get().await.map_err(pool_error)?;
            let (filter_clause, filter_params) = Self::build_where_clause(filter, 1);

            let where_prefix = if filter_clause.is_empty() {
                String::new()
            } else {
                format!("WHERE {}", filter_clause.trim_start_matches(" AND "))
            };

            let list_query = format!(
                r"SELECT id, 1.0::real as score
                FROM {}
                {}
                ORDER BY updated_at DESC
                LIMIT {}",
                self.table_name, where_prefix, limit
            );

            let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
                filter_params.iter().map(|p| p as _).collect();

            let rows = client
                .query(&list_query, &params)
                .await
                .map_err(|e| query_error("postgres_list_all", e))?;

            Ok(rows
                .iter()
                .map(|row| {
                    let id: String = row.get(0);
                    let score: f32 = row.get(1);
                    (MemoryId::new(&id), score)
                })
                .collect())
        }

        /// Async implementation of clear operation.
        async fn clear_async(&self) -> Result<()> {
            let client = self.pool.get().await.map_err(pool_error)?;
            let truncate = format!("TRUNCATE TABLE {}", self.table_name);
            client
                .execute(&truncate, &[])
                .await
                .map_err(|e| query_error("postgres_clear", e))?;
            Ok(())
        }
    }

    impl IndexBackend for PostgresIndexBackend {
        fn index(&mut self, memory: &Memory) -> Result<()> {
            self.block_on(self.index_async(memory))
        }

        fn remove(&mut self, id: &MemoryId) -> Result<bool> {
            self.block_on(self.remove_async(id))
        }

        fn search(
            &self,
            query: &str,
            filter: &SearchFilter,
            limit: usize,
        ) -> Result<Vec<(MemoryId, f32)>> {
            self.block_on(self.search_async(query, filter, limit))
        }

        fn list_all(&self, filter: &SearchFilter, limit: usize) -> Result<Vec<(MemoryId, f32)>> {
            self.block_on(self.list_all_async(filter, limit))
        }

        fn clear(&mut self) -> Result<()> {
            self.block_on(self.clear_async())
        }
    }
}

#[cfg(feature = "postgres")]
pub use implementation::PostgresIndexBackend;

#[cfg(not(feature = "postgres"))]
mod stub {
    use crate::models::{Memory, MemoryId, SearchFilter};
    use crate::storage::traits::IndexBackend;
    use crate::{Error, Result};

    /// Stub PostgreSQL backend when feature is not enabled.
    pub struct PostgresIndexBackend;

    impl PostgresIndexBackend {
        /// Creates a new PostgreSQL index backend (stub).
        ///
        /// # Errors
        ///
        /// Always returns an error because the feature is not enabled.
        pub fn new(_connection_url: &str, _table_name: impl Into<String>) -> Result<Self> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }

        /// Creates a backend with default settings (stub).
        ///
        /// # Errors
        ///
        /// Always returns an error because the feature is not enabled.
        pub fn with_defaults() -> Result<Self> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }
    }

    impl IndexBackend for PostgresIndexBackend {
        fn index(&mut self, _memory: &Memory) -> Result<()> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }

        fn remove(&mut self, _id: &MemoryId) -> Result<bool> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }

        fn search(
            &self,
            _query: &str,
            _filter: &SearchFilter,
            _limit: usize,
        ) -> Result<Vec<(MemoryId, f32)>> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }

        fn list_all(&self, _filter: &SearchFilter, _limit: usize) -> Result<Vec<(MemoryId, f32)>> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }

        fn clear(&mut self) -> Result<()> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }
    }
}

#[cfg(not(feature = "postgres"))]
pub use stub::PostgresIndexBackend;
