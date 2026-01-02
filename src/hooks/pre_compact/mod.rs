//! Pre-compact hook handler.
//!
//! Analyzes content being compacted and auto-captures important memories.
//! Integrates with `DeduplicationService` to avoid capturing duplicate memories.
//!
//! # Module Structure
//!
//! - [`analyzer`]: Content analysis and namespace classification
//! - [`orchestrator`]: Capture coordination with deduplication
//! - [`formatter`]: Response formatting for hook output

mod analyzer;
mod formatter;
mod orchestrator;

pub use analyzer::{
    CaptureCandidate, calculate_section_confidence, contains_blocker_language,
    contains_context_language, contains_decision_language, contains_learning_language,
    contains_pattern_language,
};
pub use formatter::ResponseFormatter;
// CapturedMemory and SkippedDuplicate are internal types used by orchestrator and formatter
pub use orchestrator::CaptureOrchestrator;

use crate::Result;
use crate::hooks::HookHandler;
use crate::llm::LlmProvider;
use crate::models::Namespace;
use crate::services::CaptureService;
use crate::services::deduplication::Deduplicator;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;
use tracing::instrument;

// Content analysis thresholds
//
// These constants are intentionally kept in the pre_compact module rather than
// centralized in config for the following reasons:
// 1. They are implementation details specific to the pre-compact hook algorithm
// 2. They are not user-configurable and don't need environment variable support
// 3. Moving them to config would increase coupling between modules
// 4. They are compile-time constants that benefit from inlining

/// Minimum section length to consider for capture.
pub const MIN_SECTION_LENGTH: usize = 20;
/// Length of content fingerprint for deduplication.
pub const FINGERPRINT_LENGTH: usize = 50;
/// Minimum common characters to consider a duplicate.
pub const MIN_COMMON_CHARS_FOR_DUPLICATE: usize = 30;

/// Handler for the `PreCompact` hook event.
///
/// Analyzes context being compacted and auto-captures valuable memories.
/// Integrates with `DeduplicationService` to check for duplicates before capture.
///
/// # Deduplication
///
/// When a deduplication service is configured, each candidate is checked against:
/// 1. **Exact match**: SHA256 hash comparison
/// 2. **Semantic similarity**: Embedding cosine similarity (if embeddings available)
/// 3. **Recent capture**: LRU cache with TTL
///
/// Duplicates are skipped and reported in the hook output.
///
/// # LLM Analysis Mode
///
/// When an LLM provider is configured and `use_llm_analysis` is enabled, content
/// that doesn't match keyword-based detection patterns will be analyzed by the
/// LLM for classification. This provides more accurate namespace assignment at
/// the cost of increased latency.
///
/// Configure via environment variable: `SUBCOG_AUTO_CAPTURE_USE_LLM=true`
pub struct PreCompactHandler {
    /// Capture orchestrator for coordinating captures with deduplication.
    orchestrator: CaptureOrchestrator,
    /// Optional LLM provider for content classification.
    llm: Option<Arc<dyn LlmProvider>>,
    /// Whether to use LLM for classifying ambiguous content.
    use_llm_analysis: bool,
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

impl PreCompactHandler {
    /// Creates a new `PreCompact` handler.
    ///
    /// The `use_llm_analysis` setting is loaded from the environment variable
    /// `SUBCOG_AUTO_CAPTURE_USE_LLM` (default: false).
    #[must_use]
    pub fn new() -> Self {
        let use_llm_analysis = std::env::var("SUBCOG_AUTO_CAPTURE_USE_LLM")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);

        Self {
            orchestrator: CaptureOrchestrator::new(),
            llm: None,
            use_llm_analysis,
        }
    }

    /// Sets the capture service.
    #[must_use]
    pub fn with_capture(mut self, capture: CaptureService) -> Self {
        self.orchestrator = self.orchestrator.with_capture(capture);
        self
    }

    /// Sets the deduplication service.
    ///
    /// When set, candidates are checked for duplicates before capture.
    /// Duplicates are skipped and reported in the hook output.
    #[must_use]
    pub fn with_deduplication(mut self, dedup: Arc<dyn Deduplicator>) -> Self {
        self.orchestrator = self.orchestrator.with_deduplication(dedup);
        self
    }

    /// Sets the LLM provider for content classification.
    ///
    /// When set (and `SUBCOG_AUTO_CAPTURE_USE_LLM=true`), content that doesn't
    /// match keyword-based detection will be analyzed by the LLM.
    #[must_use]
    pub fn with_llm(mut self, llm: Arc<dyn LlmProvider>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Enables or disables LLM analysis mode.
    ///
    /// This overrides the `SUBCOG_AUTO_CAPTURE_USE_LLM` environment variable.
    #[must_use]
    pub const fn with_llm_analysis(mut self, enabled: bool) -> Self {
        self.use_llm_analysis = enabled;
        self
    }

    /// Analyzes content and extracts capture candidates.
    fn analyze_content(&self, input: &PreCompactInput) -> Vec<CaptureCandidate> {
        let mut candidates = Vec::new();

        // Analyze the full context
        if !input.context.is_empty() {
            candidates.extend(self.extract_from_text(&input.context));
        }

        // Analyze individual sections
        for section in &input.sections {
            if section.role == "assistant" {
                candidates.extend(self.extract_from_text(&section.content));
            }
        }

        // Deduplicate similar candidates
        analyzer::deduplicate_candidates(candidates)
    }

    /// Uses LLM to classify content that didn't match keyword detection.
    ///
    /// Returns `Some(CaptureCandidate)` if the LLM suggests capturing,
    /// `None` if LLM is unavailable or suggests not capturing.
    fn classify_with_llm(&self, section: &str) -> Option<CaptureCandidate> {
        let llm = self.llm.as_ref()?;

        match llm.analyze_for_capture(section) {
            Ok(analysis) if analysis.should_capture && analysis.confidence > 0.6 => {
                let namespace = analysis
                    .suggested_namespace
                    .as_ref()
                    .and_then(|ns| Namespace::parse(ns))
                    .unwrap_or(Namespace::Context);

                tracing::debug!(
                    namespace = %namespace.as_str(),
                    confidence = analysis.confidence,
                    reasoning = %analysis.reasoning,
                    "LLM classified content for capture"
                );

                metrics::counter!(
                    "hook_llm_classifications_total",
                    "hook_type" => "PreCompact",
                    "namespace" => namespace.as_str().to_string(),
                    "result" => "capture"
                )
                .increment(1);

                Some(CaptureCandidate {
                    content: section.to_string(),
                    namespace,
                    confidence: analysis.confidence,
                })
            },
            Ok(analysis) => {
                tracing::debug!(
                    confidence = analysis.confidence,
                    should_capture = analysis.should_capture,
                    "LLM analysis did not suggest capture"
                );

                metrics::counter!(
                    "hook_llm_classifications_total",
                    "hook_type" => "PreCompact",
                    "result" => "skip"
                )
                .increment(1);

                None
            },
            Err(e) => {
                tracing::warn!(error = %e, "LLM classification failed, skipping content");
                None
            },
        }
    }

    /// Extracts potential memories from text.
    ///
    /// Uses keyword-based detection first, then optionally falls back to LLM
    /// classification if `use_llm_analysis` is enabled.
    fn extract_from_text(&self, text: &str) -> Vec<CaptureCandidate> {
        let mut candidates = Vec::new();

        // Split into paragraphs or logical sections
        let sections: Vec<&str> = text
            .split("\n\n")
            .filter(|s| !s.trim().is_empty())
            .collect();

        for section in sections {
            let section = section.trim();
            if section.len() < MIN_SECTION_LENGTH {
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
            // Check for context-related language (explains "why" behind decisions)
            else if contains_context_language(section) {
                candidates.push(CaptureCandidate {
                    content: section.to_string(),
                    namespace: Namespace::Context,
                    confidence: calculate_section_confidence(section),
                });
            }
            // No keyword match - try LLM classification if enabled
            else if self.use_llm_analysis {
                candidates.extend(self.classify_with_llm(section));
            }
        }

        candidates
    }

    /// Records metrics for the hook execution.
    fn record_metrics(status: &str, duration_ms: f64, capture_count: usize, skip_count: usize) {
        metrics::counter!(
            "hook_executions_total",
            "hook_type" => "PreCompact",
            "status" => status.to_string()
        )
        .increment(1);
        metrics::histogram!("hook_duration_ms", "hook_type" => "PreCompact").record(duration_ms);
        if capture_count > 0 {
            metrics::counter!(
                "hook_auto_capture_total",
                "hook_type" => "PreCompact",
                "namespace" => "mixed"
            )
            .increment(capture_count as u64);
        }
        if skip_count > 0 {
            metrics::counter!(
                "hook_deduplication_skipped_total",
                "hook_type" => "PreCompact",
                "reason" => "aggregate"
            )
            .increment(skip_count as u64);
        }
    }
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

        // Parse input
        let parsed: PreCompactInput =
            serde_json::from_str(input).unwrap_or_else(|_| PreCompactInput {
                context: input.to_string(),
                ..Default::default()
            });

        // Analyze content for capture candidates
        let candidates = self.analyze_content(&parsed);

        // Capture the candidates (with deduplication if configured)
        let (captured, skipped) = self.orchestrator.capture_candidates(candidates);
        let capture_count = captured.len();
        let skip_count = skipped.len();

        // Record captures in tracing span
        tracing::Span::current().record("captures", capture_count);

        // Build response
        let response = ResponseFormatter::build_hook_response(&captured, &skipped);
        let result = serde_json::to_string(&response).map_err(|e| crate::Error::OperationFailed {
            operation: "serialize_output".to_string(),
            cause: e.to_string(),
        });

        // Record metrics
        let status = if result.is_ok() { "success" } else { "error" };
        Self::record_metrics(
            status,
            start.elapsed().as_secs_f64() * 1000.0,
            capture_count,
            skip_count,
        );

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::deduplication::{Deduplicator, DuplicateCheckResult, DuplicateReason};

    #[test]
    fn test_handler_creation() {
        let handler = PreCompactHandler::default();
        assert_eq!(handler.event_type(), "PreCompact");
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

    #[test]
    fn test_with_deduplication_builder() {
        // Mock deduplicator that always returns not duplicate
        struct MockDedup;
        impl Deduplicator for MockDedup {
            fn check_duplicate(
                &self,
                _content: &str,
                _namespace: Namespace,
            ) -> crate::Result<DuplicateCheckResult> {
                Ok(DuplicateCheckResult::not_duplicate(0))
            }
            fn record_capture(&self, _hash: &str, _memory_id: &crate::models::MemoryId) {}
        }

        let dedup = Arc::new(MockDedup);
        let handler = PreCompactHandler::new().with_deduplication(dedup);
        assert!(handler.orchestrator.has_deduplication());
    }

    #[test]
    fn test_reason_to_str() {
        assert_eq!(
            orchestrator::reason_to_str(Some(DuplicateReason::ExactMatch)),
            "exact_match"
        );
        assert_eq!(
            orchestrator::reason_to_str(Some(DuplicateReason::SemanticSimilar)),
            "semantic_similar"
        );
        assert_eq!(
            orchestrator::reason_to_str(Some(DuplicateReason::RecentCapture)),
            "recent_capture"
        );
        assert_eq!(orchestrator::reason_to_str(None), "unknown");
    }
}
