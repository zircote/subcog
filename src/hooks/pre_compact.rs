//! Pre-compact hook handler.
//!
//! Analyzes content being compacted and auto-captures important memories.
//! Integrates with `DeduplicationService` to avoid capturing duplicate memories.

use crate::Result;
use crate::hooks::HookHandler;
use crate::models::{CaptureRequest, Domain, MemoryId, Namespace};
use crate::services::CaptureService;
use crate::services::deduplication::{Deduplicator, DuplicateReason};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::instrument;

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
pub struct PreCompactHandler {
    /// Capture service instance.
    capture: Option<CaptureService>,
    /// Deduplication service instance (trait object for flexibility).
    dedup: Option<Arc<dyn Deduplicator>>,
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

/// A candidate that was skipped due to duplication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedDuplicate {
    /// The reason it was skipped.
    pub reason: String,
    /// URN of the existing memory it matched.
    pub matched_urn: String,
    /// Similarity score (for semantic matches).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub similarity_score: Option<f32>,
    /// Namespace of the candidate.
    pub namespace: String,
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
        Self {
            capture: None,
            dedup: None,
        }
    }

    /// Sets the capture service.
    #[must_use]
    pub fn with_capture(mut self, capture: CaptureService) -> Self {
        self.capture = Some(capture);
        self
    }

    /// Sets the deduplication service.
    ///
    /// When set, candidates are checked for duplicates before capture.
    /// Duplicates are skipped and reported in the hook output.
    #[must_use]
    pub fn with_deduplication(mut self, dedup: Arc<dyn Deduplicator>) -> Self {
        self.dedup = Some(dedup);
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

    /// Checks if a candidate is a duplicate and returns skip info if so.
    ///
    /// Returns `Some(SkippedDuplicate)` if the candidate should be skipped,
    /// `None` if it should be captured.
    fn check_for_duplicate(&self, candidate: &CaptureCandidate) -> Option<SkippedDuplicate> {
        let dedup = self.dedup.as_ref()?;

        match dedup.check_duplicate(&candidate.content, candidate.namespace) {
            Ok(result) if result.is_duplicate => {
                let reason_str = Self::reason_to_str(result.reason);
                let matched_urn = result.matched_urn.unwrap_or_default();

                tracing::debug!(
                    namespace = %candidate.namespace.as_str(),
                    matched_urn = %matched_urn,
                    reason = reason_str,
                    "Skipping duplicate candidate"
                );

                metrics::counter!(
                    "hook_deduplication_skipped_total",
                    "hook_type" => "PreCompact",
                    "namespace" => candidate.namespace.as_str().to_string(),
                    "reason" => reason_str.to_string()
                )
                .increment(1);

                Some(SkippedDuplicate {
                    reason: reason_str.to_string(),
                    matched_urn,
                    similarity_score: result.similarity_score,
                    namespace: candidate.namespace.as_str().to_string(),
                })
            },
            Ok(_) => None, // Not a duplicate
            Err(e) => {
                // Graceful degradation: log error and proceed with capture
                tracing::warn!(
                    error = %e,
                    namespace = %candidate.namespace.as_str(),
                    "Deduplication check failed, proceeding with capture"
                );
                None
            },
        }
    }

    /// Converts a `DuplicateReason` to a string.
    fn reason_to_str(reason: Option<DuplicateReason>) -> &'static str {
        reason.map_or("unknown", |r| match r {
            DuplicateReason::ExactMatch => "exact_match",
            DuplicateReason::SemanticSimilar => "semantic_similar",
            DuplicateReason::RecentCapture => "recent_capture",
        })
    }

    /// Records a successful capture in the deduplication service.
    fn record_capture_for_dedup(&self, content: &str, memory_id: &MemoryId) {
        if let Some(dedup) = &self.dedup {
            let hash = crate::services::deduplication::ContentHasher::hash(content);
            dedup.record_capture(&hash, memory_id);
        }
    }

    /// Performs the actual capture of candidates.
    ///
    /// If a deduplication service is configured, checks each candidate for
    /// duplicates before capture. Returns both captured memories and skipped duplicates.
    fn capture_candidates(
        &self,
        candidates: Vec<CaptureCandidate>,
    ) -> (Vec<CapturedMemory>, Vec<SkippedDuplicate>) {
        let Some(capture) = &self.capture else {
            return (Vec::new(), Vec::new());
        };

        let mut captured = Vec::new();
        let mut skipped = Vec::new();

        for candidate in candidates {
            if candidate.confidence < 0.6 {
                continue;
            }

            // Check for duplicates
            if let Some(skip_info) = self.check_for_duplicate(&candidate) {
                skipped.push(skip_info);
                continue;
            }

            // Capture the candidate
            let request = CaptureRequest {
                content: candidate.content.clone(),
                namespace: candidate.namespace,
                domain: Domain::default(),
                tags: vec!["auto-captured".to_string(), "pre-compact".to_string()],
                source: Some("PreCompactHandler".to_string()),
                skip_security_check: false,
            };

            if let Ok(result) = capture.capture(request.clone()) {
                self.record_capture_for_dedup(&request.content, &result.memory_id);

                captured.push(CapturedMemory {
                    memory_id: result.memory_id.to_string(),
                    namespace: candidate.namespace.as_str().to_string(),
                    confidence: candidate.confidence,
                });
            }
            // Errors are silently ignored, continue with other candidates
        }

        (captured, skipped)
    }

    /// Builds the human-readable context message for the hook response.
    fn build_context_message(
        captured: &[CapturedMemory],
        skipped: &[SkippedDuplicate],
    ) -> Option<String> {
        if captured.is_empty() && skipped.is_empty() {
            return None;
        }

        let mut lines = vec!["**Subcog Pre-Compact Auto-Capture**\n".to_string()];

        if !captured.is_empty() {
            lines.push(format!(
                "Captured {} memories before context compaction:\n",
                captured.len()
            ));
            for c in captured {
                lines.push(format!(
                    "- `{}`: {} (confidence: {:.0}%)",
                    c.namespace,
                    c.memory_id,
                    c.confidence * 100.0
                ));
            }
        }

        if !skipped.is_empty() {
            if !captured.is_empty() {
                lines.push(String::new()); // blank line
            }
            lines.push(format!("Skipped {} duplicates:\n", skipped.len()));
            for s in skipped {
                let score_str = s
                    .similarity_score
                    .map_or(String::new(), |sc| format!(" ({:.0}% similar)", sc * 100.0));
                lines.push(format!(
                    "- `{}`: {} ({}{})",
                    s.namespace, s.matched_urn, s.reason, score_str
                ));
            }
        }

        Some(lines.join("\n"))
    }

    /// Builds the Claude Code hook response JSON.
    fn build_hook_response(
        captured: &[CapturedMemory],
        skipped: &[SkippedDuplicate],
    ) -> serde_json::Value {
        let metadata = serde_json::json!({
            "captured": !captured.is_empty(),
            "captures": captured.iter().map(|c| serde_json::json!({
                "memory_id": c.memory_id,
                "namespace": c.namespace,
                "confidence": c.confidence
            })).collect::<Vec<_>>(),
            "skipped_duplicates": skipped.len(),
            "duplicates": skipped.iter().map(|s| serde_json::json!({
                "reason": s.reason,
                "matched_urn": s.matched_urn,
                "namespace": s.namespace,
                "similarity_score": s.similarity_score
            })).collect::<Vec<_>>()
        });

        Self::build_context_message(captured, skipped).map_or_else(
            || serde_json::json!({}),
            |ctx| {
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
        )
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

        // Parse input
        let parsed: PreCompactInput =
            serde_json::from_str(input).unwrap_or_else(|_| PreCompactInput {
                context: input.to_string(),
                ..Default::default()
            });

        // Analyze content for capture candidates
        let candidates = self.analyze_content(&parsed);

        // Capture the candidates (with deduplication if configured)
        let (captured, skipped) = self.capture_candidates(candidates);
        let capture_count = captured.len();
        let skip_count = skipped.len();

        // Record captures in tracing span
        tracing::Span::current().record("captures", capture_count);

        // Build response
        let response = Self::build_hook_response(&captured, &skipped);
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

    #[test]
    fn test_with_deduplication_builder() {
        use crate::services::deduplication::{Deduplicator, DuplicateCheckResult};

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
            fn record_capture(&self, _hash: &str, _memory_id: &MemoryId) {}
        }

        let dedup = Arc::new(MockDedup);
        let handler = PreCompactHandler::new().with_deduplication(dedup);
        assert!(handler.dedup.is_some());
    }

    #[test]
    fn test_check_for_duplicate_skips() {
        use crate::services::deduplication::{Deduplicator, DuplicateCheckResult};

        // Mock deduplicator that always returns duplicate
        struct MockDedupAlwaysDup;
        impl Deduplicator for MockDedupAlwaysDup {
            fn check_duplicate(
                &self,
                _content: &str,
                _namespace: Namespace,
            ) -> crate::Result<DuplicateCheckResult> {
                Ok(DuplicateCheckResult::exact_match(
                    MemoryId::new("123"),
                    "subcog://test/decisions/123".to_string(),
                    0,
                ))
            }
            fn record_capture(&self, _hash: &str, _memory_id: &MemoryId) {}
        }

        let dedup = Arc::new(MockDedupAlwaysDup);
        let handler = PreCompactHandler::new().with_deduplication(dedup);

        let candidate = CaptureCandidate {
            content: "Test content".to_string(),
            namespace: Namespace::Decisions,
            confidence: 0.8,
        };

        let result = handler.check_for_duplicate(&candidate);
        assert!(result.is_some());
        let skip = result.unwrap();
        assert_eq!(skip.reason, "exact_match");
        assert_eq!(skip.matched_urn, "subcog://test/decisions/123");
    }

    #[test]
    fn test_check_for_duplicate_passes() {
        use crate::services::deduplication::{Deduplicator, DuplicateCheckResult};

        // Mock deduplicator that returns not duplicate
        struct MockDedupNoDup;
        impl Deduplicator for MockDedupNoDup {
            fn check_duplicate(
                &self,
                _content: &str,
                _namespace: Namespace,
            ) -> crate::Result<DuplicateCheckResult> {
                Ok(DuplicateCheckResult::not_duplicate(0))
            }
            fn record_capture(&self, _hash: &str, _memory_id: &MemoryId) {}
        }

        let dedup = Arc::new(MockDedupNoDup);
        let handler = PreCompactHandler::new().with_deduplication(dedup);

        let candidate = CaptureCandidate {
            content: "Test content".to_string(),
            namespace: Namespace::Decisions,
            confidence: 0.8,
        };

        let result = handler.check_for_duplicate(&candidate);
        assert!(result.is_none());
    }

    #[test]
    fn test_check_for_duplicate_graceful_degradation() {
        use crate::services::deduplication::{Deduplicator, DuplicateCheckResult};

        // Mock deduplicator that returns an error
        struct MockDedupError;
        impl Deduplicator for MockDedupError {
            fn check_duplicate(
                &self,
                _content: &str,
                _namespace: Namespace,
            ) -> crate::Result<DuplicateCheckResult> {
                Err(crate::Error::OperationFailed {
                    operation: "test".to_string(),
                    cause: "simulated error".to_string(),
                })
            }
            fn record_capture(&self, _hash: &str, _memory_id: &MemoryId) {}
        }

        let dedup = Arc::new(MockDedupError);
        let handler = PreCompactHandler::new().with_deduplication(dedup);

        let candidate = CaptureCandidate {
            content: "Test content".to_string(),
            namespace: Namespace::Decisions,
            confidence: 0.8,
        };

        // Error should result in None (proceed with capture)
        let result = handler.check_for_duplicate(&candidate);
        assert!(result.is_none());
    }

    #[test]
    fn test_build_context_message_empty() {
        let captured: Vec<CapturedMemory> = vec![];
        let skipped: Vec<SkippedDuplicate> = vec![];

        let result = PreCompactHandler::build_context_message(&captured, &skipped);
        assert!(result.is_none());
    }

    #[test]
    fn test_build_context_message_with_captures() {
        let captured = vec![CapturedMemory {
            memory_id: "mem-123".to_string(),
            namespace: "decisions".to_string(),
            confidence: 0.85,
        }];
        let skipped: Vec<SkippedDuplicate> = vec![];

        let result = PreCompactHandler::build_context_message(&captured, &skipped);
        assert!(result.is_some());
        let msg = result.unwrap();
        assert!(msg.contains("Captured 1 memories"));
        assert!(msg.contains("mem-123"));
        assert!(msg.contains("85%"));
    }

    #[test]
    fn test_build_context_message_with_skipped() {
        let captured: Vec<CapturedMemory> = vec![];
        let skipped = vec![SkippedDuplicate {
            reason: "exact_match".to_string(),
            matched_urn: "subcog://test/decisions/456".to_string(),
            similarity_score: None,
            namespace: "decisions".to_string(),
        }];

        let result = PreCompactHandler::build_context_message(&captured, &skipped);
        assert!(result.is_some());
        let msg = result.unwrap();
        assert!(msg.contains("Skipped 1 duplicates"));
        assert!(msg.contains("exact_match"));
        assert!(msg.contains("subcog://test/decisions/456"));
    }

    #[test]
    fn test_build_hook_response_empty() {
        let captured: Vec<CapturedMemory> = vec![];
        let skipped: Vec<SkippedDuplicate> = vec![];

        let response = PreCompactHandler::build_hook_response(&captured, &skipped);
        assert!(response.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_build_hook_response_with_data() {
        let captured = vec![CapturedMemory {
            memory_id: "mem-789".to_string(),
            namespace: "learnings".to_string(),
            confidence: 0.9,
        }];
        let skipped: Vec<SkippedDuplicate> = vec![];

        let response = PreCompactHandler::build_hook_response(&captured, &skipped);
        assert!(response.get("hookSpecificOutput").is_some());
        let hook_output = response.get("hookSpecificOutput").unwrap();
        assert_eq!(
            hook_output.get("hookEventName").unwrap().as_str().unwrap(),
            "PreCompact"
        );
        assert!(hook_output.get("additionalContext").is_some());
    }

    #[test]
    fn test_reason_to_str() {
        assert_eq!(
            PreCompactHandler::reason_to_str(Some(DuplicateReason::ExactMatch)),
            "exact_match"
        );
        assert_eq!(
            PreCompactHandler::reason_to_str(Some(DuplicateReason::SemanticSimilar)),
            "semantic_similar"
        );
        assert_eq!(
            PreCompactHandler::reason_to_str(Some(DuplicateReason::RecentCapture)),
            "recent_capture"
        );
        assert_eq!(PreCompactHandler::reason_to_str(None), "unknown");
    }
}
