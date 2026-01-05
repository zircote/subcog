//! FastEmbed-based embedder.
//!
//! Provides semantic embeddings using the all-MiniLM-L6-v2 model via fastembed-rs.
//! When the `fastembed-embeddings` feature is enabled, this uses real ONNX-based
//! semantic embeddings. Otherwise, falls back to deterministic hash-based pseudo-embeddings.

use super::{DEFAULT_DIMENSIONS, Embedder};
use crate::{Error, Result};

// ============================================================================
// Native FastEmbed Implementation (with feature)
// ============================================================================

#[cfg(feature = "fastembed-embeddings")]
mod native {
    use super::{DEFAULT_DIMENSIONS, Embedder, Error, Result};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::OnceLock;
    use std::time::Instant;

    /// Thread-safe singleton for the embedding model.
    /// Uses `OnceLock` for lazy initialization on first use.
    static EMBEDDING_MODEL: OnceLock<fastembed::TextEmbedding> = OnceLock::new();

    /// `FastEmbed` embedder using all-MiniLM-L6-v2.
    ///
    /// Uses the fastembed-rs library for real semantic embeddings.
    /// The model is lazily loaded on first embed call to preserve cold start time.
    pub struct FastEmbedEmbedder {
        /// Model name for logging/debugging.
        model_name: &'static str,
    }

    impl FastEmbedEmbedder {
        /// Default embedding dimensions for all-MiniLM-L6-v2.
        pub const DEFAULT_DIMENSIONS: usize = DEFAULT_DIMENSIONS;

        /// Creates a new `FastEmbed` embedder.
        ///
        /// Note: Model is lazily loaded on first `embed()` call.
        #[must_use]
        pub const fn new() -> Self {
            Self {
                model_name: "all-MiniLM-L6-v2",
            }
        }

        /// Creates a new embedder with custom dimensions.
        ///
        /// Note: This is provided for API compatibility but dimensions are
        /// fixed by the model (384 for all-MiniLM-L6-v2).
        #[must_use]
        #[allow(clippy::unused_self)]
        pub const fn with_dimensions(_dimensions: usize) -> Self {
            // Dimensions are fixed by the model
            Self::new()
        }

        /// Gets or initializes the embedding model (thread-safe).
        ///
        /// # Performance Note
        ///
        /// The model is loaded lazily on first use to preserve cold start time.
        /// Subsequent calls return the cached instance.
        ///
        /// The first call blocks synchronously (~100-500ms) while loading the ONNX model.
        /// This is an intentional design decision:
        /// - One-time cost amortized over all subsequent calls (instant)
        /// - Sync API is simpler and doesn't require async runtime everywhere
        /// - Alternative (`tokio::spawn_blocking`) would require async `Embedder` trait
        ///
        /// For applications sensitive to first-call latency, consider warming up the
        /// embedder during startup: `FastEmbedEmbedder::new().embed("warmup").ok();`
        fn get_model() -> Result<&'static fastembed::TextEmbedding> {
            // Check if already initialized
            if let Some(model) = EMBEDDING_MODEL.get() {
                return Ok(model);
            }

            // Initialize the model
            tracing::info!("Loading embedding model (first use)...");
            let start = Instant::now();

            let options = fastembed::InitOptions::new(fastembed::EmbeddingModel::AllMiniLML6V2)
                .with_show_download_progress(false);

            let model =
                fastembed::TextEmbedding::try_new(options).map_err(|e| Error::OperationFailed {
                    operation: "load_embedding_model".to_string(),
                    cause: e.to_string(),
                })?;

            tracing::info!(
                elapsed_ms = start.elapsed().as_millis() as u64,
                model = "all-MiniLM-L6-v2",
                "Embedding model loaded successfully"
            );

            // Store the model, ignoring if another thread beat us to it
            let _ = EMBEDDING_MODEL.set(model);
            // Return the (possibly other thread's) model
            // SAFETY: We just set the model, so it must be present
            EMBEDDING_MODEL.get().ok_or_else(|| Error::OperationFailed {
                operation: "get_embedding_model".to_string(),
                cause: "Model initialization race condition".to_string(),
            })
        }

        /// Returns the model name.
        #[must_use]
        pub const fn model_name(&self) -> &'static str {
            self.model_name
        }
    }

    impl Default for FastEmbedEmbedder {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Embedder for FastEmbedEmbedder {
        fn dimensions(&self) -> usize {
            Self::DEFAULT_DIMENSIONS
        }

        fn embed(&self, text: &str) -> Result<Vec<f32>> {
            if text.is_empty() {
                return Err(Error::InvalidInput("Cannot embed empty text".to_string()));
            }

            let model = Self::get_model()?;
            let text_owned = text.to_string();

            // Wrap ONNX runtime call in catch_unwind for graceful degradation (RES-M1).
            // ONNX runtime can panic on malformed inputs or internal errors.
            // AssertUnwindSafe is safe here because we don't access any mutable state
            // after the panic, and fastembed::TextEmbedding is Send + Sync.
            let result = catch_unwind(AssertUnwindSafe(|| model.embed(vec![text_owned], None)));

            let embeddings = result
                .map_err(|panic_info| {
                    let panic_msg = panic_info
                        .downcast_ref::<&str>()
                        .map(|s| (*s).to_string())
                        .or_else(|| panic_info.downcast_ref::<String>().cloned())
                        .unwrap_or_else(|| "unknown panic".to_string());
                    tracing::error!(
                        panic_message = %panic_msg,
                        "ONNX runtime panicked during embedding"
                    );
                    Error::OperationFailed {
                        operation: "embed".to_string(),
                        cause: format!("ONNX runtime panic: {panic_msg}"),
                    }
                })?
                .map_err(|e| Error::OperationFailed {
                    operation: "embed".to_string(),
                    cause: e.to_string(),
                })?;

            embeddings
                .into_iter()
                .next()
                .ok_or_else(|| Error::OperationFailed {
                    operation: "embed".to_string(),
                    cause: "No embedding returned from model".to_string(),
                })
        }

        fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
            if texts.is_empty() {
                return Ok(Vec::new());
            }

            if texts.iter().any(|t| t.is_empty()) {
                return Err(Error::InvalidInput("Cannot embed empty text".to_string()));
            }

            let model = Self::get_model()?;

            // Convert &[&str] to Vec<String> for fastembed
            let texts_owned: Vec<String> = texts.iter().map(|s| (*s).to_string()).collect();

            // Wrap ONNX runtime call in catch_unwind for graceful degradation (RES-M1).
            let result = catch_unwind(AssertUnwindSafe(|| model.embed(texts_owned, None)));

            result
                .map_err(|panic_info| {
                    let panic_msg = panic_info
                        .downcast_ref::<&str>()
                        .map(|s| (*s).to_string())
                        .or_else(|| panic_info.downcast_ref::<String>().cloned())
                        .unwrap_or_else(|| "unknown panic".to_string());
                    tracing::error!(
                        panic_message = %panic_msg,
                        batch_size = texts.len(),
                        "ONNX runtime panicked during batch embedding"
                    );
                    Error::OperationFailed {
                        operation: "embed_batch".to_string(),
                        cause: format!("ONNX runtime panic: {panic_msg}"),
                    }
                })?
                .map_err(|e| Error::OperationFailed {
                    operation: "embed_batch".to_string(),
                    cause: e.to_string(),
                })
        }
    }
}

// ============================================================================
// Fallback Implementation (without feature)
// ============================================================================

#[cfg(not(feature = "fastembed-embeddings"))]
mod fallback {
    use super::{DEFAULT_DIMENSIONS, Embedder, Error, Result};
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    /// `FastEmbed` embedder using hash-based pseudo-embeddings.
    ///
    /// This is a placeholder implementation that generates deterministic
    /// pseudo-embeddings based on content hashing. For production use,
    /// enable the `fastembed-embeddings` feature.
    ///
    /// Note: Hash-based embeddings do NOT capture semantic similarity.
    /// "database storage" and "PostgreSQL database" will NOT be similar.
    pub struct FastEmbedEmbedder {
        /// Embedding dimensions.
        dimensions: usize,
        /// Whether the embedder is initialized.
        initialized: bool,
    }

    impl FastEmbedEmbedder {
        /// Default embedding dimensions for all-MiniLM-L6-v2.
        pub const DEFAULT_DIMENSIONS: usize = DEFAULT_DIMENSIONS;

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
        #[allow(clippy::cast_precision_loss)]
        #[allow(clippy::cast_possible_truncation)]
        fn pseudo_embed(&self, text: &str) -> Vec<f32> {
            // Limit word iteration to prevent DoS on very long texts (PERF-H1)
            const MAX_WORDS: usize = 1000;
            let mut embedding = vec![0.0f32; self.dimensions];

            // Generate deterministic values based on text content
            // Iterate directly without collecting to avoid allocation
            // Limit to MAX_WORDS to bound computation time
            for (i, word) in text.split_whitespace().take(MAX_WORDS).enumerate() {
                let mut hasher = DefaultHasher::new();
                word.hash(&mut hasher);
                let hash = hasher.finish();
                Self::distribute_hash(&mut embedding, hash, i, self.dimensions);
            }

            Self::normalize_embedding(&mut embedding);
            embedding
        }

        /// Distributes a hash value across embedding dimensions.
        #[allow(clippy::cast_precision_loss)]
        #[allow(clippy::cast_possible_truncation)]
        fn distribute_hash(embedding: &mut [f32], hash: u64, word_idx: usize, dimensions: usize) {
            for j in 0..8 {
                let idx = ((hash >> (j * 8)) as usize + word_idx) % dimensions;
                let value = ((hash >> (j * 4)) & 0xFF) as f32 / 255.0 - 0.5;
                embedding[idx] += value;
            }
        }

        /// Normalizes an embedding vector in-place.
        fn normalize_embedding(embedding: &mut [f32]) {
            let norm_sq: f32 = embedding.iter().map(|x| x * x).sum();
            if norm_sq <= 0.0 {
                return;
            }
            let inv_norm = norm_sq.sqrt().recip();
            for v in embedding.iter_mut() {
                *v *= inv_norm;
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

            // Use pseudo-embedding (hash-based fallback)
            // WARNING: This does NOT provide semantic similarity
            tracing::debug!(
                "Using pseudo-embedding fallback (fastembed-embeddings feature not enabled)"
            );
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
}

// ============================================================================
// Public Re-exports
// ============================================================================

#[cfg(feature = "fastembed-embeddings")]
pub use native::FastEmbedEmbedder;

#[cfg(not(feature = "fastembed-embeddings"))]
pub use fallback::FastEmbedEmbedder;

// ============================================================================
// Utility Functions
// ============================================================================

/// Computes cosine similarity between two embedding vectors.
///
/// # Arguments
///
/// * `a` - First embedding vector
/// * `b` - Second embedding vector
///
/// # Returns
///
/// Cosine similarity in range [-1.0, 1.0], or 0.0 if vectors are invalid.
#[must_use]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot_product / (norm_a * norm_b)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedder_creation() {
        let embedder = FastEmbedEmbedder::new();
        assert_eq!(embedder.dimensions(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
    }

    #[test]
    fn test_embed_empty_text() {
        let embedder = FastEmbedEmbedder::new();
        let result = embedder.embed("");
        assert!(result.is_err());
    }

    #[test]
    fn test_embedder_default_trait() {
        let embedder = FastEmbedEmbedder::default();
        assert_eq!(embedder.dimensions(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
    }

    #[test]
    fn test_embed_batch_empty_list() {
        let embedder = FastEmbedEmbedder::new();
        let texts: Vec<&str> = vec![];

        let result = embedder.embed_batch(&texts);
        assert!(result.is_ok());
        assert!(result.expect("embed_batch failed").is_empty());
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
    fn test_cosine_similarity_identical() {
        let v = vec![1.0, 0.0, 0.0];
        let similarity = cosine_similarity(&v, &v);
        assert!(
            (similarity - 1.0).abs() < 0.001,
            "Identical vectors should have similarity ~1.0"
        );
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        let similarity = cosine_similarity(&v1, &v2);
        assert!(
            similarity.abs() < 0.001,
            "Orthogonal vectors should have similarity ~0.0"
        );
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![-1.0, 0.0, 0.0];
        let similarity = cosine_similarity(&v1, &v2);
        assert!(
            (similarity + 1.0).abs() < 0.001,
            "Opposite vectors should have similarity ~-1.0"
        );
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let v1 = vec![1.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0];
        let similarity = cosine_similarity(&v1, &v2);
        assert!(
            similarity.abs() < f32::EPSILON,
            "Different length vectors should return 0.0, got {similarity}"
        );
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let v1: Vec<f32> = vec![];
        let v2: Vec<f32> = vec![];
        let similarity = cosine_similarity(&v1, &v2);
        assert!(
            similarity.abs() < f32::EPSILON,
            "Empty vectors should return 0.0, got {similarity}"
        );
    }

    // Tests that require the fastembed feature
    #[cfg(feature = "fastembed-embeddings")]
    mod fastembed_tests {
        use super::*;

        #[test]
        fn test_embed_success() {
            let embedder = FastEmbedEmbedder::new();
            let result = embedder.embed("Hello, world!");

            assert!(result.is_ok());
            let embedding = result.expect("embed failed");
            assert_eq!(embedding.len(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
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

            let emb1 = result1.expect("embed failed");
            let emb2 = result2.expect("embed failed");

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
            let emb1 = result1.expect("embed failed");
            let emb2 = result2.expect("embed failed");

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
            let emb = result.expect("embed failed");

            // Check that the embedding is normalized (magnitude ~= 1)
            let magnitude: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!(
                (magnitude - 1.0).abs() < 0.01,
                "Embedding magnitude should be ~1.0, got {magnitude}"
            );
        }

        #[test]
        fn test_embed_batch() {
            let embedder = FastEmbedEmbedder::new();
            let texts = vec!["First text", "Second text", "Third text"];

            let result = embedder.embed_batch(&texts);
            assert!(result.is_ok());

            let embeddings = result.expect("embed_batch failed");
            assert_eq!(embeddings.len(), 3);

            for emb in &embeddings {
                assert_eq!(emb.len(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
            }
        }

        #[test]
        fn test_semantic_similarity_related_text() {
            let embedder = FastEmbedEmbedder::new();

            let emb_db = embedder.embed("database storage").expect("embed failed");
            let emb_pg = embedder.embed("PostgreSQL database").expect("embed failed");
            let emb_cat = embedder.embed("cat dog pet animal").expect("embed failed");

            let sim_related = cosine_similarity(&emb_db, &emb_pg);
            let sim_unrelated = cosine_similarity(&emb_db, &emb_cat);

            assert!(
                sim_related > sim_unrelated,
                "Related text ({sim_related}) should be more similar than unrelated ({sim_unrelated})"
            );
            assert!(
                sim_related > 0.5,
                "Related text should have high similarity (>0.5), got {sim_related}"
            );
        }

        #[test]
        fn test_embed_unicode_text() {
            let embedder = FastEmbedEmbedder::new();

            // Unicode text should embed without error
            let result = embedder.embed("Hello 世界 🌍 café");
            assert!(result.is_ok());

            let embedding = result.expect("embed failed");
            assert_eq!(embedding.len(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
        }

        #[test]
        fn test_embed_single_word() {
            let embedder = FastEmbedEmbedder::new();
            let result = embedder.embed("hello");

            assert!(result.is_ok());
            let embedding = result.expect("embed failed");
            assert_eq!(embedding.len(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);

            // Should be normalized
            let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!(magnitude > 0.9 && magnitude < 1.1);
        }

        #[test]
        fn test_embed_all_values_finite() {
            let embedder = FastEmbedEmbedder::new();
            let result = embedder.embed("Test for finite values");

            assert!(result.is_ok());
            let embedding = result.expect("embed failed");

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
            let embedding = result.expect("embed failed");

            // Normalized embeddings should have values roughly in [-1, 1]
            for val in &embedding {
                assert!(
                    *val >= -2.0 && *val <= 2.0,
                    "Value {val} outside expected range"
                );
            }
        }
    }

    // Fallback-specific tests
    #[cfg(not(feature = "fastembed-embeddings"))]
    mod fallback_tests {
        use super::*;

        #[test]
        fn test_embed_success() {
            let embedder = FastEmbedEmbedder::new();
            let result = embedder.embed("Hello, world!");

            assert!(result.is_ok());
            let embedding = result.expect("embed failed");
            assert_eq!(embedding.len(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
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

            let emb1 = result1.expect("embed failed");
            let emb2 = result2.expect("embed failed");

            for (v1, v2) in emb1.iter().zip(emb2.iter()) {
                assert!((v1 - v2).abs() < f32::EPSILON);
            }
        }

        #[test]
        fn test_custom_dimensions() {
            let embedder = FastEmbedEmbedder::with_dimensions(512);
            assert_eq!(embedder.dimensions(), 512);
        }

        #[test]
        fn test_custom_dimensions_embed() {
            let embedder = FastEmbedEmbedder::with_dimensions(128);

            let result = embedder.embed("Test with custom dimensions");
            assert!(result.is_ok());

            let embedding = result.expect("embed failed");
            assert_eq!(embedding.len(), 128);
        }

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
        fn test_embed_normalized() {
            let embedder = FastEmbedEmbedder::new();
            let result = embedder.embed("Test embedding normalization");

            assert!(result.is_ok());
            let emb = result.expect("embed failed");

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

            let embeddings = result.expect("embed_batch failed");
            assert_eq!(embeddings.len(), 3);

            for emb in &embeddings {
                assert_eq!(emb.len(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
            }
        }

        #[test]
        fn test_embed_batch_single_item() {
            let embedder = FastEmbedEmbedder::new();
            let texts = vec!["Single item"];

            let result = embedder.embed_batch(&texts);
            assert!(result.is_ok());

            let embeddings = result.expect("embed_batch failed");
            assert_eq!(embeddings.len(), 1);
        }

        #[test]
        fn test_embed_case_sensitivity() {
            let embedder = FastEmbedEmbedder::new();

            let lower = embedder.embed("hello world").expect("embed failed");
            let upper = embedder.embed("HELLO WORLD").expect("embed failed");
            let mixed = embedder.embed("Hello World").expect("embed failed");

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

            let emb1 = embedder.embed("the quick brown fox").expect("embed failed");
            let emb2 = embedder.embed("brown quick the fox").expect("embed failed");

            // Different word order should produce different embeddings
            let different = emb1
                .iter()
                .zip(emb2.iter())
                .any(|(a, b)| (a - b).abs() > f32::EPSILON);
            assert!(different);
        }

        #[test]
        fn test_embed_all_values_finite() {
            let embedder = FastEmbedEmbedder::new();
            let result = embedder.embed("Test for finite values");

            assert!(result.is_ok());
            let embedding = result.expect("embed failed");

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
            let embedding = result.expect("embed failed");

            // Normalized embeddings should have values roughly in [-1, 1]
            for val in &embedding {
                assert!(
                    *val >= -2.0 && *val <= 2.0,
                    "Value {val} outside expected range"
                );
            }
        }

        #[test]
        fn test_embed_unicode_text() {
            let embedder = FastEmbedEmbedder::new();

            // Unicode text should embed without error
            let result = embedder.embed("Hello 世界 🌍 café");
            assert!(result.is_ok());

            let embedding = result.expect("embed failed");
            assert_eq!(embedding.len(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
        }

        #[test]
        fn test_embed_very_long_text() {
            let embedder = FastEmbedEmbedder::new();

            // Create a long text
            let long_text = "word ".repeat(10000);
            let result = embedder.embed(&long_text);

            assert!(result.is_ok());
            let embedding = result.expect("embed failed");
            assert_eq!(embedding.len(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
        }

        #[test]
        fn test_embed_special_characters() {
            let embedder = FastEmbedEmbedder::new();

            let result = embedder.embed("!@#$%^&*()_+-=[]{}|;':\",./<>?");
            assert!(result.is_ok());
        }

        #[test]
        fn test_embed_numeric_text() {
            let embedder = FastEmbedEmbedder::new();

            let result = embedder.embed("12345 67890");
            assert!(result.is_ok());

            let embedding = result.expect("embed failed");
            assert_eq!(embedding.len(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
        }
    }
}
