//! Claude Code hooks.
//!
//! Implements handlers for Claude Code hook events.

// Allow unused_self for methods that are kept as methods for API consistency
// or may use self in future implementations.
#![allow(clippy::unused_self)]

mod post_tool_use;
mod pre_compact;
mod search_context;
mod search_intent;
mod search_patterns;
mod session_start;
mod stop;
mod user_prompt;

pub use post_tool_use::PostToolUseHandler;
pub use pre_compact::PreCompactHandler;
pub use search_context::{
    AdaptiveContextConfig, InjectedMemory, MemoryContext, NamespaceWeights, SearchContextBuilder,
};
pub use search_intent::{
    DetectionSource, SearchIntent, SearchIntentType, classify_intent_with_llm,
    detect_search_intent, detect_search_intent_hybrid, detect_search_intent_with_timeout,
};
pub use session_start::SessionStartHandler;
pub use stop::StopHandler;
pub use user_prompt::UserPromptHandler;

use crate::Result;

/// Trait for hook handlers.
pub trait HookHandler: Send + Sync {
    /// The hook event type this handler processes.
    fn event_type(&self) -> &'static str;

    /// Handles the hook event.
    ///
    /// # Errors
    ///
    /// Returns an error if handling fails.
    fn handle(&self, input: &str) -> Result<String>;
}
