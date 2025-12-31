# Architecture: User Prompt Management

**Version**: 1.0
**Status**: Draft
**Last Updated**: 2025-12-30

## 1. System Context

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              User                                       │
└─────────────────────────────────────────────────────────────────────────┘
           │                    │                      │
           ▼                    ▼                      ▼
    ┌─────────────┐     ┌─────────────┐       ┌─────────────┐
    │  CLI        │     │  MCP Host   │       │  File       │
    │  (terminal) │     │  (IDE/LLM)  │       │  System     │
    └─────────────┘     └─────────────┘       └─────────────┘
           │                    │                      │
           └────────────────────┼──────────────────────┘
                                ▼
    ┌─────────────────────────────────────────────────────────────────────┐
    │                         Subcog                                      │
    │  ┌──────────────────────────────────────────────────────────────┐   │
    │  │                    Prompt Management                         │   │
    │  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐      │   │
    │  │  │  Models  │  │ Parsers  │  │ Service  │  │   CLI    │      │   │
    │  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘      │   │
    │  │  ┌──────────┐  ┌──────────┐  ┌──────────┐                    │   │
    │  │  │MCP Tools │  │ Sampling │  │  Hooks   │                    │   │
    │  │  └──────────┘  └──────────┘  └──────────┘                    │   │
    │  └──────────────────────────────────────────────────────────────┘   │
    │                                │                                    │
    │                                ▼                                    │
    │  ┌──────────────────────────────────────────────────────────────┐   │
    │  │               Existing Subcog Infrastructure                 │   │
    │  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐      │   │
    │  │  │ Capture  │  │  Recall  │  │  Index   │  │ Git Notes│      │   │
    │  │  │ Service  │  │ Service  │  │ (SQLite) │  │ (Storage)│      │   │
    │  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘      │   │
    │  └──────────────────────────────────────────────────────────────┘   │
    └─────────────────────────────────────────────────────────────────────┘
```

## 2. Component Architecture

### 2.1 Models (`src/models/prompt.rs`)

```rust
/// A user-defined prompt template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    /// Unique prompt name (kebab-case).
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// The prompt content with {{variable}} placeholders.
    pub content: String,
    /// Extracted variables with optional metadata.
    pub variables: Vec<PromptVariable>,
    /// Categorization tags.
    pub tags: Vec<String>,
    /// Author identifier.
    pub author: Option<String>,
    /// Usage count for popularity ranking.
    pub usage_count: u64,
    /// Creation timestamp (Unix epoch).
    pub created_at: u64,
    /// Last update timestamp (Unix epoch).
    pub updated_at: u64,
}

/// A template variable definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptVariable {
    /// Variable name (without braces).
    pub name: String,
    /// Human-readable description for elicitation.
    pub description: Option<String>,
    /// Default value if not provided.
    pub default: Option<String>,
    /// Whether the variable is required.
    #[serde(default = "default_required")]
    pub required: bool,
}

fn default_required() -> bool { true }

/// Result of variable extraction.
#[derive(Debug, Clone)]
pub struct ExtractedVariable {
    pub name: String,
    pub position: usize,
}

/// Validation result for prompt content.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub issues: Vec<ValidationIssue>,
}

#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub severity: IssueSeverity,
    pub message: String,
    pub position: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
pub enum IssueSeverity {
    Error,
    Warning,
}
```

### 2.2 Parsers (`src/services/prompt_parser.rs`)

```rust
/// Trait for prompt file parsers.
pub trait PromptParser {
    /// Parse content into a PromptTemplate.
    fn parse(&self, content: &str) -> Result<PromptTemplate>;
}

/// Parser for markdown files with YAML frontmatter.
pub struct MarkdownParser;

/// Parser for pure YAML files.
pub struct YamlParser;

/// Parser for JSON files.
pub struct JsonParser;

/// Parser for plain text files (variables auto-detected).
pub struct PlainTextParser;

/// Main parser that routes to appropriate format.
pub struct PromptFileParser;

impl PromptFileParser {
    /// Parse from file path, auto-detecting format.
    pub fn parse_file(path: &Path) -> Result<PromptTemplate>;

    /// Parse from string with explicit format.
    pub fn parse_string(content: &str, format: PromptFormat) -> Result<PromptTemplate>;

    /// Detect format from file extension.
    pub fn detect_format(path: &Path) -> PromptFormat;
}

#[derive(Debug, Clone, Copy)]
pub enum PromptFormat {
    Markdown,
    Yaml,
    Json,
    PlainText,
}
```

### 2.3 Service (`src/services/prompt.rs`)

```rust
/// Service for prompt CRUD operations.
pub struct PromptService {
    capture: CaptureService,
    recall: RecallService,
}

impl PromptService {
    /// Create a new prompt service.
    pub fn new(capture: CaptureService, recall: RecallService) -> Self;

    /// Save or update a prompt template.
    pub fn save(&self, template: PromptTemplate, domain: Domain) -> Result<String>;

    /// Get a prompt by name, searching domain hierarchy.
    pub fn get(&self, name: &str, domain: Option<DomainScope>) -> Result<Option<PromptTemplate>>;

    /// List prompts with filters.
    pub fn list(&self, filter: &PromptFilter) -> Result<Vec<PromptTemplate>>;

    /// Delete a prompt by name.
    pub fn delete(&self, name: &str, domain: DomainScope) -> Result<bool>;

    /// Search prompts semantically.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<PromptTemplate>>;

    /// Increment usage count for a prompt.
    pub fn increment_usage(&self, name: &str, domain: DomainScope) -> Result<()>;
}

/// Filter for listing prompts.
#[derive(Debug, Clone, Default)]
pub struct PromptFilter {
    pub domain: Option<DomainScope>,
    pub tags: Vec<String>,
    pub name_pattern: Option<String>,
    pub limit: Option<usize>,
}
```

### 2.4 Variable Handling

```rust
/// Extract variables from prompt content.
/// Returns variables in order of appearance.
pub fn extract_variables(content: &str) -> Vec<ExtractedVariable> {
    // Regex: \{\{(\w+)\}\}
    // Captures variable names without braces
}

/// Substitute variables in prompt content.
pub fn substitute_variables(
    content: &str,
    values: &HashMap<String, String>,
    variables: &[PromptVariable],
) -> Result<String> {
    // 1. Check all required variables have values or defaults
    // 2. Apply defaults for missing optional variables
    // 3. Replace all {{var}} patterns
}

/// Validate prompt content for common issues.
pub fn validate_prompt(content: &str) -> ValidationResult {
    // Check for:
    // - Unclosed braces
    // - Invalid variable names
    // - Valid YAML frontmatter (if present)
}
```

## 3. Data Flow

### 3.1 Save Prompt Flow

```
┌──────────────────────────────────────────────────────────────────────────┐
│                          Save Prompt Flow                                │
└──────────────────────────────────────────────────────────────────────────┘

User/AI ──┬── CLI: --from-file path.md ──┬── PromptFileParser.parse_file()
          │                               │           │
          ├── CLI: --from-stdin ──────────┼── PromptFileParser.parse_string()
          │                               │           │
          ├── CLI: inline "content" ──────┤           ▼
          │                               │   PromptTemplate
          └── MCP: prompt.save ───────────┘           │
                                                      ▼
                                            PromptService.save()
                                                      │
                                          ┌───────────┴───────────┐
                                          │                       │
                                          ▼                       ▼
                                   CaptureService          SQLite Index
                                   (Git Notes)            (name lookup)
```

### 3.2 Run Prompt Flow

```
┌──────────────────────────────────────────────────────────────────────────┐
│                           Run Prompt Flow                                │
└──────────────────────────────────────────────────────────────────────────┘

                        ┌──────────────────────┐
                        │   prompt.run(name)   │
                        └──────────┬───────────┘
                                   │
                                   ▼
                        ┌──────────────────────┐
                        │ PromptService.get()  │
                        │ (domain hierarchy)   │
                        └──────────┬───────────┘
                                   │
                                   ▼
                        ┌──────────────────────┐
                        │  Extract Variables   │
                        │ {{var1}}, {{var2}}   │
                        └──────────┬───────────┘
                                   │
              ┌────────────────────┴────────────────────┐
              │                                         │
              ▼                                         ▼
    ┌──────────────────┐                      ┌──────────────────┐
    │ CLI: dialoguer   │                      │ MCP: sampling/   │
    │ interactive      │                      │ createMessage    │
    └────────┬─────────┘                      └────────┬─────────┘
             │                                         │
             └────────────────────┬────────────────────┘
                                  │
                                  ▼
                        ┌──────────────────────┐
                        │ substitute_variables │
                        └──────────┬───────────┘
                                   │
                                   ▼
                        ┌──────────────────────┐
                        │ Return populated     │
                        │ prompt content       │
                        └──────────────────────┘
```

### 3.3 MCP Variable Elicitation Sequence

```
┌────────┐          ┌────────┐          ┌────────┐
│  Host  │          │Subcog  │          │ Store  │
│ (LLM)  │          │  MCP   │          │        │
└───┬────┘          └───┬────┘          └───┬────┘
    │                   │                   │
    │ prompts/get       │                   │
    │ ("grocery-list")  │                   │
    │──────────────────>│                   │
    │                   │ recall(name,      │
    │                   │ ns=prompts)       │
    │                   │──────────────────>│
    │                   │                   │
    │                   │<──────────────────│
    │                   │ PromptTemplate    │
    │                   │                   │
    │                   │ Extract: {{items}},│
    │                   │ {{budget}}        │
    │                   │                   │
    │ sampling/create   │                   │
    │ Message           │                   │
    │<──────────────────│                   │
    │ "What items?"     │                   │
    │                   │                   │
    │──────────────────>│                   │
    │ "milk, eggs"      │                   │
    │                   │                   │
    │ sampling/create   │                   │
    │ Message           │                   │
    │<──────────────────│                   │
    │ "Budget?"         │                   │
    │                   │                   │
    │──────────────────>│                   │
    │ "$50"             │                   │
    │                   │                   │
    │<──────────────────│                   │
    │ PromptMessage     │                   │
    │ (populated)       │                   │
    │                   │                   │
```

## 4. Storage Design

### 4.1 Memory Format

Prompts are stored as memories in the `prompts` namespace with this structure:

```yaml
---
id: prompt-code-review-1234567890
namespace: prompts
domain: project
tags:
  - coding
  - review
source: file:///path/to/code-review.md
created_at: 1735570000
updated_at: 1735570000
---
{
  "name": "code-review",
  "description": "Review code for quality issues",
  "content": "Review this {{language}} code:\n\n```{{language}}\n{{code}}\n```",
  "variables": [
    {"name": "language", "required": true},
    {"name": "code", "required": true}
  ],
  "tags": ["coding", "review"],
  "author": "user@example.com",
  "usage_count": 42
}
```

### 4.2 Index Schema

Extend SQLite index for prompt name lookup:

```sql
-- Existing memories table has:
-- id, namespace, content, embedding, created_at, updated_at

-- Add index for prompt names (extracted from JSON content)
CREATE INDEX IF NOT EXISTS idx_prompt_name
ON memories(json_extract(content, '$.name'))
WHERE namespace = 'prompts';
```

### 4.3 Domain Hierarchy

| Priority | Domain | Storage Location |
|----------|--------|------------------|
| 1 (highest) | Project | `.git/notes/subcog` in repo |
| 2 | User | `~/.subcog/` |
| 3 (lowest) | Org | Configured org index |

## 5. MCP Integration

### 5.1 Tool Schemas

```json
{
  "name": "prompt.save",
  "description": "Save a reusable prompt template with variables",
  "inputSchema": {
    "type": "object",
    "properties": {
      "name": {
        "type": "string",
        "description": "Unique prompt name (kebab-case)"
      },
      "description": {
        "type": "string",
        "description": "What this prompt does"
      },
      "content": {
        "type": "string",
        "description": "Prompt content with {{variable}} placeholders"
      },
      "file_path": {
        "type": "string",
        "description": "Path to file containing prompt (alternative to content)"
      },
      "tags": {
        "type": "array",
        "items": { "type": "string" }
      },
      "domain": {
        "type": "string",
        "enum": ["project", "user", "org"],
        "default": "project"
      },
      "variables": {
        "type": "array",
        "description": "Explicit variable definitions",
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
      }
    },
    "required": ["name"],
    "oneOf": [
      { "required": ["content"] },
      { "required": ["file_path"] }
    ]
  }
}
```

```json
{
  "name": "prompt.run",
  "description": "Run a saved prompt, eliciting variable values as needed",
  "inputSchema": {
    "type": "object",
    "properties": {
      "name": {
        "type": "string",
        "description": "Prompt name to run"
      },
      "variables": {
        "type": "object",
        "description": "Pre-filled variable values",
        "additionalProperties": { "type": "string" }
      },
      "domain": {
        "type": "string",
        "enum": ["project", "user", "org"]
      }
    },
    "required": ["name"]
  }
}
```

### 5.2 Prompts/List Extension

Extend `prompts/list` to include user prompts:

```rust
impl PromptRegistry {
    pub fn list_prompts(&self, prompt_service: &PromptService) -> Vec<PromptInfo> {
        let mut prompts = self.builtin_prompts.clone();

        // Add user prompts from all domains
        if let Ok(user_prompts) = prompt_service.list(&PromptFilter::default()) {
            for template in user_prompts {
                prompts.push(PromptInfo {
                    name: template.name,
                    description: Some(template.description),
                    arguments: template.variables.iter().map(|v| PromptArgument {
                        name: v.name.clone(),
                        description: v.description.clone(),
                        required: Some(v.required),
                    }).collect(),
                });
            }
        }

        prompts
    }
}
```

## 6. CLI Design

### 6.1 Command Structure

```
subcog prompt
├── save [OPTIONS] [CONTENT]
│   ├── --name <NAME>        (required)
│   ├── --description <DESC>
│   ├── --tags <TAGS>
│   ├── --domain <DOMAIN>
│   ├── --from-file <PATH>
│   └── --from-stdin
├── list [OPTIONS]
│   ├── --domain <DOMAIN>
│   ├── --tags <TAGS>
│   └── --format <table|json>
├── get <NAME> [OPTIONS]
│   ├── --domain <DOMAIN>
│   └── --format <template|json>
├── run <NAME> [OPTIONS]
│   ├── --var <KEY=VALUE>... (repeatable)
│   └── --domain <DOMAIN>
├── delete <NAME> [OPTIONS]
│   ├── --domain <DOMAIN>    (required)
│   └── --force
└── export <NAME> [OPTIONS]
    ├── --output <PATH>
    └── --format <markdown|yaml|json>
```

### 6.2 Interactive Prompts

Using `dialoguer` for interactive variable elicitation:

```rust
use dialoguer::{Input, theme::ColorfulTheme};

fn elicit_variable(var: &PromptVariable) -> Result<String> {
    let prompt = match &var.description {
        Some(desc) => format!("{} ({})", var.name, desc),
        None => var.name.clone(),
    };

    let mut input = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(&prompt);

    if let Some(default) = &var.default {
        input = input.default(default.clone());
    }

    input.interact_text()
        .map_err(|e| Error::InteractionFailed(e.to_string()))
}
```

## 7. Hook Integration

### 7.1 PostToolUse Validation

```rust
impl PostToolUseHandler {
    fn handle_prompt_save(&self, tool_result: &ToolResult) -> Option<HookResponse> {
        // Extract content from tool input
        let content = tool_result.input.get("content")?;

        // Validate format
        let validation = validate_prompt(content);

        if !validation.is_valid {
            let issues: Vec<String> = validation.issues
                .iter()
                .map(|i| i.message.clone())
                .collect();

            return Some(HookResponse {
                additional_context: Some(format!(
                    "**Prompt Format Issues:**\n{}\n\nSee `subcog://help/prompts` for format guide.",
                    issues.join("\n")
                )),
                ..Default::default()
            });
        }

        None
    }
}
```

### 7.2 Help Resource

Add `subcog://help/prompts` resource:

```rust
fn read_resource(&self, uri: &str) -> Result<ResourceContent> {
    if uri == "subcog://help/prompts" {
        return Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("text/markdown".to_string()),
            text: Some(include_str!("../help/content/prompts.md").to_string()),
            ..Default::default()
        });
    }
    // ... other resources
}
```

## 8. Error Handling

### 8.1 Error Types

```rust
#[derive(Debug, Error)]
pub enum PromptError {
    #[error("Prompt not found: {0}")]
    NotFound(String),

    #[error("Invalid prompt name: {0}. Use kebab-case (e.g., 'my-prompt')")]
    InvalidName(String),

    #[error("Variable extraction failed: {0}")]
    VariableExtraction(String),

    #[error("Missing required variable: {0}")]
    MissingVariable(String),

    #[error("Invalid file format: {0}")]
    InvalidFormat(String),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),
}
```

### 8.2 User-Friendly Messages

All errors should include:
1. What went wrong
2. Why it's a problem
3. How to fix it

Example:
```
Error: Invalid variable name in prompt: "user-name"

Variable names must contain only letters, numbers, and underscores.
Hyphens are not allowed.

Suggested fix: Use "user_name" instead of "user-name"
```

## 9. Performance Considerations

### 9.1 Caching

```rust
pub struct PromptService {
    // LRU cache for frequently accessed prompts
    cache: Mutex<LruCache<String, PromptTemplate>>,
    cache_ttl: Duration,
}
```

### 9.2 Lazy Loading

- Prompt list only loads metadata, not full content
- Content loaded on demand (get/run)
- Variables extracted once and cached in template

### 9.3 Indexing

- SQLite index on prompt name for O(1) lookup
- Semantic search uses existing vector index
- Tags indexed for filtered queries

## 10. Security Considerations

### 10.1 Input Validation

- Prompt names: kebab-case, alphanumeric + hyphens
- Variable names: alphanumeric + underscores
- File paths: validated, no path traversal
- Content: checked for secrets before storage

### 10.2 Variable Injection

- Variables are plain string substitution
- No code execution or shell expansion
- Output escaping not performed (user responsibility)

### 10.3 File Access

- `--from-file` uses absolute paths
- Relative paths resolved from cwd
- No access outside allowed directories
