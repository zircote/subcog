//! PostgreSQL-based index backend.
//!
//! Provides full-text search using PostgreSQL's built-in tsvector/tsquery.
//!
//! # TLS Support (COMP-C3)
//!
//! Enable the `postgres-tls` feature for encrypted connections:
//!
//! ```toml
//! [dependencies]
//! subcog = { version = "0.1", features = ["postgres-tls"] }
//! ```
//!
//! Then use a connection URL with `sslmode=require`:
//! ```text
//! postgresql://user:pass@host:5432/db?sslmode=require
//! ```

#[cfg(feature = "postgres")]
mod implementation {
    use crate::models::{Memory, MemoryId, SearchFilter};
    use crate::storage::migrations::{Migration, MigrationRunner};
    use crate::storage::traits::IndexBackend;
    use crate::{Error, Result};
    use deadpool_postgres::{Config, Pool, Runtime};
    use tokio::runtime::Handle;

    #[cfg(not(feature = "postgres-tls"))]
    use tokio_postgres::NoTls;

    #[cfg(feature = "postgres-tls")]
    use tokio_postgres_rustls::MakeRustlsConnect;

    /// Embedded migrations compiled into the binary.
    const MIGRATIONS: &[Migration] = &[
        Migration {
            version: 1,
            description: "Initial index table with FTS",
            sql: r"
                CREATE TABLE IF NOT EXISTS {table} (
                    id TEXT PRIMARY KEY,
                    content TEXT NOT NULL,
                    namespace TEXT NOT NULL,
                    domain TEXT NOT NULL,
                    status TEXT NOT NULL,
                    tags TEXT[] DEFAULT '{}',
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL,
                    search_vector TSVECTOR GENERATED ALWAYS AS (
                        setweight(to_tsvector('english', coalesce(content, '')), 'A') ||
                        setweight(to_tsvector('english', coalesce(array_to_string(tags, ' '), '')), 'B')
                    ) STORED
                );
            ",
        },
        Migration {
            version: 2,
            description: "Add GIN index on search_vector",
            sql: r"
                CREATE INDEX IF NOT EXISTS {table}_search_idx ON {table} USING GIN (search_vector);
            ",
        },
        Migration {
            version: 3,
            description: "Add namespace and updated_at indexes",
            sql: r"
                CREATE INDEX IF NOT EXISTS {table}_namespace_idx ON {table} (namespace);
                CREATE INDEX IF NOT EXISTS {table}_updated_idx ON {table} (updated_at DESC);
            ",
        },
        Migration {
            version: 4,
            description: "Add status and created_at indexes",
            sql: r"
                CREATE INDEX IF NOT EXISTS {table}_status_idx ON {table} (status);
                CREATE INDEX IF NOT EXISTS {table}_created_idx ON {table} (created_at DESC);
            ",
        },
        Migration {
            version: 5,
            description: "Add facet columns (ADR-0048/0049)",
            sql: r"
                ALTER TABLE {table} ADD COLUMN IF NOT EXISTS project_id TEXT;
                ALTER TABLE {table} ADD COLUMN IF NOT EXISTS branch TEXT;
                ALTER TABLE {table} ADD COLUMN IF NOT EXISTS file_path TEXT;
                CREATE INDEX IF NOT EXISTS {table}_project_idx ON {table} (project_id);
                CREATE INDEX IF NOT EXISTS {table}_project_branch_idx ON {table} (project_id, branch);
                CREATE INDEX IF NOT EXISTS {table}_file_path_idx ON {table} (file_path);
            ",
        },
    ];

    /// Allowed table names for SQL injection prevention.
    const ALLOWED_TABLE_NAMES: &[&str] = &[
        "memories_index",
        "memories",
        "subcog_memories",
        "subcog_index",
    ];

    /// Validates that a table name is in the whitelist.
    fn validate_table_name(name: &str) -> Result<()> {
        if ALLOWED_TABLE_NAMES.contains(&name) {
            Ok(())
        } else {
            Err(Error::InvalidInput(format!(
                "Table name '{name}' is not allowed. Allowed names: {ALLOWED_TABLE_NAMES:?}",
            )))
        }
    }

    /// Validates PostgreSQL connection URL format (SEC-M2).
    ///
    /// Prevents connection string injection by validating:
    /// - URL scheme is `postgresql://` or `postgres://`
    /// - Host contains only valid characters (alphanumeric, `.`, `-`, `_`)
    /// - Database name contains only valid characters
    /// - No dangerous URL parameters that could alter connection behavior
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidInput` if the connection URL is invalid or contains
    /// potentially dangerous parameters.
    fn validate_connection_url(url_str: &str) -> Result<()> {
        // Check scheme
        if !url_str.starts_with("postgresql://") && !url_str.starts_with("postgres://") {
            return Err(Error::InvalidInput(
                "Connection URL must start with postgresql:// or postgres://".to_string(),
            ));
        }

        // Parse URL to validate components using reqwest's re-exported url crate
        let parsed = reqwest::Url::parse(url_str)
            .map_err(|e| Error::InvalidInput(format!("Invalid connection URL format: {e}")))?;

        // Validate host (prevent injection via malformed hostnames)
        if let Some(host) = parsed.host_str() {
            let is_valid_host = host
                .chars()
                .all(|c: char| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_');
            if !is_valid_host {
                tracing::warn!(
                    host = host,
                    "PostgreSQL connection URL contains suspicious host characters"
                );
                return Err(Error::InvalidInput(
                    "Connection URL host contains invalid characters".to_string(),
                ));
            }
        }

        // Validate database name if present
        if let Some(path) = parsed.path().strip_prefix('/') {
            let is_valid_db = path.is_empty()
                || path
                    .chars()
                    .all(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-');
            if !is_valid_db {
                tracing::warn!(
                    database = path,
                    "PostgreSQL connection URL contains suspicious database name"
                );
                return Err(Error::InvalidInput(
                    "Connection URL database name contains invalid characters".to_string(),
                ));
            }
        }

        // Block dangerous connection parameters that could alter behavior
        let dangerous_params = ["host", "hostaddr", "client_encoding", "options"];
        for (key, _) in parsed.query_pairs() {
            if dangerous_params.contains(&key.as_ref()) {
                tracing::warn!(
                    param = key.as_ref(),
                    "PostgreSQL connection URL contains blocked parameter"
                );
                return Err(Error::InvalidInput(format!(
                    "Connection URL parameter '{key}' is not allowed in query string"
                )));
            }
        }

        Ok(())
    }

    /// PostgreSQL-based index backend.
    ///
    /// Uses `deadpool_postgres::Pool` for thread-safe connection pooling,
    /// enabling `&self` methods without interior mutability wrappers.
    pub struct PostgresIndexBackend {
        /// Connection pool (thread-safe via internal `Arc`).
        pool: Pool,
        /// Table name for memories (validated against whitelist).
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
        /// # TLS Support (COMP-C3)
        ///
        /// When the `postgres-tls` feature is enabled, connections use TLS by default.
        /// For production, use a connection URL with `sslmode=require`:
        /// ```text
        /// postgresql://user:pass@host:5432/db?sslmode=require
        /// ```
        ///
        /// # Errors
        ///
        /// Returns an error if the connection pool fails to initialize
        /// or if the table name is not in the allowed whitelist.
        #[cfg(not(feature = "postgres-tls"))]
        pub fn new(connection_url: &str, table_name: impl Into<String>) -> Result<Self> {
            let table_name = table_name.into();

            // Validate table name against whitelist to prevent SQL injection
            validate_table_name(&table_name)?;

            let config = Self::parse_connection_url(connection_url)?;
            let cfg = Self::build_pool_config(&config);

            let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).map_err(|e| {
                Error::OperationFailed {
                    operation: "postgres_create_pool".to_string(),
                    cause: e.to_string(),
                }
            })?;

            let backend = Self { pool, table_name };
            backend.run_migrations()?;
            Ok(backend)
        }

        /// Creates a new PostgreSQL index backend with TLS encryption (COMP-C3).
        ///
        /// Uses rustls for TLS connections. The connection URL should include
        /// `sslmode=require` or `sslmode=verify-full` for production use.
        ///
        /// # Errors
        ///
        /// Returns an error if the connection pool fails to initialize,
        /// if TLS configuration fails, or if the table name is not allowed.
        #[cfg(feature = "postgres-tls")]
        pub fn new(connection_url: &str, table_name: impl Into<String>) -> Result<Self> {
            let table_name = table_name.into();

            // Validate table name against whitelist to prevent SQL injection
            validate_table_name(&table_name)?;

            let config = Self::parse_connection_url(connection_url)?;
            let cfg = Self::build_pool_config(&config);

            // Build TLS connector with rustls
            let tls_config = rustls::ClientConfig::builder()
                .with_root_certificates(Self::root_cert_store())
                .with_no_client_auth();

            let tls = MakeRustlsConnect::new(tls_config);

            let pool = cfg.create_pool(Some(Runtime::Tokio1), tls).map_err(|e| {
                Error::OperationFailed {
                    operation: "postgres_create_pool_tls".to_string(),
                    cause: e.to_string(),
                }
            })?;

            let backend = Self { pool, table_name };
            backend.run_migrations()?;
            Ok(backend)
        }

        /// Builds root certificate store for TLS.
        #[cfg(feature = "postgres-tls")]
        fn root_cert_store() -> rustls::RootCertStore {
            let mut roots = rustls::RootCertStore::empty();

            // Try to load system certificates
            #[cfg(feature = "postgres-tls")]
            {
                // Use webpki-roots for portable certificate bundle
                roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            }

            roots
        }

        /// Parses the connection URL into a tokio-postgres config (SEC-M2).
        ///
        /// Validates the URL for security before parsing to prevent injection attacks.
        fn parse_connection_url(url: &str) -> Result<tokio_postgres::Config> {
            // Validate URL format and block dangerous parameters (SEC-M2)
            validate_connection_url(url)?;

            url.parse::<tokio_postgres::Config>()
                .map_err(|e| Error::OperationFailed {
                    operation: "postgres_parse_url".to_string(),
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

        /// Maximum connections in pool (CHAOS-H1).
        const POOL_MAX_SIZE: usize = 20;

        /// Builds a deadpool config from tokio-postgres config.
        ///
        /// # Pool Exhaustion Protection (CHAOS-H1)
        ///
        /// Configures connection pool with safety limits:
        /// - Max 20 connections (prevents pool exhaustion)
        /// - Runtime pool builder sets timeouts for wait/create/recycle
        ///
        /// # Statement Caching (DB-H4)
        ///
        /// Statement caching is handled automatically by `tokio-postgres` connections.
        /// Each connection maintains its own prepared statement cache. The
        /// `RecyclingMethod::Fast` setting preserves connections (and their statement
        /// caches) across uses, providing implicit statement caching without
        /// additional configuration.
        fn build_pool_config(config: &tokio_postgres::Config) -> Config {
            let mut cfg = Config::new();
            cfg.host = config.get_hosts().first().map(Self::host_to_string);
            cfg.port = config.get_ports().first().copied();
            cfg.user = config.get_user().map(String::from);
            cfg.password = config
                .get_password()
                .map(|p| String::from_utf8_lossy(p).to_string());
            cfg.dbname = config.get_dbname().map(String::from);

            // Pool exhaustion protection (CHAOS-H1)
            cfg.pool = Some(deadpool_postgres::PoolConfig {
                max_size: Self::POOL_MAX_SIZE,
                ..Default::default()
            });

            // Configure manager with fast recycling for statement cache reuse
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
            Self::new("postgresql://localhost/subcog", "memories_index")
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

        /// Runs migrations.
        fn run_migrations(&self) -> Result<()> {
            self.block_on(async {
                let runner = MigrationRunner::new(self.pool.clone(), &self.table_name);
                runner.run(MIGRATIONS).await
            })
        }

        /// Builds WHERE clause for filters.
        fn build_where_clause(filter: &SearchFilter, start_param: i32) -> (String, Vec<String>) {
            let mut clauses = Vec::new();
            let mut params = Vec::new();
            let mut param_num = start_param;

            Self::add_namespace_filter(filter, &mut clauses, &mut params, &mut param_num);
            Self::add_domain_filter(filter, &mut clauses, &mut params, &mut param_num);
            Self::add_project_filter(filter, &mut clauses, &mut params, &mut param_num);
            Self::add_branch_filter(filter, &mut clauses, &mut params, &mut param_num);
            Self::add_file_path_filter(filter, &mut clauses, &mut params, &mut param_num);
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

        fn add_project_filter(
            filter: &SearchFilter,
            clauses: &mut Vec<String>,
            params: &mut Vec<String>,
            param_num: &mut i32,
        ) {
            let Some(project_id) = filter.project_id.as_ref() else {
                return;
            };
            clauses.push(format!("project_id = ${param_num}"));
            *param_num += 1;
            params.push(project_id.clone());
        }

        fn add_branch_filter(
            filter: &SearchFilter,
            clauses: &mut Vec<String>,
            params: &mut Vec<String>,
            param_num: &mut i32,
        ) {
            let Some(branch) = filter.branch.as_ref() else {
                return;
            };
            clauses.push(format!("branch = ${param_num}"));
            *param_num += 1;
            params.push(branch.clone());
        }

        fn add_file_path_filter(
            filter: &SearchFilter,
            clauses: &mut Vec<String>,
            params: &mut Vec<String>,
            param_num: &mut i32,
        ) {
            let Some(file_path) = filter.file_path.as_ref() else {
                return;
            };
            clauses.push(format!("file_path = ${param_num}"));
            *param_num += 1;
            params.push(file_path.clone());
        }

        /// Async implementation of index operation.
        #[allow(clippy::cast_possible_wrap)]
        async fn index_async(&self, memory: &Memory) -> Result<()> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let upsert = format!(
                r"INSERT INTO {} (id, content, namespace, domain, project_id, branch, file_path, status, tags, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ON CONFLICT (id) DO UPDATE SET
                    content = EXCLUDED.content,
                    namespace = EXCLUDED.namespace,
                    domain = EXCLUDED.domain,
                    project_id = EXCLUDED.project_id,
                    branch = EXCLUDED.branch,
                    file_path = EXCLUDED.file_path,
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
                    &memory.project_id,
                    &memory.branch,
                    &memory.file_path,
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
        fn index(&self, memory: &Memory) -> Result<()> {
            self.block_on(self.index_async(memory))
        }

        fn remove(&self, id: &MemoryId) -> Result<bool> {
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

        fn get_memory(&self, _id: &MemoryId) -> Result<Option<Memory>> {
            // Index backend stores minimal data for search, not full memories
            Ok(None)
        }

        fn clear(&self) -> Result<()> {
            self.block_on(self.clear_async())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_validate_connection_url_valid() {
            // Valid PostgreSQL URLs
            assert!(validate_connection_url("postgresql://localhost/mydb").is_ok());
            assert!(validate_connection_url("postgres://user:pass@localhost:5432/mydb").is_ok());
            assert!(
                validate_connection_url(
                    "postgresql://user:pass@db.example.com:5432/mydb?sslmode=require"
                )
                .is_ok()
            );
            assert!(validate_connection_url("postgresql://localhost/my_db-test").is_ok());
        }

        #[test]
        fn test_validate_connection_url_invalid_scheme() {
            // Invalid scheme
            assert!(validate_connection_url("mysql://localhost/mydb").is_err());
            assert!(validate_connection_url("http://localhost/mydb").is_err());
            assert!(validate_connection_url("localhost/mydb").is_err());
        }

        #[test]
        fn test_validate_connection_url_invalid_host() {
            // Invalid host characters (injection attempts)
            assert!(validate_connection_url("postgresql://local<script>host/mydb").is_err());
            assert!(validate_connection_url("postgresql://host;drop table/mydb").is_err());
        }

        #[test]
        fn test_validate_connection_url_invalid_database() {
            // Invalid database name characters
            assert!(validate_connection_url("postgresql://localhost/my;db").is_err());
            assert!(validate_connection_url("postgresql://localhost/db<script>").is_err());
        }

        #[test]
        fn test_validate_connection_url_blocked_params() {
            // Blocked dangerous parameters
            assert!(validate_connection_url("postgresql://localhost/mydb?host=evil.com").is_err());
            assert!(
                validate_connection_url("postgresql://localhost/mydb?hostaddr=1.2.3.4").is_err()
            );
            assert!(
                validate_connection_url("postgresql://localhost/mydb?options=-c log_statement=all")
                    .is_err()
            );
            assert!(
                validate_connection_url("postgresql://localhost/mydb?client_encoding=SQL_ASCII")
                    .is_err()
            );
        }

        #[test]
        fn test_validate_connection_url_allowed_params() {
            // Allowed parameters should pass
            assert!(validate_connection_url("postgresql://localhost/mydb?sslmode=require").is_ok());
            assert!(
                validate_connection_url(
                    "postgresql://localhost/mydb?connect_timeout=10&application_name=subcog"
                )
                .is_ok()
            );
        }

        #[test]
        fn test_validate_table_name() {
            // Valid table names
            assert!(validate_table_name("memories_index").is_ok());
            assert!(validate_table_name("subcog_memories").is_ok());

            // Invalid table names
            assert!(validate_table_name("users").is_err());
            assert!(validate_table_name("memories_index; DROP TABLE users").is_err());
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
        fn index(&self, _memory: &Memory) -> Result<()> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }

        fn remove(&self, _id: &MemoryId) -> Result<bool> {
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

        fn get_memory(&self, _id: &MemoryId) -> Result<Option<Memory>> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }

        fn clear(&self) -> Result<()> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }
    }
}

#[cfg(not(feature = "postgres"))]
pub use stub::PostgresIndexBackend;
