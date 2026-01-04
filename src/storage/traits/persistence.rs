//! Persistence backend trait.
//!
//! The persistence layer is the authoritative source of truth for all memories.
//! It handles durable storage, ensuring memories survive process restarts.
//!
//! # Available Implementations
//!
//! | Backend | Use Case | Trade-offs |
//! |---------|----------|------------|
//! | `SqliteBackend` | Primary; embedded, ACID | Single-process access |
//! | `PostgresBackend` | Multi-user, ACID | Requires PostgreSQL server |
//! | `FilesystemBackend` | Fallback; simple | No transactional guarantees |
//!
//! # Error Modes and Guarantees
//!
//! All backends return `Result<T>` with errors propagated via [`crate::Error`].
//!
//! ## Transactional Behavior
//!
//! | Backend | Atomicity | Isolation | Durability |
//! |---------|-----------|-----------|------------|
//! | `SQLite` | Full ACID | Serializable | On commit (WAL) |
//! | PostgreSQL | Full ACID | Serializable | On commit |
//! | Filesystem | None | None | On fsync |
//!
//! ## Error Recovery
//!
//! | Error Type | Recovery Strategy |
//! |------------|-------------------|
//! | `Error::Storage` | Retry with exponential backoff |
//! | `Error::NotFound` | Expected for missing IDs; handle gracefully |
//! | `Error::InvalidInput` | Validate input before calling |
//! | `Error::OperationFailed` | Log and surface to user |
//!
//! ## Consistency Guarantees
//!
//! - **Read-after-write**: Guaranteed for all backends
//! - **Concurrent writes**: `SQLite` uses WAL mode with busy timeout; `PostgreSQL` uses transactions
//! - **Partial failures**: `SQLite` rolls back; `PostgreSQL` rolls back

use crate::Result;
use crate::models::{Memory, MemoryId};

/// Trait for persistence layer backends.
///
/// Persistence backends are the authoritative source of truth for memories.
/// They handle long-term storage and retrieval.
///
/// # Implementor Notes
///
/// - Methods use `&self` to enable sharing via `Arc<dyn PersistenceBackend>`
/// - Use interior mutability (e.g., `Mutex`) for mutable state
/// - All methods must be thread-safe (`Send + Sync` bound)
/// - Prefer returning `Error::NotFound` over `None` for missing IDs
/// - Use structured error variants from [`crate::Error`]
pub trait PersistenceBackend: Send + Sync {
    /// Stores a memory.
    ///
    /// Uses interior mutability for thread-safe concurrent access.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage operation fails.
    fn store(&self, memory: &Memory) -> Result<()>;

    /// Retrieves a memory by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the retrieval operation fails.
    fn get(&self, id: &MemoryId) -> Result<Option<Memory>>;

    /// Deletes a memory by ID.
    ///
    /// Uses interior mutability for thread-safe concurrent access.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion operation fails.
    fn delete(&self, id: &MemoryId) -> Result<bool>;

    /// Lists all memory IDs.
    ///
    /// # Errors
    ///
    /// Returns an error if the list operation fails.
    fn list_ids(&self) -> Result<Vec<MemoryId>>;

    /// Retrieves multiple memories by their IDs in a single batch operation.
    ///
    /// This method avoids N+1 queries by fetching all requested memories
    /// in a single database round-trip (where supported by the backend).
    ///
    /// # Default Implementation
    ///
    /// Falls back to calling `get()` for each ID. Backends should override
    /// this with an optimized batch query (e.g., `SELECT ... WHERE id IN (...)`).
    ///
    /// # Errors
    ///
    /// Returns an error if any retrieval operation fails.
    fn get_batch(&self, ids: &[MemoryId]) -> Result<Vec<Memory>> {
        ids.iter()
            .filter_map(|id| self.get(id).transpose())
            .collect()
    }

    /// Checks if a memory exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the existence check fails.
    fn exists(&self, id: &MemoryId) -> Result<bool> {
        Ok(self.get(id)?.is_some())
    }

    /// Returns the total count of memories.
    ///
    /// # Errors
    ///
    /// Returns an error if the count operation fails.
    fn count(&self) -> Result<usize> {
        Ok(self.list_ids()?.len())
    }
}
