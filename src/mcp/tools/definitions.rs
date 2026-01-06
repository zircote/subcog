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
                }
            },
            "required": ["content", "namespace"]
        }),
    }
}

/// Defines the recall tool.
pub fn recall_tool() -> ToolDefinition {
    ToolDefinition {
        name: "subcog_recall".to_string(),
        description: "Search for relevant memories using semantic and text search. Returns normalized scores (0.0-1.0 where 1.0 is the best match) with raw RRF scores shown in parentheses for debugging.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
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
                    "description": "Maximum number of results (default: 10)",
                    "minimum": 1,
                    "maximum": 50
                }
            },
            "required": ["query"]
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
        description: "Consolidate related memories using LLM to merge and summarize. Uses MCP sampling to request LLM completion.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "namespace": {
                    "type": "string",
                    "description": "Namespace to consolidate",
                    "enum": ["decisions", "patterns", "learnings", "context", "tech-debt", "apis", "config", "security", "performance", "testing"]
                },
                "query": {
                    "type": "string",
                    "description": "Optional query to filter memories for consolidation"
                },
                "strategy": {
                    "type": "string",
                    "description": "Consolidation strategy: merge (combine similar), summarize (create summary), dedupe (remove duplicates)",
                    "enum": ["merge", "summarize", "dedupe"],
                    "default": "merge"
                },
                "dry_run": {
                    "type": "boolean",
                    "description": "If true, show what would be consolidated without making changes",
                    "default": false
                }
            },
            "required": ["namespace"]
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

// ============================================================================
// Prompt Management Tools
// ============================================================================

/// Defines the prompt.save tool.
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
                    "description": "Prompt content with {{variable}} placeholders (required if file_path not provided, unless merge is true)"
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to file containing prompt (alternative to content; required if content missing unless merge is true)"
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
                },
                "merge": {
                    "type": "boolean",
                    "description": "Merge with existing prompt metadata when updating (default: false)",
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
