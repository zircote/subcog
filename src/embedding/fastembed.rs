//! FastEmbed-based embedder.

use super::Embedder;
use crate::{Error, Result};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// `FastEmbed` embedder using all-MiniLM-L6-v2.
///
/// Note: This is a placeholder implementation that generates deterministic
/// pseudo-embeddings based on content hashing. For production use, integrate
/// the actual `fastembed-rs` crate.
pub struct FastEmbedEmbedder {
    /// Embedding dimensions.
    dimensions: usize,
    /// Whether the embedder is initialized.
    initialized: bool,
}

impl FastEmbedEmbedder {
    /// Default embedding dimensions for all-MiniLM-L6-v2.
    pub const DEFAULT_DIMENSIONS: usize = 384;

    /// Creates a new `FastEmbed` embedder.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            dimensions: Self::DEFAULT_DIMENSIONS,
            initialized: true,
        }
    }

    /// Creates a new embedder with custom dimensions.
    #[must_use]
    pub const fn with_dimensions(dimensions: usize) -> Self {
        Self {
            dimensions,
            initialized: true,
        }
    }

    /// Generates a deterministic pseudo-embedding from text.
    ///
    /// This creates a normalized vector based on content hashing.
    /// Not suitable for semantic similarity but useful for testing.
    fn pseudo_embed(&self, text: &str) -> Vec<f32> {
        let mut embedding = vec![0.0f32; self.dimensions];

        // Generate deterministic values based on text content
        // Iterate directly without collecting to avoid allocation
        for (i, word) in text.split_whitespace().enumerate() {
            let mut hasher = DefaultHasher::new();
            word.hash(&mut hasher);
            let hash = hasher.finish();

            // Distribute hash across embedding dimensions
            for j in 0..8 {
                let idx = ((hash >> (j * 8)) as usize + i) % self.dimensions;
                let value = ((hash >> (j * 4)) & 0xFF) as f32 / 255.0 - 0.5;
                embedding[idx] += value;
            }
        }

        // Normalize the embedding using SIMD-friendly pattern
        let norm_sq: f32 = embedding.iter().map(|x| x * x).sum();
        if norm_sq > 0.0 {
            let inv_norm = norm_sq.sqrt().recip();
            for v in &mut embedding {
                *v *= inv_norm;
            }
        }

        embedding
    }
}

impl Default for FastEmbedEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

impl Embedder for FastEmbedEmbedder {
    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        if !self.initialized {
            return Err(Error::OperationFailed {
                operation: "embed".to_string(),
                cause: "Embedder not initialized".to_string(),
            });
        }

        if text.is_empty() {
            return Err(Error::InvalidInput("Cannot embed empty text".to_string()));
        }

        // For now, use pseudo-embedding
        // In production, this would call fastembed-rs
        Ok(self.pseudo_embed(text))
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if !self.initialized {
            return Err(Error::OperationFailed {
                operation: "embed_batch".to_string(),
                cause: "Embedder not initialized".to_string(),
            });
        }

        texts.iter().map(|t| self.embed(t)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedder_creation() {
        let embedder = FastEmbedEmbedder::new();
        assert_eq!(embedder.dimensions(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
    }

    #[test]
    fn test_custom_dimensions() {
        let embedder = FastEmbedEmbedder::with_dimensions(512);
        assert_eq!(embedder.dimensions(), 512);
    }

    #[test]
    fn test_embed_success() {
        let embedder = FastEmbedEmbedder::new();
        let result = embedder.embed("Hello, world!");

        assert!(result.is_ok());
        let embedding = result.unwrap();
        assert_eq!(embedding.len(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
    }

    #[test]
    fn test_embed_empty_text() {
        let embedder = FastEmbedEmbedder::new();
        let result = embedder.embed("");
        assert!(result.is_err());
    }

    #[test]
    fn test_embed_deterministic() {
        let embedder = FastEmbedEmbedder::new();
        let text = "Rust programming language";

        let result1 = embedder.embed(text);
        let result2 = embedder.embed(text);

        // Same text should produce same embedding
        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let emb1 = result1.unwrap();
        let emb2 = result2.unwrap();

        for (v1, v2) in emb1.iter().zip(emb2.iter()) {
            assert!((v1 - v2).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn test_embed_different_text() {
        let embedder = FastEmbedEmbedder::new();

        let result1 = embedder.embed("Rust programming");
        let result2 = embedder.embed("Python scripting");

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        // Different text should produce different embeddings
        let emb1 = result1.unwrap();
        let emb2 = result2.unwrap();

        let different = emb1
            .iter()
            .zip(emb2.iter())
            .any(|(v1, v2)| (v1 - v2).abs() > f32::EPSILON);
        assert!(different);
    }

    #[test]
    fn test_embed_normalized() {
        let embedder = FastEmbedEmbedder::new();
        let result = embedder.embed("Test embedding normalization");

        assert!(result.is_ok());
        let emb = result.unwrap();

        // Check that the embedding is normalized (magnitude ~= 1)
        let magnitude: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_embed_batch() {
        let embedder = FastEmbedEmbedder::new();
        let texts = vec!["First text", "Second text", "Third text"];

        let result = embedder.embed_batch(&texts);
        assert!(result.is_ok());

        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), 3);

        for emb in &embeddings {
            assert_eq!(emb.len(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
        }
    }

    // ============================================================================
    // Additional Embedding Generation Tests
    // ============================================================================

    #[test]
    fn test_embed_whitespace_only() {
        let embedder = FastEmbedEmbedder::new();

        // Whitespace-only should produce an embedding (not empty text)
        let result = embedder.embed("   \t\n  ");
        // Depending on implementation, could be error or valid embedding
        // Current implementation: whitespace splits to no words, produces zero vector
        assert!(result.is_ok());
    }

    #[test]
    fn test_embed_single_word() {
        let embedder = FastEmbedEmbedder::new();
        let result = embedder.embed("hello");

        assert!(result.is_ok());
        let embedding = result.unwrap();
        assert_eq!(embedding.len(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);

        // Should be normalized
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(magnitude > 0.9 && magnitude < 1.1);
    }

    #[test]
    fn test_embed_unicode_text() {
        let embedder = FastEmbedEmbedder::new();

        // Unicode text should embed without error
        let result = embedder.embed("Hello 世界 🌍 café");
        assert!(result.is_ok());

        let embedding = result.unwrap();
        assert_eq!(embedding.len(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
    }

    #[test]
    fn test_embed_very_long_text() {
        let embedder = FastEmbedEmbedder::new();

        // Create a long text
        let long_text = "word ".repeat(10000);
        let result = embedder.embed(&long_text);

        assert!(result.is_ok());
        let embedding = result.unwrap();
        assert_eq!(embedding.len(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
    }

    #[test]
    fn test_embed_special_characters() {
        let embedder = FastEmbedEmbedder::new();

        let result = embedder.embed("!@#$%^&*()_+-=[]{}|;':\",./<>?");
        assert!(result.is_ok());
    }

    #[test]
    fn test_embed_batch_with_empty_fails() {
        let embedder = FastEmbedEmbedder::new();
        let texts = vec!["Valid text", "", "Another valid"];

        // Batch with empty string should fail
        let result = embedder.embed_batch(&texts);
        assert!(result.is_err());
    }

    #[test]
    fn test_embed_batch_empty_list() {
        let embedder = FastEmbedEmbedder::new();
        let texts: Vec<&str> = vec![];

        let result = embedder.embed_batch(&texts);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_embed_batch_single_item() {
        let embedder = FastEmbedEmbedder::new();
        let texts = vec!["Single item"];

        let result = embedder.embed_batch(&texts);
        assert!(result.is_ok());

        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), 1);
    }

    #[test]
    fn test_embed_case_sensitivity() {
        let embedder = FastEmbedEmbedder::new();

        let lower = embedder.embed("hello world").unwrap();
        let upper = embedder.embed("HELLO WORLD").unwrap();
        let mixed = embedder.embed("Hello World").unwrap();

        // Different cases should produce different embeddings
        let lower_upper_different = lower
            .iter()
            .zip(upper.iter())
            .any(|(a, b)| (a - b).abs() > f32::EPSILON);
        let lower_mixed_different = lower
            .iter()
            .zip(mixed.iter())
            .any(|(a, b)| (a - b).abs() > f32::EPSILON);

        assert!(lower_upper_different);
        assert!(lower_mixed_different);
    }

    #[test]
    fn test_embed_word_order_matters() {
        let embedder = FastEmbedEmbedder::new();

        let emb1 = embedder.embed("the quick brown fox").unwrap();
        let emb2 = embedder.embed("brown quick the fox").unwrap();

        // Different word order should produce different embeddings
        let different = emb1
            .iter()
            .zip(emb2.iter())
            .any(|(a, b)| (a - b).abs() > f32::EPSILON);
        assert!(different);
    }

    #[test]
    fn test_embed_numeric_text() {
        let embedder = FastEmbedEmbedder::new();

        let result = embedder.embed("12345 67890");
        assert!(result.is_ok());

        let embedding = result.unwrap();
        assert_eq!(embedding.len(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
    }

    #[test]
    fn test_embedder_default_trait() {
        let embedder = FastEmbedEmbedder::default();
        assert_eq!(embedder.dimensions(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
    }

    #[test]
    fn test_custom_dimensions_embed() {
        let embedder = FastEmbedEmbedder::with_dimensions(128);

        let result = embedder.embed("Test with custom dimensions");
        assert!(result.is_ok());

        let embedding = result.unwrap();
        assert_eq!(embedding.len(), 128);
    }

    #[test]
    fn test_embed_all_values_finite() {
        let embedder = FastEmbedEmbedder::new();
        let result = embedder.embed("Test for finite values");

        assert!(result.is_ok());
        let embedding = result.unwrap();

        // All values should be finite (not NaN or Inf)
        for val in &embedding {
            assert!(
                val.is_finite(),
                "Embedding contains non-finite value: {val}"
            );
        }
    }

    #[test]
    fn test_embed_values_in_range() {
        let embedder = FastEmbedEmbedder::new();
        let result = embedder.embed("Test for value range");

        assert!(result.is_ok());
        let embedding = result.unwrap();

        // Normalized embeddings should have values roughly in [-1, 1]
        for val in &embedding {
            assert!(
                *val >= -2.0 && *val <= 2.0,
                "Value {val} outside expected range"
            );
        }
    }

    #[test]
    fn test_embed_similarity_for_similar_text() {
        let embedder = FastEmbedEmbedder::new();

        let emb1 = embedder.embed("Rust programming language").unwrap();
        let emb2 = embedder.embed("Rust programming").unwrap();

        // Calculate cosine similarity
        let dot_product: f32 = emb1.iter().zip(emb2.iter()).map(|(a, b)| a * b).sum();
        let norm1: f32 = emb1.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm2: f32 = emb2.iter().map(|x| x * x).sum::<f32>().sqrt();
        let similarity = dot_product / (norm1 * norm2);

        // Similar text should have positive similarity
        assert!(
            similarity > 0.0,
            "Expected positive similarity for similar text, got {similarity}"
        );
    }
}
