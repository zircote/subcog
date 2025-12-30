//! MCP resource handlers.
//!
//! Provides resource access for the Model Context Protocol.
//! Resources are accessed via URN scheme: subcog://help/{category}

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Handler for MCP resources (URN scheme).
pub struct ResourceHandler {
    /// Help content by category.
    help_content: HashMap<String, HelpCategory>,
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
                description: "Understanding namespaces, domains, URNs, and memory lifecycle".to_string(),
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

        Self { help_content }
    }

    /// Lists all available resources.
    #[must_use]
    pub fn list_resources(&self) -> Vec<ResourceDefinition> {
        self.help_content
            .values()
            .map(|cat| ResourceDefinition {
                uri: format!("subcog://help/{}", cat.name),
                name: cat.title.clone(),
                description: Some(cat.description.clone()),
                mime_type: Some("text/markdown".to_string()),
            })
            .collect()
    }

    /// Gets a resource by URI.
    ///
    /// # Errors
    ///
    /// Returns an error if the resource is not found.
    pub fn get_resource(&self, uri: &str) -> Result<ResourceContent> {
        // Parse URI: subcog://help/{category}
        let uri = uri.trim();

        if !uri.starts_with("subcog://") {
            return Err(Error::InvalidInput(format!("Invalid URI scheme: {uri}")));
        }

        let path = &uri["subcog://".len()..];
        let parts: Vec<&str> = path.split('/').collect();

        if parts.is_empty() || parts[0] != "help" {
            return Err(Error::InvalidInput(format!("Unknown resource path: {path}")));
        }

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

    /// Gets the help index listing all categories.
    fn get_help_index(&self) -> String {
        let mut index = "# Subcog Help\n\nWelcome to Subcog, the persistent memory system for AI coding assistants.\n\n## Available Topics\n\n".to_string();

        for cat in self.help_content.values() {
            index.push_str(&format!(
                "- **[{}](subcog://help/{})**: {}\n",
                cat.title, cat.name, cat.description
            ));
        }

        index.push_str("\n## Quick Start\n\n");
        index.push_str("1. Capture a decision: `subcog capture --namespace decisions \"Use PostgreSQL\"`\n");
        index.push_str("2. Search memories: `subcog recall \"database choice\"`\n");
        index.push_str("3. Check status: `subcog status`\n");

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

const HELP_SETUP: &str = r#"
## Installation

Subcog is distributed as a single binary. Install via:

```bash
# From crates.io
cargo install subcog

# From source
git clone https://github.com/zircote/subcog
cd subcog
cargo install --path .
```

## Configuration

Create `~/.config/subcog/config.yaml`:

```yaml
data_dir: ~/.subcog
features:
  redact_secrets: true
  block_secrets: false
  auto_sync: true
```

## Claude Code Integration

Add to `~/.claude/settings.json`:

```json
{
  "hooks": {
    "SessionStart": [{ "command": "subcog hook session-start" }],
    "UserPromptSubmit": [{ "command": "subcog hook user-prompt-submit" }],
    "Stop": [{ "command": "subcog hook stop" }]
  }
}
```

## MCP Server

For Claude Desktop, add to `~/.config/claude/claude_desktop_config.json`:

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
"#;

const HELP_CONCEPTS: &str = r#"
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
"#;

const HELP_CAPTURE: &str = r#"
## Basic Capture

```bash
subcog capture --namespace decisions "Use PostgreSQL for primary storage"
```

## With Tags

```bash
subcog capture --namespace patterns \
  --tags "rust,error-handling" \
  "Use thiserror for custom error types"
```

## With Source Reference

```bash
subcog capture --namespace learnings \
  --source "src/auth.rs:42" \
  "JWT validation requires explicit algorithm specification"
```

## Best Practices

1. **Be Specific**: Include context and rationale
2. **Use Tags**: Add relevant keywords for better search
3. **Reference Sources**: Link to code or documentation
4. **Choose Correct Namespace**: Match content to category

## Auto-Capture

With hooks enabled, subcog can auto-detect and suggest captures:

- Decision language: "we decided to...", "let's use..."
- Pattern recognition: "always...", "never..."
- Learning indicators: "TIL", "learned that..."
"#;

const HELP_SEARCH: &str = r#"
## Basic Search

```bash
subcog recall "database storage decision"
```

## Search Modes

### Hybrid (Default)
Combines vector similarity and BM25 text search with RRF fusion:

```bash
subcog recall --mode hybrid "authentication patterns"
```

### Vector Only
Pure semantic similarity search:

```bash
subcog recall --mode vector "how to handle errors"
```

### Text Only
Traditional keyword search with BM25 ranking:

```bash
subcog recall --mode text "PostgreSQL"
```

## Filtering

### By Namespace

```bash
subcog recall --namespace decisions "storage"
```

### Limit Results

```bash
subcog recall --limit 5 "API design"
```

## Understanding Scores

- **Score 0.9+**: Very high relevance
- **Score 0.7-0.9**: Good relevance
- **Score 0.5-0.7**: Moderate relevance
- **Score <0.5**: Low relevance
"#;

const HELP_WORKFLOWS: &str = r#"
## Claude Code Hooks

### SessionStart

Injects relevant context at session start:

```json
{ "command": "subcog hook session-start" }
```

### UserPromptSubmit

Detects capture signals in prompts:

```json
{ "command": "subcog hook user-prompt-submit" }
```

### PostToolUse

Surfaces related memories after tool use:

```json
{ "command": "subcog hook post-tool-use" }
```

### PreCompact

Auto-captures before context compaction:

```json
{ "command": "subcog hook pre-compact" }
```

### Stop

Session summary and sync on exit:

```json
{ "command": "subcog hook stop" }
```

## MCP Integration

Start MCP server:

```bash
# Stdio transport (for Claude Desktop)
subcog serve

# HTTP transport (for programmatic access)
subcog serve --transport http --port 3000
```

## Git Sync

Sync memories with remote:

```bash
# Push to remote
subcog sync --push

# Fetch from remote
subcog sync --fetch

# Full sync
subcog sync
```
"#;

const HELP_TROUBLESHOOTING: &str = r#"
## Common Issues

### "No repository configured"

Ensure you're in a git repository or set `repo_path` in config.

### "Index not found"

Run initial indexing:

```bash
subcog status  # Will initialize if needed
```

### "Secret detected"

Content contains potential secrets. Either:
1. Remove the secret
2. Disable blocking: `block_secrets: false`
3. Enable redaction: `redact_secrets: true`

### Slow Search

1. Ensure SQLite index exists
2. Check vector index is loaded
3. Reduce result limit

## Debug Mode

Enable verbose logging:

```bash
RUST_LOG=debug subcog recall "test"
```

## Reset Index

```bash
rm -rf ~/.subcog/index.db
subcog status  # Rebuilds index
```

## Report Issues

GitHub: https://github.com/zircote/subcog/issues
"#;

const HELP_ADVANCED: &str = r#"
## LLM Integration

Configure an LLM provider for enhanced features:

```yaml
llm:
  provider: anthropic  # or openai, ollama, lmstudio
  api_key: ${ANTHROPIC_API_KEY}
  model: claude-3-haiku-20240307
```

### Supported Providers

| Provider | Model | Use Case |
|----------|-------|----------|
| Anthropic | claude-3-* | Best quality |
| OpenAI | gpt-4o-mini | Fast, good quality |
| Ollama | llama3.2 | Local, private |
| LM Studio | varies | Local, flexible |

## Memory Consolidation

Automatically merge and summarize related memories:

```bash
subcog consolidate
```

Options:
- Merge similar memories
- Archive old, unused memories
- Promote frequently accessed memories

## Custom Embeddings

Default: `all-MiniLM-L6-v2` (384 dimensions)

For custom models, implement the `Embedder` trait.

## Performance Tuning

### Index Settings

```yaml
index:
  fts5_tokenizer: porter  # or unicode61, ascii
  vector_dimensions: 384
  hnsw_ef_construction: 100
  hnsw_m: 16
```

### Caching

Enable memory caching for faster repeated queries:

```yaml
cache:
  enabled: true
  max_entries: 1000
  ttl_seconds: 3600
```
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
        assert!(result.text.unwrap().contains("Installation"));

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
