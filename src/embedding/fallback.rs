//! Fallback embedder (BM25-only mode).

use super::Embedder;
use crate::Result;

/// Fallback embedder that returns empty vectors.
///
/// Used when embedding is not available, falling back to BM25-only search.
pub struct FallbackEmbedder;

impl FallbackEmbedder {
    /// Creates a new fallback embedder.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for FallbackEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

impl Embedder for FallbackEmbedder {
    fn dimensions(&self) -> usize {
        0
    }

    fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        Ok(Vec::new())
    }
}
