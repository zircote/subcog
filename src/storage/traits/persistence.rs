//! Persistence backend trait.
//!
//! The persistence layer is the authoritative source of truth for all memories.
//! It handles durable storage, ensuring memories survive process restarts.
//!
//! # Available Implementations
//!
//! | Backend | Use Case | Trade-offs |
//! |---------|----------|------------|
//! | `GitNotesBackend` | Primary; portable, versioned | Requires git repo |
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
//! | `GitNotes` | Single-op | None | On flush |
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
//! - **Concurrent writes**: `GitNotes` uses file locking; `PostgreSQL` uses transactions
//! - **Partial failures**: `GitNotes` may leave `.lock` files; `PostgreSQL` rolls back

use crate::Result;
use crate::models::{Memory, MemoryId};

/// Trait for persistence layer backends.
///
/// Persistence backends are the authoritative source of truth for memories.
/// They handle long-term storage and retrieval.
///
/// # Implementor Notes
///
/// - All methods must be thread-safe (`Send + Sync` bound)
/// - Prefer returning `Error::NotFound` over `None` for missing IDs
/// - Use structured error variants from [`crate::Error`]
pub trait PersistenceBackend: Send + Sync {
    /// Stores a memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage operation fails.
    fn store(&mut self, memory: &Memory) -> Result<()>;

    /// Retrieves a memory by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the retrieval operation fails.
    fn get(&self, id: &MemoryId) -> Result<Option<Memory>>;

    /// Deletes a memory by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion operation fails.
    fn delete(&mut self, id: &MemoryId) -> Result<bool>;

    /// Lists all memory IDs.
    ///
    /// # Errors
    ///
    /// Returns an error if the list operation fails.
    fn list_ids(&self) -> Result<Vec<MemoryId>>;

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
