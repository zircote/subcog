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
    pub tombstoned_at: Option<DateTime<Utc>>,
    /// Optional embedding vector.
    pub embedding: Option<Vec<f32>>,
    /// Optional tags for categorization.
    pub tags: Vec<String>,
    /// Optional source reference (file path, URL, etc.).
    pub source: Option<String>,
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
