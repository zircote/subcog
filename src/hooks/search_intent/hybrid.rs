//! Hybrid and timeout-aware search intent detection.
//!
//! This module combines keyword and LLM detection for optimal accuracy
//! while maintaining responsive performance through configurable timeouts.

use super::keyword::detect_search_intent;
use super::llm::classify_intent_with_llm;
use super::types::{DetectionSource, SearchIntent};
use crate::config::SearchIntentConfig;
use crate::llm::LlmProvider as LlmProviderTrait;
use crate::observability::{RequestContext, current_request_id, enter_request_context};
use std::sync::{Arc, mpsc};
use std::time::Duration;

/// Detects search intent with LLM classification and timeout.
///
/// Uses LLM classification with a configurable timeout. Falls back to keyword
/// detection if LLM times out or fails.
///
/// # Arguments
///
/// * `provider` - Optional LLM provider. If None, uses keyword-only detection.
/// * `prompt` - The user prompt to analyze.
/// * `config` - Configuration for intent detection.
///
/// # Returns
///
/// A `SearchIntent` from either LLM or keyword detection.
///
/// # Panics
///
/// This function does not panic under normal operation.
#[must_use]
pub fn detect_search_intent_with_timeout(
    provider: Option<Arc<dyn LlmProviderTrait>>,
    prompt: &str,
    config: &SearchIntentConfig,
) -> SearchIntent {
    // If LLM is disabled or no provider, use keyword detection
    if !config.use_llm || provider.is_none() {
        return detect_search_intent(prompt).unwrap_or_default();
    }

    let timeout = Duration::from_millis(config.llm_timeout_ms);
    let llm_result = run_llm_with_timeout(provider, prompt.to_string(), timeout);

    // Return LLM result if successful, otherwise fall back to keyword
    llm_result.unwrap_or_else(|| detect_search_intent(prompt).unwrap_or_default())
}

/// Detects search intent using hybrid keyword + LLM detection.
///
/// Runs keyword detection immediately and LLM detection in parallel.
/// Merges results with LLM taking precedence for intent type if confidence is high.
///
/// # Arguments
///
/// * `provider` - Optional LLM provider.
/// * `prompt` - The user prompt to analyze.
/// * `config` - Configuration for intent detection.
///
/// # Returns
///
/// A merged `SearchIntent` from both detection methods.
///
/// # Panics
///
/// This function does not panic under normal operation.
#[must_use]
pub fn detect_search_intent_hybrid(
    provider: Option<Arc<dyn LlmProviderTrait>>,
    prompt: &str,
    config: &SearchIntentConfig,
) -> SearchIntent {
    // Always run keyword detection (fast)
    let keyword_result = detect_search_intent(prompt);

    // If LLM is disabled or no provider, return keyword result
    if !config.use_llm || provider.is_none() {
        return keyword_result.unwrap_or_default();
    }

    let timeout = Duration::from_millis(config.llm_timeout_ms);
    let llm_result = run_llm_with_timeout(provider, prompt.to_string(), timeout);

    // Merge results
    merge_intent_results(keyword_result, llm_result, config)
}

/// Runs LLM classification with a timeout (CHAOS-H3).
///
/// # Thread Lifecycle
///
/// Spawns a background thread for LLM classification. If the timeout is exceeded:
/// - The result is discarded (receiver times out)
/// - The thread continues to completion naturally (Rust threads cannot be killed)
/// - A metric is recorded for monitoring orphaned operations
/// - The thread will complete its HTTP request and exit cleanly
///
/// This is a deliberate design choice because:
/// 1. Rust has no safe way to forcefully terminate threads
/// 2. Interrupting HTTP requests mid-flight can cause resource leaks
/// 3. The thread will complete quickly once the LLM responds
///
/// For production, consider using async with timeout + cancellation tokens.
fn run_llm_with_timeout(
    provider: Option<Arc<dyn LlmProviderTrait>>,
    prompt: String,
    timeout: Duration,
) -> Option<SearchIntent> {
    let provider = provider?;
    let (tx, rx) = mpsc::channel();
    let parent_span = tracing::Span::current();
    let request_id = current_request_id();

    // Record that we're starting an LLM call
    metrics::counter!("search_intent_llm_started").increment(1);

    std::thread::spawn(move || {
        let _request_guard = request_id
            .map(RequestContext::from_id)
            .map(enter_request_context);
        let _parent = parent_span.enter();
        let span = tracing::info_span!("search_intent.llm");
        let _guard = span.enter();
        let result = classify_intent_with_llm(provider.as_ref(), &prompt);
        // If receiver dropped (timeout), send will fail silently - this is expected
        let _ = tx.send(result);
    });

    // Record completion based on whether we received a result within timeout (CHAOS-H3)
    match rx.recv_timeout(timeout) {
        Ok(Ok(intent)) => {
            metrics::counter!("search_intent_llm_completed", "status" => "success").increment(1);
            Some(intent)
        },
        Ok(Err(_)) => {
            // LLM returned an error - no timeout, just failure
            metrics::counter!("search_intent_llm_completed", "status" => "error").increment(1);
            None
        },
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            // CHAOS-H3: Record metric for monitoring orphaned threads
            // Note: The spawned thread will continue running in the background.
            // Rust cannot cancel/kill threads, so it will complete eventually.
            // This is acceptable for short-lived LLM classification tasks.
            metrics::counter!("search_intent_llm_timeout_total", "reason" => "timeout")
                .increment(1);
            metrics::counter!("search_intent_llm_completed", "status" => "timeout").increment(1);
            tracing::debug!("LLM classification timed out, thread will complete in background");
            None
        },
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            // Thread panicked or dropped sender - unusual but handle gracefully
            metrics::counter!("search_intent_llm_timeout_total", "reason" => "disconnected")
                .increment(1);
            metrics::counter!("search_intent_llm_completed", "status" => "disconnected")
                .increment(1);
            None
        },
    }
}

/// Merges keyword and LLM intent results.
fn merge_intent_results(
    keyword: Option<SearchIntent>,
    llm: Option<SearchIntent>,
    config: &SearchIntentConfig,
) -> SearchIntent {
    match (keyword, llm) {
        // Both available: prefer LLM if high confidence
        (Some(kw), Some(llm_intent)) => {
            if llm_intent.confidence >= config.min_confidence {
                SearchIntent {
                    intent_type: llm_intent.intent_type,
                    // Average confidence weighted toward LLM
                    confidence: llm_intent
                        .confidence
                        .mul_add(0.7, kw.confidence * 0.3)
                        .min(0.95),
                    // Combine keywords from keyword detection
                    keywords: kw.keywords,
                    // Prefer LLM topics if available, otherwise keyword topics
                    topics: if llm_intent.topics.is_empty() {
                        kw.topics
                    } else {
                        llm_intent.topics
                    },
                    source: DetectionSource::Hybrid,
                }
            } else {
                // LLM confidence too low, use keyword result
                SearchIntent {
                    source: DetectionSource::Hybrid,
                    ..kw
                }
            }
        },
        // Only keyword available
        (Some(kw), None) => kw,
        // Only LLM available (unusual but possible)
        (None, Some(llm_intent)) => SearchIntent {
            source: DetectionSource::Llm,
            ..llm_intent
        },
        // Neither available
        (None, None) => SearchIntent::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::search_intent::types::SearchIntentType;

    fn test_config() -> SearchIntentConfig {
        SearchIntentConfig {
            enabled: true,
            use_llm: true,
            llm_timeout_ms: 200,
            min_confidence: 0.5,
            ..Default::default()
        }
    }

    #[test]
    fn test_merge_both_available_llm_high_confidence() {
        let keyword = Some(
            SearchIntent::new(SearchIntentType::HowTo)
                .with_confidence(0.6)
                .with_keywords(vec!["how".to_string()])
                .with_topics(vec!["keyword_topic".to_string()]),
        );
        let llm = Some(
            SearchIntent::new(SearchIntentType::Troubleshoot)
                .with_confidence(0.9)
                .with_topics(vec!["llm_topic".to_string()]),
        );

        let result = merge_intent_results(keyword, llm, &test_config());

        // Should use LLM intent type (high confidence)
        assert_eq!(result.intent_type, SearchIntentType::Troubleshoot);
        // Should keep keyword keywords
        assert_eq!(result.keywords, vec!["how"]);
        // Should use LLM topics
        assert_eq!(result.topics, vec!["llm_topic"]);
        assert_eq!(result.source, DetectionSource::Hybrid);
    }

    #[test]
    fn test_merge_both_available_llm_low_confidence() {
        let keyword = Some(
            SearchIntent::new(SearchIntentType::HowTo)
                .with_confidence(0.6)
                .with_keywords(vec!["how".to_string()]),
        );
        let llm = Some(
            SearchIntent::new(SearchIntentType::Troubleshoot).with_confidence(0.3), // Below min_confidence
        );

        let result = merge_intent_results(keyword, llm, &test_config());

        // Should use keyword intent type (LLM confidence too low)
        assert_eq!(result.intent_type, SearchIntentType::HowTo);
        assert_eq!(result.source, DetectionSource::Hybrid);
    }

    #[test]
    fn test_merge_keyword_only() {
        let keyword = Some(SearchIntent::new(SearchIntentType::Location).with_confidence(0.7));

        let result = merge_intent_results(keyword, None, &test_config());

        assert_eq!(result.intent_type, SearchIntentType::Location);
        assert_eq!(result.source, DetectionSource::Keyword);
    }

    #[test]
    fn test_merge_llm_only() {
        let llm = Some(
            SearchIntent::new(SearchIntentType::Explanation)
                .with_confidence(0.8)
                .with_source(DetectionSource::Llm),
        );

        let result = merge_intent_results(None, llm, &test_config());

        assert_eq!(result.intent_type, SearchIntentType::Explanation);
        assert_eq!(result.source, DetectionSource::Llm);
    }

    #[test]
    fn test_merge_neither_available() {
        let result = merge_intent_results(None, None, &test_config());

        assert_eq!(result.intent_type, SearchIntentType::General);
        assert!(result.confidence.abs() < f32::EPSILON);
    }

    #[test]
    fn test_detect_with_timeout_no_provider() {
        let config = SearchIntentConfig {
            enabled: true,
            use_llm: true,
            llm_timeout_ms: 200,
            min_confidence: 0.5,
            ..Default::default()
        };

        let result = detect_search_intent_with_timeout(None, "How do I test?", &config);

        // Should fall back to keyword detection
        assert_eq!(result.intent_type, SearchIntentType::HowTo);
        assert_eq!(result.source, DetectionSource::Keyword);
    }

    #[test]
    fn test_detect_with_timeout_llm_disabled() {
        let config = SearchIntentConfig {
            enabled: true,
            use_llm: false,
            llm_timeout_ms: 200,
            min_confidence: 0.5,
            ..Default::default()
        };

        let result = detect_search_intent_with_timeout(None, "Where is the config?", &config);

        assert_eq!(result.intent_type, SearchIntentType::Location);
        assert_eq!(result.source, DetectionSource::Keyword);
    }

    #[test]
    fn test_detect_hybrid_no_provider() {
        let config = test_config();

        // Use "difference between" which triggers Comparison without "What is" (Explanation)
        let result = detect_search_intent_hybrid(None, "Difference between X and Y?", &config);

        // Should fall back to keyword detection
        assert_eq!(result.intent_type, SearchIntentType::Comparison);
        assert_eq!(result.source, DetectionSource::Keyword);
    }
}
