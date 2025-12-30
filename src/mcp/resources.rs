//! MCP resource handlers.
//!
//! Provides resource access for the Model Context Protocol.
//! Resources are accessed via URN scheme:
//!
//! ## Help Resources
//! - `subcog://help` - Help index
//! - `subcog://help/{topic}` - Topic-specific help
//!
//! ## Memory Resources
//! - `subcog://_` - All memories across all domains
//! - `subcog://_/{namespace}` - All memories in a namespace (e.g., `subcog://_/learnings`)
//! - `subcog://memory/{id}` - Get a specific memory by ID
//!
//! ## Domain-Scoped Resources (future)
//! - `subcog://project/_` - Project-scoped memories only
//! - `subcog://org/{org}/_` - Organization-scoped memories
//! - `subcog://global/_` - Global memories
//!
//! For advanced filtering and discovery, use the `subcog_browse` prompt
//! which supports filtering by namespace, tags, time, source, and status.

use crate::Namespace;
use crate::services::RecallService;
use crate::{Error, Result, SearchFilter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Handler for MCP resources (URN scheme).
pub struct ResourceHandler {
    /// Help content by category.
    help_content: HashMap<String, HelpCategory>,
    /// Optional recall service for memory browsing.
    recall_service: Option<RecallService>,
}

impl ResourceHandler {
    /// Creates a new resource handler.
    #[must_use]
    pub fn new() -> Self {
        let mut help_content = HashMap::new();

        // Setup category
        help_content.insert(
            "setup".to_string(),
            HelpCategory {
                name: "setup".to_string(),
                title: "Getting Started with Subcog".to_string(),
                description: "Installation and initial configuration guide".to_string(),
                content: HELP_SETUP.to_string(),
            },
        );

        // Concepts category
        help_content.insert(
            "concepts".to_string(),
            HelpCategory {
                name: "concepts".to_string(),
                title: "Core Concepts".to_string(),
                description: "Understanding namespaces, domains, URNs, and memory lifecycle"
                    .to_string(),
                content: HELP_CONCEPTS.to_string(),
            },
        );

        // Capture category
        help_content.insert(
            "capture".to_string(),
            HelpCategory {
                name: "capture".to_string(),
                title: "Capturing Memories".to_string(),
                description: "How to capture and store memories effectively".to_string(),
                content: HELP_CAPTURE.to_string(),
            },
        );

        // Search category
        help_content.insert(
            "search".to_string(),
            HelpCategory {
                name: "search".to_string(),
                title: "Searching Memories".to_string(),
                description: "Using hybrid search to find relevant memories".to_string(),
                content: HELP_SEARCH.to_string(),
            },
        );

        // Workflows category
        help_content.insert(
            "workflows".to_string(),
            HelpCategory {
                name: "workflows".to_string(),
                title: "Integration Workflows".to_string(),
                description: "Hooks, MCP server, and IDE integration".to_string(),
                content: HELP_WORKFLOWS.to_string(),
            },
        );

        // Troubleshooting category
        help_content.insert(
            "troubleshooting".to_string(),
            HelpCategory {
                name: "troubleshooting".to_string(),
                title: "Troubleshooting".to_string(),
                description: "Common issues and solutions".to_string(),
                content: HELP_TROUBLESHOOTING.to_string(),
            },
        );

        // Advanced category
        help_content.insert(
            "advanced".to_string(),
            HelpCategory {
                name: "advanced".to_string(),
                title: "Advanced Features".to_string(),
                description: "LLM integration, consolidation, and optimization".to_string(),
                content: HELP_ADVANCED.to_string(),
            },
        );

        Self {
            help_content,
            recall_service: None,
        }
    }

    /// Creates a resource handler with a recall service for memory browsing.
    #[must_use]
    pub fn with_recall(recall_service: RecallService) -> Self {
        let mut handler = Self::new();
        handler.recall_service = Some(recall_service);
        handler
    }

    /// Lists all available resources.
    ///
    /// Returns resources organized by type:
    /// - Help topics
    /// - Memory browsing patterns
    ///
    /// For advanced filtering, use the `subcog_browse` prompt.
    #[must_use]
    pub fn list_resources(&self) -> Vec<ResourceDefinition> {
        let mut resources: Vec<ResourceDefinition> = self
            .help_content
            .values()
            .map(|cat| ResourceDefinition {
                uri: format!("subcog://help/{}", cat.name),
                name: cat.title.clone(),
                description: Some(cat.description.clone()),
                mime_type: Some("text/markdown".to_string()),
            })
            .collect();

        // All memories across all domains
        resources.push(ResourceDefinition {
            uri: "subcog://_".to_string(),
            name: "All Memories".to_string(),
            description: Some("All memories across all domains".to_string()),
            mime_type: Some("application/json".to_string()),
        });

        // Namespace-scoped patterns
        for ns in Namespace::user_namespaces() {
            let ns_str = ns.as_str();
            resources.push(ResourceDefinition {
                uri: format!("subcog://_/{ns_str}"),
                name: format!("{ns_str} memories"),
                description: Some(format!("All memories in {ns_str} namespace")),
                mime_type: Some("application/json".to_string()),
            });
        }

        resources
    }

    /// Gets a resource by URI.
    ///
    /// Supported URI patterns:
    /// - `subcog://help` - Help index
    /// - `subcog://help/{topic}` - Help topic
    /// - `subcog://_` - All memories across all domains
    /// - `subcog://_/{namespace}` - All memories in a namespace
    /// - `subcog://memory/{id}` - Get specific memory by ID
    /// - `subcog://project/_` - Project-scoped memories (alias for `subcog://_`)
    ///
    /// For advanced filtering, use the `subcog_browse` prompt instead.
    ///
    /// # Errors
    ///
    /// Returns an error if the resource is not found.
    pub fn get_resource(&self, uri: &str) -> Result<ResourceContent> {
        let uri = uri.trim();

        if !uri.starts_with("subcog://") {
            return Err(Error::InvalidInput(format!("Invalid URI scheme: {uri}")));
        }

        let path = &uri["subcog://".len()..];
        let parts: Vec<&str> = path.split('/').collect();

        if parts.is_empty() {
            return Err(Error::InvalidInput("Empty resource path".to_string()));
        }

        match parts[0] {
            "help" => self.get_help_resource(uri, &parts),
            "_" => self.get_all_memories_resource(uri, &parts),
            "project" => self.get_all_memories_resource(uri, &parts), // Alias for now
            "memory" => self.get_memory_resource(uri, &parts),
            _ => Err(Error::InvalidInput(format!(
                "Unknown resource type: {}. Valid: _, help, memory, project",
                parts[0]
            ))),
        }
    }

    /// Gets a help resource.
    fn get_help_resource(&self, uri: &str, parts: &[&str]) -> Result<ResourceContent> {
        if parts.len() == 1 {
            // Return help index
            return Ok(ResourceContent {
                uri: uri.to_string(),
                mime_type: Some("text/markdown".to_string()),
                text: Some(self.get_help_index()),
                blob: None,
            });
        }

        let category = parts[1];
        let content = self
            .help_content
            .get(category)
            .ok_or_else(|| Error::InvalidInput(format!("Unknown help category: {category}")))?;

        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("text/markdown".to_string()),
            text: Some(format!("# {}\n\n{}", content.title, content.content)),
            blob: None,
        })
    }

    /// Gets all memories resource with optional namespace filter.
    ///
    /// URI patterns:
    /// - `subcog://_` - All memories across all domains
    /// - `subcog://_/{namespace}` - All memories in a namespace
    /// - `subcog://project/_` - Alias for `subcog://_` (project-scoped, future domain filter)
    ///
    /// For advanced filtering, use the `subcog_browse` prompt.
    fn get_all_memories_resource(&self, uri: &str, parts: &[&str]) -> Result<ResourceContent> {
        let recall = self.recall_service.as_ref().ok_or_else(|| {
            Error::InvalidInput("Memory browsing requires RecallService".to_string())
        })?;

        // Parse namespace filter from URI
        // subcog://_ -> no filter
        // subcog://_/learnings -> filter by namespace
        // subcog://project/_ -> no filter (legacy)
        let namespace_filter = if parts[0] == "_" && parts.len() >= 2 {
            Some(parts[1])
        } else {
            None
        };

        // Build filter
        let mut filter = SearchFilter::new();
        if let Some(ns_str) = namespace_filter {
            let ns = Namespace::parse(ns_str)
                .ok_or_else(|| Error::InvalidInput(format!("Unknown namespace: {ns_str}")))?;
            filter = filter.with_namespace(ns);
        }

        let results = recall.list_all(&filter, 500)?;

        // Bare minimum for informed selection: id, ns, tags, uri
        let memories: Vec<serde_json::Value> = results
            .memories
            .iter()
            .map(|hit| {
                serde_json::json!({
                    "id": hit.memory.id.as_str(),
                    "ns": hit.memory.namespace.as_str(),
                    "tags": hit.memory.tags,
                    "uri": format!("subcog://memory/{}", hit.memory.id.as_str()),
                })
            })
            .collect();

        let response = serde_json::json!({
            "count": memories.len(),
            "memories": memories,
        });

        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json".to_string()),
            text: Some(serde_json::to_string_pretty(&response).unwrap_or_default()),
            blob: None,
        })
    }

    /// Gets a specific memory by ID with full content (cross-domain lookup).
    ///
    /// This is the targeted fetch endpoint - returns complete memory data.
    /// Use `subcog://memory/{id}` for cross-domain lookups when ID is known.
    fn get_memory_resource(&self, uri: &str, parts: &[&str]) -> Result<ResourceContent> {
        use crate::models::MemoryId;

        if parts.len() < 2 {
            return Err(Error::InvalidInput(
                "Memory ID required: subcog://memory/{id}".to_string(),
            ));
        }

        let memory_id = parts[1];
        let recall = self.recall_service.as_ref().ok_or_else(|| {
            Error::InvalidInput("Memory browsing requires RecallService".to_string())
        })?;

        // Direct fetch by ID - returns full content
        let memory = recall
            .get_by_id(&MemoryId::new(memory_id))?
            .ok_or_else(|| Error::InvalidInput(format!("Memory not found: {memory_id}")))?;

        self.format_memory_response(uri, &memory)
    }

    /// Formats a memory as a JSON response.
    fn format_memory_response(
        &self,
        uri: &str,
        memory: &crate::models::Memory,
    ) -> Result<ResourceContent> {
        let response = serde_json::json!({
            "id": memory.id.as_str(),
            "namespace": memory.namespace.as_str(),
            "domain": memory.domain.to_string(),
            "content": memory.content,
            "tags": memory.tags,
            "source": memory.source,
            "status": memory.status.as_str(),
            "created_at": memory.created_at,
            "updated_at": memory.updated_at,
        });

        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json".to_string()),
            text: Some(serde_json::to_string_pretty(&response).unwrap_or_default()),
            blob: None,
        })
    }

    /// Gets the help index listing all categories.
    fn get_help_index(&self) -> String {
        let mut index = "# Subcog Help\n\nWelcome to Subcog, the persistent memory system for AI coding assistants.\n\n## Available Topics\n\n".to_string();

        for cat in self.help_content.values() {
            index.push_str(&format!(
                "- **[{}](subcog://help/{})**: {}\n",
                cat.title, cat.name, cat.description
            ));
        }

        index.push_str("\n## Quick Start (MCP Tools)\n\n");
        index
            .push_str("1. **Capture**: Use `subcog_capture` tool with `namespace` and `content`\n");
        index.push_str("2. **Search**: Use `subcog_recall` tool with `query` parameter\n");
        index.push_str("3. **Status**: Use `subcog_status` tool\n");
        index.push_str(
            "4. **Browse**: Use `subcog_browse` prompt or `subcog://project/_` resource\n",
        );

        index
    }

    /// Gets a list of help categories.
    #[must_use]
    pub fn list_categories(&self) -> Vec<&HelpCategory> {
        self.help_content.values().collect()
    }
}

impl Default for ResourceHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Definition of an MCP resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDefinition {
    /// Resource URI.
    pub uri: String,
    /// Human-readable name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// MIME type of the resource.
    pub mime_type: Option<String>,
}

/// Content of an MCP resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    /// Resource URI.
    pub uri: String,
    /// MIME type.
    pub mime_type: Option<String>,
    /// Text content (for text resources).
    pub text: Option<String>,
    /// Binary content as base64 (for binary resources).
    pub blob: Option<String>,
}

/// Help category definition.
#[derive(Debug, Clone)]
pub struct HelpCategory {
    /// Category identifier.
    pub name: String,
    /// Human-readable title.
    pub title: String,
    /// Short description.
    pub description: String,
    /// Full content in Markdown.
    pub content: String,
}

// Help content constants
// Note: Use r"..."# for strings containing quotes, r"..." for those without

const HELP_SETUP: &str = r#"
## MCP Server Configuration

Subcog exposes tools, resources, and prompts via the Model Context Protocol (MCP).

### Claude Desktop Setup

Add to `~/.config/claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "subcog": {
      "command": "subcog",
      "args": ["serve"]
    }
  }
}
```

### Claude Code Plugin Setup

Add to `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "subcog": {
      "command": "subcog",
      "args": ["serve"]
    }
  }
}
```

### Configuration File

Create `~/.config/subcog/config.yaml`:

```yaml
data_dir: ~/.subcog
features:
  redact_secrets: true
  block_secrets: false
  auto_sync: true
```

## Available MCP Tools

Once configured, these tools are available:

| Tool | Description |
|------|-------------|
| `subcog_capture` | Capture a memory |
| `subcog_recall` | Search memories |
| `subcog_status` | Check system status |
| `subcog_consolidate` | Consolidate memories (LLM) |
| `subcog_enrich` | Enrich a memory (LLM) |
| `subcog_sync` | Sync with git remote |

## Available MCP Resources

| Resource | Description |
|----------|-------------|
| `subcog://help` | Help index |
| `subcog://help/{topic}` | Topic-specific help |
| `subcog://project/_` | List all memories |
| `subcog://memory/{id}` | Get specific memory |

## Available MCP Prompts

| Prompt | Description |
|--------|-------------|
| `subcog_browse` | Interactive memory browser with faceted discovery |
| `subcog_list` | Formatted memory listing with filtering |
| `subcog_tutorial` | Interactive learning guide |
| `subcog_capture_assistant` | Help decide what to capture |
| `subcog_review` | Review and consolidate memories |
| `subcog_search_help` | Craft effective search queries |

## Filter Syntax (for browse/list)

```
ns:decisions          # filter by namespace
tag:rust              # filter by tag
tag:rust,mcp          # OR (any tag)
tag:rust tag:error    # AND (all tags)
-tag:test             # exclude tag
since:7d              # last 7 days
source:src/*          # source path
```
"#;

const HELP_CONCEPTS: &str = r"
## Namespaces

Memories are organized into namespaces:

| Namespace | Purpose |
|-----------|---------|
| `decisions` | Architectural and design decisions |
| `patterns` | Discovered patterns and conventions |
| `learnings` | Lessons learned from debugging |
| `context` | Important background information |
| `tech-debt` | Technical debt tracking |
| `apis` | API documentation and contracts |
| `config` | Configuration details |
| `security` | Security findings and notes |
| `performance` | Optimization notes |
| `testing` | Test strategies and edge cases |

## Domains

Domains provide scope isolation:

- **Global**: Shared across all projects
- **Organization**: Shared within an org (e.g., `zircote`)
- **Repository**: Specific to a repo (e.g., `zircote/subcog`)

## URN Scheme

Memories are addressed via URNs:

```
urn:subcog:{domain}:{namespace}:{id}
```

Example:
```
urn:subcog:zircote:subcog:decisions:decisions_abc123
```

## Memory Lifecycle

1. **Active**: Default state, fully searchable
2. **Archived**: Less frequently accessed
3. **Superseded**: Replaced by newer memory
4. **Pending**: Awaiting review
5. **Deleted**: Marked for cleanup
";

const HELP_CAPTURE: &str = r#"
## Using the subcog_capture Tool

### Basic Capture

```json
{
  "tool": "subcog_capture",
  "arguments": {
    "namespace": "decisions",
    "content": "Use PostgreSQL for primary storage"
  }
}
```

### With Tags

```json
{
  "tool": "subcog_capture",
  "arguments": {
    "namespace": "patterns",
    "content": "Use thiserror for custom error types",
    "tags": ["rust", "error-handling"]
  }
}
```

### With Source Reference

```json
{
  "tool": "subcog_capture",
  "arguments": {
    "namespace": "learnings",
    "content": "JWT validation requires explicit algorithm specification",
    "source": "src/auth.rs:42"
  }
}
```

## Tool Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| `namespace` | Yes | One of: decisions, patterns, learnings, context, tech-debt, apis, config, security, performance, testing |
| `content` | Yes | The memory content to capture |
| `tags` | No | Array of tags for categorization |
| `source` | No | Source file reference (e.g., "src/auth.rs:42") |

## Best Practices

1. **Be Specific**: Include context and rationale
2. **Use Tags**: Add relevant keywords for better search
3. **Reference Sources**: Link to code or documentation
4. **Choose Correct Namespace**: Match content to category

## Namespace Selection Guide

| Signal Words | Namespace |
|--------------|-----------|
| "decided", "chose", "going with" | `decisions` |
| "always", "never", "convention" | `patterns` |
| "TIL", "learned", "discovered" | `learnings` |
| "because", "constraint" | `context` |
| "TODO", "FIXME", "temporary" | `tech-debt` |
"#;

const HELP_SEARCH: &str = r#"
## Using the subcog_recall Tool

### Basic Search

```json
{
  "tool": "subcog_recall",
  "arguments": {
    "query": "database storage decision"
  }
}
```

### Search Modes

#### Hybrid (Default)
Combines vector similarity and BM25 text search with RRF fusion:

```json
{
  "tool": "subcog_recall",
  "arguments": {
    "query": "authentication patterns",
    "mode": "hybrid"
  }
}
```

#### Vector Only
Pure semantic similarity search:

```json
{
  "tool": "subcog_recall",
  "arguments": {
    "query": "how to handle errors",
    "mode": "vector"
  }
}
```

#### Text Only
Traditional keyword search with BM25 ranking:

```json
{
  "tool": "subcog_recall",
  "arguments": {
    "query": "PostgreSQL",
    "mode": "text"
  }
}
```

### Filtering by Namespace

```json
{
  "tool": "subcog_recall",
  "arguments": {
    "query": "storage",
    "namespace": "decisions"
  }
}
```

### Limiting Results

```json
{
  "tool": "subcog_recall",
  "arguments": {
    "query": "API design",
    "limit": 5
  }
}
```

## Tool Parameters

| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `query` | Yes | - | Natural language search query |
| `mode` | No | `hybrid` | Search mode: `hybrid`, `vector`, or `text` |
| `namespace` | No | all | Filter by namespace |
| `limit` | No | 10 | Maximum results (max: 50) |

## Browsing Memories via Resources

Access memories directly via MCP resources:

- `subcog://project/_` - List all memories
- `subcog://memory/{id}` - Get specific memory by ID

For advanced filtering by namespace, tags, time, etc., use the `subcog_browse` prompt.

## Understanding Scores

| Score Range | Relevance |
|-------------|-----------|
| 0.9+ | Very high (likely exact match) |
| 0.7-0.9 | Good (closely related) |
| 0.5-0.7 | Moderate (broader context) |
| <0.5 | Low (tangential) |
"#;

const HELP_WORKFLOWS: &str = r#"
## Common MCP Workflows

### Session Start: Load Context

At session start, search for relevant memories based on the current project:

```json
{
  "tool": "subcog_recall",
  "arguments": {
    "query": "current project context patterns decisions",
    "limit": 10
  }
}
```

### During Work: Capture Insights

When you discover something worth remembering:

```json
{
  "tool": "subcog_capture",
  "arguments": {
    "namespace": "learnings",
    "content": "JWT tokens must specify algorithm explicitly to prevent alg:none attacks",
    "tags": ["security", "jwt", "authentication"]
  }
}
```

### Related Context: Find Similar

When working on a topic, find related memories:

```json
{
  "tool": "subcog_recall",
  "arguments": {
    "query": "authentication security patterns",
    "mode": "hybrid",
    "namespace": "patterns"
  }
}
```

### Session End: Sync Changes

Sync memories to the git remote:

```json
{
  "tool": "subcog_sync",
  "arguments": {
    "direction": "full"
  }
}
```

## Browsing via Resources

Access memories directly without search:

| Resource URI | Returns |
|--------------|---------|
| `subcog://project/_` | All memories (JSON) |
| `subcog://memory/{id}` | Specific memory by ID |

For filtering by namespace, tags, time, etc., use the `subcog_browse` prompt.

## Status Check

Monitor system health:

```json
{
  "tool": "subcog_status",
  "arguments": {}
}
```

Returns: memory count, index status, sync state, storage backend info.
"#;

const HELP_TROUBLESHOOTING: &str = r#"
## Common Issues

### Tool Returns Empty Results

If `subcog_recall` returns no results:

1. **Check status**: Use `subcog_status` tool to verify index exists
2. **Try broader query**: Use simpler search terms
3. **Check namespace**: Remove namespace filter to search all

```json
{
  "tool": "subcog_status",
  "arguments": {}
}
```

### "Secret detected" Error

The `subcog_capture` tool blocked content with potential secrets:

1. Remove the secret from content
2. Check `~/.config/subcog/config.yaml`:
   - `block_secrets: false` to allow (not recommended)
   - `redact_secrets: true` to auto-redact

### "Index not found"

Call the status tool to trigger initialization:

```json
{
  "tool": "subcog_status",
  "arguments": {}
}
```

### Slow Search Performance

1. Reduce `limit` parameter (default 10, max 50)
2. Use `mode: "text"` for faster keyword-only search
3. Add `namespace` filter to narrow scope

```json
{
  "tool": "subcog_recall",
  "arguments": {
    "query": "specific term",
    "mode": "text",
    "namespace": "decisions",
    "limit": 5
  }
}
```

### Sync Failures

Check sync status and retry:

```json
{
  "tool": "subcog_sync",
  "arguments": {
    "direction": "fetch"
  }
}
```

If push fails, ensure the git remote is configured and you have write access.

## Report Issues

GitHub: https://github.com/zircote/subcog/issues
"#;

const HELP_ADVANCED: &str = r#"
## LLM-Powered Tools

These tools require an LLM provider configured in `~/.config/subcog/config.yaml`.

### Memory Consolidation

Merge similar memories using LLM analysis:

```json
{
  "tool": "subcog_consolidate",
  "arguments": {
    "namespace": "learnings",
    "strategy": "merge",
    "dry_run": true
  }
}
```

**Strategies:**
- `merge` - Combine similar memories into one
- `summarize` - Create summary of related memories
- `dedupe` - Remove exact duplicates

### Memory Enrichment

Improve a memory with better structure and tags:

```json
{
  "tool": "subcog_enrich",
  "arguments": {
    "memory_id": "decisions_abc123",
    "enrich_tags": true,
    "enrich_structure": true,
    "add_context": true
  }
}
```

## LLM Provider Configuration

Configure in `~/.config/subcog/config.yaml`:

```yaml
llm:
  provider: anthropic  # or openai, ollama, lmstudio
  api_key: ${ANTHROPIC_API_KEY}
  model: claude-3-haiku-20240307
```

| Provider | Model | Use Case |
|----------|-------|----------|
| Anthropic | claude-3-* | Best quality |
| OpenAI | gpt-4o-mini | Fast, good quality |
| Ollama | llama3.2 | Local, private |
| LM Studio | varies | Local, flexible |

## Search Optimization

### Hybrid Search Tuning

For precision-focused results:

```json
{
  "tool": "subcog_recall",
  "arguments": {
    "query": "exact topic",
    "mode": "text",
    "limit": 5
  }
}
```

For concept-focused results:

```json
{
  "tool": "subcog_recall",
  "arguments": {
    "query": "general concept or idea",
    "mode": "vector",
    "limit": 10
  }
}
```

## Embedding Model

Default: `all-MiniLM-L6-v2` (384 dimensions)

The embedding model is used for semantic similarity search in vector mode.
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_handler_creation() {
        let handler = ResourceHandler::new();
        let resources = handler.list_resources();

        assert!(!resources.is_empty());
        assert!(resources.iter().any(|r| r.uri.contains("setup")));
        assert!(resources.iter().any(|r| r.uri.contains("concepts")));
    }

    #[test]
    fn test_get_help_index() {
        let handler = ResourceHandler::new();
        let result = handler.get_resource("subcog://help").unwrap();

        assert!(result.text.is_some());
        let text = result.text.unwrap();
        assert!(text.contains("Subcog Help"));
        assert!(text.contains("Quick Start"));
    }

    #[test]
    fn test_get_help_category() {
        let handler = ResourceHandler::new();

        let result = handler.get_resource("subcog://help/setup").unwrap();
        assert!(result.text.is_some());
        assert!(result.text.unwrap().contains("MCP Server Configuration"));

        let result = handler.get_resource("subcog://help/concepts").unwrap();
        assert!(result.text.is_some());
        assert!(result.text.unwrap().contains("Namespaces"));
    }

    #[test]
    fn test_invalid_uri() {
        let handler = ResourceHandler::new();

        let result = handler.get_resource("http://example.com");
        assert!(result.is_err());

        let result = handler.get_resource("subcog://unknown");
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_category() {
        let handler = ResourceHandler::new();

        let result = handler.get_resource("subcog://help/nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_categories() {
        let handler = ResourceHandler::new();
        let categories = handler.list_categories();

        assert_eq!(categories.len(), 7);
    }
}
