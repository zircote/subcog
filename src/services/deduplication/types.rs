//! Deduplication result types.
//!
//! This module defines the result types returned by deduplication checks.

use crate::models::MemoryId;
use serde::{Deserialize, Serialize};

/// Result of a deduplication check.
///
/// Contains information about whether content was found to be a duplicate,
/// the reason for duplication, and any matched memory information.
///
/// # Example
///
/// ```rust
/// use subcog::services::deduplication::{DuplicateCheckResult, DuplicateReason};
/// use subcog::models::MemoryId;
///
/// let result = DuplicateCheckResult {
///     is_duplicate: true,
///     reason: Some(DuplicateReason::ExactMatch),
///     similarity_score: None,
///     matched_memory_id: Some(MemoryId::new("abc123")),
///     matched_urn: Some("subcog://global/decisions/abc123".to_string()),
///     check_duration_ms: 5,
/// };
///
/// assert!(result.is_duplicate);
/// assert_eq!(result.reason, Some(DuplicateReason::ExactMatch));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateCheckResult {
    /// Whether the content is a duplicate.
    pub is_duplicate: bool,

    /// The reason content was identified as a duplicate.
    pub reason: Option<DuplicateReason>,

    /// Similarity score for semantic matches (0.0 to 1.0).
    pub similarity_score: Option<f32>,

    /// The memory ID of the matched duplicate.
    pub matched_memory_id: Option<MemoryId>,

    /// Full URN of matched memory: `subcog://{domain}/{namespace}/{id}`.
    ///
    /// MUST be populated when `is_duplicate == true`.
    /// All external outputs (logs, metrics labels, hook responses) MUST reference
    /// memories by URN, not bare ID.
    pub matched_urn: Option<String>,

    /// Duration of the deduplication check in milliseconds.
    pub check_duration_ms: u64,
}

impl DuplicateCheckResult {
    /// Creates a result indicating no duplicate was found.
    ///
    /// # Arguments
    ///
    /// * `duration_ms` - Time taken for the check in milliseconds
    ///
    /// # Example
    ///
    /// ```rust
    /// use subcog::services::deduplication::DuplicateCheckResult;
    ///
    /// let result = DuplicateCheckResult::not_duplicate(10);
    /// assert!(!result.is_duplicate);
    /// assert!(result.reason.is_none());
    /// ```
    #[must_use]
    pub const fn not_duplicate(duration_ms: u64) -> Self {
        Self {
            is_duplicate: false,
            reason: None,
            similarity_score: None,
            matched_memory_id: None,
            matched_urn: None,
            check_duration_ms: duration_ms,
        }
    }

    /// Creates a result indicating an exact match was found.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - The ID of the matched memory
    /// * `urn` - The full URN of the matched memory
    /// * `duration_ms` - Time taken for the check in milliseconds
    ///
    /// # Example
    ///
    /// ```rust
    /// use subcog::services::deduplication::DuplicateCheckResult;
    /// use subcog::models::MemoryId;
    ///
    /// let result = DuplicateCheckResult::exact_match(
    ///     MemoryId::new("abc123"),
    ///     "subcog://global/decisions/abc123".to_string(),
    ///     5,
    /// );
    /// assert!(result.is_duplicate);
    /// ```
    #[must_use]
    pub const fn exact_match(memory_id: MemoryId, urn: String, duration_ms: u64) -> Self {
        Self {
            is_duplicate: true,
            reason: Some(DuplicateReason::ExactMatch),
            similarity_score: None,
            matched_memory_id: Some(memory_id),
            matched_urn: Some(urn),
            check_duration_ms: duration_ms,
        }
    }

    /// Creates a result indicating a semantic similarity match was found.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - The ID of the matched memory
    /// * `urn` - The full URN of the matched memory
    /// * `score` - The similarity score (0.0 to 1.0)
    /// * `duration_ms` - Time taken for the check in milliseconds
    ///
    /// # Example
    ///
    /// ```rust
    /// use subcog::services::deduplication::DuplicateCheckResult;
    /// use subcog::models::MemoryId;
    ///
    /// let result = DuplicateCheckResult::semantic_match(
    ///     MemoryId::new("abc123"),
    ///     "subcog://global/decisions/abc123".to_string(),
    ///     0.94,
    ///     20,
    /// );
    /// assert!(result.is_duplicate);
    /// assert_eq!(result.similarity_score, Some(0.94));
    /// ```
    #[must_use]
    pub const fn semantic_match(
        memory_id: MemoryId,
        urn: String,
        score: f32,
        duration_ms: u64,
    ) -> Self {
        Self {
            is_duplicate: true,
            reason: Some(DuplicateReason::SemanticSimilar),
            similarity_score: Some(score),
            matched_memory_id: Some(memory_id),
            matched_urn: Some(urn),
            check_duration_ms: duration_ms,
        }
    }

    /// Creates a result indicating the content was recently captured.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - The ID of the matched memory
    /// * `urn` - The full URN of the matched memory
    /// * `duration_ms` - Time taken for the check in milliseconds
    ///
    /// # Example
    ///
    /// ```rust
    /// use subcog::services::deduplication::DuplicateCheckResult;
    /// use subcog::models::MemoryId;
    ///
    /// let result = DuplicateCheckResult::recent_capture(
    ///     MemoryId::new("abc123"),
    ///     "subcog://global/decisions/abc123".to_string(),
    ///     1,
    /// );
    /// assert!(result.is_duplicate);
    /// ```
    #[must_use]
    pub const fn recent_capture(memory_id: MemoryId, urn: String, duration_ms: u64) -> Self {
        Self {
            is_duplicate: true,
            reason: Some(DuplicateReason::RecentCapture),
            similarity_score: None,
            matched_memory_id: Some(memory_id),
            matched_urn: Some(urn),
            check_duration_ms: duration_ms,
        }
    }
}

impl Default for DuplicateCheckResult {
    fn default() -> Self {
        Self::not_duplicate(0)
    }
}

/// The reason content was identified as a duplicate.
///
/// # Variants
///
/// - `ExactMatch`: Content hash matches an existing memory exactly
/// - `SemanticSimilar`: Embedding similarity exceeds the configured threshold
/// - `RecentCapture`: Content was captured within the recent time window
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateReason {
    /// Content hash matches exactly (SHA256).
    ExactMatch,

    /// Semantic similarity exceeds threshold.
    SemanticSimilar,

    /// Content was captured within the recent time window.
    RecentCapture,
}

impl std::fmt::Display for DuplicateReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExactMatch => write!(f, "exact_match"),
            Self::SemanticSimilar => write!(f, "semantic_similar"),
            Self::RecentCapture => write!(f, "recent_capture"),
        }
    }
}

/// Trait for deduplication checking.
///
/// Allows for different implementations (e.g., mock for testing).
pub trait Deduplicator: Send + Sync {
    /// Checks if content is a duplicate.
    ///
    /// # Arguments
    ///
    /// * `content` - The content to check
    /// * `namespace` - The namespace to check within
    ///
    /// # Returns
    ///
    /// A result indicating whether the content is a duplicate and why.
    ///
    /// # Errors
    ///
    /// Returns an error if the check fails.
    fn check_duplicate(
        &self,
        content: &str,
        namespace: crate::models::Namespace,
    ) -> crate::Result<DuplicateCheckResult>;

    /// Records a successful capture for recent-capture tracking.
    ///
    /// # Arguments
    ///
    /// * `content_hash` - The SHA256 hash of the content
    /// * `memory_id` - The ID of the captured memory
    fn record_capture(&self, content_hash: &str, memory_id: &MemoryId);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_duplicate_result() {
        let result = DuplicateCheckResult::not_duplicate(10);
        assert!(!result.is_duplicate);
        assert!(result.reason.is_none());
        assert!(result.matched_memory_id.is_none());
        assert!(result.matched_urn.is_none());
        assert_eq!(result.check_duration_ms, 10);
    }

    #[test]
    fn test_exact_match_result() {
        let result = DuplicateCheckResult::exact_match(
            MemoryId::new("test123"),
            "subcog://global/decisions/test123".to_string(),
            5,
        );
        assert!(result.is_duplicate);
        assert_eq!(result.reason, Some(DuplicateReason::ExactMatch));
        assert!(result.similarity_score.is_none());
        assert_eq!(result.matched_memory_id, Some(MemoryId::new("test123")));
        assert_eq!(
            result.matched_urn,
            Some("subcog://global/decisions/test123".to_string())
        );
    }

    #[test]
    fn test_semantic_match_result() {
        let result = DuplicateCheckResult::semantic_match(
            MemoryId::new("test456"),
            "subcog://global/patterns/test456".to_string(),
            0.94,
            20,
        );
        assert!(result.is_duplicate);
        assert_eq!(result.reason, Some(DuplicateReason::SemanticSimilar));
        assert_eq!(result.similarity_score, Some(0.94));
    }

    #[test]
    fn test_recent_capture_result() {
        let result = DuplicateCheckResult::recent_capture(
            MemoryId::new("test789"),
            "subcog://global/learnings/test789".to_string(),
            1,
        );
        assert!(result.is_duplicate);
        assert_eq!(result.reason, Some(DuplicateReason::RecentCapture));
    }

    #[test]
    fn test_duplicate_reason_display() {
        assert_eq!(DuplicateReason::ExactMatch.to_string(), "exact_match");
        assert_eq!(
            DuplicateReason::SemanticSimilar.to_string(),
            "semantic_similar"
        );
        assert_eq!(DuplicateReason::RecentCapture.to_string(), "recent_capture");
    }
}
