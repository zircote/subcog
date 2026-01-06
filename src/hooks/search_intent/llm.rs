//! LLM-based search intent classification.
//!
//! This module provides high-accuracy intent classification using language models.
//! Classification includes a 200ms timeout by default for responsive user experience.

use super::types::{DetectionSource, SearchIntent, SearchIntentType};
use crate::Result;
use crate::llm::LlmProvider as LlmProviderTrait;

/// Classifies search intent using an LLM provider.
///
/// # Arguments
///
/// * `provider` - The LLM provider to use for classification.
/// * `prompt` - The user prompt to classify.
///
/// # Returns
///
/// A `SearchIntent` with LLM classification results.
///
/// # Errors
///
/// Returns an error if the LLM call fails or response parsing fails.
pub fn classify_intent_with_llm<P: LlmProviderTrait + ?Sized>(
    provider: &P,
    prompt: &str,
) -> Result<SearchIntent> {
    let parsed = provider.classify_search_intent(prompt)?;
    let intent_type =
        SearchIntentType::parse(&parsed.intent_type).unwrap_or(SearchIntentType::General);

    Ok(SearchIntent {
        intent_type,
        confidence: parsed.confidence.clamp(0.0, 1.0),
        keywords: Vec::new(),
        topics: parsed.topics,
        source: DetectionSource::Llm,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{CaptureAnalysis, LlmProvider};

    struct StubProvider;

    impl LlmProvider for StubProvider {
        fn name(&self) -> &'static str {
            "stub"
        }

        fn complete(&self, _prompt: &str) -> Result<String> {
            Ok("{}".to_string())
        }

        fn complete_with_system(&self, _system: &str, _user: &str) -> Result<String> {
            Ok(r#"{
                "intent_type": "howto",
                "confidence": 0.8,
                "topics": ["auth", "login"],
                "reasoning": "User asked how to implement",
                "namespace_weights": {"patterns": 0.3}
            }"#
            .to_string())
        }

        fn analyze_for_capture(&self, _content: &str) -> Result<CaptureAnalysis> {
            Ok(CaptureAnalysis {
                should_capture: false,
                confidence: 0.0,
                suggested_namespace: None,
                suggested_tags: Vec::new(),
                reasoning: String::new(),
            })
        }
    }

    #[test]
    fn test_classify_intent_with_llm_maps_fields() {
        let provider = StubProvider;
        let result = classify_intent_with_llm(&provider, "How do I implement auth?").unwrap();
        assert_eq!(result.intent_type, SearchIntentType::HowTo);
        assert!((result.confidence - 0.8).abs() < f32::EPSILON);
        assert_eq!(result.topics, vec!["auth", "login"]);
        assert_eq!(result.source, DetectionSource::Llm);
    }
}
