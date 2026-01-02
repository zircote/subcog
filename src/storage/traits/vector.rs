//! Vector backend trait (DOC-H4).
//!
//! Provides the abstraction layer for vector similarity search backends.
//! Implementations use HNSW or similar algorithms for approximate nearest neighbor search.
//!
//! # Available Implementations
//!
//! | Backend | Use Case | Configuration |
//! |---------|----------|---------------|
//! | `UsearchBackend` | Local HNSW index | Default, no external deps |
//! | `PgVectorBackend` | PostgreSQL with pgvector | Requires PostgreSQL + pgvector extension |
//! | `RedisVectorBackend` | Redis Vector Search | Requires Redis Stack |
//!
//! # Usage Example
//!
//! ```rust,ignore
//! use subcog::storage::vector::UsearchBackend;
//! use subcog::storage::traits::VectorBackend;
//! use subcog::models::{MemoryId, SearchFilter};
//!
//! // Create a 384-dimensional vector backend (MiniLM-L6 embeddings)
//! let mut backend = UsearchBackend::new(384)?;
//!
//! // Insert embeddings
//! let embedding: Vec<f32> = generate_embedding("Use PostgreSQL for storage");
//! backend.upsert(&MemoryId::new("mem-001"), &embedding)?;
//!
//! // Search for similar vectors
//! let query_embedding: Vec<f32> = generate_embedding("database storage choice");
//! let results = backend.search(&query_embedding, &SearchFilter::new(), 10)?;
//!
//! for (id, similarity) in results {
//!     println!("{}: {:.2}% similar", id.as_str(), similarity * 100.0);
//! }
//! ```
//!
//! # Hybrid Search
//!
//! Vector search is typically combined with BM25 text search using Reciprocal Rank Fusion:
//!
//! ```rust,ignore
//! use subcog::services::RecallService;
//!
//! // RecallService automatically combines vector + BM25 results
//! let service = RecallService::new(config)?;
//! let results = service.search("database decisions", SearchFilter::new(), 10)?;
//! ```

use crate::Result;
use crate::models::{MemoryId, SearchFilter};

/// Trait for vector layer backends.
///
/// Vector backends provide similarity search using embedding vectors.
/// Implementations should be thread-safe (`Send + Sync`).
///
/// # Dimensionality
///
/// All embeddings must match the backend's [`dimensions()`](VectorBackend::dimensions).
/// The default `FastEmbed` model (`all-MiniLM-L6-v2`) produces 384-dimensional vectors.
pub trait VectorBackend: Send + Sync {
    /// The dimensionality of embedding vectors.
    fn dimensions(&self) -> usize;

    /// Inserts or updates an embedding for a memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the upsert operation fails.
    fn upsert(&mut self, id: &MemoryId, embedding: &[f32]) -> Result<()>;

    /// Removes an embedding by memory ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the removal operation fails.
    fn remove(&mut self, id: &MemoryId) -> Result<bool>;

    /// Searches for similar embeddings.
    ///
    /// Returns memory IDs with their cosine similarity scores (0.0 to 1.0),
    /// ordered by descending similarity.
    ///
    /// # Errors
    ///
    /// Returns an error if the search operation fails.
    fn search(
        &self,
        query_embedding: &[f32],
        filter: &SearchFilter,
        limit: usize,
    ) -> Result<Vec<(MemoryId, f32)>>;

    /// Returns the total count of indexed embeddings.
    ///
    /// # Errors
    ///
    /// Returns an error if the count operation fails.
    fn count(&self) -> Result<usize>;

    /// Clears all embeddings.
    ///
    /// # Errors
    ///
    /// Returns an error if the clear operation fails.
    fn clear(&mut self) -> Result<()>;
}
