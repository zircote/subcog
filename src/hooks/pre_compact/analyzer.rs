//! Content analysis for pre-compact hook.
//!
//! This module handles extracting capture candidates from conversation content
//! using keyword-based language detection.

use super::{FINGERPRINT_LENGTH, MIN_COMMON_CHARS_FOR_DUPLICATE};
use crate::models::Namespace;

/// Candidate for capture.
#[derive(Debug, Clone)]
pub struct CaptureCandidate {
    /// The content to capture.
    pub content: String,
    /// Detected namespace for this content.
    pub namespace: Namespace,
    /// Confidence score (0.0-1.0).
    pub confidence: f32,
}

/// Checks if text contains decision-related language.
#[must_use]
pub fn contains_decision_language(text: &str) -> bool {
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
#[must_use]
pub fn contains_learning_language(text: &str) -> bool {
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
#[must_use]
pub fn contains_blocker_language(text: &str) -> bool {
    let lower = text.to_lowercase();
    (lower.contains("fixed") || lower.contains("resolved") || lower.contains("solved"))
        && (lower.contains("issue")
            || lower.contains("bug")
            || lower.contains("error")
            || lower.contains("problem"))
}

/// Checks if text contains pattern-related language.
#[must_use]
pub fn contains_pattern_language(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("pattern")
        || lower.contains("best practice")
        || lower.contains("convention")
        || lower.contains("always ")
        || lower.contains("never ")
        || lower.contains("should always")
        || lower.contains("must ")
}

/// Checks if text contains context-related language.
///
/// Context captures explain the "why" behind decisions - constraints,
/// requirements, and important background information.
#[must_use]
pub fn contains_context_language(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("because")
        || lower.contains("constraint")
        || lower.contains("requirement")
        || lower.contains("context:")
        || lower.contains("important:")
        || lower.contains("note:")
        || lower.contains("background:")
        || lower.contains("rationale")
        || lower.contains("reason why")
        || lower.contains("due to")
}

/// Calculates confidence for a section based on heuristics.
///
/// Higher confidence for:
/// - Longer content (more complete thought)
/// - Multiple sentences
/// - Technical content (code blocks)
#[must_use]
pub fn calculate_section_confidence(section: &str) -> f32 {
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

/// Removes duplicate/similar candidates based on content fingerprints.
///
/// Keeps highest-confidence candidates when similar content is detected.
#[must_use]
pub fn deduplicate_candidates(mut candidates: Vec<CaptureCandidate>) -> Vec<CaptureCandidate> {
    // Sort by confidence descending
    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut result = Vec::new();
    let mut seen_prefixes: Vec<String> = Vec::new();

    for candidate in candidates {
        // Take first N chars as a "fingerprint"
        let prefix: String = candidate.content.chars().take(FINGERPRINT_LENGTH).collect();

        // Check if we've seen a similar prefix
        let is_duplicate = seen_prefixes.iter().any(|p| {
            let common = p
                .chars()
                .zip(prefix.chars())
                .take_while(|(a, b)| a == b)
                .count();
            common > MIN_COMMON_CHARS_FOR_DUPLICATE
        });

        if !is_duplicate {
            seen_prefixes.push(prefix);
            result.push(candidate);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_contains_context_language() {
        assert!(contains_context_language(
            "We did this because of performance requirements"
        ));
        assert!(contains_context_language("Context: the system needs X"));
        assert!(contains_context_language(
            "The constraint here is memory usage"
        ));
        assert!(contains_context_language(
            "Important: this must complete fast"
        ));
        assert!(contains_context_language("Note: this is a workaround"));
        assert!(contains_context_language(
            "Due to backwards compatibility, we chose this"
        ));
        // Text with no context language
        assert!(!contains_context_language(
            "Just some regular implementation code"
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
    fn test_deduplicate_candidates() {
        let candidates = vec![
            CaptureCandidate {
                content: "This is a test content that is quite long and should be unique"
                    .to_string(),
                namespace: Namespace::Decisions,
                confidence: 0.8,
            },
            CaptureCandidate {
                content: "This is a test content that is quite long and should match".to_string(),
                namespace: Namespace::Decisions,
                confidence: 0.7,
            },
            CaptureCandidate {
                content: "Completely different content here with no similarity".to_string(),
                namespace: Namespace::Learnings,
                confidence: 0.9,
            },
        ];

        let result = deduplicate_candidates(candidates);
        // Should keep highest confidence of similar ones + the unique one
        assert_eq!(result.len(), 2);
    }
}
