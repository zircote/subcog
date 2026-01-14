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
    /// Filter by project identifier (normalized git remote URL).
    pub project_id: Option<String>,
    /// Filter by branch name.
    pub branch: Option<String>,
    /// Filter by file path (relative to repo root).
    pub file_path: Option<String>,
    /// Minimum creation timestamp.
    pub created_after: Option<u64>,
    /// Maximum creation timestamp.
    pub created_before: Option<u64>,
    /// Minimum similarity score (0.0 to 1.0).
    pub min_score: Option<f32>,
    /// Include tombstoned memories (default: false).
    pub include_tombstoned: bool,
    /// Filter by entity names (memories mentioning these entities).
    /// Uses OR logic - matches memories mentioning ANY of the listed entities.
    pub entity_names: Vec<String>,
    /// Filter by group identifiers (group-scoped memories).
    /// Uses OR logic - matches memories in ANY of the listed groups.
    #[cfg(feature = "group-scope")]
    pub group_ids: Vec<String>,
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
            project_id: None,
            branch: None,
            file_path: None,
            created_after: None,
            created_before: None,
            min_score: None,
            include_tombstoned: false,
            entity_names: Vec::new(),
            #[cfg(feature = "group-scope")]
            group_ids: Vec::new(),
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

    /// Sets the project identifier filter.
    #[must_use]
    pub fn with_project_id(mut self, project_id: impl Into<String>) -> Self {
        self.project_id = Some(project_id.into());
        self
    }

    /// Sets the branch filter.
    #[must_use]
    pub fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    /// Sets the file path filter.
    #[must_use]
    pub fn with_file_path(mut self, file_path: impl Into<String>) -> Self {
        self.file_path = Some(file_path.into());
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

    /// Includes tombstoned memories in results.
    #[must_use]
    pub const fn with_include_tombstoned(mut self, include: bool) -> Self {
        self.include_tombstoned = include;
        self
    }

    /// Returns true if the filter is empty (matches all).
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Can't be const due to cfg attributes
    pub fn is_empty(&self) -> bool {
        let base_empty = self.namespaces.is_empty()
            && self.domains.is_empty()
            && self.statuses.is_empty()
            && self.tags.is_empty()
            && self.tags_any.is_empty()
            && self.excluded_tags.is_empty()
            && self.source_pattern.is_none()
            && self.project_id.is_none()
            && self.branch.is_none()
            && self.file_path.is_none()
            && self.created_after.is_none()
            && self.created_before.is_none()
            && self.min_score.is_none()
            && self.entity_names.is_empty();

        #[cfg(feature = "group-scope")]
        {
            base_empty && self.group_ids.is_empty()
        }
        #[cfg(not(feature = "group-scope"))]
        {
            base_empty
        }
    }

    /// Adds an entity name filter.
    /// Filters to memories mentioning this entity (OR logic with other entities).
    #[must_use]
    pub fn with_entity(mut self, entity: impl Into<String>) -> Self {
        self.entity_names.push(entity.into());
        self
    }

    /// Adds multiple entity name filters.
    #[must_use]
    pub fn with_entities(mut self, entities: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.entity_names
            .extend(entities.into_iter().map(Into::into));
        self
    }

    /// Adds a group identifier filter.
    ///
    /// Filters to memories belonging to this group (OR logic with other groups).
    #[cfg(feature = "group-scope")]
    #[must_use]
    pub fn with_group_id(mut self, group_id: impl Into<String>) -> Self {
        self.group_ids.push(group_id.into());
        self
    }

    /// Adds multiple group identifier filters.
    #[cfg(feature = "group-scope")]
    #[must_use]
    pub fn with_group_ids(
        mut self,
        group_ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.group_ids.extend(group_ids.into_iter().map(Into::into));
        self
    }
}

/// Result of a memory search.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The matching memories.
    pub memories: Vec<SearchHit>,
    /// Total count of matches (may be more than returned).
    pub total_count: usize,
    /// The search mode used.
    pub mode: SearchMode,
    /// Search execution time in milliseconds.
    pub execution_time_ms: u64,
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
