//! Post tool use hook handler.

use super::HookHandler;
use crate::Result;
use crate::models::{IssueSeverity, SearchFilter, SearchMode, validate_prompt_content};
use crate::observability::current_request_id;
use crate::services::RecallService;
use std::time::Instant;
use tracing::instrument;

/// Handles `PostToolUse` hook events.
///
/// Surfaces related memories after tool usage.
pub struct PostToolUseHandler {
    /// Recall service for searching memories.
    recall: Option<RecallService>,
    /// Maximum number of memories to surface.
    max_memories: usize,
    /// Minimum relevance score to surface.
    min_relevance: f32,
}

/// Tools that may benefit from memory context.
const CONTEXTUAL_TOOLS: &[&str] = &[
    "Read", "Write", "Edit", "Bash", "Search", "Grep", "Glob", "LSP",
];

impl PostToolUseHandler {
    /// Creates a new handler.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            recall: None,
            max_memories: 3,
            min_relevance: 0.5,
        }
    }

    /// Sets the recall service.
    #[must_use]
    pub fn with_recall(mut self, recall: RecallService) -> Self {
        self.recall = Some(recall);
        self
    }

    /// Sets the maximum number of memories to surface.
    #[must_use]
    pub const fn with_max_memories(mut self, max: usize) -> Self {
        self.max_memories = max;
        self
    }

    /// Sets the minimum relevance score.
    #[must_use]
    pub const fn with_min_relevance(mut self, min: f32) -> Self {
        self.min_relevance = min;
        self
    }

    /// Determines if a tool use warrants memory lookup.
    /// Kept as method for API consistency.
    #[allow(clippy::unused_self)]
    fn should_lookup(&self, tool_name: &str) -> bool {
        CONTEXTUAL_TOOLS
            .iter()
            .any(|t| t.eq_ignore_ascii_case(tool_name))
    }

    /// Checks if a tool is a prompt save tool.
    fn is_prompt_save_tool(tool_name: &str) -> bool {
        let lower = tool_name.to_lowercase();
        lower == "prompt_save" || lower == "prompt.save" || lower == "subcog_prompt_save"
    }

    /// Validates prompt content and returns any issues.
    ///
    /// Returns a guidance message if validation issues are found.
    fn validate_prompt(&self, tool_input: &serde_json::Value) -> Option<String> {
        // Extract content from tool input
        let content = tool_input.get("content").and_then(|v| v.as_str())?;

        // Skip validation for empty content
        if content.is_empty() {
            return None;
        }

        // Validate the prompt content
        let validation = validate_prompt_content(content);

        if validation.is_valid {
            return None;
        }

        // Build guidance message for issues
        let mut guidance = vec!["**Prompt Validation Issues**\n".to_string()];

        for issue in &validation.issues {
            let severity_icon = match issue.severity {
                IssueSeverity::Error => "\u{274c}",   // X
                IssueSeverity::Warning => "\u{26a0}", // Warning sign
            };

            let position_info = issue
                .position
                .map_or(String::new(), |pos| format!(" at position {pos}"));

            guidance.push(format!(
                "- {severity_icon} {}{position_info}",
                issue.message
            ));
        }

        guidance.push("\n**Tips:**".to_string());
        guidance.push("- Variables use `{{variable_name}}` syntax".to_string());
        guidance.push("- Ensure all `{{` have matching `}}`".to_string());
        guidance.push("- Variable names should be alphanumeric with underscores".to_string());
        guidance.push("- See `subcog://help/prompts` for format documentation".to_string());

        Some(guidance.join("\n"))
    }

    /// Extracts a search query from tool input.
    /// Kept as method for API consistency.
    #[allow(clippy::unused_self)]
    fn extract_query(&self, tool_name: &str, tool_input: &serde_json::Value) -> Option<String> {
        match tool_name.to_lowercase().as_str() {
            "read" | "write" | "edit" => {
                // Use file path as query
                tool_input
                    .get("file_path")
                    .or_else(|| tool_input.get("path"))
                    .and_then(|v| v.as_str())
                    .map(|p| {
                        // Extract meaningful parts from path
                        let parts: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
                        parts.join(" ")
                    })
            },
            "bash" => {
                // Use command as query
                tool_input.get("command").and_then(|v| v.as_str()).map(|c| {
                    // Extract key terms from command
                    c.split_whitespace().take(5).collect::<Vec<_>>().join(" ")
                })
            },
            "search" | "grep" => {
                // Use pattern as query
                tool_input
                    .get("pattern")
                    .or_else(|| tool_input.get("query"))
                    .and_then(|v| v.as_str())
                    .map(String::from)
            },
            "glob" => {
                // Use pattern as query
                tool_input
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .map(|p| p.replace(['*', '.'], " "))
            },
            "lsp" => {
                // Use symbol or file as query
                tool_input
                    .get("symbol")
                    .or_else(|| tool_input.get("file_path"))
                    .and_then(|v| v.as_str())
                    .map(String::from)
            },
            _ => None,
        }
    }

    /// Searches for related memories.
    fn find_related_memories(&self, query: &str) -> Result<Vec<RelatedMemory>> {
        let Some(recall) = &self.recall else {
            return Ok(Vec::new());
        };

        let result = recall.search(
            query,
            SearchMode::Hybrid,
            &SearchFilter::new(),
            self.max_memories,
        )?;

        let memories: Vec<RelatedMemory> = result
            .memories
            .into_iter()
            .filter(|hit| hit.score >= self.min_relevance)
            .map(|hit| {
                // Build full URN: subcog://{domain}/{namespace}/{id}
                let domain_part = if hit.memory.domain.is_project_scoped() {
                    "project".to_string()
                } else {
                    hit.memory.domain.to_string()
                };
                let urn = format!(
                    "subcog://{}/{}/{}",
                    domain_part,
                    hit.memory.namespace.as_str(),
                    hit.memory.id.as_str()
                );
                RelatedMemory {
                    urn,
                    namespace: hit.memory.namespace.as_str().to_string(),
                    content: truncate_content(&hit.memory.content, 200),
                    relevance: hit.score,
                }
            })
            .collect();

        Ok(memories)
    }

    fn empty_response() -> Result<String> {
        Self::serialize_response(&serde_json::json!({}))
    }

    fn serialize_response(response: &serde_json::Value) -> Result<String> {
        serde_json::to_string(response).map_err(|e| crate::Error::OperationFailed {
            operation: "serialize_response".to_string(),
            cause: e.to_string(),
        })
    }

    fn build_memories_response(
        tool_name: &str,
        query: &str,
        memories: &[RelatedMemory],
    ) -> serde_json::Value {
        if memories.is_empty() {
            return serde_json::json!({});
        }

        let memories_json: Vec<serde_json::Value> = memories
            .iter()
            .map(|m| {
                serde_json::json!({
                    "urn": m.urn,
                    "namespace": m.namespace,
                    "content": m.content,
                    "relevance": m.relevance
                })
            })
            .collect();

        let metadata = serde_json::json!({
            "memories": memories_json,
            "lookup_performed": true,
            "query": query,
            "tool_name": tool_name
        });

        let mut lines = vec!["**Related Subcog Memories**\n".to_string()];
        for m in memories {
            lines.push(format!(
                "- **{}** (relevance: {:.0}%): {}",
                m.urn,
                m.relevance * 100.0,
                m.content
            ));
        }
        let context = lines.join("\n");

        let metadata_str = serde_json::to_string(&metadata).unwrap_or_default();
        let context_with_metadata =
            format!("{context}\n\n<!-- subcog-metadata: {metadata_str} -->");

        serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PostToolUse",
                "additionalContext": context_with_metadata
            }
        })
    }

    fn handle_inner(
        &self,
        input: &str,
        lookup_performed: &mut bool,
        memories_found: &mut usize,
    ) -> Result<String> {
        let input_json: serde_json::Value =
            serde_json::from_str(input).unwrap_or_else(|_| serde_json::json!({}));

        let tool_name = input_json
            .get("tool_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let span = tracing::Span::current();
        span.record("tool_name", tool_name);

        let tool_input = input_json
            .get("tool_input")
            .unwrap_or(&serde_json::Value::Null);

        if Self::is_prompt_save_tool(tool_name) {
            if let Some(guidance) = self.validate_prompt(tool_input) {
                let response = serde_json::json!({
                    "hookSpecificOutput": {
                        "hookEventName": "PostToolUse",
                        "additionalContext": guidance
                    }
                });
                return Self::serialize_response(&response);
            }
            return Self::empty_response();
        }

        if !self.should_lookup(tool_name) {
            return Self::empty_response();
        }

        let query = self
            .extract_query(tool_name, tool_input)
            .filter(|q| !q.is_empty());
        let Some(query) = query else {
            return Self::empty_response();
        };

        let memories = self.find_related_memories(&query)?;
        *lookup_performed = true;
        *memories_found = memories.len();
        span.record("lookup_performed", *lookup_performed);
        span.record("memories_found", *memories_found);

        let response = Self::build_memories_response(tool_name, &query, &memories);
        Self::serialize_response(&response)
    }
}

/// Truncates content to a maximum length.
fn truncate_content(content: &str, max_len: usize) -> String {
    if content.len() <= max_len {
        content.to_string()
    } else {
        format!("{}...", &content[..max_len.saturating_sub(3)])
    }
}

impl Default for PostToolUseHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HookHandler for PostToolUseHandler {
    fn event_type(&self) -> &'static str {
        "PostToolUse"
    }

    #[instrument(
        name = "subcog.hook.post_tool_use",
        skip(self, input),
        fields(
            request_id = tracing::field::Empty,
            component = "hooks",
            operation = "post_tool_use",
            hook = "PostToolUse",
            tool_name = tracing::field::Empty,
            lookup_performed = tracing::field::Empty,
            memories_found = tracing::field::Empty
        )
    )]
    fn handle(&self, input: &str) -> Result<String> {
        let start = Instant::now();
        let mut lookup_performed = false;
        let mut memories_found = 0usize;
        if let Some(request_id) = current_request_id() {
            tracing::Span::current().record("request_id", request_id.as_str());
        }

        let result = self.handle_inner(input, &mut lookup_performed, &mut memories_found);

        let status = if result.is_ok() { "success" } else { "error" };
        metrics::counter!(
            "hook_executions_total",
            "hook_type" => "PostToolUse",
            "status" => status
        )
        .increment(1);
        metrics::histogram!("hook_duration_ms", "hook_type" => "PostToolUse")
            .record(start.elapsed().as_secs_f64() * 1000.0);
        if lookup_performed {
            metrics::counter!(
                "hook_memory_lookup_total",
                "hook_type" => "PostToolUse",
                "result" => if memories_found > 0 { "hit" } else { "miss" }
            )
            .increment(1);
        }

        result
    }
}

/// A related memory surfaced by the handler.
#[derive(Debug, Clone)]
pub struct RelatedMemory {
    /// Full URN (`subcog://{domain}/{namespace}/{id}`).
    pub urn: String,
    /// Namespace.
    pub namespace: String,
    /// Truncated content.
    pub content: String,
    /// Relevance score.
    pub relevance: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_creation() {
        let handler = PostToolUseHandler::default();
        assert_eq!(handler.event_type(), "PostToolUse");
    }

    #[test]
    fn test_should_lookup() {
        let handler = PostToolUseHandler::default();

        assert!(handler.should_lookup("Read"));
        assert!(handler.should_lookup("read"));
        assert!(handler.should_lookup("Write"));
        assert!(handler.should_lookup("Bash"));
        assert!(handler.should_lookup("Grep"));
        assert!(!handler.should_lookup("Unknown"));
        assert!(!handler.should_lookup(""));
    }

    #[test]
    fn test_extract_query_read() {
        let handler = PostToolUseHandler::default();

        let input = serde_json::json!({
            "file_path": "/src/services/capture.rs"
        });

        let query = handler.extract_query("Read", &input);
        assert!(query.is_some());
        assert!(query.as_ref().is_some_and(|q| q.contains("capture")));
    }

    #[test]
    fn test_extract_query_bash() {
        let handler = PostToolUseHandler::default();

        let input = serde_json::json!({
            "command": "cargo test --all-features"
        });

        let query = handler.extract_query("Bash", &input);
        assert!(query.is_some());
        assert!(query.as_ref().is_some_and(|q| q.contains("cargo")));
    }

    #[test]
    fn test_extract_query_grep() {
        let handler = PostToolUseHandler::default();

        let input = serde_json::json!({
            "pattern": "fn capture"
        });

        let query = handler.extract_query("grep", &input);
        assert!(query.is_some());
        assert_eq!(query, Some("fn capture".to_string()));
    }

    #[test]
    fn test_handle_non_contextual_tool() {
        let handler = PostToolUseHandler::default();

        let input = r#"{"tool_name": "SomeOtherTool", "tool_input": {}}"#;

        let result = handler.handle(input);
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Claude Code hook format - empty response for non-contextual tools
        assert!(response.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_handle_contextual_tool() {
        let handler = PostToolUseHandler::default();

        let input = r#"{"tool_name": "Read", "tool_input": {"file_path": "/src/main.rs"}}"#;

        let result = handler.handle(input);
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Without recall service, no memories found - empty response
        // (memories would be returned in hookSpecificOutput.additionalContext if found)
        assert!(response.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_truncate_content() {
        let short = "Short text";
        assert_eq!(truncate_content(short, 100), short);

        let long =
            "This is a much longer text that should be truncated because it exceeds the limit";
        let truncated = truncate_content(long, 30);
        assert!(truncated.ends_with("..."));
        assert!(truncated.len() <= 30);
    }

    #[test]
    fn test_configuration() {
        let handler = PostToolUseHandler::default()
            .with_max_memories(5)
            .with_min_relevance(0.7);

        assert_eq!(handler.max_memories, 5);
        assert!((handler.min_relevance - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_is_prompt_save_tool() {
        assert!(PostToolUseHandler::is_prompt_save_tool("prompt_save"));
        assert!(PostToolUseHandler::is_prompt_save_tool("PROMPT_SAVE"));
        assert!(PostToolUseHandler::is_prompt_save_tool("prompt.save"));
        assert!(PostToolUseHandler::is_prompt_save_tool(
            "subcog_prompt_save"
        ));
        assert!(!PostToolUseHandler::is_prompt_save_tool("prompt_get"));
        assert!(!PostToolUseHandler::is_prompt_save_tool("subcog_capture"));
    }

    #[test]
    fn test_handle_prompt_save_valid() {
        let handler = PostToolUseHandler::default();

        let input = serde_json::json!({
            "tool_name": "prompt_save",
            "tool_input": {
                "name": "test-prompt",
                "content": "Hello {{name}}, welcome to {{place}}!"
            }
        });

        let result = handler.handle(&serde_json::to_string(&input).unwrap());
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Valid prompt - empty response (no validation issues)
        assert!(response.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_handle_prompt_save_invalid_braces() {
        let handler = PostToolUseHandler::default();

        let input = serde_json::json!({
            "tool_name": "prompt_save",
            "tool_input": {
                "name": "test-prompt",
                "content": "Hello {{name, this is broken"
            }
        });

        let result = handler.handle(&serde_json::to_string(&input).unwrap());
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Invalid prompt - should have validation guidance
        assert!(response.get("hookSpecificOutput").is_some());

        let additional_context = response
            .get("hookSpecificOutput")
            .and_then(|o| o.get("additionalContext"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(additional_context.contains("Prompt Validation Issues"));
        assert!(additional_context.contains("subcog://help/prompts"));
    }

    #[test]
    fn test_validate_prompt_empty_content() {
        let handler = PostToolUseHandler::default();

        let input = serde_json::json!({
            "content": ""
        });

        // Empty content should return None (no validation needed)
        let guidance = handler.validate_prompt(&input);
        assert!(guidance.is_none());
    }

    #[test]
    fn test_validate_prompt_missing_content() {
        let handler = PostToolUseHandler::default();

        let input = serde_json::json!({
            "name": "test"
        });

        // Missing content should return None
        let guidance = handler.validate_prompt(&input);
        assert!(guidance.is_none());
    }
}
