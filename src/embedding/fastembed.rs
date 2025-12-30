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
    pub fn new() -> Self {
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
        let words: Vec<&str> = text.split_whitespace().collect();

        for (i, word) in words.iter().enumerate() {
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

        // Normalize the embedding
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut embedding {
                *v /= norm;
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
        let embedding = result.ok();
        assert!(embedding.is_some());
        let embedding = embedding.as_ref();
        assert!(embedding.map_or(false, |e| e.len() == FastEmbedEmbedder::DEFAULT_DIMENSIONS));
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

        let result1 = embedder.embed(text).ok();
        let result2 = embedder.embed(text).ok();

        // Same text should produce same embedding
        assert!(result1.is_some());
        assert!(result2.is_some());

        let emb1 = result1.as_ref();
        let emb2 = result2.as_ref();

        if let (Some(e1), Some(e2)) = (emb1, emb2) {
            for (v1, v2) in e1.iter().zip(e2.iter()) {
                assert!((v1 - v2).abs() < f32::EPSILON);
            }
        }
    }

    #[test]
    fn test_embed_different_text() {
        let embedder = FastEmbedEmbedder::new();

        let result1 = embedder.embed("Rust programming").ok();
        let result2 = embedder.embed("Python scripting").ok();

        assert!(result1.is_some());
        assert!(result2.is_some());

        // Different text should produce different embeddings
        let emb1 = result1.as_ref();
        let emb2 = result2.as_ref();

        if let (Some(e1), Some(e2)) = (emb1, emb2) {
            let mut different = false;
            for (v1, v2) in e1.iter().zip(e2.iter()) {
                if (v1 - v2).abs() > f32::EPSILON {
                    different = true;
                    break;
                }
            }
            assert!(different);
        }
    }

    #[test]
    fn test_embed_normalized() {
        let embedder = FastEmbedEmbedder::new();
        let result = embedder.embed("Test embedding normalization");

        assert!(result.is_ok());
        let embedding = result.ok();
        assert!(embedding.is_some());

        if let Some(emb) = embedding {
            // Check that the embedding is normalized (magnitude ~= 1)
            let magnitude: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((magnitude - 1.0).abs() < 0.01);
        }
    }

    #[test]
    fn test_embed_batch() {
        let embedder = FastEmbedEmbedder::new();
        let texts = vec!["First text", "Second text", "Third text"];

        let result = embedder.embed_batch(&texts);
        assert!(result.is_ok());

        let embeddings = result.ok();
        assert!(embeddings.is_some());
        let embeddings = embeddings.as_ref();
        assert!(embeddings.map_or(false, |e| e.len() == 3));

        if let Some(embs) = embeddings {
            for emb in embs {
                assert_eq!(emb.len(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
            }
        }
    }
}
