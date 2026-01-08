//! Index backend implementations.
//!
//! This module contains backends that implement the [`IndexBackend`](crate::storage::traits::IndexBackend)
//! trait, providing full-text search and filtering capabilities.
//!
//! ## Available Backends
//!
//! - [`SqliteBackend`]: `SQLite` FTS5 full-text search (local, high performance)
//! - [`PostgresIndexBackend`]: PostgreSQL full-text search (enterprise, distributed)
//! - [`RedisBackend`]: `RediSearch` for distributed indexing (requires `redis` feature)
//!
//! ## Design Notes
//!
//! - [`SqliteBackend`] implements ONLY the [`IndexBackend`](crate::storage::traits::IndexBackend) trait
//! - For `SQLite` persistence operations, use [`crate::storage::persistence::SqlitePersistenceBackend`]
//! - [`SqliteBackend`] uses shared infrastructure from [`crate::storage::sqlite`]
//! - Index backends focus solely on search and retrieval, not content storage

mod domain;
mod postgresql;
mod sqlite;

pub use domain::{
    DomainIndexConfig, DomainIndexManager, DomainScope, OrgIndexConfig, find_repo_root,
    get_user_data_dir, is_in_git_repo, is_path_in_git_repo,
};
pub use postgresql::PostgresIndexBackend;
pub use sqlite::SqliteBackend;

// Redis backend available with feature flag
#[cfg(feature = "redis")]
mod redis;
#[cfg(feature = "redis")]
pub use redis::RedisBackend;
