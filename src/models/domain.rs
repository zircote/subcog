//! Domain and namespace types.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Memory namespace categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
    #[serde(alias = "techdebt", alias = "tech_debt")]
    #[serde(rename = "tech-debt")]
    TechDebt,
    /// Blockers and impediments.
    Blockers,
    /// Work progress and milestones.
    Progress,
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
    /// Built-in help content (read-only system namespace).
    Help,
    /// Reusable prompt templates with variable substitution.
    Prompts,
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
            Self::Blockers,
            Self::Progress,
            Self::Apis,
            Self::Config,
            Self::Security,
            Self::Performance,
            Self::Testing,
            Self::Help,
            Self::Prompts,
        ]
    }

    /// Returns user-facing namespaces (excludes system namespaces like Help).
    #[must_use]
    pub const fn user_namespaces() -> &'static [Self] {
        &[
            Self::Decisions,
            Self::Patterns,
            Self::Learnings,
            Self::Context,
            Self::TechDebt,
            Self::Blockers,
            Self::Progress,
            Self::Apis,
            Self::Config,
            Self::Security,
            Self::Performance,
            Self::Testing,
            Self::Prompts,
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
            Self::Blockers => "blockers",
            Self::Progress => "progress",
            Self::Apis => "apis",
            Self::Config => "config",
            Self::Security => "security",
            Self::Performance => "performance",
            Self::Testing => "testing",
            Self::Help => "help",
            Self::Prompts => "prompts",
        }
    }

    /// Returns true if this is a system namespace (read-only).
    #[must_use]
    pub const fn is_system(&self) -> bool {
        matches!(self, Self::Help)
    }

    /// Parses a namespace from a string.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "decisions" => Some(Self::Decisions),
            "patterns" => Some(Self::Patterns),
            "learnings" => Some(Self::Learnings),
            "context" => Some(Self::Context),
            "tech-debt" | "techdebt" | "tech_debt" => Some(Self::TechDebt),
            "blockers" => Some(Self::Blockers),
            "progress" => Some(Self::Progress),
            "apis" => Some(Self::Apis),
            "config" => Some(Self::Config),
            "security" => Some(Self::Security),
            "performance" => Some(Self::Performance),
            "testing" => Some(Self::Testing),
            "help" => Some(Self::Help),
            "prompts" => Some(Self::Prompts),
            _ => None,
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

    /// Returns the scope string for URN construction.
    ///
    /// - `"project"` for project-scoped (org + repo)
    /// - `"org/{name}"` for organization-scoped
    /// - `"global"` for global domain
    #[must_use]
    pub fn to_scope_string(&self) -> String {
        match (&self.organization, &self.repository) {
            (Some(org), Some(repo)) => format!("{org}/{repo}"),
            (Some(org), None) => format!("org/{org}"),
            (None, Some(repo)) => repo.clone(),
            (None, None) => "project".to_string(),
        }
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
