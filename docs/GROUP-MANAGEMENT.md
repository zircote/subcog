# Group Management & Shared Knowledge Graphs

This guide covers Subcog's group management feature for team collaboration through shared memory graphs.

> **Note**: This feature requires the `group-scope` feature flag to be enabled at compile time.

## Overview

Groups enable teams within an organization to share memories and knowledge graphs. Each group has:

- **Unique identifier** within the organization
- **Members with roles** (admin, write, read)
- **Shared memory space** for collaborative knowledge capture
- **Token-based invites** for adding new members

## Quick Start

### 1. Enable the Feature

Build Subcog with the `group-scope` feature:

```bash
cargo build --features group-scope
```

### 2. Set Environment Variables

```bash
export SUBCOG_USER_ID="alice@example.com"
export SUBCOG_ORG_ID="acme-corp"
```

### 3. Create Your First Group

Using the MCP tool:

```json
{
  "tool": "subcog_group_create",
  "arguments": {
    "name": "engineering",
    "description": "Engineering team shared knowledge"
  }
}
```

## Role-Based Access Control (RBAC)

Groups use a three-tier permission model:

| Role | Capture Memories | Recall Memories | Manage Members | Delete Group |
|------|-----------------|-----------------|----------------|--------------|
| **Admin** | ✅ | ✅ | ✅ | ✅ |
| **Write** | ✅ | ✅ | ❌ | ❌ |
| **Read** | ❌ | ✅ | ❌ | ❌ |

### Permission Rules

| Operation | Required Role |
|-----------|--------------|
| Create group | Org member |
| Delete group | Group admin |
| Add member | Group admin |
| Remove member | Group admin (cannot remove last admin) |
| Update role | Group admin (cannot demote last admin) |
| Create invite | Group admin |
| List members | Group member |
| Join via invite | Anyone with valid token |
| Leave group | Self (cannot leave if last admin) |

## MCP Tools Reference

### `subcog_group_create`

Create a new group. You automatically become the admin.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `name` | string | ✅ | Group name (must be unique within org) |
| `description` | string | ❌ | Description of the group's purpose |

**Example:**

```json
{
  "name": "research-team",
  "description": "ML research collaboration space"
}
```

**Response:**

```
## Group Created

**ID:** a1b2c3d4e5f6
**Name:** research-team
**Description:** ML research collaboration space
**Your Role:** Admin

You can now:
- Add members with `subcog_group_add_member`
- Capture memories to this group with `group_id` parameter
```

---

### `subcog_group_list`

List all groups you belong to.

**Parameters:** None

**Response:**

```
## Your Groups

### engineering
- **ID:** abc123def456
- **Description:** Engineering team shared knowledge
- **Your Role:** admin

### research-team
- **ID:** a1b2c3d4e5f6
- **Description:** ML research collaboration space
- **Your Role:** write
```

---

### `subcog_group_get`

Get detailed information about a specific group.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `group_id` | string | ✅ | The ID of the group to retrieve |

**Example:**

```json
{
  "group_id": "abc123def456"
}
```

**Response:**

```
## Group: engineering

**ID:** abc123def456
**Description:** Engineering team shared knowledge
**Organization:** acme-corp

### Members (3):

- **alice@example.com** (admin)
- **bob@example.com** (write)
- **carol@example.com** (read)
```

---

### `subcog_group_add_member`

Add a new member to a group. Requires admin role.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `group_id` | string | ✅ | The ID of the group |
| `user_id` | string | ✅ | Email/ID of the user to add |
| `role` | string | ❌ | Role: `read` (default), `write`, or `admin` |

**Example:**

```json
{
  "group_id": "abc123def456",
  "user_id": "dave@example.com",
  "role": "write"
}
```

**Response:**

```
## Member Added

**User:** dave@example.com
**Group:** abc123def456
**Role:** write

The user can now access group memories based on their role.
```

---

### `subcog_group_remove_member`

Remove a member from a group. Requires admin role.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `group_id` | string | ✅ | The ID of the group |
| `user_id` | string | ✅ | Email/ID of the user to remove |

**Example:**

```json
{
  "group_id": "abc123def456",
  "user_id": "dave@example.com"
}
```

---

### `subcog_group_update_role`

Change a member's role. Requires admin role.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `group_id` | string | ✅ | The ID of the group |
| `user_id` | string | ✅ | Email/ID of the user to update |
| `role` | string | ✅ | New role: `read`, `write`, or `admin` |

**Example:**

```json
{
  "group_id": "abc123def456",
  "user_id": "bob@example.com",
  "role": "admin"
}
```

---

### `subcog_group_delete`

Delete a group. Requires admin role.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `group_id` | string | ✅ | The ID of the group to delete |

**Example:**

```json
{
  "group_id": "abc123def456"
}
```

> **Note:** Existing memories remain but are no longer group-accessible after deletion.

## Working with Group Memories

### Capturing to a Group

Use the `group_id` parameter when capturing memories:

```json
{
  "tool": "subcog_capture",
  "arguments": {
    "content": "Decision: Use PostgreSQL for the analytics database",
    "namespace": "decisions",
    "group_id": "abc123def456"
  }
}
```

### Recalling from a Group

Filter recalls by group:

```json
{
  "tool": "subcog_recall",
  "arguments": {
    "query": "database decisions",
    "group_id": "abc123def456"
  }
}
```

## Architecture

### Data Model

```
┌─────────────────────────────────────────────────────────────┐
│                       Organization                           │
│  (SUBCOG_ORG_ID)                                            │
│                                                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │   Group A   │  │   Group B   │  │   Group C   │         │
│  │             │  │             │  │             │         │
│  │ ┌─────────┐ │  │ ┌─────────┐ │  │ ┌─────────┐ │         │
│  │ │ Members │ │  │ │ Members │ │  │ │ Members │ │         │
│  │ │ ──────  │ │  │ │ ──────  │ │  │ │ ──────  │ │         │
│  │ │ Alice A │ │  │ │ Bob   W │ │  │ │ Carol A │ │         │
│  │ │ Bob   W │ │  │ │ Carol R │ │  │ │ Dave  W │ │         │
│  │ │ Carol R │ │  │ └─────────┘ │  │ └─────────┘ │         │
│  │ └─────────┘ │  │             │  │             │         │
│  │             │  │ ┌─────────┐ │  │ ┌─────────┐ │         │
│  │ ┌─────────┐ │  │ │Memories │ │  │ │Memories │ │         │
│  │ │Memories │ │  │ │ • • •   │ │  │ │ • • •   │ │         │
│  │ │ • • •   │ │  │ └─────────┘ │  │ └─────────┘ │         │
│  │ └─────────┘ │  └─────────────┘  └─────────────┘         │
│  └─────────────┘                                            │
└─────────────────────────────────────────────────────────────┘

Legend: A=Admin, W=Write, R=Read
```

### Storage

Groups are stored in a dedicated SQLite database:

```
~/.local/share/subcog/index/groups.db
```

**Schema:**

```sql
-- Groups table
CREATE TABLE groups (
    id TEXT PRIMARY KEY,
    org_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT DEFAULT '',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    created_by TEXT NOT NULL,
    UNIQUE(org_id, name)
);

-- Group members table
CREATE TABLE group_members (
    id TEXT PRIMARY KEY,
    group_id TEXT NOT NULL REFERENCES groups(id),
    email TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('admin', 'write', 'read')),
    joined_at INTEGER NOT NULL,
    added_by TEXT NOT NULL,
    UNIQUE(group_id, email)
);
```

## Invite System (Future)

> **Note:** The invite system is defined in the data model but MCP tools are not yet implemented.

Groups support token-based invites with:

- **Expiration**: Default 7 days
- **Usage limits**: Single or multi-use
- **Role assignment**: Pre-defined role for invitees
- **Secure tokens**: SHA256-hashed, never stored in plaintext

```rust
// Create an invite (programmatic API)
let (invite, token) = service.create_invite(
    &group_id,
    GroupRole::Write,
    "admin@example.com",
    Some(7 * 24 * 60 * 60),  // 7 days
    Some(5),                  // 5 uses max
)?;

// Share the token out-of-band
println!("Invite link: https://subcog.io/join/{}", token);
```

## Best Practices

### 1. Group Organization

- **One group per team/project**: Avoid mixing unrelated memories
- **Clear naming**: Use descriptive names like `ml-research` or `infra-team`
- **Document purpose**: Always add a description

### 2. Role Assignment

- **Least privilege**: Start with `read`, upgrade as needed
- **Multiple admins**: Have at least 2 admins per group
- **Regular audits**: Review member access periodically

### 3. Memory Hygiene

- **Tag consistently**: Use agreed-upon tags within the group
- **Namespace properly**: Follow namespace conventions
- **Avoid duplicates**: Check for existing memories before capturing

## Troubleshooting

### "Group not found"

- Verify the `group_id` is correct
- Check you have access to the group with `subcog_group_list`

### "Permission denied"

- Verify your role in the group with `subcog_group_get`
- Admin operations require `admin` role
- Capture operations require `write` or `admin` role

### "Cannot remove last admin"

- Groups must always have at least one admin
- Promote another member to admin first, then remove/demote

### Environment Variables Not Set

```bash
# Check current values
echo $SUBCOG_USER_ID
echo $SUBCOG_ORG_ID

# Set defaults in your shell profile
export SUBCOG_USER_ID="your-email@example.com"
export SUBCOG_ORG_ID="your-organization"
```

## Related Documentation

- [URN Guide](URN-GUIDE.md) - Memory URN format and structure
- [MCP Tools](mcp/) - Full MCP tool reference
- [Architecture](architecture/) - System architecture overview
