//! User prompt submit hook handler.
// Allow expect() on static regex patterns - these are guaranteed to compile
#![allow(clippy::expect_used)]

use super::HookHandler;
use super::search_context::{AdaptiveContextConfig, MemoryContext, SearchContextBuilder};
use super::search_intent::{SearchIntent, detect_search_intent};
use crate::Result;
use crate::models::Namespace;
use crate::services::RecallService;
use regex::Regex;
use std::sync::LazyLock;
use tracing::instrument;

/// Handles `UserPromptSubmit` hook events.
///
/// Detects signals for memory capture in user prompts and search intent.
pub struct UserPromptHandler {
    /// Minimum confidence threshold for capture.
    confidence_threshold: f32,
    /// Minimum confidence threshold for search intent injection.
    search_intent_threshold: f32,
    /// Configuration for adaptive context injection.
    context_config: AdaptiveContextConfig,
    /// Optional recall service for memory retrieval.
    recall_service: Option<RecallService>,
}

/// Signal patterns for memory capture detection.
static DECISION_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)\b(we('re| are|'ll| will) (going to |gonna )?use|let's use|using)\b").ok(),
        Regex::new(r"(?i)\b(decided|decision|choosing|chose|picked|selected)\b").ok(),
        Regex::new(r"(?i)\b(architecture|design|approach|strategy|solution)\b").ok(),
        Regex::new(r"(?i)\b(from now on|going forward|henceforth)\b").ok(),
        Regex::new(r"(?i)\b(always|never) (do|use|implement)\b").ok(),
    ]
    .into_iter()
    .flatten()
    .collect()
});

static PATTERN_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)\b(pattern|convention|standard|best practice)\b").ok(),
        Regex::new(r"(?i)\b(always|never|should|must)\b.*\b(when|if|before|after)\b").ok(),
        Regex::new(r"(?i)\b(rule|guideline|principle)\b").ok(),
    ]
    .into_iter()
    .flatten()
    .collect()
});

static LEARNING_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)\b(learned|discovered|realized|found out|figured out)\b").ok(),
        Regex::new(r"(?i)\b(TIL|turns out|apparently|actually)\b").ok(),
        Regex::new(r"(?i)\b(gotcha|caveat|quirk|edge case)\b").ok(),
        Regex::new(r"(?i)\b(insight|understanding|revelation)\b").ok(),
    ]
    .into_iter()
    .flatten()
    .collect()
});

static BLOCKER_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)\b(blocked|stuck|issue|problem|bug|error)\b").ok(),
        Regex::new(r"(?i)\b(fixed|solved|resolved|workaround|solution)\b").ok(),
        Regex::new(r"(?i)\b(doesn't work|not working|broken|fails)\b").ok(),
    ]
    .into_iter()
    .flatten()
    .collect()
});

static TECH_DEBT_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)\b(tech debt|technical debt|refactor|cleanup)\b").ok(),
        Regex::new(r"(?i)\b(TODO|FIXME|HACK|XXX)\b").ok(),
        Regex::new(r"(?i)\b(temporary|workaround|quick fix|shortcut)\b").ok(),
    ]
    .into_iter()
    .flatten()
    .collect()
});

/// Explicit capture commands.
static CAPTURE_COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    // This regex is static and guaranteed to compile
    Regex::new(r"(?i)^@?subcog\s+(capture|remember|save|store)\b")
        .expect("static regex: capture command pattern")
});

/// A detected signal for memory capture.
#[derive(Debug, Clone)]
pub struct CaptureSignal {
    /// Suggested namespace for the memory.
    pub namespace: Namespace,
    /// Confidence score (0.0 to 1.0).
    pub confidence: f32,
    /// Matched patterns.
    pub matched_patterns: Vec<String>,
    /// Whether this was an explicit command.
    pub is_explicit: bool,
}

impl UserPromptHandler {
    /// Creates a new handler.
    #[must_use]
    pub fn new() -> Self {
        Self {
            confidence_threshold: 0.6,
            search_intent_threshold: 0.5,
            context_config: AdaptiveContextConfig::default(),
            recall_service: None,
        }
    }

    /// Sets the confidence threshold for capture.
    #[must_use]
    pub const fn with_confidence_threshold(mut self, threshold: f32) -> Self {
        self.confidence_threshold = threshold;
        self
    }

    /// Sets the confidence threshold for search intent injection.
    #[must_use]
    pub const fn with_search_intent_threshold(mut self, threshold: f32) -> Self {
        self.search_intent_threshold = threshold;
        self
    }

    /// Sets the adaptive context configuration.
    #[must_use]
    pub const fn with_context_config(mut self, config: AdaptiveContextConfig) -> Self {
        self.context_config = config;
        self
    }

    /// Sets the recall service for memory retrieval.
    #[must_use]
    pub fn with_recall_service(mut self, service: RecallService) -> Self {
        self.recall_service = Some(service);
        self
    }

    /// Builds memory context from a search intent using the `SearchContextBuilder`.
    fn build_memory_context(&self, intent: &SearchIntent) -> MemoryContext {
        let mut builder = SearchContextBuilder::new().with_config(self.context_config.clone());

        if let Some(ref recall) = self.recall_service {
            builder = builder.with_recall_service(recall);
        }

        // Build context, falling back to empty on error
        builder
            .build_context(intent)
            .unwrap_or_else(|_| MemoryContext::empty())
    }

    /// Detects search intent from the prompt.
    fn detect_search_intent(&self, prompt: &str) -> Option<SearchIntent> {
        let intent = detect_search_intent(prompt)?;
        if intent.confidence >= self.search_intent_threshold {
            Some(intent)
        } else {
            None
        }
    }

    /// Detects capture signals in the prompt.
    fn detect_signals(&self, prompt: &str) -> Vec<CaptureSignal> {
        let mut signals = Vec::new();

        // Check for explicit capture command first
        if CAPTURE_COMMAND.is_match(prompt) {
            signals.push(CaptureSignal {
                namespace: Namespace::Decisions,
                confidence: 1.0,
                matched_patterns: vec!["explicit_command".to_string()],
                is_explicit: true,
            });
            return signals;
        }

        // Check each namespace's patterns
        self.check_patterns(
            &DECISION_PATTERNS,
            Namespace::Decisions,
            prompt,
            &mut signals,
        );
        self.check_patterns(&PATTERN_PATTERNS, Namespace::Patterns, prompt, &mut signals);
        self.check_patterns(
            &LEARNING_PATTERNS,
            Namespace::Learnings,
            prompt,
            &mut signals,
        );
        self.check_patterns(&BLOCKER_PATTERNS, Namespace::Blockers, prompt, &mut signals);
        self.check_patterns(
            &TECH_DEBT_PATTERNS,
            Namespace::TechDebt,
            prompt,
            &mut signals,
        );

        // Sort by confidence, highest first
        signals.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        signals
    }

    /// Checks patterns for a specific namespace and adds matching signals.
    fn check_patterns(
        &self,
        patterns: &[Regex],
        namespace: Namespace,
        prompt: &str,
        signals: &mut Vec<CaptureSignal>,
    ) {
        let pattern_matches: Vec<String> = patterns
            .iter()
            .filter(|p| p.is_match(prompt))
            .map(std::string::ToString::to_string)
            .collect();

        if pattern_matches.is_empty() {
            return;
        }

        let confidence = calculate_confidence(&pattern_matches, prompt);
        if confidence < self.confidence_threshold {
            return;
        }

        signals.push(CaptureSignal {
            namespace,
            confidence,
            matched_patterns: pattern_matches,
            is_explicit: false,
        });
    }

    /// Extracts the content to capture from the prompt.
    fn extract_content(&self, prompt: &str) -> String {
        // Remove explicit command prefix if present
        let content = CAPTURE_COMMAND.replace(prompt, "").trim().to_string();

        // Clean up common prefixes
        let content = content
            .trim_start_matches(':')
            .trim_start_matches('-')
            .trim();

        content.to_string()
    }
}

/// Calculates confidence score based on pattern matches.
#[allow(clippy::cast_precision_loss)]
fn calculate_confidence(pattern_matches: &[String], prompt: &str) -> f32 {
    let base_confidence = 0.5;
    let match_bonus = 0.15_f32.min(pattern_matches.len() as f32 * 0.1);

    // Longer prompts with patterns are more likely to be intentional
    let length_factor = if prompt.len() > 50 { 0.1 } else { 0.0 };

    // Multiple sentences suggest more context
    let sentence_factor = if prompt.contains('.') || prompt.contains('!') || prompt.contains('?') {
        0.1
    } else {
        0.0
    };

    (base_confidence + match_bonus + length_factor + sentence_factor).min(0.95)
}

impl Default for UserPromptHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HookHandler for UserPromptHandler {
    fn event_type(&self) -> &'static str {
        "UserPromptSubmit"
    }

    #[instrument(skip(self, input), fields(hook = "UserPromptSubmit"))]
    fn handle(&self, input: &str) -> Result<String> {
        // Parse input as JSON
        let input_json: serde_json::Value =
            serde_json::from_str(input).unwrap_or_else(|_| serde_json::json!({}));

        // Extract prompt from input
        let prompt = input_json
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if prompt.is_empty() {
            let response = serde_json::json!({
                "continue": true,
                "metadata": {
                    "signals": [],
                    "should_capture": false
                }
            });
            return serde_json::to_string(&response).map_err(|e| crate::Error::OperationFailed {
                operation: "serialize_response".to_string(),
                cause: e.to_string(),
            });
        }

        // Detect capture signals
        let signals = self.detect_signals(prompt);

        // Determine if we should capture
        let should_capture = signals
            .iter()
            .any(|s| s.confidence >= self.confidence_threshold);

        // Extract content if capturing
        let content = if should_capture {
            Some(self.extract_content(prompt))
        } else {
            None
        };

        // Build signals JSON for metadata
        let signals_json: Vec<serde_json::Value> = signals
            .iter()
            .map(|s| {
                serde_json::json!({
                    "namespace": s.namespace.as_str(),
                    "confidence": s.confidence,
                    "matched_patterns": s.matched_patterns,
                    "is_explicit": s.is_explicit
                })
            })
            .collect();

        let mut metadata = serde_json::json!({
            "signals": signals_json,
            "should_capture": should_capture,
            "confidence_threshold": self.confidence_threshold
        });

        // Detect search intent for proactive memory surfacing
        let search_intent = self.detect_search_intent(prompt);

        // Build memory context and add to metadata if search intent detected
        let memory_context = if let Some(ref intent) = search_intent {
            let ctx = self.build_memory_context(intent);
            metadata["search_intent"] = serde_json::json!({
                "detected": ctx.search_intent_detected,
                "intent_type": ctx.intent_type,
                "confidence": intent.confidence,
                "topics": ctx.topics,
                "keywords": intent.keywords,
                "source": intent.source.as_str()
            });
            metadata["memory_context"] =
                serde_json::to_value(&ctx).unwrap_or(serde_json::Value::Null);
            Some(ctx)
        } else {
            metadata["search_intent"] = serde_json::json!({
                "detected": false
            });
            None
        };

        // Build context message for capture suggestions
        let context_message =
            build_capture_context(should_capture, content.as_ref(), &signals, &mut metadata);

        // Build search intent context (if detected)
        let search_context = memory_context.as_ref().map(build_memory_context_text);

        // Build Claude Code hook response format
        let mut response = serde_json::json!({
            "continue": true,
            "metadata": metadata
        });

        // Combine context messages
        let combined_context = match (context_message, search_context) {
            (Some(capture), Some(search)) => Some(format!("{capture}\n\n---\n\n{search}")),
            (Some(capture), None) => Some(capture),
            (None, Some(search)) => Some(search),
            (None, None) => None,
        };

        // Add context only if we have a suggestion
        if let Some(ctx) = combined_context {
            response["context"] = serde_json::Value::String(ctx);
        }

        serde_json::to_string(&response).map_err(|e| crate::Error::OperationFailed {
            operation: "serialize_response".to_string(),
            cause: e.to_string(),
        })
    }
}

/// Builds context message for capture suggestions.
fn build_capture_context(
    should_capture: bool,
    content: Option<&String>,
    signals: &[CaptureSignal],
    metadata: &mut serde_json::Value,
) -> Option<String> {
    if !should_capture {
        return None;
    }

    let content_str = content.map_or("", String::as_str);
    if content_str.is_empty() {
        return None;
    }

    // Get the top signal for suggestions
    let top_signal = signals.first()?;

    // Add capture suggestion to metadata
    metadata["capture_suggestion"] = serde_json::json!({
        "namespace": top_signal.namespace.as_str(),
        "content_preview": truncate_for_display(content_str, 100),
        "confidence": top_signal.confidence,
    });

    // Build context message
    let mut lines = vec!["**Subcog Capture Suggestion**\n".to_string()];

    if top_signal.is_explicit {
        lines.push(format!(
            "Explicit capture command detected. Capturing to `{}`:\n",
            top_signal.namespace.as_str()
        ));
        lines.push(format!("> {}", truncate_for_display(content_str, 200)));
        lines.push("\nUse `subcog_capture` tool to save this memory.".to_string());
    } else {
        lines.push(format!(
            "Detected {} signal (confidence: {:.0}%):\n",
            top_signal.namespace.as_str(),
            top_signal.confidence * 100.0
        ));
        lines.push(format!("> {}", truncate_for_display(content_str, 200)));
        lines.push(format!(
            "\n**Suggestion**: Consider capturing this as a `{}` memory.",
            top_signal.namespace.as_str()
        ));
        lines.push(
            "Use `subcog_capture` tool or ask: \"Should I save this to subcog?\"".to_string(),
        );
    }

    Some(lines.join("\n"))
}

/// Truncates content for display in suggestions.
fn truncate_for_display(content: &str, max_len: usize) -> String {
    if content.len() <= max_len {
        content.to_string()
    } else {
        format!("{}...", &content[..max_len.saturating_sub(3)])
    }
}

/// Builds context message from memory context.
fn build_memory_context_text(ctx: &MemoryContext) -> String {
    let mut lines = vec!["**Subcog Memory Context**\n".to_string()];

    if let Some(ref intent_type) = ctx.intent_type {
        lines.push(format!("Intent type: **{intent_type}**\n"));
    }

    if !ctx.topics.is_empty() {
        lines.push(format!("Topics: {}\n", ctx.topics.join(", ")));
    }

    // Show injected memories if any
    if !ctx.injected_memories.is_empty() {
        lines.push("\n**Relevant memories**:".to_string());
        for memory in ctx.injected_memories.iter().take(5) {
            lines.push(format!(
                "- [{}] {}: {}",
                memory.namespace,
                memory.id,
                truncate_for_display(&memory.content_preview, 80)
            ));
        }
    }

    // Show reminder if present
    if let Some(ref reminder) = ctx.reminder {
        lines.push(format!("\n**Reminder**: {reminder}"));
    }

    // Suggest resources
    if !ctx.suggested_resources.is_empty() {
        lines.push("\n**Suggested resources**:".to_string());
        for resource in ctx.suggested_resources.iter().take(4) {
            lines.push(format!("- `{resource}`"));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_creation() {
        let handler = UserPromptHandler::default();
        assert_eq!(handler.event_type(), "UserPromptSubmit");
    }

    #[test]
    fn test_explicit_capture_command() {
        let handler = UserPromptHandler::default();

        let input = r#"{"prompt": "@subcog capture Use PostgreSQL for storage"}"#;

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
            metadata.get("should_capture"),
            Some(&serde_json::Value::Bool(true))
        );
        // Should have context with capture suggestion
        assert!(response.get("context").is_some());
    }

    #[test]
    fn test_decision_signal_detection() {
        let handler = UserPromptHandler::default();

        let signals = handler.detect_signals("We're going to use Rust for this project");
        assert!(!signals.is_empty());
        assert!(signals.iter().any(|s| s.namespace == Namespace::Decisions));
    }

    #[test]
    fn test_learning_signal_detection() {
        let handler = UserPromptHandler::default();

        let signals = handler.detect_signals("TIL that SQLite has a row limit of 2GB");
        assert!(!signals.is_empty());
        assert!(signals.iter().any(|s| s.namespace == Namespace::Learnings));
    }

    #[test]
    fn test_pattern_signal_detection() {
        let handler = UserPromptHandler::default();

        let signals = handler
            .detect_signals("The best practice is to always validate input before processing");
        assert!(!signals.is_empty());
        assert!(signals.iter().any(|s| s.namespace == Namespace::Patterns));
    }

    #[test]
    fn test_blocker_signal_detection() {
        let handler = UserPromptHandler::default();

        let signals = handler.detect_signals("I fixed the bug by adding a null check");
        assert!(!signals.is_empty());
        assert!(signals.iter().any(|s| s.namespace == Namespace::Blockers));
    }

    #[test]
    fn test_tech_debt_signal_detection() {
        let handler = UserPromptHandler::default();

        let signals =
            handler.detect_signals("This is a temporary workaround, we need to refactor later");
        assert!(!signals.is_empty());
        assert!(signals.iter().any(|s| s.namespace == Namespace::TechDebt));
    }

    #[test]
    fn test_no_signals_for_generic_prompt() {
        let handler = UserPromptHandler::default();

        let signals = handler.detect_signals("Hello, how are you?");
        // May or may not have signals, but confidence should be low
        for signal in &signals {
            assert!(signal.confidence < 0.8);
        }
    }

    #[test]
    fn test_empty_prompt() {
        let handler = UserPromptHandler::default();

        let input = r#"{"prompt": ""}"#;

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
            metadata.get("should_capture"),
            Some(&serde_json::Value::Bool(false))
        );
    }

    #[test]
    fn test_confidence_threshold() {
        let handler = UserPromptHandler::default().with_confidence_threshold(0.9);

        // Even with patterns, high threshold should reject low-confidence signals
        let signals = handler.detect_signals("maybe use something");
        let high_confidence: Vec<_> = signals.iter().filter(|s| s.confidence >= 0.9).collect();
        // Most implicit signals won't reach 0.9
        assert!(high_confidence.is_empty() || high_confidence.iter().all(|s| s.is_explicit));
    }

    #[test]
    fn test_extract_content() {
        let handler = UserPromptHandler::default();

        let content = handler.extract_content("@subcog capture: Use PostgreSQL");
        assert_eq!(content, "Use PostgreSQL");

        let content = handler.extract_content("Just a regular prompt");
        assert_eq!(content, "Just a regular prompt");
    }

    #[test]
    fn test_calculate_confidence() {
        // More matches = higher confidence
        let low = calculate_confidence(&["pattern1".to_string()], "short");
        let high = calculate_confidence(
            &["pattern1".to_string(), "pattern2".to_string()],
            "This is a longer prompt with more context.",
        );
        assert!(high >= low);
    }

    #[test]
    fn test_search_intent_detection_in_handle() {
        let handler = UserPromptHandler::default();

        // Test with a clear search intent prompt
        let input = r#"{"prompt": "How do I implement authentication in this project?"}"#;
        let result = handler.handle(input);
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        let metadata = response.get("metadata").unwrap();

        // Should have search_intent in metadata
        let search_intent = metadata.get("search_intent").unwrap();
        assert_eq!(
            search_intent.get("detected"),
            Some(&serde_json::Value::Bool(true))
        );
        assert_eq!(
            search_intent.get("intent_type"),
            Some(&serde_json::Value::String("howto".to_string()))
        );
    }

    #[test]
    fn test_search_intent_no_detection() {
        let handler = UserPromptHandler::default();

        // Test with a prompt that doesn't have search intent
        let input = r#"{"prompt": "I finished the task."}"#;
        let result = handler.handle(input);
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        let metadata = response.get("metadata").unwrap();

        // Should have search_intent in metadata but detected=false
        let search_intent = metadata.get("search_intent").unwrap();
        assert_eq!(
            search_intent.get("detected"),
            Some(&serde_json::Value::Bool(false))
        );
    }

    #[test]
    fn test_search_intent_threshold() {
        let handler = UserPromptHandler::default().with_search_intent_threshold(0.9);

        // Test with a prompt that would normally detect intent
        let input = r#"{"prompt": "how to"}"#;
        let result = handler.handle(input);
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        let metadata = response.get("metadata").unwrap();

        // Should NOT detect because confidence won't meet 0.9 threshold
        let search_intent = metadata.get("search_intent").unwrap();
        assert_eq!(
            search_intent.get("detected"),
            Some(&serde_json::Value::Bool(false))
        );
    }

    #[test]
    fn test_search_intent_topics_extraction() {
        let handler = UserPromptHandler::default();

        let input = r#"{"prompt": "How do I configure the database connection?"}"#;
        let result = handler.handle(input);
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        let metadata = response.get("metadata").unwrap();

        let search_intent = metadata.get("search_intent").unwrap();
        let topics = search_intent.get("topics").unwrap().as_array().unwrap();

        // Should extract topics like "database", "connection", "configure"
        assert!(!topics.is_empty());
    }
}
