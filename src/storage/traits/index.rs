//! Index backend trait.
//!
//! The index layer provides full-text search capabilities using BM25 or similar algorithms.
//! It enables keyword-based retrieval of memories.
//!
//! # Available Implementations
//!
//! | Backend | Use Case | Features |
//! |---------|----------|----------|
//! | `SqliteBackend` | Default; embedded | FTS5 with BM25 ranking |
//! | `PostgresBackend` | Multi-user | `ts_rank` with stemming |
//! | `RediSearchBackend` | High throughput | Prefix/fuzzy matching |
//!
//! # Error Modes and Guarantees
//!
//! All backends return `Result<T>` with errors propagated via [`crate::Error`].
//!
//! ## Indexing Behavior
//!
//! | Backend | Atomicity | Index Lag | Rebuild Cost |
//! |---------|-----------|-----------|--------------|
//! | `SQLite` | Transactional | Immediate | O(n) |
//! | PostgreSQL | Transactional | Immediate | O(n log n) |
//! | `RediSearch` | Eventual | <100ms | O(n) |
//!
//! ## Error Recovery
//!
//! | Error Type | Recovery Strategy |
//! |------------|-------------------|
//! | `Error::Storage` | Check DB connection; retry |
//! | `Error::InvalidInput` | Query syntax error; validate before calling |
//! | `Error::OperationFailed` | Index corruption; call `reindex()` |
//!
//! ## Consistency with Persistence Layer
//!
//! The index is a **derived view** of the persistence layer. If the index becomes
//! stale or corrupted, call `reindex()` to rebuild from the authoritative persistence store.
//!
//! ## Performance Characteristics
//!
//! - **Search complexity**: O(log n) for indexed queries
//! - **Batch efficiency**: `get_memories_batch()` avoids N+1 query pattern
//! - **FTS tokenization**: Whitespace + punctuation split (`SQLite`), language-aware (`PostgreSQL`)

use crate::Result;
use crate::models::{Memory, MemoryId, SearchFilter};

/// Trait for index layer backends.
///
/// Index backends provide full-text search capabilities using BM25 or similar algorithms.
///
/// # Implementor Notes
///
/// - Methods use `&self` to enable sharing via `Arc<dyn IndexBackend>`
/// - Use interior mutability (e.g., `Mutex<Connection>`) for mutable state
/// - Implement `get_memories_batch()` with an optimized query (e.g., SQL `IN` clause)
/// - Use FTS ranking scores for the `f32` score in search results
/// - Ensure `clear()` does not affect the persistence layer
pub trait IndexBackend: Send + Sync {
    /// Indexes a memory for full-text search.
    ///
    /// Uses interior mutability for thread-safe concurrent access.
    ///
    /// # Errors
    ///
    /// Returns an error if the indexing operation fails.
    fn index(&self, memory: &Memory) -> Result<()>;

    /// Removes a memory from the index.
    ///
    /// Uses interior mutability for thread-safe concurrent access.
    ///
    /// # Errors
    ///
    /// Returns an error if the removal operation fails.
    fn remove(&self, id: &MemoryId) -> Result<bool>;

    /// Searches for memories matching a text query.
    ///
    /// Returns memory IDs with their BM25 scores, ordered by relevance.
    ///
    /// # Errors
    ///
    /// Returns an error if the search operation fails.
    fn search(
        &self,
        query: &str,
        filter: &SearchFilter,
        limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>>;

    /// Re-indexes all memories.
    ///
    /// Uses interior mutability for thread-safe concurrent access.
    ///
    /// # Errors
    ///
    /// Returns an error if any memory fails to index.
    fn reindex(&self, memories: &[Memory]) -> Result<()> {
        for memory in memories {
            self.index(memory)?;
        }
        Ok(())
    }

    /// Clears the entire index.
    ///
    /// Uses interior mutability for thread-safe concurrent access.
    ///
    /// # Errors
    ///
    /// Returns an error if the clear operation fails.
    fn clear(&self) -> Result<()>;

    /// Lists all indexed memories, optionally filtered.
    ///
    /// Unlike `search`, this doesn't require a query and returns all entries.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn list_all(&self, filter: &SearchFilter, limit: usize) -> Result<Vec<(MemoryId, f32)>>;

    /// Retrieves a memory by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn get_memory(&self, id: &MemoryId) -> Result<Option<Memory>>;

    /// Retrieves multiple memories by their IDs in a single batch query.
    ///
    /// This is more efficient than calling `get_memory` in a loop (N+1 query pattern).
    /// Returns memories in the same order as the input IDs, with None for missing IDs.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn get_memories_batch(&self, ids: &[MemoryId]) -> Result<Vec<Option<Memory>>> {
        // Default implementation falls back to individual queries
        ids.iter().map(|id| self.get_memory(id)).collect()
    }
}
