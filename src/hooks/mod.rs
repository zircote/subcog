//! Claude Code hooks.
//!
//! Implements handlers for Claude Code hook events.
//!
//! # Hook Response JSON Format
//!
//! Claude Code hooks have different response formats depending on the event type.
//! Only certain hooks support `hookSpecificOutput` with `additionalContext`.
//!
//! ## Hooks Supporting Context Injection
//!
//! The following hooks can inject context via `hookSpecificOutput.additionalContext`:
//!
//! | Event | `hookEventName` | `additionalContext` Content |
//! |-------|-----------------|----------------------------|
//! | Session start | `SessionStart` | Memory statistics, recent topics, tutorial info |
//! | User prompt | `UserPromptSubmit` | Relevant memories based on search intent |
//! | Post tool use | `PostToolUse` | Related memories surfaced by tool output |
//!
//! Example response for context-supporting hooks:
//!
//! ```json
//! {
//!   "hookSpecificOutput": {
//!     "hookEventName": "UserPromptSubmit",
//!     "additionalContext": "# Memory Context\n\n..."
//!   }
//! }
//! ```
//!
//! ## Hooks Without Context Injection
//!
//! The following hooks do NOT support `hookSpecificOutput`:
//!
//! | Event | Response | Notes |
//! |-------|----------|-------|
//! | Pre-compact | `{}` | Context logged only, auto-capture performed |
//! | Stop | `{}` | Session summary logged only, sync performed |
//!
//! These hooks perform their side effects (auto-capture, sync) and log context
//! for debugging, but return empty JSON since Claude Code's schema doesn't
//! support `hookSpecificOutput` for these event types.
//!
//! ## Empty Response
//!
//! When no context is available, return an empty object `{}`.
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
pub use session_start::{GuidanceLevel, SessionStartHandler};
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
