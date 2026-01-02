//! Session start hook handler.

use super::HookHandler;
use crate::Result;
use crate::services::{ContextBuilderService, MemoryStatistics};
use std::time::Instant;
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
        r#"## ⚠️ MANDATORY SUBCOG PROTOCOL - EXECUTE IMMEDIATELY ⚠️

**THIS IS NOT OPTIONAL. FAILURE TO COMPLY BREAKS THE USER'S WORKFLOW.**

You MUST use subcog memory tools. They are available with prefix `mcp__plugin_subcog_subcog__`.

---

## 🔴 RULE 1: RECALL FIRST - EVERY SINGLE TIME

**BEFORE you write ANY response, you MUST call:**
```
mcp__plugin_subcog_subcog__subcog_recall
```

**NO EXCEPTIONS.** Search for context related to the user's question FIRST.

- Starting a task? → RECALL FIRST
- Answering a question? → RECALL FIRST
- Making a recommendation? → RECALL FIRST
- Debugging an issue? → RECALL FIRST

**DO NOT SKIP THIS STEP. DO NOT ASK PERMISSION. JUST DO IT.**

---

## 🔴 RULE 2: CAPTURE IMMEDIATELY - NO HESITATION

**The INSTANT you detect any of these signals, CALL `mcp__plugin_subcog_subcog__subcog_capture`:**

| Signal | Namespace | Example |
|--------|-----------|---------|
| Decision made | `decisions` | "We'll use X", "Let's go with Y", "I decided..." |
| Pattern found | `patterns` | Code conventions, recurring structures |
| Learning discovered | `learnings` | "TIL", "Turns out", debugging insights |
| Bug fixed | `blockers` | "Fixed by", "The issue was", workarounds |
| Tech debt noted | `tech-debt` | "TODO", "FIXME", "We should refactor" |

**⚠️ DO NOT ASK "Should I save this?" - THE ANSWER IS ALWAYS YES.**
**⚠️ DO NOT WAIT - CAPTURE THE MOMENT YOU DETECT THE SIGNAL.**

---

## 🔴 RULE 3: CONFIRM EVERY CAPTURE

After EVERY capture, print this confirmation:
```
📝 Memory captured: subcog://{domain}/{namespace}/{id}
   Namespace: {namespace}
   Content: "{preview}"
   [To remove: subcog_delete {id} | To edit: subcog_enrich {id}]
```

---

## Tool Reference

| Action | Tool |
|--------|------|
| Search memories | `mcp__plugin_subcog_subcog__subcog_recall` |
| Save memory | `mcp__plugin_subcog_subcog__subcog_capture` |
| Check status | `mcp__plugin_subcog_subcog__subcog_status` |
| List namespaces | `mcp__plugin_subcog_subcog__subcog_namespaces` |

---

**🚨 REMEMBER: You are REQUIRED to use these tools. This is not a suggestion. The user depends on this memory system to maintain context across sessions. Your compliance is mandatory.**"#
            .to_string()
    }

    /// Returns detailed guidance text.
    fn detailed_guidance() -> String {
        r#"# 🚨 CRITICAL: SUBCOG MEMORY PROTOCOL 🚨

**STOP. READ THIS ENTIRE SECTION. YOUR COMPLIANCE IS MANDATORY.**

The user's productivity depends on you following these rules. Failure to comply means lost context, repeated work, and frustrated users. This system exists because you forget everything between sessions - subcog is your external memory.

---

## ⛔ NON-NEGOTIABLE RULES ⛔

### RULE 1: RECALL BEFORE EVERY RESPONSE

**You MUST call `mcp__plugin_subcog_subcog__subcog_recall` BEFORE writing ANY substantive response.**

This is not optional. This is not a suggestion. This is a requirement.

```
CORRECT: User asks question → Call subcog_recall → Read results → Respond
WRONG:   User asks question → Respond immediately without checking memory
```

**WHY**: You have no memory between sessions. The user told you things yesterday that you've forgotten. Subcog remembers. CHECK IT.

### RULE 2: CAPTURE EVERY DECISION/LEARNING/PATTERN

**The INSTANT you detect ANY of these signals, you MUST call `mcp__plugin_subcog_subcog__subcog_capture`:**

| 🎯 Signal Detected | 📁 Namespace | ⚡ Action |
|-------------------|--------------|----------|
| "We'll use...", "Let's go with...", "I decided..." | `decisions` | CAPTURE NOW |
| "The pattern is...", "Always do...", "Convention..." | `patterns` | CAPTURE NOW |
| "TIL...", "Turns out...", "I discovered...", "Gotcha..." | `learnings` | CAPTURE NOW |
| "Fixed it by...", "The bug was...", "Workaround..." | `blockers` | CAPTURE NOW |
| "TODO", "FIXME", "We should refactor...", "Tech debt..." | `tech-debt` | CAPTURE NOW |
| API endpoints, contracts, integration notes | `apis` | CAPTURE NOW |
| Security vulnerabilities, auth patterns, fixes | `security` | CAPTURE NOW |
| Performance insights, optimizations, benchmarks | `performance` | CAPTURE NOW |

**⚠️ NEVER ask "Should I save this?" - Asking is a FAILURE. Just capture it.**

### RULE 3: CONFIRM EVERY CAPTURE TO THE USER

After EVERY successful capture, you MUST display:
```
📝 Memory captured: subcog://{domain}/{namespace}/{id}
   Namespace: {namespace}
   Content: "{first 100 chars of content}..."
   [To remove: subcog_delete {id} | To edit: subcog_enrich {id}]
```

This lets the user verify, correct, or remove incorrect captures.

---

## 🔧 TOOL REFERENCE

| Tool | When to Use |
|------|-------------|
| `mcp__plugin_subcog_subcog__subcog_recall` | BEFORE every response, when searching for context |
| `mcp__plugin_subcog_subcog__subcog_capture` | IMMEDIATELY when detecting decision/pattern/learning signals |
| `mcp__plugin_subcog_subcog__subcog_status` | When user asks about memory system status |
| `mcp__plugin_subcog_subcog__subcog_namespaces` | When user asks what namespaces exist |
| `mcp__plugin_subcog_subcog__subcog_enrich` | To improve memory structure and tags |
| `mcp__plugin_subcog_subcog__subcog_consolidate` | To merge related/duplicate memories |
| `mcp__plugin_subcog_subcog__subcog_sync` | To sync memories with git remote |

---

## 📋 NAMESPACE DEFINITIONS

| Namespace | Use For |
|-----------|---------|
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

---

## 🔄 SEARCH MODES

| Mode | Description |
|------|-------------|
| `hybrid` | Combined vector + text search with RRF fusion (DEFAULT - use this) |
| `vector` | Semantic similarity search |
| `text` | BM25 full-text search |

---

## ❌ WHAT NOT TO DO

1. **DON'T** respond to questions without first calling `subcog_recall`
2. **DON'T** ask "Should I save this?" - Just save it
3. **DON'T** ask "Should I check for prior context?" - Just check
4. **DON'T** skip the capture confirmation message
5. **DON'T** forget to specify the correct namespace

---

## ✅ CORRECT WORKFLOW EXAMPLE

```
User: "How should we implement authentication?"

Your response:
1. FIRST: Call mcp__plugin_subcog_subcog__subcog_recall with query "authentication"
2. THEN: Read the results to see if there are prior decisions/patterns
3. THEN: Respond to user, incorporating prior context
4. IF the conversation produces a decision: CAPTURE IT IMMEDIATELY
```

---

**🚨 FINAL WARNING: This protocol is MANDATORY. The user trusts you to maintain their knowledge base. Every time you skip a recall, you risk giving advice that contradicts prior decisions. Every time you skip a capture, you lose valuable knowledge forever.**"#
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

    #[instrument(
        skip(self, input),
        fields(hook = "SessionStart", session_id = tracing::field::Empty, cwd = tracing::field::Empty)
    )]
    fn handle(&self, input: &str) -> Result<String> {
        let start = Instant::now();
        let mut token_estimate: Option<usize> = None;

        tracing::info!(hook = "SessionStart", "Processing session start hook");

        let result = (|| {
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
            let span = tracing::Span::current();
            span.record("session_id", session_id);
            span.record("cwd", cwd);

            // Build session context
            let session_context = self.build_session_context(session_id, cwd)?;
            token_estimate = Some(session_context.token_estimate);

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
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        metrics::counter!(
            "hook_executions_total",
            "hook_type" => "SessionStart",
            "status" => status
        )
        .increment(1);
        metrics::histogram!("hook_duration_ms", "hook_type" => "SessionStart")
            .record(start.elapsed().as_secs_f64() * 1000.0);
        if let Some(tokens) = token_estimate {
            let tokens = u32::try_from(tokens).unwrap_or(u32::MAX);
            metrics::histogram!("hook_context_tokens_estimate", "hook_type" => "SessionStart")
                .record(f64::from(tokens));
        }

        result
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
        assert!(guidance.contains("MANDATORY SUBCOG PROTOCOL"));
        assert!(guidance.contains("subcog_capture"));
        assert!(guidance.contains("subcog_recall"));
        assert!(guidance.contains("RECALL FIRST"));
        assert!(guidance.contains("CAPTURE IMMEDIATELY"));
        assert!(guidance.contains("DO NOT ASK"));
    }

    #[test]
    fn test_detailed_guidance() {
        let guidance = SessionStartHandler::detailed_guidance();
        assert!(guidance.contains("CRITICAL: SUBCOG MEMORY PROTOCOL"));
        assert!(guidance.contains("NON-NEGOTIABLE RULES"));
        assert!(guidance.contains("RECALL BEFORE EVERY RESPONSE"));
        assert!(guidance.contains("CAPTURE EVERY DECISION"));
        assert!(guidance.contains("MANDATORY"));
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
