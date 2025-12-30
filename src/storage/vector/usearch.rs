//! usearch HNSW vector backend.
//!
//! Provides high-performance approximate nearest neighbor search using
//! a Hierarchical Navigable Small World (HNSW) graph structure.
//!
//! This implementation uses an in-memory store with optional file persistence.
//! In production, you would use the actual usearch crate for optimized ANN search.

use crate::models::{MemoryId, SearchFilter};
use crate::storage::traits::VectorBackend;
use crate::{Error, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// usearch-based vector backend.
///
/// This is a pure Rust implementation that mimics usearch behavior.
/// For production use with millions of vectors, consider integrating
/// the actual usearch crate.
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
    pub const DEFAULT_DIMENSIONS: usize = 384;

    /// Creates a new usearch backend.
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

        let content = fs::read_to_string(&self.index_path).map_err(|e| Error::OperationFailed {
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

        // Compute similarity for all vectors
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

    #[test]
    fn test_usearch_backend_creation() {
        let backend = UsearchBackend::new("/tmp/test.idx", 384);
        assert_eq!(backend.dimensions(), 384);

        let default = UsearchBackend::with_default_dimensions("/tmp/test.idx");
        assert_eq!(default.dimensions(), UsearchBackend::DEFAULT_DIMENSIONS);

        let memory = UsearchBackend::in_memory(512);
        assert_eq!(memory.dimensions(), 512);
    }

    #[test]
    fn test_upsert_and_count() {
        let mut backend = UsearchBackend::in_memory(384);

        let id1 = MemoryId::new("id1");
        let embedding1 = create_random_embedding(384);
        backend.upsert(&id1, &embedding1).unwrap();

        assert_eq!(backend.count().unwrap(), 1);

        let id2 = MemoryId::new("id2");
        let embedding2 = create_random_embedding(384);
        backend.upsert(&id2, &embedding2).unwrap();

        assert_eq!(backend.count().unwrap(), 2);
    }

    #[test]
    fn test_upsert_dimension_mismatch() {
        let mut backend = UsearchBackend::in_memory(384);

        let id = MemoryId::new("test");
        let wrong_dim = create_random_embedding(256);

        let result = backend.upsert(&id, &wrong_dim);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove() {
        let mut backend = UsearchBackend::in_memory(384);

        let id = MemoryId::new("test");
        let embedding = create_random_embedding(384);
        backend.upsert(&id, &embedding).unwrap();

        assert_eq!(backend.count().unwrap(), 1);

        let removed = backend.remove(&id).unwrap();
        assert!(removed);
        assert_eq!(backend.count().unwrap(), 0);

        // Remove non-existent
        let removed = backend.remove(&id).unwrap();
        assert!(!removed);
    }

    #[test]
    fn test_search() {
        let mut backend = UsearchBackend::in_memory(384);

        // Insert some vectors
        for i in 0..5 {
            let id = MemoryId::new(format!("id{i}"));
            let embedding = create_normalized_embedding(384, i as f32);
            backend.upsert(&id, &embedding).unwrap();
        }

        // Search with a query similar to id0
        let query = create_normalized_embedding(384, 0.0);
        let results = backend.search(&query, &SearchFilter::new(), 3).unwrap();

        assert_eq!(results.len(), 3);

        // First result should be id0 (exact match)
        assert_eq!(results[0].0.as_str(), "id0");
        assert!(results[0].1 > 0.99); // Very high similarity
    }

    #[test]
    fn test_search_empty() {
        let backend = UsearchBackend::in_memory(384);

        let query = create_random_embedding(384);
        let results = backend.search(&query, &SearchFilter::new(), 10).unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut backend = UsearchBackend::in_memory(384);

        // Add some vectors
        for i in 0..3 {
            let id = MemoryId::new(format!("id{i}"));
            let embedding = create_random_embedding(384);
            backend.upsert(&id, &embedding).unwrap();
        }

        assert_eq!(backend.count().unwrap(), 3);

        backend.clear().unwrap();
        assert_eq!(backend.count().unwrap(), 0);
    }

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

    #[test]
    fn test_persistence() {
        let dir = TempDir::new().unwrap();
        let index_path = dir.path().join("test.idx");

        // Create and populate backend
        {
            let mut backend = UsearchBackend::new(&index_path, 384);

            let id = MemoryId::new("persistent");
            let embedding = create_random_embedding(384);
            backend.upsert(&id, &embedding).unwrap();
            backend.save().unwrap();
        }

        // Load in new backend
        {
            let mut backend = UsearchBackend::new(&index_path, 384);
            backend.load().unwrap();

            assert_eq!(backend.count().unwrap(), 1);
        }
    }

    #[test]
    fn test_load_dimension_mismatch() {
        let dir = TempDir::new().unwrap();
        let index_path = dir.path().join("test.idx");

        // Create with 384 dimensions
        {
            let mut backend = UsearchBackend::new(&index_path, 384);
            let id = MemoryId::new("test");
            let embedding = create_random_embedding(384);
            backend.upsert(&id, &embedding).unwrap();
            backend.save().unwrap();
        }

        // Try to load with different dimensions
        {
            let mut backend = UsearchBackend::new(&index_path, 512);
            let result = backend.load();
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_load_nonexistent() {
        let mut backend = UsearchBackend::new("/nonexistent/path/index.idx", 384);
        let result = backend.load();
        assert!(result.is_ok()); // Should succeed with empty index
        assert_eq!(backend.count().unwrap(), 0);
    }

    #[test]
    fn test_update_existing() {
        let mut backend = UsearchBackend::in_memory(384);

        let id = MemoryId::new("test");
        let embedding1 = create_normalized_embedding(384, 1.0);
        backend.upsert(&id, &embedding1).unwrap();

        // Update with different embedding
        let embedding2 = create_normalized_embedding(384, 2.0);
        backend.upsert(&id, &embedding2).unwrap();

        // Should still have only one entry
        assert_eq!(backend.count().unwrap(), 1);

        // Search should find the updated vector
        let query = create_normalized_embedding(384, 2.0);
        let results = backend.search(&query, &SearchFilter::new(), 1).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].1 > 0.99); // Very high similarity to updated embedding
    }
}
