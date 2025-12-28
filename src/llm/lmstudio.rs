//! LM Studio client.

use super::{CaptureAnalysis, LlmProvider};
use crate::Result;

/// LM Studio local LLM client.
pub struct LmStudioClient {
    // TODO: Add API client
}

impl LmStudioClient {
    /// Creates a new LM Studio client.
    #[must_use]
    pub const fn new() -> Self {
        Self {}
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

    fn complete(&self, _prompt: &str) -> Result<String> {
        todo!("LmStudioClient::complete not yet implemented")
    }

    fn analyze_for_capture(&self, _content: &str) -> Result<CaptureAnalysis> {
        todo!("LmStudioClient::analyze_for_capture not yet implemented")
    }
}
