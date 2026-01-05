//! LLM-based search intent classification.
//!
//! This module provides high-accuracy intent classification using language models.
//! Classification includes a 200ms timeout by default for responsive user experience.

use super::types::{DetectionSource, SearchIntent, SearchIntentType};
use crate::Result;
use crate::llm::LlmProvider as LlmProviderTrait;
use serde::{Deserialize, Serialize};

/// Prompt template for LLM intent classification.
const LLM_INTENT_PROMPT: &str = "Classify the search intent of the following user prompt.

USER PROMPT:
<<PROMPT>>

Respond with a JSON object containing:
- \"intent_type\": one of \"howto\", \"location\", \"explanation\", \"comparison\", \"troubleshoot\", \"general\"
- \"confidence\": a float from 0.0 to 1.0
- \"topics\": array of up to 5 relevant topic strings
- \"reasoning\": brief explanation of classification

Response (JSON only):";

/// LLM classification result structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmIntentResponse {
    pub intent_type: String,
    pub confidence: f32,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub reasoning: String,
}

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
    let classification_prompt = LLM_INTENT_PROMPT.replace("<<PROMPT>>", prompt);
    let response = provider.complete(&classification_prompt)?;
    parse_llm_response(&response)
}

/// Parses LLM response into a `SearchIntent`.
fn parse_llm_response(response: &str) -> Result<SearchIntent> {
    // Try to extract JSON from response (handle markdown code blocks)
    let json_str = extract_json_from_response(response);

    let parsed: LlmIntentResponse =
        serde_json::from_str(json_str).map_err(|e| crate::Error::OperationFailed {
            operation: "parse_llm_intent_response".to_string(),
            cause: format!("Invalid JSON: {e}"),
        })?;

    let intent_type =
        SearchIntentType::parse(&parsed.intent_type).unwrap_or(SearchIntentType::General);

    Ok(SearchIntent {
        intent_type,
        confidence: parsed.confidence.clamp(0.0, 1.0),
        keywords: Vec::new(), // LLM doesn't provide keywords
        topics: parsed.topics,
        source: DetectionSource::Llm,
    })
}

/// Extracts JSON from LLM response, handling markdown code blocks.
fn extract_json_from_response(response: &str) -> &str {
    let trimmed = response.trim();

    // Handle ```json ... ``` blocks
    if let Some((json_start, end)) = trimmed.find("```json").and_then(|start| {
        let json_start = start + 7;
        trimmed[json_start..]
            .find("```")
            .map(|end| (json_start, end))
    }) {
        return trimmed[json_start..json_start + end].trim();
    }

    // Handle ``` ... ``` blocks (without json marker)
    if let Some((json_start, end)) = trimmed.find("```").and_then(|start| {
        let content_start = start + 3;
        // Skip language identifier if present (e.g., "json\n")
        let after_marker = &trimmed[content_start..];
        let json_start = after_marker
            .find('{')
            .map_or(content_start, |pos| content_start + pos);
        trimmed[json_start..]
            .find("```")
            .map(|end| (json_start, end))
    }) {
        return trimmed[json_start..json_start + end].trim();
    }

    // Handle raw JSON (find first { to last })
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        return &trimmed[start..=end];
    }

    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_from_code_block() {
        let response = r#"```json
{"intent_type": "howto", "confidence": 0.9, "topics": ["rust"]}
```"#;
        let json = extract_json_from_response(response);
        assert!(json.starts_with('{'));
        assert!(json.contains("howto"));
    }

    #[test]
    fn test_extract_json_raw() {
        let response = r#"{"intent_type": "location", "confidence": 0.8, "topics": []}"#;
        let json = extract_json_from_response(response);
        assert_eq!(json, response);
    }

    #[test]
    fn test_extract_json_with_text_before() {
        let response = r#"Here's the classification:
{"intent_type": "troubleshoot", "confidence": 0.75, "topics": ["error"]}"#;
        let json = extract_json_from_response(response);
        assert!(json.starts_with('{'));
        assert!(json.contains("troubleshoot"));
    }

    #[test]
    fn test_parse_llm_response_valid() {
        let response = r#"{"intent_type": "howto", "confidence": 0.85, "topics": ["authentication", "oauth"]}"#;
        let result = parse_llm_response(response);
        assert!(result.is_ok());
        let intent = result.unwrap();
        assert_eq!(intent.intent_type, SearchIntentType::HowTo);
        assert!((intent.confidence - 0.85).abs() < f32::EPSILON);
        assert_eq!(intent.topics, vec!["authentication", "oauth"]);
        assert_eq!(intent.source, DetectionSource::Llm);
    }

    #[test]
    fn test_parse_llm_response_unknown_intent_defaults_to_general() {
        let response = r#"{"intent_type": "unknown_type", "confidence": 0.5, "topics": []}"#;
        let result = parse_llm_response(response);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().intent_type, SearchIntentType::General);
    }

    #[test]
    fn test_parse_llm_response_confidence_clamped() {
        let response = r#"{"intent_type": "howto", "confidence": 1.5, "topics": []}"#;
        let result = parse_llm_response(response);
        assert!(result.is_ok());
        assert!((result.unwrap().confidence - 1.0).abs() < f32::EPSILON);

        let response = r#"{"intent_type": "howto", "confidence": -0.5, "topics": []}"#;
        let result = parse_llm_response(response);
        assert!(result.is_ok());
        assert!(result.unwrap().confidence.abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_llm_response_invalid_json() {
        let response = "not valid json";
        let result = parse_llm_response(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_llm_response_missing_optional_fields() {
        let response = r#"{"intent_type": "location", "confidence": 0.7}"#;
        let result = parse_llm_response(response);
        assert!(result.is_ok());
        let intent = result.unwrap();
        assert!(intent.topics.is_empty());
    }
}
