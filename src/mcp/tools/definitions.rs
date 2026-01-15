//! Tool definitions for MCP tools.
//!
//! Contains the JSON Schema definitions for all subcog tools.

use super::ToolDefinition;

/// Defines the capture tool.
pub fn capture_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_capture".to_string(),
        description: "Capture a memory (decision, learning, pattern, etc.) for future recall"
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The memory content to capture"
                },
                "namespace": {
                    "type": "string",
                    "description": "Memory category: decisions, patterns, learnings, context, tech-debt, apis, config, security, performance, testing",
                    "enum": ["decisions", "patterns", "learnings", "context", "tech-debt", "apis", "config", "security", "performance", "testing"]
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional tags for categorization"
                },
                "source": {
                    "type": "string",
                    "description": "Optional source reference (file path, URL)"
                },
                "ttl": {
                    "type": "string",
                    "description": "Optional TTL for automatic expiration. Supports: '7d' (days), '24h' (hours), '60m' (minutes), '3600s' or '3600' (seconds), '0' (never expire)"
                },
                "domain": {
                    "type": "string",
                    "description": "Storage scope: 'project' (default, stored with project context), 'user' (global across all projects), 'org' (organization-shared)",
                    "enum": ["project", "user", "org"],
                    "default": "project"
                }
            },
            "required": ["content", "namespace"]
        }),
    }
}

/// Defines the recall tool.
///
/// When `query` is omitted, behaves like `subcog_list` and returns all memories
/// matching the filter criteria (with pagination support).
pub fn recall_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_recall".to_string(),
        description: "Search for relevant memories using semantic and text search, or list all memories when no query is provided. Returns normalized scores (0.0-1.0 where 1.0 is the best match) with raw RRF scores shown in parentheses for debugging. Subsumes subcog_list functionality.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query. If omitted, lists all memories matching the filter criteria."
                },
                "filter": {
                    "type": "string",
                    "description": "Filter query using GitHub-style syntax: ns:decisions tag:rust -tag:test since:7d source:src/*"
                },
                "namespace": {
                    "type": "string",
                    "description": "Optional: Filter by namespace (deprecated, use filter instead)",
                    "enum": ["decisions", "patterns", "learnings", "context", "tech-debt", "apis", "config", "security", "performance", "testing"]
                },
                "mode": {
                    "type": "string",
                    "description": "Search mode: hybrid (default), vector, text",
                    "enum": ["hybrid", "vector", "text"]
                },
                "detail": {
                    "type": "string",
                    "description": "Detail level: light (frontmatter only), medium (+ summary), everything (full content). Default: medium",
                    "enum": ["light", "medium", "everything"]
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 10 for search, 50 for list)",
                    "minimum": 1,
                    "maximum": 1000
                },
                "entity": {
                    "type": "string",
                    "description": "Filter by entity names (memories mentioning these entities). Comma-separated for OR logic (e.g., 'PostgreSQL,Redis')"
                },
                "offset": {
                    "type": "integer",
                    "description": "Offset for pagination (default: 0). Used when listing without query.",
                    "minimum": 0,
                    "default": 0
                },
                "user_id": {
                    "type": "string",
                    "description": "Filter by user ID (for multi-tenant scoping)"
                },
                "agent_id": {
                    "type": "string",
                    "description": "Filter by agent ID (for multi-agent scoping)"
                }
            },
            "required": []
        }),
    }
}

/// Defines the status tool.
pub fn status_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_status".to_string(),
        description: "Get memory system status and statistics".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Defines the `prompt_understanding` tool.
pub fn prompt_understanding_tool() -> ToolDefinition {
    ToolDefinition {
        name: "prompt_understanding".to_string(),
        description: "Detailed guidance for using Subcog MCP tools effectively".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Defines the namespaces tool.
pub fn namespaces_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_namespaces".to_string(),
        description: "List available memory namespaces and their descriptions".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Defines the consolidate tool.
pub fn consolidate_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_consolidate".to_string(),
        description: "Consolidate related memories by finding semantic clusters and creating summary nodes. Returns consolidation statistics.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "namespaces": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": ["decisions", "patterns", "learnings", "context", "tech-debt", "apis", "config", "security", "performance", "testing"]
                    },
                    "description": "Namespaces to consolidate (optional, defaults to all)"
                },
                "days": {
                    "type": "integer",
                    "description": "Time window in days for memories to consolidate (optional)",
                    "minimum": 1
                },
                "dry_run": {
                    "type": "boolean",
                    "description": "If true, show what would be consolidated without making changes",
                    "default": false
                },
                "min_memories": {
                    "type": "integer",
                    "description": "Minimum number of memories required to form a group (optional, default: 3)",
                    "minimum": 2
                },
                "similarity": {
                    "type": "number",
                    "description": "Similarity threshold 0.0-1.0 for grouping related memories (optional, default: 0.7)",
                    "minimum": 0.0,
                    "maximum": 1.0
                }
            },
            "required": []
        }),
    }
}

/// Defines the get summary tool.
pub fn get_summary_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_get_summary".to_string(),
        description: "Retrieve a summary node and its linked source memories. Uses edge relationships to show which memories were consolidated into the summary.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "memory_id": {
                    "type": "string",
                    "description": "ID of the summary memory to retrieve"
                }
            },
            "required": ["memory_id"]
        }),
    }
}

/// Defines the enrich tool.
pub fn enrich_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_enrich".to_string(),
        description: "Enrich a memory with better structure, tags, and context using LLM. Uses MCP sampling to request LLM completion.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "memory_id": {
                    "type": "string",
                    "description": "ID of the memory to enrich"
                },
                "enrich_tags": {
                    "type": "boolean",
                    "description": "Generate or improve tags",
                    "default": true
                },
                "enrich_structure": {
                    "type": "boolean",
                    "description": "Restructure content for clarity",
                    "default": true
                },
                "add_context": {
                    "type": "boolean",
                    "description": "Add inferred context and rationale",
                    "default": false
                }
            },
            "required": ["memory_id"]
        }),
    }
}

/// Defines the sync tool.
///
/// **DEPRECATED**: `SQLite` is now the authoritative storage. This tool is retained
/// for backward compatibility but is a no-op. Will be removed in a future version.
pub fn sync_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_sync".to_string(),
        description: "[DEPRECATED] Sync memories with git remote. No longer needed as SQLite is now authoritative storage. This tool is a no-op and will be removed in a future version.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "direction": {
                    "type": "string",
                    "description": "Sync direction: push (upload), fetch (download), full (both)",
                    "enum": ["push", "fetch", "full"],
                    "default": "full"
                }
            },
            "required": []
        }),
    }
}

/// Defines the reindex tool.
pub fn reindex_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_reindex".to_string(),
        description: "Rebuild the search index from stored memories. Use when index is out of sync with stored memories.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "repo_path": {
                    "type": "string",
                    "description": "Path to git repository (default: current directory)"
                }
            },
            "required": []
        }),
    }
}

/// Defines the GDPR data export tool.
///
/// Implements GDPR Article 20 (Right to Data Portability).
pub fn gdpr_export_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_gdpr_export".to_string(),
        description: "Export all user data in a portable JSON format (GDPR Article 20 - Right to Data Portability). Returns all memories with metadata for download or transfer to another system.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

// ============================================================================
// Prompt Management Tools
// ============================================================================

/// Defines the consolidated `subcog_prompts` tool.
///
/// Combines all prompt operations into a single action-based tool.
pub fn prompts_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_prompts".to_string(),
        description: "Manage prompt templates. Actions: save, list, get, run, delete.".to_string(),
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
                    "description": "Prompt content with {{variable}} placeholders (for save, required if file_path not provided)"
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to file containing prompt (for save, alternative to content)"
                },
                "description": {
                    "type": "string",
                    "description": "Human-readable description (for save)"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags for categorization (for save/list)"
                },
                "domain": {
                    "type": "string",
                    "description": "Storage scope: project (default), user, or org",
                    "enum": ["project", "user", "org"],
                    "default": "project"
                },
                "variables_def": {
                    "type": "array",
                    "description": "Variable definitions with metadata (for save)",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "description": { "type": "string" },
                            "default": { "type": "string" },
                            "required": { "type": "boolean", "default": true }
                        },
                        "required": ["name"]
                    }
                },
                "variables": {
                    "type": "object",
                    "description": "Variable values to substitute (for run)",
                    "additionalProperties": { "type": "string" }
                },
                "skip_enrichment": {
                    "type": "boolean",
                    "description": "Skip LLM-powered metadata enrichment (for save)",
                    "default": false
                },
                "name_pattern": {
                    "type": "string",
                    "description": "Filter by name pattern (for list, glob-style)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (for list)",
                    "minimum": 1,
                    "maximum": 100,
                    "default": 20
                }
            },
            "required": ["action"]
        }),
    }
}

/// Defines the prompt.save tool (legacy).
pub fn prompt_save_tool() -> ToolDefinition {
    ToolDefinition {
        name: "prompt_save".to_string(),
        description: "Save a user-defined prompt template. Provide either 'content' or 'file_path' (not both)".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Unique prompt name (kebab-case, e.g., 'code-review')"
                },
                "content": {
                    "type": "string",
                    "description": "Prompt content with {{variable}} placeholders (required if file_path not provided)"
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to file containing prompt (alternative to content)"
                },
                "description": {
                    "type": "string",
                    "description": "Human-readable description of the prompt"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags for categorization and search"
                },
                "domain": {
                    "type": "string",
                    "description": "Storage scope: project (default), user, or org",
                    "enum": ["project", "user", "org"],
                    "default": "project"
                },
                "variables": {
                    "type": "array",
                    "description": "Explicit variable definitions with metadata",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "Variable name (without braces)" },
                            "description": { "type": "string", "description": "Human-readable description" },
                            "default": { "type": "string", "description": "Default value if not provided" },
                            "required": { "type": "boolean", "default": true, "description": "Whether variable is required" }
                        },
                        "required": ["name"]
                    }
                },
                "skip_enrichment": {
                    "type": "boolean",
                    "description": "Skip LLM-powered metadata enrichment (default: false)",
                    "default": false
                }
            },
            "required": ["name"],
            "additionalProperties": false
        }),
    }
}

/// Defines the prompt.list tool.
pub fn prompt_list_tool() -> ToolDefinition {
    ToolDefinition {
        name: "prompt_list".to_string(),
        description: "List saved prompt templates with optional filtering".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "domain": {
                    "type": "string",
                    "description": "Filter by domain scope",
                    "enum": ["project", "user", "org"]
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Filter by tags (AND logic - must have all)"
                },
                "name_pattern": {
                    "type": "string",
                    "description": "Filter by name pattern (glob-style, e.g., 'code-*')"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results",
                    "minimum": 1,
                    "maximum": 100,
                    "default": 20
                }
            },
            "required": []
        }),
    }
}

/// Defines the prompt.get tool.
pub fn prompt_get_tool() -> ToolDefinition {
    ToolDefinition {
        name: "prompt_get".to_string(),
        description: "Get a prompt template by name".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Prompt name to retrieve"
                },
                "domain": {
                    "type": "string",
                    "description": "Domain to search (if not specified, searches Project → User → Org)",
                    "enum": ["project", "user", "org"]
                }
            },
            "required": ["name"]
        }),
    }
}

/// Defines the prompt.run tool.
pub fn prompt_run_tool() -> ToolDefinition {
    ToolDefinition {
        name: "prompt_run".to_string(),
        description: "Run a saved prompt, substituting variable values".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Prompt name to run"
                },
                "variables": {
                    "type": "object",
                    "description": "Variable values to substitute (key: value pairs)",
                    "additionalProperties": { "type": "string" }
                },
                "domain": {
                    "type": "string",
                    "description": "Domain to search for the prompt",
                    "enum": ["project", "user", "org"]
                }
            },
            "required": ["name"]
        }),
    }
}

/// Defines the prompt.delete tool.
pub fn prompt_delete_tool() -> ToolDefinition {
    ToolDefinition {
        name: "prompt_delete".to_string(),
        description: "Delete a saved prompt template".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Prompt name to delete"
                },
                "domain": {
                    "type": "string",
                    "description": "Domain scope to delete from (required for safety)",
                    "enum": ["project", "user", "org"]
                }
            },
            "required": ["name", "domain"]
        }),
    }
}

// ============================================================================
// Core CRUD Tools (Industry Parity: Mem0, Zep, LangMem)
// ============================================================================

/// Defines the get tool for direct memory retrieval by ID.
///
/// This is a fundamental CRUD operation present in all major memory systems:
/// - Mem0: `get(memory_id)`
/// - Zep: `get_memory(session_id, memory_id)`
/// - `LangMem`: `get_memories()`
pub fn get_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_get".to_string(),
        description: "Get a memory by its ID. Returns the full memory content, metadata, and URN."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "memory_id": {
                    "type": "string",
                    "description": "The ID of the memory to retrieve"
                }
            },
            "required": ["memory_id"]
        }),
    }
}

/// Defines the delete tool for removing memories.
///
/// Supports both soft delete (tombstone) and hard delete (permanent).
/// Defaults to soft delete for safety, matching the CLI behavior.
pub fn delete_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_delete".to_string(),
        description: "Delete a memory by its ID. Defaults to soft delete (tombstone) which can be restored. Use hard=true for permanent deletion.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "memory_id": {
                    "type": "string",
                    "description": "The ID of the memory to delete"
                },
                "hard": {
                    "type": "boolean",
                    "description": "If true, permanently delete the memory. If false (default), soft delete (tombstone) - can be restored later.",
                    "default": false
                }
            },
            "required": ["memory_id"]
        }),
    }
}

/// Defines the update tool for modifying existing memories.
///
/// Allows partial updates to content and/or tags. Follows the industry
/// pattern of partial updates (Mem0 `update`, `LangMem` `update`).
pub fn update_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_update".to_string(),
        description: "Update an existing memory's content and/or tags. Provide only the fields you want to change.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "memory_id": {
                    "type": "string",
                    "description": "The ID of the memory to update"
                },
                "content": {
                    "type": "string",
                    "description": "New content for the memory (optional - omit to keep existing)"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "New tags for the memory (optional - omit to keep existing). Replaces all existing tags when provided."
                }
            },
            "required": ["memory_id"]
        }),
    }
}

/// Defines the list tool for listing all memories.
///
/// Lists memories with optional filtering and pagination.
/// Matches Mem0's `get_all()` and Zep's `list_memories()` patterns.
pub fn list_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_list".to_string(),
        description: "List all memories with optional filtering and pagination. Unlike recall, this doesn't require a search query.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "filter": {
                    "type": "string",
                    "description": "GitHub-style filter query: ns:decisions tag:rust -tag:test status:active"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 50, max: 1000)",
                    "minimum": 1,
                    "maximum": 1000,
                    "default": 50
                },
                "offset": {
                    "type": "integer",
                    "description": "Offset for pagination (default: 0)",
                    "minimum": 0,
                    "default": 0
                },
                "user_id": {
                    "type": "string",
                    "description": "Filter by user ID (for multi-tenant scoping)"
                },
                "agent_id": {
                    "type": "string",
                    "description": "Filter by agent ID (for multi-agent scoping)"
                }
            },
            "required": []
        }),
    }
}

/// Defines the `delete_all` tool for bulk deletion.
///
/// Bulk deletes memories matching filter criteria with dry-run safety.
/// Implements Mem0's `delete_all()` pattern.
pub fn delete_all_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_delete_all".to_string(),
        description: "Bulk delete memories matching filter criteria. Defaults to dry-run mode for safety - set dry_run=false to execute.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "filter": {
                    "type": "string",
                    "description": "GitHub-style filter query: ns:decisions tag:deprecated -tag:important"
                },
                "dry_run": {
                    "type": "boolean",
                    "description": "If true (default), show what would be deleted without making changes",
                    "default": true
                },
                "hard": {
                    "type": "boolean",
                    "description": "If true, permanently delete. If false (default), soft delete (tombstone).",
                    "default": false
                },
                "user_id": {
                    "type": "string",
                    "description": "Filter by user ID for scoped deletion"
                }
            },
            "required": []
        }),
    }
}

/// Defines the restore tool for recovering soft-deleted memories.
///
/// Restores a tombstoned memory back to active status.
/// Implements the inverse of soft delete for data recovery.
pub fn restore_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_restore".to_string(),
        description:
            "Restore a soft-deleted (tombstoned) memory. Returns the memory to active status."
                .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "memory_id": {
                    "type": "string",
                    "description": "The ID of the tombstoned memory to restore"
                }
            },
            "required": ["memory_id"]
        }),
    }
}

/// Defines the history tool for memory change audit trail.
///
/// Retrieves change history for a memory by querying the event log.
/// Provides audit trail visibility for compliance and debugging.
pub fn history_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_history".to_string(),
        description: "Get the change history for a memory. Shows creation, updates, and deletions from the audit log.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "memory_id": {
                    "type": "string",
                    "description": "The ID of the memory to get history for"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of events to return (default: 20)",
                    "minimum": 1,
                    "maximum": 100,
                    "default": 20
                }
            },
            "required": ["memory_id"]
        }),
    }
}

// ============================================================================
// Graph / Knowledge Graph Tools
// ============================================================================

/// Defines the entities tool for entity CRUD operations.
///
/// Provides operations to create, read, update, and delete entities
/// in the knowledge graph.
pub fn entities_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_entities".to_string(),
        description: "Manage entities in the knowledge graph. Supports CRUD operations for people, organizations, technologies, concepts, and files.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Operation to perform",
                    "enum": ["create", "get", "list", "delete"]
                },
                "entity_id": {
                    "type": "string",
                    "description": "Entity ID (required for get/delete)"
                },
                "name": {
                    "type": "string",
                    "description": "Entity name (required for create)"
                },
                "entity_type": {
                    "type": "string",
                    "description": "Type of entity",
                    "enum": ["Person", "Organization", "Technology", "Concept", "File"]
                },
                "aliases": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Alternative names for the entity"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum results for list operation (default: 20)",
                    "minimum": 1,
                    "maximum": 100,
                    "default": 20
                }
            },
            "required": ["action"]
        }),
    }
}

/// Defines the relationships tool for relationship CRUD operations.
///
/// Provides operations to create, read, and delete relationships
/// between entities in the knowledge graph.
pub fn relationships_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_relationships".to_string(),
        description: "Manage relationships between entities in the knowledge graph. Supports creating, querying, and deleting relationships like WorksAt, Uses, Created, etc.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Operation to perform",
                    "enum": ["create", "get", "list", "delete"]
                },
                "from_entity": {
                    "type": "string",
                    "description": "Source entity ID (required for create)"
                },
                "to_entity": {
                    "type": "string",
                    "description": "Target entity ID (required for create)"
                },
                "relationship_type": {
                    "type": "string",
                    "description": "Type of relationship",
                    "enum": ["WorksAt", "Created", "Uses", "Implements", "PartOf", "RelatesTo", "MentionedIn", "Supersedes", "ConflictsWith"]
                },
                "entity_id": {
                    "type": "string",
                    "description": "Entity ID to get relationships for (for get/list)"
                },
                "direction": {
                    "type": "string",
                    "description": "Relationship direction for get/list",
                    "enum": ["outgoing", "incoming", "both"],
                    "default": "both"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum results (default: 20)",
                    "minimum": 1,
                    "maximum": 100,
                    "default": 20
                }
            },
            "required": ["action"]
        }),
    }
}

/// Defines the graph query tool for traversing the knowledge graph.
///
/// Enables graph traversal operations like finding neighbors,
/// paths between entities, and subgraph extraction.
pub fn graph_query_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_graph_query".to_string(),
        description: "Query and traverse the knowledge graph. Find paths between entities, get neighbors at various depths, and explore connections.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "description": "Query operation to perform",
                    "enum": ["neighbors", "path", "stats"]
                },
                "entity_id": {
                    "type": "string",
                    "description": "Starting entity ID (required for neighbors)"
                },
                "from_entity": {
                    "type": "string",
                    "description": "Source entity ID (required for path)"
                },
                "to_entity": {
                    "type": "string",
                    "description": "Target entity ID (required for path)"
                },
                "depth": {
                    "type": "integer",
                    "description": "Traversal depth for neighbors/path (default: 2, max: 5)",
                    "minimum": 1,
                    "maximum": 5,
                    "default": 2
                }
            },
            "required": ["operation"]
        }),
    }
}

/// Defines the extract entities tool for LLM-powered entity extraction.
///
/// Extracts entities and relationships from text content using
/// LLM analysis with pattern-based fallback.
pub fn extract_entities_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_extract_entities".to_string(),
        description: "Extract entities and relationships from text using LLM. Identifies people, organizations, technologies, concepts, and their relationships. Falls back to pattern-based extraction if LLM unavailable.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "Text content to extract entities from"
                },
                "store": {
                    "type": "boolean",
                    "description": "Whether to store extracted entities in the graph (default: false)",
                    "default": false
                },
                "memory_id": {
                    "type": "string",
                    "description": "Optional memory ID to link extracted entities to"
                },
                "min_confidence": {
                    "type": "number",
                    "description": "Minimum confidence threshold (0.0-1.0, default: 0.5)",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.5
                }
            },
            "required": ["content"]
        }),
    }
}

/// Defines the entity merge tool for deduplicating entities.
///
/// Merges duplicate or similar entities into a single canonical entity,
/// preserving relationships and mentions.
pub fn entity_merge_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_entity_merge".to_string(),
        description: "Merge duplicate entities into a single canonical entity. Transfers all relationships and mentions to the merged entity. Use 'find_duplicates' to identify candidates first.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Operation to perform",
                    "enum": ["find_duplicates", "merge"]
                },
                "entity_id": {
                    "type": "string",
                    "description": "Entity ID to find duplicates for (for find_duplicates)"
                },
                "entity_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Entity IDs to merge (for merge, minimum 2)"
                },
                "canonical_name": {
                    "type": "string",
                    "description": "Name for the merged entity (required for merge)"
                },
                "threshold": {
                    "type": "number",
                    "description": "Similarity threshold for finding duplicates (0.0-1.0, default: 0.7)",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.7
                }
            },
            "required": ["action"]
        }),
    }
}

/// Defines the relationship inference tool for discovering implicit relationships.
///
/// Uses LLM analysis to infer relationships between existing entities
/// based on context and patterns.
pub fn relationship_infer_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_relationship_infer".to_string(),
        description: "Infer implicit relationships between entities using LLM analysis. Discovers connections based on entity types, names, and context. Falls back to heuristic-based inference if LLM unavailable.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "entity_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Entity IDs to analyze for relationships (optional, analyzes all recent if not provided)"
                },
                "store": {
                    "type": "boolean",
                    "description": "Whether to store inferred relationships in the graph (default: false)",
                    "default": false
                },
                "min_confidence": {
                    "type": "number",
                    "description": "Minimum confidence threshold for inferred relationships (0.0-1.0, default: 0.6)",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.6
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum entities to analyze if entity_ids not provided (default: 50)",
                    "minimum": 1,
                    "maximum": 200,
                    "default": 50
                }
            },
            "required": []
        }),
    }
}

/// Defines the graph visualize tool for generating graph visualizations.
///
/// Produces ASCII art, Mermaid diagrams, or DOT format representations
/// of the knowledge graph.
pub fn graph_visualize_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_graph_visualize".to_string(),
        description: "Visualize the knowledge graph or a subgraph. Generates Mermaid diagrams, DOT format, or ASCII art representation of entities and their relationships.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "description": "Output format for visualization",
                    "enum": ["mermaid", "dot", "ascii"],
                    "default": "mermaid"
                },
                "entity_id": {
                    "type": "string",
                    "description": "Center visualization on this entity (optional)"
                },
                "depth": {
                    "type": "integer",
                    "description": "Depth of relationships to include from center entity (default: 2)",
                    "minimum": 1,
                    "maximum": 4,
                    "default": 2
                },
                "entity_types": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": ["Person", "Organization", "Technology", "Concept", "File"]
                    },
                    "description": "Filter to specific entity types"
                },
                "relationship_types": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": ["WorksAt", "Created", "Uses", "Implements", "PartOf", "RelatesTo", "MentionedIn", "Supersedes", "ConflictsWith"]
                    },
                    "description": "Filter to specific relationship types"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum entities to include (default: 50)",
                    "minimum": 1,
                    "maximum": 200,
                    "default": 50
                }
            },
            "required": []
        }),
    }
}

/// Defines the consolidated graph tool for knowledge graph operations.
///
/// Combines query (neighbors, path, stats) and visualize operations
/// into a single action-based tool. Reduces tool proliferation.
pub fn graph_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_graph".to_string(),
        description: "Consolidated knowledge graph operations. Query graph structure (neighbors, paths, stats) or generate visualizations. Combines subcog_graph_query and subcog_graph_visualize.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "operation": {
                    "type": "string",
                    "description": "Graph operation to perform",
                    "enum": ["neighbors", "path", "stats", "visualize"]
                },
                "entity_id": {
                    "type": "string",
                    "description": "Entity ID for neighbors/visualize operations (center point)"
                },
                "from_entity": {
                    "type": "string",
                    "description": "Source entity ID for path operation"
                },
                "to_entity": {
                    "type": "string",
                    "description": "Target entity ID for path operation"
                },
                "depth": {
                    "type": "integer",
                    "description": "Depth of traversal for neighbors/visualize (default: 2)",
                    "minimum": 1,
                    "maximum": 4,
                    "default": 2
                },
                "format": {
                    "type": "string",
                    "description": "Output format for visualize operation",
                    "enum": ["mermaid", "dot", "ascii"],
                    "default": "mermaid"
                },
                "entity_types": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": ["Person", "Organization", "Technology", "Concept", "File"]
                    },
                    "description": "Filter to specific entity types"
                },
                "relationship_types": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": ["WorksAt", "Created", "Uses", "Implements", "PartOf", "RelatesTo", "MentionedIn", "Supersedes", "ConflictsWith"]
                    },
                    "description": "Filter to specific relationship types"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum entities to include (default: 50)",
                    "minimum": 1,
                    "maximum": 200,
                    "default": 50
                }
            },
            "required": ["operation"]
        }),
    }
}

/// Defines the init tool for session initialization.
///
/// Combines `prompt_understanding`, status, and optional context recall
/// into a single initialization call. Marks the session as initialized.
pub fn init_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_init".to_string(),
        description: "Initialize a Subcog session. Combines prompt_understanding (guidance), status (health check), and optional context recall into one call. Call this at the start of every session for optimal memory integration.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "include_recall": {
                    "type": "boolean",
                    "description": "Whether to recall project context (default: true)",
                    "default": true
                },
                "recall_query": {
                    "type": "string",
                    "description": "Custom recall query (default: 'project setup OR architecture OR conventions')"
                },
                "recall_limit": {
                    "type": "integer",
                    "description": "Maximum memories to recall (default: 5)",
                    "minimum": 1,
                    "maximum": 20,
                    "default": 5
                }
            },
            "required": []
        }),
    }
}

// ============================================================================
// Context Template Tools
// ============================================================================

/// Defines the consolidated `subcog_templates` tool.
///
/// Combines all context template operations into a single action-based tool.
pub fn templates_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_templates".to_string(),
        description: "Manage context templates for formatting memories. Actions: save, list, get, render, delete.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Operation to perform",
                    "enum": ["save", "list", "get", "render", "delete"]
                },
                "name": {
                    "type": "string",
                    "description": "Template name (required for save/get/render/delete)"
                },
                "content": {
                    "type": "string",
                    "description": "Template content with {{variable}} placeholders and {{#each memories}} iteration (for save)"
                },
                "description": {
                    "type": "string",
                    "description": "Human-readable description (for save)"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags for categorization (for save/list)"
                },
                "domain": {
                    "type": "string",
                    "description": "Storage scope: project (default), user, or org",
                    "enum": ["project", "user", "org"],
                    "default": "project"
                },
                "output_format": {
                    "type": "string",
                    "description": "Default output format (for save): markdown, json, or xml",
                    "enum": ["markdown", "json", "xml"],
                    "default": "markdown"
                },
                "variables_def": {
                    "type": "array",
                    "description": "Variable definitions with metadata (for save)",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "description": { "type": "string" },
                            "default": { "type": "string" },
                            "required": { "type": "boolean", "default": true }
                        },
                        "required": ["name"]
                    }
                },
                "variables": {
                    "type": "object",
                    "description": "Custom variable values for rendering (for render)",
                    "additionalProperties": { "type": "string" }
                },
                "name_pattern": {
                    "type": "string",
                    "description": "Filter by name pattern (for list, glob-style)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum results (for list) or memories (for render)",
                    "minimum": 1,
                    "maximum": 100,
                    "default": 20
                },
                "version": {
                    "type": "integer",
                    "description": "Specific version (for get/render/delete)",
                    "minimum": 1
                },
                "query": {
                    "type": "string",
                    "description": "Query string for memory search to populate template (for render)"
                },
                "namespaces": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Namespaces to filter memories (for render)"
                },
                "format": {
                    "type": "string",
                    "description": "Output format override (for render): markdown, json, or xml",
                    "enum": ["markdown", "json", "xml"]
                }
            },
            "required": ["action"]
        }),
    }
}

/// Defines the `context_template_save` tool (legacy).
pub fn context_template_save_tool() -> ToolDefinition {
    ToolDefinition {
        name: "context_template_save".to_string(),
        description: "Save or update a context template for formatting memories. Templates support variable substitution ({{var}}), iteration ({{#each memories}}...{{/each}}), and multiple output formats.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Template name (unique identifier)"
                },
                "content": {
                    "type": "string",
                    "description": "Template content with variable placeholders. Use {{var}} for variables, {{#each memories}}...{{/each}} for iteration over memories."
                },
                "description": {
                    "type": "string",
                    "description": "Human-readable description of the template's purpose"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags for categorization and filtering"
                },
                "domain": {
                    "type": "string",
                    "description": "Storage scope: project (default), user, or org",
                    "enum": ["project", "user", "org"],
                    "default": "project"
                },
                "output_format": {
                    "type": "string",
                    "description": "Default output format: markdown (default), json, or xml",
                    "enum": ["markdown", "json", "xml"],
                    "default": "markdown"
                },
                "variables": {
                    "type": "array",
                    "description": "User-defined variable declarations",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": {
                                "type": "string",
                                "description": "Variable name (used as {{name}} in content)"
                            },
                            "description": {
                                "type": "string",
                                "description": "Description of what this variable represents"
                            },
                            "default": {
                                "type": "string",
                                "description": "Default value if not provided at render time"
                            },
                            "required": {
                                "type": "boolean",
                                "description": "Whether this variable must be provided (default: true)"
                            }
                        },
                        "required": ["name"]
                    }
                }
            },
            "required": ["name", "content"]
        }),
    }
}

/// Defines the `context_template_list` tool.
pub fn context_template_list_tool() -> ToolDefinition {
    ToolDefinition {
        name: "context_template_list".to_string(),
        description: "List available context templates with optional filtering by domain, tags, or name pattern.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "domain": {
                    "type": "string",
                    "description": "Filter by storage scope: project, user, or org",
                    "enum": ["project", "user", "org"]
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Filter by tags (must have ALL specified tags)"
                },
                "name_pattern": {
                    "type": "string",
                    "description": "Filter by name pattern (substring match)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 20, max: 100)",
                    "minimum": 1,
                    "maximum": 100,
                    "default": 20
                }
            },
            "required": []
        }),
    }
}

/// Defines the `context_template_get` tool.
pub fn context_template_get_tool() -> ToolDefinition {
    ToolDefinition {
        name: "context_template_get".to_string(),
        description: "Get a context template by name, optionally specifying version and domain."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Template name to retrieve"
                },
                "version": {
                    "type": "integer",
                    "description": "Specific version to retrieve (default: latest)",
                    "minimum": 1
                },
                "domain": {
                    "type": "string",
                    "description": "Storage scope to search: project, user, or org",
                    "enum": ["project", "user", "org"]
                }
            },
            "required": ["name"]
        }),
    }
}

/// Defines the `context_template_render` tool.
pub fn context_template_render_tool() -> ToolDefinition {
    ToolDefinition {
        name: "context_template_render".to_string(),
        description: "Render a context template with memories from a search query. Combines template rendering with memory recall for formatted output.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Template name to render"
                },
                "version": {
                    "type": "integer",
                    "description": "Specific template version (default: latest)",
                    "minimum": 1
                },
                "query": {
                    "type": "string",
                    "description": "Search query to find memories for the template"
                },
                "namespaces": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": ["decisions", "patterns", "learnings", "context", "tech-debt", "apis", "config", "security", "performance", "testing"]
                    },
                    "description": "Filter memories by namespaces"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum memories to include (default: 10)",
                    "minimum": 1,
                    "maximum": 50,
                    "default": 10
                },
                "format": {
                    "type": "string",
                    "description": "Override output format: markdown, json, or xml",
                    "enum": ["markdown", "json", "xml"]
                },
                "variables": {
                    "type": "object",
                    "description": "Custom variable values as key-value pairs",
                    "additionalProperties": { "type": "string" }
                }
            },
            "required": ["name"]
        }),
    }
}

/// Defines the `context_template_delete` tool.
pub fn context_template_delete_tool() -> ToolDefinition {
    ToolDefinition {
        name: "context_template_delete".to_string(),
        description: "Delete a context template. Can delete a specific version or all versions."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Template name to delete"
                },
                "version": {
                    "type": "integer",
                    "description": "Specific version to delete (omit to delete all versions)",
                    "minimum": 1
                },
                "domain": {
                    "type": "string",
                    "description": "Storage scope: project (default), user, or org",
                    "enum": ["project", "user", "org"],
                    "default": "project"
                }
            },
            "required": ["name", "domain"]
        }),
    }
}

// ============================================================================
// Group Management Tools (Feature-gated: group-scope)
// ============================================================================

/// Defines the `group_create` tool for creating new groups.
#[cfg(feature = "group-scope")]
pub fn group_create_tool() -> super::ToolDefinition {
    super::ToolDefinition {
        name: "subcog_group_create".to_string(),
        description:
            "Create a new group for shared memory access. You become the admin of the group."
                .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Group name (must be unique)"
                },
                "description": {
                    "type": "string",
                    "description": "Optional description of the group's purpose"
                }
            },
            "required": ["name"]
        }),
    }
}

/// Defines the `group_list` tool for listing groups.
#[cfg(feature = "group-scope")]
pub fn group_list_tool() -> super::ToolDefinition {
    super::ToolDefinition {
        name: "subcog_group_list".to_string(),
        description: "List all groups you have access to, including your role in each.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// Defines the `group_get` tool for getting group details.
#[cfg(feature = "group-scope")]
pub fn group_get_tool() -> super::ToolDefinition {
    super::ToolDefinition {
        name: "subcog_group_get".to_string(),
        description: "Get details of a specific group, including members and their roles."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "group_id": {
                    "type": "string",
                    "description": "The ID of the group to retrieve"
                }
            },
            "required": ["group_id"]
        }),
    }
}

/// Defines the `group_add_member` tool for adding members to a group.
#[cfg(feature = "group-scope")]
pub fn group_add_member_tool() -> super::ToolDefinition {
    super::ToolDefinition {
        name: "subcog_group_add_member".to_string(),
        description:
            "Add a member to a group with a specified role. Requires admin role in the group."
                .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "group_id": {
                    "type": "string",
                    "description": "The ID of the group"
                },
                "user_id": {
                    "type": "string",
                    "description": "The user ID to add"
                },
                "role": {
                    "type": "string",
                    "description": "Role for the new member",
                    "enum": ["read", "write", "admin"],
                    "default": "read"
                }
            },
            "required": ["group_id", "user_id"]
        }),
    }
}

/// Defines the `group_remove_member` tool for removing members from a group.
#[cfg(feature = "group-scope")]
pub fn group_remove_member_tool() -> super::ToolDefinition {
    super::ToolDefinition {
        name: "subcog_group_remove_member".to_string(),
        description: "Remove a member from a group. Requires admin role in the group.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "group_id": {
                    "type": "string",
                    "description": "The ID of the group"
                },
                "user_id": {
                    "type": "string",
                    "description": "The user ID to remove"
                }
            },
            "required": ["group_id", "user_id"]
        }),
    }
}

/// Defines the `group_update_role` tool for updating a member's role.
#[cfg(feature = "group-scope")]
pub fn group_update_role_tool() -> super::ToolDefinition {
    super::ToolDefinition {
        name: "subcog_group_update_role".to_string(),
        description: "Update a member's role in a group. Requires admin role in the group."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "group_id": {
                    "type": "string",
                    "description": "The ID of the group"
                },
                "user_id": {
                    "type": "string",
                    "description": "The user ID to update"
                },
                "role": {
                    "type": "string",
                    "description": "New role for the member",
                    "enum": ["read", "write", "admin"]
                }
            },
            "required": ["group_id", "user_id", "role"]
        }),
    }
}

/// Defines the `group_delete` tool for deleting a group.
#[cfg(feature = "group-scope")]
pub fn group_delete_tool() -> super::ToolDefinition {
    super::ToolDefinition {
        name: "subcog_group_delete".to_string(),
        description: "Delete a group. Requires admin role. Warning: This does not delete memories, but they will no longer be group-accessible.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "group_id": {
                    "type": "string",
                    "description": "The ID of the group to delete"
                }
            },
            "required": ["group_id"]
        }),
    }
}

/// Defines the consolidated `subcog_groups` tool.
///
/// Combines all group management operations into a single action-based tool.
#[cfg(feature = "group-scope")]
pub fn groups_tool() -> super::ToolDefinition {
    super::ToolDefinition {
        name: "subcog_groups".to_string(),
        description: "Manage groups for shared memory access. Actions: create, list, get, add_member, remove_member, update_role, delete.".to_string(),
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
                    "description": "Group description (for create)"
                },
                "user_id": {
                    "type": "string",
                    "description": "User ID to add/remove/update (for add_member/remove_member/update_role)"
                },
                "role": {
                    "type": "string",
                    "description": "Role for the member (for add_member/update_role)",
                    "enum": ["read", "write", "admin"],
                    "default": "read"
                }
            },
            "required": ["action"]
        }),
    }
}
