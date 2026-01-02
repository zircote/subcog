//! Claude Code hooks.
//!
//! Implements handlers for Claude Code hook events.
//!
//! # Hook Response JSON Format
//!
//! All hooks return responses in the Claude Code hook format:
//!
//! ```json
//! {
//!   "hookSpecificOutput": {
//!     "hookEventName": "SessionStart",
//!     "additionalContext": "# Memory Context\n\n..."
//!   }
//! }
//! ```
//!
//! ## Field Descriptions
//!
//! | Field | Type | Description |
//! |-------|------|-------------|
//! | `hookSpecificOutput` | object | Required wrapper for hook-specific data |
//! | `hookEventName` | string | One of: `SessionStart`, `UserPromptSubmit`, `PostToolUse`, `PreCompact`, `Stop` |
//! | `additionalContext` | string | Markdown content to inject into context |
//!
//! ## Hook Event Responses
//!
//! | Event | `hookEventName` | `additionalContext` Content |
//! |-------|-----------------|----------------------------|
//! | Session start | `SessionStart` | Memory statistics, recent topics, tutorial info |
//! | User prompt | `UserPromptSubmit` | Relevant memories based on search intent |
//! | Post tool use | `PostToolUse` | Related memories surfaced by tool output |
//! | Pre-compact | `PreCompact` | Auto-captured memories before context compaction |
//! | Stop | `Stop` | Sync status, session summary |
//!
//! ## Empty Response
//!
//! When no context is available, return an empty object:
//!
//! ```json
//! {
//!   "hookSpecificOutput": {
//!     "hookEventName": "UserPromptSubmit",
//!     "additionalContext": ""
//!   }
//! }
//! ```
//!
//! # Handler Configuration
//!
//! All handlers use a builder pattern for dependency injection. While handlers
//! can be created with `Handler::new()`, they require specific services to
//! function properly:
//!
//! | Handler | Required Service | Builder Method |
//! |---------|-----------------|----------------|
//! | [`PreCompactHandler`] | [`CaptureService`](crate::services::CaptureService) | `with_capture()` |
//! | [`UserPromptHandler`] | [`RecallService`](crate::services::RecallService) | `with_recall()` |
//! | [`PostToolUseHandler`] | [`RecallService`](crate::services::RecallService) | `with_recall()` |
//! | [`StopHandler`] | [`SyncService`](crate::services::SyncService) | `with_sync()` |
//! | [`SessionStartHandler`] | [`ContextBuilderService`](crate::services::ContextBuilderService) | `with_context_builder()` |
//!
//! Handlers degrade gracefully when required services are not configured,
//! returning empty results and logging debug messages.

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
