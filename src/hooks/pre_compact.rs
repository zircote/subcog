//! Pre-compact hook handler.
//!
//! Analyzes content being compacted and auto-captures important memories.

use crate::Result;
use crate::hooks::HookHandler;
use crate::models::{CaptureRequest, Domain, Namespace};
use crate::services::CaptureService;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::instrument;

/// Handler for the `PreCompact` hook event.
///
/// Analyzes context being compacted and auto-captures valuable memories.
pub struct PreCompactHandler {
    /// Capture service instance.
    capture: Option<CaptureService>,
}

/// Input for the `PreCompact` hook.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PreCompactInput {
    /// Conversation context being compacted.
    #[serde(default)]
    pub context: String,
    /// Sections of the conversation.
    #[serde(default)]
    pub sections: Vec<ConversationSection>,
}

/// A section of conversation.
#[derive(Debug, Clone, Deserialize)]
pub struct ConversationSection {
    /// Section content.
    pub content: String,
    /// Type of content (user, assistant, `tool_result`).
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String {
    "assistant".to_string()
}

/// A memory that was auto-captured.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedMemory {
    /// Memory ID.
    pub memory_id: String,
    /// Namespace.
    pub namespace: String,
    /// Confidence score.
    pub confidence: f32,
}

/// Candidate for capture.
struct CaptureCandidate {
    content: String,
    namespace: Namespace,
    confidence: f32,
}

impl PreCompactHandler {
    /// Creates a new `PreCompact` handler.
    #[must_use]
    pub const fn new() -> Self {
        Self { capture: None }
    }

    /// Sets the capture service.
    #[must_use]
    pub fn with_capture(mut self, capture: CaptureService) -> Self {
        self.capture = Some(capture);
        self
    }

    /// Analyzes content and extracts capture candidates.
    fn analyze_content(&self, input: &PreCompactInput) -> Vec<CaptureCandidate> {
        let mut candidates = Vec::new();

        // Analyze the full context
        if !input.context.is_empty() {
            candidates.extend(Self::extract_from_text(&input.context));
        }

        // Analyze individual sections
        for section in &input.sections {
            if section.role == "assistant" {
                candidates.extend(Self::extract_from_text(&section.content));
            }
        }

        // Deduplicate similar candidates
        Self::deduplicate_candidates(candidates)
    }

    /// Extracts potential memories from text.
    fn extract_from_text(text: &str) -> Vec<CaptureCandidate> {
        let mut candidates = Vec::new();

        // Split into paragraphs or logical sections
        let sections: Vec<&str> = text
            .split("\n\n")
            .filter(|s| !s.trim().is_empty())
            .collect();

        for section in sections {
            let section = section.trim();
            if section.len() < 20 {
                continue;
            }

            // Check for decision-related language
            if contains_decision_language(section) {
                candidates.push(CaptureCandidate {
                    content: section.to_string(),
                    namespace: Namespace::Decisions,
                    confidence: calculate_section_confidence(section),
                });
            }
            // Check for learning-related language
            else if contains_learning_language(section) {
                candidates.push(CaptureCandidate {
                    content: section.to_string(),
                    namespace: Namespace::Learnings,
                    confidence: calculate_section_confidence(section),
                });
            }
            // Check for blocker/issue resolution language
            else if contains_blocker_language(section) {
                candidates.push(CaptureCandidate {
                    content: section.to_string(),
                    namespace: Namespace::Blockers,
                    confidence: calculate_section_confidence(section),
                });
            }
            // Check for pattern-related language
            else if contains_pattern_language(section) {
                candidates.push(CaptureCandidate {
                    content: section.to_string(),
                    namespace: Namespace::Patterns,
                    confidence: calculate_section_confidence(section),
                });
            }
        }

        candidates
    }

    /// Removes duplicate/similar candidates.
    fn deduplicate_candidates(mut candidates: Vec<CaptureCandidate>) -> Vec<CaptureCandidate> {
        // Sort by confidence descending
        candidates.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut result = Vec::new();
        let mut seen_prefixes: Vec<String> = Vec::new();

        for candidate in candidates {
            // Take first 50 chars as a "fingerprint"
            let prefix: String = candidate.content.chars().take(50).collect();

            // Check if we've seen a similar prefix
            let is_duplicate = seen_prefixes.iter().any(|p| {
                let common = p
                    .chars()
                    .zip(prefix.chars())
                    .take_while(|(a, b)| a == b)
                    .count();
                common > 30
            });

            if !is_duplicate {
                seen_prefixes.push(prefix);
                result.push(candidate);
            }
        }

        result
    }

    /// Performs the actual capture of candidates.
    fn capture_candidates(&self, candidates: Vec<CaptureCandidate>) -> Vec<CapturedMemory> {
        let Some(capture) = &self.capture else {
            return Vec::new();
        };

        let mut captured = Vec::new();

        for candidate in candidates {
            if candidate.confidence < 0.6 {
                continue;
            }

            let request = CaptureRequest {
                content: candidate.content.clone(),
                namespace: candidate.namespace,
                domain: Domain::default(),
                tags: vec!["auto-captured".to_string(), "pre-compact".to_string()],
                source: Some("PreCompactHandler".to_string()),
                skip_security_check: false,
            };

            if let Ok(result) = capture.capture(request) {
                captured.push(CapturedMemory {
                    memory_id: result.memory_id.as_str().to_string(),
                    namespace: candidate.namespace.as_str().to_string(),
                    confidence: candidate.confidence,
                });
            }
            // Errors are silently ignored, continue with other candidates
        }

        captured
    }
}

/// Checks if text contains decision-related language.
fn contains_decision_language(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("decided")
        || lower.contains("decision")
        || lower.contains("we'll use")
        || lower.contains("we're using")
        || lower.contains("going to use")
        || lower.contains("chose")
        || lower.contains("selected")
        || lower.contains("approach")
}

/// Checks if text contains learning-related language.
fn contains_learning_language(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("learned")
        || lower.contains("discovered")
        || lower.contains("realized")
        || lower.contains("til ")
        || lower.contains("turns out")
        || lower.contains("found out")
        || lower.contains("gotcha")
        || lower.contains("caveat")
}

/// Checks if text contains blocker-related language.
fn contains_blocker_language(text: &str) -> bool {
    let lower = text.to_lowercase();
    (lower.contains("fixed") || lower.contains("resolved") || lower.contains("solved"))
        && (lower.contains("issue")
            || lower.contains("bug")
            || lower.contains("error")
            || lower.contains("problem"))
}

/// Checks if text contains pattern-related language.
fn contains_pattern_language(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("pattern")
        || lower.contains("best practice")
        || lower.contains("convention")
        || lower.contains("always ")
        || lower.contains("never ")
        || lower.contains("should always")
        || lower.contains("must ")
}

/// Calculates confidence for a section.
fn calculate_section_confidence(section: &str) -> f32 {
    let mut confidence: f32 = 0.5;

    // Longer sections are more likely to be meaningful
    if section.len() > 100 {
        confidence += 0.1;
    }
    if section.len() > 200 {
        confidence += 0.1;
    }

    // Multiple sentences suggest more complete thought
    let sentence_count = section.matches('.').count() + section.matches('!').count();
    if sentence_count >= 2 {
        confidence += 0.1;
    }

    // Code blocks suggest technical content
    if section.contains("```") || section.contains("    ") {
        confidence += 0.05;
    }

    confidence.min(0.95)
}

impl Default for PreCompactHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HookHandler for PreCompactHandler {
    fn event_type(&self) -> &'static str {
        "PreCompact"
    }

    #[instrument(
        skip(self, input),
        fields(hook = "PreCompact", captures = tracing::field::Empty)
    )]
    fn handle(&self, input: &str) -> Result<String> {
        let start = Instant::now();
        let (result, capture_count) = {
            let parsed: PreCompactInput =
                serde_json::from_str(input).unwrap_or_else(|_| PreCompactInput {
                    context: input.to_string(),
                    ..Default::default()
                });

            // Analyze content for capture candidates
            let candidates = self.analyze_content(&parsed);

            // Capture the candidates
            let captured = self.capture_candidates(candidates);
            let capture_count = captured.len();
            let span = tracing::Span::current();
            span.record("captures", capture_count);

            // Build metadata
            let metadata = serde_json::json!({
                "captured": !captured.is_empty(),
                "captures": captured.iter().map(|c| serde_json::json!({
                    "memory_id": c.memory_id,
                    "namespace": c.namespace,
                    "confidence": c.confidence
                })).collect::<Vec<_>>()
            });

            // Build context message about captured memories
            let context_message = if captured.is_empty() {
                None
            } else {
                let mut lines = vec![
                    "**Subcog Pre-Compact Auto-Capture**\n".to_string(),
                    format!(
                        "Captured {} memories before context compaction:\n",
                        captured.len()
                    ),
                ];
                for c in &captured {
                    lines.push(format!(
                        "- `{}`: {} (confidence: {:.0}%)",
                        c.namespace,
                        c.memory_id,
                        c.confidence * 100.0
                    ));
                }
                Some(lines.join("\n"))
            };

            // Build Claude Code hook response format per specification
            // See: https://docs.anthropic.com/en/docs/claude-code/hooks
            let response = context_message.map_or_else(
                || serde_json::json!({}),
                |ctx| {
                    // Embed metadata as XML comment for debugging
                    let metadata_str = serde_json::to_string(&metadata).unwrap_or_default();
                    let context_with_metadata =
                        format!("{ctx}\n\n<!-- subcog-metadata: {metadata_str} -->");
                    serde_json::json!({
                        "hookSpecificOutput": {
                            "hookEventName": "PreCompact",
                            "additionalContext": context_with_metadata
                        }
                    })
                },
            );

            (
                serde_json::to_string(&response).map_err(|e| crate::Error::OperationFailed {
                    operation: "serialize_output".to_string(),
                    cause: e.to_string(),
                }),
                capture_count,
            )
        };

        let status = if result.is_ok() { "success" } else { "error" };
        metrics::counter!(
            "hook_executions_total",
            "hook_type" => "PreCompact",
            "status" => status
        )
        .increment(1);
        metrics::histogram!("hook_duration_ms", "hook_type" => "PreCompact")
            .record(start.elapsed().as_secs_f64() * 1000.0);
        if capture_count > 0 {
            metrics::counter!(
                "hook_auto_capture_total",
                "hook_type" => "PreCompact",
                "namespace" => "mixed"
            )
            .increment(capture_count as u64);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_creation() {
        let handler = PreCompactHandler::default();
        assert_eq!(handler.event_type(), "PreCompact");
    }

    #[test]
    fn test_contains_decision_language() {
        assert!(contains_decision_language("We decided to use PostgreSQL"));
        assert!(contains_decision_language("The decision was made"));
        assert!(contains_decision_language("We chose this approach"));
        assert!(!contains_decision_language("Just some regular text"));
    }

    #[test]
    fn test_contains_learning_language() {
        assert!(contains_learning_language("TIL that Rust has great safety"));
        assert!(contains_learning_language("I realized the problem"));
        assert!(contains_learning_language("Turns out it was a bug"));
        assert!(!contains_learning_language("Regular text here"));
    }

    #[test]
    fn test_contains_blocker_language() {
        assert!(contains_blocker_language("Fixed the issue with auth"));
        assert!(contains_blocker_language("Resolved the bug in parser"));
        assert!(!contains_blocker_language("Just fixed the typo"));
    }

    #[test]
    fn test_contains_pattern_language() {
        assert!(contains_pattern_language("This is a common pattern"));
        assert!(contains_pattern_language("Best practice is to..."));
        assert!(contains_pattern_language("You should always check..."));
        // Use text that truly has no pattern-related words
        assert!(!contains_pattern_language(
            "Hello world, this is regular code"
        ));
    }

    #[test]
    fn test_calculate_confidence() {
        let short_text = "Short";
        let medium_text =
            "This is a medium length text that contains some words. It has multiple sentences.";
        let long_text = "This is a much longer text that contains many words and sentences. It should have higher confidence. The text goes on and on with more information. Here is even more content to make it longer.";

        let short_conf = calculate_section_confidence(short_text);
        let medium_conf = calculate_section_confidence(medium_text);
        let long_conf = calculate_section_confidence(long_text);

        assert!(short_conf < medium_conf);
        assert!(medium_conf < long_conf);
    }

    #[test]
    fn test_handle_empty_input() {
        let handler = PreCompactHandler::default();
        let result = handler.handle("{}");

        assert!(result.is_ok());
        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Claude Code hook format - empty response when nothing captured
        assert!(response.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_analyze_content() {
        let handler = PreCompactHandler::default();
        let input = PreCompactInput {
            context: "We decided to use PostgreSQL for the database. This was a key architectural decision.\n\nTIL that connection pooling is important for performance.".to_string(),
            sections: vec![],
        };

        let candidates = handler.analyze_content(&input);
        assert!(!candidates.is_empty());
    }
}
