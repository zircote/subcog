//! Help content for MCP resources.
//!
//! Contains all the help documentation displayed via `subcog://help/*` resources.
//! This module is split from resources.rs to keep file sizes manageable (ARCH-C1).

/// Setup and configuration guide.
pub const SETUP: &str = r#"
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

Create `~/.config/subcog/config.toml`:

```toml
# Default data_dir is ~/.config/subcog; override if desired.
data_dir = "~/.config/subcog"

[features]
secrets_filter = true
pii_filter = false
auto_capture = true
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
| `subcog_reindex` | Rebuild search index |
| `prompt_understanding` | Guidance for using Subcog MCP tools |

## Available MCP Resources

| Resource | Description |
|----------|-------------|
| `subcog://help` | Help index |
| `subcog://help/{topic}` | Topic-specific help |
| `subcog://_` | List all memories across all domains |
| `subcog://project/_` | List project-scoped memories |
| `subcog://user/_` | List user-scoped memories |
| `subcog://org/_` | List org-scoped memories (if enabled) |
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

/// Core concepts documentation.
pub const CONCEPTS: &str = r"
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

- **Project** (`project`): Scoped to the current repository
- **User** (`user`): Shared across all projects for the current user
- **Organization** (`org`): Shared within an org when enabled

Org scope is optional and controlled by `SUBCOG_ORG_SCOPE_ENABLED`.

## URN Scheme

Memories are addressed via URNs:

```
subcog://{domain}/{namespace}/{id}
```

Examples:
```
subcog://project/decisions/abc123
subcog://user/decisions/def456
```

## Memory Lifecycle

1. **Active**: Default state, fully searchable
2. **Archived**: Less frequently accessed
3. **Superseded**: Replaced by newer memory
4. **Pending**: Awaiting review
5. **Deleted**: Marked for cleanup
6. **Tombstoned**: Removed from search results; retained for cleanup/audit
";

/// Capture documentation.
pub const CAPTURE: &str = r#"
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

/// Search documentation.
pub const SEARCH: &str = r#"
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

- `subcog://_` - All memories across all domains
- `subcog://project/_` - Project-scoped memories
- `subcog://user/_` - User-scoped memories
- `subcog://org/_` - Org-scoped memories (if enabled)
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

/// Workflow documentation.
pub const WORKFLOWS: &str = r#"
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

## Browsing via Resources

Access memories directly without search:

| Resource URI | Returns |
|--------------|---------|
| `subcog://_` | All memories across all domains (JSON) |
| `subcog://project/_` | Project-scoped memories |
| `subcog://user/_` | User-scoped memories |
| `subcog://org/_` | Org-scoped memories (if enabled) |
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

Returns: memory count, index status, storage backend info.
"#;

/// Troubleshooting documentation.
pub const TROUBLESHOOTING: &str = r#"
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
2. Check `~/.config/subcog/config.toml`:
   - `[features] secrets_filter = false` to disable secrets filtering

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

## Report Issues

GitHub: https://github.com/zircote/subcog/issues
"#;

/// Advanced features documentation.
pub const ADVANCED: &str = r#"
## LLM-Powered Tools

These tools require an LLM provider configured in `~/.config/subcog/config.toml`.

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

Configure in `~/.config/subcog/config.toml`:

```toml
[llm]
provider = "anthropic" # or "openai", "ollama", "lmstudio"
api_key = "${ANTHROPIC_API_KEY}"
model = "claude-3-haiku-20240307"
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

/// Prompts documentation.
pub const PROMPTS: &str = r#"
## User-Defined Prompts

Subcog supports saving and reusing prompt templates with variable substitution.

## MCP Tools for Prompts

| Tool | Description |
|------|-------------|
| `prompt_save` | Save a prompt template |
| `prompt_list` | List saved prompts |
| `prompt_get` | Get a prompt by name |
| `prompt_run` | Execute a prompt with variables |
| `prompt_delete` | Delete a prompt |

## Saving Prompts

### From Content

```json
{
  "tool": "prompt_save",
  "arguments": {
    "name": "code-review",
    "content": "Review the {{language}} code in {{file}} for:\n- Security issues\n- Performance\n- Best practices",
    "description": "Code review checklist template",
    "tags": ["review", "quality"]
  }
}
```

### From File

```json
{
  "tool": "prompt_save",
  "arguments": {
    "name": "refactor-plan",
    "file_path": "/path/to/prompt.md"
  }
}
```

## Variable Syntax

Variables use double-brace syntax: `{{variable_name}}`

### Required vs Optional

| Syntax | Type | Behavior |
|--------|------|----------|
| `{{name}}` | Required | Must be provided |
| `{{name:default}}` | Optional | Uses default if not provided |

### Example Template

```markdown
---
name: api-design
description: API endpoint design guide
tags:
  - api
  - design
variables:
  - name: resource
    description: The resource being designed
    required: true
  - name: version
    description: API version
    default: v1
---

Design a REST API for the {{resource}} resource.

API Version: {{version}}

Include:
- Endpoints (GET, POST, PUT, DELETE)
- Request/response schemas
- Error handling
```

## Running Prompts

### With All Variables

```json
{
  "tool": "prompt_run",
  "arguments": {
    "name": "code-review",
    "variables": {
      "language": "Rust",
      "file": "src/main.rs"
    }
  }
}
```

### With Defaults

```json
{
  "tool": "prompt_run",
  "arguments": {
    "name": "api-design",
    "variables": {
      "resource": "users"
    }
  }
}
```

The `version` variable will use its default value of "v1".

## Domain Scopes

Prompts support three domain scopes:

| Scope | Description | Search Order |
|-------|-------------|--------------|
| `project` | Current repository | Searched first |
| `user` | User-wide prompts | Searched second |
| `org` | Organization-wide | Searched last |

### Saving to a Specific Domain

```json
{
  "tool": "prompt_save",
  "arguments": {
    "name": "deploy-checklist",
    "content": "...",
    "domain": "org"
  }
}
```

### Retrieving with Domain Fallback

When getting a prompt, subcog searches in order: project -> user -> org

```json
{
  "tool": "prompt_get",
  "arguments": {
    "name": "deploy-checklist"
  }
}
```

## Listing and Filtering

### List All Prompts

```json
{
  "tool": "prompt_list",
  "arguments": {}
}
```

### Filter by Domain

```json
{
  "tool": "prompt_list",
  "arguments": {
    "domain": "user"
  }
}
```

### Filter by Tags

```json
{
  "tool": "prompt_list",
  "arguments": {
    "tags": ["api", "design"]
  }
}
```

### Filter by Name Pattern

```json
{
  "tool": "prompt_list",
  "arguments": {
    "name_pattern": "code-*"
  }
}
```

## CLI Commands

### Save a Prompt

```bash
# From content
subcog prompt save my-prompt "Template with {{var}}"

# From file
subcog prompt save my-prompt --from-file prompt.md

# With options
subcog prompt save my-prompt "content" \
  --description "Description here" \
  --tags "tag1,tag2" \
  --domain user
```

### List Prompts

```bash
subcog prompt list
subcog prompt list --domain user
subcog prompt list --tags api,design
subcog prompt list --format json
```

### Get a Prompt

```bash
subcog prompt get my-prompt
subcog prompt get my-prompt --format yaml
```

### Run a Prompt

```bash
# With variables
subcog prompt run my-prompt var1=value1 var2=value2

# Interactive mode (prompts for missing variables)
subcog prompt run my-prompt --interactive
```

### Export a Prompt

```bash
subcog prompt export my-prompt --output prompt.md
subcog prompt export my-prompt --format yaml
```

### Delete a Prompt

```bash
subcog prompt delete my-prompt --domain project --force
```

## Supported Formats

| Format | Extension | Description |
|--------|-----------|-------------|
| Markdown | `.md` | YAML front matter + content |
| YAML | `.yaml`, `.yml` | Full structured format |
| JSON | `.json` | Machine-readable format |
| Plain Text | `.txt` | Content only (no metadata) |

## Best Practices

1. **Use descriptive names**: `api-design` not `prompt1`
2. **Add descriptions**: Explain the prompt's purpose
3. **Tag consistently**: Use standard tags across prompts
4. **Provide defaults**: Make prompts easier to use
5. **Document variables**: Add descriptions for clarity
6. **Use domain scoping**: Share org-wide, customize per project
"#;
