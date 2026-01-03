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

impl std::str::FromStr for Namespace {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("unknown namespace: {s}"))
    }
}

/// Domain separation for memories.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Domain {
    /// Organization or team identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    /// Project identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Repository identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

    /// Creates a domain based on the current working directory context.
    ///
    /// - If in a git repository: returns a project-scoped domain
    /// - If NOT in a git repository: returns a user-scoped domain
    ///
    /// This ensures memories are routed to the appropriate storage backend:
    /// - Project domains use project-scoped `SQLite` storage
    /// - User domains use user-scoped `SQLite` storage
    #[must_use]
    pub fn default_for_context() -> Self {
        use crate::storage::index::is_in_git_repo;

        if is_in_git_repo() {
            // In a git repo - use project scope (empty domain = project-local)
            Self::new()
        } else {
            // Not in a git repo - use user scope
            Self::for_user()
        }
    }

    /// Creates a user-scoped domain.
    ///
    /// User-scoped memories are stored in the user's personal sqlite database
    /// and are accessible across all projects.
    #[must_use]
    pub fn for_user() -> Self {
        Self {
            organization: None,
            project: Some("user".to_string()),
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

    /// Returns true if this is a global/project domain (no restrictions).
    #[must_use]
    pub const fn is_global(&self) -> bool {
        self.organization.is_none() && self.project.is_none() && self.repository.is_none()
    }

    /// Returns true if this is a user-scoped domain.
    #[must_use]
    pub fn is_user(&self) -> bool {
        self.project.as_deref() == Some("user") && self.organization.is_none()
    }

    /// Returns the scope string for URN construction.
    ///
    /// This method provides consistent URN scope generation across the codebase.
    /// URN format: `subcog://{scope}/{namespace}/{id}`
    ///
    /// # Scope Values
    ///
    /// - `"project"` - Project-local domain (default, in git repo context)
    /// - `"user"` - User-scoped domain (outside git repo or explicit user scope)
    /// - `"{org}/{repo}"` - Repository-scoped domain
    /// - `"org/{org}"` - Organization-scoped domain
    ///
    /// # Examples
    ///
    /// ```rust
    /// use subcog::models::Domain;
    ///
    /// // Project-local scope
    /// let domain = Domain::new();
    /// assert_eq!(domain.urn_scope(), "project");
    ///
    /// // User scope
    /// let domain = Domain::for_user();
    /// assert_eq!(domain.urn_scope(), "user");
    ///
    /// // Repository scope
    /// let domain = Domain::for_repository("zircote", "subcog");
    /// assert_eq!(domain.urn_scope(), "zircote/subcog");
    /// ```
    #[must_use]
    pub fn urn_scope(&self) -> String {
        // Check for user scope first (explicit check)
        if self.is_user() {
            return "user".to_string();
        }

        // Check for project-local scope (empty domain)
        if self.is_global() {
            return "project".to_string();
        }

        // For other scopes, use the display representation
        self.to_string()
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
            // User-scoped domain shows as "user"
            (None, Some(proj), _) if proj == "user" => write!(f, "user"),
            (None, Some(proj), _) => write!(f, "{proj}"),
            (None, None, Some(repo)) => write!(f, "{repo}"),
            // Project-local domain (empty domain)
            (None, None, None) => write!(f, "project"),
        }
    }
}

/// Status of a memory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
    /// Memory from deleted branch, excluded from search by default.
    Tombstoned,
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
            Self::Tombstoned => "tombstoned",
        }
    }

    /// Parses a status from a string.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "active" => Some(Self::Active),
            "archived" => Some(Self::Archived),
            "superseded" => Some(Self::Superseded),
            "pending" => Some(Self::Pending),
            "deleted" => Some(Self::Deleted),
            "tombstoned" => Some(Self::Tombstoned),
            _ => None,
        }
    }
}

impl fmt::Display for MemoryStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for MemoryStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("unknown memory status: {s}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_status_as_str() {
        assert_eq!(MemoryStatus::Active.as_str(), "active");
        assert_eq!(MemoryStatus::Archived.as_str(), "archived");
        assert_eq!(MemoryStatus::Superseded.as_str(), "superseded");
        assert_eq!(MemoryStatus::Pending.as_str(), "pending");
        assert_eq!(MemoryStatus::Deleted.as_str(), "deleted");
        assert_eq!(MemoryStatus::Tombstoned.as_str(), "tombstoned");
    }

    #[test]
    fn test_memory_status_display() {
        assert_eq!(format!("{}", MemoryStatus::Active), "active");
        assert_eq!(format!("{}", MemoryStatus::Tombstoned), "tombstoned");
    }

    #[test]
    fn test_memory_status_parse() {
        assert_eq!(MemoryStatus::parse("active"), Some(MemoryStatus::Active));
        assert_eq!(MemoryStatus::parse("ACTIVE"), Some(MemoryStatus::Active));
        assert_eq!(
            MemoryStatus::parse("tombstoned"),
            Some(MemoryStatus::Tombstoned)
        );
        assert_eq!(
            MemoryStatus::parse("TOMBSTONED"),
            Some(MemoryStatus::Tombstoned)
        );
        assert_eq!(MemoryStatus::parse("invalid"), None);
    }

    #[test]
    fn test_memory_status_from_str() {
        assert_eq!("active".parse::<MemoryStatus>(), Ok(MemoryStatus::Active));
        assert_eq!(
            "tombstoned".parse::<MemoryStatus>(),
            Ok(MemoryStatus::Tombstoned)
        );
        assert!("invalid".parse::<MemoryStatus>().is_err());
    }

    #[test]
    fn test_memory_status_default() {
        assert_eq!(MemoryStatus::default(), MemoryStatus::Active);
    }

    #[test]
    fn test_memory_status_serde() {
        // Test serialization
        let status = MemoryStatus::Tombstoned;
        let json = serde_json::to_string(&status).expect("serialize");
        assert_eq!(json, "\"tombstoned\"");

        // Test deserialization
        let parsed: MemoryStatus = serde_json::from_str("\"tombstoned\"").expect("deserialize");
        assert_eq!(parsed, MemoryStatus::Tombstoned);

        // Test all variants roundtrip
        for status in [
            MemoryStatus::Active,
            MemoryStatus::Archived,
            MemoryStatus::Superseded,
            MemoryStatus::Pending,
            MemoryStatus::Deleted,
            MemoryStatus::Tombstoned,
        ] {
            let json = serde_json::to_string(&status).expect("serialize");
            let parsed: MemoryStatus = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn test_domain_serde() {
        // Test empty domain
        let domain = Domain::new();
        let json = serde_json::to_string(&domain).expect("serialize");
        assert_eq!(json, "{}");

        let parsed: Domain = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, domain);

        // Test domain with all fields
        let domain = Domain {
            organization: Some("org".to_string()),
            project: Some("proj".to_string()),
            repository: Some("repo".to_string()),
        };
        let json = serde_json::to_string(&domain).expect("serialize");
        let parsed: Domain = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, domain);

        // Test user domain
        let domain = Domain::for_user();
        let json = serde_json::to_string(&domain).expect("serialize");
        let parsed: Domain = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, domain);
    }

    // ========================================================================
    // URN Scope Tests (Task 3.4: Storage Simplification)
    // ========================================================================

    #[test]
    fn test_urn_scope_project() {
        let domain = Domain::new();
        assert!(domain.is_global());
        assert_eq!(domain.urn_scope(), "project");
    }

    #[test]
    fn test_urn_scope_user() {
        let domain = Domain::for_user();
        assert!(domain.is_user());
        assert_eq!(domain.urn_scope(), "user");
    }

    #[test]
    fn test_urn_scope_repository() {
        let domain = Domain::for_repository("zircote", "subcog");
        assert!(!domain.is_global());
        assert!(!domain.is_user());
        assert_eq!(domain.urn_scope(), "zircote/subcog");
    }

    #[test]
    fn test_urn_scope_organization_only() {
        let domain = Domain {
            organization: Some("acme".to_string()),
            project: None,
            repository: None,
        };
        assert_eq!(domain.urn_scope(), "acme");
    }

    #[test]
    fn test_urn_scope_consistency() {
        // Ensure URN patterns are consistent
        // Project-local domain -> "project"
        assert_eq!(Domain::new().urn_scope(), "project");
        assert_eq!(Domain::new().to_string(), "project");

        // User domain -> "user"
        assert_eq!(Domain::for_user().urn_scope(), "user");
        assert_eq!(Domain::for_user().to_string(), "user");

        // Repository domain -> "org/repo"
        let repo_domain = Domain::for_repository("org", "repo");
        assert_eq!(repo_domain.urn_scope(), "org/repo");
        assert_eq!(repo_domain.to_string(), "org/repo");
    }
}
