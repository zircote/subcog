//! Claude Code hooks.
//!
//! Implements handlers for Claude Code hook events.

mod post_tool_use;
mod pre_compact;
mod session_start;
mod stop;
mod user_prompt;

pub use post_tool_use::PostToolUseHandler;
pub use pre_compact::PreCompactHandler;
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
