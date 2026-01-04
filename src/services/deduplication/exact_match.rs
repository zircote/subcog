//! Exact match deduplication checker.
//!
//! Detects duplicates by comparing SHA256 content hashes stored as tags.
//! Uses `hash:sha256:<prefix>` tag format for efficient lookup.

use crate::Result;
use crate::models::{MemoryId, Namespace, SearchFilter};
use crate::services::recall::RecallService;
use std::sync::Arc;
use std::time::Instant;
use tracing::instrument;

use super::hasher::ContentHasher;

/// Checker for exact content match via SHA256 hash.
///
/// # How it works
///
/// 1. Computes SHA256 hash of normalized content
/// 2. Converts hash to tag format: `hash:sha256:<16-char-prefix>`
/// 3. Searches for memories with matching tag in the specified namespace
/// 4. Returns the first matching memory ID if found
///
/// # Example
///
/// ```rust,ignore
/// use subcog::services::deduplication::ExactMatchChecker;
/// use subcog::services::recall::RecallService;
/// use std::sync::Arc;
///
/// let recall = Arc::new(RecallService::default());
/// let checker = ExactMatchChecker::new(recall);
///
/// let result = checker.check("Use PostgreSQL for storage", Namespace::Decisions)?;
/// if let Some((memory_id, urn)) = result {
///     println!("Exact match found: {}", urn);
/// }
/// ```
pub struct ExactMatchChecker {
    /// Recall service for searching memories.
    recall: Arc<RecallService>,
}

impl ExactMatchChecker {
    /// Creates a new exact match checker.
    ///
    /// # Arguments
    ///
    /// * `recall` - The recall service for searching memories
    #[must_use]
    pub const fn new(recall: Arc<RecallService>) -> Self {
        Self { recall }
    }

    /// Checks if content has an exact match in the given namespace.
    ///
    /// # Arguments
    ///
    /// * `content` - The content to check for duplicates
    /// * `namespace` - The namespace to search within
    /// * `domain` - The domain string for URN construction
    ///
    /// # Returns
    ///
    /// Returns `Some((MemoryId, URN))` if an exact match is found, `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the search operation fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = checker.check("content", Namespace::Decisions, "project")?;
    /// match result {
    ///     Some((id, urn)) => println!("Duplicate: {}", urn),
    ///     None => println!("No duplicate found"),
    /// }
    /// ```
    #[instrument(
        skip(self, content),
        fields(
            operation = "exact_match_check",
            namespace = %namespace.as_str(),
            content_length = content.len()
        )
    )]
    #[allow(clippy::cast_precision_loss)] // Precision loss acceptable for duration metrics
    #[allow(clippy::option_if_let_else)] // if-let is clearer for this pattern
    pub fn check(
        &self,
        content: &str,
        namespace: Namespace,
        domain: &str,
    ) -> Result<Option<(MemoryId, String)>> {
        let start = Instant::now();

        // Compute hash and convert to tag
        let hash = ContentHasher::hash(content);
        let hash_tag = ContentHasher::hash_to_tag(&hash);

        tracing::debug!(hash_tag = %hash_tag, "Searching for exact match");

        // Build filter for namespace and hash tag
        let filter = SearchFilter::new()
            .with_namespace(namespace)
            .with_tag(&hash_tag);

        // Use list_all to find memories with matching tag
        // We only need 1 result since exact match means identical
        let result = self.recall.list_all(&filter, 1)?;

        // Record metrics
        let duration_ms = start.elapsed().as_millis();
        metrics::histogram!(
            "deduplication_check_duration_ms",
            "checker" => "exact_match",
            "found" => if result.memories.is_empty() { "false" } else { "true" }
        )
        .record(duration_ms as f64);

        if let Some(hit) = result.memories.first() {
            let memory_id = hit.memory.id.clone();
            let urn = format!("subcog://{}/{}/{}", domain, namespace.as_str(), memory_id);

            tracing::debug!(
                memory_id = %memory_id,
                urn = %urn,
                duration_ms = %duration_ms,
                "Exact match found"
            );

            Ok(Some((memory_id, urn)))
        } else {
            tracing::debug!(duration_ms = %duration_ms, "No exact match found");
            Ok(None)
        }
    }

    /// Returns the hash tag for the given content.
    ///
    /// Useful for recording captures - the hash tag should be added
    /// to the memory's tags for future exact match detection.
    ///
    /// # Arguments
    ///
    /// * `content` - The content to hash
    ///
    /// # Returns
    ///
    /// The hash tag in format `hash:sha256:<16-char-prefix>`
    #[must_use]
    pub fn content_to_tag(content: &str) -> String {
        let hash = ContentHasher::hash(content);
        ContentHasher::hash_to_tag(&hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, Memory, MemoryStatus};
    use crate::storage::index::SqliteBackend;
    use crate::storage::traits::IndexBackend;

    fn create_test_memory(
        id: &str,
        content: &str,
        namespace: Namespace,
        tags: Vec<String>,
    ) -> Memory {
        Memory {
            id: MemoryId::new(id),
            content: content.to_string(),
            namespace,
            domain: Domain::new(),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: 1_234_567_890,
            updated_at: 1_234_567_890,
            tombstoned_at: None,
            embedding: None,
            tags,
            source: None,
        }
    }

    #[test]
    fn test_content_to_tag() {
        let content = "Use PostgreSQL for storage";
        let tag = ExactMatchChecker::content_to_tag(content);

        assert!(tag.starts_with("hash:sha256:"));
        assert_eq!(tag.len(), "hash:sha256:".len() + 16);
    }

    #[test]
    fn test_content_to_tag_normalization() {
        // Same content with different whitespace should produce same tag
        let tag1 = ExactMatchChecker::content_to_tag("Use PostgreSQL for storage");
        let tag2 = ExactMatchChecker::content_to_tag("  Use  PostgreSQL   for   storage  ");

        assert_eq!(tag1, tag2);
    }

    #[test]
    fn test_content_to_tag_case_insensitive() {
        // Same content with different case should produce same tag
        let tag1 = ExactMatchChecker::content_to_tag("Use PostgreSQL");
        let tag2 = ExactMatchChecker::content_to_tag("use postgresql");

        assert_eq!(tag1, tag2);
    }

    #[test]
    fn test_check_no_match() {
        // Create in-memory backend
        let index = SqliteBackend::in_memory().unwrap();
        let recall = Arc::new(RecallService::with_index(index));
        let checker = ExactMatchChecker::new(recall);

        // Check for content that doesn't exist
        let result = checker
            .check("Non-existent content", Namespace::Decisions, "project")
            .unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn test_check_with_match() {
        // Create in-memory backend
        let index = SqliteBackend::in_memory().unwrap();

        // Create a memory with the hash tag
        let content = "Use PostgreSQL for storage";
        let hash_tag = ExactMatchChecker::content_to_tag(content);
        let memory = create_test_memory(
            "test-memory-123",
            content,
            Namespace::Decisions,
            vec![hash_tag],
        );

        index.index(&memory).unwrap();

        let recall = Arc::new(RecallService::with_index(index));
        let checker = ExactMatchChecker::new(recall);

        // Check for the same content
        let result = checker
            .check(content, Namespace::Decisions, "project")
            .unwrap();

        assert!(result.is_some());
        let (id, urn) = result.unwrap();
        assert_eq!(id.as_str(), "test-memory-123");
        assert_eq!(urn, "subcog://project/decisions/test-memory-123");
    }

    #[test]
    fn test_check_different_namespace() {
        // Create in-memory backend
        let index = SqliteBackend::in_memory().unwrap();

        // Create a memory in Decisions namespace
        let content = "Use PostgreSQL for storage";
        let hash_tag = ExactMatchChecker::content_to_tag(content);
        let memory = create_test_memory(
            "test-memory-123",
            content,
            Namespace::Decisions,
            vec![hash_tag],
        );

        index.index(&memory).unwrap();

        let recall = Arc::new(RecallService::with_index(index));
        let checker = ExactMatchChecker::new(recall);

        // Check in different namespace should not find match
        let result = checker
            .check(content, Namespace::Patterns, "project")
            .unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn test_check_normalized_content_matches() {
        // Create in-memory backend
        let index = SqliteBackend::in_memory().unwrap();

        // Create a memory with normalized content hash
        let original_content = "Use PostgreSQL";
        let hash_tag = ExactMatchChecker::content_to_tag(original_content);
        let memory = create_test_memory(
            "test-memory-456",
            original_content,
            Namespace::Decisions,
            vec![hash_tag],
        );

        index.index(&memory).unwrap();

        let recall = Arc::new(RecallService::with_index(index));
        let checker = ExactMatchChecker::new(recall);

        // Check with whitespace and case variations should still match
        let result = checker
            .check("  USE  postgresql  ", Namespace::Decisions, "project")
            .unwrap();

        assert!(result.is_some());
        let (id, _) = result.unwrap();
        assert_eq!(id.as_str(), "test-memory-456");
    }
}
