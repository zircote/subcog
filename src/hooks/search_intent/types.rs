//! Types for search intent detection.
//!
//! This module contains the core types used for representing search intent:
//! - [`SearchIntentType`]: The type of search intent detected
//! - [`DetectionSource`]: How the intent was detected (keyword, LLM, or hybrid)
//! - [`SearchIntent`]: The complete intent detection result

use serde::{Deserialize, Serialize};

/// Types of search intent detected from user prompts.
///
/// Each intent type corresponds to a specific information-seeking pattern
/// and has associated namespace weights for memory retrieval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SearchIntentType {
    /// "How do I...", "How to..." - seeking implementation guidance.
    HowTo,
    /// "Where is...", "Find..." - seeking file or code location.
    Location,
    /// "What is...", "What does..." - seeking explanation.
    Explanation,
    /// "Difference between...", "X vs Y" - seeking comparison.
    Comparison,
    /// "Why is...error", "...not working" - seeking troubleshooting help.
    Troubleshoot,
    /// Generic search or unclassified intent.
    #[default]
    General,
}

impl SearchIntentType {
    /// Returns namespace weight multipliers for this intent type.
    ///
    /// Higher weights mean more memories from that namespace should be retrieved.
    #[must_use]
    pub fn namespace_weights(&self) -> Vec<(&'static str, f32)> {
        match self {
            Self::HowTo => vec![
                ("patterns", 2.0),
                ("learnings", 1.5),
                ("decisions", 1.0),
                ("context", 1.0),
            ],
            Self::Location => vec![("context", 1.5), ("patterns", 1.2), ("decisions", 1.0)],
            Self::Explanation => vec![("decisions", 1.5), ("context", 1.2), ("learnings", 1.0)],
            Self::Comparison => vec![("decisions", 2.0), ("patterns", 1.5), ("learnings", 1.0)],
            Self::Troubleshoot => vec![
                ("blockers", 2.0),
                ("learnings", 1.5),
                ("tech-debt", 1.2),
                ("patterns", 1.0),
            ],
            Self::General => vec![
                ("decisions", 1.0),
                ("patterns", 1.0),
                ("learnings", 1.0),
                ("context", 1.0),
            ],
        }
    }

    /// Parses a string into a `SearchIntentType`.
    ///
    /// Case-insensitive matching with support for common aliases.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "howto" | "how_to" | "how-to" | "implementation" => Some(Self::HowTo),
            "location" | "find" | "where" => Some(Self::Location),
            "explanation" | "explain" | "what" => Some(Self::Explanation),
            "comparison" | "compare" | "vs" => Some(Self::Comparison),
            "troubleshoot" | "debug" | "error" | "fix" => Some(Self::Troubleshoot),
            "general" | "search" | "query" => Some(Self::General),
            _ => None,
        }
    }

    /// Returns the string representation used in serialization.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::HowTo => "howto",
            Self::Location => "location",
            Self::Explanation => "explanation",
            Self::Comparison => "comparison",
            Self::Troubleshoot => "troubleshoot",
            Self::General => "general",
        }
    }
}

impl std::fmt::Display for SearchIntentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Source of intent detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DetectionSource {
    /// Detected via keyword pattern matching.
    #[default]
    Keyword,
    /// Detected via LLM classification.
    Llm,
    /// Detected via hybrid (keyword + LLM) approach.
    Hybrid,
}

impl DetectionSource {
    /// Returns the string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Keyword => "keyword",
            Self::Llm => "llm",
            Self::Hybrid => "hybrid",
        }
    }
}

impl std::fmt::Display for DetectionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Result of search intent detection.
///
/// Contains the detected intent type, confidence score, matched keywords,
/// extracted topics, and detection source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchIntent {
    /// The type of search intent detected.
    pub intent_type: SearchIntentType,
    /// Confidence score (0.0 to 1.0).
    pub confidence: f32,
    /// Keywords that triggered the detection.
    pub keywords: Vec<String>,
    /// Extracted topics from the prompt.
    pub topics: Vec<String>,
    /// How the intent was detected.
    pub source: DetectionSource,
}

impl Default for SearchIntent {
    fn default() -> Self {
        Self {
            intent_type: SearchIntentType::General,
            confidence: 0.0,
            keywords: Vec::new(),
            topics: Vec::new(),
            source: DetectionSource::Keyword,
        }
    }
}

impl SearchIntent {
    /// Creates a new `SearchIntent` with the given type.
    #[must_use]
    pub fn new(intent_type: SearchIntentType) -> Self {
        Self {
            intent_type,
            ..Default::default()
        }
    }

    /// Sets the confidence score.
    #[must_use]
    pub const fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence;
        self
    }

    /// Sets the matched keywords.
    #[must_use]
    pub fn with_keywords(mut self, keywords: Vec<String>) -> Self {
        self.keywords = keywords;
        self
    }

    /// Sets the extracted topics.
    #[must_use]
    pub fn with_topics(mut self, topics: Vec<String>) -> Self {
        self.topics = topics;
        self
    }

    /// Sets the detection source.
    #[must_use]
    pub const fn with_source(mut self, source: DetectionSource) -> Self {
        self.source = source;
        self
    }

    /// Returns whether this is a high-confidence detection (≥ 0.8).
    #[must_use]
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.8
    }

    /// Returns whether this is a medium-confidence detection (≥ 0.5).
    #[must_use]
    pub fn is_medium_confidence(&self) -> bool {
        self.confidence >= 0.5
    }

    /// Returns the recommended memory count based on confidence.
    ///
    /// - High confidence (≥ 0.8): 15 memories
    /// - Medium confidence (≥ 0.5): 10 memories
    /// - Low confidence (< 0.5): 5 memories
    #[must_use]
    pub const fn recommended_memory_count(&self) -> usize {
        if self.confidence >= 0.8 {
            15
        } else if self.confidence >= 0.5 {
            10
        } else {
            5
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intent_type_parse() {
        assert_eq!(
            SearchIntentType::parse("howto"),
            Some(SearchIntentType::HowTo)
        );
        assert_eq!(
            SearchIntentType::parse("HowTo"),
            Some(SearchIntentType::HowTo)
        );
        assert_eq!(
            SearchIntentType::parse("how_to"),
            Some(SearchIntentType::HowTo)
        );
        assert_eq!(
            SearchIntentType::parse("troubleshoot"),
            Some(SearchIntentType::Troubleshoot)
        );
        assert_eq!(SearchIntentType::parse("unknown"), None);
    }

    #[test]
    fn test_intent_type_as_str() {
        assert_eq!(SearchIntentType::HowTo.as_str(), "howto");
        assert_eq!(SearchIntentType::Location.as_str(), "location");
        assert_eq!(SearchIntentType::General.as_str(), "general");
    }

    #[test]
    fn test_detection_source_display() {
        assert_eq!(DetectionSource::Keyword.to_string(), "keyword");
        assert_eq!(DetectionSource::Llm.to_string(), "llm");
        assert_eq!(DetectionSource::Hybrid.to_string(), "hybrid");
    }

    #[test]
    fn test_search_intent_default() {
        let intent = SearchIntent::default();
        assert_eq!(intent.intent_type, SearchIntentType::General);
        assert!(intent.confidence.abs() < f32::EPSILON);
        assert!(intent.keywords.is_empty());
        assert!(intent.topics.is_empty());
        assert_eq!(intent.source, DetectionSource::Keyword);
    }

    #[test]
    fn test_search_intent_builder() {
        let intent = SearchIntent::new(SearchIntentType::HowTo)
            .with_confidence(0.85)
            .with_keywords(vec!["how".to_string()])
            .with_topics(vec!["rust".to_string()])
            .with_source(DetectionSource::Llm);

        assert_eq!(intent.intent_type, SearchIntentType::HowTo);
        assert!((intent.confidence - 0.85).abs() < f32::EPSILON);
        assert_eq!(intent.keywords, vec!["how"]);
        assert_eq!(intent.topics, vec!["rust"]);
        assert_eq!(intent.source, DetectionSource::Llm);
    }

    #[test]
    fn test_confidence_levels() {
        let high = SearchIntent::new(SearchIntentType::General).with_confidence(0.9);
        assert!(high.is_high_confidence());
        assert!(high.is_medium_confidence());
        assert_eq!(high.recommended_memory_count(), 15);

        let medium = SearchIntent::new(SearchIntentType::General).with_confidence(0.6);
        assert!(!medium.is_high_confidence());
        assert!(medium.is_medium_confidence());
        assert_eq!(medium.recommended_memory_count(), 10);

        let low = SearchIntent::new(SearchIntentType::General).with_confidence(0.3);
        assert!(!low.is_high_confidence());
        assert!(!low.is_medium_confidence());
        assert_eq!(low.recommended_memory_count(), 5);
    }

    #[test]
    fn test_namespace_weights() {
        let weights = SearchIntentType::Troubleshoot.namespace_weights();
        assert!(
            weights
                .iter()
                .any(|(ns, w)| *ns == "blockers" && (*w - 2.0).abs() < f32::EPSILON)
        );
        assert!(
            weights
                .iter()
                .any(|(ns, w)| *ns == "learnings" && (*w - 1.5).abs() < f32::EPSILON)
        );
    }
}
