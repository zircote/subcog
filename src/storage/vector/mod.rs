//! Vector backend implementations.

mod pgvector;
mod usearch;

pub use pgvector::{DEFAULT_DIMENSIONS, PgvectorBackend};
pub use usearch::UsearchBackend;

// Redis backend available with feature flag
#[cfg(feature = "redis")]
mod redis;
#[cfg(feature = "redis")]
pub use redis::RedisVectorBackend;
