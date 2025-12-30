//! `OpenAI` client.

use super::{CaptureAnalysis, LlmProvider};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// `OpenAI` LLM client.
pub struct OpenAiClient {
    /// API key.
    api_key: Option<String>,
    /// API endpoint.
    endpoint: String,
    /// Model to use.
    model: String,
    /// HTTP client.
    client: reqwest::blocking::Client,
}

impl OpenAiClient {
    /// Default API endpoint.
    pub const DEFAULT_ENDPOINT: &'static str = "https://api.openai.com/v1";

    /// Default model.
    pub const DEFAULT_MODEL: &'static str = "gpt-4o-mini";

    /// Creates a new `OpenAI` client.
    #[must_use]
    pub fn new() -> Self {
        let api_key = std::env::var("OPENAI_API_KEY").ok();
        Self {
            api_key,
            endpoint: Self::DEFAULT_ENDPOINT.to_string(),
            model: Self::DEFAULT_MODEL.to_string(),
            client: reqwest::blocking::Client::new(),
        }
    }

    /// Sets the API key.
    #[must_use]
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Sets the API endpoint.
    #[must_use]
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Sets the model.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Validates that the client is configured.
    fn validate(&self) -> Result<()> {
        if self.api_key.is_none() {
            return Err(Error::OperationFailed {
                operation: "openai_request".to_string(),
                cause: "OPENAI_API_KEY not set".to_string(),
            });
        }
        Ok(())
    }

    /// Makes a request to the OpenAI API.
    fn request(&self, messages: Vec<ChatMessage>) -> Result<String> {
        self.validate()?;

        let api_key = self.api_key.as_ref().ok_or_else(|| Error::OperationFailed {
            operation: "openai_request".to_string(),
            cause: "API key not configured".to_string(),
        })?;

        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            max_tokens: Some(1024),
            temperature: Some(0.7),
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.endpoint))
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .map_err(|e| Error::OperationFailed {
                operation: "openai_request".to_string(),
                cause: e.to_string(),
            })?;

        if !response.status().is_success() {
            return Err(Error::OperationFailed {
                operation: "openai_request".to_string(),
                cause: format!("API returned status: {}", response.status()),
            });
        }

        let response: ChatCompletionResponse = response.json().map_err(|e| Error::OperationFailed {
            operation: "openai_response".to_string(),
            cause: e.to_string(),
        })?;

        // Extract content from first choice
        response
            .choices
            .first()
            .map(|choice| choice.message.content.clone())
            .ok_or_else(|| Error::OperationFailed {
                operation: "openai_response".to_string(),
                cause: "No choices in response".to_string(),
            })
    }
}

impl Default for OpenAiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmProvider for OpenAiClient {
    fn name(&self) -> &'static str {
        "openai"
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

        // Parse JSON response
        let analysis: AnalysisResponse = serde_json::from_str(&response).map_err(|e| {
            Error::OperationFailed {
                operation: "parse_analysis".to_string(),
                cause: e.to_string(),
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
        let client = OpenAiClient::new();
        assert_eq!(client.name(), "openai");
        assert_eq!(client.model, OpenAiClient::DEFAULT_MODEL);
    }

    #[test]
    fn test_client_configuration() {
        let client = OpenAiClient::new()
            .with_api_key("test-key")
            .with_endpoint("https://custom.endpoint")
            .with_model("gpt-4");

        assert_eq!(client.api_key, Some("test-key".to_string()));
        assert_eq!(client.endpoint, "https://custom.endpoint");
        assert_eq!(client.model, "gpt-4");
    }

    #[test]
    fn test_validate_no_key() {
        let client = OpenAiClient {
            api_key: None,
            endpoint: OpenAiClient::DEFAULT_ENDPOINT.to_string(),
            model: OpenAiClient::DEFAULT_MODEL.to_string(),
            client: reqwest::blocking::Client::new(),
        };

        let result = client.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_with_key() {
        let client = OpenAiClient::new().with_api_key("test-key");
        let result = client.validate();
        assert!(result.is_ok());
    }
}
