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
//! use subcog::storage::traits::{VectorBackend, VectorFilter};
//! use subcog::models::MemoryId;
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
//! let results = backend.search(&query_embedding, &VectorFilter::new(), 10)?;
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
use crate::models::{Domain, MemoryId, Namespace, SearchFilter};

/// Filter criteria specific to vector similarity search.
///
/// This type provides a subset of [`SearchFilter`] fields that are applicable
/// to vector search operations, making the API more type-safe and explicit.
///
/// # Fields
///
/// | Field | Description |
/// |-------|-------------|
/// | `namespaces` | Filter by memory namespaces |
/// | `domains` | Filter by memory domains |
/// | `min_score` | Minimum cosine similarity threshold (0.0 to 1.0) |
///
/// # Example
///
/// ```rust
/// use subcog::storage::traits::VectorFilter;
/// use subcog::models::Namespace;
///
/// let filter = VectorFilter::new()
///     .with_namespace(Namespace::Decisions)
///     .with_min_score(0.7);
/// ```
#[derive(Debug, Clone, Default)]
pub struct VectorFilter {
    /// Filter by namespaces.
    pub namespaces: Vec<Namespace>,
    /// Filter by domains.
    pub domains: Vec<Domain>,
    /// Minimum similarity score (0.0 to 1.0).
    pub min_score: Option<f32>,
}

impl VectorFilter {
    /// Creates an empty filter (matches all).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            namespaces: Vec::new(),
            domains: Vec::new(),
            min_score: None,
        }
    }

    /// Adds a namespace filter.
    #[must_use]
    pub fn with_namespace(mut self, namespace: Namespace) -> Self {
        self.namespaces.push(namespace);
        self
    }

    /// Adds a domain filter.
    #[must_use]
    pub fn with_domain(mut self, domain: Domain) -> Self {
        self.domains.push(domain);
        self
    }

    /// Sets the minimum score threshold.
    #[must_use]
    pub const fn with_min_score(mut self, score: f32) -> Self {
        self.min_score = Some(score);
        self
    }

    /// Returns true if the filter is empty (matches all).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.namespaces.is_empty() && self.domains.is_empty() && self.min_score.is_none()
    }
}

impl From<&SearchFilter> for VectorFilter {
    /// Converts a [`SearchFilter`] to a [`VectorFilter`], extracting only
    /// the fields applicable to vector search.
    fn from(filter: &SearchFilter) -> Self {
        Self {
            namespaces: filter.namespaces.clone(),
            domains: filter.domains.clone(),
            min_score: filter.min_score,
        }
    }
}

impl From<SearchFilter> for VectorFilter {
    fn from(filter: SearchFilter) -> Self {
        Self::from(&filter)
    }
}

/// Trait for vector layer backends.
///
/// Vector backends provide similarity search using embedding vectors.
/// Implementations should be thread-safe (`Send + Sync`).
///
/// # Implementor Notes
///
/// - Methods use `&self` to enable sharing via `Arc<dyn VectorBackend>`
/// - Use interior mutability (e.g., `Mutex<HashMap<K,V>>`) for mutable state
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
    /// Uses interior mutability for thread-safe concurrent access.
    ///
    /// # Errors
    ///
    /// Returns an error if the upsert operation fails.
    fn upsert(&self, id: &MemoryId, embedding: &[f32]) -> Result<()>;

    /// Removes an embedding by memory ID.
    ///
    /// Uses interior mutability for thread-safe concurrent access.
    ///
    /// # Errors
    ///
    /// Returns an error if the removal operation fails.
    fn remove(&self, id: &MemoryId) -> Result<bool>;

    /// Searches for similar embeddings.
    ///
    /// Returns memory IDs with their cosine similarity scores (0.0 to 1.0),
    /// ordered by descending similarity.
    ///
    /// # Arguments
    ///
    /// * `query_embedding` - The query vector to find similar embeddings for
    /// * `filter` - A [`VectorFilter`] for namespace/domain filtering and min score threshold
    /// * `limit` - Maximum number of results to return
    ///
    /// # Errors
    ///
    /// Returns an error if the search operation fails.
    fn search(
        &self,
        query_embedding: &[f32],
        filter: &VectorFilter,
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
    /// Uses interior mutability for thread-safe concurrent access.
    ///
    /// # Errors
    ///
    /// Returns an error if the clear operation fails.
    fn clear(&self) -> Result<()>;
}
