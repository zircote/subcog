//! Session start hook handler.
//!
//! # Security
//!
//! This module validates session IDs for sufficient entropy to prevent:
//! - Predictable session attacks
//! - Session enumeration attacks
//! - Weak identifier exploitation

use super::HookHandler;
use crate::Result;
use crate::observability::current_request_id;
use crate::services::{ContextBuilderService, MemoryStatistics};
use std::time::{Duration, Instant};
use tracing::instrument;

/// Minimum length for session IDs (security requirement).
const MIN_SESSION_ID_LENGTH: usize = 16;

/// Maximum length for session IDs (denial of service prevention).
const MAX_SESSION_ID_LENGTH: usize = 256;

/// Minimum number of unique characters required for entropy.
const MIN_UNIQUE_CHARS: usize = 4;

/// Minimum consecutive sequential characters to flag as low entropy.
const MIN_SEQUENTIAL_RUN: usize = 8;

/// Default timeout for context loading (PERF-M3: prevents session start blocking).
const DEFAULT_CONTEXT_TIMEOUT_MS: u64 = 500;

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
    /// Timeout for context loading in milliseconds (PERF-M3).
    context_timeout_ms: u64,
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

/// Result of session ID validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionIdValidation {
    /// Session ID is valid with sufficient entropy.
    Valid,
    /// Session ID is too short (< 16 characters).
    TooShort,
    /// Session ID is too long (> 256 characters).
    TooLong,
    /// Session ID has low entropy (predictable patterns).
    LowEntropy,
    /// Session ID is missing or empty.
    Missing,
}

impl SessionIdValidation {
    /// Returns a human-readable description of the validation result.
    pub const fn description(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::TooShort => "too short (minimum 16 characters)",
            Self::TooLong => "too long (maximum 256 characters)",
            Self::LowEntropy => "low entropy (predictable pattern detected)",
            Self::Missing => "missing or empty",
        }
    }
}

/// Validates a session ID for sufficient entropy.
///
/// # Security
///
/// This function checks session IDs for:
/// - Minimum length (16 characters) to prevent enumeration
/// - Maximum length (256 characters) to prevent `DoS`
/// - Sufficient unique characters to prevent predictable patterns
/// - Detection of repeating/sequential patterns
///
/// # Returns
///
/// A `SessionIdValidation` enum indicating the validation result.
pub fn validate_session_id(session_id: &str) -> SessionIdValidation {
    // Check for missing/empty
    if session_id.is_empty() || session_id == "unknown" {
        return SessionIdValidation::Missing;
    }

    // Check minimum length
    if session_id.len() < MIN_SESSION_ID_LENGTH {
        return SessionIdValidation::TooShort;
    }

    // Check maximum length (DoS prevention)
    if session_id.len() > MAX_SESSION_ID_LENGTH {
        return SessionIdValidation::TooLong;
    }

    // Check for low entropy
    if has_low_entropy(session_id) {
        return SessionIdValidation::LowEntropy;
    }

    SessionIdValidation::Valid
}

/// Checks if a session ID has low entropy (predictable patterns).
fn has_low_entropy(session_id: &str) -> bool {
    // Count unique characters
    let unique_chars: std::collections::HashSet<char> = session_id.chars().collect();
    if unique_chars.len() < MIN_UNIQUE_CHARS {
        return true;
    }

    // Check for repeating patterns (e.g., "abcabcabc" or "111111111")
    let chars: Vec<char> = session_id.chars().collect();

    // Check for all same character
    if chars.iter().all(|&c| c == chars[0]) {
        return true;
    }

    // Check for simple repeating pattern (pattern length 1-4)
    for pattern_len in 1..=4 {
        if chars.len() >= pattern_len * 3 {
            let pattern = &chars[..pattern_len];
            let is_repeating = chars
                .chunks(pattern_len)
                .all(|chunk| chunk == pattern || chunk.len() < pattern_len);
            if is_repeating {
                return true;
            }
        }
    }

    // Check for long sequential patterns (e.g., "12345678" or "abcdefgh")
    // Only flag if there's a consecutive run of MIN_SEQUENTIAL_RUN or more
    if has_long_sequential_run(session_id) {
        return true;
    }

    false
}

/// Checks if a string contains a long consecutive sequential run.
///
/// This detects patterns like "12345678" or "abcdefgh" by looking for
/// consecutive runs of characters where each differs from the previous by +1 or -1.
/// Random-looking IDs (like UUIDs) may have scattered sequential pairs but not long runs.
fn has_long_sequential_run(s: &str) -> bool {
    if s.len() < MIN_SEQUENTIAL_RUN {
        return false;
    }

    // Only check alphanumeric characters for sequences
    let chars: Vec<i32> = s
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c as i32)
        .collect();

    if chars.len() < MIN_SEQUENTIAL_RUN {
        return false;
    }

    // Check for consecutive ascending runs
    let mut ascending_run = 1;
    for window in chars.windows(2) {
        if window[1] == window[0] + 1 {
            ascending_run += 1;
            if ascending_run >= MIN_SEQUENTIAL_RUN {
                return true;
            }
        } else {
            ascending_run = 1;
        }
    }

    // Check for consecutive descending runs
    let mut descending_run = 1;
    for window in chars.windows(2) {
        if window[0] == window[1] + 1 {
            descending_run += 1;
            if descending_run >= MIN_SEQUENTIAL_RUN {
                return true;
            }
        } else {
            descending_run = 1;
        }
    }

    false
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
            context_timeout_ms: DEFAULT_CONTEXT_TIMEOUT_MS,
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

    /// Sets the context loading timeout in milliseconds (PERF-M3).
    ///
    /// If context loading takes longer than this timeout, the handler
    /// will return minimal context instead of blocking session start.
    #[must_use]
    pub const fn with_context_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.context_timeout_ms = timeout_ms;
        self
    }

    /// Helper to build context from the builder service (PERF-M3).
    ///
    /// Returns a tuple of (context string, statistics, memory count).
    fn build_context_from_builder(
        &self,
        max_tokens: usize,
        start: Instant,
        deadline: Duration,
    ) -> Result<(Option<String>, Option<MemoryStatistics>, usize)> {
        let Some(ref builder) = self.context_builder else {
            return Ok((None, None, 0));
        };

        let context = builder.build_context(max_tokens)?;
        let ctx = if context.is_empty() {
            None
        } else {
            Some(context)
        };

        // PERF-M3: Check timeout before statistics gathering
        if start.elapsed() >= deadline {
            tracing::debug!(
                elapsed_ms = start.elapsed().as_millis(),
                deadline_ms = self.context_timeout_ms,
                "Skipping statistics due to timeout (PERF-M3)"
            );
            let count = usize::from(ctx.is_some());
            return Ok((ctx, None, count));
        }

        let has_context = ctx.is_some();
        let (stats, count) = match builder.get_statistics() {
            Ok(s) => {
                let c = s.total_count;
                (Some(s), c)
            },
            Err(_) => (None, usize::from(has_context)),
        };

        Ok((ctx, stats, count))
    }

    /// Helper to add guidance based on level (PERF-M3).
    fn add_guidance(&self, context_parts: &mut Vec<String>) {
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
    }

    /// Builds context for the session with inline timeout checking (PERF-M3).
    ///
    /// Monitors elapsed time and returns early with minimal context if approaching
    /// the timeout. This provides timeout safety without requiring thread spawning.
    fn build_session_context(&self, session_id: &str, cwd: &str) -> Result<SessionContext> {
        let start = Instant::now();
        let deadline = Duration::from_millis(self.context_timeout_ms);
        let mut context_parts = Vec::new();
        let mut memory_count = 0;
        let mut statistics: Option<MemoryStatistics> = None;
        let mut timed_out = false;

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

        // PERF-M3: Check timeout before expensive context building
        let within_deadline = start.elapsed() < deadline;
        if !within_deadline {
            timed_out = true;
            tracing::warn!(
                elapsed_ms = start.elapsed().as_millis(),
                deadline_ms = self.context_timeout_ms,
                "Context loading timed out, using minimal context (PERF-M3)"
            );
            metrics::counter!("session_context_timeout_total", "reason" => "deadline_exceeded")
                .increment(1);
        }

        // Build context from builder if available and within deadline
        if within_deadline {
            let (ctx, stats, count) =
                self.build_context_from_builder(max_tokens, start, deadline)?;
            if let Some(c) = ctx {
                context_parts.push(c);
            }
            if let Some(s) = stats.as_ref() {
                add_statistics_if_present(&mut context_parts, s);
            }
            statistics = stats;
            memory_count = count;
            timed_out = start.elapsed() >= deadline;
        }

        // PERF-M3: Only add guidance if not timed out and within deadline
        if !timed_out && start.elapsed() < deadline {
            self.add_guidance(&mut context_parts);
        }

        let content = context_parts.join("\n\n");
        let token_estimate = ContextBuilderService::estimate_tokens(&content);

        // Record timing metrics
        if timed_out {
            metrics::histogram!(
                "session_context_build_duration_ms",
                "status" => "timeout"
            )
            .record(start.elapsed().as_secs_f64() * 1000.0);
        } else {
            metrics::histogram!(
                "session_context_build_duration_ms",
                "status" => "success"
            )
            .record(start.elapsed().as_secs_f64() * 1000.0);
        }

        Ok(SessionContext {
            content,
            memory_count,
            token_estimate,
            was_truncated: token_estimate > max_tokens || timed_out,
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
        r"## Subcog Memory Protocol (Quick Start)

Use the `prompt_understanding` tool for full, authoritative guidance.

**Required steps:**
1) Call `mcp__plugin_subcog_subcog__prompt_understanding` at session start.
2) Before any substantive response, call `mcp__plugin_subcog_subcog__subcog_recall`.
3) Capture decisions/patterns/learnings immediately with `mcp__plugin_subcog_subcog__subcog_capture`.

Call `mcp__plugin_subcog_subcog__prompt_understanding` whenever you need the full protocol, namespaces, and workflow examples."
            .to_string()
    }

    /// Returns detailed guidance text.
    fn detailed_guidance() -> String {
        r"# Subcog Memory Protocol (Detailed)

This hook is intentionally concise. Use the `prompt_understanding` tool for the full protocol,
namespaces, workflows, and examples.

**Required steps:**
1) Call `mcp__plugin_subcog_subcog__prompt_understanding` at session start.
2) Before any substantive response, call `mcp__plugin_subcog_subcog__subcog_recall`.
3) Capture decisions/patterns/learnings immediately with `mcp__plugin_subcog_subcog__subcog_capture`.

**Reminder:** The authoritative guidance lives in `prompt_understanding` and should be used
whenever you need detailed instructions."
            .to_string()
    }

    /// Checks if this is the first session (no user memories).
    fn is_first_session(&self) -> bool {
        // Check if we have any user memories
        self.context_builder
            .as_ref()
            .and_then(|builder| builder.build_context(100).ok())
            .is_none_or(|context| context.is_empty())
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
        name = "subcog.hook.session_start",
        skip(self, input),
        fields(
            request_id = tracing::field::Empty,
            component = "hooks",
            operation = "session_start",
            hook = "SessionStart",
            session_id = tracing::field::Empty,
            cwd = tracing::field::Empty
        )
    )]
    fn handle(&self, input: &str) -> Result<String> {
        let start = Instant::now();
        let mut token_estimate: Option<usize> = None;
        if let Some(request_id) = current_request_id() {
            tracing::Span::current().record("request_id", request_id.as_str());
        }

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

            // MED-SEC-003: Validate session ID entropy
            let validation = validate_session_id(session_id);
            if validation != SessionIdValidation::Valid {
                tracing::warn!(
                    session_id = session_id,
                    validation = validation.description(),
                    "Session ID validation warning"
                );
                metrics::counter!(
                    "session_id_validation_warnings_total",
                    "reason" => validation.description()
                )
                .increment(1);
            }

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

        let input = r#"{"session_id": "test-session-abc123def456", "cwd": "/path/to/project"}"#;

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
        assert!(context.contains("test-session-abc123def456"));
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
        assert!(guidance.contains("prompt_understanding"));
        assert!(guidance.contains("subcog_recall"));
        assert!(guidance.contains("subcog_capture"));
    }

    #[test]
    fn test_detailed_guidance() {
        let guidance = SessionStartHandler::detailed_guidance();
        assert!(guidance.contains("prompt_understanding"));
        assert!(guidance.contains("subcog_recall"));
        assert!(guidance.contains("subcog_capture"));
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

    // ==========================================================================
    // MED-SEC-003: Session ID Entropy Validation Tests
    // ==========================================================================

    #[test]
    fn test_session_id_validation_valid() {
        // Valid session IDs with good entropy
        assert_eq!(
            validate_session_id("abc123def456ghi789"),
            SessionIdValidation::Valid
        );
        // UUID format - should be valid (scattered pairs, no long runs)
        assert_eq!(
            validate_session_id("f0504ebb-ca72-4d1a-8b7c-53fc85a1a8ba"),
            SessionIdValidation::Valid
        );
        assert_eq!(
            validate_session_id("session_2024_01_03_xyz"),
            SessionIdValidation::Valid
        );
    }

    #[test]
    fn test_session_id_validation_missing() {
        assert_eq!(validate_session_id(""), SessionIdValidation::Missing);
        assert_eq!(validate_session_id("unknown"), SessionIdValidation::Missing);
    }

    #[test]
    fn test_session_id_validation_too_short() {
        assert_eq!(validate_session_id("short"), SessionIdValidation::TooShort);
        assert_eq!(
            validate_session_id("123456789012345"),
            SessionIdValidation::TooShort
        );
    }

    #[test]
    fn test_session_id_validation_too_long() {
        let long_id = "x".repeat(257);
        assert_eq!(validate_session_id(&long_id), SessionIdValidation::TooLong);
    }

    #[test]
    fn test_session_id_validation_low_entropy() {
        // All same character
        assert_eq!(
            validate_session_id("aaaaaaaaaaaaaaaaaaaaaaaaa"),
            SessionIdValidation::LowEntropy
        );

        // Simple repeating pattern
        assert_eq!(
            validate_session_id("abababababababababab"),
            SessionIdValidation::LowEntropy
        );

        // Long sequential pattern (8+ consecutive ascending)
        assert_eq!(
            validate_session_id("abcdefghijklmnop"),
            SessionIdValidation::LowEntropy
        );
    }

    #[test]
    fn test_session_id_validation_description() {
        assert_eq!(SessionIdValidation::Valid.description(), "valid");
        assert!(
            SessionIdValidation::TooShort
                .description()
                .contains("minimum")
        );
        assert!(
            SessionIdValidation::TooLong
                .description()
                .contains("maximum")
        );
        assert!(
            SessionIdValidation::LowEntropy
                .description()
                .contains("entropy")
        );
        assert!(
            SessionIdValidation::Missing
                .description()
                .contains("missing")
        );
    }

    #[test]
    fn test_has_low_entropy_few_unique_chars() {
        assert!(has_low_entropy("aaa")); // Only 1 unique char
        assert!(has_low_entropy("aabb")); // Only 2 unique chars
        assert!(has_low_entropy("aaabbbccc")); // Only 3 unique chars
    }

    #[test]
    fn test_has_long_sequential_run_ascending() {
        // 8+ consecutive ascending is flagged
        assert!(has_long_sequential_run("abcdefgh"));
        assert!(has_long_sequential_run("12345678"));
        assert!(has_long_sequential_run("abcdefghijklmnop"));
    }

    #[test]
    fn test_has_long_sequential_run_descending() {
        // 8+ consecutive descending is flagged
        assert!(has_long_sequential_run("hgfedcba"));
        assert!(has_long_sequential_run("87654321"));
    }

    #[test]
    fn test_has_long_sequential_run_non_sequential() {
        // Random-looking IDs should NOT be flagged
        assert!(!has_long_sequential_run("axbyczdwev"));
        assert!(!has_long_sequential_run("8372619450"));
        // UUIDs should NOT be flagged (scattered sequential pairs, no long runs)
        assert!(!has_long_sequential_run(
            "f0504ebb-ca72-4d1a-8b7c-53fc85a1a8ba"
        ));
        assert!(!has_long_sequential_run(
            "550e8400-e29b-41d4-a716-446655440000"
        ));
    }

    #[test]
    fn test_has_long_sequential_run_short_sequences_ok() {
        // Short sequential runs (< 8) are acceptable
        assert!(!has_long_sequential_run("abc123xyz")); // "abc" is only 3
        assert!(!has_long_sequential_run("1234abc5678")); // "1234" is only 4, "5678" is only 4
        assert!(!has_long_sequential_run("abcdefg")); // Only 7, needs 8+
    }

    // ==========================================================================
    // PERF-M3: Context Loading Timeout Tests
    // ==========================================================================

    #[test]
    fn test_context_timeout_configuration() {
        // Default timeout
        let handler = SessionStartHandler::new();
        assert_eq!(handler.context_timeout_ms, DEFAULT_CONTEXT_TIMEOUT_MS);

        // Custom timeout
        let handler = SessionStartHandler::new().with_context_timeout_ms(1000);
        assert_eq!(handler.context_timeout_ms, 1000);
    }

    #[test]
    fn test_context_timeout_zero_still_works() {
        // Even with 0ms timeout, handler should not panic - just skip expensive work
        let handler = SessionStartHandler::new().with_context_timeout_ms(0);
        let result = handler.build_session_context("test-session", "/project");

        // Should succeed with minimal context
        assert!(result.is_ok());
        let context = result.unwrap();
        // Should still have session header
        assert!(context.content.contains("test-session"));
        // was_truncated should be true due to timeout
        assert!(context.was_truncated);
    }

    #[test]
    fn test_context_timeout_large_value() {
        // With a very large timeout, normal context should be returned
        let handler = SessionStartHandler::new().with_context_timeout_ms(60_000);
        let result = handler.build_session_context("test-session", "/project");

        assert!(result.is_ok());
        let context = result.unwrap();
        // Should include guidance content
        assert!(context.content.contains("prompt_understanding"));
    }

    #[test]
    fn test_build_context_records_was_truncated_on_timeout() {
        // Very short timeout should mark context as truncated
        let handler = SessionStartHandler::new().with_context_timeout_ms(0);
        let result = handler.build_session_context("test", "/path");

        assert!(result.is_ok());
        let context = result.unwrap();
        // With 0ms timeout, should be truncated
        assert!(context.was_truncated);
    }
}
