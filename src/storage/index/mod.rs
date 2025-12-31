//! Index backend implementations.

mod domain;
mod postgresql;
mod sqlite;

pub use domain::{
    DomainIndexConfig, DomainIndexManager, DomainScope, OrgIndexConfig, find_repo_root,
};
pub use postgresql::PostgresIndexBackend;
pub use sqlite::SqliteBackend;

// Redis backend available with feature flag
#[cfg(feature = "redis")]
mod redis;
#[cfg(feature = "redis")]
pub use redis::RedisBackend;
