//! Session start hook handler.

use super::HookHandler;
use crate::Result;
use crate::services::{ContextBuilderService, MemoryStatistics};
use tracing::instrument;

/// Handles `SessionStart` hook events.
///
/// Injects relevant context at the start of a Claude Code session.
pub struct SessionStartHandler {
    /// Context builder service.
    context_builder: Option<ContextBuilderService>,
    /// Maximum tokens for context.
    max_context_tokens: usize,
    /// Guidance level for context injection.
    guidance_level: GuidanceLevel,
}

/// Level of guidance to provide in context.
#[derive(Debug, Clone, Copy, Default)]
pub enum GuidanceLevel {
    /// Minimal context - just key decisions.
    Minimal,
    /// Standard context - decisions, patterns, and relevant context.
    #[default]
    Standard,
    /// Detailed context - full context with examples.
    Detailed,
}

/// Context prepared for a session.
#[derive(Debug, Clone)]
struct SessionContext {
    /// The formatted context string.
    content: String,
    /// Number of memories included.
    memory_count: usize,
    /// Estimated token count.
    token_estimate: usize,
    /// Whether context was truncated.
    was_truncated: bool,
    /// Memory statistics for the project.
    statistics: Option<MemoryStatistics>,
}

impl SessionStartHandler {
    /// Creates a new handler.
    #[must_use]
    pub fn new() -> Self {
        Self {
            context_builder: None,
            max_context_tokens: 2000,
            guidance_level: GuidanceLevel::default(),
        }
    }

    /// Sets the context builder service.
    #[must_use]
    pub fn with_context_builder(mut self, builder: ContextBuilderService) -> Self {
        self.context_builder = Some(builder);
        self
    }

    /// Sets the maximum context tokens.
    #[must_use]
    pub const fn with_max_tokens(mut self, tokens: usize) -> Self {
        self.max_context_tokens = tokens;
        self
    }

    /// Sets the guidance level.
    #[must_use]
    pub const fn with_guidance_level(mut self, level: GuidanceLevel) -> Self {
        self.guidance_level = level;
        self
    }

    /// Builds context for the session.
    fn build_session_context(&self, session_id: &str, cwd: &str) -> Result<SessionContext> {
        let mut context_parts = Vec::new();
        let mut memory_count = 0;
        let mut statistics: Option<MemoryStatistics> = None;

        // Add session header
        context_parts.push(format!(
            "# Subcog Memory Context\n\nSession: {session_id}\nWorking Directory: {cwd}"
        ));

        // Build context based on guidance level
        let max_tokens = match self.guidance_level {
            GuidanceLevel::Minimal => self.max_context_tokens / 2,
            GuidanceLevel::Standard => self.max_context_tokens,
            GuidanceLevel::Detailed => self.max_context_tokens * 2,
        };

        if let Some(ref builder) = self.context_builder {
            let context = builder.build_context(max_tokens)?;
            if !context.is_empty() {
                context_parts.push(context);
                memory_count += 1; // Approximate
            }

            // Get memory statistics for dynamic context
            if let Ok(stats) = builder.get_statistics() {
                memory_count = stats.total_count;
                add_statistics_if_present(&mut context_parts, &stats);
                statistics = Some(stats);
            }
        }

        // Add guidance based on level
        match self.guidance_level {
            GuidanceLevel::Minimal => {
                // Just the essential context
            },
            GuidanceLevel::Standard => {
                context_parts.push(Self::standard_guidance());
            },
            GuidanceLevel::Detailed => {
                context_parts.push(Self::detailed_guidance());
            },
        }

        let content = context_parts.join("\n\n");
        let token_estimate = ContextBuilderService::estimate_tokens(&content);

        Ok(SessionContext {
            content,
            memory_count,
            token_estimate,
            was_truncated: token_estimate > max_tokens,
            statistics,
        })
    }

    /// Formats memory statistics for context injection.
    fn format_statistics(stats: &MemoryStatistics) -> String {
        let mut parts = vec!["## Project Memory Summary".to_string()];
        parts.push(format!("\n**Total memories**: {}", stats.total_count));

        // Namespace breakdown
        if !stats.namespace_counts.is_empty() {
            parts.push("\n**By namespace**:".to_string());
            let mut sorted: Vec<_> = stats.namespace_counts.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1));
            for (ns, count) in sorted.iter().take(6) {
                parts.push(format!("- `{ns}`: {count}"));
            }
        }

        // Top tags
        if !stats.top_tags.is_empty() {
            parts.push("\n**Top tags**:".to_string());
            let tag_list: Vec<String> = stats
                .top_tags
                .iter()
                .take(8)
                .map(|(tag, count)| format!("`{tag}` ({count})"))
                .collect();
            parts.push(tag_list.join(", "));
        }

        // Recent topics
        if !stats.recent_topics.is_empty() {
            parts.push("\n**Recent topics**:".to_string());
            for topic in stats.recent_topics.iter().take(5) {
                parts.push(format!("- {topic}"));
            }
        }

        // Proactive nudge
        parts.push("\n**Tip**: Use `mcp__plugin_subcog_subcog__subcog_recall` to search for relevant memories when these topics come up in conversation.".to_string());

        parts.join("\n")
    }

    /// Returns standard guidance text.
    fn standard_guidance() -> String {
        r#"## Subcog Memory Protocol

You have access to subcog, a persistent memory system. MCP tools are available with the prefix `mcp__plugin_subcog_subcog__`.

### Available Tools
| Short Name | Full MCP Tool Name |
|------------|-------------------|
| `subcog_capture` | `mcp__plugin_subcog_subcog__subcog_capture` |
| `subcog_recall` | `mcp__plugin_subcog_subcog__subcog_recall` |
| `subcog_status` | `mcp__plugin_subcog_subcog__subcog_status` |
| `subcog_namespaces` | `mcp__plugin_subcog_subcog__subcog_namespaces` |

### Capture Memories
When the user makes a decision, discovers a pattern, or learns something important:
- Use `mcp__plugin_subcog_subcog__subcog_capture` to record it
- Choose the appropriate namespace: decisions, patterns, learnings, context, tech-debt, blockers, apis, config, security, performance, testing

### Recall Memories
Before making recommendations or decisions:
- Use `mcp__plugin_subcog_subcog__subcog_recall` to search for relevant prior context
- Consider past decisions and learnings that may apply

### Proactive Behavior
- **Decisions**: When the user says "we'll use X" or "let's go with Y", capture it
- **Patterns**: When identifying recurring code patterns or conventions, capture them
- **Learnings**: When discovering gotchas, caveats, or insights, capture them
- **Blockers**: When resolving issues or bugs, capture the solution"#
            .to_string()
    }

    /// Returns detailed guidance text.
    fn detailed_guidance() -> String {
        r#"## Subcog Memory Protocol

You have access to subcog, a persistent memory system for capturing and recalling project knowledge across sessions.

### Available MCP Tools

MCP tools are available with the prefix `mcp__plugin_subcog_subcog__`.

| Short Name | Full MCP Tool Name | Purpose |
|------------|-------------------|---------|
| `subcog_capture` | `mcp__plugin_subcog_subcog__subcog_capture` | Record decisions, patterns, learnings, and context |
| `subcog_recall` | `mcp__plugin_subcog_subcog__subcog_recall` | Search for relevant memories using semantic + text search |
| `subcog_status` | `mcp__plugin_subcog_subcog__subcog_status` | Check memory system status and statistics |
| `subcog_namespaces` | `mcp__plugin_subcog_subcog__subcog_namespaces` | List available memory namespaces |
| `subcog_consolidate` | `mcp__plugin_subcog_subcog__subcog_consolidate` | Merge related memories (with LLM) |
| `subcog_enrich` | `mcp__plugin_subcog_subcog__subcog_enrich` | Improve memory structure and tags (with LLM) |
| `subcog_sync` | `mcp__plugin_subcog_subcog__subcog_sync` | Sync memories with git remote |

### Memory Namespaces

| Namespace | When to Use |
|-----------|-------------|
| `decisions` | Architecture choices, technology selections, design decisions |
| `patterns` | Recurring code patterns, conventions, best practices |
| `learnings` | Discoveries, gotchas, caveats, insights, TILs |
| `context` | Project background, domain knowledge, requirements |
| `tech-debt` | TODOs, FIXMEs, known issues to address later |
| `blockers` | Bug resolutions, issue fixes, workarounds |
| `apis` | API contracts, endpoint documentation, integration notes |
| `config` | Configuration patterns, environment setup |
| `security` | Security considerations, vulnerabilities, fixes |
| `performance` | Performance insights, optimization patterns |
| `testing` | Test patterns, coverage insights, testing strategies |

### Capture Protocol

**Trigger phrases to detect:**
- "We decided to...", "Let's use...", "Going with..."  → `decisions`
- "The pattern is...", "Always do...", "Best practice..."  → `patterns`
- "TIL...", "Turns out...", "Gotcha...", "Discovered..."  → `learnings`
- "Fixed the bug by...", "Resolved by...", "Workaround..."  → `blockers`
- "TODO:", "FIXME:", "Technical debt..."  → `tech-debt`

**Capture command format:**
```bash
subcog capture --namespace <namespace> "<content>"
```

### Recall Protocol

**Before making recommendations:**
1. Search for relevant prior decisions: `subcog recall "topic"`
2. Check for existing patterns that apply
3. Consider past learnings that inform the current task

**Search modes:**
- `hybrid` (default): Combined vector + text search with RRF fusion
- `vector`: Semantic similarity search
- `text`: BM25 full-text search

### Proactive Behavior

1. **Capture automatically** when detecting decision/pattern/learning signals
2. **Recall proactively** before suggesting architectural changes
3. **Surface related memories** after reading files to provide context
4. **Sync memories** at session end to persist across machines

### Memory Awareness

**Remind the user to capture:**
- After significant architectural discussions, ask: "Should I capture this decision?"
- When patterns emerge in code review, suggest: "This looks like a recurring pattern. Want me to save it?"
- When debugging reveals insights, offer: "This was a useful learning. Capture it for future reference?"

**Check memories before acting:**
- Before suggesting new patterns, recall existing patterns in this codebase
- Before making technology recommendations, check for prior decisions
- When encountering errors, search for similar blockers that were resolved
- When writing new code, recall relevant security and performance patterns

**Surface relevant context:**
- When reading files, automatically surface related memories
- When the user asks about prior work, search and present relevant memories
- When starting new features, recall related context and decisions"#
            .to_string()
    }

    /// Checks if this is the first session (no user memories).
    fn is_first_session(&self) -> bool {
        // Check if we have any user memories
        if let Some(ref builder) = self.context_builder {
            if let Ok(context) = builder.build_context(100) {
                return context.is_empty();
            }
        }
        true
    }
}

impl Default for SessionStartHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HookHandler for SessionStartHandler {
    fn event_type(&self) -> &'static str {
        "SessionStart"
    }

    #[instrument(skip(self, input), fields(hook = "SessionStart"))]
    fn handle(&self, input: &str) -> Result<String> {
        // Parse input as JSON
        let input_json: serde_json::Value =
            serde_json::from_str(input).unwrap_or_else(|_| serde_json::json!({}));

        // Extract session info from input
        let session_id = input_json
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let cwd = input_json
            .get("cwd")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        // Build session context
        let session_context = self.build_session_context(session_id, cwd)?;

        // Check for first session tutorial
        let is_first = self.is_first_session();

        // Build metadata
        let mut metadata = serde_json::json!({
            "memory_count": session_context.memory_count,
            "token_estimate": session_context.token_estimate,
            "was_truncated": session_context.was_truncated,
            "guidance_level": format!("{:?}", self.guidance_level),
        });

        // Add statistics to metadata if available
        if let Some(ref stats) = session_context.statistics {
            metadata["statistics"] = serde_json::json!({
                "total_count": stats.total_count,
                "namespace_counts": stats.namespace_counts,
                "top_tags": stats.top_tags,
                "recent_topics": stats.recent_topics
            });
        }

        // Add tutorial invitation for first session
        if is_first {
            metadata["tutorial_invitation"] = serde_json::json!({
                "prompt_name": "subcog_tutorial",
                "message": "Welcome to Subcog! Use the subcog_tutorial prompt to get started."
            });
        }

        // Build Claude Code hook response format per specification
        // See: https://docs.anthropic.com/en/docs/claude-code/hooks
        let response = if session_context.content.is_empty() {
            // Empty response when no context to inject
            serde_json::json!({})
        } else {
            // Embed metadata as XML comment for debugging
            let metadata_str = serde_json::to_string(&metadata).unwrap_or_default();
            let context_with_metadata = format!(
                "{}\n\n<!-- subcog-metadata: {} -->",
                session_context.content, metadata_str
            );
            serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "SessionStart",
                    "additionalContext": context_with_metadata
                }
            })
        };

        serde_json::to_string(&response).map_err(|e| crate::Error::OperationFailed {
            operation: "serialize_response".to_string(),
            cause: e.to_string(),
        })
    }
}

/// Adds formatted statistics to context if memories exist.
fn add_statistics_if_present(context_parts: &mut Vec<String>, stats: &MemoryStatistics) {
    if stats.total_count > 0 {
        context_parts.push(SessionStartHandler::format_statistics(stats));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_creation() {
        let handler = SessionStartHandler::default();
        assert_eq!(handler.event_type(), "SessionStart");
    }

    #[test]
    fn test_guidance_levels() {
        let handler = SessionStartHandler::new().with_guidance_level(GuidanceLevel::Minimal);
        assert!(matches!(handler.guidance_level, GuidanceLevel::Minimal));

        let handler = SessionStartHandler::new().with_guidance_level(GuidanceLevel::Detailed);
        assert!(matches!(handler.guidance_level, GuidanceLevel::Detailed));
    }

    #[test]
    fn test_handle_basic() {
        let handler = SessionStartHandler::default();

        let input = r#"{"session_id": "test-session-123", "cwd": "/path/to/project"}"#;

        let result = handler.handle(input);
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Claude Code hook format - should have hookSpecificOutput
        let hook_output = response.get("hookSpecificOutput").unwrap();
        assert_eq!(
            hook_output.get("hookEventName"),
            Some(&serde_json::Value::String("SessionStart".to_string()))
        );
        // Should have additionalContext with session info and metadata embedded
        let context = hook_output
            .get("additionalContext")
            .unwrap()
            .as_str()
            .unwrap();
        assert!(context.contains("Subcog Memory Context"));
        assert!(context.contains("test-session-123"));
        assert!(context.contains("subcog-metadata"));
    }

    #[test]
    fn test_handle_missing_fields() {
        let handler = SessionStartHandler::default();

        let input = "{}";

        let result = handler.handle(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_first_session_detection() {
        let handler = SessionStartHandler::default();
        // Without context builder, should be first session
        assert!(handler.is_first_session());
    }

    #[test]
    fn test_standard_guidance() {
        let guidance = SessionStartHandler::standard_guidance();
        assert!(guidance.contains("Subcog Memory Protocol"));
        assert!(guidance.contains("subcog_capture"));
        assert!(guidance.contains("subcog_recall"));
        assert!(guidance.contains("Proactive Behavior"));
    }

    #[test]
    fn test_detailed_guidance() {
        let guidance = SessionStartHandler::detailed_guidance();
        assert!(guidance.contains("Subcog Memory Protocol"));
        assert!(guidance.contains("Memory Namespaces"));
        assert!(guidance.contains("Capture Protocol"));
        assert!(guidance.contains("Recall Protocol"));
        assert!(guidance.contains("Memory Awareness"));
        assert!(guidance.contains("decisions"));
    }

    #[test]
    fn test_max_tokens_configuration() {
        let handler = SessionStartHandler::default().with_max_tokens(5000);
        assert_eq!(handler.max_context_tokens, 5000);
    }

    #[test]
    fn test_build_session_context() {
        let handler = SessionStartHandler::default();
        let result = handler.build_session_context("test-session", "/project");

        assert!(result.is_ok());
        let context = result.unwrap();
        assert!(context.content.contains("test-session"));
    }
}
