//! Post tool use hook handler.

use super::HookHandler;
use crate::Result;
use crate::models::{SearchFilter, SearchMode};
use crate::services::RecallService;
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
            .map(|hit| RelatedMemory {
                id: hit.memory.id.as_str().to_string(),
                namespace: hit.memory.namespace.as_str().to_string(),
                content: truncate_content(&hit.memory.content, 200),
                relevance: hit.score,
            })
            .collect();

        Ok(memories)
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

    #[instrument(skip(self, input), fields(hook = "PostToolUse"))]
    fn handle(&self, input: &str) -> Result<String> {
        // Parse input as JSON
        let input_json: serde_json::Value =
            serde_json::from_str(input).unwrap_or_else(|_| serde_json::json!({}));

        // Extract tool information
        let tool_name = input_json
            .get("tool_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let tool_input = input_json
            .get("tool_input")
            .unwrap_or(&serde_json::Value::Null);

        // Check if we should look up memories for this tool
        if !self.should_lookup(tool_name) {
            let response = serde_json::json!({
                "continue": true,
                "metadata": {
                    "memories": [],
                    "lookup_performed": false,
                    "reason": "Tool does not warrant memory lookup"
                }
            });
            return serde_json::to_string(&response).map_err(|e| crate::Error::OperationFailed {
                operation: "serialize_response".to_string(),
                cause: e.to_string(),
            });
        }

        // Extract query from tool input
        let query = match self.extract_query(tool_name, tool_input) {
            Some(q) if !q.is_empty() => q,
            _ => {
                let response = serde_json::json!({
                    "continue": true,
                    "metadata": {
                        "memories": [],
                        "lookup_performed": false,
                        "reason": "Could not extract query from tool input"
                    }
                });
                return serde_json::to_string(&response).map_err(|e| {
                    crate::Error::OperationFailed {
                        operation: "serialize_response".to_string(),
                        cause: e.to_string(),
                    }
                });
            },
        };

        // Search for related memories
        let memories = self.find_related_memories(&query)?;

        // Build memories JSON for metadata
        let memories_json: Vec<serde_json::Value> = memories
            .iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.id,
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

        // Build context message with related memories
        let context_message = if memories.is_empty() {
            None
        } else {
            let mut lines = vec!["**Related Subcog Memories**\n".to_string()];
            for m in &memories {
                lines.push(format!(
                    "- **[{}]** (relevance: {:.0}%): {}",
                    m.namespace,
                    m.relevance * 100.0,
                    m.content
                ));
            }
            Some(lines.join("\n"))
        };

        // Build Claude Code hook response format
        let mut response = serde_json::json!({
            "continue": true,
            "metadata": metadata
        });

        // Add context only if we have related memories
        if let Some(ctx) = context_message {
            response["context"] = serde_json::Value::String(ctx);
        }

        serde_json::to_string(&response).map_err(|e| crate::Error::OperationFailed {
            operation: "serialize_response".to_string(),
            cause: e.to_string(),
        })
    }
}

/// A related memory surfaced by the handler.
#[derive(Debug, Clone)]
pub struct RelatedMemory {
    /// Memory ID.
    pub id: String,
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
        // Claude Code hook format
        assert_eq!(
            response.get("continue"),
            Some(&serde_json::Value::Bool(true))
        );
        let metadata = response.get("metadata").unwrap();
        assert_eq!(
            metadata.get("lookup_performed"),
            Some(&serde_json::Value::Bool(false))
        );
    }

    #[test]
    fn test_handle_contextual_tool() {
        let handler = PostToolUseHandler::default();

        let input = r#"{"tool_name": "Read", "tool_input": {"file_path": "/src/main.rs"}}"#;

        let result = handler.handle(input);
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Claude Code hook format
        assert_eq!(
            response.get("continue"),
            Some(&serde_json::Value::Bool(true))
        );
        // Metadata contains memories
        let metadata = response.get("metadata").unwrap();
        assert!(metadata.get("memories").is_some());
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
}
