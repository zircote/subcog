//! Ollama (local) client.

use super::{CaptureAnalysis, LlmHttpConfig, LlmProvider, build_http_client};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// Ollama local LLM client.
pub struct OllamaClient {
    /// API endpoint.
    endpoint: String,
    /// Model to use.
    model: String,
    /// HTTP client.
    client: reqwest::blocking::Client,
}

impl OllamaClient {
    /// Default API endpoint.
    pub const DEFAULT_ENDPOINT: &'static str = "http://localhost:11434";

    /// Default model.
    pub const DEFAULT_MODEL: &'static str = "llama3.2";

    /// Creates a new Ollama client.
    #[must_use]
    pub fn new() -> Self {
        let endpoint =
            std::env::var("OLLAMA_HOST").unwrap_or_else(|_| Self::DEFAULT_ENDPOINT.to_string());
        let model =
            std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| Self::DEFAULT_MODEL.to_string());

        Self {
            endpoint,
            model,
            client: build_http_client(LlmHttpConfig::from_env()),
        }
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

    /// Checks if Ollama is available.
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.client
            .get(format!("{}/api/tags", self.endpoint))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Makes a request to the Ollama API.
    fn request(&self, prompt: &str) -> Result<String> {
        let request = GenerateRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/api/generate", self.endpoint))
            .json(&request)
            .send()
            .map_err(|e| {
                let error_kind = if e.is_timeout() {
                    "timeout"
                } else if e.is_connect() {
                    "connect"
                } else if e.is_request() {
                    "request"
                } else {
                    "unknown"
                };
                tracing::error!(
                    provider = "ollama",
                    model = %self.model,
                    error = %e,
                    error_kind = error_kind,
                    is_timeout = e.is_timeout(),
                    is_connect = e.is_connect(),
                    "LLM request failed"
                );
                Error::OperationFailed {
                    operation: "ollama_request".to_string(),
                    cause: format!("{error_kind} error: {e}"),
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            tracing::error!(
                provider = "ollama",
                model = %self.model,
                status = %status,
                body = %body,
                "LLM API returned error status"
            );
            return Err(Error::OperationFailed {
                operation: "ollama_request".to_string(),
                cause: format!("API returned status: {status} - {body}"),
            });
        }

        let response: GenerateResponse = response.json().map_err(|e| {
            tracing::error!(
                provider = "ollama",
                model = %self.model,
                error = %e,
                "Failed to parse LLM response"
            );
            Error::OperationFailed {
                operation: "ollama_response".to_string(),
                cause: e.to_string(),
            }
        })?;

        Ok(response.response)
    }

    /// Makes a chat request to the Ollama API.
    fn chat(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/api/chat", self.endpoint))
            .json(&request)
            .send()
            .map_err(|e| {
                let error_kind = if e.is_timeout() {
                    "timeout"
                } else if e.is_connect() {
                    "connect"
                } else if e.is_request() {
                    "request"
                } else {
                    "unknown"
                };
                tracing::error!(
                    provider = "ollama",
                    model = %self.model,
                    error = %e,
                    error_kind = error_kind,
                    is_timeout = e.is_timeout(),
                    is_connect = e.is_connect(),
                    "LLM chat request failed"
                );
                Error::OperationFailed {
                    operation: "ollama_chat".to_string(),
                    cause: format!("{error_kind} error: {e}"),
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            tracing::error!(
                provider = "ollama",
                model = %self.model,
                status = %status,
                body = %body,
                "LLM chat API returned error status"
            );
            return Err(Error::OperationFailed {
                operation: "ollama_chat".to_string(),
                cause: format!("API returned status: {status} - {body}"),
            });
        }

        let response: ChatResponse = response.json().map_err(|e| {
            tracing::error!(
                provider = "ollama",
                model = %self.model,
                error = %e,
                "Failed to parse LLM chat response"
            );
            Error::OperationFailed {
                operation: "ollama_chat_response".to_string(),
                cause: e.to_string(),
            }
        })?;

        Ok(response.message.content)
    }
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmProvider for OllamaClient {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn complete(&self, prompt: &str) -> Result<String> {
        self.request(prompt)
    }

    fn analyze_for_capture(&self, content: &str) -> Result<CaptureAnalysis> {
        let system_prompt = "You are an AI assistant that analyzes content to determine if it should be captured as a memory for an AI coding assistant. Always respond with valid JSON only, no other text.";

        let user_prompt = format!(
            r#"Analyze the following content and determine if it should be captured as a memory.

Content:
{content}

Respond in JSON format with these fields:
- should_capture: boolean
- confidence: number from 0.0 to 1.0
- suggested_namespace: one of "decisions", "patterns", "learnings", "blockers", "tech-debt", "context"
- suggested_tags: array of relevant tags
- reasoning: brief explanation

Only output the JSON, nothing else."#
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

        let response = self.chat(messages)?;

        // Try to extract JSON from response (may have extra text)
        let json_str = extract_json(&response).unwrap_or(&response);

        // Parse JSON response
        let analysis: AnalysisResponse =
            serde_json::from_str(json_str).map_err(|e| Error::OperationFailed {
                operation: "parse_analysis".to_string(),
                cause: format!("Failed to parse: {e} - Response was: {response}"),
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

/// Request to the Generate API.
#[derive(Debug, Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
}

/// Response from the Generate API.
#[derive(Debug, Deserialize)]
struct GenerateResponse {
    response: String,
}

/// Request to the Chat API.
#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

/// A message in the chat.
#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// Response from the Chat API.
#[derive(Debug, Deserialize)]
struct ChatResponse {
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
        let client = OllamaClient::new();
        assert_eq!(client.name(), "ollama");
    }

    #[test]
    fn test_client_configuration() {
        let client = OllamaClient::new()
            .with_endpoint("http://localhost:12345")
            .with_model("codellama");

        assert_eq!(client.endpoint, "http://localhost:12345");
        assert_eq!(client.model, "codellama");
    }

    #[test]
    fn test_extract_json() {
        let text = r#"Here's the JSON: {"key": "value"} and some more text"#;
        let json = extract_json(text);
        assert_eq!(json, Some(r#"{"key": "value"}"#));

        let clean = r#"{"key": "value"}"#;
        let json = extract_json(clean);
        assert_eq!(json, Some(r#"{"key": "value"}"#));

        let no_json = "no json here";
        let json = extract_json(no_json);
        assert!(json.is_none());
    }

    #[test]
    fn test_default_values() {
        // This test doesn't set env vars, so uses defaults
        let client = OllamaClient {
            endpoint: OllamaClient::DEFAULT_ENDPOINT.to_string(),
            model: OllamaClient::DEFAULT_MODEL.to_string(),
            client: reqwest::blocking::Client::new(),
        };

        assert_eq!(client.endpoint, "http://localhost:11434");
        assert_eq!(client.model, "llama3.2");
    }
}
