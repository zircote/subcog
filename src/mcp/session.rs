//! Session state tracking for MCP clients.
//!
//! Tracks whether the current session has been properly initialized via `subcog_init`.
//! Provides lightweight hints to uninitiated sessions for the first few tool calls.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Maximum number of hints to show for uninitialized sessions.
const MAX_HINTS: u32 = 3;

/// Global session state tracker.
///
/// Uses atomic operations for thread-safe state management without locks.
/// State is reset on server restart (ephemeral by design).
static INITIALIZED: AtomicBool = AtomicBool::new(false);
static HINT_COUNT: AtomicU32 = AtomicU32::new(0);

/// Marks the session as initialized.
///
/// Called by `subcog_init` after successful initialization.
pub fn mark_initialized() {
    INITIALIZED.store(true, Ordering::Release);
}

/// Checks if the session has been initialized.
#[must_use]
pub fn is_initialized() -> bool {
    INITIALIZED.load(Ordering::Acquire)
}

/// Checks if a hint should be shown and increments the counter.
///
/// Returns `true` if:
/// - Session is not initialized AND
/// - Hint count is below `MAX_HINTS`
///
/// Atomically increments the hint counter when returning `true`.
#[must_use]
pub fn should_show_hint() -> bool {
    if is_initialized() {
        return false;
    }

    // Atomically check and increment hint count
    let current = HINT_COUNT.fetch_add(1, Ordering::AcqRel);
    current < MAX_HINTS
}

/// Returns the initialization hint message.
///
/// This is appended to tool responses when `should_show_hint()` returns `true`.
#[must_use]
pub const fn get_hint_message() -> &'static str {
    "\n\n---\n💡 **Tip**: Call `subcog_init` at session start to load memory context and best practices."
}

/// Resets session state (primarily for testing).
#[cfg(test)]
pub fn reset() {
    INITIALIZED.store(false, Ordering::Release);
    HINT_COUNT.store(0, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialization_flow() {
        reset();

        // Initially not initialized
        assert!(!is_initialized());

        // Mark as initialized
        mark_initialized();
        assert!(is_initialized());
    }

    #[test]
    fn test_hint_limiting() {
        reset();

        // Should show hints for first MAX_HINTS calls
        for i in 0..MAX_HINTS {
            assert!(
                should_show_hint(),
                "Should show hint for call {i}"
            );
        }

        // Should stop showing hints after MAX_HINTS
        assert!(
            !should_show_hint(),
            "Should not show hint after MAX_HINTS"
        );
    }

    #[test]
    fn test_no_hints_when_initialized() {
        reset();

        // Initialize first
        mark_initialized();

        // Should never show hints when initialized
        for _ in 0..10 {
            assert!(
                !should_show_hint(),
                "Should not show hints when initialized"
            );
        }
    }

    #[test]
    fn test_hint_message_content() {
        let msg = get_hint_message();
        assert!(msg.contains("subcog_init"));
        assert!(msg.contains("session start"));
    }
}
