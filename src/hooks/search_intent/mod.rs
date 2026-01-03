//! Search intent detection for proactive memory surfacing.
//!
//! Detects user intent to search for information and extracts topics for memory injection.
//!
//! # Architecture
//!
//! This module is organized into focused submodules:
//!
//! - [`types`]: Core types (`SearchIntentType`, `DetectionSource`, `SearchIntent`)
//! - [`keyword`]: Fast pattern-based detection (<10ms)
//! - [`llm`]: LLM-powered classification for higher accuracy
//! - [`hybrid`]: Combined detection with timeout support
//!
//! # Detection Modes
//!
//! | Mode | Latency | Accuracy | Use Case |
//! |------|---------|----------|----------|
//! | Keyword | <10ms | Good | Default, always available |
//! | LLM | ~200ms | Excellent | When LLM provider configured |
//! | Hybrid | <200ms | Best | Combines both with fallback |
//!
//! # Intent Types and Detection
//!
//! | Intent | Trigger Patterns | Namespace Weights |
//! |--------|------------------|-------------------|
//! | `HowTo` | "how do I", "how to", "implement", "create" | patterns (2.0), learnings (1.5) |
//! | `Location` | "where is", "find", "locate" | context (1.5), patterns (1.2) |
//! | `Explanation` | "what is", "explain", "describe" | decisions (1.5), context (1.2) |
//! | `Comparison` | "difference between", "vs", "compare" | decisions (2.0), patterns (1.5) |
//! | `Troubleshoot` | "error", "fix", "not working", "debug" | blockers (2.0), learnings (1.5) |
//! | `General` | "search", "show me" | All namespaces weighted equally |
//!
//! # Detection Flow
//!
//! ```text
//! User Prompt
//!     │
//!     ├─► Keyword Detection (<10ms)
//!     │       │
//!     │       └─► Intent + Confidence + Topics
//!     │
//!     └─► LLM Classification (200ms timeout) [optional]
//!             │
//!             └─► Intent + Confidence + Topics
//!                     │
//!                     └─► Merge Results (hybrid mode)
//!                             │
//!                             └─► Final Intent + Topics
//! ```
//!
//! # Confidence-Based Memory Injection
//!
//! | Confidence | Memory Count | Behavior |
//! |------------|--------------|----------|
//! | ≥ 0.8 (high) | 15 memories | Full context injection |
//! | ≥ 0.5 (medium) | 10 memories | Standard injection |
//! | < 0.5 (low) | 5 memories | Minimal injection |
//!
//! # Configuration
//!
//! | Environment Variable | Description | Default |
//! |---------------------|-------------|---------|
//! | `SUBCOG_SEARCH_INTENT_ENABLED` | Enable intent detection | `true` |
//! | `SUBCOG_SEARCH_INTENT_USE_LLM` | Enable LLM classification | `true` |
//! | `SUBCOG_SEARCH_INTENT_LLM_TIMEOUT_MS` | LLM timeout | `200` |
//! | `SUBCOG_SEARCH_INTENT_MIN_CONFIDENCE` | Minimum confidence | `0.5` |
//!
//! # Examples
//!
//! ```rust,ignore
//! use subcog::hooks::search_intent::{detect_search_intent, SearchIntentType};
//!
//! // Simple keyword detection
//! if let Some(intent) = detect_search_intent("How do I implement OAuth?") {
//!     assert_eq!(intent.intent_type, SearchIntentType::HowTo);
//!     println!("Detected: {} with confidence {}", intent.intent_type, intent.confidence);
//! }
//! ```

mod hybrid;
mod keyword;
mod llm;
mod types;

// Re-export public types
pub use types::{DetectionSource, SearchIntent, SearchIntentType};

// Re-export detection functions
pub use hybrid::{detect_search_intent_hybrid, detect_search_intent_with_timeout};
pub use keyword::detect_search_intent;
pub use llm::classify_intent_with_llm;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all public items are accessible
        let intent_type = SearchIntentType::HowTo;
        assert_eq!(intent_type.as_str(), "howto");

        let source = DetectionSource::Keyword;
        assert_eq!(source.as_str(), "keyword");

        let intent = SearchIntent::default();
        assert_eq!(intent.intent_type, SearchIntentType::General);

        // Verify functions are accessible
        let result = detect_search_intent("How do I test?");
        assert!(result.is_some());
    }

    #[test]
    fn test_end_to_end_keyword_detection() {
        let prompts_and_intents = [
            ("How do I implement caching?", SearchIntentType::HowTo),
            ("Where is the config file?", SearchIntentType::Location),
            (
                "What is dependency injection?",
                SearchIntentType::Explanation,
            ),
            (
                "What's the difference between sync and async?",
                SearchIntentType::Comparison,
            ),
            (
                "Why am I getting this error?",
                SearchIntentType::Troubleshoot,
            ),
        ];

        for (prompt, expected_intent) in prompts_and_intents {
            let result = detect_search_intent(prompt);
            assert!(result.is_some(), "Should detect intent for: {prompt}");
            assert_eq!(
                result.unwrap().intent_type,
                expected_intent,
                "Wrong intent for: {prompt}"
            );
        }
    }

    #[test]
    fn test_namespace_weights_vary_by_intent() {
        let howto_weights = SearchIntentType::HowTo.namespace_weights();
        let troubleshoot_weights = SearchIntentType::Troubleshoot.namespace_weights();

        // HowTo should prioritize patterns
        assert!(
            howto_weights
                .iter()
                .any(|(ns, w)| *ns == "patterns" && *w >= 2.0)
        );

        // Troubleshoot should prioritize blockers
        assert!(
            troubleshoot_weights
                .iter()
                .any(|(ns, w)| *ns == "blockers" && *w >= 2.0)
        );
    }
}
