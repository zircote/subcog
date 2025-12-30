//! Stop hook handler.

use super::HookHandler;
use crate::config::SubcogConfig;
use crate::services::SyncService;
use crate::Result;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::instrument;

/// Handles Stop hook events.
///
/// Performs session analysis and sync at session end.
pub struct StopHandler {
    /// Configuration.
    config: SubcogConfig,
    /// Sync service.
    sync: Option<SyncService>,
    /// Whether to auto-sync on stop.
    auto_sync: bool,
}

impl StopHandler {
    /// Creates a new handler.
    #[must_use]
    pub fn new(config: SubcogConfig) -> Self {
        Self {
            config,
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
    fn generate_summary(&self, input: &serde_json::Value) -> SessionSummary {
        // Extract session info
        let session_id = input
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let start_time = input
            .get("start_time")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let end_time = current_timestamp();
        let duration_seconds = end_time.saturating_sub(start_time);

        // Count interactions (from transcript if available)
        let interaction_count = input
            .get("interaction_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        // Count memories captured during session
        let memories_captured = input
            .get("memories_captured")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        // Count tools used
        let tools_used = input
            .get("tools_used")
            .and_then(|v| v.as_array())
            .map(|arr| arr.len())
            .unwrap_or(0);

        SessionSummary {
            session_id,
            start_time,
            end_time,
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
        Self::new(SubcogConfig::default())
    }
}

impl HookHandler for StopHandler {
    fn event_type(&self) -> &'static str {
        "Stop"
    }

    #[instrument(skip(self, input), fields(hook = "Stop"))]
    fn handle(&self, input: &str) -> Result<String> {
        // Parse input as JSON
        let input_json: serde_json::Value = serde_json::from_str(input).unwrap_or_else(|_| {
            serde_json::json!({})
        });

        // Generate session summary
        let summary = self.generate_summary(&input_json);

        // Perform sync if enabled
        let sync_result = self.perform_sync();

        // Build response
        let mut response = serde_json::json!({
            "session_id": summary.session_id,
            "duration_seconds": summary.duration_seconds,
            "interaction_count": summary.interaction_count,
            "memories_captured": summary.memories_captured,
            "tools_used": summary.tools_used,
        });

        // Add sync results if performed
        if let Some(sync) = sync_result {
            response["sync"] = serde_json::json!({
                "performed": true,
                "success": sync.success,
                "pushed": sync.pushed,
                "pulled": sync.pulled,
                "error": sync.error
            });
        } else {
            response["sync"] = serde_json::json!({
                "performed": false
            });
        }

        // Add session analysis hints
        if summary.memories_captured == 0 && summary.interaction_count > 5 {
            response["hints"] = serde_json::json!([
                "Consider capturing key decisions made during this session",
                "Use 'subcog capture' to save important learnings"
            ]);
        }

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
    /// Start timestamp.
    start_time: u64,
    /// End timestamp.
    end_time: u64,
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
        assert!(response.get("session_id").is_some());
        assert!(response.get("sync").is_some());
    }

    #[test]
    fn test_handle_with_hints() {
        let handler = StopHandler::default();

        let input = r#"{"session_id": "test-session", "interaction_count": 10, "memories_captured": 0}"#;

        let result = handler.handle(input);
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Should have hints when no memories captured but many interactions
        assert!(response.get("hints").is_some());
    }

    #[test]
    fn test_auto_sync_disabled() {
        let handler = StopHandler::default()
            .with_auto_sync(false);

        let sync_result = handler.perform_sync();
        assert!(sync_result.is_none());
    }

    #[test]
    fn test_configuration() {
        let handler = StopHandler::default()
            .with_auto_sync(true);

        assert!(handler.auto_sync);
    }

    #[test]
    fn test_empty_input() {
        let handler = StopHandler::default();

        let input = "{}";

        let result = handler.handle(input);
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Should handle gracefully with defaults
        assert_eq!(response.get("session_id"), Some(&serde_json::Value::String("unknown".to_string())));
    }
}
