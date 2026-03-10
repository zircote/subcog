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
    use crate::models::{Domain, Memory, MemoryId, MemoryStatus, Namespace, SearchFilter};
    use crate::models::graph::{
        Entity, EntityId, EntityMention, EntityQuery, EntityType, Relationship,
        RelationshipQuery, RelationshipType, TraversalResult,
    };
    use crate::models::temporal::{BitemporalPoint, TransactionTime, ValidTimeRange};
    use crate::storage::migrations::{Migration, MigrationRunner};
    use crate::storage::traits::graph::{GraphBackend, GraphStats};
    use crate::storage::traits::{IndexBackend, VectorBackend, VectorFilter};
    use crate::{Error, Result};
    use chrono::TimeZone;
    use std::collections::HashMap;
    use deadpool_postgres::{Config, Pool, Runtime};
    use tokio::runtime::Handle;

    #[cfg(not(feature = "postgres-tls"))]
    use tokio_postgres::NoTls;

    #[cfg(feature = "postgres-tls")]
    use tokio_postgres_rustls::MakeRustlsConnect;

    /// Embedded migrations compiled into the binary.
    ///
    /// Migration v1 uses a trigger-based approach for the `search_vector` column
    /// instead of `GENERATED ALWAYS AS` because `to_tsvector()` is STABLE, not
    /// IMMUTABLE, and PostgreSQL 18+ enforces immutability for generated columns.
    /// The trigger approach works across all PostgreSQL versions (12+).
    const MIGRATIONS: &[Migration] = &[
        Migration {
            version: 1,
            description: "Initial memories table with FTS and indexes",
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
                    search_vector TSVECTOR,
                    project_id TEXT,
                    branch TEXT,
                    file_path TEXT,
                    source TEXT,
                    tombstoned_at BIGINT,
                    expires_at BIGINT,
                    is_summary BOOLEAN DEFAULT FALSE,
                    source_memory_ids JSONB,
                    consolidation_timestamp BIGINT
                );

                CREATE OR REPLACE FUNCTION {table}_search_vector_update() RETURNS trigger AS $$
                BEGIN
                    NEW.search_vector :=
                        setweight(to_tsvector('english', coalesce(NEW.content, '')), 'A') ||
                        setweight(to_tsvector('english', coalesce(array_to_string(NEW.tags, ' '), '')), 'B');
                    RETURN NEW;
                END
                $$ LANGUAGE plpgsql;

                DROP TRIGGER IF EXISTS {table}_search_vector_trigger ON {table};
                CREATE TRIGGER {table}_search_vector_trigger
                    BEFORE INSERT OR UPDATE ON {table}
                    FOR EACH ROW EXECUTE FUNCTION {table}_search_vector_update();

                CREATE INDEX IF NOT EXISTS {table}_search_idx ON {table} USING GIN (search_vector);
                CREATE INDEX IF NOT EXISTS {table}_namespace_idx ON {table} (namespace);
                CREATE INDEX IF NOT EXISTS {table}_updated_idx ON {table} (updated_at DESC);
                CREATE INDEX IF NOT EXISTS {table}_status_idx ON {table} (status);
                CREATE INDEX IF NOT EXISTS {table}_created_idx ON {table} (created_at DESC);
                CREATE INDEX IF NOT EXISTS {table}_project_idx ON {table} (project_id);
                CREATE INDEX IF NOT EXISTS {table}_project_branch_idx ON {table} (project_id, branch);
                CREATE INDEX IF NOT EXISTS {table}_file_path_idx ON {table} (file_path);
                CREATE INDEX IF NOT EXISTS {table}_tombstoned_idx ON {table} (tombstoned_at) WHERE tombstoned_at IS NOT NULL;
                CREATE INDEX IF NOT EXISTS {table}_expires_idx ON {table} (expires_at) WHERE expires_at IS NOT NULL;

                CREATE EXTENSION IF NOT EXISTS vector;
                CREATE TABLE IF NOT EXISTS {vector_table} (
                    id TEXT PRIMARY KEY,
                    embedding vector(384),
                    created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT
                );
                CREATE INDEX IF NOT EXISTS {vector_table}_embedding_idx
                    ON {vector_table} USING hnsw (embedding vector_cosine_ops)
                    WITH (m = 16, ef_construction = 64);

                -- Graph tables
                CREATE TABLE IF NOT EXISTS graph_entities (
                    id TEXT PRIMARY KEY,
                    entity_type TEXT NOT NULL,
                    name TEXT NOT NULL,
                    aliases TEXT,
                    domain_org TEXT,
                    domain_project TEXT,
                    domain_repo TEXT,
                    confidence DOUBLE PRECISION NOT NULL DEFAULT 1.0,
                    valid_time_start BIGINT,
                    valid_time_end BIGINT,
                    transaction_time BIGINT NOT NULL,
                    properties TEXT,
                    mention_count INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS graph_relationships (
                    from_entity_id TEXT NOT NULL,
                    to_entity_id TEXT NOT NULL,
                    relationship_type TEXT NOT NULL,
                    confidence DOUBLE PRECISION NOT NULL DEFAULT 1.0,
                    valid_time_start BIGINT,
                    valid_time_end BIGINT,
                    transaction_time BIGINT NOT NULL,
                    properties TEXT,
                    PRIMARY KEY (from_entity_id, to_entity_id, relationship_type),
                    FOREIGN KEY (from_entity_id) REFERENCES graph_entities(id) ON DELETE CASCADE,
                    FOREIGN KEY (to_entity_id) REFERENCES graph_entities(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS graph_entity_mentions (
                    entity_id TEXT NOT NULL,
                    memory_id TEXT NOT NULL,
                    confidence DOUBLE PRECISION NOT NULL DEFAULT 1.0,
                    start_offset BIGINT,
                    end_offset BIGINT,
                    matched_text TEXT,
                    transaction_time BIGINT NOT NULL,
                    PRIMARY KEY (entity_id, memory_id),
                    FOREIGN KEY (entity_id) REFERENCES graph_entities(id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_graph_entities_type ON graph_entities(entity_type);
                CREATE INDEX IF NOT EXISTS idx_graph_entities_name ON graph_entities(name);
                CREATE INDEX IF NOT EXISTS idx_graph_entities_domain ON graph_entities(domain_org, domain_project, domain_repo);
                CREATE INDEX IF NOT EXISTS idx_graph_entities_confidence ON graph_entities(confidence DESC);
                CREATE INDEX IF NOT EXISTS idx_graph_entities_mention_count ON graph_entities(mention_count DESC);

                CREATE INDEX IF NOT EXISTS idx_graph_relationships_from ON graph_relationships(from_entity_id);
                CREATE INDEX IF NOT EXISTS idx_graph_relationships_to ON graph_relationships(to_entity_id);
                CREATE INDEX IF NOT EXISTS idx_graph_relationships_type ON graph_relationships(relationship_type);

                CREATE INDEX IF NOT EXISTS idx_graph_entity_mentions_entity ON graph_entity_mentions(entity_id);
                CREATE INDEX IF NOT EXISTS idx_graph_entity_mentions_memory ON graph_entity_mentions(memory_id)
            ",
        },
    ];

    /// Allowed table names for SQL injection prevention.
    const ALLOWED_TABLE_NAMES: &[&str] = &[
        "memories",
        "subcog_memories",
        "org_memories_index",
    ];

    /// Allowed vector table names.
    const ALLOWED_VECTOR_TABLE_NAMES: &[&str] = &[
        "memory_vectors",
        "subcog_vectors",
        "org_memory_vectors",
    ];

    /// Validates that a table name is in the whitelist.
    fn validate_table_name(name: &str) -> Result<()> {
        if ALLOWED_TABLE_NAMES.contains(&name) || ALLOWED_VECTOR_TABLE_NAMES.contains(&name) {
            Ok(())
        } else {
            let all: Vec<&&str> = ALLOWED_TABLE_NAMES.iter().chain(ALLOWED_VECTOR_TABLE_NAMES.iter()).collect();
            Err(Error::InvalidInput(format!(
                "Table name '{name}' is not allowed. Allowed names: {all:?}",
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
    pub struct PostgresBackend {
        /// Connection pool (thread-safe via internal `Arc`).
        pool: Pool,
        /// Table name for memories (validated against whitelist).
        table_name: String,
        /// Table name for vector embeddings.
        vector_table_name: String,
        /// Embedding dimensions.
        dimensions: usize,
    }

    /// Helper to map pool errors.
    fn pool_error(e: impl std::fmt::Debug) -> Error {
        Error::OperationFailed {
            operation: "postgres_get_client".to_string(),
            cause: format!("{e:?}"),
        }
    }

    /// Helper to map query errors.
    fn query_error(op: &str, e: impl std::fmt::Debug) -> Error {
        Error::OperationFailed {
            operation: op.to_string(),
            cause: format!("{e:?}"),
        }
    }

    impl PostgresBackend {
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
        pub fn new(
            connection_url: &str,
            table_name: impl Into<String>,
            vector_table_name: impl Into<String>,
        ) -> Result<Self> {
            Self::with_pool_size(connection_url, table_name, vector_table_name, None)
        }

        /// Creates a new PostgreSQL backend with a custom pool size.
        ///
        /// Initializes both the memories table (FTS) and vector table (pgvector)
        /// using a single shared connection pool.
        ///
        /// # Errors
        ///
        /// Returns an error if the connection pool fails to initialize
        /// or if table names are not in the allowed whitelist.
        #[cfg(not(feature = "postgres-tls"))]
        pub fn with_pool_size(
            connection_url: &str,
            table_name: impl Into<String>,
            vector_table_name: impl Into<String>,
            pool_max_size: Option<usize>,
        ) -> Result<Self> {
            let table_name = table_name.into();
            let vector_table_name = vector_table_name.into();

            validate_table_name(&table_name)?;
            validate_table_name(&vector_table_name)?;

            let config = Self::parse_connection_url(connection_url)?;
            let cfg = Self::build_pool_config(&config, pool_max_size);

            let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).map_err(|e| {
                Error::OperationFailed {
                    operation: "postgres_create_pool".to_string(),
                    cause: e.to_string(),
                }
            })?;

            let dimensions = crate::embedding::DEFAULT_DIMENSIONS;
            let backend = Self { pool, table_name, vector_table_name, dimensions };
            backend.run_migrations()?;
            Ok(backend)
        }

        /// Creates a new PostgreSQL backend with TLS encryption (COMP-C3).
        ///
        /// # Errors
        ///
        /// Returns an error if the connection pool fails to initialize,
        /// if TLS configuration fails, or if table names are not allowed.
        #[cfg(feature = "postgres-tls")]
        pub fn new(
            connection_url: &str,
            table_name: impl Into<String>,
            vector_table_name: impl Into<String>,
        ) -> Result<Self> {
            Self::with_pool_size(connection_url, table_name, vector_table_name, None)
        }

        /// Creates a new PostgreSQL backend with TLS and a custom pool size.
        ///
        /// # Errors
        ///
        /// Returns an error if the connection pool fails to initialize,
        /// if TLS configuration fails, or if table names are not allowed.
        #[cfg(feature = "postgres-tls")]
        pub fn with_pool_size(
            connection_url: &str,
            table_name: impl Into<String>,
            vector_table_name: impl Into<String>,
            pool_max_size: Option<usize>,
        ) -> Result<Self> {
            let table_name = table_name.into();
            let vector_table_name = vector_table_name.into();

            validate_table_name(&table_name)?;
            validate_table_name(&vector_table_name)?;

            let config = Self::parse_connection_url(connection_url)?;
            let cfg = Self::build_pool_config(&config, pool_max_size);

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

            let dimensions = crate::embedding::DEFAULT_DIMENSIONS;
            let backend = Self { pool, table_name, vector_table_name, dimensions };
            backend.run_migrations()?;
            Ok(backend)
        }

        /// Builds root certificate store for TLS.
        #[cfg(feature = "postgres-tls")]
        fn root_cert_store() -> rustls::RootCertStore {
            let mut roots = rustls::RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
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
        fn build_pool_config(config: &tokio_postgres::Config, pool_max_size: Option<usize>) -> Config {
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
                max_size: pool_max_size.unwrap_or(Self::POOL_MAX_SIZE),
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
            Self::new("postgresql://localhost/subcog", "memories", "memory_vectors")
        }

        /// Returns the embedding dimensions.
        #[must_use]
        pub fn dimensions(&self) -> usize {
            self.dimensions
        }

        /// Runs a blocking operation on the async pool.
        fn block_on<F, T>(&self, f: F) -> Result<T>
        where
            F: std::future::Future<Output = Result<T>>,
        {
            if let Ok(handle) = Handle::try_current() {
                tokio::task::block_in_place(|| handle.block_on(f))
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

        /// Runs migrations for all PostgreSQL tables.
        fn run_migrations(&self) -> Result<()> {
            self.block_on(async {
                let runner = MigrationRunner::new(self.pool.clone(), &self.table_name)
                    .with_replacement("{vector_table}", &self.vector_table_name);
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
                r"INSERT INTO {} (id, content, namespace, domain, project_id, branch, file_path, status, tags, created_at, updated_at, source, tombstoned_at, expires_at, is_summary, source_memory_ids, consolidation_timestamp)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
                ON CONFLICT (id) DO UPDATE SET
                    content = EXCLUDED.content,
                    namespace = EXCLUDED.namespace,
                    domain = EXCLUDED.domain,
                    project_id = EXCLUDED.project_id,
                    branch = EXCLUDED.branch,
                    file_path = EXCLUDED.file_path,
                    status = EXCLUDED.status,
                    tags = EXCLUDED.tags,
                    updated_at = EXCLUDED.updated_at,
                    source = EXCLUDED.source,
                    tombstoned_at = EXCLUDED.tombstoned_at,
                    expires_at = EXCLUDED.expires_at,
                    is_summary = EXCLUDED.is_summary,
                    source_memory_ids = EXCLUDED.source_memory_ids,
                    consolidation_timestamp = EXCLUDED.consolidation_timestamp",
                self.table_name
            );

            let tags: Vec<&str> = memory.tags.iter().map(String::as_str).collect();
            let domain_str = serde_json::to_string(&memory.domain)
                .unwrap_or_else(|_| memory.domain.to_string());
            let namespace_str = memory.namespace.as_str();
            let status_str = memory.status.as_str();
            #[allow(clippy::cast_possible_wrap)]
            let created_at = memory.created_at as i64;
            #[allow(clippy::cast_possible_wrap)]
            let updated_at = memory.updated_at as i64;
            let tombstoned_at = memory.tombstoned_at.map(|t| t.timestamp());
            #[allow(clippy::cast_possible_wrap)]
            let expires_at = memory.expires_at.map(|t| t as i64);
            #[allow(clippy::cast_possible_wrap)]
            let consolidation_ts = memory.consolidation_timestamp.map(|t| t as i64);
            let source_memory_ids_json: Option<serde_json::Value> = memory
                .source_memory_ids
                .as_ref()
                .map(|ids| {
                    let strs: Vec<&str> = ids.iter().map(|id| id.as_str()).collect();
                    serde_json::Value::Array(strs.into_iter().map(|s| serde_json::Value::String(s.to_string())).collect())
                });

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
                        &created_at,
                        &updated_at,
                        &memory.source,
                        &tombstoned_at,
                        &expires_at,
                        &memory.is_summary,
                        &source_memory_ids_json,
                        &consolidation_ts,
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

        /// Retrieves a single memory by ID.
        async fn get_memory_async(&self, id: &MemoryId) -> Result<Option<Memory>> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let query = format!(
                r"SELECT id, content, namespace, domain, project_id, branch, file_path,
                         status, tags, created_at, updated_at, source, tombstoned_at,
                         expires_at, is_summary, source_memory_ids, consolidation_timestamp
                  FROM {}
                  WHERE id = $1",
                self.table_name
            );

            let row = client
                .query_opt(&query, &[&id.as_str()])
                .await
                .map_err(|e| query_error("postgres_get_memory", e))?;

            Ok(row.map(|r| Self::row_to_memory(&r)))
        }

        /// Retrieves multiple memories by ID in a single query.
        async fn get_memories_batch_async(&self, ids: &[MemoryId]) -> Result<Vec<Option<Memory>>> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let id_strs: Vec<&str> = ids.iter().map(MemoryId::as_str).collect();

            let query = format!(
                r"SELECT id, content, namespace, domain, project_id, branch, file_path,
                         status, tags, created_at, updated_at, source, tombstoned_at,
                         expires_at, is_summary, source_memory_ids, consolidation_timestamp
                  FROM {}
                  WHERE id = ANY($1)",
                self.table_name
            );

            let rows = client
                .query(&query, &[&id_strs])
                .await
                .map_err(|e| query_error("postgres_get_memories_batch", e))?;

            // Build a map for O(1) lookup
            let mut memory_map: std::collections::HashMap<String, Memory> =
                std::collections::HashMap::with_capacity(rows.len());
            for row in &rows {
                let memory = Self::row_to_memory(row);
                memory_map.insert(memory.id.as_str().to_string(), memory);
            }

            // Return in the same order as input IDs
            Ok(ids
                .iter()
                .map(|id| memory_map.remove(id.as_str()))
                .collect())
        }

        /// Async implementation of list_ids operation.
        async fn list_ids_async(&self) -> Result<Vec<MemoryId>> {
            let client = self.pool.get().await.map_err(pool_error)?;
            let query = format!("SELECT id FROM {}", self.table_name);
            let rows = client
                .query(&query, &[])
                .await
                .map_err(|e| query_error("postgres_list_ids", e))?;

            Ok(rows.iter().map(|row| {
                let id: String = row.get(0);
                MemoryId::new(id)
            }).collect())
        }

        /// Converts a PostgreSQL row into a `Memory`.
        #[allow(clippy::cast_sign_loss)]
        fn row_to_memory(row: &tokio_postgres::Row) -> Memory {
            let id: String = row.get(0);
            let content: String = row.get(1);
            let namespace_str: String = row.get(2);
            let domain_str: String = row.get(3);
            let project_id: Option<String> = row.get(4);
            let branch: Option<String> = row.get(5);
            let file_path: Option<String> = row.get(6);
            let status_str: String = row.get(7);
            let tags: Vec<String> = row.get::<_, Option<Vec<String>>>(8).unwrap_or_default();
            let created_at: i64 = row.get(9);
            let updated_at: i64 = row.get(10);
            let source: Option<String> = row.get(11);
            let tombstoned_at_epoch: Option<i64> = row.get(12);
            let expires_at_i64: Option<i64> = row.get(13);
            let is_summary: bool = row.get::<_, Option<bool>>(14).unwrap_or(false);
            let source_memory_ids_json: Option<serde_json::Value> = row.get(15);
            let consolidation_ts: Option<i64> = row.get(16);

            let namespace = Namespace::parse(&namespace_str).unwrap_or_default();
            let domain = serde_json::from_str::<Domain>(&domain_str).unwrap_or_default();

            let status = match status_str.to_lowercase().as_str() {
                "active" => MemoryStatus::Active,
                "archived" => MemoryStatus::Archived,
                "superseded" => MemoryStatus::Superseded,
                "pending" => MemoryStatus::Pending,
                "deleted" => MemoryStatus::Deleted,
                "tombstoned" => MemoryStatus::Tombstoned,
                "consolidated" => MemoryStatus::Consolidated,
                _ => MemoryStatus::Active,
            };

            let tombstoned_at = tombstoned_at_epoch
                .and_then(|ts| chrono::Utc.timestamp_opt(ts, 0).single());

            let source_memory_ids = source_memory_ids_json.and_then(|v| {
                v.as_array().map(|arr| {
                    arr.iter()
                        .filter_map(|s| s.as_str().map(|s| MemoryId::new(s)))
                        .collect()
                })
            });

            Memory {
                id: MemoryId::new(id),
                content,
                namespace,
                domain,
                project_id,
                branch,
                file_path,
                status,
                created_at: created_at as u64,
                updated_at: updated_at as u64,
                tombstoned_at,
                expires_at: expires_at_i64.map(|t| t as u64),
                embedding: None,
                tags,
                #[cfg(feature = "group-scope")]
                group_id: None,
                source,
                is_summary,
                source_memory_ids,
                consolidation_timestamp: consolidation_ts.map(|t| t as u64),
            }
        }
    }

    impl IndexBackend for PostgresBackend {
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

        fn get_memory(&self, id: &MemoryId) -> Result<Option<Memory>> {
            self.block_on(self.get_memory_async(id))
        }

        fn get_memories_batch(&self, ids: &[MemoryId]) -> Result<Vec<Option<Memory>>> {
            if ids.is_empty() {
                return Ok(Vec::new());
            }
            self.block_on(self.get_memories_batch_async(ids))
        }

        fn clear(&self) -> Result<()> {
            self.block_on(self.clear_async())
        }
    }

    // Implement PersistenceBackend for PostgresBackend so it can be used
    // with ConsolidationService (same pattern as SqliteBackend).
    impl crate::storage::traits::PersistenceBackend for PostgresBackend {
        fn store(&self, memory: &Memory) -> Result<()> {
            self.block_on(self.index_async(memory))
        }

        fn get(&self, id: &MemoryId) -> Result<Option<Memory>> {
            self.block_on(self.get_memory_async(id))
        }

        fn delete(&self, id: &MemoryId) -> Result<bool> {
            self.block_on(self.remove_async(id))
        }

        fn list_ids(&self) -> Result<Vec<MemoryId>> {
            self.block_on(self.list_ids_async())
        }
    }

    // --- Vector backend methods ---

    impl PostgresBackend {
        /// Formats an embedding as a pgvector string: `'[1.0,2.0,3.0]'`.
        fn format_embedding(embedding: &[f32]) -> String {
            let values: Vec<String> = embedding
                .iter()
                .map(std::string::ToString::to_string)
                .collect();
            format!("[{}]", values.join(","))
        }

        /// Async implementation of vector upsert.
        async fn vector_upsert_async(&self, id: &MemoryId, embedding: &[f32]) -> Result<()> {
            let client = self.pool.get().await.map_err(pool_error)?;
            let embedding_str = Self::format_embedding(embedding);

            let upsert = format!(
                r"INSERT INTO {} (id, embedding)
                VALUES ($1, $2::text::vector)
                ON CONFLICT (id) DO UPDATE SET
                    embedding = EXCLUDED.embedding",
                self.vector_table_name
            );

            client
                .execute(&upsert, &[&id.as_str(), &embedding_str])
                .await
                .map_err(|e| query_error("postgres_vector_upsert", e))?;

            Ok(())
        }

        /// Async implementation of vector remove.
        async fn vector_remove_async(&self, id: &MemoryId) -> Result<bool> {
            let client = self.pool.get().await.map_err(pool_error)?;
            let delete = format!("DELETE FROM {} WHERE id = $1", self.vector_table_name);
            let rows = client
                .execute(&delete, &[&id.as_str()])
                .await
                .map_err(|e| query_error("postgres_vector_remove", e))?;
            Ok(rows > 0)
        }

        /// Async implementation of vector search.
        /// Returns cosine similarity (1 - cosine_distance).
        async fn vector_search_async(
            &self,
            query_embedding: &[f32],
            filter: &VectorFilter,
            limit: usize,
        ) -> Result<Vec<(MemoryId, f32)>> {
            let client = self.pool.get().await.map_err(pool_error)?;
            let embedding_str = Self::format_embedding(query_embedding);

            let (namespace_join, namespace_params) =
                self.build_vector_namespace_filter(filter);

            let search_query = format!(
                r"SELECT v.id, 1 - (v.embedding <=> $1::text::vector) as similarity
                FROM {} v
                {}
                ORDER BY v.embedding <=> $1::text::vector
                LIMIT {}",
                self.vector_table_name, namespace_join, limit
            );

            let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = Vec::new();
            params.push(&embedding_str);
            for p in &namespace_params {
                params.push(p);
            }

            let rows = client
                .query(&search_query, &params)
                .await
                .map_err(|e| query_error("postgres_vector_search", e))?;

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

        /// Builds namespace filter by joining with the memories table.
        /// The vector table has no namespace column — namespace lives on memories.
        fn build_vector_namespace_filter(&self, filter: &VectorFilter) -> (String, Vec<String>) {
            if filter.namespaces.is_empty() {
                return (String::new(), Vec::new());
            }

            let placeholders: Vec<String> = filter
                .namespaces
                .iter()
                .enumerate()
                .map(|(i, _)| format!("${}", i + 2))
                .collect();

            let join = format!(
                "JOIN {} m ON m.id = v.id WHERE m.namespace IN ({})",
                self.table_name,
                placeholders.join(", ")
            );
            let params: Vec<String> = filter
                .namespaces
                .iter()
                .map(|ns| ns.as_str().to_string())
                .collect();

            (join, params)
        }

        /// Async implementation of vector count.
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        async fn vector_count_async(&self) -> Result<usize> {
            let client = self.pool.get().await.map_err(pool_error)?;
            let query = format!("SELECT COUNT(*) FROM {}", self.vector_table_name);
            let row = client
                .query_one(&query, &[])
                .await
                .map_err(|e| query_error("postgres_vector_count", e))?;
            let count: i64 = row.get(0);
            Ok(count as usize)
        }

        /// Async implementation of vector clear.
        async fn vector_clear_async(&self) -> Result<()> {
            let client = self.pool.get().await.map_err(pool_error)?;
            let truncate = format!("TRUNCATE TABLE {}", self.vector_table_name);
            client
                .execute(&truncate, &[])
                .await
                .map_err(|e| query_error("postgres_vector_clear", e))?;
            Ok(())
        }
    }

    impl VectorBackend for PostgresBackend {
        fn dimensions(&self) -> usize {
            self.dimensions
        }

        fn upsert(&self, id: &MemoryId, embedding: &[f32]) -> Result<()> {
            self.block_on(self.vector_upsert_async(id, embedding))
        }

        fn remove(&self, id: &MemoryId) -> Result<bool> {
            self.block_on(self.vector_remove_async(id))
        }

        fn search(
            &self,
            query_embedding: &[f32],
            filter: &VectorFilter,
            limit: usize,
        ) -> Result<Vec<(MemoryId, f32)>> {
            self.block_on(self.vector_search_async(query_embedding, filter, limit))
        }

        fn count(&self) -> Result<usize> {
            self.block_on(self.vector_count_async())
        }

        fn clear(&self) -> Result<()> {
            self.block_on(self.vector_clear_async())
        }
    }

    // ========================================================================
    // Graph helper methods on PostgresBackend
    // ========================================================================

    impl PostgresBackend {
        /// Parses an entity from a tokio_postgres Row.
        fn parse_entity(row: &tokio_postgres::Row) -> Entity {
            let id: String = row.get("id");
            let entity_type_str: String = row.get("entity_type");
            let name: String = row.get("name");
            let aliases_json: Option<String> = row.get("aliases");
            let domain_org: Option<String> = row.get("domain_org");
            let domain_project: Option<String> = row.get("domain_project");
            let domain_repo: Option<String> = row.get("domain_repo");
            let confidence: f64 = row.get("confidence");
            let valid_time_start: Option<i64> = row.get("valid_time_start");
            let valid_time_end: Option<i64> = row.get("valid_time_end");
            let transaction_time: i64 = row.get("transaction_time");
            let properties_json: Option<String> = row.get("properties");
            let mention_count: i32 = row.get("mention_count");

            let entity_type = EntityType::parse(&entity_type_str).unwrap_or(EntityType::Concept);
            let aliases: Vec<String> = aliases_json
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            let properties: HashMap<String, String> = properties_json
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

            Entity {
                id: EntityId::new(id),
                entity_type,
                name,
                aliases,
                domain: Domain {
                    organization: domain_org,
                    project: domain_project,
                    repository: domain_repo,
                },
                confidence: confidence as f32,
                valid_time: ValidTimeRange {
                    start: valid_time_start,
                    end: valid_time_end,
                },
                transaction_time: TransactionTime::at(transaction_time),
                properties,
                mention_count: mention_count as u32,
            }
        }

        /// Parses a relationship from a tokio_postgres Row.
        fn parse_relationship(row: &tokio_postgres::Row) -> Relationship {
            let from_entity_id: String = row.get("from_entity_id");
            let to_entity_id: String = row.get("to_entity_id");
            let relationship_type_str: String = row.get("relationship_type");
            let confidence: f64 = row.get("confidence");
            let valid_time_start: Option<i64> = row.get("valid_time_start");
            let valid_time_end: Option<i64> = row.get("valid_time_end");

            let relationship_type =
                RelationshipType::parse(&relationship_type_str).unwrap_or(RelationshipType::RelatesTo);

            Relationship::new(
                EntityId::new(from_entity_id),
                EntityId::new(to_entity_id),
                relationship_type,
            )
            .with_confidence(confidence as f32)
            .with_valid_time(ValidTimeRange {
                start: valid_time_start,
                end: valid_time_end,
            })
        }

        /// Parses a mention from a tokio_postgres Row.
        fn parse_mention(row: &tokio_postgres::Row) -> EntityMention {
            let entity_id: String = row.get("entity_id");
            let memory_id: String = row.get("memory_id");
            let confidence: f64 = row.get("confidence");
            let start_offset: Option<i64> = row.get("start_offset");
            let end_offset: Option<i64> = row.get("end_offset");
            let matched_text: Option<String> = row.get("matched_text");
            let tx_time: i64 = row.get("transaction_time");

            EntityMention {
                entity_id: EntityId::new(entity_id),
                memory_id: MemoryId::new(memory_id),
                confidence: confidence as f32,
                start_offset: start_offset.map(|v| v as usize),
                end_offset: end_offset.map(|v| v as usize),
                matched_text,
                transaction_time: TransactionTime::at(tx_time),
            }
        }

        /// Builds WHERE clause for entity queries.
        fn build_entity_where(
            query: &EntityQuery,
        ) -> (String, Vec<Box<dyn tokio_postgres::types::ToSql + Sync>>) {
            let mut conditions = Vec::new();
            let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync>> = Vec::new();

            if let Some(ref entity_type) = query.entity_type {
                params.push(Box::new(entity_type.as_str().to_string()));
                conditions.push(format!("entity_type = ${}", params.len()));
            }
            if let Some(ref name) = query.name {
                params.push(Box::new(format!("%{name}%")));
                conditions.push(format!("name LIKE ${}", params.len()));
            }
            if let Some(ref domain) = query.domain {
                if let Some(ref org) = domain.organization {
                    params.push(Box::new(org.clone()));
                    conditions.push(format!("domain_org = ${}", params.len()));
                }
                if let Some(ref project) = domain.project {
                    params.push(Box::new(project.clone()));
                    conditions.push(format!("domain_project = ${}", params.len()));
                }
                if let Some(ref repo) = domain.repository {
                    params.push(Box::new(repo.clone()));
                    conditions.push(format!("domain_repo = ${}", params.len()));
                }
            }
            if let Some(min_confidence) = query.min_confidence {
                params.push(Box::new(f64::from(min_confidence)));
                conditions.push(format!("confidence >= ${}", params.len()));
            }

            let where_clause = if conditions.is_empty() {
                String::new()
            } else {
                format!("WHERE {}", conditions.join(" AND "))
            };

            (where_clause, params)
        }

        // ====================================================================
        // Graph async implementations
        // ====================================================================

        async fn store_entity_async(&self, entity: &Entity) -> Result<()> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let aliases_json =
                serde_json::to_string(&entity.aliases).unwrap_or_else(|_| "[]".to_string());
            let properties_json =
                serde_json::to_string(&entity.properties).unwrap_or_else(|_| "{}".to_string());

            client
                .execute(
                    "INSERT INTO graph_entities (
                        id, entity_type, name, aliases, domain_org, domain_project, domain_repo,
                        confidence, valid_time_start, valid_time_end, transaction_time, properties, mention_count
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
                    ON CONFLICT(id) DO UPDATE SET
                        entity_type = EXCLUDED.entity_type,
                        name = EXCLUDED.name,
                        aliases = EXCLUDED.aliases,
                        domain_org = EXCLUDED.domain_org,
                        domain_project = EXCLUDED.domain_project,
                        domain_repo = EXCLUDED.domain_repo,
                        confidence = EXCLUDED.confidence,
                        valid_time_start = EXCLUDED.valid_time_start,
                        valid_time_end = EXCLUDED.valid_time_end,
                        properties = EXCLUDED.properties,
                        mention_count = EXCLUDED.mention_count",
                    &[
                        &entity.id.as_str(),
                        &entity.entity_type.as_str(),
                        &entity.name.as_str(),
                        &aliases_json.as_str(),
                        &entity.domain.organization.as_deref() as &(dyn tokio_postgres::types::ToSql + Sync),
                        &entity.domain.project.as_deref() as &(dyn tokio_postgres::types::ToSql + Sync),
                        &entity.domain.repository.as_deref() as &(dyn tokio_postgres::types::ToSql + Sync),
                        &f64::from(entity.confidence),
                        &entity.valid_time.start as &(dyn tokio_postgres::types::ToSql + Sync),
                        &entity.valid_time.end as &(dyn tokio_postgres::types::ToSql + Sync),
                        &entity.transaction_time.timestamp(),
                        &properties_json.as_str(),
                        &(entity.mention_count as i32),
                    ],
                )
                .await
                .map_err(|e| query_error("store_entity", e))?;

            Ok(())
        }

        async fn get_entity_async(&self, id: &EntityId) -> Result<Option<Entity>> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let rows = client
                .query(
                    "SELECT * FROM graph_entities WHERE id = $1",
                    &[&id.as_str()],
                )
                .await
                .map_err(|e| query_error("get_entity", e))?;

            Ok(rows.first().map(Self::parse_entity))
        }

        async fn query_entities_async(&self, query: &EntityQuery) -> Result<Vec<Entity>> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let (where_clause, params) = Self::build_entity_where(query);
            let limit = query.limit.unwrap_or(100);

            let sql = format!(
                "SELECT * FROM graph_entities {} ORDER BY mention_count DESC, confidence DESC LIMIT {}",
                where_clause, limit
            );

            let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
                params.iter().map(|p| p.as_ref()).collect();

            let rows = client.query(&sql, &param_refs).await.map_err(|e| query_error("query_entities", e))?;

            Ok(rows.iter().map(Self::parse_entity).collect())
        }

        async fn delete_entity_async(&self, id: &EntityId) -> Result<bool> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let rows = client
                .execute(
                    "DELETE FROM graph_entities WHERE id = $1",
                    &[&id.as_str()],
                )
                .await
                .map_err(|e| query_error("delete_entity", e))?;

            Ok(rows > 0)
        }

        async fn merge_entities_async(
            &self,
            entity_ids: &[EntityId],
            canonical_name: &str,
        ) -> Result<Entity> {
            if entity_ids.is_empty() {
                return Err(Error::OperationFailed {
                    operation: "merge_entities".to_string(),
                    cause: "No entity IDs provided".to_string(),
                });
            }

            let mut client = self.pool.get().await.map_err(pool_error)?;

            let canonical_id = &entity_ids[0];

            let rows = client
                .query(
                    "SELECT * FROM graph_entities WHERE id = $1",
                    &[&canonical_id.as_str()],
                )
                .await
                .map_err(|e| query_error("merge_entities_get_canonical", e))?;

            let canonical_entity = rows
                .first()
                .map(Self::parse_entity)
                .ok_or_else(|| Error::OperationFailed {
                    operation: "merge_entities".to_string(),
                    cause: format!("Canonical entity not found: {}", canonical_id.as_str()),
                })?;

            let mut all_aliases = canonical_entity.aliases.clone();
            all_aliases.push(canonical_entity.name.clone());

            // Run the entire merge in a transaction to prevent partial updates
            let tx = client
                .transaction()
                .await
                .map_err(|e| query_error("merge_entities_begin_tx", e))?;

            for other_id in entity_ids.iter().skip(1) {
                let other_rows = tx
                    .query(
                        "SELECT * FROM graph_entities WHERE id = $1",
                        &[&other_id.as_str()],
                    )
                    .await
                    .map_err(|e| query_error("merge_entities_get_other", e))?;

                if let Some(other) = other_rows.first().map(Self::parse_entity) {
                    all_aliases.push(other.name);
                    all_aliases.extend(other.aliases);

                    // Re-point relationships to canonical entity, ignoring
                    // conflicts from duplicate (from, to, type) tuples
                    tx.execute(
                        "UPDATE graph_relationships SET from_entity_id = $1 WHERE from_entity_id = $2
                         AND NOT EXISTS (
                             SELECT 1 FROM graph_relationships r2
                             WHERE r2.from_entity_id = $1
                               AND r2.to_entity_id = graph_relationships.to_entity_id
                               AND r2.relationship_type = graph_relationships.relationship_type
                         )",
                        &[&canonical_id.as_str(), &other_id.as_str()],
                    )
                    .await
                    .map_err(|e| query_error("merge_entities_repoint_from", e))?;

                    tx.execute(
                        "UPDATE graph_relationships SET to_entity_id = $1 WHERE to_entity_id = $2
                         AND NOT EXISTS (
                             SELECT 1 FROM graph_relationships r2
                             WHERE r2.to_entity_id = $1
                               AND r2.from_entity_id = graph_relationships.from_entity_id
                               AND r2.relationship_type = graph_relationships.relationship_type
                         )",
                        &[&canonical_id.as_str(), &other_id.as_str()],
                    )
                    .await
                    .map_err(|e| query_error("merge_entities_repoint_to", e))?;

                    // Re-point mentions, skip duplicates (same entity+memory)
                    tx.execute(
                        "UPDATE graph_entity_mentions AS outer_m SET entity_id = $1
                         WHERE outer_m.entity_id = $2
                         AND NOT EXISTS (
                             SELECT 1 FROM graph_entity_mentions inner_m
                             WHERE inner_m.entity_id = $1 AND inner_m.memory_id = outer_m.memory_id
                         )",
                        &[&canonical_id.as_str(), &other_id.as_str()],
                    )
                    .await
                    .map_err(|e| query_error("merge_entities_repoint_mentions", e))?;

                    // Delete orphaned mentions that couldn't be re-pointed (duplicates)
                    tx.execute(
                        "DELETE FROM graph_entity_mentions WHERE entity_id = $1",
                        &[&other_id.as_str()],
                    )
                    .await
                    .map_err(|e| query_error("merge_entities_delete_dup_mentions", e))?;

                    // Delete orphaned relationships that couldn't be re-pointed
                    tx.execute(
                        "DELETE FROM graph_relationships WHERE from_entity_id = $1 OR to_entity_id = $1",
                        &[&other_id.as_str()],
                    )
                    .await
                    .map_err(|e| query_error("merge_entities_delete_dup_rels", e))?;

                    tx.execute(
                        "DELETE FROM graph_entities WHERE id = $1",
                        &[&other_id.as_str()],
                    )
                    .await
                    .map_err(|e| query_error("merge_entities_delete", e))?;
                }
            }

            all_aliases.sort();
            all_aliases.dedup();
            all_aliases.retain(|a| a != canonical_name);

            let aliases_json =
                serde_json::to_string(&all_aliases).unwrap_or_else(|_| "[]".to_string());

            tx.execute(
                "UPDATE graph_entities SET name = $1, aliases = $2 WHERE id = $3",
                &[&canonical_name, &aliases_json.as_str(), &canonical_id.as_str()],
            )
            .await
            .map_err(|e| query_error("merge_entities_update", e))?;

            tx.commit()
                .await
                .map_err(|e| query_error("merge_entities_commit", e))?;

            let merged = Entity::new(
                canonical_entity.entity_type,
                canonical_name,
                canonical_entity.domain.clone(),
            )
            .with_id(canonical_entity.id)
            .with_confidence(canonical_entity.confidence)
            .with_aliases(all_aliases);

            Ok(merged)
        }

        async fn find_entities_by_name_async(
            &self,
            name: &str,
            entity_type: Option<EntityType>,
            domain: Option<&Domain>,
            limit: usize,
        ) -> Result<Vec<Entity>> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let like_pattern = format!("%{name}%");
            let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync>> = vec![Box::new(like_pattern)];
            let mut conditions = vec!["(name LIKE $1 OR aliases LIKE $1)".to_string()];

            if let Some(ref et) = entity_type {
                params.push(Box::new(et.as_str().to_string()));
                conditions.push(format!("entity_type = ${}", params.len()));
            }

            if let Some(d) = domain {
                if let Some(ref org) = d.organization {
                    params.push(Box::new(org.clone()));
                    conditions.push(format!("domain_org = ${}", params.len()));
                }
                if let Some(ref project) = d.project {
                    params.push(Box::new(project.clone()));
                    conditions.push(format!("domain_project = ${}", params.len()));
                }
                if let Some(ref repo) = d.repository {
                    params.push(Box::new(repo.clone()));
                    conditions.push(format!("domain_repo = ${}", params.len()));
                }
            }

            let sql = format!(
                "SELECT * FROM graph_entities WHERE {} ORDER BY confidence DESC LIMIT {}",
                conditions.join(" AND "),
                limit
            );

            let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
                params.iter().map(|p| p.as_ref()).collect();

            let rows = client.query(&sql, &param_refs).await.map_err(|e| query_error("find_entities_by_name", e))?;

            Ok(rows.iter().map(Self::parse_entity).collect())
        }

        async fn store_relationship_async(&self, relationship: &Relationship) -> Result<()> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let properties_json =
                serde_json::to_string(&relationship.properties).unwrap_or_else(|_| "{}".to_string());

            client
                .execute(
                    "INSERT INTO graph_relationships (
                        from_entity_id, to_entity_id, relationship_type, confidence,
                        valid_time_start, valid_time_end, transaction_time, properties
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                    ON CONFLICT(from_entity_id, to_entity_id, relationship_type) DO UPDATE SET
                        confidence = EXCLUDED.confidence,
                        valid_time_start = EXCLUDED.valid_time_start,
                        valid_time_end = EXCLUDED.valid_time_end,
                        properties = EXCLUDED.properties",
                    &[
                        &relationship.from_entity.as_str(),
                        &relationship.to_entity.as_str(),
                        &relationship.relationship_type.as_str(),
                        &f64::from(relationship.confidence),
                        &relationship.valid_time.start as &(dyn tokio_postgres::types::ToSql + Sync),
                        &relationship.valid_time.end as &(dyn tokio_postgres::types::ToSql + Sync),
                        &relationship.transaction_time.timestamp(),
                        &properties_json.as_str(),
                    ],
                )
                .await
                .map_err(|e| query_error("store_relationship", e))?;

            Ok(())
        }

        async fn query_relationships_async(
            &self,
            query: &RelationshipQuery,
        ) -> Result<Vec<Relationship>> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let mut conditions = Vec::new();
            let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync>> = Vec::new();

            if let Some(ref from_entity) = query.from_entity {
                params.push(Box::new(from_entity.as_str().to_string()));
                conditions.push(format!("from_entity_id = ${}", params.len()));
            }
            if let Some(ref to_entity) = query.to_entity {
                params.push(Box::new(to_entity.as_str().to_string()));
                conditions.push(format!("to_entity_id = ${}", params.len()));
            }
            if let Some(ref rt) = query.relationship_type {
                params.push(Box::new(rt.as_str().to_string()));
                conditions.push(format!("relationship_type = ${}", params.len()));
            }
            if let Some(min_confidence) = query.min_confidence {
                params.push(Box::new(f64::from(min_confidence)));
                conditions.push(format!("confidence >= ${}", params.len()));
            }

            let where_clause = if conditions.is_empty() {
                String::new()
            } else {
                format!("WHERE {}", conditions.join(" AND "))
            };

            let limit = query.limit.unwrap_or(100);
            let sql = format!(
                "SELECT * FROM graph_relationships {} ORDER BY confidence DESC LIMIT {}",
                where_clause, limit
            );

            let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
                params.iter().map(|p| p.as_ref()).collect();

            let rows = client.query(&sql, &param_refs).await.map_err(|e| query_error("query_relationships", e))?;

            Ok(rows.iter().map(Self::parse_relationship).collect())
        }

        async fn delete_relationships_async(&self, query: &RelationshipQuery) -> Result<usize> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let mut conditions = Vec::new();
            let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync>> = Vec::new();

            if let Some(ref from_entity) = query.from_entity {
                params.push(Box::new(from_entity.as_str().to_string()));
                conditions.push(format!("from_entity_id = ${}", params.len()));
            }
            if let Some(ref to_entity) = query.to_entity {
                params.push(Box::new(to_entity.as_str().to_string()));
                conditions.push(format!("to_entity_id = ${}", params.len()));
            }
            if let Some(ref rt) = query.relationship_type {
                params.push(Box::new(rt.as_str().to_string()));
                conditions.push(format!("relationship_type = ${}", params.len()));
            }

            if conditions.is_empty() {
                return Ok(0);
            }

            let sql = format!(
                "DELETE FROM graph_relationships WHERE {}",
                conditions.join(" AND ")
            );

            let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
                params.iter().map(|p| p.as_ref()).collect();

            let rows = client.execute(&sql, &param_refs).await.map_err(|e| query_error("delete_relationships", e))?;

            Ok(rows as usize)
        }

        async fn get_relationship_types_async(
            &self,
            from_entity: &EntityId,
            to_entity: &EntityId,
        ) -> Result<Vec<RelationshipType>> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let rows = client
                .query(
                    "SELECT DISTINCT relationship_type FROM graph_relationships
                     WHERE from_entity_id = $1 AND to_entity_id = $2",
                    &[&from_entity.as_str(), &to_entity.as_str()],
                )
                .await
                .map_err(|e| query_error("get_relationship_types", e))?;

            Ok(rows
                .iter()
                .filter_map(|row| {
                    let type_str: String = row.get(0);
                    RelationshipType::parse(&type_str)
                })
                .collect())
        }

        async fn store_mention_async(&self, mention: &EntityMention) -> Result<()> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let row = client
                .query_one(
                    "INSERT INTO graph_entity_mentions (entity_id, memory_id, confidence, start_offset, end_offset, matched_text, transaction_time)
                     VALUES ($1, $2, $3, $4, $5, $6, $7)
                     ON CONFLICT(entity_id, memory_id) DO UPDATE SET
                        confidence = EXCLUDED.confidence,
                        start_offset = EXCLUDED.start_offset,
                        end_offset = EXCLUDED.end_offset,
                        matched_text = EXCLUDED.matched_text
                     RETURNING (xmax = 0) AS was_insert",
                    &[
                        &mention.entity_id.as_str(),
                        &mention.memory_id.as_str(),
                        &f64::from(mention.confidence),
                        &mention.start_offset.map(|v| v as i64) as &(dyn tokio_postgres::types::ToSql + Sync),
                        &mention.end_offset.map(|v| v as i64) as &(dyn tokio_postgres::types::ToSql + Sync),
                        &mention.matched_text as &(dyn tokio_postgres::types::ToSql + Sync),
                        &mention.transaction_time.timestamp(),
                    ],
                )
                .await
                .map_err(|e| query_error("store_mention", e))?;

            let was_insert: bool = row.get(0);
            if was_insert {
                let _ = client
                    .execute(
                        "UPDATE graph_entities SET mention_count = mention_count + 1 WHERE id = $1",
                        &[&mention.entity_id.as_str()],
                    )
                    .await;
            }

            Ok(())
        }

        async fn get_mentions_for_entity_async(&self, entity_id: &EntityId) -> Result<Vec<EntityMention>> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let rows = client
                .query(
                    "SELECT entity_id, memory_id, confidence, start_offset, end_offset, matched_text, transaction_time
                     FROM graph_entity_mentions WHERE entity_id = $1 ORDER BY transaction_time DESC",
                    &[&entity_id.as_str()],
                )
                .await
                .map_err(|e| query_error("get_mentions_for_entity", e))?;

            Ok(rows.iter().map(Self::parse_mention).collect())
        }

        async fn get_entities_in_memory_async(&self, memory_id: &MemoryId) -> Result<Vec<Entity>> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let rows = client
                .query(
                    "SELECT e.* FROM graph_entities e
                     INNER JOIN graph_entity_mentions m ON e.id = m.entity_id
                     WHERE m.memory_id = $1
                     ORDER BY m.confidence DESC",
                    &[&memory_id.as_str()],
                )
                .await
                .map_err(|e| query_error("get_entities_in_memory", e))?;

            Ok(rows.iter().map(Self::parse_entity).collect())
        }

        async fn delete_mentions_for_entity_async(&self, entity_id: &EntityId) -> Result<usize> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let rows = client
                .execute(
                    "DELETE FROM graph_entity_mentions WHERE entity_id = $1",
                    &[&entity_id.as_str()],
                )
                .await
                .map_err(|e| query_error("delete_mentions_for_entity", e))?;

            let _ = client
                .execute(
                    "UPDATE graph_entities SET mention_count = 0 WHERE id = $1",
                    &[&entity_id.as_str()],
                )
                .await;

            Ok(rows as usize)
        }

        async fn delete_mentions_for_memory_async(&self, memory_id: &MemoryId) -> Result<usize> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let affected = client
                .query(
                    "SELECT entity_id FROM graph_entity_mentions WHERE memory_id = $1",
                    &[&memory_id.as_str()],
                )
                .await
                .map_err(|e| query_error("delete_mentions_for_memory_get", e))?;

            let entity_ids: Vec<String> = affected.iter().map(|r| r.get(0)).collect();

            let rows = client
                .execute(
                    "DELETE FROM graph_entity_mentions WHERE memory_id = $1",
                    &[&memory_id.as_str()],
                )
                .await
                .map_err(|e| query_error("delete_mentions_for_memory", e))?;

            for eid in &entity_ids {
                let _ = client
                    .execute(
                        "UPDATE graph_entities SET mention_count = GREATEST(0, mention_count - 1) WHERE id = $1",
                        &[&eid],
                    )
                    .await;
            }

            Ok(rows as usize)
        }

        async fn traverse_async(
            &self,
            start: &EntityId,
            max_depth: u32,
            relationship_types: Option<&[RelationshipType]>,
            min_confidence: Option<f32>,
        ) -> Result<TraversalResult> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let type_filter = relationship_types
                .map(|types| {
                    let type_strs: Vec<String> =
                        types.iter().map(|t| format!("'{}'", t.as_str())).collect();
                    format!("AND r.relationship_type IN ({})", type_strs.join(", "))
                })
                .unwrap_or_default();

            let confidence_filter = min_confidence
                .map(|c| format!("AND r.confidence >= {c}"))
                .unwrap_or_default();

            let sql = format!(
                "WITH RECURSIVE reachable(entity_id, depth, path) AS (
                    SELECT $1::text, 0, ',' || $1::text || ','
                    UNION ALL
                    SELECT r.to_entity_id, reachable.depth + 1, reachable.path || r.to_entity_id || ','
                    FROM reachable
                    JOIN graph_relationships r ON r.from_entity_id = reachable.entity_id
                    WHERE reachable.depth < $2
                      AND reachable.path NOT LIKE '%,' || r.to_entity_id || ',%'
                      {type_filter}
                      {confidence_filter}
                )
                SELECT DISTINCT e.*, reachable.depth
                FROM reachable
                JOIN graph_entities e ON e.id = reachable.entity_id
                ORDER BY reachable.depth, e.mention_count DESC"
            );

            let max_depth_i32 = max_depth as i32;
            let rows = client
                .query(&sql, &[&start.as_str(), &max_depth_i32])
                .await
                .map_err(|e| query_error("traverse", e))?;

            let entities: Vec<Entity> = rows.iter().map(Self::parse_entity).collect();

            let relationships = if entities.is_empty() {
                Vec::new()
            } else {
                let entity_ids: Vec<String> =
                    entities.iter().map(|e| e.id.as_str().to_string()).collect();

                let from_placeholders: Vec<String> =
                    (1..=entity_ids.len()).map(|i| format!("${i}")).collect();
                let to_placeholders: Vec<String> = (entity_ids.len() + 1..=entity_ids.len() * 2)
                    .map(|i| format!("${i}"))
                    .collect();

                let rel_sql = format!(
                    "SELECT * FROM graph_relationships WHERE from_entity_id IN ({}) AND to_entity_id IN ({})",
                    from_placeholders.join(", "),
                    to_placeholders.join(", ")
                );

                let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = Vec::new();
                for id in &entity_ids {
                    params.push(id);
                }
                for id in &entity_ids {
                    params.push(id);
                }

                let rel_rows = client
                    .query(&rel_sql, &params)
                    .await
                    .map_err(|e| query_error("traverse_relationships", e))?;

                rel_rows.iter().map(Self::parse_relationship).collect()
            };

            let total_count = entities.len();
            Ok(TraversalResult {
                entities,
                relationships,
                total_count,
            })
        }

        async fn find_path_async(
            &self,
            from: &EntityId,
            to: &EntityId,
            max_depth: u32,
        ) -> Result<Option<TraversalResult>> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let max_depth_i32 = max_depth as i32;
            let rows = client
                .query(
                    "WITH RECURSIVE path_finder(entity_id, depth, path, visited) AS (
                        SELECT $1::text, 0, $1::text, ',' || $1::text || ','
                        UNION ALL
                        SELECT r.to_entity_id, path_finder.depth + 1,
                               path_finder.path || ',' || r.to_entity_id,
                               path_finder.visited || r.to_entity_id || ','
                        FROM path_finder
                        JOIN graph_relationships r ON r.from_entity_id = path_finder.entity_id
                        WHERE path_finder.depth < $3
                          AND path_finder.visited NOT LIKE '%,' || r.to_entity_id || ',%'
                    )
                    SELECT path FROM path_finder WHERE entity_id = $2 ORDER BY depth LIMIT 1",
                    &[&from.as_str(), &to.as_str(), &max_depth_i32],
                )
                .await
                .map_err(|e| query_error("find_path", e))?;

            let path_str: Option<String> = rows.first().map(|r| r.get(0));

            if let Some(path_str) = path_str {
                let path_ids: Vec<&str> = path_str.split(',').collect();

                let placeholders: Vec<String> =
                    (1..=path_ids.len()).map(|i| format!("${i}")).collect();
                let entity_sql = format!(
                    "SELECT * FROM graph_entities WHERE id IN ({})",
                    placeholders.join(", ")
                );
                let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
                    path_ids.iter().map(|s| s as &(dyn tokio_postgres::types::ToSql + Sync)).collect();

                let entity_rows = client
                    .query(&entity_sql, &params)
                    .await
                    .map_err(|e| query_error("find_path_entities", e))?;

                let entities: Vec<Entity> = entity_rows.iter().map(Self::parse_entity).collect();

                let mut relationships = Vec::new();
                for window in path_ids.windows(2) {
                    if let [from_id, to_id] = window {
                        let rel_rows = client
                            .query(
                                "SELECT * FROM graph_relationships WHERE from_entity_id = $1 AND to_entity_id = $2 LIMIT 1",
                                &[&from_id.to_string(), &to_id.to_string()],
                            )
                            .await
                            .ok()
                            .unwrap_or_default();

                        if let Some(row) = rel_rows.first() {
                            relationships.push(Self::parse_relationship(row));
                        }
                    }
                }

                let total_count = entities.len();
                Ok(Some(TraversalResult {
                    entities,
                    relationships,
                    total_count,
                }))
            } else {
                Ok(None)
            }
        }

        async fn query_entities_at_async(
            &self,
            query: &EntityQuery,
            point: &BitemporalPoint,
        ) -> Result<Vec<Entity>> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let (base_where, mut params) = Self::build_entity_where(query);

            let temporal_start = format!(
                "AND (valid_time_start IS NULL OR valid_time_start <= ${})",
                params.len() + 1
            );
            params.push(Box::new(point.valid_at));

            let temporal_end = format!(
                "AND (valid_time_end IS NULL OR valid_time_end > ${})",
                params.len() + 1
            );
            params.push(Box::new(point.valid_at));

            let temporal_tx = format!("AND transaction_time <= ${}", params.len() + 1);
            params.push(Box::new(point.as_of));

            let where_clause = if base_where.is_empty() {
                format!("WHERE 1=1 {temporal_start} {temporal_end} {temporal_tx}")
            } else {
                format!("{base_where} {temporal_start} {temporal_end} {temporal_tx}")
            };

            let limit = query.limit.unwrap_or(100);
            let sql = format!(
                "SELECT * FROM graph_entities {} ORDER BY mention_count DESC LIMIT {}",
                where_clause, limit
            );

            let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
                params.iter().map(|p| p.as_ref()).collect();

            let rows = client.query(&sql, &param_refs).await.map_err(|e| query_error("query_entities_at", e))?;

            Ok(rows.iter().map(Self::parse_entity).collect())
        }

        async fn query_relationships_at_async(
            &self,
            query: &RelationshipQuery,
            point: &BitemporalPoint,
        ) -> Result<Vec<Relationship>> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let mut conditions = Vec::new();
            let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync>> = Vec::new();

            if let Some(ref from_entity) = query.from_entity {
                params.push(Box::new(from_entity.as_str().to_string()));
                conditions.push(format!("from_entity_id = ${}", params.len()));
            }
            if let Some(ref to_entity) = query.to_entity {
                params.push(Box::new(to_entity.as_str().to_string()));
                conditions.push(format!("to_entity_id = ${}", params.len()));
            }
            if let Some(ref rt) = query.relationship_type {
                params.push(Box::new(rt.as_str().to_string()));
                conditions.push(format!("relationship_type = ${}", params.len()));
            }

            params.push(Box::new(point.valid_at));
            conditions.push(format!(
                "(valid_time_start IS NULL OR valid_time_start <= ${})",
                params.len()
            ));
            params.push(Box::new(point.valid_at));
            conditions.push(format!(
                "(valid_time_end IS NULL OR valid_time_end > ${})",
                params.len()
            ));
            params.push(Box::new(point.as_of));
            conditions.push(format!("transaction_time <= ${}", params.len()));

            let where_clause = format!("WHERE {}", conditions.join(" AND "));
            let limit = query.limit.unwrap_or(100);

            let sql = format!(
                "SELECT * FROM graph_relationships {} ORDER BY confidence DESC LIMIT {}",
                where_clause, limit
            );

            let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
                params.iter().map(|p| p.as_ref()).collect();

            let rows = client.query(&sql, &param_refs).await.map_err(|e| query_error("query_relationships_at", e))?;

            Ok(rows.iter().map(Self::parse_relationship).collect())
        }

        async fn close_entity_valid_time_async(&self, id: &EntityId, end_time: i64) -> Result<()> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let rows = client
                .execute(
                    "UPDATE graph_entities SET valid_time_end = $1 WHERE id = $2",
                    &[&end_time, &id.as_str()],
                )
                .await
                .map_err(|e| query_error("close_entity_valid_time", e))?;

            if rows == 0 {
                return Err(Error::OperationFailed {
                    operation: "close_entity_valid_time".to_string(),
                    cause: format!("Entity not found: {}", id.as_str()),
                });
            }
            Ok(())
        }

        async fn close_relationship_valid_time_async(
            &self,
            from_entity: &EntityId,
            to_entity: &EntityId,
            relationship_type: RelationshipType,
            end_time: i64,
        ) -> Result<()> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let rows = client
                .execute(
                    "UPDATE graph_relationships SET valid_time_end = $1
                     WHERE from_entity_id = $2 AND to_entity_id = $3 AND relationship_type = $4",
                    &[
                        &end_time,
                        &from_entity.as_str(),
                        &to_entity.as_str(),
                        &relationship_type.as_str(),
                    ],
                )
                .await
                .map_err(|e| query_error("close_relationship_valid_time", e))?;

            if rows == 0 {
                return Err(Error::OperationFailed {
                    operation: "close_relationship_valid_time".to_string(),
                    cause: "Relationship not found".to_string(),
                });
            }
            Ok(())
        }

        async fn get_graph_stats_async(&self) -> Result<GraphStats> {
            let client = self.pool.get().await.map_err(pool_error)?;

            let entity_count: i64 = client
                .query_one("SELECT COUNT(*) FROM graph_entities", &[])
                .await
                .map(|r| r.get(0))
                .unwrap_or(0);

            let relationship_count: i64 = client
                .query_one("SELECT COUNT(*) FROM graph_relationships", &[])
                .await
                .map(|r| r.get(0))
                .unwrap_or(0);

            let mention_count: i64 = client
                .query_one("SELECT COUNT(*) FROM graph_entity_mentions", &[])
                .await
                .map(|r| r.get(0))
                .unwrap_or(0);

            let mut entities_by_type = HashMap::new();
            let type_rows = client
                .query(
                    "SELECT entity_type, COUNT(*) FROM graph_entities GROUP BY entity_type",
                    &[],
                )
                .await
                .unwrap_or_default();

            for row in &type_rows {
                let type_str: String = row.get(0);
                let count: i64 = row.get(1);
                if let Some(et) = EntityType::parse(&type_str) {
                    entities_by_type.insert(et, count as usize);
                }
            }

            let mut relationships_by_type = HashMap::new();
            let rel_rows = client
                .query(
                    "SELECT relationship_type, COUNT(*) FROM graph_relationships GROUP BY relationship_type",
                    &[],
                )
                .await
                .unwrap_or_default();

            for row in &rel_rows {
                let type_str: String = row.get(0);
                let count: i64 = row.get(1);
                if let Some(rt) = RelationshipType::parse(&type_str) {
                    relationships_by_type.insert(rt, count as usize);
                }
            }

            let avg_relationships_per_entity = if entity_count > 0 {
                relationship_count as f32 / entity_count as f32
            } else {
                0.0
            };

            Ok(GraphStats {
                entity_count: entity_count as usize,
                entities_by_type,
                relationship_count: relationship_count as usize,
                relationships_by_type,
                mention_count: mention_count as usize,
                avg_relationships_per_entity,
            })
        }

        async fn graph_clear_async(&self) -> Result<()> {
            let client = self.pool.get().await.map_err(pool_error)?;

            client
                .execute(
                    "TRUNCATE graph_entity_mentions, graph_relationships, graph_entities",
                    &[],
                )
                .await
                .map_err(|e| query_error("graph_clear", e))?;

            Ok(())
        }
    }

    // ========================================================================
    // GraphBackend trait implementation
    // ========================================================================

    impl GraphBackend for PostgresBackend {
        fn store_entity(&self, entity: &Entity) -> Result<()> {
            self.block_on(self.store_entity_async(entity))
        }

        fn get_entity(&self, id: &EntityId) -> Result<Option<Entity>> {
            self.block_on(self.get_entity_async(id))
        }

        fn query_entities(&self, query: &EntityQuery) -> Result<Vec<Entity>> {
            self.block_on(self.query_entities_async(query))
        }

        fn delete_entity(&self, id: &EntityId) -> Result<bool> {
            self.block_on(self.delete_entity_async(id))
        }

        fn merge_entities(&self, entity_ids: &[EntityId], canonical_name: &str) -> Result<Entity> {
            self.block_on(self.merge_entities_async(entity_ids, canonical_name))
        }

        fn find_entities_by_name(
            &self,
            name: &str,
            entity_type: Option<EntityType>,
            domain: Option<&Domain>,
            limit: usize,
        ) -> Result<Vec<Entity>> {
            self.block_on(self.find_entities_by_name_async(name, entity_type, domain, limit))
        }

        fn store_relationship(&self, relationship: &Relationship) -> Result<()> {
            self.block_on(self.store_relationship_async(relationship))
        }

        fn query_relationships(&self, query: &RelationshipQuery) -> Result<Vec<Relationship>> {
            self.block_on(self.query_relationships_async(query))
        }

        fn delete_relationships(&self, query: &RelationshipQuery) -> Result<usize> {
            self.block_on(self.delete_relationships_async(query))
        }

        fn get_relationship_types(
            &self,
            from_entity: &EntityId,
            to_entity: &EntityId,
        ) -> Result<Vec<RelationshipType>> {
            self.block_on(self.get_relationship_types_async(from_entity, to_entity))
        }

        fn store_mention(&self, mention: &EntityMention) -> Result<()> {
            self.block_on(self.store_mention_async(mention))
        }

        fn get_mentions_for_entity(&self, entity_id: &EntityId) -> Result<Vec<EntityMention>> {
            self.block_on(self.get_mentions_for_entity_async(entity_id))
        }

        fn get_entities_in_memory(&self, memory_id: &MemoryId) -> Result<Vec<Entity>> {
            self.block_on(self.get_entities_in_memory_async(memory_id))
        }

        fn delete_mentions_for_entity(&self, entity_id: &EntityId) -> Result<usize> {
            self.block_on(self.delete_mentions_for_entity_async(entity_id))
        }

        fn delete_mentions_for_memory(&self, memory_id: &MemoryId) -> Result<usize> {
            self.block_on(self.delete_mentions_for_memory_async(memory_id))
        }

        fn traverse(
            &self,
            start: &EntityId,
            max_depth: u32,
            relationship_types: Option<&[RelationshipType]>,
            min_confidence: Option<f32>,
        ) -> Result<TraversalResult> {
            self.block_on(self.traverse_async(start, max_depth, relationship_types, min_confidence))
        }

        fn find_path(
            &self,
            from: &EntityId,
            to: &EntityId,
            max_depth: u32,
        ) -> Result<Option<TraversalResult>> {
            self.block_on(self.find_path_async(from, to, max_depth))
        }

        fn query_entities_at(
            &self,
            query: &EntityQuery,
            point: &BitemporalPoint,
        ) -> Result<Vec<Entity>> {
            self.block_on(self.query_entities_at_async(query, point))
        }

        fn query_relationships_at(
            &self,
            query: &RelationshipQuery,
            point: &BitemporalPoint,
        ) -> Result<Vec<Relationship>> {
            self.block_on(self.query_relationships_at_async(query, point))
        }

        fn close_entity_valid_time(&self, id: &EntityId, end_time: i64) -> Result<()> {
            self.block_on(self.close_entity_valid_time_async(id, end_time))
        }

        fn close_relationship_valid_time(
            &self,
            from_entity: &EntityId,
            to_entity: &EntityId,
            relationship_type: RelationshipType,
            end_time: i64,
        ) -> Result<()> {
            self.block_on(self.close_relationship_valid_time_async(
                from_entity,
                to_entity,
                relationship_type,
                end_time,
            ))
        }

        fn get_stats(&self) -> Result<GraphStats> {
            self.block_on(self.get_graph_stats_async())
        }

        fn clear(&self) -> Result<()> {
            self.block_on(self.graph_clear_async())
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
            assert!(validate_table_name("memories").is_ok());
            assert!(validate_table_name("subcog_memories").is_ok());

            // Invalid table names
            assert!(validate_table_name("users").is_err());
            assert!(validate_table_name("memories; DROP TABLE users").is_err());
        }
    }
}

#[cfg(feature = "postgres")]
pub use implementation::PostgresBackend;

#[cfg(not(feature = "postgres"))]
mod stub {
    use crate::models::{Memory, MemoryId, SearchFilter};
    use crate::storage::traits::IndexBackend;
    use crate::{Error, Result};

    /// Stub PostgreSQL backend when feature is not enabled.
    pub struct PostgresBackend;

    impl PostgresBackend {
        /// Creates a new PostgreSQL index backend (stub).
        ///
        /// # Errors
        ///
        /// Always returns an error because the feature is not enabled.
        pub fn new(
            _connection_url: &str,
            _table_name: impl Into<String>,
            _vector_table_name: impl Into<String>,
        ) -> Result<Self> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }

        /// Creates a backend with a custom pool size (stub).
        ///
        /// # Errors
        ///
        /// Always returns an error because the feature is not enabled.
        pub fn with_pool_size(
            _connection_url: &str,
            _table_name: impl Into<String>,
            _vector_table_name: impl Into<String>,
            _pool_max_size: Option<usize>,
        ) -> Result<Self> {
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

    impl IndexBackend for PostgresBackend {
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

        fn get_memories_batch(&self, _ids: &[MemoryId]) -> Result<Vec<Option<Memory>>> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }

        fn clear(&self) -> Result<()> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }
    }

    impl crate::storage::traits::PersistenceBackend for PostgresBackend {
        fn store(&self, _memory: &Memory) -> Result<()> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }

        fn get(&self, _id: &MemoryId) -> Result<Option<Memory>> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }

        fn delete(&self, _id: &MemoryId) -> Result<bool> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }

        fn list_ids(&self) -> Result<Vec<MemoryId>> {
            Err(Error::FeatureNotEnabled("postgres".to_string()))
        }
    }

    impl crate::storage::traits::graph::GraphBackend for PostgresBackend {
        fn store_entity(&self, _entity: &crate::models::graph::Entity) -> Result<()> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn get_entity(&self, _id: &crate::models::graph::EntityId) -> Result<Option<crate::models::graph::Entity>> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn query_entities(&self, _query: &crate::models::graph::EntityQuery) -> Result<Vec<crate::models::graph::Entity>> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn delete_entity(&self, _id: &crate::models::graph::EntityId) -> Result<bool> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn merge_entities(&self, _entity_ids: &[crate::models::graph::EntityId], _canonical_name: &str) -> Result<crate::models::graph::Entity> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn find_entities_by_name(&self, _name: &str, _entity_type: Option<crate::models::graph::EntityType>, _domain: Option<&crate::models::Domain>, _limit: usize) -> Result<Vec<crate::models::graph::Entity>> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn store_relationship(&self, _relationship: &crate::models::graph::Relationship) -> Result<()> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn query_relationships(&self, _query: &crate::models::graph::RelationshipQuery) -> Result<Vec<crate::models::graph::Relationship>> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn delete_relationships(&self, _query: &crate::models::graph::RelationshipQuery) -> Result<usize> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn get_relationship_types(&self, _from: &crate::models::graph::EntityId, _to: &crate::models::graph::EntityId) -> Result<Vec<crate::models::graph::RelationshipType>> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn store_mention(&self, _mention: &crate::models::graph::EntityMention) -> Result<()> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn get_mentions_for_entity(&self, _entity_id: &crate::models::graph::EntityId) -> Result<Vec<crate::models::graph::EntityMention>> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn get_entities_in_memory(&self, _memory_id: &MemoryId) -> Result<Vec<crate::models::graph::Entity>> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn delete_mentions_for_entity(&self, _entity_id: &crate::models::graph::EntityId) -> Result<usize> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn delete_mentions_for_memory(&self, _memory_id: &MemoryId) -> Result<usize> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn traverse(&self, _start: &crate::models::graph::EntityId, _max_depth: u32, _relationship_types: Option<&[crate::models::graph::RelationshipType]>, _min_confidence: Option<f32>) -> Result<crate::models::graph::TraversalResult> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn find_path(&self, _from: &crate::models::graph::EntityId, _to: &crate::models::graph::EntityId, _max_depth: u32) -> Result<Option<crate::models::graph::TraversalResult>> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn query_entities_at(&self, _query: &crate::models::graph::EntityQuery, _point: &crate::models::temporal::BitemporalPoint) -> Result<Vec<crate::models::graph::Entity>> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn query_relationships_at(&self, _query: &crate::models::graph::RelationshipQuery, _point: &crate::models::temporal::BitemporalPoint) -> Result<Vec<crate::models::graph::Relationship>> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn close_entity_valid_time(&self, _id: &crate::models::graph::EntityId, _end_time: i64) -> Result<()> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn close_relationship_valid_time(&self, _from: &crate::models::graph::EntityId, _to: &crate::models::graph::EntityId, _rt: crate::models::graph::RelationshipType, _end_time: i64) -> Result<()> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn get_stats(&self) -> Result<crate::storage::traits::graph::GraphStats> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
        fn clear(&self) -> Result<()> { Err(Error::FeatureNotEnabled("postgres".to_string())) }
    }
}

#[cfg(not(feature = "postgres"))]
pub use stub::PostgresBackend;
