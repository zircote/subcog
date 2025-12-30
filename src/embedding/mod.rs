//! Embedding generation.
//!
//! Provides embedding generation using fastembed or fallback to BM25-only.

// Allow cast precision loss for hash-based embedding calculations.
#![allow(clippy::cast_precision_loss)]
// Allow cast possible truncation for hash index calculations on 32-bit platforms.
#![allow(clippy::cast_possible_truncation)]

mod fallback;
mod fastembed;

pub use fallback::FallbackEmbedder;
pub use fastembed::FastEmbedEmbedder;

use crate::Result;

/// Trait for embedding generators.
pub trait Embedder: Send + Sync {
    /// Returns the embedding dimensions.
    fn dimensions(&self) -> usize;

    /// Generates an embedding for the given text.
    ///
    /// # Errors
    ///
    /// Returns an error if embedding generation fails.
    fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Generates embeddings for multiple texts.
    ///
    /// # Errors
    ///
    /// Returns an error if embedding generation fails.
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }
}
