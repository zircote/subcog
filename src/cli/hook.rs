//! Hook CLI command.
//!
//! Provides the `subcog hook` subcommand for Claude Code integration.
//! Hooks are invoked by Claude Code at specific lifecycle events to
//! inject context, detect signals, and capture memories.
//!
//! # Supported Hooks
//!
//! | Hook | Trigger | Purpose |
//! |------|---------|---------|
//! | `session-start` | Session begins | Inject relevant memories into context |
//! | `user-prompt-submit` | User sends message | Detect search intent, surface memories |
//! | `post-tool-use` | Tool execution completes | Surface related memories |
//! | `pre-compact` | Before context compaction | Auto-capture important content |
//! | `stop` | Session ends | Analyze session, sync to remote |
//!
//! # Usage
//!
//! ```bash
//! # Called by Claude Code hooks configuration
//! subcog hook session-start
//! subcog hook user-prompt-submit --prompt "How do I implement auth?"
//! subcog hook post-tool-use --tool-name "Read" --result "..."
//! subcog hook pre-compact --context "..."
//! subcog hook stop
//! ```
//!
//! # Configuration
//!
//! Hooks are configured in `.claude/settings.json`:
//!
//! ```json
//! {
//!   "hooks": {
//!     "SessionStart": [{ "type": "command", "command": "subcog hook session-start" }],
//!     "UserPromptSubmit": [{ "type": "command", "command": "subcog hook user-prompt-submit" }]
//!   }
//! }
//! ```

/// Hook command handler (DOC-H1).
///
/// Entry point for Claude Code hook integrations. Dispatches to specific
/// hook handlers based on the subcommand.
///
/// # Example
///
/// ```rust,ignore
/// use subcog::cli::HookCommand;
///
/// let cmd = HookCommand::new();
/// // Dispatch is handled by clap subcommand parsing in main.rs
/// ```
pub struct HookCommand;

impl HookCommand {
    /// Creates a new hook command handler.
    ///
    /// # Returns
    ///
    /// A new `HookCommand` instance ready to dispatch hook events.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for HookCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_command_new() {
        let _cmd = HookCommand::new();
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn test_hook_command_default() {
        let _cmd = HookCommand::default();
    }
}
