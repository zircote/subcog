//! Content hashing utility for deduplication.
//!
//! This module provides SHA256-based content hashing for exact match detection.
//! Content is normalized before hashing to ensure consistent matches despite
//! minor formatting differences.

use sha2::{Digest, Sha256};

/// Content hasher for deduplication.
///
/// Normalizes content and produces SHA256 hashes for exact match detection.
///
/// # Normalization
///
/// Before hashing, content is normalized:
/// - Trimmed of leading/trailing whitespace
/// - Converted to lowercase
/// - Multiple whitespace characters collapsed to single spaces
///
/// # Example
///
/// ```rust
/// use subcog::services::deduplication::ContentHasher;
///
/// let hash = ContentHasher::hash("Use PostgreSQL for primary storage");
/// assert_eq!(hash.len(), 64); // SHA256 produces 64 hex chars
///
/// // Normalized content produces the same hash
/// let hash2 = ContentHasher::hash("  Use  postgresql  for  primary  storage  ");
/// assert_eq!(hash, hash2);
/// ```
pub struct ContentHasher;

impl ContentHasher {
    /// Computes the SHA256 hash of normalized content.
    ///
    /// # Arguments
    ///
    /// * `content` - The content to hash
    ///
    /// # Returns
    ///
    /// The lowercase hex-encoded SHA256 hash (64 characters).
    ///
    /// # Example
    ///
    /// ```rust
    /// use subcog::services::deduplication::ContentHasher;
    ///
    /// let hash = ContentHasher::hash("Hello, world!");
    /// assert_eq!(hash.len(), 64);
    /// ```
    #[must_use]
    pub fn hash(content: &str) -> String {
        let normalized = Self::normalize(content);
        let mut hasher = Sha256::new();
        hasher.update(normalized.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Converts a hash to a tag format.
    ///
    /// The tag format is `hash:sha256:<16-char-prefix>`.
    ///
    /// # Arguments
    ///
    /// * `hash` - The full SHA256 hash
    ///
    /// # Returns
    ///
    /// The hash tag string.
    ///
    /// # Panics
    ///
    /// Does not panic. If the hash is shorter than 16 chars, uses the full hash.
    ///
    /// # Example
    ///
    /// ```rust
    /// use subcog::services::deduplication::ContentHasher;
    ///
    /// let hash = ContentHasher::hash("test content");
    /// let tag = ContentHasher::hash_to_tag(&hash);
    /// assert!(tag.starts_with("hash:sha256:"));
    /// assert_eq!(tag.len(), "hash:sha256:".len() + 16);
    /// ```
    #[must_use]
    pub fn hash_to_tag(hash: &str) -> String {
        let prefix_len = hash.len().min(16);
        format!("hash:sha256:{}", &hash[..prefix_len])
    }

    /// Computes a hash and returns it in tag format.
    ///
    /// Convenience method that combines `hash()` and `hash_to_tag()`.
    ///
    /// # Arguments
    ///
    /// * `content` - The content to hash
    ///
    /// # Returns
    ///
    /// The hash tag string.
    ///
    /// # Example
    ///
    /// ```rust
    /// use subcog::services::deduplication::ContentHasher;
    ///
    /// let tag = ContentHasher::content_to_tag("Use PostgreSQL");
    /// assert!(tag.starts_with("hash:sha256:"));
    /// ```
    #[must_use]
    pub fn content_to_tag(content: &str) -> String {
        let hash = Self::hash(content);
        Self::hash_to_tag(&hash)
    }

    /// Normalizes content for consistent hashing.
    ///
    /// Normalization steps:
    /// 1. Trim leading/trailing whitespace
    /// 2. Convert to lowercase
    /// 3. Collapse multiple whitespace to single space
    ///
    /// # Arguments
    ///
    /// * `content` - The content to normalize
    ///
    /// # Returns
    ///
    /// The normalized content string.
    ///
    /// # Example
    ///
    /// ```rust
    /// use subcog::services::deduplication::ContentHasher;
    ///
    /// let normalized = ContentHasher::normalize("  Hello   WORLD  ");
    /// assert_eq!(normalized, "hello world");
    /// ```
    #[must_use]
    pub fn normalize(content: &str) -> String {
        content
            .trim()
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_produces_64_char_hex() {
        let hash = ContentHasher::hash("test content");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_same_content_same_hash() {
        let hash1 = ContentHasher::hash("Use PostgreSQL for storage");
        let hash2 = ContentHasher::hash("Use PostgreSQL for storage");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_different_content_different_hash() {
        let hash1 = ContentHasher::hash("Use PostgreSQL");
        let hash2 = ContentHasher::hash("Use MySQL");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_normalization_case_insensitive() {
        let hash1 = ContentHasher::hash("Use PostgreSQL");
        let hash2 = ContentHasher::hash("use postgresql");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_normalization_whitespace_collapse() {
        let hash1 = ContentHasher::hash("Use PostgreSQL");
        let hash2 = ContentHasher::hash("  Use   PostgreSQL  ");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_normalization_mixed() {
        let hash1 = ContentHasher::hash("use postgresql");
        let hash2 = ContentHasher::hash("  USE    POSTGRESQL  ");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_to_tag_format() {
        let hash = ContentHasher::hash("test");
        let tag = ContentHasher::hash_to_tag(&hash);

        assert!(tag.starts_with("hash:sha256:"));
        // Total length should be "hash:sha256:" (12) + 16 chars = 28
        assert_eq!(tag.len(), 28);
    }

    #[test]
    fn test_content_to_tag_convenience() {
        let tag = ContentHasher::content_to_tag("Use PostgreSQL for storage");
        assert!(tag.starts_with("hash:sha256:"));
        assert_eq!(tag.len(), 28);
    }

    #[test]
    fn test_normalize_function() {
        assert_eq!(ContentHasher::normalize("  Hello  "), "hello");
        assert_eq!(ContentHasher::normalize("Hello   World"), "hello world");
        assert_eq!(ContentHasher::normalize("UPPER"), "upper");
        assert_eq!(ContentHasher::normalize("  a  b  c  "), "a b c");
    }

    #[test]
    fn test_empty_content() {
        let hash = ContentHasher::hash("");
        // Empty string should still produce a valid hash
        assert_eq!(hash.len(), 64);

        let tag = ContentHasher::hash_to_tag(&hash);
        assert!(tag.starts_with("hash:sha256:"));
    }

    #[test]
    fn test_unicode_content() {
        let hash = ContentHasher::hash("Use PostgreSQL for 数据库");
        assert_eq!(hash.len(), 64);

        // Unicode is preserved but lowercased where applicable
        let normalized = ContentHasher::normalize("Use POSTGRESQL for 数据库");
        assert!(normalized.contains("数据库"));
    }

    #[test]
    fn test_hash_to_tag_short_hash() {
        // Edge case: if somehow given a short hash
        let tag = ContentHasher::hash_to_tag("abc");
        assert_eq!(tag, "hash:sha256:abc");
    }

    #[test]
    fn test_newline_handling() {
        // Newlines should be treated as whitespace
        let hash1 = ContentHasher::hash("line one\nline two");
        let hash2 = ContentHasher::hash("line one line two");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_tab_handling() {
        // Tabs should be treated as whitespace
        let hash1 = ContentHasher::hash("col1\tcol2");
        let hash2 = ContentHasher::hash("col1 col2");
        assert_eq!(hash1, hash2);
    }

    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Hash output is always 64 hex characters.
            #[test]
            fn prop_hash_length(content in any::<String>()) {
                let hash = ContentHasher::hash(&content);
                prop_assert_eq!(hash.len(), 64, "Hash length should be 64, got {}", hash.len());
            }

            /// Same input always produces same hash (deterministic).
            #[test]
            fn prop_hash_deterministic(content in any::<String>()) {
                let hash1 = ContentHasher::hash(&content);
                let hash2 = ContentHasher::hash(&content);
                prop_assert_eq!(hash1, hash2, "Hash should be deterministic");
            }

            /// Normalization is idempotent: normalize(normalize(x)) == normalize(x).
            #[test]
            fn prop_normalize_idempotent(content in any::<String>()) {
                let once = ContentHasher::normalize(&content);
                let twice = ContentHasher::normalize(&once);
                prop_assert_eq!(once, twice, "Normalization should be idempotent");
            }

            /// Normalized content produces same hash regardless of whitespace/case.
            #[test]
            fn prop_normalized_hash_invariant(content in "[a-z ]{1,50}") {
                let with_spaces = format!("  {content}  ");
                let uppercased = content.to_uppercase();

                let hash_original = ContentHasher::hash(&content);
                let hash_spaces = ContentHasher::hash(&with_spaces);
                let hash_upper = ContentHasher::hash(&uppercased);

                prop_assert_eq!(
                    &hash_original, &hash_spaces,
                    "Extra whitespace should not affect hash"
                );
                prop_assert_eq!(
                    &hash_original, &hash_upper,
                    "Case should not affect hash"
                );
            }

            /// Tag format is always correct.
            #[test]
            fn prop_tag_format(content in any::<String>()) {
                let tag = ContentHasher::content_to_tag(&content);
                prop_assert!(
                    tag.starts_with("hash:sha256:"),
                    "Tag should start with 'hash:sha256:', got {tag}"
                );
                prop_assert_eq!(
                    tag.len(), 28,
                    "Tag length should be 28 (12 prefix + 16 hash), got {}", tag.len()
                );
            }
        }
    }
}
