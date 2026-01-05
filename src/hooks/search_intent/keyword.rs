//! Keyword-based search intent detection.
//!
//! This module provides fast, pattern-based intent detection using regex signals.
//! Detection typically completes in under 10ms.

use super::types::{DetectionSource, SearchIntent, SearchIntentType};
use crate::hooks::search_patterns::{SEARCH_SIGNALS, STOP_WORDS, SearchSignal};

/// Detects search intent from a user prompt using keyword pattern matching.
///
/// Analyzes the prompt for search signals (e.g., "how do I", "where is")
/// and extracts intent type, confidence, keywords, and topics.
///
/// # Arguments
///
/// * `prompt` - The user prompt to analyze.
///
/// # Returns
///
/// A `SearchIntent` if search signals are detected, `None` otherwise.
///
/// # Performance
///
/// Typically completes in under 10ms.
#[must_use]
pub fn detect_search_intent(prompt: &str) -> Option<SearchIntent> {
    if prompt.is_empty() {
        return None;
    }

    let prompt_lower = prompt.to_lowercase();
    let mut matched_signals: Vec<(&SearchSignal, String)> = Vec::new();

    // Check each signal pattern
    for signal in SEARCH_SIGNALS.iter() {
        if let Some(matched) = signal.pattern.find(&prompt_lower) {
            matched_signals.push((signal, matched.as_str().to_string()));
        }
    }

    if matched_signals.is_empty() {
        return None;
    }

    // Determine primary intent type by counting matches
    let intent_type = determine_primary_intent(&matched_signals);

    // Calculate confidence before consuming matched_signals
    let confidence = calculate_confidence(&matched_signals, prompt);

    // Extract keywords that triggered detection - consume matched_signals to avoid clones
    let keywords: Vec<String> = matched_signals.into_iter().map(|(_, m)| m).collect();

    // Extract topics from the prompt
    let topics = extract_topics(prompt);

    Some(SearchIntent {
        intent_type,
        confidence,
        keywords,
        topics,
        source: DetectionSource::Keyword,
    })
}

/// Determines the primary intent type from matched signals.
fn determine_primary_intent(matched_signals: &[(&SearchSignal, String)]) -> SearchIntentType {
    use std::collections::HashMap;

    let mut intent_counts: HashMap<SearchIntentType, usize> = HashMap::new();

    for (signal, _) in matched_signals {
        *intent_counts.entry(signal.intent_type).or_insert(0) += 1;
    }

    // Prioritize more specific intents over General
    let priority_order = [
        SearchIntentType::HowTo,
        SearchIntentType::Troubleshoot,
        SearchIntentType::Location,
        SearchIntentType::Explanation,
        SearchIntentType::Comparison,
        SearchIntentType::General,
    ];

    for intent in priority_order {
        if intent_counts.contains_key(&intent) {
            return intent;
        }
    }

    SearchIntentType::General
}

/// Calculates confidence score based on matched signals and prompt characteristics.
#[allow(clippy::cast_precision_loss)]
fn calculate_confidence(matched_signals: &[(&SearchSignal, String)], prompt: &str) -> f32 {
    let base_confidence: f32 = 0.5;

    // Bonus for multiple matches (max +0.15)
    let match_bonus = 0.15_f32.min(matched_signals.len() as f32 * 0.05);

    // Bonus for longer prompts (more context)
    let length_factor = if prompt.len() > 50 { 0.1 } else { 0.0 };

    // Bonus for multiple sentences (more structured query)
    let sentence_count = prompt
        .chars()
        .filter(|&c| c == '.' || c == '?' || c == '!')
        .count();
    let sentence_factor = if sentence_count > 1 { 0.1 } else { 0.0 };

    // Bonus for question marks (explicit question)
    let question_factor = if prompt.contains('?') { 0.1 } else { 0.0 };

    (base_confidence + match_bonus + length_factor + sentence_factor + question_factor).min(0.95)
}

/// Extracts topics from a prompt.
///
/// Topics are significant words that might map to memory tags or namespaces.
///
/// # Performance
///
/// Uses linear deduplication via `Vec::contains()` instead of `HashSet` since
/// we limit to 5 topics. This avoids allocating a separate `HashSet` and cloning
/// strings for both collections.
pub fn extract_topics(prompt: &str) -> Vec<String> {
    let mut topics = Vec::with_capacity(5);

    // Simple word tokenization and filtering - iterate directly without collecting
    for word in prompt.split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == ':') {
        if word.is_empty() {
            continue;
        }

        // Clean up the word
        let cleaned = word
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
            .to_lowercase();

        // Filter criteria
        if cleaned.len() < 3 {
            continue;
        }
        if STOP_WORDS.contains(cleaned.as_str()) {
            continue;
        }
        // Skip pure numbers
        if cleaned.chars().all(char::is_numeric) {
            continue;
        }
        // Deduplicate using linear search (O(n) but n <= 5)
        if topics.contains(&cleaned) {
            continue;
        }

        topics.push(cleaned);

        // Early exit once we have 5 topics
        if topics.len() >= 5 {
            break;
        }
    }

    topics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_howto_intent() {
        let result = detect_search_intent("How do I implement authentication?");
        assert!(result.is_some());
        let intent = result.unwrap();
        assert_eq!(intent.intent_type, SearchIntentType::HowTo);
        assert!(intent.confidence >= 0.5);
    }

    #[test]
    fn test_detect_troubleshoot_intent() {
        let result = detect_search_intent("Why am I getting an error in the database?");
        assert!(result.is_some());
        let intent = result.unwrap();
        assert_eq!(intent.intent_type, SearchIntentType::Troubleshoot);
    }

    #[test]
    fn test_detect_location_intent() {
        let result = detect_search_intent("Where is the configuration file?");
        assert!(result.is_some());
        let intent = result.unwrap();
        assert_eq!(intent.intent_type, SearchIntentType::Location);
    }

    #[test]
    fn test_detect_explanation_intent() {
        let result = detect_search_intent("What is the ServiceContainer?");
        assert!(result.is_some());
        let intent = result.unwrap();
        assert_eq!(intent.intent_type, SearchIntentType::Explanation);
    }

    #[test]
    fn test_detect_comparison_intent() {
        let result = detect_search_intent("What's the difference between SQLite and PostgreSQL?");
        assert!(result.is_some());
        let intent = result.unwrap();
        assert_eq!(intent.intent_type, SearchIntentType::Comparison);
    }

    #[test]
    fn test_no_intent_detected() {
        let result = detect_search_intent("Hello, world!");
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_prompt() {
        let result = detect_search_intent("");
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_topics() {
        let topics = extract_topics("How do I implement authentication with OAuth?");
        assert!(topics.contains(&"implement".to_string()));
        assert!(topics.contains(&"authentication".to_string()));
        assert!(topics.contains(&"oauth".to_string()));
        // "how", "do", "I", "with" should be filtered out
        assert!(!topics.contains(&"how".to_string()));
        assert!(!topics.contains(&"with".to_string()));
    }

    #[test]
    fn test_topics_limit() {
        let topics =
            extract_topics("one two three four five six seven eight nine ten eleven twelve");
        assert!(topics.len() <= 5);
    }

    #[test]
    fn test_confidence_increases_with_question_mark() {
        let with_question = detect_search_intent("How do I test this?").unwrap();
        let without_question = detect_search_intent("How do I test this").unwrap();
        assert!(with_question.confidence > without_question.confidence);
    }

    #[test]
    fn test_confidence_capped_at_95() {
        // Multiple signals should not exceed 0.95
        let result = detect_search_intent(
            "How do I fix this error? Where is the problem? What is the solution?",
        );
        assert!(result.is_some());
        assert!(result.unwrap().confidence <= 0.95);
    }
}
