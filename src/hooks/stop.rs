//! Stop hook handler.

use super::HookHandler;
use crate::Result;
use crate::current_timestamp;
use crate::observability::current_request_id;
use crate::services::SyncService;
use std::time::{Duration, Instant};
use tracing::instrument;

/// Default timeout for stop hook operations (30 seconds).
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// Handles Stop hook events.
///
/// Performs session analysis and sync at session end.
/// Includes timeout enforcement to prevent hanging (RES-M2).
pub struct StopHandler {
    /// Sync service.
    sync: Option<SyncService>,
    /// Whether to auto-sync on stop.
    auto_sync: bool,
    /// Timeout for stop hook operations in milliseconds.
    timeout_ms: u64,
}

impl StopHandler {
    /// Creates a new handler with default 30s timeout.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sync: None,
            auto_sync: true,
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    /// Sets the timeout in milliseconds.
    ///
    /// Operations that exceed this timeout will return a partial response.
    #[must_use]
    pub const fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
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

        // Extract namespace counts
        let namespace_counts = Self::extract_namespace_counts(input);

        // Extract tags used with frequencies
        let tags_used = Self::extract_tags_used(input);

        // Extract query patterns
        let query_patterns = Self::extract_query_patterns(input);

        // Extract resources read
        let resources_read = Self::extract_resources_read(input);

        SessionSummary {
            session_id,
            duration_seconds,
            interaction_count,
            memories_captured,
            tools_used,
            namespace_counts,
            tags_used,
            query_patterns,
            resources_read,
        }
    }

    /// Extracts namespace statistics from input.
    #[allow(clippy::cast_possible_truncation)]
    fn extract_namespace_counts(
        input: &serde_json::Value,
    ) -> std::collections::HashMap<String, NamespaceStats> {
        let Some(ns_stats) = input.get("namespace_stats").and_then(|v| v.as_object()) else {
            return std::collections::HashMap::new();
        };

        ns_stats
            .iter()
            .filter_map(|(ns, stats)| {
                let captures = stats
                    .get("captures")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as usize;
                let recalls = stats
                    .get("recalls")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as usize;

                if captures > 0 || recalls > 0 {
                    Some((ns.clone(), NamespaceStats { captures, recalls }))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Extracts tags used with frequencies, sorted by count descending.
    #[allow(clippy::cast_possible_truncation)]
    fn extract_tags_used(input: &serde_json::Value) -> Vec<(String, usize)> {
        let Some(tag_array) = input.get("tags_used").and_then(|v| v.as_array()) else {
            return Vec::new();
        };

        let mut tags: Vec<(String, usize)> = tag_array
            .iter()
            .filter_map(|entry| {
                let obj = entry.as_object()?;
                let tag = obj.get("tag").and_then(|v| v.as_str())?;
                let count = obj.get("count").and_then(serde_json::Value::as_u64)? as usize;
                Some((tag.to_string(), count))
            })
            .collect();

        // Sort by count descending, limit to top 10
        tags.sort_by(|a, b| b.1.cmp(&a.1));
        tags.truncate(10);
        tags
    }

    /// Extracts query patterns from the session.
    fn extract_query_patterns(input: &serde_json::Value) -> Vec<String> {
        input
            .get("query_patterns")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Extracts MCP resources read during the session.
    fn extract_resources_read(input: &serde_json::Value) -> Vec<String> {
        input
            .get("resources_read")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
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

    /// Builds metadata JSON from session summary.
    fn build_metadata(
        summary: &SessionSummary,
        sync_result: Option<&SyncResult>,
    ) -> serde_json::Value {
        let mut metadata = serde_json::json!({
            "session_id": summary.session_id,
            "duration_seconds": summary.duration_seconds,
            "interaction_count": summary.interaction_count,
            "memories_captured": summary.memories_captured,
            "tools_used": summary.tools_used,
        });

        // Add namespace stats
        if !summary.namespace_counts.is_empty() {
            let ns_json: serde_json::Map<String, serde_json::Value> = summary
                .namespace_counts
                .iter()
                .map(|(ns, stats)| {
                    (
                        ns.clone(),
                        serde_json::json!({
                            "captures": stats.captures,
                            "recalls": stats.recalls
                        }),
                    )
                })
                .collect();
            metadata["namespace_stats"] = serde_json::Value::Object(ns_json);
        }

        // Add tags
        if !summary.tags_used.is_empty() {
            metadata["tags_used"] = serde_json::json!(summary.tags_used);
        }

        // Add query patterns
        if !summary.query_patterns.is_empty() {
            metadata["query_patterns"] = serde_json::json!(summary.query_patterns);
        }

        // Add resources read
        if !summary.resources_read.is_empty() {
            metadata["resources_read"] = serde_json::json!(summary.resources_read);
        }

        // Add sync results
        if let Some(sync) = sync_result {
            metadata["sync"] = serde_json::json!({
                "performed": true,
                "success": sync.success,
                "pushed": sync.pushed,
                "pulled": sync.pulled,
                "error": sync.error
            });
        } else {
            metadata["sync"] = serde_json::json!({ "performed": false });
        }

        // Add hints if applicable
        if summary.memories_captured == 0 && summary.interaction_count > 5 {
            metadata["hints"] = serde_json::json!([
                "Consider capturing key decisions made during this session",
                "Use 'mcp__plugin_subcog_subcog__subcog_capture' to save important learnings"
            ]);
        }

        metadata
    }

    /// Builds context message lines from session summary.
    fn build_context_lines(summary: &SessionSummary, sync_result: Option<&SyncResult>) -> String {
        let mut lines = vec![
            "**Subcog Session Summary**\n".to_string(),
            format!("Session: `{}`", summary.session_id),
            format!("Duration: {} seconds", summary.duration_seconds),
            format!("Interactions: {}", summary.interaction_count),
            format!("Memories captured: {}", summary.memories_captured),
            format!("Tools used: {}", summary.tools_used),
        ];

        // Namespace breakdown
        if !summary.namespace_counts.is_empty() {
            lines.push("\n**Namespace Breakdown**:".to_string());
            lines.push("| Namespace | Captures | Recalls |".to_string());
            lines.push("|-----------|----------|---------|".to_string());
            let mut sorted_ns: Vec<_> = summary.namespace_counts.iter().collect();
            sorted_ns.sort_by_key(|(ns, _)| *ns);
            for (ns, stats) in sorted_ns {
                lines.push(format!(
                    "| {} | {} | {} |",
                    ns, stats.captures, stats.recalls
                ));
            }
        }

        // Top tags
        if !summary.tags_used.is_empty() {
            let tags_str: Vec<String> = summary
                .tags_used
                .iter()
                .take(5)
                .map(|(tag, count)| format!("`{tag}` ({count})"))
                .collect();
            lines.push(format!("\n**Top Tags**: {}", tags_str.join(", ")));
        }

        // Query patterns
        if !summary.query_patterns.is_empty() {
            let patterns_str: Vec<String> = summary
                .query_patterns
                .iter()
                .take(5)
                .map(|p| format!("`{p}`"))
                .collect();
            lines.push(format!("\n**Query Patterns**: {}", patterns_str.join(", ")));
        }

        // Resources read
        if !summary.resources_read.is_empty() {
            lines.push(format!(
                "\n**Resources Read**: {} unique resources",
                summary.resources_read.len()
            ));
        }

        // Sync status
        if let Some(sync) = sync_result {
            if sync.success {
                lines.push(format!(
                    "\n**Sync**: ✓ {} pushed, {} pulled",
                    sync.pushed, sync.pulled
                ));
            } else {
                lines.push(format!(
                    "\n**Sync**: ✗ Failed - {}",
                    sync.error.as_deref().unwrap_or("Unknown error")
                ));
            }
        }

        // Capture hint
        if summary.memories_captured == 0 && summary.interaction_count > 5 {
            lines.push("\n**Tip**: No memories were captured this session. Consider using `mcp__plugin_subcog_subcog__subcog_capture` to save important decisions and learnings.".to_string());
        }

        lines.join("\n")
    }
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

    #[instrument(
        name = "subcog.hook.stop",
        skip(self, input),
        fields(
            request_id = tracing::field::Empty,
            component = "hooks",
            operation = "stop",
            hook = "Stop",
            session_id = tracing::field::Empty,
            sync_performed = tracing::field::Empty,
            timed_out = tracing::field::Empty
        )
    )]
    fn handle(&self, input: &str) -> Result<String> {
        let start = Instant::now();
        let deadline = Duration::from_millis(self.timeout_ms);
        let mut timed_out = false;
        if let Some(request_id) = current_request_id() {
            tracing::Span::current().record("request_id", request_id.as_str());
        }

        tracing::info!(
            hook = "Stop",
            timeout_ms = self.timeout_ms,
            "Processing stop hook"
        );

        // Parse input and generate summary
        let input_json: serde_json::Value =
            serde_json::from_str(input).unwrap_or_else(|_| serde_json::json!({}));
        let summary = self.generate_summary(&input_json);

        // Record session ID in span
        let span = tracing::Span::current();
        span.record("session_id", summary.session_id.as_str());

        // Check deadline before sync (RES-M2)
        // Reserve 1 second for response building
        let sync_result = if start.elapsed() < deadline.saturating_sub(Duration::from_secs(1)) {
            self.perform_sync()
        } else {
            tracing::warn!(
                hook = "Stop",
                elapsed_ms = start.elapsed().as_millis(),
                deadline_ms = self.timeout_ms,
                "Skipping sync due to timeout deadline"
            );
            timed_out = true;
            None
        };
        span.record("sync_performed", sync_result.is_some());

        // Check deadline before response building
        if start.elapsed() >= deadline {
            tracing::warn!(
                hook = "Stop",
                elapsed_ms = start.elapsed().as_millis(),
                deadline_ms = self.timeout_ms,
                "Stop hook exceeded timeout, returning minimal response"
            );
            metrics::counter!(
                "hook_timeouts_total",
                "hook_type" => "Stop"
            )
            .increment(1);

            // Return empty response on timeout
            // Note: Stop hooks don't support hookSpecificOutput/additionalContext
            // per Claude Code hook specification. Context is logged but not returned.
            tracing::debug!(
                session_id = %summary.session_id,
                timed_out = true,
                elapsed_ms = start.elapsed().as_millis(),
                "Stop hook timed out, returning empty response"
            );
            span.record("timed_out", true);
            return Ok("{}".to_string());
        }

        // Build response components for logging/debugging
        let mut metadata = Self::build_metadata(&summary, sync_result.as_ref());
        let context = Self::build_context_lines(&summary, sync_result.as_ref());

        // Add timeout info to metadata if we were close to deadline
        if timed_out {
            metadata["sync_skipped_timeout"] = serde_json::json!(true);
        }
        #[allow(clippy::cast_possible_truncation)]
        let elapsed_ms = start.elapsed().as_millis() as u64; // u128 to u64 safe for <584M years
        metadata["elapsed_ms"] = serde_json::json!(elapsed_ms);

        // Log the session summary for debugging (Stop hooks don't support
        // hookSpecificOutput/additionalContext per Claude Code hook specification)
        tracing::info!(
            session_id = %summary.session_id,
            duration_seconds = summary.duration_seconds,
            interaction_count = summary.interaction_count,
            memories_captured = summary.memories_captured,
            sync_performed = sync_result.is_some(),
            "Session ended"
        );
        tracing::debug!(context = %context, metadata = ?metadata, "Stop hook context (not returned)");

        span.record("timed_out", timed_out);

        // Return empty response - Stop hooks don't support context injection
        let result = Ok("{}".to_string());

        // Record metrics
        let status = if result.is_ok() { "success" } else { "error" };
        metrics::counter!("hook_executions_total", "hook_type" => "Stop", "status" => status)
            .increment(1);
        metrics::histogram!("hook_duration_ms", "hook_type" => "Stop")
            .record(start.elapsed().as_secs_f64() * 1000.0);

        result
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
    /// Per-namespace statistics (captures and recalls).
    namespace_counts: std::collections::HashMap<String, NamespaceStats>,
    /// Tags used with frequency (sorted by count, descending).
    tags_used: Vec<(String, usize)>,
    /// Query patterns seen during the session.
    query_patterns: Vec<String>,
    /// MCP resources read during the session.
    resources_read: Vec<String>,
}

/// Statistics for a specific namespace.
#[derive(Debug, Clone, Default)]
struct NamespaceStats {
    /// Number of captures in this namespace.
    captures: usize,
    /// Number of recalls in this namespace.
    recalls: usize,
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
        // Stop hooks don't support hookSpecificOutput per Claude Code spec
        // Response should be empty JSON (context is logged only)
        assert!(response.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_handle_with_hints() {
        let handler = StopHandler::default();

        let input =
            r#"{"session_id": "test-session", "interaction_count": 10, "memories_captured": 0}"#;

        let result = handler.handle(input);
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Stop hooks return empty JSON - context is logged only
        assert!(response.as_object().unwrap().is_empty());
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
        // Stop hooks return empty JSON - context is logged only
        assert!(response.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_namespace_breakdown() {
        let handler = StopHandler::default();

        let input = serde_json::json!({
            "session_id": "test-session",
            "namespace_stats": {
                "decisions": {"captures": 3, "recalls": 5},
                "learnings": {"captures": 2, "recalls": 1}
            }
        });

        let result = handler.handle(&input.to_string());
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Stop hooks return empty JSON - context is logged only
        assert!(response.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_tags_analysis() {
        let handler = StopHandler::default();

        let input = serde_json::json!({
            "session_id": "test-session",
            "tags_used": [
                {"tag": "rust", "count": 5},
                {"tag": "architecture", "count": 3},
                {"tag": "testing", "count": 2}
            ]
        });

        let result = handler.handle(&input.to_string());
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Stop hooks return empty JSON - context is logged only
        assert!(response.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_query_patterns() {
        let handler = StopHandler::default();

        let input = serde_json::json!({
            "session_id": "test-session",
            "query_patterns": ["how to implement", "where is the config"]
        });

        let result = handler.handle(&input.to_string());
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Stop hooks return empty JSON - context is logged only
        assert!(response.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_resources_tracking() {
        let handler = StopHandler::default();

        let input = serde_json::json!({
            "session_id": "test-session",
            "resources_read": [
                "subcog://decisions/mem-1",
                "subcog://learnings/mem-2"
            ]
        });

        let result = handler.handle(&input.to_string());
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Stop hooks return empty JSON - context is logged only
        assert!(response.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_extract_namespace_counts() {
        let input = serde_json::json!({
            "namespace_stats": {
                "decisions": {"captures": 3, "recalls": 5},
                "patterns": {"captures": 1, "recalls": 0}
            }
        });

        let counts = StopHandler::extract_namespace_counts(&input);

        assert_eq!(counts.len(), 2);
        assert_eq!(counts.get("decisions").map(|s| s.captures), Some(3));
        assert_eq!(counts.get("decisions").map(|s| s.recalls), Some(5));
        assert_eq!(counts.get("patterns").map(|s| s.captures), Some(1));
    }

    #[test]
    fn test_extract_tags_used() {
        let input = serde_json::json!({
            "tags_used": [
                {"tag": "rust", "count": 10},
                {"tag": "testing", "count": 5},
                {"tag": "docs", "count": 3}
            ]
        });

        let tags = StopHandler::extract_tags_used(&input);

        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0], ("rust".to_string(), 10)); // Highest count first
        assert_eq!(tags[1], ("testing".to_string(), 5));
    }

    #[test]
    fn test_default_timeout() {
        let handler = StopHandler::new();
        assert_eq!(handler.timeout_ms, DEFAULT_TIMEOUT_MS);
        assert_eq!(handler.timeout_ms, 30_000);
    }

    #[test]
    fn test_with_timeout_ms() {
        let handler = StopHandler::new().with_timeout_ms(5_000);
        assert_eq!(handler.timeout_ms, 5_000);
    }

    #[test]
    fn test_returns_empty_json() {
        let handler = StopHandler::new();
        let input = r#"{"session_id": "test-session"}"#;

        let result = handler.handle(input);
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Stop hooks return empty JSON - context and metadata logged only
        assert!(response.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_builder_chaining() {
        let handler = StopHandler::new()
            .with_timeout_ms(10_000)
            .with_auto_sync(false);

        assert_eq!(handler.timeout_ms, 10_000);
        assert!(!handler.auto_sync);
    }
}
