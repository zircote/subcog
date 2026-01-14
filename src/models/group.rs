//! Group and membership models for shared memory graphs.
//!
//! This module provides types for organizing memories into groups
//! that can be shared across team members within an organization.
//!
//! # Overview
//!
//! Groups exist within an organization scope and allow multiple users
//! to share memories. Each group has:
//! - A unique identifier within the organization
//! - Members with roles (admin, write, read)
//! - Token-based invite system for adding members
//!
//! # Example
//!
//! ```rust,ignore
//! use subcog::models::group::{Group, GroupRole, GroupMember};
//!
//! let group = Group::new("research-team", "acme-corp", "alice@example.com");
//! let member = GroupMember::new(group.id.clone(), "bob@example.com", GroupRole::Write);
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a group.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GroupId(String);

impl GroupId {
    /// Creates a new group ID from the given string.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generates a new random group ID using UUID v4.
    ///
    /// Uses v4 (random) instead of v7 (time-based) to ensure uniqueness
    /// even when generating multiple IDs in rapid succession.
    #[must_use]
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().simple().to_string()[..12].to_string())
    }

    /// Returns the ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for GroupId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for GroupId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Role-based access control for group members.
///
/// Roles determine what actions a member can perform on group memories:
/// - `Admin`: Full control (manage members, delete group, capture, recall)
/// - `Write`: Capture and recall memories
/// - `Read`: Recall memories only
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupRole {
    /// Full control over the group.
    ///
    /// Admins can:
    /// - Add and remove members
    /// - Change member roles
    /// - Delete the group
    /// - Capture and recall memories
    Admin,

    /// Read and write access.
    ///
    /// Writers can:
    /// - Capture memories to the group
    /// - Recall group memories
    Write,

    /// Read-only access.
    ///
    /// Readers can:
    /// - Recall group memories
    Read,
}

impl GroupRole {
    /// Returns the role as a string slice.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Write => "write",
            Self::Read => "read",
        }
    }

    /// Parses a role from a string.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// assert_eq!(GroupRole::parse("admin"), Some(GroupRole::Admin));
    /// assert_eq!(GroupRole::parse("WRITE"), Some(GroupRole::Write));
    /// assert_eq!(GroupRole::parse("invalid"), None);
    /// ```
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "admin" => Some(Self::Admin),
            "write" => Some(Self::Write),
            "read" => Some(Self::Read),
            _ => None,
        }
    }

    /// Returns `true` if this role can capture memories to the group.
    #[must_use]
    pub const fn can_write(&self) -> bool {
        matches!(self, Self::Admin | Self::Write)
    }

    /// Returns `true` if this role can recall memories from the group.
    #[must_use]
    pub const fn can_read(&self) -> bool {
        // All roles can read
        true
    }

    /// Returns `true` if this role can manage group members.
    #[must_use]
    pub const fn can_manage(&self) -> bool {
        matches!(self, Self::Admin)
    }
}

impl fmt::Display for GroupRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for GroupRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("unknown group role: {s}"))
    }
}

/// A group for sharing memories within an organization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    /// Unique identifier for the group.
    pub id: GroupId,

    /// Organization this group belongs to.
    pub org_id: String,

    /// Human-readable name for the group.
    pub name: String,

    /// Optional description of the group's purpose.
    pub description: String,

    /// When the group was created (Unix timestamp).
    pub created_at: u64,

    /// When the group was last updated (Unix timestamp).
    pub updated_at: u64,

    /// Email of the user who created the group.
    pub created_by: String,
}

impl Group {
    /// Creates a new group with the given name and organization.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name for the group
    /// * `org_id` - Organization identifier
    /// * `created_by` - Email of the creator
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        org_id: impl Into<String>,
        created_by: impl Into<String>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            id: GroupId::generate(),
            org_id: org_id.into(),
            name: name.into(),
            description: String::new(),
            created_at: now,
            updated_at: now,
            created_by: created_by.into(),
        }
    }

    /// Creates a new group with a description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
}

/// A member of a group with an assigned role.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMember {
    /// Unique identifier for this membership record.
    pub id: String,

    /// The group this member belongs to.
    pub group_id: GroupId,

    /// Email address of the member (identity).
    pub email: String,

    /// Role within the group.
    pub role: GroupRole,

    /// When the member joined (Unix timestamp).
    pub joined_at: u64,

    /// Email of the user who added this member.
    pub added_by: String,
}

impl GroupMember {
    /// Creates a new group member.
    ///
    /// # Arguments
    ///
    /// * `group_id` - ID of the group
    /// * `email` - Email address of the member
    /// * `role` - Role to assign
    /// * `added_by` - Email of the user adding this member
    #[must_use]
    pub fn new(
        group_id: GroupId,
        email: impl Into<String>,
        role: GroupRole,
        added_by: impl Into<String>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            // Use v4 for member IDs to ensure uniqueness in rapid succession
            id: uuid::Uuid::new_v4().simple().to_string()[..12].to_string(),
            group_id,
            email: email.into().to_lowercase(),
            role,
            joined_at: now,
            added_by: added_by.into().to_lowercase(),
        }
    }
}

/// An invitation to join a group.
///
/// Invites use a token-based system where:
/// 1. An admin creates an invite with a role
/// 2. The invite generates a random token
/// 3. The token is shared out-of-band (email, Slack, etc.)
/// 4. The recipient joins using the token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInvite {
    /// Unique identifier for this invite.
    pub id: String,

    /// The group this invite is for.
    pub group_id: GroupId,

    /// SHA256 hash of the invite token (never store plaintext).
    pub token_hash: String,

    /// Role to assign when the invite is used.
    pub role: GroupRole,

    /// Email of the user who created the invite.
    pub created_by: String,

    /// When the invite was created (Unix timestamp).
    pub created_at: u64,

    /// When the invite expires (Unix timestamp).
    pub expires_at: u64,

    /// Maximum number of times this invite can be used.
    ///
    /// `None` means unlimited uses.
    pub max_uses: Option<u32>,

    /// Number of times this invite has been used.
    pub current_uses: u32,

    /// Whether this invite has been revoked.
    pub revoked: bool,
}

impl GroupInvite {
    /// Default expiration time in seconds (7 days).
    pub const DEFAULT_EXPIRY_SECS: u64 = 7 * 24 * 60 * 60;

    /// Default maximum uses for an invite.
    pub const DEFAULT_MAX_USES: u32 = 1;

    /// Creates a new invite for a group.
    ///
    /// Returns both the invite and the plaintext token.
    /// The token should be shared with the invitee and never stored.
    ///
    /// # Arguments
    ///
    /// * `group_id` - ID of the group to invite to
    /// * `role` - Role to assign when joined
    /// * `created_by` - Email of the admin creating the invite
    /// * `expires_in_secs` - How long until the invite expires
    /// * `max_uses` - Maximum number of uses (None for unlimited)
    #[must_use]
    pub fn new(
        group_id: GroupId,
        role: GroupRole,
        created_by: impl Into<String>,
        expires_in_secs: Option<u64>,
        max_uses: Option<u32>,
    ) -> (Self, String) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Generate a secure random token
        let token = uuid::Uuid::now_v7().to_string();
        let token_hash = Self::hash_token(&token);

        let invite = Self {
            // Use v4 for invite IDs to ensure uniqueness in rapid succession
            id: uuid::Uuid::new_v4().simple().to_string()[..12].to_string(),
            group_id,
            token_hash,
            role,
            created_by: created_by.into().to_lowercase(),
            created_at: now,
            expires_at: now + expires_in_secs.unwrap_or(Self::DEFAULT_EXPIRY_SECS),
            max_uses: Some(max_uses.unwrap_or(Self::DEFAULT_MAX_USES)),
            current_uses: 0,
            revoked: false,
        };

        (invite, token)
    }

    /// Hashes a token using SHA256.
    #[must_use]
    pub fn hash_token(token: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Checks if this invite is still valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        if self.revoked {
            return false;
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if now > self.expires_at {
            return false;
        }

        if let Some(max) = self.max_uses
            && self.current_uses >= max
        {
            return false;
        }

        true
    }

    /// Verifies a token against this invite's hash.
    #[must_use]
    pub fn verify_token(&self, token: &str) -> bool {
        Self::hash_token(token) == self.token_hash
    }
}

/// Summary of a user's membership in a group.
///
/// Used for listing accessible groups without full member details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMembership {
    /// The group ID.
    pub group_id: GroupId,

    /// The group name.
    pub group_name: String,

    /// The organization ID.
    pub org_id: String,

    /// User's role in this group.
    pub role: GroupRole,
}

/// Request to create a new group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGroupRequest {
    /// Name for the new group.
    pub name: String,

    /// Optional description.
    pub description: Option<String>,

    /// Initial members to add (email, role pairs).
    pub initial_members: Vec<(String, GroupRole)>,
}

/// Request to add a member to a group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMemberRequest {
    /// Group to add the member to.
    pub group_id: GroupId,

    /// Email of the new member.
    pub email: String,

    /// Role to assign.
    pub role: GroupRole,
}

/// Request to create an invite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInviteRequest {
    /// Group to create the invite for.
    pub group_id: GroupId,

    /// Role to assign when the invite is used.
    pub role: GroupRole,

    /// How long the invite should be valid (seconds).
    pub expires_in_secs: Option<u64>,

    /// Maximum number of uses.
    pub max_uses: Option<u32>,
}

/// Email validation helper.
///
/// Performs basic RFC 5322 format validation.
#[must_use]
pub fn is_valid_email(email: &str) -> bool {
    // Basic validation: contains @, has local and domain parts
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }

    let local = parts[0];
    let domain = parts[1];

    // Local part must be non-empty
    if local.is_empty() {
        return false;
    }

    // Domain must have at least one dot and non-empty parts
    let domain_parts: Vec<&str> = domain.split('.').collect();
    if domain_parts.len() < 2 {
        return false;
    }

    domain_parts.iter().all(|part| !part.is_empty())
}

/// Normalizes an email address to lowercase.
#[must_use]
pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_id_generate() {
        let id1 = GroupId::generate();
        let id2 = GroupId::generate();
        assert_ne!(id1, id2);
        assert_eq!(id1.as_str().len(), 12);
    }

    #[test]
    fn test_group_role_parsing() {
        assert_eq!(GroupRole::parse("admin"), Some(GroupRole::Admin));
        assert_eq!(GroupRole::parse("WRITE"), Some(GroupRole::Write));
        assert_eq!(GroupRole::parse("Read"), Some(GroupRole::Read));
        assert_eq!(GroupRole::parse("invalid"), None);
    }

    #[test]
    fn test_group_role_permissions() {
        assert!(GroupRole::Admin.can_write());
        assert!(GroupRole::Admin.can_read());
        assert!(GroupRole::Admin.can_manage());

        assert!(GroupRole::Write.can_write());
        assert!(GroupRole::Write.can_read());
        assert!(!GroupRole::Write.can_manage());

        assert!(!GroupRole::Read.can_write());
        assert!(GroupRole::Read.can_read());
        assert!(!GroupRole::Read.can_manage());
    }

    #[test]
    fn test_group_creation() {
        let group = Group::new("test-group", "acme-corp", "admin@example.com");
        assert_eq!(group.name, "test-group");
        assert_eq!(group.org_id, "acme-corp");
        assert_eq!(group.created_by, "admin@example.com");
        assert!(group.created_at > 0);
    }

    #[test]
    fn test_group_member_email_normalization() {
        let member = GroupMember::new(
            GroupId::new("group-1"),
            "Bob@Example.COM",
            GroupRole::Write,
            "Admin@Example.COM",
        );
        assert_eq!(member.email, "bob@example.com");
        assert_eq!(member.added_by, "admin@example.com");
    }

    #[test]
    fn test_invite_token_hashing() {
        let (invite, token) = GroupInvite::new(
            GroupId::new("group-1"),
            GroupRole::Write,
            "admin@example.com",
            None,
            None,
        );

        assert!(invite.verify_token(&token));
        assert!(!invite.verify_token("wrong-token"));
    }

    #[test]
    fn test_invite_validity() {
        let (mut invite, _) = GroupInvite::new(
            GroupId::new("group-1"),
            GroupRole::Write,
            "admin@example.com",
            Some(3600), // 1 hour
            Some(2),    // 2 uses max
        );

        assert!(invite.is_valid());

        // Test max uses
        invite.current_uses = 2;
        assert!(!invite.is_valid());

        // Reset and test revocation
        invite.current_uses = 0;
        invite.revoked = true;
        assert!(!invite.is_valid());

        // Reset and test expiration
        invite.revoked = false;
        invite.expires_at = 0;
        assert!(!invite.is_valid());
    }

    #[test]
    fn test_email_validation() {
        assert!(is_valid_email("user@example.com"));
        assert!(is_valid_email("user.name@example.co.uk"));
        assert!(is_valid_email("user+tag@example.com"));

        assert!(!is_valid_email("invalid"));
        assert!(!is_valid_email("@example.com"));
        assert!(!is_valid_email("user@"));
        assert!(!is_valid_email("user@example"));
        assert!(!is_valid_email(""));
    }

    #[test]
    fn test_email_normalization() {
        assert_eq!(normalize_email("User@Example.COM"), "user@example.com");
        assert_eq!(normalize_email("  user@example.com  "), "user@example.com");
    }
}
