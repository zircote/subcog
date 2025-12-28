//! Index backend implementations.

mod sqlite;

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
