//! Index backend implementations.

mod domain;
mod sqlite;

pub use domain::{
    DomainIndexConfig, DomainIndexManager, DomainScope, OrgIndexConfig, find_repo_root,
};
pub use sqlite::SqliteBackend;

// Redis backend available with feature flag
#[cfg(feature = "redis")]
mod redis;
#[cfg(feature = "redis")]
pub use redis::RedisBackend;

// PostgreSQL backend available with feature flag
#[cfg(feature = "postgres")]
mod postgresql;
#[cfg(feature = "postgres")]
pub use postgresql::PostgresIndexBackend;
