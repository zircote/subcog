//! Shared `SQLite` infrastructure for storage backends.
//!
//! This module contains common utilities used by both the `SQLite` index backend
//! ([`crate::storage::index::SqliteBackend`]) and the `SQLite` persistence backend
//! ([`crate::storage::persistence::SqlitePersistenceBackend`]).
//!
//! ## Purpose
//!
//! Originally, both [`IndexBackend`](crate::storage::traits::IndexBackend) and
//! [`PersistenceBackend`](crate::storage::traits::PersistenceBackend) were implemented
//! in a single 1957-line file. This module was created by extracting shared code into
//! reusable utilities, reducing duplication by ~440 lines (22%) and improving maintainability.
//!
//! ## Module Structure
//!
//! - [`connection`]: Connection handling ([`Mutex<Connection>`](rusqlite::Connection), lock acquisition, configuration)
//! - [`sql`]: SQL helper functions (LIKE escaping, glob patterns, filter building)
//! - [`memory_row`]: Row conversion logic for [`Memory`](crate::models::Memory) objects
//! - [`metrics`]: Shared metrics recording helpers
//!
//! ## Design Principles
//!
//! - **DRY**: Single source of truth for common `SQLite` operations
//! - **Independence**: Each backend maintains its own connection (no shared state)
//! - **Graceful Degradation**: Backends can fail independently without affecting each other
//! - **Performance**: `SQLite` WAL mode enables excellent concurrency even with separate connections

// Module exports - will be populated as submodules are created
mod connection;
mod memory_row;
mod metrics;
mod sql;

// Public re-exports will be added here as modules are created
pub use connection::{
    MUTEX_LOCK_TIMEOUT, acquire_lock, acquire_lock_with_timeout, configure_connection,
};
pub use memory_row::{MemoryRow, build_memory_from_row, fetch_memory_row};
pub use metrics::record_operation_metrics;
pub use sql::{build_filter_clause_numbered, escape_like_wildcards, glob_to_like_pattern};
