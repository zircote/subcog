//! Role-Based Access Control (RBAC) Foundation.
//!
//! Provides separation of duties through role-based permissions for SOC2 compliance.
//!
//! # Overview
//!
//! This module implements the foundation for RBAC with:
//! - Pre-defined roles with appropriate permission sets
//! - Fine-grained permissions for all operations
//! - Permission checking and enforcement
//! - Audit integration for access control events
//!
//! # Roles
//!
//! | Role | Description | Key Permissions |
//! |------|-------------|-----------------|
//! | `Admin` | Full system access | All permissions |
//! | `Operator` | Day-to-day operations | Capture, Recall, Sync, Configure |
//! | `User` | Standard user access | Capture, Recall |
//! | `Auditor` | Read-only audit access | `ViewAudit`, `GenerateReports` |
//! | `ReadOnly` | Read-only data access | Recall only |
//!
//! # Example
//!
//! ```rust
//! use subcog::security::rbac::{Role, Permission, AccessControl};
//!
//! let ac = AccessControl::new();
//!
//! // Check if a role has a permission
//! assert!(ac.has_permission(&Role::Admin, &Permission::Delete));
//! assert!(!ac.has_permission(&Role::ReadOnly, &Permission::Delete));
//!
//! // Get all permissions for a role
//! let user_perms = ac.permissions_for(&Role::User);
//! assert!(user_perms.contains(&Permission::Capture));
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// System roles with predefined permission sets.
///
/// Roles implement the principle of least privilege, giving each role
/// only the permissions necessary for its function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Full system administrator with all permissions.
    Admin,
    /// Operations role for day-to-day management.
    Operator,
    /// Standard user with basic capture and recall.
    User,
    /// Audit role with read-only access to audit logs and reports.
    Auditor,
    /// Read-only access to data (no capture or modification).
    ReadOnly,
}

impl Role {
    /// Returns all available roles.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Admin,
            Self::Operator,
            Self::User,
            Self::Auditor,
            Self::ReadOnly,
        ]
    }

    /// Returns the display name for the role.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Admin => "Administrator",
            Self::Operator => "Operator",
            Self::User => "User",
            Self::Auditor => "Auditor",
            Self::ReadOnly => "Read-Only",
        }
    }

    /// Returns a description of the role's purpose.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Admin => "Full system access with all permissions",
            Self::Operator => "Day-to-day operations and configuration",
            Self::User => "Standard user access for capture and recall",
            Self::Auditor => "Read-only access to audit logs and reports",
            Self::ReadOnly => "Read-only access to data without modification",
        }
    }
}

/// Fine-grained permissions for system operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    // Memory operations
    /// Capture new memories.
    Capture,
    /// Recall/search memories.
    Recall,
    /// Delete memories.
    Delete,
    /// Consolidate memories.
    Consolidate,

    // Sync operations
    /// Sync memories with remote.
    Sync,
    /// Push changes to remote.
    Push,
    /// Pull changes from remote.
    Pull,

    // Configuration
    /// Modify system configuration.
    Configure,
    /// Manage feature flags.
    ManageFeatures,

    // User management
    /// Manage user accounts.
    ManageUsers,
    /// Assign roles to users.
    AssignRoles,

    // Audit and compliance
    /// View audit logs.
    ViewAudit,
    /// Generate compliance reports.
    GenerateReports,
    /// Export audit data.
    ExportAudit,

    // Data subject rights (GDPR)
    /// Export user data (GDPR Article 15).
    ExportData,
    /// Delete user data (GDPR Article 17).
    DeleteUserData,
    /// Manage consent records.
    ManageConsent,

    // Prompt management
    /// Create and save prompts.
    CreatePrompt,
    /// Execute prompts.
    RunPrompt,
    /// Delete prompts.
    DeletePrompt,

    // System administration
    /// Access system health and metrics.
    ViewHealth,
    /// Manage encryption keys.
    ManageEncryption,
    /// Perform maintenance operations.
    Maintenance,
}

impl Permission {
    /// Returns all available permissions.
    #[must_use]
    pub fn all() -> Vec<Self> {
        vec![
            Self::Capture,
            Self::Recall,
            Self::Delete,
            Self::Consolidate,
            Self::Sync,
            Self::Push,
            Self::Pull,
            Self::Configure,
            Self::ManageFeatures,
            Self::ManageUsers,
            Self::AssignRoles,
            Self::ViewAudit,
            Self::GenerateReports,
            Self::ExportAudit,
            Self::ExportData,
            Self::DeleteUserData,
            Self::ManageConsent,
            Self::CreatePrompt,
            Self::RunPrompt,
            Self::DeletePrompt,
            Self::ViewHealth,
            Self::ManageEncryption,
            Self::Maintenance,
        ]
    }

    /// Returns the display name for the permission.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Capture => "Capture Memories",
            Self::Recall => "Recall Memories",
            Self::Delete => "Delete Memories",
            Self::Consolidate => "Consolidate Memories",
            Self::Sync => "Sync",
            Self::Push => "Push to Remote",
            Self::Pull => "Pull from Remote",
            Self::Configure => "Configure System",
            Self::ManageFeatures => "Manage Features",
            Self::ManageUsers => "Manage Users",
            Self::AssignRoles => "Assign Roles",
            Self::ViewAudit => "View Audit Logs",
            Self::GenerateReports => "Generate Reports",
            Self::ExportAudit => "Export Audit Data",
            Self::ExportData => "Export User Data",
            Self::DeleteUserData => "Delete User Data",
            Self::ManageConsent => "Manage Consent",
            Self::CreatePrompt => "Create Prompts",
            Self::RunPrompt => "Run Prompts",
            Self::DeletePrompt => "Delete Prompts",
            Self::ViewHealth => "View Health",
            Self::ManageEncryption => "Manage Encryption",
            Self::Maintenance => "Maintenance",
        }
    }

    /// Returns the category of this permission.
    #[must_use]
    pub const fn category(&self) -> PermissionCategory {
        match self {
            Self::Capture | Self::Recall | Self::Delete | Self::Consolidate => {
                PermissionCategory::Memory
            },
            Self::Sync | Self::Push | Self::Pull => PermissionCategory::Sync,
            Self::Configure | Self::ManageFeatures => PermissionCategory::Configuration,
            Self::ManageUsers | Self::AssignRoles => PermissionCategory::UserManagement,
            Self::ViewAudit | Self::GenerateReports | Self::ExportAudit => {
                PermissionCategory::Audit
            },
            Self::ExportData | Self::DeleteUserData | Self::ManageConsent => {
                PermissionCategory::DataSubject
            },
            Self::CreatePrompt | Self::RunPrompt | Self::DeletePrompt => {
                PermissionCategory::Prompts
            },
            Self::ViewHealth | Self::ManageEncryption | Self::Maintenance => {
                PermissionCategory::System
            },
        }
    }
}

/// Categories of permissions for grouping and display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionCategory {
    /// Memory operations (capture, recall, delete).
    Memory,
    /// Sync operations (push, pull).
    Sync,
    /// System configuration.
    Configuration,
    /// User and role management.
    UserManagement,
    /// Audit and compliance.
    Audit,
    /// Data subject rights (GDPR).
    DataSubject,
    /// Prompt management.
    Prompts,
    /// System administration.
    System,
}

impl PermissionCategory {
    /// Returns the display name for the category.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Memory => "Memory Operations",
            Self::Sync => "Synchronization",
            Self::Configuration => "Configuration",
            Self::UserManagement => "User Management",
            Self::Audit => "Audit & Compliance",
            Self::DataSubject => "Data Subject Rights",
            Self::Prompts => "Prompt Management",
            Self::System => "System Administration",
        }
    }
}

/// Result of an access control check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessResult {
    /// Access granted.
    Granted,
    /// Access denied with reason.
    Denied(String),
}

impl AccessResult {
    /// Returns true if access was granted.
    #[must_use]
    pub const fn is_granted(&self) -> bool {
        matches!(self, Self::Granted)
    }

    /// Returns true if access was denied.
    #[must_use]
    pub const fn is_denied(&self) -> bool {
        matches!(self, Self::Denied(_))
    }
}

/// Access control manager for checking role permissions.
#[derive(Debug, Clone)]
pub struct AccessControl {
    /// Mapping of roles to their permissions.
    role_permissions: HashMap<Role, HashSet<Permission>>,
}

impl Default for AccessControl {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessControl {
    /// Creates a new access control instance with default role-permission mappings.
    #[must_use]
    pub fn new() -> Self {
        let mut role_permissions = HashMap::new();

        // Admin: All permissions
        role_permissions.insert(Role::Admin, Permission::all().into_iter().collect());

        // Operator: Day-to-day operations
        role_permissions.insert(
            Role::Operator,
            [
                Permission::Capture,
                Permission::Recall,
                Permission::Delete,
                Permission::Consolidate,
                Permission::Sync,
                Permission::Push,
                Permission::Pull,
                Permission::Configure,
                Permission::CreatePrompt,
                Permission::RunPrompt,
                Permission::DeletePrompt,
                Permission::ViewHealth,
            ]
            .into_iter()
            .collect(),
        );

        // User: Basic capture and recall
        role_permissions.insert(
            Role::User,
            [
                Permission::Capture,
                Permission::Recall,
                Permission::Sync,
                Permission::CreatePrompt,
                Permission::RunPrompt,
            ]
            .into_iter()
            .collect(),
        );

        // Auditor: Audit and reporting only
        role_permissions.insert(
            Role::Auditor,
            [
                Permission::Recall,
                Permission::ViewAudit,
                Permission::GenerateReports,
                Permission::ExportAudit,
                Permission::ViewHealth,
            ]
            .into_iter()
            .collect(),
        );

        // ReadOnly: Just recall
        role_permissions.insert(
            Role::ReadOnly,
            [Permission::Recall, Permission::RunPrompt]
                .into_iter()
                .collect(),
        );

        Self { role_permissions }
    }

    /// Checks if a role has a specific permission.
    #[must_use]
    pub fn has_permission(&self, role: &Role, permission: &Permission) -> bool {
        self.role_permissions
            .get(role)
            .is_some_and(|perms| perms.contains(permission))
    }

    /// Checks access and returns a detailed result.
    #[must_use]
    pub fn check_access(&self, role: &Role, permission: &Permission) -> AccessResult {
        if self.has_permission(role, permission) {
            AccessResult::Granted
        } else {
            AccessResult::Denied(format!(
                "Role '{}' does not have permission '{}'",
                role.display_name(),
                permission.display_name()
            ))
        }
    }

    /// Returns all permissions for a role.
    #[must_use]
    pub fn permissions_for(&self, role: &Role) -> HashSet<Permission> {
        self.role_permissions.get(role).cloned().unwrap_or_default()
    }

    /// Returns all roles that have a specific permission.
    #[must_use]
    pub fn roles_with_permission(&self, permission: &Permission) -> Vec<Role> {
        self.role_permissions
            .iter()
            .filter(|(_, perms)| perms.contains(permission))
            .map(|(role, _)| *role)
            .collect()
    }

    /// Adds a custom permission to a role.
    pub fn grant_permission(&mut self, role: &Role, permission: Permission) {
        self.role_permissions
            .entry(*role)
            .or_default()
            .insert(permission);
    }

    /// Removes a permission from a role.
    pub fn revoke_permission(&mut self, role: &Role, permission: &Permission) {
        if let Some(perms) = self.role_permissions.get_mut(role) {
            perms.remove(permission);
        }
    }

    /// Returns a summary of all role-permission mappings.
    #[must_use]
    pub fn summary(&self) -> RbacSummary {
        let role_summaries: Vec<RoleSummary> = Role::all()
            .iter()
            .map(|role| {
                let permissions = self.permissions_for(role);
                RoleSummary {
                    role: *role,
                    permission_count: permissions.len(),
                    permissions: permissions.into_iter().collect(),
                }
            })
            .collect();

        RbacSummary {
            total_roles: Role::all().len(),
            total_permissions: Permission::all().len(),
            role_summaries,
        }
    }
}

/// Summary of a role's permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleSummary {
    /// The role.
    pub role: Role,
    /// Number of permissions.
    pub permission_count: usize,
    /// List of permissions.
    pub permissions: Vec<Permission>,
}

/// Summary of the entire RBAC configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacSummary {
    /// Total number of roles.
    pub total_roles: usize,
    /// Total number of permissions.
    pub total_permissions: usize,
    /// Per-role summaries.
    pub role_summaries: Vec<RoleSummary>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_has_all_permissions() {
        let ac = AccessControl::new();
        for permission in Permission::all() {
            assert!(
                ac.has_permission(&Role::Admin, &permission),
                "Admin should have {permission:?}"
            );
        }
    }

    #[test]
    fn test_readonly_limited_permissions() {
        let ac = AccessControl::new();

        // ReadOnly should have recall
        assert!(ac.has_permission(&Role::ReadOnly, &Permission::Recall));
        assert!(ac.has_permission(&Role::ReadOnly, &Permission::RunPrompt));

        // ReadOnly should NOT have these
        assert!(!ac.has_permission(&Role::ReadOnly, &Permission::Capture));
        assert!(!ac.has_permission(&Role::ReadOnly, &Permission::Delete));
        assert!(!ac.has_permission(&Role::ReadOnly, &Permission::Configure));
    }

    #[test]
    fn test_auditor_audit_permissions() {
        let ac = AccessControl::new();

        // Auditor should have audit permissions
        assert!(ac.has_permission(&Role::Auditor, &Permission::ViewAudit));
        assert!(ac.has_permission(&Role::Auditor, &Permission::GenerateReports));
        assert!(ac.has_permission(&Role::Auditor, &Permission::ExportAudit));

        // Auditor should NOT have modification permissions
        assert!(!ac.has_permission(&Role::Auditor, &Permission::Capture));
        assert!(!ac.has_permission(&Role::Auditor, &Permission::Delete));
        assert!(!ac.has_permission(&Role::Auditor, &Permission::Configure));
    }

    #[test]
    fn test_user_basic_permissions() {
        let ac = AccessControl::new();

        // User should have basic permissions
        assert!(ac.has_permission(&Role::User, &Permission::Capture));
        assert!(ac.has_permission(&Role::User, &Permission::Recall));
        assert!(ac.has_permission(&Role::User, &Permission::Sync));

        // User should NOT have admin permissions
        assert!(!ac.has_permission(&Role::User, &Permission::ManageUsers));
        assert!(!ac.has_permission(&Role::User, &Permission::Configure));
        assert!(!ac.has_permission(&Role::User, &Permission::Delete));
    }

    #[test]
    fn test_operator_operations_permissions() {
        let ac = AccessControl::new();

        // Operator should have operational permissions
        assert!(ac.has_permission(&Role::Operator, &Permission::Capture));
        assert!(ac.has_permission(&Role::Operator, &Permission::Recall));
        assert!(ac.has_permission(&Role::Operator, &Permission::Delete));
        assert!(ac.has_permission(&Role::Operator, &Permission::Configure));

        // Operator should NOT have user management
        assert!(!ac.has_permission(&Role::Operator, &Permission::ManageUsers));
        assert!(!ac.has_permission(&Role::Operator, &Permission::AssignRoles));
    }

    #[test]
    fn test_check_access_granted() {
        let ac = AccessControl::new();
        let result = ac.check_access(&Role::Admin, &Permission::Delete);
        assert!(result.is_granted());
    }

    #[test]
    fn test_check_access_denied() {
        let ac = AccessControl::new();
        let result = ac.check_access(&Role::ReadOnly, &Permission::Delete);
        assert!(result.is_denied());
        if let AccessResult::Denied(reason) = result {
            assert!(reason.contains("Read-Only"));
            assert!(reason.contains("Delete"));
        }
    }

    #[test]
    fn test_permissions_for_role() {
        let ac = AccessControl::new();
        let admin_perms = ac.permissions_for(&Role::Admin);
        assert_eq!(admin_perms.len(), Permission::all().len());

        let readonly_perms = ac.permissions_for(&Role::ReadOnly);
        assert!(readonly_perms.len() < admin_perms.len());
    }

    #[test]
    fn test_roles_with_permission() {
        let ac = AccessControl::new();

        // All roles except ReadOnly should have Recall
        let recall_roles = ac.roles_with_permission(&Permission::Recall);
        assert!(recall_roles.contains(&Role::Admin));
        assert!(recall_roles.contains(&Role::User));
        assert!(recall_roles.contains(&Role::Auditor));
        assert!(recall_roles.contains(&Role::ReadOnly));

        // Only Admin should have ManageUsers
        let manage_roles = ac.roles_with_permission(&Permission::ManageUsers);
        assert_eq!(manage_roles.len(), 1);
        assert!(manage_roles.contains(&Role::Admin));
    }

    #[test]
    fn test_grant_permission() {
        let mut ac = AccessControl::new();

        assert!(!ac.has_permission(&Role::ReadOnly, &Permission::Capture));
        ac.grant_permission(&Role::ReadOnly, Permission::Capture);
        assert!(ac.has_permission(&Role::ReadOnly, &Permission::Capture));
    }

    #[test]
    fn test_revoke_permission() {
        let mut ac = AccessControl::new();

        assert!(ac.has_permission(&Role::User, &Permission::Capture));
        ac.revoke_permission(&Role::User, &Permission::Capture);
        assert!(!ac.has_permission(&Role::User, &Permission::Capture));
    }

    #[test]
    fn test_permission_categories() {
        assert_eq!(Permission::Capture.category(), PermissionCategory::Memory);
        assert_eq!(Permission::Sync.category(), PermissionCategory::Sync);
        assert_eq!(
            Permission::Configure.category(),
            PermissionCategory::Configuration
        );
        assert_eq!(
            Permission::ManageUsers.category(),
            PermissionCategory::UserManagement
        );
        assert_eq!(Permission::ViewAudit.category(), PermissionCategory::Audit);
        assert_eq!(
            Permission::ExportData.category(),
            PermissionCategory::DataSubject
        );
        assert_eq!(
            Permission::CreatePrompt.category(),
            PermissionCategory::Prompts
        );
        assert_eq!(
            Permission::ViewHealth.category(),
            PermissionCategory::System
        );
    }

    #[test]
    fn test_role_display_names() {
        assert_eq!(Role::Admin.display_name(), "Administrator");
        assert_eq!(Role::ReadOnly.display_name(), "Read-Only");
    }

    #[test]
    fn test_permission_display_names() {
        assert_eq!(Permission::Capture.display_name(), "Capture Memories");
        assert_eq!(Permission::ViewAudit.display_name(), "View Audit Logs");
    }

    #[test]
    fn test_rbac_summary() {
        let ac = AccessControl::new();
        let summary = ac.summary();

        assert_eq!(summary.total_roles, 5);
        assert_eq!(summary.total_permissions, Permission::all().len());
        assert_eq!(summary.role_summaries.len(), 5);

        // Admin should have most permissions
        let admin_summary = summary
            .role_summaries
            .iter()
            .find(|s| s.role == Role::Admin)
            .unwrap();
        assert_eq!(admin_summary.permission_count, Permission::all().len());
    }

    #[test]
    fn test_role_serialization() {
        let role = Role::Admin;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"admin\"");

        let deserialized: Role = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Role::Admin);
    }

    #[test]
    fn test_permission_serialization() {
        let perm = Permission::Capture;
        let json = serde_json::to_string(&perm).unwrap();
        assert_eq!(json, "\"capture\"");

        let deserialized: Permission = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Permission::Capture);
    }

    #[test]
    fn test_access_result_methods() {
        let granted = AccessResult::Granted;
        assert!(granted.is_granted());
        assert!(!granted.is_denied());

        let denied = AccessResult::Denied("test".to_string());
        assert!(!denied.is_granted());
        assert!(denied.is_denied());
    }

    #[test]
    fn test_separation_of_duties() {
        let ac = AccessControl::new();

        // Auditor should not be able to modify data they audit
        assert!(ac.has_permission(&Role::Auditor, &Permission::ViewAudit));
        assert!(!ac.has_permission(&Role::Auditor, &Permission::Capture));
        assert!(!ac.has_permission(&Role::Auditor, &Permission::Delete));

        // User should not be able to manage users
        assert!(!ac.has_permission(&Role::User, &Permission::ManageUsers));
        assert!(!ac.has_permission(&Role::User, &Permission::AssignRoles));

        // Operator should not manage users (separation from admin)
        assert!(!ac.has_permission(&Role::Operator, &Permission::ManageUsers));
    }
}
