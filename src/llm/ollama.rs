//! Ollama (local) client.

use super::{CaptureAnalysis, LlmProvider};
use crate::Result;

/// Ollama local LLM client.
pub struct OllamaClient {
    // TODO: Add API client
}

impl OllamaClient {
    /// Creates a new Ollama client.
    #[must_use]
    pub const fn new() -> Self {
        Self {}
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

    fn complete(&self, _prompt: &str) -> Result<String> {
        todo!("OllamaClient::complete not yet implemented")
    }

    fn analyze_for_capture(&self, _content: &str) -> Result<CaptureAnalysis> {
        todo!("OllamaClient::analyze_for_capture not yet implemented")
    }
}
