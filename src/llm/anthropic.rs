//! Anthropic Claude client.

use super::{CaptureAnalysis, LlmHttpConfig, LlmProvider, build_http_client};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// Anthropic Claude LLM client.
pub struct AnthropicClient {
    /// API key.
    api_key: Option<String>,
    /// API endpoint.
    endpoint: String,
    /// Model to use.
    model: String,
    /// HTTP client.
    client: reqwest::blocking::Client,
}

impl AnthropicClient {
    /// Default API endpoint.
    pub const DEFAULT_ENDPOINT: &'static str = "https://api.anthropic.com/v1";

    /// Default model.
    pub const DEFAULT_MODEL: &'static str = "claude-3-haiku-20240307";

    /// Creates a new Anthropic client.
    #[must_use]
    pub fn new() -> Self {
        let api_key = std::env::var("ANTHROPIC_API_KEY").ok();
        Self {
            api_key,
            endpoint: Self::DEFAULT_ENDPOINT.to_string(),
            model: Self::DEFAULT_MODEL.to_string(),
            client: build_http_client(LlmHttpConfig::from_env()),
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

    /// Sets HTTP client timeouts for LLM requests.
    #[must_use]
    pub fn with_http_config(mut self, config: LlmHttpConfig) -> Self {
        self.client = build_http_client(config);
        self
    }

    /// Validates that the client is configured.
    fn validate(&self) -> Result<()> {
        if self.api_key.is_none() {
            return Err(Error::OperationFailed {
                operation: "anthropic_request".to_string(),
                cause: "ANTHROPIC_API_KEY not set".to_string(),
            });
        }
        Ok(())
    }

    /// Makes a request to the Anthropic API.
    fn request(&self, messages: Vec<Message>) -> Result<String> {
        self.validate()?;

        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| Error::OperationFailed {
                operation: "anthropic_request".to_string(),
                cause: "API key not configured".to_string(),
            })?;

        let request = MessagesRequest {
            model: self.model.clone(),
            max_tokens: 1024,
            messages,
        };

        let response = self
            .client
            .post(format!("{}/messages", self.endpoint))
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .map_err(|e| Error::OperationFailed {
                operation: "anthropic_request".to_string(),
                cause: e.to_string(),
            })?;

        if !response.status().is_success() {
            return Err(Error::OperationFailed {
                operation: "anthropic_request".to_string(),
                cause: format!("API returned status: {}", response.status()),
            });
        }

        let response: MessagesResponse = response.json().map_err(|e| Error::OperationFailed {
            operation: "anthropic_response".to_string(),
            cause: e.to_string(),
        })?;

        // Extract text from first content block
        response
            .content
            .first()
            .and_then(|block| {
                if block.block_type == "text" {
                    Some(block.text.clone())
                } else {
                    None
                }
            })
            .ok_or_else(|| Error::OperationFailed {
                operation: "anthropic_response".to_string(),
                cause: "No text content in response".to_string(),
            })
    }
}

impl Default for AnthropicClient {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmProvider for AnthropicClient {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn complete(&self, prompt: &str) -> Result<String> {
        let messages = vec![Message {
            role: "user".to_string(),
            content: prompt.to_string(),
        }];

        self.request(messages)
    }

    fn analyze_for_capture(&self, content: &str) -> Result<CaptureAnalysis> {
        let prompt = format!(
            r#"Analyze the following content and determine if it should be captured as a memory for an AI coding assistant.

Content:
{content}

Respond in JSON format with these fields:
- should_capture: boolean
- confidence: number from 0.0 to 1.0
- suggested_namespace: one of "decisions", "patterns", "learnings", "blockers", "tech-debt", "context"
- suggested_tags: array of relevant tags
- reasoning: brief explanation

Only the JSON, no other text."#
        );

        let response = self.complete(&prompt)?;

        // Parse JSON response
        let analysis: AnalysisResponse =
            serde_json::from_str(&response).map_err(|e| Error::OperationFailed {
                operation: "parse_analysis".to_string(),
                cause: e.to_string(),
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

/// Request to the Messages API.
#[derive(Debug, Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
}

/// A message in the conversation.
#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

/// Response from the Messages API.
#[derive(Debug, Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

/// A content block in the response.
#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    text: String,
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
        let client = AnthropicClient::new();
        assert_eq!(client.name(), "anthropic");
        assert_eq!(client.model, AnthropicClient::DEFAULT_MODEL);
    }

    #[test]
    fn test_client_configuration() {
        let client = AnthropicClient::new()
            .with_api_key("test-key")
            .with_endpoint("https://custom.endpoint")
            .with_model("claude-3-opus-20240229");

        assert_eq!(client.api_key, Some("test-key".to_string()));
        assert_eq!(client.endpoint, "https://custom.endpoint");
        assert_eq!(client.model, "claude-3-opus-20240229");
    }

    #[test]
    fn test_validate_no_key() {
        // Create client without setting env var
        let client = AnthropicClient {
            api_key: None,
            endpoint: AnthropicClient::DEFAULT_ENDPOINT.to_string(),
            model: AnthropicClient::DEFAULT_MODEL.to_string(),
            client: reqwest::blocking::Client::new(),
        };

        let result = client.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_with_key() {
        let client = AnthropicClient::new().with_api_key("test-key");
        let result = client.validate();
        assert!(result.is_ok());
    }
}
