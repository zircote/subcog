//! Memory types and identifiers.

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Unique identifier.
    pub id: MemoryId,
    /// The memory content.
    pub content: String,
    /// The namespace this memory belongs to.
    pub namespace: super::Namespace,
    /// The domain this memory is associated with.
    pub domain: super::Domain,
    /// Current status of the memory.
    pub status: super::MemoryStatus,
    /// Creation timestamp (Unix epoch seconds).
    pub created_at: u64,
    /// Last update timestamp (Unix epoch seconds).
    pub updated_at: u64,
    /// Optional embedding vector.
    #[serde(default)]
    pub embedding: Option<Vec<f32>>,
    /// Optional tags for categorization.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Optional source reference (file path, URL, etc.).
    #[serde(default)]
    pub source: Option<String>,

    // --- Facet fields for storage simplification (Issue #43) ---
    /// Project identifier derived from git remote URL or repository name.
    ///
    /// Used for filtering memories by project in multi-project environments.
    /// Example: `"zircote/subcog"` or `"my-project"`.
    #[serde(default)]
    pub project_id: Option<String>,

    /// Git branch name where the memory was captured.
    ///
    /// Enables branch-scoped memory queries (e.g., feature branch context).
    /// Example: `"main"`, `"feature/auth-system"`.
    #[serde(default)]
    pub branch: Option<String>,

    /// Source file path associated with this memory.
    ///
    /// Used for file-scoped memory surfacing. This differs from `source`
    /// which may contain URLs or other reference types.
    /// Example: `"src/services/capture.rs"`.
    #[serde(default)]
    pub file_path: Option<String>,

    /// Unix timestamp (seconds) when the memory was marked as tombstoned.
    ///
    /// A tombstoned memory is logically deleted but retained for sync
    /// consistency. When set, the memory should be excluded from search
    /// results but preserved in storage for replication.
    #[serde(default)]
    pub tombstoned_at: Option<u64>,
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
