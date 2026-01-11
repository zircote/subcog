//! LLM client abstraction (DOC-H3).
//!
//! Provides a unified interface for different LLM providers including
//! Anthropic Claude, `OpenAI` GPT, Ollama (local), and LM Studio (local).
//!
//! # Supported Providers
//!
//! | Provider | Client | Environment Variables |
//! |----------|--------|----------------------|
//! | Anthropic | [`AnthropicClient`] | `ANTHROPIC_API_KEY` |
//! | `OpenAI` | [`OpenAiClient`] | `OPENAI_API_KEY` |
//! | Ollama | [`OllamaClient`] | `OLLAMA_HOST`, `OLLAMA_MODEL` |
//! | LM Studio | [`LmStudioClient`] | `LMSTUDIO_ENDPOINT`, `LMSTUDIO_MODEL` |
//!
//! # Usage Examples
//!
//! ## Basic Completion
//!
//! ```rust,ignore
//! use subcog::llm::{LlmProvider, AnthropicClient};
//!
//! let client = AnthropicClient::new();
//! let response = client.complete("Explain Rust ownership in one sentence")?;
//! println!("{response}");
//! ```
//!
//! ## With System Prompt
//!
//! ```rust,ignore
//! use subcog::llm::{LlmProvider, OpenAiClient};
//!
//! let client = OpenAiClient::new();
//! let response = client.complete_with_system(
//!     "You are a helpful coding assistant.",
//!     "How do I parse JSON in Rust?"
//! )?;
//! ```
//!
//! ## Capture Analysis
//!
//! ```rust,ignore
//! use subcog::llm::{LlmProvider, OllamaClient};
//!
//! let client = OllamaClient::new();
//! let analysis = client.analyze_for_capture(
//!     "Decision: Use PostgreSQL for the primary database due to JSONB support"
//! )?;
//!
//! if analysis.should_capture && analysis.confidence > 0.8 {
//!     println!("Suggested namespace: {:?}", analysis.suggested_namespace);
//!     println!("Suggested tags: {:?}", analysis.suggested_tags);
//! }
//! ```
//!
//! ## Resilient Provider with Circuit Breaker
//!
//! ```rust,ignore
//! use subcog::llm::{LlmProvider, AnthropicClient, ResilientLlmProvider, LlmResilienceConfig};
//!
//! let base_client = AnthropicClient::new();
//! let resilient = ResilientLlmProvider::new(
//!     Box::new(base_client),
//!     LlmResilienceConfig::default()
//! );
//!
//! // Automatically retries with exponential backoff and circuit breaker
//! let response = resilient.complete("Hello world")?;
//! ```
//!
//! # Configuration
//!
//! Providers can be configured via environment variables or the config file:
//!
//! ```toml
//! [llm]
//! provider = "anthropic"  # or "openai", "ollama", "lmstudio"
//! model = "claude-sonnet-4-20250514"
//! timeout_ms = 30000
//! max_retries = 3
//! ```
//!
//! # Implementing a New Provider
//!
//! To add a new LLM provider:
//!
//! 1. Create a new module (e.g., `src/llm/newprovider.rs`)
//! 2. Implement the [`LlmProvider`] trait
//! 3. Export the client from this module
//!
//! ## Required Trait Methods
//!
//! | Method | Purpose |
//! |--------|---------|
//! | [`LlmProvider::name`] | Return provider identifier (e.g., "anthropic") |
//! | [`LlmProvider::complete`] | Generate completion for a prompt |
//! | [`LlmProvider::analyze_for_capture`] | Analyze content for memory capture |
//!
//! ## Optional Methods (with defaults)
//!
//! | Method | Default Behavior |
//! |--------|------------------|
//! | `complete_with_system` | Concatenates system and user prompts |
//! | `analyze_for_capture_extended` | Uses unified system prompt |
//! | `classify_search_intent` | Uses unified system prompt |
//! | `analyze_for_consolidation` | Uses unified system prompt |
//!
//! ## Example Implementation
//!
//! ```rust,ignore
//! use subcog::llm::{LlmProvider, CaptureAnalysis};
//! use subcog::Result;
//!
//! pub struct MyProvider {
//!     api_key: String,
//!     model: String,
//! }
//!
//! impl LlmProvider for MyProvider {
//!     fn name(&self) -> &'static str {
//!         "myprovider"
//!     }
//!
//!     fn complete(&self, prompt: &str) -> Result<String> {
//!         // Make API call to your provider
//!         todo!()
//!     }
//!
//!     fn analyze_for_capture(&self, content: &str) -> Result<CaptureAnalysis> {
//!         // Use CAPTURE_ANALYSIS_PROMPT or custom prompt
//!         let prompt = format!(
//!             "{}\n\nContent: {content}",
//!             subcog::llm::CAPTURE_ANALYSIS_PROMPT
//!         );
//!         let response = self.complete(&prompt)?;
//!         // Parse JSON response into CaptureAnalysis
//!         todo!()
//!     }
//! }
//! ```
//!
//! ## HTTP Client Guidelines
//!
//! Use [`build_http_client`] with [`LlmHttpConfig`] for consistent timeout handling:
//!
//! ```rust,ignore
//! use subcog::llm::{build_http_client, LlmHttpConfig};
//!
//! let config = LlmHttpConfig::from_env();
//! let client = build_http_client(config);
//! ```

mod anthropic;
mod lmstudio;
mod ollama;
mod openai;
mod resilience;
pub mod system_prompt;

pub use anthropic::AnthropicClient;
pub use lmstudio::LmStudioClient;
pub use ollama::OllamaClient;
pub use openai::OpenAiClient;
pub use resilience::{LlmResilienceConfig, ResilientLlmProvider};
pub use system_prompt::{
    ArchiveCandidate, BASE_SYSTEM_PROMPT, CAPTURE_ANALYSIS_PROMPT, CONSOLIDATION_PROMPT,
    ConsolidationAnalysis, ContradictionAssessment, ContradictionDetail, ENRICHMENT_PROMPT,
    ExtendedCaptureAnalysis, ExtendedSearchIntent, MEMORY_SUMMARIZATION_PROMPT, MergeCandidate,
    OperationMode, SEARCH_INTENT_PROMPT, SecurityAssessment, build_system_prompt,
    build_system_prompt_with_config,
};

use crate::Result;
use crate::security::{ContentRedactor, RedactionConfig};
use std::sync::LazyLock;
use std::time::Duration;

/// Trait for LLM providers.
pub trait LlmProvider: Send + Sync {
    /// The provider name.
    fn name(&self) -> &'static str;

    /// Generates a completion for the given prompt.
    ///
    /// # Errors
    ///
    /// Returns an error if the completion fails.
    fn complete(&self, prompt: &str) -> Result<String>;

    /// Generates a completion with a system prompt.
    ///
    /// # Errors
    ///
    /// Returns an error if the completion fails.
    ///
    /// Default implementation concatenates system and user prompts.
    /// Providers should override this to use native system prompt support.
    fn complete_with_system(&self, system: &str, user: &str) -> Result<String> {
        let combined = format!("{system}\n\n---\n\nUser message:\n{user}");
        self.complete(&combined)
    }

    /// Analyzes content for memory capture.
    ///
    /// # Errors
    ///
    /// Returns an error if analysis fails.
    fn analyze_for_capture(&self, content: &str) -> Result<CaptureAnalysis>;

    /// Analyzes content for memory capture with extended security analysis.
    ///
    /// Uses the unified subcog system prompt for comprehensive analysis
    /// including adversarial detection and contradiction checking.
    ///
    /// # Arguments
    ///
    /// * `content` - The content to analyze.
    /// * `existing_memories` - Optional context of existing memories for contradiction detection.
    ///
    /// # Errors
    ///
    /// Returns an error if analysis fails.
    fn analyze_for_capture_extended(
        &self,
        content: &str,
        existing_memories: Option<&str>,
    ) -> Result<ExtendedCaptureAnalysis> {
        let system = build_system_prompt(OperationMode::CaptureAnalysis, existing_memories);
        let user = format!("Analyze this content for capture:\n\n{content}");
        let response = self.complete_with_system(&system, &user)?;
        parse_extended_capture_analysis(&response)
    }

    /// Classifies search intent with namespace weights.
    ///
    /// Uses the unified subcog system prompt for intent classification
    /// with enhanced topic extraction and namespace weighting.
    ///
    /// # Errors
    ///
    /// Returns an error if classification fails.
    fn classify_search_intent(&self, prompt: &str) -> Result<ExtendedSearchIntent> {
        let system = build_system_prompt(OperationMode::SearchIntent, None);
        let user = format!("Classify the search intent of this prompt:\n\n{prompt}");
        let response = self.complete_with_system(&system, &user)?;
        parse_extended_search_intent(&response)
    }

    /// Analyzes memories for consolidation.
    ///
    /// Uses the unified subcog system prompt to identify merge candidates,
    /// archive candidates, and contradictions.
    ///
    /// # Arguments
    ///
    /// * `memories` - JSON array of memories to analyze.
    ///
    /// # Errors
    ///
    /// Returns an error if analysis fails.
    fn analyze_for_consolidation(&self, memories: &str) -> Result<ConsolidationAnalysis> {
        let system = build_system_prompt(OperationMode::Consolidation, None);
        let user = format!("Analyze these memories for consolidation:\n\n{memories}");
        let response = self.complete_with_system(&system, &user)?;
        parse_consolidation_analysis(&response)
    }
}

/// Analysis result for content capture.
#[derive(Debug, Clone)]
pub struct CaptureAnalysis {
    /// Whether the content should be captured.
    pub should_capture: bool,
    /// Confidence score (0.0 to 1.0).
    pub confidence: f32,
    /// Suggested namespace.
    pub suggested_namespace: Option<String>,
    /// Suggested tags.
    pub suggested_tags: Vec<String>,
    /// Reasoning for the decision.
    pub reasoning: String,
}

/// HTTP client configuration for LLM providers.
#[derive(Debug, Clone, Copy)]
pub struct LlmHttpConfig {
    /// Request timeout in milliseconds (0 to disable).
    pub timeout_ms: u64,
    /// Connect timeout in milliseconds (0 to disable).
    pub connect_timeout_ms: u64,
}

const MAX_LLM_ERROR_RESPONSE_CHARS: usize = 200;
static LLM_ERROR_REDACTOR: LazyLock<ContentRedactor> =
    LazyLock::new(|| ContentRedactor::with_config(RedactionConfig::new().with_pii()));

impl Default for LlmHttpConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,
            connect_timeout_ms: 3_000,
        }
    }
}

impl LlmHttpConfig {
    /// Loads HTTP configuration from environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        Self::default().with_env_overrides()
    }

    /// Loads HTTP configuration from config file settings.
    #[must_use]
    pub fn from_config(config: &crate::config::LlmConfig) -> Self {
        let mut settings = Self::default();
        if let Some(timeout_ms) = config.timeout_ms {
            settings.timeout_ms = timeout_ms;
        }
        if let Some(connect_timeout_ms) = config.connect_timeout_ms {
            settings.connect_timeout_ms = connect_timeout_ms;
        }
        settings
    }

    /// Applies environment variable overrides.
    #[must_use]
    pub fn with_env_overrides(mut self) -> Self {
        if let Some(timeout_ms) = std::env::var("SUBCOG_LLM_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            self.timeout_ms = timeout_ms;
        }
        if let Some(connect_timeout_ms) = std::env::var("SUBCOG_LLM_CONNECT_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            self.connect_timeout_ms = connect_timeout_ms;
        }
        self
    }
}

/// Builds a blocking HTTP client for LLM requests with configured timeouts.
#[must_use]
pub fn build_http_client(config: LlmHttpConfig) -> reqwest::blocking::Client {
    let mut builder = reqwest::blocking::Client::builder();
    if config.timeout_ms > 0 {
        builder = builder.timeout(Duration::from_millis(config.timeout_ms));
    }
    if config.connect_timeout_ms > 0 {
        builder = builder.connect_timeout(Duration::from_millis(config.connect_timeout_ms));
    }

    builder.build().unwrap_or_else(|err| {
        tracing::warn!("Failed to build LLM HTTP client: {err}");
        let mut fallback = reqwest::blocking::Client::builder();
        if config.timeout_ms > 0 {
            fallback = fallback.timeout(Duration::from_millis(config.timeout_ms));
        }
        if config.connect_timeout_ms > 0 {
            fallback = fallback.connect_timeout(Duration::from_millis(config.connect_timeout_ms));
        }
        fallback.build().unwrap_or_else(|fallback_err| {
            tracing::warn!("Failed to build LLM HTTP fallback client: {fallback_err}");
            reqwest::blocking::Client::new()
        })
    })
}

pub(crate) fn sanitize_llm_response_for_error(response: &str) -> String {
    let redacted = LLM_ERROR_REDACTOR.redact(response);
    if redacted.chars().count() > MAX_LLM_ERROR_RESPONSE_CHARS {
        let truncated: String = redacted
            .chars()
            .take(MAX_LLM_ERROR_RESPONSE_CHARS)
            .collect();
        format!("{truncated}...(truncated)")
    } else {
        redacted
    }
}

/// Parses an extended capture analysis response from LLM output.
///
/// Handles various JSON formats and extracts from markdown code blocks.
fn parse_extended_capture_analysis(response: &str) -> Result<ExtendedCaptureAnalysis> {
    let json_str = extract_json_from_response(response);
    let sanitized = sanitize_llm_response_for_error(response);
    serde_json::from_str(json_str).map_err(|e| crate::Error::OperationFailed {
        operation: "parse_extended_capture_analysis".to_string(),
        cause: format!("Invalid JSON: {e}. Response: {sanitized}"),
    })
}

/// Parses an extended search intent response from LLM output.
fn parse_extended_search_intent(response: &str) -> Result<ExtendedSearchIntent> {
    let json_str = extract_json_from_response(response);
    let sanitized = sanitize_llm_response_for_error(response);
    serde_json::from_str(json_str).map_err(|e| crate::Error::OperationFailed {
        operation: "parse_extended_search_intent".to_string(),
        cause: format!("Invalid JSON: {e}. Response: {sanitized}"),
    })
}

/// Parses a consolidation analysis response from LLM output.
fn parse_consolidation_analysis(response: &str) -> Result<ConsolidationAnalysis> {
    let json_str = extract_json_from_response(response);
    let sanitized = sanitize_llm_response_for_error(response);
    serde_json::from_str(json_str).map_err(|e| crate::Error::OperationFailed {
        operation: "parse_consolidation_analysis".to_string(),
        cause: format!("Invalid JSON: {e}. Response: {sanitized}"),
    })
}

/// Extracts JSON from LLM response, handling markdown code blocks (CQ-H2).
///
/// This is a centralized utility for extracting JSON from LLM responses that may
/// include markdown formatting, prose, or other surrounding text.
///
/// # Handling
///
/// 1. Markdown code blocks with `json` language marker
/// 2. Markdown code blocks without language marker
/// 3. Raw JSON objects (first `{` to last `}`)
/// 4. JSON arrays (first `[` to last `]`)
///
/// # Arguments
///
/// * `response` - The raw LLM response text
///
/// # Returns
///
/// The extracted JSON string, or the trimmed input if no JSON found
#[must_use]
pub fn extract_json_from_response(response: &str) -> &str {
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

    // Handle JSON array (for enrichment responses)
    if let (Some(start), Some(end)) = (trimmed.find('['), trimmed.rfind(']')) {
        return &trimmed[start..=end];
    }

    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_raw() {
        let response = r#"{"key": "value"}"#;
        let json = extract_json_from_response(response);
        assert_eq!(json, r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_markdown() {
        let response = "```json\n{\"key\": \"value\"}\n```";
        let json = extract_json_from_response(response);
        assert!(json.contains("\"key\""));
    }

    #[test]
    fn test_extract_json_with_prefix() {
        let response = "Here is the result: {\"key\": \"value\"} hope this helps";
        let json = extract_json_from_response(response);
        assert_eq!(json, r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_array() {
        let response = r#"["tag1", "tag2", "tag3"]"#;
        let json = extract_json_from_response(response);
        assert_eq!(json, r#"["tag1", "tag2", "tag3"]"#);
    }

    #[test]
    fn test_parse_extended_capture_analysis_success() {
        let response = r#"{
            "should_capture": true,
            "confidence": 0.85,
            "suggested_namespace": "decisions",
            "suggested_tags": ["rust"],
            "reasoning": "Clear decision",
            "security_assessment": {
                "injection_risk": 0.0,
                "poisoning_risk": 0.0,
                "social_engineering_risk": 0.0,
                "flags": [],
                "recommendation": "capture"
            },
            "contradiction_assessment": {
                "has_contradictions": false,
                "contradiction_risk": 0.0
            }
        }"#;

        let result = parse_extended_capture_analysis(response);
        assert!(result.is_ok());
        let analysis = result.unwrap();
        assert!(analysis.should_capture);
        assert!((analysis.confidence - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_extended_search_intent_success() {
        let response = r#"{
            "intent_type": "howto",
            "confidence": 0.9,
            "topics": ["authentication"],
            "reasoning": "User asking how to implement",
            "namespace_weights": {"patterns": 0.3}
        }"#;

        let result = parse_extended_search_intent(response);
        assert!(result.is_ok());
        let intent = result.unwrap();
        assert_eq!(intent.intent_type, "howto");
    }

    #[test]
    fn test_parse_consolidation_analysis_success() {
        let response = r#"{
            "merge_candidates": [],
            "archive_candidates": [],
            "contradictions": [],
            "summary": "No consolidation needed"
        }"#;

        let result = parse_consolidation_analysis(response);
        assert!(result.is_ok());
        let analysis = result.unwrap();
        assert!(analysis.merge_candidates.is_empty());
    }
}
