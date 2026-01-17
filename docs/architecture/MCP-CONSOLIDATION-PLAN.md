# MCP Tool Consolidation Implementation Plan

> **Reference:** [ADR-0061: MCP Tool Consolidation](../adrs/adr_0061.md)

This document provides the step-by-step implementation plan for consolidating Subcog's MCP tools from 43 to 22.

## Overview

```
Current State: 43 tools
Target State:  22 tools
Reduction:     21 tools (49%)
```

## Implementation Phases

### Phase 0: Preparation (Pre-requisite)

**Estimated effort:** 1-2 hours

#### 0.1 Create Feature Flag

Add feature flag for gradual rollout:

```toml
# Cargo.toml
[features]
consolidated-tools = []  # Enable new consolidated tool API
legacy-tools = []        # Keep deprecated tools (default on for compatibility)
default = ["legacy-tools"]
```

#### 0.2 Add Deprecation Infrastructure

Create deprecation warning helper:

```rust
// src/mcp/tools/deprecation.rs
use tracing::warn;

/// Logs a deprecation warning for legacy tools.
pub fn warn_deprecated(old_tool: &str, new_tool: &str, action: &str) {
    warn!(
        old_tool = %old_tool,
        new_tool = %new_tool,
        action = %action,
        "Tool '{}' is deprecated. Use '{}' with action='{}' instead.",
        old_tool, new_tool, action
    );
}
```

#### 0.3 Create Migration Tracking

```rust
// src/mcp/tools/migration.rs
/// Tracks which legacy tools have been called for migration telemetry.
pub struct MigrationMetrics {
    pub legacy_calls: std::collections::HashMap<String, u64>,
}
```

---

### Phase 1: Template Consolidation

**Estimated effort:** 4-6 hours
**Tools affected:** 10 → 2

#### 1.1 Create `subcog_prompt` Consolidated Tool

**File:** `src/mcp/tools/definitions.rs`

```rust
/// Defines the consolidated `subcog_prompt` tool.
pub fn prompt_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_prompt".to_string(),
        description: "Manage prompt templates. Supports save, list, get, run, and delete operations.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Operation to perform",
                    "enum": ["save", "list", "get", "run", "delete"]
                },
                "name": {
                    "type": "string",
                    "description": "Prompt name (required for save/get/run/delete)"
                },
                "content": {
                    "type": "string",
                    "description": "Prompt content (required for save)"
                },
                "description": {
                    "type": "string",
                    "description": "Prompt description (optional for save)"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags for categorization"
                },
                "variables": {
                    "type": "object",
                    "description": "Variable values for run action",
                    "additionalProperties": { "type": "string" }
                },
                "domain": {
                    "type": "string",
                    "description": "Domain scope",
                    "enum": ["project", "user", "org"]
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results for list (default: 20)",
                    "minimum": 1,
                    "maximum": 100
                }
            },
            "required": ["action"]
        }),
    }
}
```

#### 1.2 Create Consolidated Handler

**File:** `src/mcp/tools/handlers/prompts.rs`

```rust
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptAction {
    Save,
    List,
    Get,
    Run,
    Delete,
}

#[derive(Debug, Deserialize)]
pub struct PromptArgs {
    pub action: PromptAction,
    pub name: Option<String>,
    pub content: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub variables: Option<serde_json::Map<String, Value>>,
    pub domain: Option<String>,
    pub limit: Option<u32>,
}

pub fn execute_prompt(arguments: Value) -> crate::Result<ToolResult> {
    let args: PromptArgs = serde_json::from_value(arguments)
        .map_err(|e| crate::Error::InvalidInput(e.to_string()))?;

    match args.action {
        PromptAction::Save => execute_prompt_save(args),
        PromptAction::List => execute_prompt_list(args),
        PromptAction::Get => execute_prompt_get(args),
        PromptAction::Run => execute_prompt_run(args),
        PromptAction::Delete => execute_prompt_delete(args),
    }
}

// Individual action implementations delegate to existing handlers
fn execute_prompt_save(args: PromptArgs) -> crate::Result<ToolResult> {
    // Reuse existing prompt_save logic
    todo!()
}
// ... etc
```

#### 1.3 Create Legacy Compatibility Layer

**File:** `src/mcp/tools/handlers/prompts_legacy.rs`

```rust
use super::prompts::{execute_prompt, PromptAction};
use crate::mcp::tools::deprecation::warn_deprecated;

/// Legacy handler that delegates to consolidated tool.
pub fn execute_prompt_save_legacy(arguments: Value) -> crate::Result<ToolResult> {
    warn_deprecated("prompt_save", "subcog_prompt", "save");

    // Inject action into arguments
    let mut args = arguments.as_object().cloned().unwrap_or_default();
    args.insert("action".to_string(), json!("save"));

    execute_prompt(Value::Object(args))
}

// Similar wrappers for prompt_list, prompt_get, prompt_run, prompt_delete
```

#### 1.4 Create `subcog_template` Consolidated Tool

Same pattern as `subcog_prompt` for context templates:

```rust
pub fn template_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_template".to_string(),
        description: "Manage context templates for hook enrichment. Supports save, list, get, render, and delete.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["save", "list", "get", "render", "delete"]
                },
                // ... rest of schema
            },
            "required": ["action"]
        }),
    }
}
```

#### 1.5 Update Tool Registry

**File:** `src/mcp/tools/mod.rs`

```rust
impl ToolRegistry {
    pub fn new() -> Self {
        let mut tools = HashMap::new();

        // Consolidated tools (always registered)
        #[cfg(feature = "consolidated-tools")]
        {
            tools.insert("subcog_prompt".to_string(), definitions::prompt_tool());
            tools.insert("subcog_template".to_string(), definitions::template_tool());
        }

        // Legacy tools (deprecated, but available for compatibility)
        #[cfg(feature = "legacy-tools")]
        {
            tools.insert("prompt_save".to_string(), definitions::prompt_save_tool());
            tools.insert("prompt_list".to_string(), definitions::prompt_list_tool());
            // ... etc
        }

        // ... rest of tools
    }
}
```

#### 1.6 Tests

**File:** `tests/mcp_prompt_consolidation.rs`

```rust
#[test]
fn test_consolidated_prompt_save() {
    let args = json!({
        "action": "save",
        "name": "test-prompt",
        "content": "Test content"
    });
    let result = execute_prompt(args).unwrap();
    assert!(!result.is_error);
}

#[test]
fn test_legacy_prompt_save_delegates() {
    // Verify legacy tool calls consolidated handler
}

#[test]
fn test_all_prompt_actions() {
    for action in ["save", "list", "get", "run", "delete"] {
        // Test each action
    }
}
```

---

### Phase 2: Group Management Consolidation

**Estimated effort:** 3-4 hours
**Tools affected:** 7 → 1

#### 2.1 Create `subcog_group` Consolidated Tool

**File:** `src/mcp/tools/definitions.rs`

```rust
#[cfg(feature = "group-scope")]
pub fn group_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_group".to_string(),
        description: "Manage groups for shared memory access. Supports create, list, get, add_member, remove_member, update_role, and delete operations.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Operation to perform",
                    "enum": ["create", "list", "get", "add_member", "remove_member", "update_role", "delete"]
                },
                "group_id": {
                    "type": "string",
                    "description": "Group ID (required for get/add_member/remove_member/update_role/delete)"
                },
                "name": {
                    "type": "string",
                    "description": "Group name (required for create)"
                },
                "description": {
                    "type": "string",
                    "description": "Group description (optional for create)"
                },
                "user_id": {
                    "type": "string",
                    "description": "User ID/email (required for add_member/remove_member/update_role)"
                },
                "role": {
                    "type": "string",
                    "description": "Member role",
                    "enum": ["read", "write", "admin"]
                }
            },
            "required": ["action"]
        }),
    }
}
```

#### 2.2 Create Consolidated Handler

**File:** `src/mcp/tools/handlers/groups.rs` (update existing)

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAction {
    Create,
    List,
    Get,
    AddMember,
    RemoveMember,
    UpdateRole,
    Delete,
}

#[derive(Debug, Deserialize)]
pub struct GroupArgs {
    pub action: GroupAction,
    pub group_id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub user_id: Option<String>,
    pub role: Option<String>,
}

pub fn execute_group(arguments: Value) -> crate::Result<ToolResult> {
    let args: GroupArgs = serde_json::from_value(arguments)
        .map_err(|e| crate::Error::InvalidInput(e.to_string()))?;

    match args.action {
        GroupAction::Create => execute_group_create_impl(args),
        GroupAction::List => execute_group_list_impl(args),
        GroupAction::Get => execute_group_get_impl(args),
        GroupAction::AddMember => execute_group_add_member_impl(args),
        GroupAction::RemoveMember => execute_group_remove_member_impl(args),
        GroupAction::UpdateRole => execute_group_update_role_impl(args),
        GroupAction::Delete => execute_group_delete_impl(args),
    }
}
```

#### 2.3 Validation Helper

```rust
fn validate_group_args(args: &GroupArgs) -> crate::Result<()> {
    match args.action {
        GroupAction::Create => {
            if args.name.is_none() {
                return Err(crate::Error::InvalidInput(
                    "name is required for create action".to_string()
                ));
            }
        }
        GroupAction::Get | GroupAction::Delete => {
            if args.group_id.is_none() {
                return Err(crate::Error::InvalidInput(
                    "group_id is required for this action".to_string()
                ));
            }
        }
        GroupAction::AddMember | GroupAction::RemoveMember | GroupAction::UpdateRole => {
            if args.group_id.is_none() || args.user_id.is_none() {
                return Err(crate::Error::InvalidInput(
                    "group_id and user_id are required for this action".to_string()
                ));
            }
        }
        GroupAction::List => {} // No required args
    }
    Ok(())
}
```

---

### Phase 3: Knowledge Graph Consolidation

**Estimated effort:** 4-5 hours
**Tools affected:** 7 → 3

#### 3.1 Extend `subcog_entities` with New Actions

Add `extract` and `merge` to existing `subcog_entities`:

```rust
pub fn entities_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_entities".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "get", "list", "delete", "extract", "merge"]
                    //                                         ^^^^^^^^^^^^^^^^ NEW
                },
                // For extract action:
                "content": {
                    "type": "string",
                    "description": "Text to extract entities from (for extract action)"
                },
                "store": {
                    "type": "boolean",
                    "description": "Store extracted entities (for extract action)"
                },
                // For merge action:
                "source_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Entity IDs to merge (for merge action)"
                },
                "target_id": {
                    "type": "string",
                    "description": "Target entity ID for merge"
                },
                // ... existing properties
            }
        }),
    }
}
```

#### 3.2 Extend `subcog_relationships` with `infer` Action

```rust
"action": {
    "enum": ["create", "get", "list", "delete", "infer"]
    //                                          ^^^^^ NEW
}
```

#### 3.3 Create `subcog_graph` Consolidated Tool

Merge `graph_query` and `graph_visualize`:

```rust
pub fn graph_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_graph".to_string(),
        description: "Query and visualize the knowledge graph.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["neighbors", "path", "stats", "visualize"]
                },
                // ... combined properties from both tools
            }
        }),
    }
}
```

#### 3.4 Deprecate Old Tools

```rust
// These become legacy aliases:
// subcog_extract_entities → subcog_entities { action: "extract" }
// subcog_entity_merge → subcog_entities { action: "merge" }
// subcog_relationship_infer → subcog_relationships { action: "infer" }
// subcog_graph_query → subcog_graph { operation: "..." }
// subcog_graph_visualize → subcog_graph { operation: "visualize" }
```

---

### Phase 4: Core CRUD Optimization

**Estimated effort:** 2-3 hours
**Tools affected:** 8 → 6

#### 4.1 Merge `subcog_list` into `subcog_recall`

Update `subcog_recall` to support null/empty query for unfiltered listing:

```rust
pub fn recall_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_recall".to_string(),
        description: "Search or list memories. Omit query for unfiltered listing.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query. Omit or set null to list all memories."
                    // Note: no longer in "required"
                },
                // ... rest unchanged
            },
            "required": []  // query no longer required
        }),
    }
}
```

#### 4.2 Update Handler

```rust
pub fn execute_recall(arguments: Value) -> Result<ToolResult> {
    let args: RecallArgs = parse_args(arguments)?;

    if args.query.is_none() || args.query.as_ref().map(|q| q.is_empty()).unwrap_or(true) {
        // Delegate to list behavior
        return execute_list_memories(args);
    }

    // Existing search behavior
    execute_search_memories(args)
}
```

#### 4.3 Remove `subcog_sync`

Already marked for removal in changelog. Delete:
- `definitions::sync_tool()`
- `handlers::execute_sync()`
- Registry entry

---

### Phase 5: Documentation & Migration

**Estimated effort:** 3-4 hours

#### 5.1 Create Migration Guide

**File:** `docs/MIGRATION-TOOLS-V2.md`

```markdown
# MCP Tools Migration Guide (v0.8.0)

## Overview

Version 0.8.0 consolidates 43 MCP tools into 22 for better maintainability.

## Tool Mapping

| Old Tool | New Tool | Action |
|----------|----------|--------|
| `prompt_save` | `subcog_prompt` | `save` |
| `prompt_list` | `subcog_prompt` | `list` |
| `prompt_get` | `subcog_prompt` | `get` |
| `prompt_run` | `subcog_prompt` | `run` |
| `prompt_delete` | `subcog_prompt` | `delete` |
| `context_template_save` | `subcog_template` | `save` |
| ... | ... | ... |

## Examples

### Before (v0.7.x)
```json
{ "tool": "prompt_save", "arguments": { "name": "review", "content": "..." } }
```

### After (v0.8.0)
```json
{ "tool": "subcog_prompt", "arguments": { "action": "save", "name": "review", "content": "..." } }
```

## Compatibility

Legacy tools remain available with `legacy-tools` feature (enabled by default).
They will be removed in v0.10.0.
```

#### 5.2 Update Tool Documentation

Update `docs/mcp/` with new consolidated tool references.

#### 5.3 Update CHANGELOG

```markdown
## [0.8.0] - YYYY-MM-DD

### Changed

#### MCP Tool Consolidation (Breaking)
- Consolidated 43 MCP tools into 22 using action-based patterns
- `prompt_*` tools → `subcog_prompt` with action parameter
- `context_template_*` tools → `subcog_template` with action parameter
- `subcog_group_*` tools → `subcog_group` with action parameter
- Extended `subcog_entities` with extract/merge actions
- Extended `subcog_relationships` with infer action
- Merged `subcog_graph_query` + `subcog_graph_visualize` → `subcog_graph`
- Merged `subcog_list` into `subcog_recall` (omit query for listing)

### Deprecated
- Legacy tool names (available via `legacy-tools` feature until v0.10.0)

### Removed
- `subcog_sync` (SQLite is authoritative, sync no longer needed)
```

---

## Implementation Schedule

| Phase | Description | Effort | Priority |
|-------|-------------|--------|----------|
| 0 | Preparation (feature flags, deprecation infra) | 1-2h | P0 |
| 1 | Template consolidation (10 → 2) | 4-6h | P1 |
| 2 | Group consolidation (7 → 1) | 3-4h | P1 |
| 3 | Graph consolidation (7 → 3) | 4-5h | P2 |
| 4 | Core CRUD optimization (8 → 6) | 2-3h | P2 |
| 5 | Documentation & migration | 3-4h | P1 |

**Total estimated effort:** 17-24 hours

## Rollout Strategy

### v0.8.0-alpha
- All consolidated tools available behind `consolidated-tools` feature
- Legacy tools still default
- Early adopter testing

### v0.8.0
- Consolidated tools become default
- Legacy tools available via `legacy-tools` feature
- Deprecation warnings emitted

### v0.9.0
- Deprecation warnings become errors (legacy tools still work)
- Migration reminder in `subcog_status` output

### v0.10.0
- Legacy tools removed
- Clean tool surface with 22 tools

## Testing Strategy

1. **Unit tests** for each consolidated handler
2. **Integration tests** verifying legacy → new delegation
3. **E2E tests** with MCP client
4. **Migration tests** ensuring same behavior for old tool calls

## Rollback Plan

If issues discovered post-release:
1. Enable `legacy-tools` feature by default
2. Document workaround
3. Fix consolidated tool bugs
4. Re-attempt in next minor version

## Success Metrics

| Metric | Before | After | Target |
|--------|--------|-------|--------|
| Tool count | 43 | 22 | ≤25 |
| Tool schema tokens | ~8000 | ~4000 | -50% |
| Handler files | 12 | 8 | -33% |
| Test coverage | 85% | 90% | ≥90% |
