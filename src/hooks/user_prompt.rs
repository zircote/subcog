//! User prompt submit hook handler.

use super::HookHandler;
use crate::config::SubcogConfig;
use crate::models::Namespace;
use crate::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use tracing::instrument;

/// Handles `UserPromptSubmit` hook events.
///
/// Detects signals for memory capture in user prompts.
pub struct UserPromptHandler {
    /// Configuration.
    config: SubcogConfig,
    /// Minimum confidence threshold for capture.
    confidence_threshold: f32,
}

/// Signal patterns for memory capture detection.
static DECISION_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
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

static PATTERN_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"(?i)\b(pattern|convention|standard|best practice)\b").ok(),
        Regex::new(r"(?i)\b(always|never|should|must)\b.*\b(when|if|before|after)\b").ok(),
        Regex::new(r"(?i)\b(rule|guideline|principle)\b").ok(),
    ]
    .into_iter()
    .flatten()
    .collect()
});

static LEARNING_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
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

static BLOCKER_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"(?i)\b(blocked|stuck|issue|problem|bug|error)\b").ok(),
        Regex::new(r"(?i)\b(fixed|solved|resolved|workaround|solution)\b").ok(),
        Regex::new(r"(?i)\b(doesn't work|not working|broken|fails)\b").ok(),
    ]
    .into_iter()
    .flatten()
    .collect()
});

static TECH_DEBT_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
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
static CAPTURE_COMMAND: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^@?subcog\s+(capture|remember|save|store)\b").ok()
        .unwrap_or_else(|| Regex::new(r"^$").ok().unwrap())
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
    pub fn new(config: SubcogConfig) -> Self {
        Self {
            config,
            confidence_threshold: 0.6,
        }
    }

    /// Sets the confidence threshold.
    #[must_use]
    pub const fn with_confidence_threshold(mut self, threshold: f32) -> Self {
        self.confidence_threshold = threshold;
        self
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
        let mut check_patterns = |patterns: &[Regex], namespace: Namespace| {
            let matches: Vec<String> = patterns
                .iter()
                .filter(|p| p.is_match(prompt))
                .map(|p| p.to_string())
                .collect();

            if !matches.is_empty() {
                let confidence = calculate_confidence(&matches, prompt);
                if confidence >= self.confidence_threshold {
                    signals.push(CaptureSignal {
                        namespace,
                        confidence,
                        matched_patterns: matches,
                        is_explicit: false,
                    });
                }
            }
        };

        check_patterns(&DECISION_PATTERNS, Namespace::Decisions);
        check_patterns(&PATTERN_PATTERNS, Namespace::Patterns);
        check_patterns(&LEARNING_PATTERNS, Namespace::Learnings);
        check_patterns(&BLOCKER_PATTERNS, Namespace::Blockers);
        check_patterns(&TECH_DEBT_PATTERNS, Namespace::TechDebt);

        // Sort by confidence, highest first
        signals.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

        signals
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
fn calculate_confidence(matches: &[String], prompt: &str) -> f32 {
    let base_confidence = 0.5;
    let match_bonus = 0.15_f32.min(matches.len() as f32 * 0.1);

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
        Self::new(SubcogConfig::default())
    }
}

impl HookHandler for UserPromptHandler {
    fn event_type(&self) -> &'static str {
        "UserPromptSubmit"
    }

    #[instrument(skip(self, input), fields(hook = "UserPromptSubmit"))]
    fn handle(&self, input: &str) -> Result<String> {
        // Parse input as JSON
        let input_json: serde_json::Value = serde_json::from_str(input).unwrap_or_else(|_| {
            serde_json::json!({})
        });

        // Extract prompt from input
        let prompt = input_json
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if prompt.is_empty() {
            let response = serde_json::json!({
                "signals": [],
                "should_capture": false
            });
            return serde_json::to_string(&response).map_err(|e| crate::Error::OperationFailed {
                operation: "serialize_response".to_string(),
                cause: e.to_string(),
            });
        }

        // Detect capture signals
        let signals = self.detect_signals(prompt);

        // Determine if we should capture
        let should_capture = signals.iter().any(|s| s.confidence >= self.confidence_threshold);

        // Extract content if capturing
        let content = if should_capture {
            Some(self.extract_content(prompt))
        } else {
            None
        };

        // Build response
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

        let mut response = serde_json::json!({
            "signals": signals_json,
            "should_capture": should_capture,
            "confidence_threshold": self.confidence_threshold
        });

        if let Some(content) = content {
            response["suggested_content"] = serde_json::Value::String(content);
            if let Some(signal) = signals.first() {
                response["suggested_namespace"] = serde_json::Value::String(signal.namespace.as_str().to_string());
            }
        }

        serde_json::to_string(&response).map_err(|e| crate::Error::OperationFailed {
            operation: "serialize_response".to_string(),
            cause: e.to_string(),
        })
    }
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
        assert_eq!(response.get("should_capture"), Some(&serde_json::Value::Bool(true)));
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

        let signals = handler.detect_signals("The best practice is to always validate input before processing");
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

        let signals = handler.detect_signals("This is a temporary workaround, we need to refactor later");
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
        assert_eq!(response.get("should_capture"), Some(&serde_json::Value::Bool(false)));
    }

    #[test]
    fn test_confidence_threshold() {
        let handler = UserPromptHandler::default()
            .with_confidence_threshold(0.9);

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
            "This is a longer prompt with more context."
        );
        assert!(high >= low);
    }
}
