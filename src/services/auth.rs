//! Service-layer authorization (CRIT-006).
//!
//! Provides authorization context that can be passed to service methods
//! for fine-grained access control. This complements MCP-layer JWT auth
//! by enforcing permissions at the service boundary.
//!
//! # Design Principles
//!
//! - **Opt-in**: Services work without auth context (CLI/local use)
//! - **Defense in depth**: Complements transport-layer auth
//! - **Audit trail**: All authorization decisions are logged
//!
//! # Usage
//!
//! ```rust,ignore
//! use subcog::services::auth::{AuthContext, Permission};
//!
//! // Create context from JWT claims
//! let ctx = AuthContext::from_scopes(vec!["read".to_string(), "write".to_string()])
//!     .with_subject("user-123");
//!
//! // Check permission before operation
//! ctx.require(Permission::Write)?;
//!
//! // Or use the builder pattern
//! let ctx = AuthContext::builder()
//!     .subject("user-123")
//!     .scope("read")
//!     .scope("write")
//!     .build();
//! ```

use crate::{Error, Result};
use std::collections::HashSet;

#[cfg(feature = "group-scope")]
use std::collections::HashMap;

#[cfg(feature = "group-scope")]
use crate::models::group::GroupRole;

/// Permissions for service operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Read operations (recall, status, list).
    Read,
    /// Write operations (capture, enrich, delete).
    Write,
    /// Admin operations (sync, reindex, consolidate).
    Admin,
}

impl Permission {
    /// Returns the scope string for this permission.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Admin => "admin",
        }
    }

    /// Parses a scope string into a permission.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "read" => Some(Self::Read),
            "write" => Some(Self::Write),
            "admin" => Some(Self::Admin),
            _ => None,
        }
    }
}

/// Authorization context for service operations.
///
/// Carries identity and permission information through the service layer.
/// Can be created from JWT claims or constructed directly for testing.
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// Subject identifier (user ID, service account, etc.).
    subject: Option<String>,
    /// Granted scopes/permissions.
    scopes: HashSet<String>,
    /// Whether this is a local/CLI context (implicitly trusted).
    is_local: bool,
    /// Organization name (for org-scoped operations).
    org_name: Option<String>,
    /// Role within the organization (admin, member, etc.).
    org_role: Option<String>,
    /// Group roles (`group_id` → role string).
    #[cfg(feature = "group-scope")]
    group_roles: HashMap<String, String>,
}

impl Default for AuthContext {
    /// Creates a default context that allows all operations.
    ///
    /// This is used for CLI/local access where there's no authentication.
    fn default() -> Self {
        Self::local()
    }
}

impl AuthContext {
    /// Creates a local context with full permissions.
    ///
    /// Used for CLI access where the user has local filesystem access.
    #[must_use]
    pub fn local() -> Self {
        Self {
            subject: None,
            scopes: HashSet::new(),
            is_local: true,
            org_name: None,
            org_role: None,
            #[cfg(feature = "group-scope")]
            group_roles: HashMap::new(),
        }
    }

    /// Creates a context from a list of scope strings.
    ///
    /// # Arguments
    ///
    /// * `scopes` - List of scope strings (e.g., `["read", "write"]`).
    #[must_use]
    pub fn from_scopes(scopes: Vec<String>) -> Self {
        Self {
            subject: None,
            scopes: scopes.into_iter().collect(),
            is_local: false,
            org_name: None,
            org_role: None,
            #[cfg(feature = "group-scope")]
            group_roles: HashMap::new(),
        }
    }

    /// Creates a builder for constructing an auth context.
    #[must_use]
    pub fn builder() -> AuthContextBuilder {
        AuthContextBuilder::default()
    }

    /// Sets the subject identifier.
    #[must_use]
    pub fn with_subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// Returns the subject identifier.
    #[must_use]
    pub fn subject(&self) -> Option<&str> {
        self.subject.as_deref()
    }

    /// Returns whether this is a local/CLI context.
    #[must_use]
    pub const fn is_local(&self) -> bool {
        self.is_local
    }

    /// Returns the organization name if set.
    #[must_use]
    pub fn org_name(&self) -> Option<&str> {
        self.org_name.as_deref()
    }

    /// Returns the organization role if set.
    #[must_use]
    pub fn org_role(&self) -> Option<&str> {
        self.org_role.as_deref()
    }

    /// Returns whether this context has org access.
    #[must_use]
    pub fn has_org_access(&self) -> bool {
        // Local contexts have org access if org is configured
        if self.is_local {
            return true;
        }
        // Remote contexts need org:read or org:write scope
        self.scopes.contains("org:read")
            || self.scopes.contains("org:write")
            || self.scopes.contains("*")
    }

    /// Checks if the context has a specific scope.
    #[must_use]
    pub fn has_scope(&self, scope: &str) -> bool {
        // Local contexts have all permissions
        if self.is_local {
            return true;
        }
        // Wildcard scope grants everything
        if self.scopes.contains("*") {
            return true;
        }
        self.scopes.contains(scope)
    }

    /// Checks if the context has a specific permission.
    #[must_use]
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.has_scope(permission.as_str())
    }

    /// Checks if the context has any of the specified permissions.
    #[must_use]
    pub fn has_any_permission(&self, permissions: &[Permission]) -> bool {
        permissions.iter().any(|p| self.has_permission(*p))
    }

    /// Requires a specific permission, returning an error if not granted.
    ///
    /// # Errors
    ///
    /// Returns `Error::Unauthorized` if the permission is not granted.
    pub fn require(&self, permission: Permission) -> Result<()> {
        if self.has_permission(permission) {
            tracing::debug!(
                subject = ?self.subject,
                permission = permission.as_str(),
                is_local = self.is_local,
                "Authorization granted"
            );
            Ok(())
        } else {
            tracing::warn!(
                subject = ?self.subject,
                permission = permission.as_str(),
                scopes = ?self.scopes,
                "Authorization denied"
            );
            Err(Error::Unauthorized(format!(
                "Permission '{}' required",
                permission.as_str()
            )))
        }
    }

    /// Requires any of the specified permissions.
    ///
    /// # Errors
    ///
    /// Returns `Error::Unauthorized` if none of the permissions are granted.
    pub fn require_any(&self, permissions: &[Permission]) -> Result<()> {
        if self.has_any_permission(permissions) {
            Ok(())
        } else {
            let required: Vec<_> = permissions.iter().map(Permission::as_str).collect();
            Err(Error::Unauthorized(format!(
                "One of permissions {required:?} required"
            )))
        }
    }

    /// Returns the user's role in a specific group.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group identifier.
    ///
    /// # Returns
    ///
    /// `Some(GroupRole)` if the user has a role in the group, `None` otherwise.
    /// For local contexts, always returns `Some(GroupRole::Admin)`.
    #[cfg(feature = "group-scope")]
    #[must_use]
    pub fn get_group_role(&self, group_id: &str) -> Option<GroupRole> {
        // Local contexts have admin access to all groups
        if self.is_local {
            return Some(GroupRole::Admin);
        }
        // Wildcard scope grants admin to all groups
        if self.scopes.contains("*") {
            return Some(GroupRole::Admin);
        }
        // Look up the specific group role
        self.group_roles
            .get(group_id)
            .and_then(|role| GroupRole::parse(role))
    }

    /// Checks if the user has at least the specified role in a group.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group identifier.
    /// * `min_role` - The minimum required role.
    ///
    /// # Returns
    ///
    /// `true` if the user has sufficient permissions, `false` otherwise.
    #[cfg(feature = "group-scope")]
    #[must_use]
    pub fn has_group_permission(&self, group_id: &str, min_role: GroupRole) -> bool {
        let Some(role) = self.get_group_role(group_id) else {
            return false;
        };
        match min_role {
            GroupRole::Admin => role.can_manage(),
            GroupRole::Write => role.can_write(),
            GroupRole::Read => role.can_read(),
        }
    }

    /// Requires at least the specified role in a group.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group identifier.
    /// * `min_role` - The minimum required role.
    ///
    /// # Errors
    ///
    /// Returns `Error::Unauthorized` if the user doesn't have the required role.
    #[cfg(feature = "group-scope")]
    pub fn require_group_role(&self, group_id: &str, min_role: GroupRole) -> Result<()> {
        if self.has_group_permission(group_id, min_role) {
            tracing::debug!(
                subject = ?self.subject,
                group_id = group_id,
                required_role = min_role.as_str(),
                is_local = self.is_local,
                "Group authorization granted"
            );
            Ok(())
        } else {
            tracing::warn!(
                subject = ?self.subject,
                group_id = group_id,
                required_role = min_role.as_str(),
                actual_role = ?self.get_group_role(group_id),
                "Group authorization denied"
            );
            Err(Error::Unauthorized(format!(
                "Role '{}' required in group '{group_id}'",
                min_role.as_str()
            )))
        }
    }
}

/// Builder for constructing an [`AuthContext`].
#[derive(Debug, Default)]
pub struct AuthContextBuilder {
    subject: Option<String>,
    scopes: HashSet<String>,
    is_local: bool,
    org_name: Option<String>,
    org_role: Option<String>,
    #[cfg(feature = "group-scope")]
    group_roles: HashMap<String, String>,
}

impl AuthContextBuilder {
    /// Sets the subject identifier.
    #[must_use]
    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// Adds a scope.
    #[must_use]
    pub fn scope(mut self, scope: impl Into<String>) -> Self {
        self.scopes.insert(scope.into());
        self
    }

    /// Adds multiple scopes.
    #[must_use]
    pub fn scopes(mut self, scopes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        for scope in scopes {
            self.scopes.insert(scope.into());
        }
        self
    }

    /// Marks this as a local context.
    #[must_use]
    pub const fn local(mut self) -> Self {
        self.is_local = true;
        self
    }

    /// Sets the organization name.
    #[must_use]
    pub fn org_name(mut self, name: impl Into<String>) -> Self {
        self.org_name = Some(name.into());
        self
    }

    /// Sets the organization role.
    #[must_use]
    pub fn org_role(mut self, role: impl Into<String>) -> Self {
        self.org_role = Some(role.into());
        self
    }

    /// Sets a group role for the user.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group identifier
    /// * `role` - The role in that group (admin, write, read)
    #[cfg(feature = "group-scope")]
    #[must_use]
    pub fn group_role(mut self, group_id: impl Into<String>, role: impl Into<String>) -> Self {
        self.group_roles.insert(group_id.into(), role.into());
        self
    }

    /// Builds the auth context.
    #[must_use]
    pub fn build(self) -> AuthContext {
        AuthContext {
            subject: self.subject,
            scopes: self.scopes,
            is_local: self.is_local,
            org_name: self.org_name,
            org_role: self.org_role,
            #[cfg(feature = "group-scope")]
            group_roles: self.group_roles,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_context_has_all_permissions() {
        let ctx = AuthContext::local();

        assert!(ctx.has_permission(Permission::Read));
        assert!(ctx.has_permission(Permission::Write));
        assert!(ctx.has_permission(Permission::Admin));
        assert!(ctx.require(Permission::Admin).is_ok());
    }

    #[test]
    fn test_default_is_local() {
        let ctx = AuthContext::default();
        assert!(ctx.is_local());
        assert!(ctx.has_permission(Permission::Admin));
    }

    #[test]
    fn test_from_scopes() {
        let ctx = AuthContext::from_scopes(vec!["read".to_string(), "write".to_string()]);

        assert!(ctx.has_permission(Permission::Read));
        assert!(ctx.has_permission(Permission::Write));
        assert!(!ctx.has_permission(Permission::Admin));
    }

    #[test]
    fn test_require_denied() {
        let ctx = AuthContext::from_scopes(vec!["read".to_string()]);

        assert!(ctx.require(Permission::Read).is_ok());
        assert!(ctx.require(Permission::Write).is_err());
    }

    #[test]
    fn test_wildcard_scope() {
        let ctx = AuthContext::from_scopes(vec!["*".to_string()]);

        assert!(ctx.has_permission(Permission::Read));
        assert!(ctx.has_permission(Permission::Write));
        assert!(ctx.has_permission(Permission::Admin));
    }

    #[test]
    fn test_with_subject() {
        let ctx = AuthContext::from_scopes(vec!["read".to_string()]).with_subject("user-123");

        assert_eq!(ctx.subject(), Some("user-123"));
    }

    #[test]
    fn test_builder() {
        let ctx = AuthContext::builder()
            .subject("test-user")
            .scope("read")
            .scope("write")
            .build();

        assert_eq!(ctx.subject(), Some("test-user"));
        assert!(ctx.has_permission(Permission::Read));
        assert!(ctx.has_permission(Permission::Write));
        assert!(!ctx.has_permission(Permission::Admin));
    }

    #[test]
    fn test_builder_scopes() {
        let ctx = AuthContext::builder().scopes(vec!["read", "admin"]).build();

        assert!(ctx.has_permission(Permission::Read));
        assert!(!ctx.has_permission(Permission::Write));
        assert!(ctx.has_permission(Permission::Admin));
    }

    #[test]
    fn test_has_any_permission() {
        let ctx = AuthContext::from_scopes(vec!["read".to_string()]);

        assert!(ctx.has_any_permission(&[Permission::Read, Permission::Write]));
        assert!(!ctx.has_any_permission(&[Permission::Write, Permission::Admin]));
    }

    #[test]
    fn test_require_any() {
        let ctx = AuthContext::from_scopes(vec!["read".to_string()]);

        assert!(
            ctx.require_any(&[Permission::Read, Permission::Write])
                .is_ok()
        );
        assert!(
            ctx.require_any(&[Permission::Write, Permission::Admin])
                .is_err()
        );
    }

    #[test]
    fn test_permission_parse() {
        assert_eq!(Permission::parse("read"), Some(Permission::Read));
        assert_eq!(Permission::parse("WRITE"), Some(Permission::Write));
        assert_eq!(Permission::parse("Admin"), Some(Permission::Admin));
        assert_eq!(Permission::parse("unknown"), None);
    }

    #[test]
    fn test_permission_as_str() {
        assert_eq!(Permission::Read.as_str(), "read");
        assert_eq!(Permission::Write.as_str(), "write");
        assert_eq!(Permission::Admin.as_str(), "admin");
    }

    // Group permission tests (only compiled with group-scope feature)

    #[test]
    #[cfg(feature = "group-scope")]
    fn test_local_context_has_admin_group_role() {
        use crate::models::group::GroupRole;

        let ctx = AuthContext::local();

        assert_eq!(ctx.get_group_role("any-group"), Some(GroupRole::Admin));
        assert!(ctx.has_group_permission("any-group", GroupRole::Admin));
        assert!(ctx.has_group_permission("any-group", GroupRole::Write));
        assert!(ctx.has_group_permission("any-group", GroupRole::Read));
    }

    #[test]
    #[cfg(feature = "group-scope")]
    fn test_wildcard_scope_has_admin_group_role() {
        use crate::models::group::GroupRole;

        let ctx = AuthContext::from_scopes(vec!["*".to_string()]);

        assert_eq!(ctx.get_group_role("any-group"), Some(GroupRole::Admin));
        assert!(ctx.has_group_permission("any-group", GroupRole::Admin));
    }

    #[test]
    #[cfg(feature = "group-scope")]
    fn test_builder_with_group_role() {
        use crate::models::group::GroupRole;

        let ctx = AuthContext::builder()
            .subject("test-user")
            .group_role("group-123", "write")
            .build();

        assert_eq!(ctx.get_group_role("group-123"), Some(GroupRole::Write));
        assert!(ctx.has_group_permission("group-123", GroupRole::Write));
        assert!(ctx.has_group_permission("group-123", GroupRole::Read));
        assert!(!ctx.has_group_permission("group-123", GroupRole::Admin));
    }

    #[test]
    #[cfg(feature = "group-scope")]
    fn test_group_role_not_found() {
        use crate::models::group::GroupRole;

        let ctx = AuthContext::builder()
            .subject("test-user")
            .group_role("group-123", "read")
            .build();

        // Different group ID returns None
        assert_eq!(ctx.get_group_role("group-456"), None);
        assert!(!ctx.has_group_permission("group-456", GroupRole::Read));
    }

    #[test]
    #[cfg(feature = "group-scope")]
    fn test_require_group_role_success() {
        use crate::models::group::GroupRole;

        let ctx = AuthContext::builder()
            .group_role("group-123", "admin")
            .build();

        assert!(
            ctx.require_group_role("group-123", GroupRole::Admin)
                .is_ok()
        );
        assert!(
            ctx.require_group_role("group-123", GroupRole::Write)
                .is_ok()
        );
        assert!(ctx.require_group_role("group-123", GroupRole::Read).is_ok());
    }

    #[test]
    #[cfg(feature = "group-scope")]
    fn test_require_group_role_denied() {
        use crate::models::group::GroupRole;

        let ctx = AuthContext::builder()
            .group_role("group-123", "read")
            .build();

        assert!(ctx.require_group_role("group-123", GroupRole::Read).is_ok());
        assert!(
            ctx.require_group_role("group-123", GroupRole::Write)
                .is_err()
        );
        assert!(
            ctx.require_group_role("group-123", GroupRole::Admin)
                .is_err()
        );
    }

    #[test]
    #[cfg(feature = "group-scope")]
    fn test_require_group_role_not_member() {
        use crate::models::group::GroupRole;

        let ctx = AuthContext::builder().subject("test-user").build();

        // No group roles set, should fail
        assert!(
            ctx.require_group_role("group-123", GroupRole::Read)
                .is_err()
        );
    }

    #[test]
    #[cfg(feature = "group-scope")]
    fn test_multiple_group_roles() {
        use crate::models::group::GroupRole;

        let ctx = AuthContext::builder()
            .subject("test-user")
            .group_role("group-1", "admin")
            .group_role("group-2", "write")
            .group_role("group-3", "read")
            .build();

        assert_eq!(ctx.get_group_role("group-1"), Some(GroupRole::Admin));
        assert_eq!(ctx.get_group_role("group-2"), Some(GroupRole::Write));
        assert_eq!(ctx.get_group_role("group-3"), Some(GroupRole::Read));

        // Check permissions hierarchy
        assert!(ctx.has_group_permission("group-1", GroupRole::Admin));
        assert!(ctx.has_group_permission("group-2", GroupRole::Write));
        assert!(!ctx.has_group_permission("group-2", GroupRole::Admin));
        assert!(ctx.has_group_permission("group-3", GroupRole::Read));
        assert!(!ctx.has_group_permission("group-3", GroupRole::Write));
    }
}
