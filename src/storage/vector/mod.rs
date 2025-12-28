//! Vector backend implementations.

mod usearch;

pub use usearch::UsearchBackend;

// Redis backend available with feature flag
#[cfg(feature = "redis")]
mod redis;
#[cfg(feature = "redis")]
pub use redis::RedisVectorBackend;

// pgvector backend available with feature flag
#[cfg(feature = "postgres")]
mod pgvector;
#[cfg(feature = "postgres")]
pub use pgvector::PgvectorBackend;
