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

impl Memory {
    /// Generates the URN (Uniform Resource Name) for this memory.
    ///
    /// URN format: `subcog://{scope}/{namespace}/{id}`
    ///
    /// The scope is derived from the domain using [`Domain::urn_scope()`]:
    /// - `"global"` for project-local memories (default when in git repo)
    /// - `"user"` for user-scoped memories (outside git repo or explicit)
    /// - `"{org}/{repo}"` for repository-scoped memories
    ///
    /// # Examples
    ///
    /// ```rust
    /// use subcog::models::{Memory, MemoryId, Domain, Namespace, MemoryStatus};
    ///
    /// let memory = Memory {
    ///     id: MemoryId::new("abc123"),
    ///     content: "Use PostgreSQL".to_string(),
    ///     namespace: Namespace::Decisions,
    ///     domain: Domain::new(),
    ///     status: MemoryStatus::Active,
    ///     created_at: 0,
    ///     updated_at: 0,
    ///     embedding: None,
    ///     tags: vec![],
    ///     source: None,
    ///     project_id: None,
    ///     branch: None,
    ///     file_path: None,
    ///     tombstoned_at: None,
    /// };
    ///
    /// assert_eq!(memory.urn(), "subcog://project/decisions/abc123");
    /// ```
    #[must_use]
    pub fn urn(&self) -> String {
        format!(
            "subcog://{}/{}/{}",
            self.domain.urn_scope(),
            self.namespace.as_str(),
            self.id.as_str()
        )
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, MemoryStatus, Namespace};

    fn test_memory(domain: Domain) -> Memory {
        Memory {
            id: MemoryId::new("test123"),
            content: "Test content".to_string(),
            namespace: Namespace::Decisions,
            domain,
            status: MemoryStatus::Active,
            created_at: 0,
            updated_at: 0,
            embedding: None,
            tags: vec![],
            source: None,
            project_id: None,
            branch: None,
            file_path: None,
            tombstoned_at: None,
        }
    }

    #[test]
    fn test_urn_project_domain() {
        let memory = test_memory(Domain::new());
        assert_eq!(memory.urn(), "subcog://project/decisions/test123");
    }

    #[test]
    fn test_urn_user_domain() {
        let memory = test_memory(Domain::for_user());
        assert_eq!(memory.urn(), "subcog://user/decisions/test123");
    }

    #[test]
    fn test_urn_repository_domain() {
        let memory = test_memory(Domain::for_repository("zircote", "subcog"));
        assert_eq!(memory.urn(), "subcog://zircote/subcog/decisions/test123");
    }

    #[test]
    fn test_urn_different_namespaces() {
        let mut memory = test_memory(Domain::new());

        memory.namespace = Namespace::Learnings;
        assert_eq!(memory.urn(), "subcog://project/learnings/test123");

        memory.namespace = Namespace::Patterns;
        assert_eq!(memory.urn(), "subcog://project/patterns/test123");

        memory.namespace = Namespace::TechDebt;
        assert_eq!(memory.urn(), "subcog://project/tech-debt/test123");
    }

    #[test]
    fn test_urn_consistency() {
        // These URN formats should match the domain naming conventions
        let project_memory = test_memory(Domain::new());
        assert!(project_memory.urn().starts_with("subcog://project/"));

        let user_memory = test_memory(Domain::for_user());
        assert!(user_memory.urn().starts_with("subcog://user/"));

        let repo_memory = test_memory(Domain::for_repository("org", "repo"));
        assert!(repo_memory.urn().starts_with("subcog://org/repo/"));
    }
}
