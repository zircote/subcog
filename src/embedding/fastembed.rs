//! FastEmbed-based embedder.

use super::Embedder;
use crate::Result;

/// FastEmbed embedder using all-MiniLM-L6-v2.
pub struct FastEmbedEmbedder {
    dimensions: usize,
}

impl FastEmbedEmbedder {
    /// Default embedding dimensions for all-MiniLM-L6-v2.
    pub const DEFAULT_DIMENSIONS: usize = 384;

    /// Creates a new FastEmbed embedder.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            dimensions: Self::DEFAULT_DIMENSIONS,
        }
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

    fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        // TODO: Implement fastembed integration
        todo!("FastEmbedEmbedder::embed not yet implemented")
    }
}
