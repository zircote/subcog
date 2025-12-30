//! Stop hook handler.

use super::HookHandler;
use crate::Result;
use crate::services::SyncService;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::instrument;

/// Handles Stop hook events.
///
/// Performs session analysis and sync at session end.
pub struct StopHandler {
    /// Sync service.
    sync: Option<SyncService>,
    /// Whether to auto-sync on stop.
    auto_sync: bool,
}

impl StopHandler {
    /// Creates a new handler.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sync: None,
            auto_sync: true,
        }
    }

    /// Sets the sync service.
    #[must_use]
    pub fn with_sync(mut self, sync: SyncService) -> Self {
        self.sync = Some(sync);
        self
    }

    /// Enables or disables auto-sync.
    #[must_use]
    pub const fn with_auto_sync(mut self, enabled: bool) -> Self {
        self.auto_sync = enabled;
        self
    }

    /// Generates a session summary.
    #[allow(clippy::cast_possible_truncation)]
    fn generate_summary(&self, input: &serde_json::Value) -> SessionSummary {
        // Extract session info
        let session_id = input
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let start_time = input
            .get("start_time")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        let end_time = current_timestamp();
        let duration_seconds = end_time.saturating_sub(start_time);

        // Count interactions (from transcript if available)
        // Safe cast: interaction counts are always small
        let interaction_count = input
            .get("interaction_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;

        // Count memories captured during session
        // Safe cast: memory counts are always small
        let memories_captured = input
            .get("memories_captured")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;

        // Count tools used
        let tools_used = input
            .get("tools_used")
            .and_then(|v| v.as_array())
            .map_or(0, std::vec::Vec::len);

        SessionSummary {
            session_id,
            duration_seconds,
            interaction_count,
            memories_captured,
            tools_used,
        }
    }

    /// Performs sync if enabled and available.
    fn perform_sync(&self) -> Option<SyncResult> {
        if !self.auto_sync {
            return None;
        }

        let sync = self.sync.as_ref()?;

        match sync.sync() {
            Ok(stats) => Some(SyncResult {
                success: true,
                pushed: stats.pushed,
                pulled: stats.pulled,
                error: None,
            }),
            Err(e) => Some(SyncResult {
                success: false,
                pushed: 0,
                pulled: 0,
                error: Some(e.to_string()),
            }),
        }
    }
}

/// Gets the current Unix timestamp.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Default for StopHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HookHandler for StopHandler {
    fn event_type(&self) -> &'static str {
        "Stop"
    }

    #[instrument(skip(self, input), fields(hook = "Stop"))]
    fn handle(&self, input: &str) -> Result<String> {
        // Parse input as JSON
        let input_json: serde_json::Value =
            serde_json::from_str(input).unwrap_or_else(|_| serde_json::json!({}));

        // Generate session summary
        let summary = self.generate_summary(&input_json);

        // Perform sync if enabled
        let sync_result = self.perform_sync();

        // Build metadata
        let mut metadata = serde_json::json!({
            "session_id": summary.session_id,
            "duration_seconds": summary.duration_seconds,
            "interaction_count": summary.interaction_count,
            "memories_captured": summary.memories_captured,
            "tools_used": summary.tools_used,
        });

        // Add sync results if performed
        if let Some(sync) = &sync_result {
            metadata["sync"] = serde_json::json!({
                "performed": true,
                "success": sync.success,
                "pushed": sync.pushed,
                "pulled": sync.pulled,
                "error": sync.error
            });
        } else {
            metadata["sync"] = serde_json::json!({
                "performed": false
            });
        }

        // Build context message for session summary
        let mut context_lines = vec![
            "**Subcog Session Summary**\n".to_string(),
            format!("Session: `{}`", summary.session_id),
            format!("Duration: {} seconds", summary.duration_seconds),
            format!("Interactions: {}", summary.interaction_count),
            format!("Memories captured: {}", summary.memories_captured),
            format!("Tools used: {}", summary.tools_used),
        ];

        // Add sync status
        if let Some(sync) = &sync_result {
            if sync.success {
                context_lines.push(format!(
                    "\n**Sync**: ✓ {} pushed, {} pulled",
                    sync.pushed, sync.pulled
                ));
            } else {
                context_lines.push(format!(
                    "\n**Sync**: ✗ Failed - {}",
                    sync.error.as_deref().unwrap_or("Unknown error")
                ));
            }
        }

        // Add hints if no memories were captured
        if summary.memories_captured == 0 && summary.interaction_count > 5 {
            metadata["hints"] = serde_json::json!([
                "Consider capturing key decisions made during this session",
                "Use 'mcp__plugin_subcog_subcog__subcog_capture' to save important learnings"
            ]);
            context_lines.push("\n**Tip**: No memories were captured this session. Consider using `mcp__plugin_subcog_subcog__subcog_capture` to save important decisions and learnings.".to_string());
        }

        // Build Claude Code hook response format per specification
        // See: https://docs.anthropic.com/en/docs/claude-code/hooks
        // Embed metadata as XML comment for debugging
        let metadata_str = serde_json::to_string(&metadata).unwrap_or_default();
        let context_with_metadata = format!(
            "{}\n\n<!-- subcog-metadata: {} -->",
            context_lines.join("\n"),
            metadata_str
        );
        let response = serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "Stop",
                "additionalContext": context_with_metadata
            }
        });

        serde_json::to_string(&response).map_err(|e| crate::Error::OperationFailed {
            operation: "serialize_response".to_string(),
            cause: e.to_string(),
        })
    }
}

/// Summary of a session.
#[derive(Debug, Clone)]
struct SessionSummary {
    /// Session identifier.
    session_id: String,
    /// Duration in seconds.
    duration_seconds: u64,
    /// Number of interactions.
    interaction_count: usize,
    /// Number of memories captured.
    memories_captured: usize,
    /// Number of tools used.
    tools_used: usize,
}

/// Result of a sync operation.
#[derive(Debug, Clone)]
struct SyncResult {
    /// Whether sync succeeded.
    success: bool,
    /// Number of memories pushed.
    pushed: usize,
    /// Number of memories pulled.
    pulled: usize,
    /// Error message if failed.
    error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_creation() {
        let handler = StopHandler::default();
        assert_eq!(handler.event_type(), "Stop");
    }

    #[test]
    fn test_generate_summary() {
        let handler = StopHandler::default();

        let now = current_timestamp();
        let input = serde_json::json!({
            "session_id": "test-session",
            "start_time": now - 3600, // 1 hour ago
            "interaction_count": 10,
            "memories_captured": 2,
            "tools_used": ["Read", "Write", "Bash"]
        });

        let summary = handler.generate_summary(&input);

        assert_eq!(summary.session_id, "test-session");
        assert_eq!(summary.interaction_count, 10);
        assert_eq!(summary.memories_captured, 2);
        assert_eq!(summary.tools_used, 3);
        assert!(summary.duration_seconds >= 3600);
    }

    #[test]
    fn test_handle_basic() {
        let handler = StopHandler::default();

        let input = r#"{"session_id": "test-session", "interaction_count": 5}"#;

        let result = handler.handle(input);
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Claude Code hook format - should have hookSpecificOutput
        let hook_output = response.get("hookSpecificOutput").unwrap();
        assert_eq!(
            hook_output.get("hookEventName"),
            Some(&serde_json::Value::String("Stop".to_string()))
        );
        // Should have additionalContext with session summary and metadata embedded
        let context = hook_output
            .get("additionalContext")
            .unwrap()
            .as_str()
            .unwrap();
        assert!(context.contains("Subcog Session Summary"));
        assert!(context.contains("test-session"));
        assert!(context.contains("subcog-metadata"));
        assert!(context.contains("\"session_id\""));
        assert!(context.contains("\"sync\""));
    }

    #[test]
    fn test_handle_with_hints() {
        let handler = StopHandler::default();

        let input =
            r#"{"session_id": "test-session", "interaction_count": 10, "memories_captured": 0}"#;

        let result = handler.handle(input);
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Claude Code hook format - should have hookSpecificOutput
        let hook_output = response.get("hookSpecificOutput").unwrap();
        let context = hook_output
            .get("additionalContext")
            .unwrap()
            .as_str()
            .unwrap();
        // Context should contain tip
        assert!(context.contains("Tip"));
        // Hints should be in embedded metadata
        assert!(context.contains("\"hints\""));
    }

    #[test]
    fn test_auto_sync_disabled() {
        let handler = StopHandler::default().with_auto_sync(false);

        let sync_result = handler.perform_sync();
        assert!(sync_result.is_none());
    }

    #[test]
    fn test_configuration() {
        let handler = StopHandler::default().with_auto_sync(true);

        assert!(handler.auto_sync);
    }

    #[test]
    fn test_empty_input() {
        let handler = StopHandler::default();

        let input = "{}";

        let result = handler.handle(input);
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Claude Code hook format - should have hookSpecificOutput
        let hook_output = response.get("hookSpecificOutput").unwrap();
        let context = hook_output
            .get("additionalContext")
            .unwrap()
            .as_str()
            .unwrap();
        // Session ID should be in embedded metadata with default "unknown"
        assert!(context.contains("\"session_id\":\"unknown\""));
    }
}
