//! MCP pre-defined prompts.
//!
//! Provides prompt templates for the Model Context Protocol.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Registry of pre-defined prompts.
pub struct PromptRegistry {
    /// Available prompts.
    prompts: HashMap<String, PromptDefinition>,
}

impl PromptRegistry {
    /// Creates a new prompt registry.
    #[must_use]
    pub fn new() -> Self {
        let mut prompts = HashMap::new();

        // Register all prompts
        for prompt in Self::all_prompts() {
            prompts.insert(prompt.name.clone(), prompt);
        }

        Self { prompts }
    }

    /// Returns all prompt definitions.
    fn all_prompts() -> Vec<PromptDefinition> {
        vec![
            Self::tutorial_prompt(),
            Self::capture_assistant_prompt(),
            Self::review_prompt(),
            Self::document_decision_prompt(),
            Self::search_help_prompt(),
            Self::browse_prompt(),
            Self::list_prompt(),
        ]
    }

    fn tutorial_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_tutorial".to_string(),
            description: Some("Interactive tutorial for learning Subcog memory system".to_string()),
            arguments: vec![
                PromptArgument {
                    name: "familiarity".to_string(),
                    description: Some("Your familiarity level with memory systems".to_string()),
                    required: false,
                },
                PromptArgument {
                    name: "focus".to_string(),
                    description: Some("Topic to focus on".to_string()),
                    required: false,
                },
            ],
        }
    }

    fn capture_assistant_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_capture_assistant".to_string(),
            description: Some("Help decide what to capture and which namespace to use".to_string()),
            arguments: vec![PromptArgument {
                name: "context".to_string(),
                description: Some(
                    "The current context or conversation to analyze for memories".to_string(),
                ),
                required: true,
            }],
        }
    }

    fn review_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_review".to_string(),
            description: Some("Review and analyze existing memories for a project".to_string()),
            arguments: vec![
                PromptArgument {
                    name: "namespace".to_string(),
                    description: Some("Optional namespace to focus on".to_string()),
                    required: false,
                },
                PromptArgument {
                    name: "action".to_string(),
                    description: Some(
                        "Action: summarize, consolidate, archive, or cleanup".to_string(),
                    ),
                    required: false,
                },
            ],
        }
    }

    fn document_decision_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_document_decision".to_string(),
            description: Some(
                "Help document an architectural or design decision properly".to_string(),
            ),
            arguments: vec![
                PromptArgument {
                    name: "decision".to_string(),
                    description: Some("Brief description of the decision".to_string()),
                    required: true,
                },
                PromptArgument {
                    name: "alternatives".to_string(),
                    description: Some("Alternatives that were considered".to_string()),
                    required: false,
                },
            ],
        }
    }

    fn search_help_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_search_help".to_string(),
            description: Some("Help craft effective memory search queries".to_string()),
            arguments: vec![PromptArgument {
                name: "goal".to_string(),
                description: Some("What you're trying to find or accomplish".to_string()),
                required: true,
            }],
        }
    }

    fn browse_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_browse".to_string(),
            description: Some(
                "Interactive memory browser with faceted discovery and filtering".to_string(),
            ),
            arguments: vec![
                PromptArgument {
                    name: "filter".to_string(),
                    description: Some(
                        "Filter expression: ns:X, tag:X, tag:X,Y (OR), -tag:X (exclude), since:Nd, source:X, status:X".to_string(),
                    ),
                    required: false,
                },
                PromptArgument {
                    name: "view".to_string(),
                    description: Some(
                        "View mode: dashboard (default), tags, namespaces, memories".to_string(),
                    ),
                    required: false,
                },
                PromptArgument {
                    name: "top".to_string(),
                    description: Some("Number of items per facet (default: 10)".to_string()),
                    required: false,
                },
            ],
        }
    }

    fn list_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_list".to_string(),
            description: Some(
                "List memories in formatted table with namespace/tag summary".to_string(),
            ),
            arguments: vec![
                PromptArgument {
                    name: "filter".to_string(),
                    description: Some(
                        "Filter expression: ns:X, tag:X, since:Nd (same syntax as subcog_browse)"
                            .to_string(),
                    ),
                    required: false,
                },
                PromptArgument {
                    name: "format".to_string(),
                    description: Some(
                        "Output format: table (default), compact, detailed".to_string(),
                    ),
                    required: false,
                },
                PromptArgument {
                    name: "limit".to_string(),
                    description: Some("Maximum memories to list (default: 50)".to_string()),
                    required: false,
                },
            ],
        }
    }

    /// Returns all prompt definitions.
    #[must_use]
    pub fn list_prompts(&self) -> Vec<&PromptDefinition> {
        self.prompts.values().collect()
    }

    /// Gets a prompt definition by name.
    #[must_use]
    pub fn get_prompt(&self, name: &str) -> Option<&PromptDefinition> {
        self.prompts.get(name)
    }

    /// Generates prompt messages for a given prompt and arguments.
    #[must_use]
    pub fn get_prompt_messages(&self, name: &str, arguments: &Value) -> Option<Vec<PromptMessage>> {
        match name {
            "subcog_tutorial" => Some(self.generate_tutorial_prompt(arguments)),
            "subcog_capture_assistant" => Some(self.generate_capture_assistant_prompt(arguments)),
            "subcog_review" => Some(self.generate_review_prompt(arguments)),
            "subcog_document_decision" => Some(self.generate_decision_prompt(arguments)),
            "subcog_search_help" => Some(self.generate_search_help_prompt(arguments)),
            "subcog_browse" => Some(self.generate_browse_prompt(arguments)),
            "subcog_list" => Some(self.generate_list_prompt(arguments)),
            _ => None,
        }
    }

    /// Generates the tutorial prompt.
    fn generate_tutorial_prompt(&self, arguments: &Value) -> Vec<PromptMessage> {
        let familiarity = arguments
            .get("familiarity")
            .and_then(|v| v.as_str())
            .unwrap_or("beginner");

        let focus = arguments
            .get("focus")
            .and_then(|v| v.as_str())
            .unwrap_or("overview");

        let intro = match familiarity {
            "advanced" => {
                "I see you're experienced with memory systems. Let me show you Subcog's advanced features."
            },
            "intermediate" => {
                "Great, you have some familiarity with memory systems. Let me explain Subcog's key concepts."
            },
            _ => "Welcome to Subcog! I'll guide you through the basics of the memory system.",
        };

        let focus_content = match focus {
            "capture" => TUTORIAL_CAPTURE,
            "recall" | "search" => TUTORIAL_SEARCH,
            "namespaces" => TUTORIAL_NAMESPACES,
            "workflows" => TUTORIAL_WORKFLOWS,
            "best-practices" => TUTORIAL_BEST_PRACTICES,
            _ => TUTORIAL_OVERVIEW,
        };

        vec![
            PromptMessage {
                role: "user".to_string(),
                content: PromptContent::Text {
                    text: format!(
                        "I'd like to learn about Subcog. My familiarity level is '{familiarity}' and I want to focus on '{focus}'."
                    ),
                },
            },
            PromptMessage {
                role: "assistant".to_string(),
                content: PromptContent::Text {
                    text: format!("{intro}\n\n{focus_content}"),
                },
            },
        ]
    }

    /// Generates the capture assistant prompt.
    fn generate_capture_assistant_prompt(&self, arguments: &Value) -> Vec<PromptMessage> {
        let context = arguments
            .get("context")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        vec![
            PromptMessage {
                role: "user".to_string(),
                content: PromptContent::Text {
                    text: format!(
                        "Please analyze this context and suggest what memories to capture:\n\n{context}"
                    ),
                },
            },
            PromptMessage {
                role: "assistant".to_string(),
                content: PromptContent::Text {
                    text: CAPTURE_ASSISTANT_SYSTEM.to_string(),
                },
            },
        ]
    }

    /// Generates the review prompt.
    fn generate_review_prompt(&self, arguments: &Value) -> Vec<PromptMessage> {
        let namespace = arguments
            .get("namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let action = arguments
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("summarize");

        vec![PromptMessage {
            role: "user".to_string(),
            content: PromptContent::Text {
                text: format!(
                    "Please {action} the memories in the '{namespace}' namespace. Help me understand what we have and identify any gaps or improvements."
                ),
            },
        }]
    }

    /// Generates the decision documentation prompt.
    fn generate_decision_prompt(&self, arguments: &Value) -> Vec<PromptMessage> {
        let decision = arguments
            .get("decision")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let alternatives = arguments
            .get("alternatives")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let mut prompt =
            format!("I need to document the following decision:\n\n**Decision**: {decision}\n");

        if !alternatives.is_empty() {
            prompt.push_str(&format!("\n**Alternatives considered**: {alternatives}\n"));
        }

        prompt.push_str(
            "\nPlease help me document this decision in a structured way that captures:\n\
            1. The context and problem being solved\n\
            2. The decision and rationale\n\
            3. Consequences and trade-offs\n\
            4. Suggested tags for searchability",
        );

        vec![PromptMessage {
            role: "user".to_string(),
            content: PromptContent::Text { text: prompt },
        }]
    }

    /// Generates the search help prompt.
    fn generate_search_help_prompt(&self, arguments: &Value) -> Vec<PromptMessage> {
        let goal = arguments.get("goal").and_then(|v| v.as_str()).unwrap_or("");

        vec![
            PromptMessage {
                role: "user".to_string(),
                content: PromptContent::Text {
                    text: format!(
                        "I'm trying to find memories related to: {goal}\n\nHelp me craft effective search queries."
                    ),
                },
            },
            PromptMessage {
                role: "assistant".to_string(),
                content: PromptContent::Text {
                    text: SEARCH_HELP_SYSTEM.to_string(),
                },
            },
        ]
    }

    /// Generates the browse prompt (discovery dashboard).
    fn generate_browse_prompt(&self, arguments: &Value) -> Vec<PromptMessage> {
        let filter = arguments
            .get("filter")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let view = arguments
            .get("view")
            .and_then(|v| v.as_str())
            .unwrap_or("dashboard");

        let top = arguments
            .get("top")
            .and_then(|v| v.as_str())
            .unwrap_or("10");

        let mut prompt = String::from(
            "Show me a memory browser dashboard.\n\n**IMPORTANT**: Use the `subcog_recall` tool to fetch memories with server-side filtering:\n",
        );

        if filter.is_empty() {
            prompt.push_str("```json\n{ \"query\": \"*\", \"limit\": 100, \"detail\": \"medium\" }\n```\n\n");
            prompt.push_str("No filters applied - show the full dashboard with:\n");
        } else {
            prompt.push_str(&format!(
                "```json\n{{ \"query\": \"*\", \"filter\": \"{filter}\", \"limit\": 100, \"detail\": \"medium\" }}\n```\n\n"
            ));
        }

        prompt.push_str(&format!(
            "View mode: {view}\nShow top {top} items per facet.\n\n"
        ));

        prompt.push_str(BROWSE_DASHBOARD_INSTRUCTIONS);

        vec![
            PromptMessage {
                role: "user".to_string(),
                content: PromptContent::Text { text: prompt },
            },
            PromptMessage {
                role: "assistant".to_string(),
                content: PromptContent::Text {
                    text: BROWSE_SYSTEM_RESPONSE.to_string(),
                },
            },
        ]
    }

    /// Generates the list prompt (formatted inventory).
    fn generate_list_prompt(&self, arguments: &Value) -> Vec<PromptMessage> {
        let filter = arguments
            .get("filter")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let format = arguments
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("table");

        let limit = arguments
            .get("limit")
            .and_then(|v| v.as_str())
            .unwrap_or("50");

        let mut prompt = String::from(
            "List memories from Subcog.\n\n**IMPORTANT**: Use the `subcog_recall` tool to fetch memories with server-side filtering:\n",
        );

        if filter.is_empty() {
            prompt.push_str(&format!(
                "```json\n{{ \"query\": \"*\", \"limit\": {limit}, \"detail\": \"medium\" }}\n```\n\n"
            ));
        } else {
            prompt.push_str(&format!(
                "```json\n{{ \"query\": \"*\", \"filter\": \"{filter}\", \"limit\": {limit}, \"detail\": \"medium\" }}\n```\n\n"
            ));
        }

        prompt.push_str(&format!("Format: {format}\n\n"));

        prompt.push_str(LIST_FORMAT_INSTRUCTIONS);

        vec![PromptMessage {
            role: "user".to_string(),
            content: PromptContent::Text { text: prompt },
        }]
    }
}

impl Default for PromptRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Definition of an MCP prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptDefinition {
    /// Prompt name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Prompt arguments.
    pub arguments: Vec<PromptArgument>,
}

/// Argument for a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    /// Argument name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Whether the argument is required.
    pub required: bool,
}

/// A message in a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    /// Role: user, assistant, or system.
    pub role: String,
    /// Message content.
    pub content: PromptContent,
}

/// Content of a prompt message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PromptContent {
    /// Text content.
    Text {
        /// The text content.
        text: String,
    },
    /// Image content.
    Image {
        /// Image data (base64 or URL).
        data: String,
        /// MIME type.
        mime_type: String,
    },
    /// Resource reference.
    Resource {
        /// Resource URI.
        uri: String,
    },
}

// Tutorial content
// Note: These strings contain double quotes, so we use r"..."# syntax

const TUTORIAL_OVERVIEW: &str = r#"
## What is Subcog?

Subcog is a **persistent memory system** for AI coding assistants. It helps you:

- **Remember decisions** you've made across sessions
- **Recall learnings** when they're relevant
- **Build up patterns** and best practices over time
- **Maintain context** even after compaction

## Key Concepts

1. **Memories**: Pieces of knowledge captured from your coding sessions
2. **Namespaces**: Categories like `decisions`, `patterns`, `learnings`
3. **Search**: Hybrid semantic + text search to find relevant memories
4. **Hooks**: Automatic integration with Claude Code

## Quick Start

```bash
# Capture a decision
subcog capture --namespace decisions "Use PostgreSQL for storage"

# Search for memories
subcog recall "database choice"

# Check status
subcog status
```

Would you like me to dive deeper into any of these areas?
"#;

const TUTORIAL_CAPTURE: &str = r#"
## Capturing Memories

Memories are the core unit of Subcog. Here's how to capture them effectively:

### Basic Capture

```bash
subcog capture --namespace decisions "Use PostgreSQL for primary storage"
```

### With Metadata

```bash
subcog capture --namespace patterns \
  --tags "rust,error-handling" \
  --source "src/main.rs:42" \
  "Always use thiserror for custom error types"
```

### What to Capture

- **Decisions**: Why you chose X over Y
- **Patterns**: Recurring approaches that work
- **Learnings**: "Aha!" moments and gotchas
- **Context**: Important background information

### Best Practices

1. Be specific - include the "why"
2. Add relevant tags for searchability
3. Reference source files when applicable
4. Use the right namespace
"#;

const TUTORIAL_SEARCH: &str = r#"
## Searching Memories

Subcog uses hybrid search combining semantic understanding with keyword matching.

### Basic Search

```bash
subcog recall "database storage decision"
```

### Search Modes

- **Hybrid** (default): Best of both worlds
- **Vector**: Pure semantic similarity
- **Text**: Traditional keyword matching

### Filtering

```bash
# By namespace
subcog recall --namespace decisions "storage"

# Limit results
subcog recall --limit 5 "API design"
```

### Tips for Better Results

1. Use natural language queries
2. Include context words
3. Try different search modes for different needs
4. Review scores to gauge relevance
"#;

const TUTORIAL_NAMESPACES: &str = r#"
## Understanding Namespaces

Namespaces organize memories by type:

| Namespace | Use For |
|-----------|---------|
| `decisions` | Architectural choices, "we decided to..." |
| `patterns` | Recurring solutions, conventions |
| `learnings` | Debugging insights, TILs |
| `context` | Background info, constraints |
| `tech-debt` | Future improvements needed |
| `apis` | Endpoint docs, contracts |
| `config` | Environment, settings |
| `security` | Auth patterns, vulnerabilities |
| `performance` | Optimization notes |
| `testing` | Test strategies, edge cases |

### Choosing the Right Namespace

- **Decision language** ("let's use", "we chose") -> `decisions`
- **Pattern language** ("always", "never", "when X do Y") -> `patterns`
- **Learning language** ("TIL", "gotcha", "realized") -> `learnings`
- **Context language** ("because", "constraint", "requirement") -> `context`
"#;

const TUTORIAL_WORKFLOWS: &str = r#"
## Integration Workflows

Subcog integrates with Claude Code through hooks:

### Available Hooks

1. **SessionStart**: Injects relevant context
2. **UserPromptSubmit**: Detects capture signals
3. **PostToolUse**: Surfaces related memories
4. **PreCompact**: Auto-captures before compaction
5. **Stop**: Session summary and sync

### Configuration

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

### MCP Server

For Claude Desktop:

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

const TUTORIAL_BEST_PRACTICES: &str = r"
## Best Practices

### Capture Discipline

1. **Capture decisions when made** - don't wait
2. **Include rationale** - why, not just what
3. **Be searchable** - think about future queries
4. **Tag consistently** - use existing tags when possible

### Memory Hygiene

1. **Review periodically** - consolidate duplicates
2. **Archive outdated** - don't delete, archive
3. **Update when wrong** - memories can be superseded

### Search Effectively

1. **Start broad, narrow down** - use filters progressively
2. **Try multiple modes** - hybrid, vector, text
3. **Trust the scores** - >0.7 is usually relevant

### Integration Tips

1. **Enable hooks** - let Subcog work automatically
2. **Check context** - review what's being injected
3. **Sync regularly** - keep memories backed up
";

const CAPTURE_ASSISTANT_SYSTEM: &str = r"
I'll analyze the context and suggest memories to capture. For each suggestion, I'll provide:

1. **Content**: The memory text to capture
2. **Namespace**: The appropriate category
3. **Tags**: Relevant keywords for searchability
4. **Rationale**: Why this should be captured

Let me analyze the context you provided...
";

const SEARCH_HELP_SYSTEM: &str = r#"
I'll help you craft effective search queries. Subcog supports:

**Hybrid Search (default)**
- Combines semantic understanding with keyword matching
- Best for natural language queries
- Example: "how we handle authentication errors"

**Vector Search**
- Pure semantic similarity
- Best for conceptual queries
- Example: "patterns for resilient services"

**Text Search**
- Traditional BM25 keyword matching
- Best for exact terms
- Example: "PostgreSQL"

Let me suggest some queries for your goal...
"#;

const BROWSE_DASHBOARD_INSTRUCTIONS: &str = r"
## Dashboard Layout

Present the data in this format:

```
┌─────────────────────────────────────────────────────────────────┐
│  SUBCOG MEMORY BROWSER                           {count} memories│
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  NAMESPACES                          TAGS (top N)               │
│  ───────────                         ──────────────             │
│  {namespace} [{count}] {bar}         {tag} [{count}] {bar}      │
│  ...                                 ...                        │
│                                                                 │
│  TIME                                STATUS                     │
│  ────                                ──────                     │
│  today     [{count}]                 active   [{count}]         │
│  this week [{count}]                 archived [{count}]         │
│  ...                                                            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Filter Syntax Reference

| Filter | Meaning | Example |
|--------|---------|---------|
| `ns:X` | namespace equals | `ns:decisions` |
| `tag:X` | has tag | `tag:rust` |
| `tag:X,Y` | has any tag (OR) | `tag:rust,mcp` |
| `tag:X tag:Y` | has all tags (AND) | `tag:rust tag:error` |
| `-tag:X` | exclude tag | `-tag:test` |
| `tag:*X` | tag wildcard | `tag:*-testing` |
| `since:Nd` | created in last N days | `since:7d` |
| `source:X` | source matches | `source:src/*` |
| `status:X` | status equals | `status:archived` |

Show example filter commands the user can use to drill down.
";

const BROWSE_SYSTEM_RESPONSE: &str = r"
I'll create a memory browser dashboard for you. Let me fetch the memories using `subcog_recall`.

I'll call the tool with the specified filter to get server-side filtered results, then compute:
1. Namespace distribution with counts
2. Tag frequency (top N most common)
3. Time-based grouping (today, this week, this month, older)
4. Status breakdown (active, archived)

I'll present this as a visual dashboard with ASCII bar charts showing relative proportions.
";

const LIST_FORMAT_INSTRUCTIONS: &str = r"
## URN Format

Rich URN encodes scope, namespace, and ID:
```
subcog://{scope}/{namespace}/{id}
```
Examples:
- `subcog://project/decisions/abc123...` - project-scoped decision
- `subcog://org/acme/patterns/def456...` - org-scoped pattern
- `subcog://acme/myrepo/learnings/ghi789...` - repo-scoped learning

## Output Formats

### Table Format (default)
Present results directly from `subcog_recall` output. Each line shows:
```
{n}. subcog://{scope}/{namespace}/{id} | {score} [{tags}]
   {content_summary}
```

Group by namespace with counts when helpful.

### Compact Format
```
subcog://{scope}/{namespace}/{id} [{tags}]
```

### Detailed Format
```
### subcog://{scope}/{namespace}/{id}
- **Score**: {score}
- **Tags**: tag1, tag2
- **Source**: {source}
- **Content**: {full_content}
```

## Filter Syntax

- `ns:decisions` - filter by namespace
- `tag:rust` - filter by tag
- `tag:rust,mcp` - OR filter (must have ANY)
- `tag:rust tag:error` - AND filter (must have ALL)
- `-tag:test` - exclude tag
- `since:7d` - time filter
- `source:src/*` - source pattern
- `status:active` - status filter
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_registry_creation() {
        let registry = PromptRegistry::new();
        let prompts = registry.list_prompts();

        assert!(!prompts.is_empty());
        assert!(registry.get_prompt("subcog_tutorial").is_some());
        assert!(registry.get_prompt("subcog_capture_assistant").is_some());
    }

    #[test]
    fn test_prompt_definitions() {
        let registry = PromptRegistry::new();

        let tutorial = registry.get_prompt("subcog_tutorial").unwrap();
        assert_eq!(tutorial.name, "subcog_tutorial");
        assert!(tutorial.description.is_some());
        assert!(!tutorial.arguments.is_empty());
    }

    #[test]
    fn test_generate_tutorial_prompt() {
        let registry = PromptRegistry::new();

        let args = serde_json::json!({
            "familiarity": "beginner",
            "focus": "capture"
        });

        let messages = registry
            .get_prompt_messages("subcog_tutorial", &args)
            .unwrap();

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");

        if let PromptContent::Text { text } = &messages[1].content {
            assert!(text.contains("Capturing Memories"));
        }
    }

    #[test]
    fn test_generate_decision_prompt() {
        let registry = PromptRegistry::new();

        let args = serde_json::json!({
            "decision": "Use PostgreSQL",
            "alternatives": "MySQL, SQLite"
        });

        let messages = registry
            .get_prompt_messages("subcog_document_decision", &args)
            .unwrap();

        assert_eq!(messages.len(), 1);
        if let PromptContent::Text { text } = &messages[0].content {
            assert!(text.contains("PostgreSQL"));
            assert!(text.contains("MySQL"));
        }
    }

    #[test]
    fn test_unknown_prompt() {
        let registry = PromptRegistry::new();

        let result = registry.get_prompt_messages("unknown_prompt", &serde_json::json!({}));
        assert!(result.is_none());
    }

    #[test]
    fn test_familiarity_levels() {
        let registry = PromptRegistry::new();

        for level in ["beginner", "intermediate", "advanced"] {
            let args = serde_json::json!({ "familiarity": level });
            let messages = registry
                .get_prompt_messages("subcog_tutorial", &args)
                .unwrap();

            if let PromptContent::Text { text } = &messages[1].content {
                // Each level should have different intro text
                assert!(!text.is_empty());
            }
        }
    }
}
