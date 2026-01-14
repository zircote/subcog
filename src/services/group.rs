//! Group management service.
//!
//! Provides business logic for group operations within an organization.
//! Groups enable team collaboration through shared memory graphs.
//!
//! # Features
//!
//! - **Group Management**: Create, list, and delete groups
//! - **Member Management**: Add/remove members with role-based permissions
//! - **Invite System**: Token-based invites with expiration and usage limits
//!
//! # Permissions
//!
//! | Operation | Required Role |
//! |-----------|--------------|
//! | Create group | Org member |
//! | Delete group | Group admin |
//! | Add member | Group admin |
//! | Remove member | Group admin (cannot remove last admin) |
//! | Update role | Group admin (cannot demote last admin) |
//! | Create invite | Group admin |
//! | List members | Group member |
//! | Join via invite | Anyone with valid token |
//! | Leave group | Self (cannot leave if last admin) |
//!
//! # Example
//!
//! ```rust,ignore
//! use subcog::services::GroupService;
//! use subcog::storage::GroupStorageFactory;
//!
//! let backend = GroupStorageFactory::create_in_memory()?;
//! let service = GroupService::new(backend);
//!
//! // Create a group
//! let group = service.create_group("my-org", "engineering", "Engineering team", "alice@example.com")?;
//!
//! // Add a member
//! service.add_member(&group.id, "bob@example.com", GroupRole::Write, "alice@example.com")?;
//!
//! // Create an invite
//! let (invite, token) = service.create_invite(&group.id, GroupRole::Read, "alice@example.com", None, None)?;
//! println!("Share this invite: {token}");
//! ```

use std::sync::Arc;

use crate::models::group::{Group, GroupId, GroupInvite, GroupMember, GroupMembership, GroupRole};
use crate::storage::group::GroupBackend;
use crate::{Error, Result};

/// Service for group management operations.
///
/// Encapsulates business logic for groups, members, and invites.
/// Uses a [`GroupBackend`] for persistence.
pub struct GroupService {
    backend: Arc<dyn GroupBackend>,
}

impl GroupService {
    /// Creates a new group service with the given backend.
    #[must_use]
    pub fn new(backend: Arc<dyn GroupBackend>) -> Self {
        Self { backend }
    }

    /// Creates a new group service with a default `SQLite` backend.
    ///
    /// Uses the user's data directory for storage.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend cannot be initialized.
    pub fn try_default() -> crate::Result<Self> {
        use crate::services::PathManager;
        use crate::storage::group::SqliteGroupBackend;

        let user_dir = crate::storage::get_user_data_dir()?;
        let paths = PathManager::for_user(&user_dir);
        let db_path = paths.index_path().join("groups.db");
        let backend = Arc::new(SqliteGroupBackend::new(&db_path)?);
        Ok(Self::new(backend))
    }

    // =========================================================================
    // Group Operations
    // =========================================================================

    /// Creates a new group in the organization.
    ///
    /// The creator is automatically added as an admin.
    ///
    /// # Arguments
    ///
    /// * `org_id` - Organization identifier
    /// * `name` - Group name (must be unique within org)
    /// * `description` - Optional description
    /// * `creator_email` - Email of the group creator
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A group with the same name already exists
    /// - Storage cannot be accessed
    pub fn create_group(
        &self,
        org_id: &str,
        name: &str,
        description: &str,
        creator_email: &str,
    ) -> Result<Group> {
        // Validate inputs
        if name.is_empty() {
            return Err(Error::InvalidInput(
                "Group name cannot be empty".to_string(),
            ));
        }
        if creator_email.is_empty() {
            return Err(Error::InvalidInput(
                "Creator email cannot be empty".to_string(),
            ));
        }

        // Create the group
        let group = self
            .backend
            .create_group(org_id, name, description, creator_email)?;

        // Add creator as admin
        self.backend
            .add_member(&group.id, creator_email, GroupRole::Admin, creator_email)?;

        tracing::info!(
            org_id = %org_id,
            group_id = %group.id.as_str(),
            group_name = %name,
            creator = %creator_email,
            "Group created"
        );

        Ok(group)
    }

    /// Gets a group by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    pub fn get_group(&self, group_id: &GroupId) -> Result<Option<Group>> {
        self.backend.get_group(group_id)
    }

    /// Gets a group by name within an organization.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    pub fn get_group_by_name(&self, org_id: &str, name: &str) -> Result<Option<Group>> {
        self.backend.get_group_by_name(org_id, name)
    }

    /// Lists all groups in an organization.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    pub fn list_groups(&self, org_id: &str) -> Result<Vec<Group>> {
        self.backend.list_groups(org_id)
    }

    /// Deletes a group and all its members and invites.
    ///
    /// Only admins can delete groups.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group to delete
    /// * `requester_email` - Email of the user requesting deletion
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The requester is not an admin
    /// - Storage cannot be accessed
    pub fn delete_group(&self, group_id: &GroupId, requester_email: &str) -> Result<bool> {
        // Check requester is admin
        self.require_admin(group_id, requester_email)?;

        let deleted = self.backend.delete_group(group_id)?;

        if deleted {
            tracing::info!(
                group_id = %group_id.as_str(),
                requester = %requester_email,
                "Group deleted"
            );
        }

        Ok(deleted)
    }

    // =========================================================================
    // Member Operations
    // =========================================================================

    /// Adds a member to a group.
    ///
    /// If the member already exists, updates their role.
    /// Only admins can add members.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group to add to
    /// * `email` - Email of the new member
    /// * `role` - Role to assign
    /// * `requester_email` - Email of the user adding the member
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The requester is not an admin
    /// - The group doesn't exist
    /// - Storage cannot be accessed
    pub fn add_member(
        &self,
        group_id: &GroupId,
        email: &str,
        role: GroupRole,
        requester_email: &str,
    ) -> Result<GroupMember> {
        // Check requester is admin
        self.require_admin(group_id, requester_email)?;

        let member = self
            .backend
            .add_member(group_id, email, role, requester_email)?;

        tracing::info!(
            group_id = %group_id.as_str(),
            member_email = %email,
            role = %role.as_str(),
            added_by = %requester_email,
            "Member added to group"
        );

        Ok(member)
    }

    /// Gets a member's record in a group.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    pub fn get_member(&self, group_id: &GroupId, email: &str) -> Result<Option<GroupMember>> {
        self.backend.get_member(group_id, email)
    }

    /// Updates a member's role in a group.
    ///
    /// Only admins can update roles. Cannot demote the last admin.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group
    /// * `email` - The member's email
    /// * `new_role` - The new role to assign
    /// * `requester_email` - Email of the user updating the role
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The requester is not an admin
    /// - Demoting the last admin
    /// - Storage cannot be accessed
    pub fn update_member_role(
        &self,
        group_id: &GroupId,
        email: &str,
        new_role: GroupRole,
        requester_email: &str,
    ) -> Result<bool> {
        // Check requester is admin
        self.require_admin(group_id, requester_email)?;

        // Check if demoting an admin
        if let Some(member) = self.backend.get_member(group_id, email)?
            && member.role == GroupRole::Admin
            && new_role != GroupRole::Admin
        {
            // Check if this is the last admin
            let admin_count = self.backend.count_admins(group_id)?;
            if admin_count <= 1 {
                return Err(Error::InvalidInput(
                    "Cannot demote the last admin. Promote another member first.".to_string(),
                ));
            }
        }

        let updated = self.backend.update_member_role(group_id, email, new_role)?;

        if updated {
            tracing::info!(
                group_id = %group_id.as_str(),
                member_email = %email,
                new_role = %new_role.as_str(),
                updated_by = %requester_email,
                "Member role updated"
            );
        }

        Ok(updated)
    }

    /// Removes a member from a group.
    ///
    /// Only admins can remove members. Cannot remove the last admin.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group
    /// * `email` - The member's email
    /// * `requester_email` - Email of the user removing the member
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The requester is not an admin
    /// - Removing the last admin
    /// - Storage cannot be accessed
    pub fn remove_member(
        &self,
        group_id: &GroupId,
        email: &str,
        requester_email: &str,
    ) -> Result<bool> {
        // Check requester is admin
        self.require_admin(group_id, requester_email)?;

        // Check if removing an admin
        if let Some(member) = self.backend.get_member(group_id, email)?
            && member.role == GroupRole::Admin
        {
            // Check if this is the last admin
            let admin_count = self.backend.count_admins(group_id)?;
            if admin_count <= 1 {
                return Err(Error::InvalidInput(
                    "Cannot remove the last admin. Promote another member first.".to_string(),
                ));
            }
        }

        let removed = self.backend.remove_member(group_id, email)?;

        if removed {
            tracing::info!(
                group_id = %group_id.as_str(),
                member_email = %email,
                removed_by = %requester_email,
                "Member removed from group"
            );
        }

        Ok(removed)
    }

    /// Lists all members of a group.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    pub fn list_members(&self, group_id: &GroupId) -> Result<Vec<GroupMember>> {
        self.backend.list_members(group_id)
    }

    /// Gets all groups a user is a member of.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    pub fn get_user_groups(&self, org_id: &str, email: &str) -> Result<Vec<GroupMembership>> {
        self.backend.get_user_groups(org_id, email)
    }

    /// Allows a user to leave a group.
    ///
    /// Cannot leave if the user is the last admin.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group to leave
    /// * `email` - Email of the user leaving
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The user is the last admin
    /// - Storage cannot be accessed
    pub fn leave_group(&self, group_id: &GroupId, email: &str) -> Result<bool> {
        // Check if leaving as the last admin
        if let Some(member) = self.backend.get_member(group_id, email)?
            && member.role == GroupRole::Admin
        {
            let admin_count = self.backend.count_admins(group_id)?;
            if admin_count <= 1 {
                return Err(Error::InvalidInput(
                    "Cannot leave as the last admin. Promote another member first.".to_string(),
                ));
            }
        }

        let left = self.backend.remove_member(group_id, email)?;

        if left {
            tracing::info!(
                group_id = %group_id.as_str(),
                member_email = %email,
                "Member left group"
            );
        }

        Ok(left)
    }

    // =========================================================================
    // Invite Operations
    // =========================================================================

    /// Creates an invite for a group.
    ///
    /// Only admins can create invites.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group to invite to
    /// * `role` - Role to assign when joined
    /// * `creator_email` - Email of the admin creating the invite
    /// * `expires_in_secs` - How long until expiration (default: 7 days)
    /// * `max_uses` - Maximum number of uses (default: unlimited)
    ///
    /// # Returns
    ///
    /// A tuple of (invite, `plaintext_token`). The token should be shared
    /// with invitees and never stored.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The creator is not an admin
    /// - Storage cannot be accessed
    pub fn create_invite(
        &self,
        group_id: &GroupId,
        role: GroupRole,
        creator_email: &str,
        expires_in_secs: Option<u64>,
        max_uses: Option<u32>,
    ) -> Result<(GroupInvite, String)> {
        // Check creator is admin
        self.require_admin(group_id, creator_email)?;

        let (invite, token) =
            self.backend
                .create_invite(group_id, role, creator_email, expires_in_secs, max_uses)?;

        tracing::info!(
            group_id = %group_id.as_str(),
            invite_id = %invite.id,
            role = %role.as_str(),
            created_by = %creator_email,
            expires_in_secs = ?expires_in_secs,
            max_uses = ?max_uses,
            "Group invite created"
        );

        Ok((invite, token))
    }

    /// Joins a group using an invite token.
    ///
    /// Validates the token and adds the user as a member with the invite's role.
    ///
    /// # Arguments
    ///
    /// * `token` - The plaintext invite token
    /// * `email` - Email of the user joining
    ///
    /// # Returns
    ///
    /// The created member record.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The token is invalid or expired
    /// - Storage cannot be accessed
    pub fn join_via_invite(&self, token: &str, email: &str) -> Result<GroupMember> {
        // Look up invite by token hash
        let token_hash = GroupInvite::hash_token(token);
        let invite = self
            .backend
            .get_invite_by_token_hash(&token_hash)?
            .ok_or_else(|| Error::InvalidInput("Invalid invite token".to_string()))?;

        // Check if invite is valid
        if !invite.is_valid() {
            return Err(Error::InvalidInput(
                "Invite is expired or has reached its usage limit".to_string(),
            ));
        }

        // Check if user is already a member
        if let Some(existing) = self.backend.get_member(&invite.group_id, email)? {
            return Err(Error::InvalidInput(format!(
                "Already a member of this group with role '{}'",
                existing.role.as_str()
            )));
        }

        // Add member with invite's role
        let member = self.backend.add_member(
            &invite.group_id,
            email,
            invite.role,
            &invite.created_by, // "added by" is the invite creator
        )?;

        // Increment invite usage
        self.backend.increment_invite_uses(&invite.id)?;

        tracing::info!(
            group_id = %invite.group_id.as_str(),
            invite_id = %invite.id,
            member_email = %email,
            role = %invite.role.as_str(),
            "Member joined via invite"
        );

        Ok(member)
    }

    /// Lists all invites for a group.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group
    /// * `include_expired` - Whether to include expired/revoked invites
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    pub fn list_invites(
        &self,
        group_id: &GroupId,
        include_expired: bool,
    ) -> Result<Vec<GroupInvite>> {
        self.backend.list_invites(group_id, include_expired)
    }

    /// Revokes an invite.
    ///
    /// Only admins can revoke invites.
    ///
    /// # Arguments
    ///
    /// * `invite_id` - The invite to revoke
    /// * `requester_email` - Email of the admin revoking
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The requester is not an admin
    /// - Storage cannot be accessed
    pub fn revoke_invite(&self, invite_id: &str, requester_email: &str) -> Result<bool> {
        // Get the invite to find the group
        let invite = self
            .backend
            .get_invite(invite_id)?
            .ok_or_else(|| Error::InvalidInput("Invite not found".to_string()))?;

        // Check requester is admin
        self.require_admin(&invite.group_id, requester_email)?;

        let revoked = self.backend.revoke_invite(invite_id)?;

        if revoked {
            tracing::info!(
                invite_id = %invite_id,
                group_id = %invite.group_id.as_str(),
                revoked_by = %requester_email,
                "Group invite revoked"
            );
        }

        Ok(revoked)
    }

    /// Cleans up expired invites.
    ///
    /// # Returns
    ///
    /// Number of invites deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    pub fn cleanup_expired_invites(&self) -> Result<u64> {
        self.backend.cleanup_expired_invites()
    }

    // =========================================================================
    // Permission Helpers
    // =========================================================================

    /// Checks if a user has admin role in a group.
    fn require_admin(&self, group_id: &GroupId, email: &str) -> Result<()> {
        let member = self.backend.get_member(group_id, email)?.ok_or_else(|| {
            Error::Unauthorized(format!("User '{email}' is not a member of this group"))
        })?;

        if member.role != GroupRole::Admin {
            return Err(Error::Unauthorized(
                "Admin role required for this operation".to_string(),
            ));
        }

        Ok(())
    }

    /// Checks if a user has at least the specified role in a group.
    ///
    /// Role hierarchy: Admin > Write > Read
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The user is not a member of the group
    /// - The user's role is insufficient
    pub fn require_role(&self, group_id: &GroupId, email: &str, min_role: GroupRole) -> Result<()> {
        let member = self.backend.get_member(group_id, email)?.ok_or_else(|| {
            Error::Unauthorized(format!("User '{email}' is not a member of this group"))
        })?;

        // Check if member's role meets the minimum requirement
        let has_permission = match min_role {
            GroupRole::Admin => member.role.can_manage(),
            GroupRole::Write => member.role.can_write(),
            GroupRole::Read => member.role.can_read(),
        };

        if !has_permission {
            return Err(Error::Unauthorized(format!(
                "Role '{}' or higher required, but user has '{}'",
                min_role.as_str(),
                member.role.as_str()
            )));
        }

        Ok(())
    }

    /// Checks if a user is a member of a group.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    pub fn is_member(&self, group_id: &GroupId, email: &str) -> Result<bool> {
        Ok(self.backend.get_member(group_id, email)?.is_some())
    }

    /// Gets a user's role in a group, if they are a member.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    pub fn get_user_role(&self, group_id: &GroupId, email: &str) -> Result<Option<GroupRole>> {
        Ok(self.backend.get_member(group_id, email)?.map(|m| m.role))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::group::GroupStorageFactory;

    fn create_test_service() -> GroupService {
        let backend = GroupStorageFactory::create_in_memory().expect("Failed to create backend");
        GroupService::new(backend)
    }

    #[test]
    fn test_create_group_adds_creator_as_admin() {
        let service = create_test_service();

        let group = service
            .create_group("test-org", "engineering", "Eng team", "alice@example.com")
            .expect("Failed to create group");

        // Verify creator is admin
        let member = service
            .get_member(&group.id, "alice@example.com")
            .expect("Failed to get member")
            .expect("Member not found");

        assert_eq!(member.role, GroupRole::Admin);
    }

    #[test]
    fn test_add_member_requires_admin() {
        let service = create_test_service();

        // Create group (alice is admin)
        let group = service
            .create_group("test-org", "engineering", "", "alice@example.com")
            .expect("Failed to create group");

        // Add bob as a writer
        service
            .add_member(
                &group.id,
                "bob@example.com",
                GroupRole::Write,
                "alice@example.com",
            )
            .expect("Failed to add member");

        // Bob cannot add members
        let result = service.add_member(
            &group.id,
            "charlie@example.com",
            GroupRole::Read,
            "bob@example.com",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Admin role"));
    }

    #[test]
    fn test_cannot_remove_last_admin() {
        let service = create_test_service();

        let group = service
            .create_group("test-org", "engineering", "", "alice@example.com")
            .expect("Failed to create group");

        // Cannot remove the only admin
        let result = service.remove_member(&group.id, "alice@example.com", "alice@example.com");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("last admin"));
    }

    #[test]
    fn test_cannot_demote_last_admin() {
        let service = create_test_service();

        let group = service
            .create_group("test-org", "engineering", "", "alice@example.com")
            .expect("Failed to create group");

        // Cannot demote the only admin
        let result = service.update_member_role(
            &group.id,
            "alice@example.com",
            GroupRole::Write,
            "alice@example.com",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("last admin"));
    }

    #[test]
    fn test_invite_workflow() {
        let service = create_test_service();

        let group = service
            .create_group("test-org", "engineering", "", "alice@example.com")
            .expect("Failed to create group");

        // Create invite
        let (invite, token) = service
            .create_invite(
                &group.id,
                GroupRole::Write,
                "alice@example.com",
                Some(3600),
                Some(5),
            )
            .expect("Failed to create invite");

        assert_eq!(invite.role, GroupRole::Write);
        assert!(!token.is_empty());

        // Join via invite
        let member = service
            .join_via_invite(&token, "bob@example.com")
            .expect("Failed to join");

        assert_eq!(member.role, GroupRole::Write);

        // Cannot join again
        let result = service.join_via_invite(&token, "bob@example.com");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Already a member"));
    }

    #[test]
    fn test_leave_group() {
        let service = create_test_service();

        let group = service
            .create_group("test-org", "engineering", "", "alice@example.com")
            .expect("Failed to create group");

        // Add bob
        service
            .add_member(
                &group.id,
                "bob@example.com",
                GroupRole::Write,
                "alice@example.com",
            )
            .expect("Failed to add member");

        // Bob can leave
        assert!(
            service
                .leave_group(&group.id, "bob@example.com")
                .expect("Failed to leave")
        );

        // Verify bob is no longer a member
        assert!(
            !service
                .is_member(&group.id, "bob@example.com")
                .expect("Failed to check membership")
        );
    }

    #[test]
    fn test_last_admin_cannot_leave() {
        let service = create_test_service();

        let group = service
            .create_group("test-org", "engineering", "", "alice@example.com")
            .expect("Failed to create group");

        // Alice cannot leave as last admin
        let result = service.leave_group(&group.id, "alice@example.com");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("last admin"));
    }

    #[test]
    fn test_get_user_groups() {
        let service = create_test_service();

        // Create two groups
        let group1 = service
            .create_group("test-org", "engineering", "", "alice@example.com")
            .expect("Failed to create group 1");
        let _group2 = service
            .create_group("test-org", "design", "", "alice@example.com")
            .expect("Failed to create group 2");

        // Add bob to engineering
        service
            .add_member(
                &group1.id,
                "bob@example.com",
                GroupRole::Write,
                "alice@example.com",
            )
            .expect("Failed to add member");

        // Get alice's groups
        let alice_groups = service
            .get_user_groups("test-org", "alice@example.com")
            .expect("Failed to get groups");
        assert_eq!(alice_groups.len(), 2);

        // Get bob's groups
        let bob_groups = service
            .get_user_groups("test-org", "bob@example.com")
            .expect("Failed to get groups");
        assert_eq!(bob_groups.len(), 1);
        assert_eq!(bob_groups[0].group_id, group1.id);
    }

    #[test]
    fn test_require_role() {
        let service = create_test_service();

        let group = service
            .create_group("test-org", "engineering", "", "alice@example.com")
            .expect("Failed to create group");

        // Add bob as writer
        service
            .add_member(
                &group.id,
                "bob@example.com",
                GroupRole::Write,
                "alice@example.com",
            )
            .expect("Failed to add member");

        // Add charlie as reader
        service
            .add_member(
                &group.id,
                "charlie@example.com",
                GroupRole::Read,
                "alice@example.com",
            )
            .expect("Failed to add member");

        // Alice (admin) passes all checks
        assert!(
            service
                .require_role(&group.id, "alice@example.com", GroupRole::Admin)
                .is_ok()
        );
        assert!(
            service
                .require_role(&group.id, "alice@example.com", GroupRole::Write)
                .is_ok()
        );
        assert!(
            service
                .require_role(&group.id, "alice@example.com", GroupRole::Read)
                .is_ok()
        );

        // Bob (writer) passes write and read checks
        assert!(
            service
                .require_role(&group.id, "bob@example.com", GroupRole::Admin)
                .is_err()
        );
        assert!(
            service
                .require_role(&group.id, "bob@example.com", GroupRole::Write)
                .is_ok()
        );
        assert!(
            service
                .require_role(&group.id, "bob@example.com", GroupRole::Read)
                .is_ok()
        );

        // Charlie (reader) only passes read check
        assert!(
            service
                .require_role(&group.id, "charlie@example.com", GroupRole::Admin)
                .is_err()
        );
        assert!(
            service
                .require_role(&group.id, "charlie@example.com", GroupRole::Write)
                .is_err()
        );
        assert!(
            service
                .require_role(&group.id, "charlie@example.com", GroupRole::Read)
                .is_ok()
        );
    }
}
