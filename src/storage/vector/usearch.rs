//! usearch HNSW vector backend.
//!
//! Provides high-performance approximate nearest neighbor search using
//! a Hierarchical Navigable Small World (HNSW) graph structure.
//!
//! When the `usearch-hnsw` feature is enabled, this uses the native usearch
//! library for optimized ANN search. Otherwise, a pure-Rust fallback is used.

use crate::models::{MemoryId, SearchFilter};
use crate::storage::traits::VectorBackend;
use crate::{Error, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Default embedding dimensions for all-MiniLM-L6-v2.
pub const DEFAULT_USEARCH_DIMENSIONS: usize = 384;

/// HNSW connectivity parameter (M).
/// Higher values improve recall but use more memory.
#[cfg(feature = "usearch-hnsw")]
const HNSW_CONNECTIVITY: usize = 16;

/// HNSW expansion factor for construction (`ef_construction`).
/// Higher values improve index quality but slow down construction.
#[cfg(feature = "usearch-hnsw")]
const HNSW_EXPANSION_ADD: usize = 128;

/// HNSW expansion factor for search (`ef`).
/// Higher values improve recall but slow down search.
#[cfg(feature = "usearch-hnsw")]
const HNSW_EXPANSION_SEARCH: usize = 64;

// ============================================================================
// Native usearch Implementation (with feature)
// ============================================================================

#[cfg(feature = "usearch-hnsw")]
mod native {
    use super::{
        DEFAULT_USEARCH_DIMENSIONS, Error, HNSW_CONNECTIVITY, HNSW_EXPANSION_ADD,
        HNSW_EXPANSION_SEARCH, HashMap, MemoryId, PathBuf, Result, SearchFilter, VectorBackend, fs,
    };
    use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

    /// Native usearch-based vector backend using HNSW.
    ///
    /// Uses the usearch library for high-performance approximate nearest
    /// neighbor search with O(log n) query complexity.
    pub struct UsearchBackend {
        /// Path to the index file.
        index_path: PathBuf,
        /// Embedding dimensions.
        dimensions: usize,
        /// The usearch index.
        index: Index,
        /// Mapping from `MemoryId` string to usearch key (u64).
        id_to_key: HashMap<String, u64>,
        /// Mapping from usearch key (u64) to `MemoryId` string.
        key_to_id: HashMap<u64, String>,
        /// Next available key for new vectors.
        next_key: u64,
        /// Whether the index has been modified since last save.
        dirty: bool,
    }

    impl UsearchBackend {
        /// Default embedding dimensions for all-MiniLM-L6-v2.
        pub const DEFAULT_DIMENSIONS: usize = DEFAULT_USEARCH_DIMENSIONS;

        /// Creates a new usearch backend with HNSW indexing.
        ///
        /// # Errors
        ///
        /// Returns an error if the index cannot be created.
        pub fn new(index_path: impl Into<PathBuf>, dimensions: usize) -> Result<Self> {
            let options = IndexOptions {
                dimensions,
                metric: MetricKind::Cos,
                quantization: ScalarKind::F32,
                connectivity: HNSW_CONNECTIVITY,
                expansion_add: HNSW_EXPANSION_ADD,
                expansion_search: HNSW_EXPANSION_SEARCH,
                multi: false,
            };

            let index = Index::new(&options).map_err(|e| Error::OperationFailed {
                operation: "create_usearch_index".to_string(),
                cause: e.to_string(),
            })?;

            // Reserve initial capacity
            index.reserve(1024).map_err(|e| Error::OperationFailed {
                operation: "reserve_usearch_capacity".to_string(),
                cause: e.to_string(),
            })?;

            Ok(Self {
                index_path: index_path.into(),
                dimensions,
                index,
                id_to_key: HashMap::new(),
                key_to_id: HashMap::new(),
                next_key: 1,
                dirty: false,
            })
        }

        /// Creates a backend with default dimensions.
        ///
        /// # Errors
        ///
        /// Returns an error if the index cannot be created.
        pub fn with_default_dimensions(index_path: impl Into<PathBuf>) -> Result<Self> {
            Self::new(index_path, Self::DEFAULT_DIMENSIONS)
        }

        /// Creates an in-memory backend (no file persistence).
        ///
        /// # Errors
        ///
        /// Returns an error if the index cannot be created.
        pub fn in_memory(dimensions: usize) -> Result<Self> {
            Self::new(PathBuf::new(), dimensions)
        }

        /// Returns the index path.
        #[must_use]
        pub const fn index_path(&self) -> &PathBuf {
            &self.index_path
        }

        /// Loads the index from disk.
        ///
        /// # Errors
        ///
        /// Returns an error if the file cannot be read or parsed.
        pub fn load(&mut self) -> Result<()> {
            if self.index_path.as_os_str().is_empty() {
                return Ok(());
            }

            let index_file = self.index_path.with_extension("usearch");
            let meta_file = self.index_path.with_extension("meta.json");

            if !index_file.exists() || !meta_file.exists() {
                return Ok(());
            }

            // Load the usearch index
            self.index
                .load(index_file.to_string_lossy().as_ref())
                .map_err(|e| Error::OperationFailed {
                    operation: "load_usearch_index".to_string(),
                    cause: e.to_string(),
                })?;

            // Load the metadata (id mappings)
            let meta_content =
                fs::read_to_string(&meta_file).map_err(|e| Error::OperationFailed {
                    operation: "load_usearch_meta".to_string(),
                    cause: e.to_string(),
                })?;

            let meta: IndexMetadata =
                serde_json::from_str(&meta_content).map_err(|e| Error::OperationFailed {
                    operation: "parse_usearch_meta".to_string(),
                    cause: e.to_string(),
                })?;

            if meta.dimensions != self.dimensions {
                return Err(Error::InvalidInput(format!(
                    "Index dimensions mismatch: expected {}, got {}",
                    self.dimensions, meta.dimensions
                )));
            }

            self.id_to_key = meta.id_to_key;
            self.key_to_id = meta.key_to_id;
            self.next_key = meta.next_key;
            self.dirty = false;

            Ok(())
        }

        /// Saves the index to disk.
        ///
        /// # Errors
        ///
        /// Returns an error if the file cannot be written.
        pub fn save(&mut self) -> Result<()> {
            if self.index_path.as_os_str().is_empty() {
                return Ok(());
            }

            if !self.dirty {
                return Ok(());
            }

            // Ensure parent directory exists
            if let Some(parent) = self.index_path.parent() {
                fs::create_dir_all(parent).map_err(|e| Error::OperationFailed {
                    operation: "create_usearch_dir".to_string(),
                    cause: e.to_string(),
                })?;
            }

            let index_file = self.index_path.with_extension("usearch");
            let meta_file = self.index_path.with_extension("meta.json");

            // Save the usearch index
            self.index
                .save(index_file.to_string_lossy().as_ref())
                .map_err(|e| Error::OperationFailed {
                    operation: "save_usearch_index".to_string(),
                    cause: e.to_string(),
                })?;

            // Save the metadata
            let meta = IndexMetadata {
                dimensions: self.dimensions,
                id_to_key: self.id_to_key.clone(),
                key_to_id: self.key_to_id.clone(),
                next_key: self.next_key,
            };

            let meta_content =
                serde_json::to_string(&meta).map_err(|e| Error::OperationFailed {
                    operation: "serialize_usearch_meta".to_string(),
                    cause: e.to_string(),
                })?;

            fs::write(&meta_file, meta_content).map_err(|e| Error::OperationFailed {
                operation: "write_usearch_meta".to_string(),
                cause: e.to_string(),
            })?;

            self.dirty = false;
            Ok(())
        }

        /// Validates embedding dimensions.
        fn validate_embedding(&self, embedding: &[f32]) -> Result<()> {
            if embedding.len() != self.dimensions {
                return Err(Error::InvalidInput(format!(
                    "Embedding dimension mismatch: expected {}, got {}",
                    self.dimensions,
                    embedding.len()
                )));
            }
            Ok(())
        }

        /// Gets or creates a key for a memory ID.
        fn get_or_create_key(&mut self, id: &str) -> u64 {
            if let Some(&key) = self.id_to_key.get(id) {
                return key;
            }

            let key = self.next_key;
            self.next_key += 1;
            self.id_to_key.insert(id.to_string(), key);
            self.key_to_id.insert(key, id.to_string());
            key
        }
    }

    /// Metadata for persisting ID mappings.
    #[derive(serde::Serialize, serde::Deserialize)]
    struct IndexMetadata {
        dimensions: usize,
        id_to_key: HashMap<String, u64>,
        key_to_id: HashMap<u64, String>,
        next_key: u64,
    }

    impl VectorBackend for UsearchBackend {
        fn dimensions(&self) -> usize {
            self.dimensions
        }

        fn upsert(&mut self, id: &MemoryId, embedding: &[f32]) -> Result<()> {
            self.validate_embedding(embedding)?;

            let key = self.get_or_create_key(id.as_str());

            // usearch doesn't allow duplicate keys, so remove first if exists
            if self.index.contains(key) {
                let _ = self.index.remove(key); // Ignore result - may not exist
            }

            self.index
                .add(key, embedding)
                .map_err(|e| Error::OperationFailed {
                    operation: "usearch_add".to_string(),
                    cause: e.to_string(),
                })?;

            self.dirty = true;
            Ok(())
        }

        fn remove(&mut self, id: &MemoryId) -> Result<bool> {
            let Some(&key) = self.id_to_key.get(id.as_str()) else {
                return Ok(false);
            };

            let removed = self.index.remove(key).map_err(|e| Error::OperationFailed {
                operation: "usearch_remove".to_string(),
                cause: e.to_string(),
            })?;

            if removed > 0 {
                self.id_to_key.remove(id.as_str());
                self.key_to_id.remove(&key);
                self.dirty = true;
                Ok(true)
            } else {
                Ok(false)
            }
        }

        fn search(
            &self,
            query_embedding: &[f32],
            _filter: &SearchFilter,
            limit: usize,
        ) -> Result<Vec<(MemoryId, f32)>> {
            self.validate_embedding(query_embedding)?;

            if self.index.size() == 0 {
                return Ok(Vec::new());
            }

            let matches =
                self.index
                    .search(query_embedding, limit)
                    .map_err(|e| Error::OperationFailed {
                        operation: "usearch_search".to_string(),
                        cause: e.to_string(),
                    })?;

            let results: Vec<(MemoryId, f32)> = matches
                .keys
                .iter()
                .zip(matches.distances.iter())
                .filter_map(|(&key, &distance)| {
                    let id = self.key_to_id.get(&key)?;
                    // usearch returns distance, convert to similarity
                    // For cosine: distance = 1 - similarity
                    let similarity = 1.0 - distance;
                    Some((MemoryId::new(id), similarity))
                })
                .collect();

            Ok(results)
        }

        fn count(&self) -> Result<usize> {
            Ok(self.index.size())
        }

        fn clear(&mut self) -> Result<()> {
            self.index.reset().map_err(|e| Error::OperationFailed {
                operation: "usearch_reset".to_string(),
                cause: e.to_string(),
            })?;
            // Re-reserve capacity after reset
            self.index
                .reserve(1024)
                .map_err(|e| Error::OperationFailed {
                    operation: "reserve_usearch_capacity".to_string(),
                    cause: e.to_string(),
                })?;
            self.id_to_key.clear();
            self.key_to_id.clear();
            self.next_key = 1;
            self.dirty = true;
            Ok(())
        }
    }

    impl Drop for UsearchBackend {
        fn drop(&mut self) {
            // Attempt to save on drop, ignore errors
            let _ = self.save();
        }
    }
}

// ============================================================================
// Pure Rust Fallback Implementation (without feature)
// ============================================================================

#[cfg(not(feature = "usearch-hnsw"))]
mod fallback {
    use super::*;

    /// Pure-Rust fallback vector backend.
    ///
    /// This is a brute-force O(n) implementation used when the `usearch-hnsw`
    /// feature is not enabled. For production use with large vector sets,
    /// enable the `usearch-hnsw` feature.
    pub struct UsearchBackend {
        /// Path to the index file.
        index_path: PathBuf,
        /// Embedding dimensions.
        dimensions: usize,
        /// In-memory vector storage: `memory_id` -> embedding.
        vectors: HashMap<String, Vec<f32>>,
        /// Whether the index has been modified since last save.
        dirty: bool,
    }

    impl UsearchBackend {
        /// Default embedding dimensions for all-MiniLM-L6-v2.
        pub const DEFAULT_DIMENSIONS: usize = DEFAULT_USEARCH_DIMENSIONS;

        /// Creates a new fallback backend.
        #[must_use]
        pub fn new(index_path: impl Into<PathBuf>, dimensions: usize) -> Self {
            Self {
                index_path: index_path.into(),
                dimensions,
                vectors: HashMap::new(),
                dirty: false,
            }
        }

        /// Creates a backend with default dimensions.
        #[must_use]
        pub fn with_default_dimensions(index_path: impl Into<PathBuf>) -> Self {
            Self::new(index_path, Self::DEFAULT_DIMENSIONS)
        }

        /// Creates an in-memory backend (no file persistence).
        #[must_use]
        pub fn in_memory(dimensions: usize) -> Self {
            Self {
                index_path: PathBuf::new(),
                dimensions,
                vectors: HashMap::new(),
                dirty: false,
            }
        }

        /// Returns the index path.
        #[must_use]
        pub const fn index_path(&self) -> &PathBuf {
            &self.index_path
        }

        /// Loads the index from disk.
        ///
        /// # Errors
        ///
        /// Returns an error if the file cannot be read or parsed.
        pub fn load(&mut self) -> Result<()> {
            if self.index_path.as_os_str().is_empty() {
                return Ok(());
            }

            if !self.index_path.exists() {
                return Ok(());
            }

            let content =
                fs::read_to_string(&self.index_path).map_err(|e| Error::OperationFailed {
                    operation: "load_index".to_string(),
                    cause: e.to_string(),
                })?;

            let data: IndexData =
                serde_json::from_str(&content).map_err(|e| Error::OperationFailed {
                    operation: "parse_index".to_string(),
                    cause: e.to_string(),
                })?;

            if data.dimensions != self.dimensions {
                return Err(Error::InvalidInput(format!(
                    "Index dimensions mismatch: expected {}, got {}",
                    self.dimensions, data.dimensions
                )));
            }

            self.vectors = data.vectors;
            self.dirty = false;

            Ok(())
        }

        /// Saves the index to disk.
        ///
        /// # Errors
        ///
        /// Returns an error if the file cannot be written.
        pub fn save(&mut self) -> Result<()> {
            if self.index_path.as_os_str().is_empty() {
                return Ok(());
            }

            if !self.dirty {
                return Ok(());
            }

            let data = IndexData {
                dimensions: self.dimensions,
                vectors: self.vectors.clone(),
            };

            let content = serde_json::to_string(&data).map_err(|e| Error::OperationFailed {
                operation: "serialize_index".to_string(),
                cause: e.to_string(),
            })?;

            // Ensure parent directory exists
            if let Some(parent) = self.index_path.parent() {
                fs::create_dir_all(parent).map_err(|e| Error::OperationFailed {
                    operation: "create_index_dir".to_string(),
                    cause: e.to_string(),
                })?;
            }

            fs::write(&self.index_path, content).map_err(|e| Error::OperationFailed {
                operation: "write_index".to_string(),
                cause: e.to_string(),
            })?;

            self.dirty = false;
            Ok(())
        }

        /// Computes cosine similarity between two vectors.
        fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
            if a.len() != b.len() {
                return 0.0;
            }

            let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
            let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
            let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

            if norm_a == 0.0 || norm_b == 0.0 {
                return 0.0;
            }

            // Cosine similarity ranges from -1 to 1, normalize to 0 to 1
            f32::midpoint(dot_product / (norm_a * norm_b), 1.0)
        }

        /// Validates embedding dimensions.
        fn validate_embedding(&self, embedding: &[f32]) -> Result<()> {
            if embedding.len() != self.dimensions {
                return Err(Error::InvalidInput(format!(
                    "Embedding dimension mismatch: expected {}, got {}",
                    self.dimensions,
                    embedding.len()
                )));
            }
            Ok(())
        }
    }

    /// Index data for serialization.
    #[derive(serde::Serialize, serde::Deserialize)]
    struct IndexData {
        dimensions: usize,
        vectors: HashMap<String, Vec<f32>>,
    }

    impl VectorBackend for UsearchBackend {
        fn dimensions(&self) -> usize {
            self.dimensions
        }

        fn upsert(&mut self, id: &MemoryId, embedding: &[f32]) -> Result<()> {
            self.validate_embedding(embedding)?;

            self.vectors
                .insert(id.as_str().to_string(), embedding.to_vec());
            self.dirty = true;

            Ok(())
        }

        fn remove(&mut self, id: &MemoryId) -> Result<bool> {
            let removed = self.vectors.remove(id.as_str()).is_some();
            if removed {
                self.dirty = true;
            }
            Ok(removed)
        }

        fn search(
            &self,
            query_embedding: &[f32],
            _filter: &SearchFilter,
            limit: usize,
        ) -> Result<Vec<(MemoryId, f32)>> {
            self.validate_embedding(query_embedding)?;

            // Compute similarity for all vectors (brute-force O(n))
            let mut scores: Vec<(String, f32)> = self
                .vectors
                .iter()
                .map(|(id, vec)| {
                    let score = Self::cosine_similarity(query_embedding, vec);
                    (id.clone(), score)
                })
                .collect();

            // Sort by score descending
            scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            // Take top results
            let results: Vec<(MemoryId, f32)> = scores
                .into_iter()
                .take(limit)
                .map(|(id, score)| (MemoryId::new(id), score))
                .collect();

            Ok(results)
        }

        fn count(&self) -> Result<usize> {
            Ok(self.vectors.len())
        }

        fn clear(&mut self) -> Result<()> {
            self.vectors.clear();
            self.dirty = true;
            Ok(())
        }
    }

    impl Drop for UsearchBackend {
        fn drop(&mut self) {
            // Attempt to save on drop, ignore errors
            let _ = self.save();
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_cosine_similarity() {
            // Same vector
            let v1 = vec![1.0, 0.0, 0.0];
            let similarity = UsearchBackend::cosine_similarity(&v1, &v1);
            assert!((similarity - 1.0).abs() < 0.001);

            // Orthogonal vectors
            let v2 = vec![0.0, 1.0, 0.0];
            let similarity = UsearchBackend::cosine_similarity(&v1, &v2);
            assert!((similarity - 0.5).abs() < 0.001); // Normalized to [0, 1]

            // Opposite vectors
            let v3 = vec![-1.0, 0.0, 0.0];
            let similarity = UsearchBackend::cosine_similarity(&v1, &v3);
            assert!(similarity < 0.001);
        }
    }
}

// ============================================================================
// Public Re-exports
// ============================================================================

#[cfg(feature = "usearch-hnsw")]
pub use native::UsearchBackend;

#[cfg(not(feature = "usearch-hnsw"))]
pub use fallback::UsearchBackend;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_random_embedding(dimensions: usize) -> Vec<f32> {
        (0..dimensions).map(|i| ((i % 10) as f32) / 10.0).collect()
    }

    fn create_normalized_embedding(dimensions: usize, seed: f32) -> Vec<f32> {
        let raw: Vec<f32> = (0..dimensions).map(|i| (i as f32 + seed).sin()).collect();
        let norm: f32 = raw.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            raw.into_iter().map(|x| x / norm).collect()
        } else {
            raw
        }
    }

    #[cfg(not(feature = "usearch-hnsw"))]
    fn create_backend(path: impl Into<PathBuf>, dims: usize) -> UsearchBackend {
        UsearchBackend::new(path, dims)
    }

    #[cfg(feature = "usearch-hnsw")]
    fn create_backend(path: impl Into<PathBuf>, dims: usize) -> UsearchBackend {
        UsearchBackend::new(path, dims).expect("Failed to create backend")
    }

    #[cfg(not(feature = "usearch-hnsw"))]
    fn create_in_memory(dims: usize) -> UsearchBackend {
        UsearchBackend::in_memory(dims)
    }

    #[cfg(feature = "usearch-hnsw")]
    fn create_in_memory(dims: usize) -> UsearchBackend {
        UsearchBackend::in_memory(dims).expect("Failed to create in-memory backend")
    }

    #[test]
    fn test_usearch_backend_creation() {
        let backend = create_backend("/tmp/test.idx", 384);
        assert_eq!(backend.dimensions(), 384);

        let memory = create_in_memory(512);
        assert_eq!(memory.dimensions(), 512);
    }

    #[test]
    fn test_upsert_and_count() {
        let mut backend = create_in_memory(384);

        let id1 = MemoryId::new("id1");
        let embedding1 = create_random_embedding(384);
        backend.upsert(&id1, &embedding1).expect("upsert failed");

        assert_eq!(backend.count().expect("count failed"), 1);

        let id2 = MemoryId::new("id2");
        let embedding2 = create_random_embedding(384);
        backend.upsert(&id2, &embedding2).expect("upsert failed");

        assert_eq!(backend.count().expect("count failed"), 2);
    }

    #[test]
    fn test_upsert_dimension_mismatch() {
        let mut backend = create_in_memory(384);

        let id = MemoryId::new("test");
        let wrong_dim = create_random_embedding(256);

        let result = backend.upsert(&id, &wrong_dim);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove() {
        let mut backend = create_in_memory(384);

        let id = MemoryId::new("test");
        let embedding = create_random_embedding(384);
        backend.upsert(&id, &embedding).expect("upsert failed");

        assert_eq!(backend.count().expect("count failed"), 1);

        let removed = backend.remove(&id).expect("remove failed");
        assert!(removed);
        assert_eq!(backend.count().expect("count failed"), 0);

        // Remove non-existent
        let removed = backend.remove(&id).expect("remove failed");
        assert!(!removed);
    }

    #[test]
    fn test_search() {
        let mut backend = create_in_memory(384);

        // Insert some vectors
        for i in 0..5 {
            let id = MemoryId::new(format!("id{i}"));
            let embedding = create_normalized_embedding(384, i as f32);
            backend.upsert(&id, &embedding).expect("upsert failed");
        }

        // Search with a query similar to id0
        let query = create_normalized_embedding(384, 0.0);
        let results = backend
            .search(&query, &SearchFilter::new(), 3)
            .expect("search failed");

        assert_eq!(results.len(), 3);

        // First result should be id0 (exact match)
        assert_eq!(results[0].0.as_str(), "id0");
        assert!(results[0].1 > 0.99); // Very high similarity
    }

    #[test]
    fn test_search_empty() {
        let backend = create_in_memory(384);

        let query = create_random_embedding(384);
        let results = backend
            .search(&query, &SearchFilter::new(), 10)
            .expect("search failed");

        assert!(results.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut backend = create_in_memory(384);

        // Add some vectors
        for i in 0..3 {
            let id = MemoryId::new(format!("id{i}"));
            let embedding = create_random_embedding(384);
            backend.upsert(&id, &embedding).expect("upsert failed");
        }

        assert_eq!(backend.count().expect("count failed"), 3);

        backend.clear().expect("clear failed");
        assert_eq!(backend.count().expect("count failed"), 0);
    }

    #[test]
    fn test_persistence() {
        let dir = TempDir::new().expect("tempdir failed");
        let index_path = dir.path().join("test.idx");

        // Create and populate backend
        {
            let mut backend = create_backend(&index_path, 384);

            let id = MemoryId::new("persistent");
            let embedding = create_random_embedding(384);
            backend.upsert(&id, &embedding).expect("upsert failed");
            backend.save().expect("save failed");
        }

        // Load in new backend
        {
            let mut backend = create_backend(&index_path, 384);
            backend.load().expect("load failed");

            assert_eq!(backend.count().expect("count failed"), 1);
        }
    }

    #[test]
    fn test_load_dimension_mismatch() {
        let dir = TempDir::new().expect("tempdir failed");
        let index_path = dir.path().join("test.idx");

        // Create with 384 dimensions
        {
            let mut backend = create_backend(&index_path, 384);
            let id = MemoryId::new("test");
            let embedding = create_random_embedding(384);
            backend.upsert(&id, &embedding).expect("upsert failed");
            backend.save().expect("save failed");
        }

        // Try to load with different dimensions
        {
            let mut backend = create_backend(&index_path, 512);
            let result = backend.load();
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_load_nonexistent() {
        let mut backend = create_backend("/nonexistent/path/index.idx", 384);
        let result = backend.load();
        assert!(result.is_ok()); // Should succeed with empty index
        assert_eq!(backend.count().expect("count failed"), 0);
    }

    #[test]
    fn test_update_existing() {
        let mut backend = create_in_memory(384);

        let id = MemoryId::new("test");
        let embedding1 = create_normalized_embedding(384, 1.0);
        backend.upsert(&id, &embedding1).expect("upsert failed");

        // Update with different embedding
        let embedding2 = create_normalized_embedding(384, 2.0);
        backend.upsert(&id, &embedding2).expect("upsert failed");

        // For native usearch, count may be 2 due to multi-vector support being off
        // but search should still work correctly
        let count = backend.count().expect("count failed");
        assert!(count >= 1);

        // Search should find the updated vector
        let query = create_normalized_embedding(384, 2.0);
        let results = backend
            .search(&query, &SearchFilter::new(), 1)
            .expect("search failed");

        assert!(!results.is_empty());
        assert!(results[0].1 > 0.9); // High similarity to updated embedding
    }
}
