//! Group storage trait definitions.
//!
//! Defines the interface for group storage backends, supporting CRUD operations
//! for groups, members, and invites.

use crate::Result;
use crate::models::group::{Group, GroupId, GroupInvite, GroupMember, GroupMembership, GroupRole};

/// Trait for group storage backends.
///
/// Provides storage operations for groups, members, and invites within an organization.
/// Implementations must be thread-safe (`Send + Sync`).
pub trait GroupBackend: Send + Sync {
    // =========================================================================
    // Group Operations
    // =========================================================================

    /// Creates a new group.
    ///
    /// # Arguments
    ///
    /// * `org_id` - Organization identifier
    /// * `name` - Group name (must be unique within org)
    /// * `description` - Optional description
    /// * `created_by` - Email of the creator
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A group with the same name already exists in the org
    /// - Storage cannot be accessed
    fn create_group(
        &self,
        org_id: &str,
        name: &str,
        description: &str,
        created_by: &str,
    ) -> Result<Group>;

    /// Gets a group by ID.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group identifier
    ///
    /// # Returns
    ///
    /// The group if found, None otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    fn get_group(&self, group_id: &GroupId) -> Result<Option<Group>>;

    /// Gets a group by name within an organization.
    ///
    /// # Arguments
    ///
    /// * `org_id` - Organization identifier
    /// * `name` - Group name
    ///
    /// # Returns
    ///
    /// The group if found, None otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    fn get_group_by_name(&self, org_id: &str, name: &str) -> Result<Option<Group>>;

    /// Lists all groups in an organization.
    ///
    /// # Arguments
    ///
    /// * `org_id` - Organization identifier
    ///
    /// # Returns
    ///
    /// List of groups in the organization.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    fn list_groups(&self, org_id: &str) -> Result<Vec<Group>>;

    /// Deletes a group and all its members and invites.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group to delete
    ///
    /// # Returns
    ///
    /// True if the group was deleted, false if it didn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    fn delete_group(&self, group_id: &GroupId) -> Result<bool>;

    // =========================================================================
    // Member Operations
    // =========================================================================

    /// Adds a member to a group.
    ///
    /// If the member already exists, updates their role.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group to add to
    /// * `email` - Email of the new member
    /// * `role` - Role to assign
    /// * `added_by` - Email of the user adding this member
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The group doesn't exist
    /// - Storage cannot be accessed
    fn add_member(
        &self,
        group_id: &GroupId,
        email: &str,
        role: GroupRole,
        added_by: &str,
    ) -> Result<GroupMember>;

    /// Gets a member's record in a group.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group
    /// * `email` - The member's email
    ///
    /// # Returns
    ///
    /// The member record if found, None otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    fn get_member(&self, group_id: &GroupId, email: &str) -> Result<Option<GroupMember>>;

    /// Updates a member's role in a group.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group
    /// * `email` - The member's email
    /// * `new_role` - The new role to assign
    ///
    /// # Returns
    ///
    /// True if the member was updated, false if not found.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    fn update_member_role(
        &self,
        group_id: &GroupId,
        email: &str,
        new_role: GroupRole,
    ) -> Result<bool>;

    /// Removes a member from a group.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group
    /// * `email` - The member's email
    ///
    /// # Returns
    ///
    /// True if the member was removed, false if not found.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    fn remove_member(&self, group_id: &GroupId, email: &str) -> Result<bool>;

    /// Lists all members of a group.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group
    ///
    /// # Returns
    ///
    /// List of members with their roles.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    fn list_members(&self, group_id: &GroupId) -> Result<Vec<GroupMember>>;

    /// Gets all groups a user is a member of.
    ///
    /// # Arguments
    ///
    /// * `org_id` - Organization to search within
    /// * `email` - The user's email
    ///
    /// # Returns
    ///
    /// List of group memberships for the user.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    fn get_user_groups(&self, org_id: &str, email: &str) -> Result<Vec<GroupMembership>>;

    /// Counts the number of admins in a group.
    ///
    /// Used to prevent removing the last admin.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group
    ///
    /// # Returns
    ///
    /// Number of members with admin role.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    fn count_admins(&self, group_id: &GroupId) -> Result<u32>;

    // =========================================================================
    // Invite Operations
    // =========================================================================

    /// Creates an invite for a group.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group to invite to
    /// * `role` - Role to assign when joined
    /// * `created_by` - Email of the admin creating the invite
    /// * `expires_in_secs` - How long until expiration
    /// * `max_uses` - Maximum number of uses
    ///
    /// # Returns
    ///
    /// A tuple of (invite, `plaintext_token`). The token should be shared
    /// with invitees and never stored.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The group doesn't exist
    /// - Storage cannot be accessed
    fn create_invite(
        &self,
        group_id: &GroupId,
        role: GroupRole,
        created_by: &str,
        expires_in_secs: Option<u64>,
        max_uses: Option<u32>,
    ) -> Result<(GroupInvite, String)>;

    /// Gets an invite by its token hash.
    ///
    /// # Arguments
    ///
    /// * `token_hash` - SHA256 hash of the token
    ///
    /// # Returns
    ///
    /// The invite if found and valid, None otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    fn get_invite_by_token_hash(&self, token_hash: &str) -> Result<Option<GroupInvite>>;

    /// Gets an invite by ID.
    ///
    /// # Arguments
    ///
    /// * `invite_id` - The invite identifier
    ///
    /// # Returns
    ///
    /// The invite if found, None otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    fn get_invite(&self, invite_id: &str) -> Result<Option<GroupInvite>>;

    /// Lists all invites for a group.
    ///
    /// # Arguments
    ///
    /// * `group_id` - The group
    /// * `include_expired` - Whether to include expired/revoked invites
    ///
    /// # Returns
    ///
    /// List of invites.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    fn list_invites(&self, group_id: &GroupId, include_expired: bool) -> Result<Vec<GroupInvite>>;

    /// Increments the use count of an invite.
    ///
    /// Called when a user successfully joins using the invite.
    ///
    /// # Arguments
    ///
    /// * `invite_id` - The invite to update
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    fn increment_invite_uses(&self, invite_id: &str) -> Result<()>;

    /// Revokes an invite.
    ///
    /// # Arguments
    ///
    /// * `invite_id` - The invite to revoke
    ///
    /// # Returns
    ///
    /// True if the invite was revoked, false if not found.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    fn revoke_invite(&self, invite_id: &str) -> Result<bool>;

    /// Deletes expired invites.
    ///
    /// # Returns
    ///
    /// Number of invites deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be accessed.
    fn cleanup_expired_invites(&self) -> Result<u64>;
}
