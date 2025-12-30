//! Session start hook handler.

use super::HookHandler;
use crate::config::SubcogConfig;
use crate::services::ContextBuilderService;
use crate::Result;
use tracing::instrument;

/// Handles `SessionStart` hook events.
///
/// Injects relevant context at the start of a Claude Code session.
pub struct SessionStartHandler {
    /// Configuration.
    config: SubcogConfig,
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

impl SessionStartHandler {
    /// Creates a new handler.
    #[must_use]
    pub fn new(config: SubcogConfig) -> Self {
        Self {
            config,
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

        // Add session header
        context_parts.push(format!(
            "# Subcog Memory Context\n\nSession: {}\nWorking Directory: {}",
            session_id, cwd
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
        }

        // Add guidance based on level
        match self.guidance_level {
            GuidanceLevel::Minimal => {
                // Just the essential context
            },
            GuidanceLevel::Standard => {
                context_parts.push(self.standard_guidance());
            },
            GuidanceLevel::Detailed => {
                context_parts.push(self.detailed_guidance());
            },
        }

        let content = context_parts.join("\n\n");
        let token_estimate = ContextBuilderService::estimate_tokens(&content);

        Ok(SessionContext {
            content,
            memory_count,
            token_estimate,
            was_truncated: token_estimate > max_tokens,
        })
    }

    /// Returns standard guidance text.
    fn standard_guidance(&self) -> String {
        r#"## Quick Reference

- Use `subcog capture --namespace decisions "decision content"` to record decisions
- Use `subcog recall "query"` to search for relevant memories
- Decisions, patterns, and learnings are automatically surfaced when relevant"#.to_string()
    }

    /// Returns detailed guidance text.
    fn detailed_guidance(&self) -> String {
        r#"## Subcog Commands

### Capture
```bash
subcog capture --namespace decisions "Use PostgreSQL for primary storage"
subcog capture --namespace patterns "Always validate input before processing"
subcog capture --namespace learnings "SQLite FTS5 requires specific tokenization"
```

### Recall
```bash
subcog recall "database storage"
subcog recall --mode vector "similar concepts"
subcog recall --namespace decisions "architecture"
```

### Namespaces
- decisions: Architecture and design decisions
- patterns: Recurring patterns and best practices
- learnings: Insights and discoveries
- context: Project context and background
- tech-debt: Technical debt tracking
- blockers: Issues and their resolutions"#.to_string()
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
        Self::new(SubcogConfig::default())
    }
}

impl HookHandler for SessionStartHandler {
    fn event_type(&self) -> &'static str {
        "SessionStart"
    }

    #[instrument(skip(self, input), fields(hook = "SessionStart"))]
    fn handle(&self, input: &str) -> Result<String> {
        // Parse input as JSON
        let input_json: serde_json::Value = serde_json::from_str(input).unwrap_or_else(|_| {
            serde_json::json!({})
        });

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
        let context = self.build_session_context(session_id, cwd)?;

        // Check for first session tutorial
        let is_first = self.is_first_session();

        // Build response
        let mut response = serde_json::json!({
            "context": context.content,
            "memory_count": context.memory_count,
            "token_estimate": context.token_estimate,
            "was_truncated": context.was_truncated,
            "guidance_level": format!("{:?}", self.guidance_level),
        });

        // Add tutorial invitation for first session
        if is_first {
            response["tutorial_invitation"] = serde_json::json!({
                "prompt_name": "subcog_tutorial",
                "message": "Welcome to Subcog! Use the subcog_tutorial prompt to get started."
            });
        }

        serde_json::to_string(&response).map_err(|e| crate::Error::OperationFailed {
            operation: "serialize_response".to_string(),
            cause: e.to_string(),
        })
    }
}

/// Context prepared for a session.
#[derive(Debug, Clone)]
pub struct SessionContext {
    /// The formatted context string.
    pub content: String,
    /// Number of memories included.
    pub memory_count: usize,
    /// Estimated token count.
    pub token_estimate: usize,
    /// Whether context was truncated.
    pub was_truncated: bool,
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
        let config = SubcogConfig::default();

        let handler = SessionStartHandler::new(config.clone())
            .with_guidance_level(GuidanceLevel::Minimal);
        assert!(matches!(handler.guidance_level, GuidanceLevel::Minimal));

        let handler = SessionStartHandler::new(config.clone())
            .with_guidance_level(GuidanceLevel::Detailed);
        assert!(matches!(handler.guidance_level, GuidanceLevel::Detailed));
    }

    #[test]
    fn test_handle_basic() {
        let handler = SessionStartHandler::default();

        let input = r#"{"session_id": "test-session-123", "cwd": "/path/to/project"}"#;

        let result = handler.handle(input);
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(response.get("context").is_some());
        assert!(response.get("memory_count").is_some());
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
        let handler = SessionStartHandler::default();
        let guidance = handler.standard_guidance();
        assert!(guidance.contains("Quick Reference"));
        assert!(guidance.contains("capture"));
        assert!(guidance.contains("recall"));
    }

    #[test]
    fn test_detailed_guidance() {
        let handler = SessionStartHandler::default();
        let guidance = handler.detailed_guidance();
        assert!(guidance.contains("Subcog Commands"));
        assert!(guidance.contains("Namespaces"));
        assert!(guidance.contains("decisions"));
    }

    #[test]
    fn test_max_tokens_configuration() {
        let handler = SessionStartHandler::default()
            .with_max_tokens(5000);
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
