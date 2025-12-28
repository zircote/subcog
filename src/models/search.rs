//! Search types and filters.

use super::{Domain, Memory, MemoryStatus, Namespace};

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

/// Filter criteria for memory search.
#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    /// Filter by namespaces.
    pub namespaces: Vec<Namespace>,
    /// Filter by domains.
    pub domains: Vec<Domain>,
    /// Filter by statuses.
    pub statuses: Vec<MemoryStatus>,
    /// Filter by tags (AND logic).
    pub tags: Vec<String>,
    /// Minimum creation timestamp.
    pub created_after: Option<u64>,
    /// Maximum creation timestamp.
    pub created_before: Option<u64>,
    /// Minimum similarity score (0.0 to 1.0).
    pub min_score: Option<f32>,
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
            created_after: None,
            created_before: None,
            min_score: None,
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

    /// Sets the minimum score threshold.
    #[must_use]
    pub const fn with_min_score(mut self, score: f32) -> Self {
        self.min_score = Some(score);
        self
    }

    /// Returns true if the filter is empty (matches all).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.namespaces.is_empty()
            && self.domains.is_empty()
            && self.statuses.is_empty()
            && self.tags.is_empty()
            && self.created_after.is_none()
            && self.created_before.is_none()
            && self.min_score.is_none()
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
    /// Combined score (0.0 to 1.0).
    pub score: f32,
    /// Vector similarity score if applicable.
    pub vector_score: Option<f32>,
    /// BM25 text score if applicable.
    pub bm25_score: Option<f32>,
}
