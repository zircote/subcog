//! `OpenAI` client.

use super::{CaptureAnalysis, LlmProvider};
use crate::Result;

/// `OpenAI` LLM client.
pub struct OpenAiClient {
    // TODO: Add API client
}

impl OpenAiClient {
    /// Creates a new `OpenAI` client.
    #[must_use]
    pub const fn new() -> Self {
        Self {}
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

    fn complete(&self, _prompt: &str) -> Result<String> {
        todo!("OpenAiClient::complete not yet implemented")
    }

    fn analyze_for_capture(&self, _content: &str) -> Result<CaptureAnalysis> {
        todo!("OpenAiClient::analyze_for_capture not yet implemented")
    }
}
