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
pub use fastembed::{FastEmbedEmbedder, cosine_similarity};

/// Default embedding dimensions for the all-MiniLM-L6-v2 model.
///
/// This is the authoritative source for embedding dimensions across the codebase (QUAL-H2).
/// All vector backends should use this constant for consistency.
pub const DEFAULT_DIMENSIONS: usize = 384;

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
