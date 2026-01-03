//! Search types and filters.

use super::{Domain, Memory, MemoryStatus, Namespace};
use std::fmt;

/// Search mode for memory recall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchMode {
    /// Vector similarity search only.
    Vector,
    /// BM25 text search only.
    Text,
    /// Hybrid search with RRF fusion (default).
    #[default]
    Hybrid,
}

/// Level of detail to include in search results.
///
/// Controls response size and token usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DetailLevel {
    /// Frontmatter only: id, namespace, domain, tags, score.
    /// No content included.
    Light,
    /// Frontmatter + summary: truncated content (~200 chars).
    #[default]
    Medium,
    /// Full memory content and all metadata.
    Everything,
}

impl DetailLevel {
    /// Returns the level as a string slice.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Medium => "medium",
            Self::Everything => "everything",
        }
    }

    /// Parses a detail level from a string.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "light" | "minimal" | "frontmatter" => Some(Self::Light),
            "medium" | "summary" | "default" => Some(Self::Medium),
            "everything" | "full" | "all" => Some(Self::Everything),
            _ => None,
        }
    }

    /// Returns the content truncation length for this level.
    #[must_use]
    pub const fn content_length(&self) -> Option<usize> {
        match self {
            Self::Light => Some(0),    // No content
            Self::Medium => Some(200), // Summary
            Self::Everything => None,  // Full content
        }
    }
}

impl fmt::Display for DetailLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl SearchMode {
    /// Returns the mode as a string slice.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Vector => "vector",
            Self::Text => "text",
            Self::Hybrid => "hybrid",
        }
    }
}

impl fmt::Display for SearchMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Filter criteria for memory search.
#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    /// Filter by namespaces.
    pub namespaces: Vec<Namespace>,
    /// Filter by domains.
    pub domains: Vec<Domain>,
    /// Filter by statuses.
    pub statuses: Vec<MemoryStatus>,
    /// Filter by tags (AND logic - must have ALL).
    pub tags: Vec<String>,
    /// Filter by tags (OR logic - must have ANY).
    pub tags_any: Vec<String>,
    /// Exclude memories with these tags.
    pub excluded_tags: Vec<String>,
    /// Filter by source pattern (glob-style).
    pub source_pattern: Option<String>,
    /// Minimum creation timestamp.
    pub created_after: Option<u64>,
    /// Maximum creation timestamp.
    pub created_before: Option<u64>,
    /// Minimum similarity score (0.0 to 1.0).
    pub min_score: Option<f32>,
    /// Filter by project identifier.
    ///
    /// When set, only memories associated with this project are returned.
    /// Projects provide logical grouping across repositories.
    pub project_id: Option<String>,
    /// Filter by git branch name.
    ///
    /// When set, only memories captured on this branch are returned.
    /// Useful for feature-branch-specific context.
    pub branch: Option<String>,
    /// Filter by file path pattern (glob-style).
    ///
    /// Matches against the file path metadata of memories.
    /// Supports glob patterns like `src/**/*.rs` or `tests/*.rs`.
    pub file_path_pattern: Option<String>,
    /// Include tombstoned (soft-deleted) memories in results.
    ///
    /// Defaults to `false`. When `true`, tombstoned memories are
    /// included in search results for audit or recovery purposes.
    pub include_tombstoned: bool,
}

impl SearchFilter {
    /// Creates an empty filter (matches all).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            namespaces: Vec::new(),
            domains: Vec::new(),
            statuses: Vec::new(),
            tags: Vec::new(),
            tags_any: Vec::new(),
            excluded_tags: Vec::new(),
            source_pattern: None,
            created_after: None,
            created_before: None,
            min_score: None,
            project_id: None,
            branch: None,
            file_path_pattern: None,
            include_tombstoned: false,
        }
    }

    /// Adds a namespace filter.
    #[must_use]
    pub fn with_namespace(mut self, namespace: Namespace) -> Self {
        self.namespaces.push(namespace);
        self
    }

    /// Adds a domain filter.
    #[must_use]
    pub fn with_domain(mut self, domain: Domain) -> Self {
        self.domains.push(domain);
        self
    }

    /// Adds a status filter.
    #[must_use]
    pub fn with_status(mut self, status: MemoryStatus) -> Self {
        self.statuses.push(status);
        self
    }

    /// Adds a tag filter (AND logic - must have ALL).
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Adds a tag filter (OR logic - must have ANY).
    #[must_use]
    pub fn with_tag_any(mut self, tag: impl Into<String>) -> Self {
        self.tags_any.push(tag.into());
        self
    }

    /// Adds an excluded tag filter.
    #[must_use]
    pub fn with_excluded_tag(mut self, tag: impl Into<String>) -> Self {
        self.excluded_tags.push(tag.into());
        self
    }

    /// Sets the source pattern filter (glob-style).
    #[must_use]
    pub fn with_source_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.source_pattern = Some(pattern.into());
        self
    }

    /// Sets the minimum score threshold.
    #[must_use]
    pub const fn with_min_score(mut self, score: f32) -> Self {
        self.min_score = Some(score);
        self
    }

    /// Sets the `created_after` filter.
    #[must_use]
    pub const fn with_created_after(mut self, timestamp: u64) -> Self {
        self.created_after = Some(timestamp);
        self
    }

    /// Sets the `created_before` filter.
    #[must_use]
    pub const fn with_created_before(mut self, timestamp: u64) -> Self {
        self.created_before = Some(timestamp);
        self
    }

    /// Sets the project identifier filter.
    ///
    /// Only memories associated with the specified project will be returned.
    #[must_use]
    pub fn with_project_id(mut self, project_id: impl Into<String>) -> Self {
        self.project_id = Some(project_id.into());
        self
    }

    /// Sets the git branch filter.
    ///
    /// Only memories captured on the specified branch will be returned.
    #[must_use]
    pub fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    /// Sets the file path pattern filter (glob-style).
    ///
    /// Matches against the file path metadata of memories.
    /// Supports glob patterns like `src/**/*.rs` or `tests/*.rs`.
    #[must_use]
    pub fn with_file_path_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.file_path_pattern = Some(pattern.into());
        self
    }

    /// Sets whether to include tombstoned memories in results.
    ///
    /// When `true`, soft-deleted memories are included for audit or recovery.
    #[must_use]
    pub const fn with_include_tombstoned(mut self, include: bool) -> Self {
        self.include_tombstoned = include;
        self
    }

    /// Returns true if the filter is empty (matches all).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.namespaces.is_empty()
            && self.domains.is_empty()
            && self.statuses.is_empty()
            && self.tags.is_empty()
            && self.tags_any.is_empty()
            && self.excluded_tags.is_empty()
            && self.source_pattern.is_none()
            && self.created_after.is_none()
            && self.created_before.is_none()
            && self.min_score.is_none()
            && self.project_id.is_none()
            && self.branch.is_none()
            && self.file_path_pattern.is_none()
            && !self.include_tombstoned
    }
}

/// Hint about tombstoned memories that may be relevant to a search.
///
/// When search results are sparse (< 3 active results), the system checks
/// if there are tombstoned memories that match the query. This hint provides
/// visibility into potentially relevant deleted content.
#[derive(Debug, Clone, Default)]
pub struct TombstoneHint {
    /// Total count of tombstoned memories matching the query.
    pub count: usize,
    /// Branch names where tombstoned memories were found.
    /// Limited to first 5 branches for display purposes.
    pub branches: Vec<String>,
}

impl TombstoneHint {
    /// Creates a new tombstone hint.
    #[must_use]
    pub fn new(count: usize, branches: Vec<String>) -> Self {
        // Vec::len() call prevents clippy::missing_const_for_fn false positive
        let _ = branches.len();
        Self { count, branches }
    }

    /// Returns true if there are tombstoned memories.
    #[must_use]
    pub const fn has_tombstones(&self) -> bool {
        self.count > 0
    }

    /// Formats the hint as a human-readable message.
    #[must_use]
    pub fn message(&self) -> Option<String> {
        if self.count == 0 {
            return None;
        }

        let branch_info = if self.branches.is_empty() {
            String::new()
        } else {
            let branch_list = self.branches.join(", ");
            format!(" in branches: {branch_list}")
        };

        Some(format!(
            "{} additional memories found in deleted/merged branches{}",
            self.count, branch_info
        ))
    }
}

/// Result of a memory search.
#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    /// The matching memories.
    pub memories: Vec<SearchHit>,
    /// Total count of matches (may be more than returned).
    pub total_count: usize,
    /// The search mode used.
    pub mode: SearchMode,
    /// Search execution time in milliseconds.
    pub execution_time_ms: u64,
    /// Hint about tombstoned memories when active results are sparse.
    ///
    /// Populated when:
    /// - Active results < 3
    /// - `include_tombstoned` was false (default)
    /// - Tombstoned memories exist that match the query
    pub tombstone_hint: Option<TombstoneHint>,
}

/// A single search hit with scoring.
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// The matched memory.
    pub memory: Memory,
    /// Normalized combined score (0.0 to 1.0).
    /// This is the primary score for display to users.
    /// The max score in a result set is always 1.0.
    pub score: f32,
    /// Raw combined score before normalization.
    /// Useful for debugging RRF fusion behavior.
    /// This is the sum of RRF contributions from text and vector search.
    pub raw_score: f32,
    /// Vector similarity score if applicable.
    pub vector_score: Option<f32>,
    /// BM25 text score if applicable.
    pub bm25_score: Option<f32>,
}
