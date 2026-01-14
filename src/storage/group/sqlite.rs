//! `SQLite` backend for group storage.
//!
//! Stores groups, members, and invites in the organization's `SQLite` database.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};

use crate::models::group::{
    Group, GroupId, GroupInvite, GroupMember, GroupMembership, GroupRole, normalize_email,
};
use crate::{Error, Result};

use super::traits::GroupBackend;

/// SQLite-based group storage backend.
///
/// Uses the same database as other org-scoped data, with dedicated tables
/// for groups, members, and invites.
pub struct SqliteGroupBackend {
    /// Database connection (mutex for interior mutability).
    conn: Mutex<Connection>,
}

impl SqliteGroupBackend {
    /// Creates a new `SQLite` group backend at the specified path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the `SQLite` database file
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or initialized.
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path.as_ref()).map_err(|e| Error::OperationFailed {
            operation: "open_group_database".to_string(),
            cause: e.to_string(),
        })?;

        let backend = Self {
            conn: Mutex::new(conn),
        };
        backend.initialize_schema()?;
        Ok(backend)
    }

    /// Creates an in-memory `SQLite` group backend (for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be initialized.
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| Error::OperationFailed {
            operation: "open_group_database_memory".to_string(),
            cause: e.to_string(),
        })?;

        let backend = Self {
            conn: Mutex::new(conn),
        };
        backend.initialize_schema()?;
        Ok(backend)
    }

    /// Returns the default path for organization-scoped group storage.
    ///
    /// The path is `~/.config/subcog/orgs/{org}/memories.db`.
    #[must_use]
    pub fn default_org_path(org: &str) -> Option<PathBuf> {
        directories::BaseDirs::new().map(|d| {
            d.home_dir()
                .join(".config")
                .join("subcog")
                .join("orgs")
                .join(org)
                .join("memories.db")
        })
    }

    /// Initializes the database schema.
    fn initialize_schema(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        conn.execute_batch(
            r"
            -- Enable foreign keys first
            PRAGMA foreign_keys = ON;

            -- Groups table
            CREATE TABLE IF NOT EXISTS groups (
                id TEXT PRIMARY KEY,
                org_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                created_by TEXT NOT NULL,
                UNIQUE(org_id, name)
            );

            CREATE INDEX IF NOT EXISTS idx_groups_org ON groups(org_id);

            -- Group members table
            CREATE TABLE IF NOT EXISTS group_members (
                id TEXT PRIMARY KEY,
                group_id TEXT NOT NULL,
                email TEXT NOT NULL,
                role TEXT NOT NULL,
                joined_at INTEGER NOT NULL,
                added_by TEXT NOT NULL,
                UNIQUE(group_id, email),
                FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_group_members_group ON group_members(group_id);
            CREATE INDEX IF NOT EXISTS idx_group_members_email ON group_members(email);

            -- Group invites table
            CREATE TABLE IF NOT EXISTS group_invites (
                id TEXT PRIMARY KEY,
                group_id TEXT NOT NULL,
                token_hash TEXT NOT NULL UNIQUE,
                role TEXT NOT NULL,
                created_by TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                max_uses INTEGER,
                current_uses INTEGER NOT NULL DEFAULT 0,
                revoked INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_group_invites_group ON group_invites(group_id);
            CREATE INDEX IF NOT EXISTS idx_group_invites_token_hash ON group_invites(token_hash);
            CREATE INDEX IF NOT EXISTS idx_group_invites_expires ON group_invites(expires_at);
            ",
        )
        .map_err(|e| Error::OperationFailed {
            operation: "initialize_group_schema".to_string(),
            cause: e.to_string(),
        })?;

        Ok(())
    }

    /// Gets the current Unix timestamp as i64 (for `SQLite` compatibility).
    #[allow(clippy::cast_possible_wrap)]
    fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    /// Converts u64 timestamp to i64 for `SQLite` storage.
    #[allow(clippy::cast_possible_wrap)]
    const fn to_db_timestamp(ts: u64) -> i64 {
        ts as i64
    }

    /// Converts i64 from `SQLite` back to u64 timestamp.
    #[allow(clippy::cast_sign_loss)]
    const fn from_db_timestamp(ts: i64) -> u64 {
        ts as u64
    }

    /// Parses a `GroupRole` from a string stored in the database.
    fn parse_role(s: &str) -> GroupRole {
        GroupRole::parse(s).unwrap_or(GroupRole::Read)
    }
}

impl GroupBackend for SqliteGroupBackend {
    fn create_group(
        &self,
        org_id: &str,
        name: &str,
        description: &str,
        created_by: &str,
    ) -> Result<Group> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let now = Self::now();
        let group = Group {
            id: GroupId::generate(),
            org_id: org_id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            created_at: Self::from_db_timestamp(now),
            updated_at: Self::from_db_timestamp(now),
            created_by: normalize_email(created_by),
        };

        conn.execute(
            "INSERT INTO groups (id, org_id, name, description, created_at, updated_at, created_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                group.id.as_str(),
                group.org_id,
                group.name,
                group.description,
                now,
                now,
                group.created_by,
            ],
        )
        .map_err(|e| {
            if e.to_string().contains("UNIQUE constraint failed") {
                Error::InvalidInput(format!(
                    "Group '{name}' already exists in organization '{org_id}'"
                ))
            } else {
                Error::OperationFailed {
                    operation: "create_group".to_string(),
                    cause: e.to_string(),
                }
            }
        })?;

        Ok(group)
    }

    fn get_group(&self, group_id: &GroupId) -> Result<Option<Group>> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, org_id, name, description, created_at, updated_at, created_by
                 FROM groups WHERE id = ?1",
            )
            .map_err(|e| Error::OperationFailed {
                operation: "prepare_get_group".to_string(),
                cause: e.to_string(),
            })?;

        let result = stmt
            .query_row(params![group_id.as_str()], |row| {
                Ok(Group {
                    id: GroupId::new(row.get::<_, String>(0)?),
                    org_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    created_at: Self::from_db_timestamp(row.get(4)?),
                    updated_at: Self::from_db_timestamp(row.get(5)?),
                    created_by: row.get(6)?,
                })
            })
            .optional()
            .map_err(|e| Error::OperationFailed {
                operation: "get_group".to_string(),
                cause: e.to_string(),
            })?;

        Ok(result)
    }

    fn get_group_by_name(&self, org_id: &str, name: &str) -> Result<Option<Group>> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, org_id, name, description, created_at, updated_at, created_by
                 FROM groups WHERE org_id = ?1 AND name = ?2",
            )
            .map_err(|e| Error::OperationFailed {
                operation: "prepare_get_group_by_name".to_string(),
                cause: e.to_string(),
            })?;

        let result = stmt
            .query_row(params![org_id, name], |row| {
                Ok(Group {
                    id: GroupId::new(row.get::<_, String>(0)?),
                    org_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    created_at: Self::from_db_timestamp(row.get(4)?),
                    updated_at: Self::from_db_timestamp(row.get(5)?),
                    created_by: row.get(6)?,
                })
            })
            .optional()
            .map_err(|e| Error::OperationFailed {
                operation: "get_group_by_name".to_string(),
                cause: e.to_string(),
            })?;

        Ok(result)
    }

    fn list_groups(&self, org_id: &str) -> Result<Vec<Group>> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, org_id, name, description, created_at, updated_at, created_by
                 FROM groups WHERE org_id = ?1 ORDER BY name",
            )
            .map_err(|e| Error::OperationFailed {
                operation: "prepare_list_groups".to_string(),
                cause: e.to_string(),
            })?;

        let groups = stmt
            .query_map(params![org_id], |row| {
                Ok(Group {
                    id: GroupId::new(row.get::<_, String>(0)?),
                    org_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    created_at: Self::from_db_timestamp(row.get(4)?),
                    updated_at: Self::from_db_timestamp(row.get(5)?),
                    created_by: row.get(6)?,
                })
            })
            .map_err(|e| Error::OperationFailed {
                operation: "list_groups".to_string(),
                cause: e.to_string(),
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::OperationFailed {
                operation: "collect_groups".to_string(),
                cause: e.to_string(),
            })?;

        Ok(groups)
    }

    fn delete_group(&self, group_id: &GroupId) -> Result<bool> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let rows = conn
            .execute(
                "DELETE FROM groups WHERE id = ?1",
                params![group_id.as_str()],
            )
            .map_err(|e| Error::OperationFailed {
                operation: "delete_group".to_string(),
                cause: e.to_string(),
            })?;

        Ok(rows > 0)
    }

    fn add_member(
        &self,
        group_id: &GroupId,
        email: &str,
        role: GroupRole,
        added_by: &str,
    ) -> Result<GroupMember> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let member = GroupMember::new(group_id.clone(), email, role, added_by);

        // Use INSERT ... ON CONFLICT to handle existing members (update role only)
        // This preserves the original joined_at timestamp for existing members
        conn.execute(
            "INSERT INTO group_members (id, group_id, email, role, joined_at, added_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (group_id, email) DO UPDATE SET
                 role = excluded.role,
                 added_by = excluded.added_by",
            params![
                member.id,
                member.group_id.as_str(),
                member.email,
                member.role.as_str(),
                Self::to_db_timestamp(member.joined_at),
                member.added_by,
            ],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "add_member".to_string(),
            cause: e.to_string(),
        })?;

        Ok(member)
    }

    fn get_member(&self, group_id: &GroupId, email: &str) -> Result<Option<GroupMember>> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let normalized_email = normalize_email(email);

        let mut stmt = conn
            .prepare(
                "SELECT id, group_id, email, role, joined_at, added_by
                 FROM group_members WHERE group_id = ?1 AND email = ?2",
            )
            .map_err(|e| Error::OperationFailed {
                operation: "prepare_get_member".to_string(),
                cause: e.to_string(),
            })?;

        let result = stmt
            .query_row(params![group_id.as_str(), normalized_email], |row| {
                Ok(GroupMember {
                    id: row.get(0)?,
                    group_id: GroupId::new(row.get::<_, String>(1)?),
                    email: row.get(2)?,
                    role: Self::parse_role(&row.get::<_, String>(3)?),
                    joined_at: Self::from_db_timestamp(row.get(4)?),
                    added_by: row.get(5)?,
                })
            })
            .optional()
            .map_err(|e| Error::OperationFailed {
                operation: "get_member".to_string(),
                cause: e.to_string(),
            })?;

        Ok(result)
    }

    fn update_member_role(
        &self,
        group_id: &GroupId,
        email: &str,
        new_role: GroupRole,
    ) -> Result<bool> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let normalized_email = normalize_email(email);

        let rows = conn
            .execute(
                "UPDATE group_members SET role = ?1 WHERE group_id = ?2 AND email = ?3",
                params![new_role.as_str(), group_id.as_str(), normalized_email],
            )
            .map_err(|e| Error::OperationFailed {
                operation: "update_member_role".to_string(),
                cause: e.to_string(),
            })?;

        Ok(rows > 0)
    }

    fn remove_member(&self, group_id: &GroupId, email: &str) -> Result<bool> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let normalized_email = normalize_email(email);

        let rows = conn
            .execute(
                "DELETE FROM group_members WHERE group_id = ?1 AND email = ?2",
                params![group_id.as_str(), normalized_email],
            )
            .map_err(|e| Error::OperationFailed {
                operation: "remove_member".to_string(),
                cause: e.to_string(),
            })?;

        Ok(rows > 0)
    }

    fn list_members(&self, group_id: &GroupId) -> Result<Vec<GroupMember>> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, group_id, email, role, joined_at, added_by
                 FROM group_members WHERE group_id = ?1 ORDER BY email",
            )
            .map_err(|e| Error::OperationFailed {
                operation: "prepare_list_members".to_string(),
                cause: e.to_string(),
            })?;

        let members = stmt
            .query_map(params![group_id.as_str()], |row| {
                Ok(GroupMember {
                    id: row.get(0)?,
                    group_id: GroupId::new(row.get::<_, String>(1)?),
                    email: row.get(2)?,
                    role: Self::parse_role(&row.get::<_, String>(3)?),
                    joined_at: Self::from_db_timestamp(row.get(4)?),
                    added_by: row.get(5)?,
                })
            })
            .map_err(|e| Error::OperationFailed {
                operation: "list_members".to_string(),
                cause: e.to_string(),
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::OperationFailed {
                operation: "collect_members".to_string(),
                cause: e.to_string(),
            })?;

        Ok(members)
    }

    fn get_user_groups(&self, org_id: &str, email: &str) -> Result<Vec<GroupMembership>> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let normalized_email = normalize_email(email);

        let mut stmt = conn
            .prepare(
                "SELECT g.id, g.name, g.org_id, gm.role
                 FROM groups g
                 JOIN group_members gm ON g.id = gm.group_id
                 WHERE g.org_id = ?1 AND gm.email = ?2
                 ORDER BY g.name",
            )
            .map_err(|e| Error::OperationFailed {
                operation: "prepare_get_user_groups".to_string(),
                cause: e.to_string(),
            })?;

        let memberships = stmt
            .query_map(params![org_id, normalized_email], |row| {
                Ok(GroupMembership {
                    group_id: GroupId::new(row.get::<_, String>(0)?),
                    group_name: row.get(1)?,
                    org_id: row.get(2)?,
                    role: Self::parse_role(&row.get::<_, String>(3)?),
                })
            })
            .map_err(|e| Error::OperationFailed {
                operation: "get_user_groups".to_string(),
                cause: e.to_string(),
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::OperationFailed {
                operation: "collect_user_groups".to_string(),
                cause: e.to_string(),
            })?;

        Ok(memberships)
    }

    fn count_admins(&self, group_id: &GroupId) -> Result<u32> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM group_members WHERE group_id = ?1 AND role = 'admin'",
                params![group_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|e| Error::OperationFailed {
                operation: "count_admins".to_string(),
                cause: e.to_string(),
            })?;

        Ok(count)
    }

    fn create_invite(
        &self,
        group_id: &GroupId,
        role: GroupRole,
        created_by: &str,
        expires_in_secs: Option<u64>,
        max_uses: Option<u32>,
    ) -> Result<(GroupInvite, String)> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let (invite, token) = GroupInvite::new(
            group_id.clone(),
            role,
            created_by,
            expires_in_secs,
            max_uses,
        );

        conn.execute(
            "INSERT INTO group_invites
             (id, group_id, token_hash, role, created_by, created_at, expires_at, max_uses, current_uses, revoked)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                invite.id,
                invite.group_id.as_str(),
                invite.token_hash,
                invite.role.as_str(),
                invite.created_by,
                Self::to_db_timestamp(invite.created_at),
                Self::to_db_timestamp(invite.expires_at),
                invite.max_uses,
                invite.current_uses,
                i32::from(invite.revoked),
            ],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "create_invite".to_string(),
            cause: e.to_string(),
        })?;

        Ok((invite, token))
    }

    fn get_invite_by_token_hash(&self, token_hash: &str) -> Result<Option<GroupInvite>> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, group_id, token_hash, role, created_by, created_at, expires_at,
                        max_uses, current_uses, revoked
                 FROM group_invites WHERE token_hash = ?1",
            )
            .map_err(|e| Error::OperationFailed {
                operation: "prepare_get_invite_by_token".to_string(),
                cause: e.to_string(),
            })?;

        let result = stmt
            .query_row(params![token_hash], |row| {
                Ok(GroupInvite {
                    id: row.get(0)?,
                    group_id: GroupId::new(row.get::<_, String>(1)?),
                    token_hash: row.get(2)?,
                    role: Self::parse_role(&row.get::<_, String>(3)?),
                    created_by: row.get(4)?,
                    created_at: Self::from_db_timestamp(row.get(5)?),
                    expires_at: Self::from_db_timestamp(row.get(6)?),
                    max_uses: row.get(7)?,
                    current_uses: row.get(8)?,
                    revoked: row.get::<_, i32>(9)? != 0,
                })
            })
            .optional()
            .map_err(|e| Error::OperationFailed {
                operation: "get_invite_by_token".to_string(),
                cause: e.to_string(),
            })?;

        Ok(result)
    }

    fn get_invite(&self, invite_id: &str) -> Result<Option<GroupInvite>> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, group_id, token_hash, role, created_by, created_at, expires_at,
                        max_uses, current_uses, revoked
                 FROM group_invites WHERE id = ?1",
            )
            .map_err(|e| Error::OperationFailed {
                operation: "prepare_get_invite".to_string(),
                cause: e.to_string(),
            })?;

        let result = stmt
            .query_row(params![invite_id], |row| {
                Ok(GroupInvite {
                    id: row.get(0)?,
                    group_id: GroupId::new(row.get::<_, String>(1)?),
                    token_hash: row.get(2)?,
                    role: Self::parse_role(&row.get::<_, String>(3)?),
                    created_by: row.get(4)?,
                    created_at: Self::from_db_timestamp(row.get(5)?),
                    expires_at: Self::from_db_timestamp(row.get(6)?),
                    max_uses: row.get(7)?,
                    current_uses: row.get(8)?,
                    revoked: row.get::<_, i32>(9)? != 0,
                })
            })
            .optional()
            .map_err(|e| Error::OperationFailed {
                operation: "get_invite".to_string(),
                cause: e.to_string(),
            })?;

        Ok(result)
    }

    fn list_invites(&self, group_id: &GroupId, include_expired: bool) -> Result<Vec<GroupInvite>> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let query = if include_expired {
            "SELECT id, group_id, token_hash, role, created_by, created_at, expires_at,
                    max_uses, current_uses, revoked
             FROM group_invites WHERE group_id = ?1 ORDER BY created_at DESC"
        } else {
            "SELECT id, group_id, token_hash, role, created_by, created_at, expires_at,
                    max_uses, current_uses, revoked
             FROM group_invites
             WHERE group_id = ?1 AND revoked = 0 AND expires_at > ?2
             ORDER BY created_at DESC"
        };

        let mut stmt = conn.prepare(query).map_err(|e| Error::OperationFailed {
            operation: "prepare_list_invites".to_string(),
            cause: e.to_string(),
        })?;

        // Helper to parse invite from row
        let parse_invite = |row: &rusqlite::Row<'_>| -> rusqlite::Result<GroupInvite> {
            Ok(GroupInvite {
                id: row.get(0)?,
                group_id: GroupId::new(row.get::<_, String>(1)?),
                token_hash: row.get(2)?,
                role: Self::parse_role(&row.get::<_, String>(3)?),
                created_by: row.get(4)?,
                created_at: Self::from_db_timestamp(row.get(5)?),
                expires_at: Self::from_db_timestamp(row.get(6)?),
                max_uses: row.get(7)?,
                current_uses: row.get(8)?,
                revoked: row.get::<_, i32>(9)? != 0,
            })
        };

        let invites = if include_expired {
            stmt.query_map(params![group_id.as_str()], parse_invite)
        } else {
            stmt.query_map(params![group_id.as_str(), Self::now()], parse_invite)
        }
        .map_err(|e| Error::OperationFailed {
            operation: "list_invites".to_string(),
            cause: e.to_string(),
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::OperationFailed {
            operation: "collect_invites".to_string(),
            cause: e.to_string(),
        })?;

        Ok(invites)
    }

    fn increment_invite_uses(&self, invite_id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        conn.execute(
            "UPDATE group_invites SET current_uses = current_uses + 1 WHERE id = ?1",
            params![invite_id],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "increment_invite_uses".to_string(),
            cause: e.to_string(),
        })?;

        Ok(())
    }

    fn revoke_invite(&self, invite_id: &str) -> Result<bool> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let rows = conn
            .execute(
                "UPDATE group_invites SET revoked = 1 WHERE id = ?1",
                params![invite_id],
            )
            .map_err(|e| Error::OperationFailed {
                operation: "revoke_invite".to_string(),
                cause: e.to_string(),
            })?;

        Ok(rows > 0)
    }

    fn cleanup_expired_invites(&self) -> Result<u64> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_connection".to_string(),
            cause: e.to_string(),
        })?;

        let rows = conn
            .execute(
                "DELETE FROM group_invites WHERE expires_at < ?1 OR revoked = 1",
                params![Self::now()],
            )
            .map_err(|e| Error::OperationFailed {
                operation: "cleanup_expired_invites".to_string(),
                cause: e.to_string(),
            })?;

        Ok(rows as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_backend() -> SqliteGroupBackend {
        // Use a unique temp file for each test to ensure isolation
        let dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let path = dir.path().join("test_groups.db");
        // We leak the TempDir to keep it alive for the duration of the test
        std::mem::forget(dir);
        SqliteGroupBackend::new(&path).expect("Failed to create test backend")
    }

    #[test]
    fn test_create_and_get_group() {
        let backend = create_test_backend();

        let group = backend
            .create_group(
                "acme-corp",
                "research",
                "Research team",
                "admin@example.com",
            )
            .expect("Failed to create group");

        assert_eq!(group.name, "research");
        assert_eq!(group.org_id, "acme-corp");
        assert_eq!(group.created_by, "admin@example.com");

        let retrieved = backend
            .get_group(&group.id)
            .expect("Failed to get group")
            .expect("Group not found");

        assert_eq!(retrieved.id, group.id);
        assert_eq!(retrieved.name, group.name);
    }

    #[test]
    fn test_duplicate_group_name() {
        let backend = create_test_backend();

        backend
            .create_group("acme-corp", "research", "", "admin@example.com")
            .expect("Failed to create first group");

        let result = backend.create_group("acme-corp", "research", "", "admin@example.com");

        assert!(result.is_err());
    }

    #[test]
    fn test_add_and_list_members() {
        let backend = create_test_backend();

        let group = backend
            .create_group("acme-corp", "team", "", "admin@example.com")
            .expect("Failed to create group");

        backend
            .add_member(
                &group.id,
                "alice@example.com",
                GroupRole::Admin,
                "admin@example.com",
            )
            .expect("Failed to add alice");
        backend
            .add_member(
                &group.id,
                "bob@example.com",
                GroupRole::Write,
                "admin@example.com",
            )
            .expect("Failed to add bob");

        let members = backend
            .list_members(&group.id)
            .expect("Failed to list members");

        assert_eq!(members.len(), 2);
        assert!(members.iter().any(|m| m.email == "alice@example.com"));
        assert!(members.iter().any(|m| m.email == "bob@example.com"));
    }

    #[test]
    fn test_get_user_groups() {
        let backend = create_test_backend();

        let group1 = backend
            .create_group("acme-corp", "team-a", "", "admin@example.com")
            .expect("Failed to create group1");
        let group2 = backend
            .create_group("acme-corp", "team-b", "", "admin@example.com")
            .expect("Failed to create group2");

        backend
            .add_member(
                &group1.id,
                "user@example.com",
                GroupRole::Write,
                "admin@example.com",
            )
            .expect("Failed to add user to group1");
        backend
            .add_member(
                &group2.id,
                "user@example.com",
                GroupRole::Read,
                "admin@example.com",
            )
            .expect("Failed to add user to group2");

        let memberships = backend
            .get_user_groups("acme-corp", "user@example.com")
            .expect("Failed to get user groups");

        assert_eq!(memberships.len(), 2);
    }

    #[test]
    fn test_invite_workflow() {
        let backend = create_test_backend();

        let group = backend
            .create_group("acme-corp", "team", "", "admin@example.com")
            .expect("Failed to create group");

        let (invite, token) = backend
            .create_invite(
                &group.id,
                GroupRole::Write,
                "admin@example.com",
                None,
                Some(1),
            )
            .expect("Failed to create invite");

        assert!(invite.is_valid());

        // Verify token hash lookup
        let token_hash = GroupInvite::hash_token(&token);
        let retrieved = backend
            .get_invite_by_token_hash(&token_hash)
            .expect("Failed to get invite")
            .expect("Invite not found");

        assert_eq!(retrieved.id, invite.id);

        // Increment uses
        backend
            .increment_invite_uses(&invite.id)
            .expect("Failed to increment uses");

        let updated = backend
            .get_invite(&invite.id)
            .expect("Failed to get invite")
            .expect("Invite not found");

        assert_eq!(updated.current_uses, 1);
        assert!(!updated.is_valid()); // max_uses = 1, so now invalid
    }

    #[test]
    fn test_count_admins() {
        let backend = create_test_backend();

        let group = backend
            .create_group("acme-corp", "team", "", "admin@example.com")
            .expect("Failed to create group");

        backend
            .add_member(&group.id, "admin1@example.com", GroupRole::Admin, "system")
            .expect("Failed to add admin1");
        backend
            .add_member(&group.id, "admin2@example.com", GroupRole::Admin, "system")
            .expect("Failed to add admin2");
        backend
            .add_member(&group.id, "user@example.com", GroupRole::Write, "system")
            .expect("Failed to add user");

        let count = backend
            .count_admins(&group.id)
            .expect("Failed to count admins");
        assert_eq!(count, 2);
    }

    #[test]
    fn test_delete_group_cascades() {
        let backend = create_test_backend();

        let group = backend
            .create_group("acme-corp", "team", "", "admin@example.com")
            .expect("Failed to create group");

        backend
            .add_member(
                &group.id,
                "user@example.com",
                GroupRole::Write,
                "admin@example.com",
            )
            .expect("Failed to add member");
        backend
            .create_invite(&group.id, GroupRole::Read, "admin@example.com", None, None)
            .expect("Failed to create invite");

        let deleted = backend
            .delete_group(&group.id)
            .expect("Failed to delete group");
        assert!(deleted);

        let members = backend
            .list_members(&group.id)
            .expect("Failed to list members");
        assert!(members.is_empty());

        let invites = backend
            .list_invites(&group.id, true)
            .expect("Failed to list invites");
        assert!(invites.is_empty());
    }
}
