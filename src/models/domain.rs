//! Domain and namespace types.

use std::fmt;

/// Memory namespace categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Namespace {
    /// Architectural and design decisions.
    #[default]
    Decisions,
    /// Discovered patterns and conventions.
    Patterns,
    /// Lessons learned from debugging or issues.
    Learnings,
    /// Important contextual information.
    Context,
    /// Technical debts and future improvements.
    TechDebt,
    /// API endpoints and contracts.
    Apis,
    /// Configuration and environment details.
    Config,
    /// Security-related information.
    Security,
    /// Performance optimizations and benchmarks.
    Performance,
    /// Testing strategies and edge cases.
    Testing,
}

impl Namespace {
    /// Returns all namespace variants.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Decisions,
            Self::Patterns,
            Self::Learnings,
            Self::Context,
            Self::TechDebt,
            Self::Apis,
            Self::Config,
            Self::Security,
            Self::Performance,
            Self::Testing,
        ]
    }

    /// Returns the namespace as a string slice.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Decisions => "decisions",
            Self::Patterns => "patterns",
            Self::Learnings => "learnings",
            Self::Context => "context",
            Self::TechDebt => "tech-debt",
            Self::Apis => "apis",
            Self::Config => "config",
            Self::Security => "security",
            Self::Performance => "performance",
            Self::Testing => "testing",
        }
    }
}

impl fmt::Display for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Domain separation for memories.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Domain {
    /// Organization or team identifier.
    pub organization: Option<String>,
    /// Project identifier.
    pub project: Option<String>,
    /// Repository identifier.
    pub repository: Option<String>,
}

impl Domain {
    /// Creates a new domain with all fields empty.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            organization: None,
            project: None,
            repository: None,
        }
    }

    /// Creates a domain for a specific repository.
    #[must_use]
    pub fn for_repository(org: impl Into<String>, repo: impl Into<String>) -> Self {
        Self {
            organization: Some(org.into()),
            project: None,
            repository: Some(repo.into()),
        }
    }

    /// Returns true if this is a global domain (no restrictions).
    #[must_use]
    pub const fn is_global(&self) -> bool {
        self.organization.is_none() && self.project.is_none() && self.repository.is_none()
    }
}

impl fmt::Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.organization, &self.project, &self.repository) {
            (Some(org), Some(proj), Some(repo)) => write!(f, "{org}/{proj}/{repo}"),
            (Some(org), None, Some(repo)) => write!(f, "{org}/{repo}"),
            (Some(org), Some(proj), None) => write!(f, "{org}/{proj}"),
            (Some(org), None, None) => write!(f, "{org}"),
            (None, Some(proj), _) => write!(f, "{proj}"),
            (None, None, Some(repo)) => write!(f, "{repo}"),
            (None, None, None) => write!(f, "global"),
        }
    }
}

/// Status of a memory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MemoryStatus {
    /// Active and searchable.
    #[default]
    Active,
    /// Archived but still searchable.
    Archived,
    /// Superseded by another memory.
    Superseded,
    /// Pending review or approval.
    Pending,
    /// Marked for deletion.
    Deleted,
}

impl MemoryStatus {
    /// Returns the status as a string slice.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
            Self::Superseded => "superseded",
            Self::Pending => "pending",
            Self::Deleted => "deleted",
        }
    }
}

impl fmt::Display for MemoryStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
