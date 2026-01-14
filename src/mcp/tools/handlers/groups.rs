//! Group management MCP tool handlers.
//!
//! Provides handlers for group CRUD operations via MCP tools.

use super::super::{ToolContent, ToolResult};
use crate::models::group::{GroupId, GroupRole};
use crate::services::group::GroupService;
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::Value;

/// Parses JSON arguments, converting errors to crate Error type.
fn parse_args<T: for<'de> Deserialize<'de>>(arguments: Value) -> Result<T> {
    serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))
}

/// Arguments for `group_create` tool.
#[derive(Debug, Deserialize)]
struct GroupCreateArgs {
    name: String,
    description: Option<String>,
}

/// Arguments for `group_get` tool.
#[derive(Debug, Deserialize)]
struct GroupGetArgs {
    group_id: String,
}

/// Arguments for `group_add_member` tool.
#[derive(Debug, Deserialize)]
struct GroupAddMemberArgs {
    group_id: String,
    user_id: String,
    role: Option<String>,
}

/// Arguments for `group_remove_member` tool.
#[derive(Debug, Deserialize)]
struct GroupRemoveMemberArgs {
    group_id: String,
    user_id: String,
}

/// Arguments for `group_update_role` tool.
#[derive(Debug, Deserialize)]
struct GroupUpdateRoleArgs {
    group_id: String,
    user_id: String,
    role: String,
}

/// Arguments for `group_delete` tool.
#[derive(Debug, Deserialize)]
struct GroupDeleteArgs {
    group_id: String,
}

/// Gets the current user ID from environment.
fn get_user_id() -> String {
    std::env::var("SUBCOG_USER_ID").unwrap_or_else(|_| "default-user".to_string())
}

/// Gets the current organization ID from environment.
fn get_org_id() -> String {
    std::env::var("SUBCOG_ORG_ID").unwrap_or_else(|_| "default-org".to_string())
}

/// Parses a role string into a `GroupRole`.
fn parse_role(role: Option<&str>) -> GroupRole {
    match role.map(str::to_lowercase).as_deref() {
        Some("admin") => GroupRole::Admin,
        Some("write") => GroupRole::Write,
        _ => GroupRole::Read,
    }
}

/// Creates a success result with text content.
fn text_result(text: String) -> ToolResult {
    ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    }
}

/// Creates an error result with text content.
fn error_result(text: String) -> ToolResult {
    ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: true,
    }
}

/// Returns the description or "(none)" if empty.
const fn desc_or_none(desc: &str) -> &str {
    if desc.is_empty() { "(none)" } else { desc }
}

/// Executes the `group_create` tool.
pub fn execute_group_create(arguments: Value) -> Result<ToolResult> {
    let args: GroupCreateArgs = parse_args(arguments)?;

    let service = GroupService::try_default()?;
    let user_id = get_user_id();
    let org_id = get_org_id();

    match service.create_group(
        &org_id,
        &args.name,
        args.description.as_deref().unwrap_or(""),
        &user_id,
    ) {
        Ok(group) => Ok(text_result(format!(
            "## Group Created\n\n\
             **ID:** {}\n\
             **Name:** {}\n\
             **Description:** {}\n\
             **Your Role:** Admin\n\n\
             You can now:\n\
             - Add members with `subcog_group_add_member`\n\
             - Capture memories to this group with `group_id` parameter",
            group.id,
            group.name,
            desc_or_none(&group.description)
        ))),
        Err(e) => Ok(error_result(format!("Failed to create group: {e}"))),
    }
}

/// Executes the `group_list` tool.
pub fn execute_group_list(_arguments: Value) -> Result<ToolResult> {
    let service = GroupService::try_default()?;
    let user_id = get_user_id();
    let org_id = get_org_id();

    match service.get_user_groups(&org_id, &user_id) {
        Ok(memberships) => {
            if memberships.is_empty() {
                return Ok(text_result(
                    "## Your Groups\n\n\
                     No groups found. Create one with `subcog_group_create`."
                        .to_string(),
                ));
            }

            let mut output = String::from("## Your Groups\n\n");
            for membership in memberships {
                // Get full group details
                if let Ok(Some(group)) = service.get_group(&membership.group_id) {
                    output.push_str(&format!(
                        "### {}\n\
                         - **ID:** {}\n\
                         - **Description:** {}\n\
                         - **Your Role:** {}\n\n",
                        group.name,
                        group.id,
                        desc_or_none(&group.description),
                        membership.role.as_str(),
                    ));
                }
            }

            Ok(text_result(output))
        },
        Err(e) => Ok(error_result(format!("Failed to list groups: {e}"))),
    }
}

/// Executes the `group_get` tool.
pub fn execute_group_get(arguments: Value) -> Result<ToolResult> {
    let args: GroupGetArgs = parse_args(arguments)?;

    let service = GroupService::try_default()?;
    let group_id = GroupId::from(args.group_id.as_str());

    match service.get_group(&group_id) {
        Ok(Some(group)) => {
            let members = service.list_members(&group_id).unwrap_or_default();

            let mut output = format!(
                "## Group: {}\n\n\
                 **ID:** {}\n\
                 **Description:** {}\n\
                 **Organization:** {}\n\n\
                 ### Members ({}):\n\n",
                group.name,
                group.id,
                desc_or_none(&group.description),
                group.org_id,
                members.len()
            );

            for member in members {
                output.push_str(&format!(
                    "- **{}** ({})\n",
                    member.email,
                    member.role.as_str()
                ));
            }

            Ok(text_result(output))
        },
        Ok(None) => Ok(error_result(format!("Group not found: {}", args.group_id))),
        Err(e) => Ok(error_result(format!("Failed to get group: {e}"))),
    }
}

/// Executes the `group_add_member` tool.
pub fn execute_group_add_member(arguments: Value) -> Result<ToolResult> {
    let args: GroupAddMemberArgs = parse_args(arguments)?;

    let service = GroupService::try_default()?;
    let acting_user = get_user_id();
    let group_id = GroupId::from(args.group_id.as_str());
    let role = parse_role(args.role.as_deref());

    match service.add_member(&group_id, &args.user_id, role, &acting_user) {
        Ok(_member) => Ok(text_result(format!(
            "## Member Added\n\n\
             **User:** {}\n\
             **Group:** {}\n\
             **Role:** {}\n\n\
             The user can now access group memories based on their role.",
            args.user_id,
            args.group_id,
            role.as_str()
        ))),
        Err(e) => Ok(error_result(format!("Failed to add member: {e}"))),
    }
}

/// Executes the `group_remove_member` tool.
pub fn execute_group_remove_member(arguments: Value) -> Result<ToolResult> {
    let args: GroupRemoveMemberArgs = parse_args(arguments)?;

    let service = GroupService::try_default()?;
    let acting_user = get_user_id();
    let group_id = GroupId::from(args.group_id.as_str());

    match service.remove_member(&group_id, &args.user_id, &acting_user) {
        Ok(removed) => {
            if removed {
                Ok(text_result(format!(
                    "## Member Removed\n\n\
                     **User:** {}\n\
                     **Group:** {}\n\n\
                     The user no longer has access to group memories.",
                    args.user_id, args.group_id
                )))
            } else {
                Ok(error_result(format!(
                    "Member '{}' not found in group",
                    args.user_id
                )))
            }
        },
        Err(e) => Ok(error_result(format!("Failed to remove member: {e}"))),
    }
}

/// Executes the `group_update_role` tool.
pub fn execute_group_update_role(arguments: Value) -> Result<ToolResult> {
    let args: GroupUpdateRoleArgs = parse_args(arguments)?;

    let service = GroupService::try_default()?;
    let acting_user = get_user_id();
    let group_id = GroupId::from(args.group_id.as_str());
    let role = parse_role(Some(&args.role));

    match service.update_member_role(&group_id, &args.user_id, role, &acting_user) {
        Ok(updated) => {
            if updated {
                Ok(text_result(format!(
                    "## Role Updated\n\n\
                     **User:** {}\n\
                     **Group:** {}\n\
                     **New Role:** {}",
                    args.user_id,
                    args.group_id,
                    role.as_str()
                )))
            } else {
                Ok(error_result(format!(
                    "Member '{}' not found in group",
                    args.user_id
                )))
            }
        },
        Err(e) => Ok(error_result(format!("Failed to update role: {e}"))),
    }
}

/// Executes the `group_delete` tool.
pub fn execute_group_delete(arguments: Value) -> Result<ToolResult> {
    let args: GroupDeleteArgs = parse_args(arguments)?;

    let service = GroupService::try_default()?;
    let acting_user = get_user_id();
    let group_id = GroupId::from(args.group_id.as_str());

    match service.delete_group(&group_id, &acting_user) {
        Ok(deleted) => {
            if deleted {
                Ok(text_result(format!(
                    "## Group Deleted\n\n\
                     **Group ID:** {}\n\n\
                     The group has been deleted. Existing memories remain but are no longer group-accessible.",
                    args.group_id
                )))
            } else {
                Ok(error_result(format!("Group not found: {}", args.group_id)))
            }
        },
        Err(e) => Ok(error_result(format!("Failed to delete group: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_role() {
        assert_eq!(parse_role(Some("admin")), GroupRole::Admin);
        assert_eq!(parse_role(Some("ADMIN")), GroupRole::Admin);
        assert_eq!(parse_role(Some("write")), GroupRole::Write);
        assert_eq!(parse_role(Some("read")), GroupRole::Read);
        assert_eq!(parse_role(None), GroupRole::Read);
        assert_eq!(parse_role(Some("invalid")), GroupRole::Read);
    }

    #[test]
    fn test_text_result() {
        let result = text_result("test".to_string());
        assert!(!result.is_error);
        assert_eq!(result.content.len(), 1);
    }

    #[test]
    fn test_error_result() {
        let result = error_result("error".to_string());
        assert!(result.is_error);
        assert_eq!(result.content.len(), 1);
    }
}
