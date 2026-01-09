//! Storage layer abstraction.
//!
//! This module provides a three-layer storage architecture:
//! - **Persistence**: Authoritative storage (`SQLite`, PostgreSQL, Filesystem)
//! - **Index**: Full-text search (`SQLite` + FTS5, PostgreSQL, `RediSearch`)
//! - **Vector**: Embedding similarity search (usearch, pgvector, Redis)

// Allow cast precision loss for score calculations where exact precision is not critical.
#![allow(clippy::cast_precision_loss)]
// Allow significant_drop_tightening - dropping database connections slightly early
// provides no meaningful benefit.
#![allow(clippy::significant_drop_tightening)]
// Allow manual_let_else for clearer error handling in some contexts.
#![allow(clippy::manual_let_else)]
// Allow match_same_arms for explicit enum handling.
#![allow(clippy::match_same_arms)]
// Allow or_fun_call - the error path is uncommon.
#![allow(clippy::or_fun_call)]
// Allow unused_self for methods kept for API consistency.
#![allow(clippy::unused_self)]

pub mod bulkhead;
pub mod index;
pub mod migrations;
pub mod persistence;
pub mod prompt;
pub mod resilience;
pub mod traits;
pub mod vector;

pub use bulkhead::{
    BulkheadIndexBackend, BulkheadPersistenceBackend, BulkheadVectorBackend, StorageBulkheadConfig,
};
pub use index::get_user_data_dir;
pub use prompt::{
    FilesystemPromptStorage, PostgresPromptStorage, PromptBackendType, PromptStorage,
    PromptStorageFactory, RedisPromptStorage, SqlitePromptStorage,
};
pub use resilience::{
    ResilientIndexBackend, ResilientPersistenceBackend, ResilientVectorBackend,
    StorageResilienceConfig, is_retryable_storage_error, retry_connection,
};
pub use traits::{IndexBackend, PersistenceBackend, VectorBackend};

/// Composite storage combining all three layers.
pub struct CompositeStorage<P, I, V>
where
    P: PersistenceBackend,
    I: IndexBackend,
    V: VectorBackend,
{
    persistence: P,
    index: I,
    vector: V,
}

impl<P, I, V> CompositeStorage<P, I, V>
where
    P: PersistenceBackend,
    I: IndexBackend,
    V: VectorBackend,
{
    /// Creates a new composite storage with the given backends.
    pub const fn new(persistence: P, index: I, vector: V) -> Self {
        Self {
            persistence,
            index,
            vector,
        }
    }

    /// Returns a reference to the persistence backend.
    pub const fn persistence(&self) -> &P {
        &self.persistence
    }

    /// Returns a reference to the index backend.
    pub const fn index(&self) -> &I {
        &self.index
    }

    /// Returns a reference to the vector backend.
    pub const fn vector(&self) -> &V {
        &self.vector
    }
}
