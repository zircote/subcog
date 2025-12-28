//! Anthropic Claude client.

use super::{CaptureAnalysis, LlmProvider};
use crate::Result;

/// Anthropic Claude LLM client.
pub struct AnthropicClient {
    // TODO: Add API client
}

impl AnthropicClient {
    /// Creates a new Anthropic client.
    #[must_use]
    pub const fn new() -> Self {
        Self {}
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

    fn complete(&self, _prompt: &str) -> Result<String> {
        todo!("AnthropicClient::complete not yet implemented")
    }

    fn analyze_for_capture(&self, _content: &str) -> Result<CaptureAnalysis> {
        todo!("AnthropicClient::analyze_for_capture not yet implemented")
    }
}
