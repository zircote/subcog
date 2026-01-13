//! Memory types and identifiers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a memory.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MemoryId(String);

impl MemoryId {
    /// Creates a new memory ID.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MemoryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for MemoryId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for MemoryId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// A captured memory entry.
#[derive(Debug, Clone)]
pub struct Memory {
    /// Unique identifier.
    pub id: MemoryId,
    /// The memory content.
    pub content: String,
    /// The namespace this memory belongs to.
    pub namespace: super::Namespace,
    /// The domain this memory is associated with.
    pub domain: super::Domain,
    /// Optional project identifier (normalized git remote URL).
    pub project_id: Option<String>,
    /// Optional branch name for project-scoped memories.
    pub branch: Option<String>,
    /// Optional file path relative to repository root.
    pub file_path: Option<String>,
    /// Current status of the memory.
    pub status: super::MemoryStatus,
    /// Creation timestamp (Unix epoch seconds).
    pub created_at: u64,
    /// Last update timestamp (Unix epoch seconds).
    pub updated_at: u64,
    /// Tombstone timestamp (UTC) when soft-deleted.
    ///
    /// Compatibility is handled in storage adapters, so explicit versioning
    /// of the Memory struct is not required at this time.
    pub tombstoned_at: Option<DateTime<Utc>>,
    /// Expiration timestamp (Unix epoch seconds).
    ///
    /// Memory is eligible for automatic cleanup after this timestamp.
    /// Set at capture time as `created_at + ttl_seconds`. Preserved on updates.
    /// `None` means no expiration (memory lives until manually deleted).
    pub expires_at: Option<u64>,
    /// Optional embedding vector.
    pub embedding: Option<Vec<f32>>,
    /// Optional tags for categorization.
    pub tags: Vec<String>,
    /// Optional source reference (file path, URL, etc.).
    pub source: Option<String>,
    /// Whether this memory is a consolidation summary.
    ///
    /// When `true`, this memory represents a consolidated summary of multiple
    /// related memories. The original memories are preserved and linked via
    /// `source_memory_ids`.
    pub is_summary: bool,
    /// IDs of memories that were consolidated into this summary.
    ///
    /// Only populated when `is_summary` is `true`. These represent the original
    /// memories that were analyzed and combined to create this summary.
    pub source_memory_ids: Option<Vec<MemoryId>>,
    /// Timestamp when this memory was consolidated (Unix epoch seconds).
    ///
    /// Only populated for consolidated memories (both summaries and source memories
    /// that have been included in a consolidation).
    pub consolidation_timestamp: Option<u64>,
}

/// Result of a memory operation with optional metadata.
#[derive(Debug, Clone)]
pub struct MemoryResult {
    /// The memory data.
    pub memory: Memory,
    /// Similarity score (0.0 to 1.0) if from a search.
    pub score: Option<f32>,
    /// BM25 score if text search was used.
    pub bm25_score: Option<f32>,
}
