//! Semantic similarity deduplication checker.
//!
//! Detects duplicates by comparing embedding vectors using cosine similarity.
//! Uses configurable per-namespace similarity thresholds.

use crate::Result;
use crate::embedding::Embedder;
use crate::models::{MemoryId, Namespace};
use crate::storage::traits::{VectorBackend, VectorFilter};
use std::sync::Arc;
use std::time::Instant;
use tracing::instrument;

use super::config::DeduplicationConfig;

/// Checker for semantic similarity using embeddings.
///
/// # How it works
///
/// 1. Generates embedding for the new content using the configured embedder
/// 2. Searches the vector index for similar embeddings
/// 3. Compares similarity scores against namespace-specific thresholds
/// 4. Returns the first match that exceeds the threshold
///
/// # Thresholds
///
/// Per-namespace thresholds are configured in `DeduplicationConfig`:
/// - Decisions: 0.92 (high - avoid losing unique decisions)
/// - Patterns: 0.90 (standard threshold)
/// - Learnings: 0.88 (lower - learnings are often phrased differently)
/// - Default: 0.90
///
/// # Example
///
/// ```rust,ignore
/// use subcog::services::deduplication::{SemanticSimilarityChecker, DeduplicationConfig};
/// use subcog::embedding::FastEmbedEmbedder;
/// use subcog::storage::vector::UsearchBackend;
/// use std::sync::Arc;
///
/// let embedder = Arc::new(FastEmbedEmbedder::new());
/// let vector = Arc::new(UsearchBackend::in_memory(384));
/// let config = DeduplicationConfig::default();
/// let checker = SemanticSimilarityChecker::new(embedder, vector, config);
///
/// let result = checker.check("Use PostgreSQL for storage", Namespace::Decisions, "global")?;
/// if let Some((memory_id, urn, score)) = result {
///     println!("Semantic match found: {} (score: {:.2})", urn, score);
/// }
/// ```
pub struct SemanticSimilarityChecker<E: Embedder + Send + Sync, V: VectorBackend + Send + Sync> {
    /// Embedder for generating vectors.
    embedder: Arc<E>,
    /// Vector backend for similarity search.
    vector: Arc<V>,
    /// Configuration with thresholds.
    config: DeduplicationConfig,
}

impl<E: Embedder + Send + Sync, V: VectorBackend + Send + Sync> SemanticSimilarityChecker<E, V> {
    /// Creates a new semantic similarity checker.
    ///
    /// # Arguments
    ///
    /// * `embedder` - The embedding generator
    /// * `vector` - The vector backend for similarity search
    /// * `config` - Configuration with per-namespace thresholds
    #[must_use]
    pub const fn new(embedder: Arc<E>, vector: Arc<V>, config: DeduplicationConfig) -> Self {
        Self {
            embedder,
            vector,
            config,
        }
    }

    /// Checks if content has a semantic match in the given namespace.
    ///
    /// Skips check if content is shorter than `min_semantic_length` configuration.
    ///
    /// # Arguments
    ///
    /// * `content` - The content to check for duplicates
    /// * `namespace` - The namespace to search within (determines threshold)
    /// * `domain` - The domain string for URN construction
    ///
    /// # Returns
    ///
    /// Returns `Some((MemoryId, URN, score))` if a semantic match is found above threshold,
    /// `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if embedding generation or vector search fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = checker.check("content", Namespace::Decisions, "global")?;
    /// match result {
    ///     Some((id, urn, score)) => println!("Similar: {} ({:.2})", urn, score),
    ///     None => println!("No similar content found"),
    /// }
    /// ```
    #[instrument(
        skip(self, content),
        fields(
            operation = "semantic_similarity_check",
            namespace = %namespace.as_str(),
            content_length = content.len()
        )
    )]
    pub fn check(
        &self,
        content: &str,
        namespace: Namespace,
        domain: &str,
    ) -> Result<Option<(MemoryId, String, f32)>> {
        let start = Instant::now();

        // Skip if content is too short for meaningful semantic comparison
        if content.len() < self.config.min_semantic_length {
            tracing::debug!(
                content_length = content.len(),
                min_length = self.config.min_semantic_length,
                "Content too short for semantic check"
            );
            return Ok(None);
        }

        // Get threshold for this namespace
        let threshold = self.config.get_threshold(namespace);

        tracing::debug!(
            threshold = threshold,
            namespace = %namespace.as_str(),
            "Checking semantic similarity"
        );

        // Generate embedding for the content
        let embedding = self.embedder.embed(content)?;

        // Build filter for namespace
        let filter = VectorFilter::new().with_namespace(namespace);

        // Search for similar vectors
        // Request only 3 results - we only need to find one above threshold (PERF-H2)
        // Reducing from 10 to 3 improves performance while maintaining effectiveness
        let results = self.vector.search(&embedding, &filter, 3)?;

        // Record metrics
        let duration_ms = start.elapsed().as_millis();

        // Find first result above threshold
        for (memory_id, score) in results {
            if score >= threshold {
                let urn = format!("subcog://{}/{}/{}", domain, namespace.as_str(), memory_id);

                tracing::debug!(
                    memory_id = %memory_id,
                    urn = %urn,
                    score = score,
                    threshold = threshold,
                    duration_ms = %duration_ms,
                    "Semantic match found"
                );

                metrics::histogram!(
                    "deduplication_check_duration_ms",
                    "checker" => "semantic_similarity",
                    "found" => "true"
                )
                .record(duration_ms as f64);

                return Ok(Some((memory_id, urn, score)));
            }
        }

        tracing::debug!(
            threshold = threshold,
            duration_ms = %duration_ms,
            "No semantic match found above threshold"
        );

        metrics::histogram!(
            "deduplication_check_duration_ms",
            "checker" => "semantic_similarity",
            "found" => "false"
        )
        .record(duration_ms as f64);

        Ok(None)
    }

    /// Generates an embedding for the given content.
    ///
    /// Useful for recording captures - the embedding should be stored
    /// in the vector index for future semantic matching.
    ///
    /// # Arguments
    ///
    /// * `content` - The content to embed
    ///
    /// # Returns
    ///
    /// The embedding vector.
    ///
    /// # Errors
    ///
    /// Returns an error if embedding generation fails.
    #[cfg(test)]
    pub fn embed(&self, content: &str) -> Result<Vec<f32>> {
        self.embedder.embed(content)
    }

    /// Returns the configured threshold for a namespace.
    ///
    /// # Arguments
    ///
    /// * `namespace` - The namespace to get threshold for
    #[cfg(test)]
    #[must_use]
    pub fn get_threshold(&self, namespace: Namespace) -> f32 {
        self.config.get_threshold(namespace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::FastEmbedEmbedder;
    use crate::storage::vector::UsearchBackend;
    use std::sync::RwLock;

    /// Computes cosine similarity between two vectors.
    ///
    /// Used only for testing similarity calculations.
    ///
    /// # Arguments
    ///
    /// * `a` - First vector
    /// * `b` - Second vector
    ///
    /// # Returns
    ///
    /// Cosine similarity normalized to [0, 1] range.
    /// Returns 0.0 if vectors have different dimensions or zero magnitude.
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

    /// Creates a usearch backend for tests.
    /// Handles the Result return type when usearch-hnsw feature is enabled.
    #[cfg(not(feature = "usearch-hnsw"))]
    fn create_usearch_backend(dimensions: usize) -> UsearchBackend {
        UsearchBackend::in_memory(dimensions)
    }

    /// Creates a usearch backend for tests.
    /// Handles the Result return type when usearch-hnsw feature is enabled.
    #[cfg(feature = "usearch-hnsw")]
    fn create_usearch_backend(dimensions: usize) -> UsearchBackend {
        UsearchBackend::in_memory(dimensions).expect("Failed to create usearch backend")
    }

    /// Helper to create a test checker with in-memory backend.
    fn create_test_checker() -> SemanticSimilarityChecker<FastEmbedEmbedder, RwLockWrapper> {
        let embedder = Arc::new(FastEmbedEmbedder::new());
        let vector = Arc::new(RwLockWrapper::new(create_usearch_backend(
            FastEmbedEmbedder::DEFAULT_DIMENSIONS,
        )));
        let config = DeduplicationConfig::default();
        SemanticSimilarityChecker::new(embedder, vector, config)
    }

    /// Wrapper to make `UsearchBackend` work with Arc (needs interior mutability for tests).
    struct RwLockWrapper {
        inner: RwLock<UsearchBackend>,
    }

    impl RwLockWrapper {
        fn new(backend: UsearchBackend) -> Self {
            Self {
                inner: RwLock::new(backend),
            }
        }
    }

    impl VectorBackend for RwLockWrapper {
        fn dimensions(&self) -> usize {
            self.inner.read().unwrap().dimensions()
        }

        fn upsert(&self, id: &MemoryId, embedding: &[f32]) -> Result<()> {
            self.inner.write().unwrap().upsert(id, embedding)
        }

        fn remove(&self, id: &MemoryId) -> Result<bool> {
            self.inner.write().unwrap().remove(id)
        }

        fn search(
            &self,
            query_embedding: &[f32],
            filter: &VectorFilter,
            limit: usize,
        ) -> Result<Vec<(MemoryId, f32)>> {
            self.inner
                .read()
                .unwrap()
                .search(query_embedding, filter, limit)
        }

        fn count(&self) -> Result<usize> {
            self.inner.read().unwrap().count()
        }

        fn clear(&self) -> Result<()> {
            self.inner.write().unwrap().clear()
        }
    }

    #[test]
    fn test_cosine_similarity_same_vector() {
        let v = vec![1.0, 0.0, 0.0];
        let similarity = cosine_similarity(&v, &v);
        assert!((similarity - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        let similarity = cosine_similarity(&v1, &v2);
        // Normalized to [0, 1], so orthogonal = 0.5
        assert!((similarity - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![-1.0, 0.0, 0.0];
        let similarity = cosine_similarity(&v1, &v2);
        // Opposite vectors = 0 in [0, 1] range
        assert!(similarity < 0.001);
    }

    #[test]
    fn test_cosine_similarity_different_dimensions() {
        let v1 = vec![1.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0];
        let similarity = cosine_similarity(&v1, &v2);
        assert!(similarity < f32::EPSILON);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let v1 = vec![0.0, 0.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0];
        let similarity = cosine_similarity(&v1, &v2);
        assert!(similarity < f32::EPSILON);
    }

    #[test]
    fn test_check_short_content_skipped() {
        let checker = create_test_checker();

        // Content shorter than min_semantic_length (50) should be skipped
        let result = checker
            .check("short", Namespace::Decisions, "global")
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_check_no_match() {
        let checker = create_test_checker();

        // Content long enough but no vectors in the index
        let content = "This is a sufficiently long piece of content that should trigger semantic similarity checking in the deduplication system.";
        let result = checker
            .check(content, Namespace::Decisions, "global")
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_check_with_match() {
        let embedder = Arc::new(FastEmbedEmbedder::new());
        let vector = Arc::new(RwLockWrapper::new(create_usearch_backend(
            FastEmbedEmbedder::DEFAULT_DIMENSIONS,
        )));
        let config = DeduplicationConfig::default();

        // Add a vector to the index
        let existing_content =
            "Use PostgreSQL as the primary database for storing user data and application state.";
        let existing_embedding = embedder.embed(existing_content).unwrap();
        vector
            .upsert(&MemoryId::new("existing-memory-123"), &existing_embedding)
            .unwrap();

        let checker = SemanticSimilarityChecker::new(embedder, vector, config);

        // Check with identical content (should match with very high score)
        let result = checker
            .check(existing_content, Namespace::Decisions, "global")
            .unwrap();

        assert!(result.is_some());
        let (id, urn, score) = result.unwrap();
        assert_eq!(id.as_str(), "existing-memory-123");
        assert_eq!(urn, "subcog://global/decisions/existing-memory-123");
        assert!(score > 0.99); // Near-identical content
    }

    #[test]
    fn test_check_below_threshold() {
        let embedder = Arc::new(FastEmbedEmbedder::new());
        let vector = Arc::new(RwLockWrapper::new(create_usearch_backend(
            FastEmbedEmbedder::DEFAULT_DIMENSIONS,
        )));

        // Use a very high threshold
        let config = DeduplicationConfig::default().with_default_threshold(0.99);

        // Add a vector
        let existing_content = "Use PostgreSQL as the primary database for storing user data.";
        let existing_embedding = embedder.embed(existing_content).unwrap();
        vector
            .upsert(&MemoryId::new("existing-memory"), &existing_embedding)
            .unwrap();

        let checker = SemanticSimilarityChecker::new(embedder, vector, config);

        // Check with different content - should be below threshold
        let new_content =
            "Use MongoDB for document storage in the application for maximum flexibility.";
        let result = checker
            .check(new_content, Namespace::Decisions, "global")
            .unwrap();

        // May or may not match depending on pseudo-embedding behavior
        // With a 0.99 threshold, different content should not match
        if let Some((_, _, score)) = result {
            assert!(score >= 0.99);
        }
    }

    #[test]
    fn test_get_threshold() {
        let checker = create_test_checker();

        // Check namespace-specific thresholds
        assert!((checker.get_threshold(Namespace::Decisions) - 0.92).abs() < f32::EPSILON);
        assert!((checker.get_threshold(Namespace::Patterns) - 0.90).abs() < f32::EPSILON);
        assert!((checker.get_threshold(Namespace::Learnings) - 0.88).abs() < f32::EPSILON);

        // Unconfigured namespaces use default
        assert!((checker.get_threshold(Namespace::Blockers) - 0.90).abs() < f32::EPSILON);
    }

    #[test]
    fn test_embed() {
        let checker = create_test_checker();

        let content = "Test content for embedding generation";
        let result = checker.embed(content);

        assert!(result.is_ok());
        let embedding = result.unwrap();
        assert_eq!(embedding.len(), FastEmbedEmbedder::DEFAULT_DIMENSIONS);
    }

    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        /// Normalize a vector to unit length, or return a default unit vector if too small.
        fn normalize_vector(v: Vec<f32>) -> Vec<f32> {
            let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm < f32::EPSILON {
                default_unit_vector(v.len())
            } else {
                v.into_iter().map(|x| x / norm).collect()
            }
        }

        /// Create a default unit vector of given dimension.
        fn default_unit_vector(dim: usize) -> Vec<f32> {
            let mut result = vec![0.0; dim];
            if !result.is_empty() {
                result[0] = 1.0;
            }
            result
        }

        /// Strategy for generating valid normalized vectors.
        fn normalized_vec(dim: usize) -> impl Strategy<Value = Vec<f32>> {
            prop::collection::vec(-1.0f32..1.0f32, dim).prop_map(normalize_vector)
        }

        proptest! {
            /// Cosine similarity of a vector with itself is always 1.0.
            #[test]
            fn prop_similarity_identity(v in normalized_vec(10)) {
                let sim = cosine_similarity(&v, &v);
                prop_assert!((sim - 1.0).abs() < 0.001, "Self-similarity should be 1.0, got {sim}");
            }

            /// Cosine similarity is symmetric: sim(a, b) == sim(b, a).
            #[test]
            fn prop_similarity_symmetric(
                v1 in normalized_vec(10),
                v2 in normalized_vec(10)
            ) {
                let sim_ab = cosine_similarity(&v1, &v2);
                let sim_ba = cosine_similarity(&v2, &v1);
                prop_assert!(
                    (sim_ab - sim_ba).abs() < 0.001,
                    "Symmetry violated: sim(a,b)={sim_ab}, sim(b,a)={sim_ba}"
                );
            }

            /// Cosine similarity is always in the range [0.0, 1.0].
            #[test]
            fn prop_similarity_bounded(
                v1 in normalized_vec(10),
                v2 in normalized_vec(10)
            ) {
                let sim = cosine_similarity(&v1, &v2);
                prop_assert!(
                    (0.0..=1.0).contains(&sim),
                    "Similarity {sim} out of bounds [0, 1]"
                );
            }

            /// Empty vectors should return 0.0.
            #[test]
            fn prop_empty_vectors_zero(_dummy: u8) {
                let sim = cosine_similarity(&[], &[]);
                prop_assert!(sim < f32::EPSILON, "Empty vectors should return 0.0, got {sim}");
            }

            /// Different dimension vectors should return 0.0.
            #[test]
            fn prop_different_dimensions_zero(
                v1 in normalized_vec(5),
                v2 in normalized_vec(10)
            ) {
                let sim = cosine_similarity(&v1, &v2);
                prop_assert!(sim < f32::EPSILON, "Different dimension vectors should return 0.0, got {sim}");
            }
        }
    }
}
