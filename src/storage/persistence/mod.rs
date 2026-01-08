//! Persistence backend implementations.
//!
//! This module contains backends that implement the [`PersistenceBackend`](crate::storage::traits::PersistenceBackend)
//! trait, providing authoritative storage for memory content with ACID guarantees.
//!
//! ## Available Backends
//!
//! - [`SqlitePersistenceBackend`]: SQLite-based storage with full ACID transactions
//! - [`PostgresBackend`]: PostgreSQL-based storage for enterprise deployments
//! - [`FilesystemBackend`]: Simple file-based storage for development and testing
//!
//! ## Design Notes
//!
//! - [`SqlitePersistenceBackend`] uses shared infrastructure from [`crate::storage::sqlite`]
//! - Each backend operates independently and can be used standalone
//! - Persistence backends focus solely on content storage, not search indexing
//! - All write operations use transactions to ensure data consistency

mod filesystem;
mod postgresql;
mod sqlite;

pub use filesystem::FilesystemBackend;
pub use postgresql::PostgresBackend;
pub use sqlite::SqlitePersistenceBackend;
