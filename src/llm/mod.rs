//! LLM client abstraction.
//!
//! Provides a unified interface for different LLM providers.

mod anthropic;
mod lmstudio;
mod ollama;
mod openai;

pub use anthropic::AnthropicClient;
pub use lmstudio::LmStudioClient;
pub use ollama::OllamaClient;
pub use openai::OpenAiClient;

use crate::Result;

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

    /// Analyzes content for memory capture.
    ///
    /// # Errors
    ///
    /// Returns an error if analysis fails.
    fn analyze_for_capture(&self, content: &str) -> Result<CaptureAnalysis>;
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
