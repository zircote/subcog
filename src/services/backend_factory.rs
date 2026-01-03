//! Backend factory for storage layer initialization.
//!
//! This module centralizes backend creation to:
//! - Reduce code duplication between `for_repo` and `for_user`
//! - Enable easier backend swapping for testing
//! - Provide consistent error handling and graceful degradation
//!
//! # Architecture
//!
//! ```text
//! BackendFactory
//!   ├── create_embedder() → Arc<dyn Embedder>
//!   ├── create_index_backend() → Option<Arc<dyn IndexBackend>>
//!   └── create_vector_backend() → Option<Arc<dyn VectorBackend>>
//! ```
//!
//! # Graceful Degradation
//!
//! Factory methods return `Option` for backends that may fail to initialize.
//! This allows the service container to continue with reduced functionality.

use crate::embedding::{Embedder, FastEmbedEmbedder};
use crate::storage::index::SqliteBackend;
use crate::storage::traits::{IndexBackend, VectorBackend};
use crate::storage::vector::UsearchBackend;
use std::path::Path;
use std::sync::Arc;

/// Result of backend initialization with optional components.
///
/// All backends are wrapped in `Arc` for shared ownership across services.
#[derive(Default)]
pub struct BackendSet {
    /// Embedder for generating vector embeddings.
    pub embedder: Option<Arc<dyn Embedder>>,
    /// Index backend for full-text search (`SQLite` FTS5).
    pub index: Option<Arc<dyn IndexBackend + Send + Sync>>,
    /// Vector backend for similarity search (usearch HNSW).
    pub vector: Option<Arc<dyn VectorBackend + Send + Sync>>,
}

impl BackendSet {
    /// Returns true if all backends were successfully initialized.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.embedder.is_some() && self.index.is_some() && self.vector.is_some()
    }

    /// Returns true if at least the embedder is available.
    #[must_use]
    pub fn has_embedder(&self) -> bool {
        self.embedder.is_some()
    }

    /// Returns true if full-text search is available.
    #[must_use]
    pub fn has_index(&self) -> bool {
        self.index.is_some()
    }

    /// Returns true if vector similarity search is available.
    #[must_use]
    pub fn has_vector(&self) -> bool {
        self.vector.is_some()
    }
}

/// Factory for creating storage backends.
///
/// Centralizes backend initialization with consistent error handling.
///
/// # Example
///
/// ```rust,ignore
/// use subcog::services::{BackendFactory, PathManager};
///
/// let paths = PathManager::for_repo("/path/to/repo");
/// let backends = BackendFactory::create_all(&paths);
///
/// if backends.is_complete() {
///     println!("All backends initialized successfully");
/// }
/// ```
pub struct BackendFactory;

impl BackendFactory {
    /// Creates all backends using the provided path configuration.
    ///
    /// Returns a `BackendSet` with successfully initialized backends.
    /// Failed backends are logged and set to `None`.
    ///
    /// # Arguments
    ///
    /// * `index_path` - Path for `SQLite` index database
    /// * `vector_path` - Path for vector index files
    ///
    /// # Returns
    ///
    /// A `BackendSet` containing available backends.
    #[must_use]
    pub fn create_all(index_path: &Path, vector_path: &Path) -> BackendSet {
        let embedder = Self::create_embedder();
        let index = Self::create_index_backend(index_path);
        let vector = Self::create_vector_backend(vector_path);

        BackendSet {
            embedder,
            index,
            vector,
        }
    }

    /// Creates the embedder backend.
    ///
    /// Currently always returns `FastEmbedEmbedder`. In the future,
    /// this could be configured to use different embedding models.
    #[must_use]
    pub fn create_embedder() -> Option<Arc<dyn Embedder>> {
        Some(Arc::new(FastEmbedEmbedder::new()))
    }

    /// Creates the index backend (`SQLite` FTS5).
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the `SQLite` database file
    ///
    /// # Returns
    ///
    /// `Some(backend)` on success, `None` if initialization fails.
    pub fn create_index_backend(path: &Path) -> Option<Arc<dyn IndexBackend + Send + Sync>> {
        match SqliteBackend::new(path) {
            Ok(backend) => {
                tracing::debug!(path = %path.display(), "Created SQLite index backend");
                Some(Arc::new(backend))
            },
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "Failed to create SQLite index backend"
                );
                None
            },
        }
    }

    /// Creates the vector backend (usearch HNSW).
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the vector index directory
    ///
    /// # Returns
    ///
    /// `Some(backend)` on success, `None` if initialization fails.
    pub fn create_vector_backend(path: &Path) -> Option<Arc<dyn VectorBackend + Send + Sync>> {
        let dimensions = FastEmbedEmbedder::DEFAULT_DIMENSIONS;

        #[cfg(feature = "usearch-hnsw")]
        let result = UsearchBackend::new(path, dimensions);

        #[cfg(not(feature = "usearch-hnsw"))]
        let result: crate::Result<UsearchBackend> = Ok(UsearchBackend::new(path, dimensions));

        match result {
            Ok(backend) => {
                tracing::debug!(path = %path.display(), "Created usearch vector backend");
                Some(Arc::new(backend))
            },
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "Failed to create usearch vector backend"
                );
                None
            },
        }
    }

    /// Creates backends with custom embedder dimensions.
    ///
    /// Useful for testing or when using non-default embedding models.
    ///
    /// # Arguments
    ///
    /// * `index_path` - Path for `SQLite` index database
    /// * `vector_path` - Path for vector index files
    /// * `dimensions` - Vector embedding dimensions
    #[must_use]
    pub fn create_with_dimensions(
        index_path: &Path,
        vector_path: &Path,
        dimensions: usize,
    ) -> BackendSet {
        let embedder = Self::create_embedder();
        let index = Self::create_index_backend(index_path);

        #[cfg(feature = "usearch-hnsw")]
        let vector_result = UsearchBackend::new(vector_path, dimensions);
        #[cfg(not(feature = "usearch-hnsw"))]
        let vector_result: crate::Result<UsearchBackend> =
            Ok(UsearchBackend::new(vector_path, dimensions));

        let vector = match vector_result {
            Ok(backend) => Some(Arc::new(backend) as Arc<dyn VectorBackend + Send + Sync>),
            Err(e) => {
                tracing::warn!(
                    path = %vector_path.display(),
                    dimensions,
                    error = %e,
                    "Failed to create usearch vector backend with custom dimensions"
                );
                None
            },
        };

        BackendSet {
            embedder,
            index,
            vector,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_backend_set_default() {
        let set = BackendSet::default();
        assert!(!set.is_complete());
        assert!(!set.has_embedder());
        assert!(!set.has_index());
        assert!(!set.has_vector());
    }

    #[test]
    fn test_create_embedder() {
        let embedder = BackendFactory::create_embedder();
        assert!(embedder.is_some());
    }

    #[test]
    fn test_create_index_backend() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let index_path = temp_dir.path().join("test_index.db");

        let index = BackendFactory::create_index_backend(&index_path);
        assert!(index.is_some());
    }

    #[test]
    fn test_create_index_backend_invalid_path() {
        // Try to create index in non-existent deeply nested path
        let invalid_path = std::path::Path::new("/nonexistent/deeply/nested/path/index.db");
        let index = BackendFactory::create_index_backend(invalid_path);
        assert!(index.is_none());
    }

    #[test]
    fn test_create_all() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let index_path = temp_dir.path().join("index.db");
        let vector_path = temp_dir.path().join("vectors");

        let backends = BackendFactory::create_all(&index_path, &vector_path);

        assert!(backends.has_embedder());
        assert!(backends.has_index());
        // Vector backend may or may not succeed depending on feature flags
    }

    #[test]
    fn test_backend_set_partial() {
        let set = BackendSet {
            embedder: BackendFactory::create_embedder(),
            ..Default::default()
        };

        assert!(!set.is_complete());
        assert!(set.has_embedder());
        assert!(!set.has_index());
        assert!(!set.has_vector());
    }
}
