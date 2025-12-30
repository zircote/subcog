//! LM Studio client.

use super::{CaptureAnalysis, LlmProvider};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// LM Studio local LLM client.
///
/// LM Studio provides an OpenAI-compatible API on localhost.
pub struct LmStudioClient {
    /// API endpoint.
    endpoint: String,
    /// Model to use (optional, LM Studio uses loaded model).
    model: Option<String>,
    /// HTTP client.
    client: reqwest::blocking::Client,
}

impl LmStudioClient {
    /// Default API endpoint.
    pub const DEFAULT_ENDPOINT: &'static str = "http://localhost:1234/v1";

    /// Creates a new LM Studio client.
    #[must_use]
    pub fn new() -> Self {
        let endpoint = std::env::var("LMSTUDIO_ENDPOINT")
            .unwrap_or_else(|_| Self::DEFAULT_ENDPOINT.to_string());

        Self {
            endpoint,
            model: None,
            client: reqwest::blocking::Client::new(),
        }
    }

    /// Sets the API endpoint.
    #[must_use]
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Sets the model (optional).
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Checks if LM Studio is available.
    pub fn is_available(&self) -> bool {
        self.client
            .get(format!("{}/models", self.endpoint))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Makes a request to the LM Studio API.
    fn request(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let request = ChatCompletionRequest {
            model: self.model.clone().unwrap_or_else(|| "local-model".to_string()),
            messages,
            max_tokens: Some(1024),
            temperature: Some(0.7),
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.endpoint))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .map_err(|e| Error::OperationFailed {
                operation: "lmstudio_request".to_string(),
                cause: e.to_string(),
            })?;

        if !response.status().is_success() {
            return Err(Error::OperationFailed {
                operation: "lmstudio_request".to_string(),
                cause: format!("API returned status: {}", response.status()),
            });
        }

        let response: ChatCompletionResponse = response.json().map_err(|e| Error::OperationFailed {
            operation: "lmstudio_response".to_string(),
            cause: e.to_string(),
        })?;

        // Extract content from first choice
        response
            .choices
            .first()
            .map(|choice| choice.message.content.clone())
            .ok_or_else(|| Error::OperationFailed {
                operation: "lmstudio_response".to_string(),
                cause: "No choices in response".to_string(),
            })
    }
}

impl Default for LmStudioClient {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmProvider for LmStudioClient {
    fn name(&self) -> &'static str {
        "lmstudio"
    }

    fn complete(&self, prompt: &str) -> Result<String> {
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        }];

        self.request(messages)
    }

    fn analyze_for_capture(&self, content: &str) -> Result<CaptureAnalysis> {
        let system_prompt = r#"You are an AI assistant that analyzes content to determine if it should be captured as a memory for an AI coding assistant. Respond only with valid JSON."#;

        let user_prompt = format!(
            r#"Analyze the following content and determine if it should be captured as a memory.

Content:
{}

Respond in JSON format with these fields:
- should_capture: boolean
- confidence: number from 0.0 to 1.0
- suggested_namespace: one of "decisions", "patterns", "learnings", "blockers", "tech-debt", "context"
- suggested_tags: array of relevant tags
- reasoning: brief explanation"#,
            content
        );

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_prompt,
            },
        ];

        let response = self.request(messages)?;

        // Try to extract JSON from response
        let json_str = extract_json(&response).unwrap_or(&response);

        // Parse JSON response
        let analysis: AnalysisResponse = serde_json::from_str(json_str).map_err(|e| {
            Error::OperationFailed {
                operation: "parse_analysis".to_string(),
                cause: format!("Failed to parse: {} - Response was: {}", e, response),
            }
        })?;

        Ok(CaptureAnalysis {
            should_capture: analysis.should_capture,
            confidence: analysis.confidence,
            suggested_namespace: Some(analysis.suggested_namespace),
            suggested_tags: analysis.suggested_tags,
            reasoning: analysis.reasoning,
        })
    }
}

/// Extracts JSON from a response that may contain extra text.
fn extract_json(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end >= start {
        Some(&text[start..=end])
    } else {
        None
    }
}

/// Request to the Chat Completions API.
#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

/// A message in the chat.
#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// Response from the Chat Completions API.
#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

/// A choice in the response.
#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

/// Parsed analysis response.
#[derive(Debug, Deserialize)]
struct AnalysisResponse {
    should_capture: bool,
    confidence: f32,
    suggested_namespace: String,
    suggested_tags: Vec<String>,
    reasoning: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = LmStudioClient::new();
        assert_eq!(client.name(), "lmstudio");
    }

    #[test]
    fn test_client_configuration() {
        let client = LmStudioClient::new()
            .with_endpoint("http://localhost:5000/v1")
            .with_model("my-model");

        assert_eq!(client.endpoint, "http://localhost:5000/v1");
        assert_eq!(client.model, Some("my-model".to_string()));
    }

    #[test]
    fn test_extract_json() {
        let text = "Here's the JSON: {\"key\": \"value\"} and some more text";
        let json = extract_json(text);
        assert_eq!(json, Some("{\"key\": \"value\"}"));
    }

    #[test]
    fn test_default_endpoint() {
        let client = LmStudioClient {
            endpoint: LmStudioClient::DEFAULT_ENDPOINT.to_string(),
            model: None,
            client: reqwest::blocking::Client::new(),
        };

        assert_eq!(client.endpoint, "http://localhost:1234/v1");
    }
}
