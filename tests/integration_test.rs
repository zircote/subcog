//! Integration tests for subcog.

use subcog::Error;

#[test]
fn test_error_types() {
    // Test InvalidInput error
    let err = Error::InvalidInput("test message".to_string());
    let display = format!("{err}");
    assert!(display.contains("invalid input"));
    assert!(display.contains("test message"));

    // Test OperationFailed error
    let err = Error::OperationFailed {
        operation: "read".to_string(),
        cause: "file not found".to_string(),
    };
    let display = format!("{err}");
    assert!(display.contains("read"));
    assert!(display.contains("file not found"));

    // Test ContentBlocked error
    let err = Error::ContentBlocked {
        reason: "secrets detected".to_string(),
    };
    let display = format!("{err}");
    assert!(display.contains("content blocked"));
    assert!(display.contains("secrets detected"));

    // Test FeatureNotEnabled error
    let err = Error::FeatureNotEnabled("vector-search".to_string());
    let display = format!("{err}");
    assert!(display.contains("not enabled"));
    assert!(display.contains("vector-search"));

    // Test NotImplemented error
    let err = Error::NotImplemented("sync feature".to_string());
    let display = format!("{err}");
    assert!(display.contains("not implemented"));
    assert!(display.contains("sync feature"));
}

/// Graceful degradation tests for proactive memory surfacing.
///
/// Tests verify that the system degrades gracefully when components are unavailable:
/// - LLM unavailable → keyword-only detection
/// - `RecallService` unavailable → skip memory injection
/// - Low confidence → reduced memory count
mod graceful_degradation_tests {
    use std::sync::Arc;
    use std::time::Duration;
    use subcog::Result;
    use subcog::config::SearchIntentConfig;
    use subcog::hooks::{
        AdaptiveContextConfig, DetectionSource, MemoryContext, NamespaceWeights,
        SearchContextBuilder, SearchIntent, SearchIntentType, detect_search_intent,
        detect_search_intent_hybrid, detect_search_intent_with_timeout,
    };
    use subcog::llm::{CaptureAnalysis, LlmProvider};

    // Mock LLM that fails
    struct FailingLlmProvider;

    impl LlmProvider for FailingLlmProvider {
        fn name(&self) -> &'static str {
            "failing"
        }

        fn complete(&self, _prompt: &str) -> Result<String> {
            Err(subcog::Error::OperationFailed {
                operation: "llm_complete".to_string(),
                cause: "Service unavailable".to_string(),
            })
        }

        fn analyze_for_capture(&self, _content: &str) -> Result<CaptureAnalysis> {
            Ok(CaptureAnalysis {
                should_capture: false,
                confidence: 0.0,
                suggested_namespace: None,
                suggested_tags: Vec::new(),
                reasoning: String::new(),
            })
        }
    }

    // Mock LLM that times out (takes too long)
    struct SlowLlmProvider {
        delay_ms: u64,
    }

    impl LlmProvider for SlowLlmProvider {
        fn name(&self) -> &'static str {
            "slow"
        }

        fn complete(&self, _prompt: &str) -> Result<String> {
            std::thread::sleep(Duration::from_millis(self.delay_ms));
            Ok(r#"{"intent_type": "howto", "confidence": 0.9}"#.to_string())
        }

        fn analyze_for_capture(&self, _content: &str) -> Result<CaptureAnalysis> {
            Ok(CaptureAnalysis {
                should_capture: false,
                confidence: 0.0,
                suggested_namespace: None,
                suggested_tags: Vec::new(),
                reasoning: String::new(),
            })
        }
    }

    #[test]
    fn test_llm_unavailable_falls_back_to_keyword() {
        // When LLM provider is None, should fall back to keyword detection
        let config = SearchIntentConfig::default();
        let intent =
            detect_search_intent_with_timeout(None, "how do I implement authentication?", &config);

        // Should still detect intent via keywords
        assert_eq!(intent.source, DetectionSource::Keyword);
        assert_eq!(intent.intent_type, SearchIntentType::HowTo);
        assert!(intent.confidence >= 0.5);
    }

    #[test]
    fn test_llm_disabled_uses_keyword_only() {
        // When LLM is disabled via config, should use keyword-only
        let failing_provider = Arc::new(FailingLlmProvider);
        let config = SearchIntentConfig::default().with_use_llm(false);

        let intent = detect_search_intent_with_timeout(
            Some(failing_provider),
            "where is the database config?",
            &config,
        );

        assert_eq!(intent.source, DetectionSource::Keyword);
        assert_eq!(intent.intent_type, SearchIntentType::Location);
    }

    #[test]
    fn test_llm_timeout_falls_back_to_keyword() {
        // When LLM times out, should fall back to keyword detection
        let slow_provider = Arc::new(SlowLlmProvider { delay_ms: 500 });
        let config = SearchIntentConfig::default().with_llm_timeout_ms(50); // 50ms timeout

        let intent = detect_search_intent_with_timeout(
            Some(slow_provider),
            "what is the purpose of this module?",
            &config,
        );

        // Should fall back to keyword detection
        assert_eq!(intent.source, DetectionSource::Keyword);
    }

    #[test]
    fn test_llm_failure_in_hybrid_falls_back_to_keyword() {
        // In hybrid mode, LLM failure should result in keyword-only results
        let failing_provider = Arc::new(FailingLlmProvider);
        let config = SearchIntentConfig::default()
            .with_llm_timeout_ms(1000)
            .with_min_confidence(0.5);

        let intent = detect_search_intent_hybrid(
            Some(failing_provider),
            "why is this error happening?",
            &config,
        );

        // Should still have valid intent from keyword detection
        assert_eq!(intent.intent_type, SearchIntentType::Troubleshoot);
        // Source might be Keyword since LLM failed
        assert!(intent.confidence >= 0.5);
    }

    #[test]
    fn test_no_recall_service_skips_memory_injection() {
        // When no RecallService is provided, should skip memory injection
        let intent = SearchIntent::new(SearchIntentType::HowTo)
            .with_confidence(0.9)
            .with_topics(vec!["authentication".to_string()]);

        let builder = SearchContextBuilder::new();
        let context = builder
            .build_context(&intent)
            .expect("SearchContextBuilder should build context for HowTo intent");

        // Should have context but no injected memories
        assert!(context.search_intent_detected);
        assert!(context.injected_memories.is_empty());
        // But should still have suggested resources
        assert!(!context.suggested_resources.is_empty());
    }

    #[test]
    fn test_low_confidence_returns_empty_context() {
        // Low confidence should skip injection entirely
        let intent = SearchIntent::new(SearchIntentType::General).with_confidence(0.3); // Below min_confidence

        let builder = SearchContextBuilder::new()
            .with_config(AdaptiveContextConfig::new().with_min_confidence(0.5));

        let context = builder
            .build_context(&intent)
            .expect("SearchContextBuilder should handle low confidence intents");

        // Should return empty context
        assert!(!context.search_intent_detected);
        assert!(context.injected_memories.is_empty());
        assert!(context.suggested_resources.is_empty());
    }

    #[test]
    fn test_confidence_determines_memory_count() {
        let config = AdaptiveContextConfig::default();

        // High confidence (>=0.8) should use max_count
        assert_eq!(config.memories_for_confidence(0.9), config.max_count);
        assert_eq!(config.memories_for_confidence(0.8), config.max_count);

        // Medium confidence (>=0.5) should use base_count + 5
        assert_eq!(config.memories_for_confidence(0.7), config.base_count + 5);
        assert_eq!(config.memories_for_confidence(0.5), config.base_count + 5);

        // Low confidence (<0.5) should use base_count
        assert_eq!(config.memories_for_confidence(0.4), config.base_count);
        assert_eq!(config.memories_for_confidence(0.1), config.base_count);
    }

    #[test]
    fn test_no_search_intent_detected() {
        // Generic prompt without search signals should return None
        let result = detect_search_intent("Hello, I'm working on a project today.");

        // May or may not detect - if detected, should be low confidence
        if let Some(intent) = result {
            assert!(intent.confidence < 0.7);
        }
    }

    #[test]
    fn test_empty_prompt_returns_default() {
        // Empty prompt should not crash, should return None
        let result = detect_search_intent("");
        assert!(result.is_none());

        // With timeout function, should return default
        let config = SearchIntentConfig::default();
        let intent = detect_search_intent_with_timeout(None, "", &config);

        assert_eq!(intent.intent_type, SearchIntentType::General);
        assert!(intent.confidence.abs() < f32::EPSILON);
    }

    #[test]
    fn test_namespace_weights_unknown_namespace_defaults_to_one() {
        // Unknown namespaces should default to weight 1.0
        let weights = NamespaceWeights::for_intent(SearchIntentType::HowTo);

        // Known namespace should have weight
        assert!((weights.get(&subcog::models::Namespace::Patterns) - 1.5).abs() < f32::EPSILON);

        // Unknown namespace should default to 1.0
        assert!((weights.get(&subcog::models::Namespace::TechDebt) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_memory_context_from_intent_preserves_topics() {
        let intent = SearchIntent::new(SearchIntentType::Explanation)
            .with_confidence(0.75)
            .with_topics(vec!["topic1".to_string(), "topic2".to_string()]);

        let context = MemoryContext::from_intent(&intent);

        assert!(context.search_intent_detected);
        assert_eq!(context.intent_type, Some("explanation".to_string()));
        assert_eq!(context.topics.len(), 2);
        assert!(context.topics.contains(&"topic1".to_string()));
    }
}

/// Hook handler integration tests.
///
/// Tests all 5 Claude Code hooks to ensure they execute correctly
/// and produce valid output for observability dashboards.
mod hook_handler_tests {
    use subcog::hooks::{
        HookHandler, PostToolUseHandler, PreCompactHandler, SessionStartHandler, StopHandler,
        UserPromptHandler,
    };

    #[test]
    fn test_session_start_hook_executes() {
        let handler = SessionStartHandler::new();
        let result = handler.handle("");

        let output = result.expect("SessionStart hook should succeed");
        assert!(
            output.contains("hookSpecificOutput"),
            "Should have hook output"
        );
        assert!(
            output.contains("SessionStart"),
            "Should identify as SessionStart"
        );
    }

    #[test]
    fn test_user_prompt_submit_hook_executes() {
        let handler = UserPromptHandler::new();
        let input = r#"{"prompt": "How do I implement authentication?"}"#;
        let result = handler.handle(input);

        let output = result.expect("UserPromptSubmit hook should succeed");
        assert!(
            output.contains("hookSpecificOutput"),
            "Should have hook output"
        );
        assert!(
            output.contains("UserPromptSubmit"),
            "Should identify as UserPromptSubmit"
        );
    }

    #[test]
    fn test_post_tool_use_hook_executes() {
        let handler = PostToolUseHandler::new();
        let input = r#"{"tool_name": "Read", "tool_input": {"file_path": "/test.rs"}, "tool_output": "contents"}"#;
        let result = handler.handle(input);

        assert!(result.is_ok(), "PostToolUse hook should succeed");
    }

    #[test]
    fn test_pre_compact_hook_executes() {
        let handler = PreCompactHandler::new();
        let input = r#"{"sections": [{"role": "user", "content": "Test content"}]}"#;
        let result = handler.handle(input);

        assert!(result.is_ok(), "PreCompact hook should succeed");
    }

    #[test]
    fn test_pre_compact_hook_with_decision_content() {
        let handler = PreCompactHandler::new();
        let input = r#"{
            "sections": [
                {"role": "user", "content": "We need to decide on a database. Should we use PostgreSQL?"},
                {"role": "assistant", "content": "I recommend PostgreSQL for better JSON support."},
                {"role": "user", "content": "OK, let's use PostgreSQL with pgbouncer for connection pooling."},
                {"role": "assistant", "content": "Great choice! I'll set that up."}
            ]
        }"#;
        let result = handler.handle(input);

        let output = result.expect("PreCompact hook with decisions should succeed");
        // May or may not capture depending on LLM availability
        assert!(
            output.contains("hookSpecificOutput") || output == "{}",
            "Should have valid output"
        );
    }

    #[test]
    fn test_stop_hook_executes() {
        let handler = StopHandler::new();
        let input = r#"{"session_duration_seconds": 120}"#;
        let result = handler.handle(input);

        let output = result.expect("Stop hook should succeed");
        // Stop hooks return empty JSON per Claude Code hook specification
        // (hookSpecificOutput is not supported for Stop events)
        assert_eq!(output, "{}", "Stop hook should return empty JSON");
    }

    #[test]
    fn test_all_hooks_return_valid_json() {
        // SessionStart
        let session_handler = SessionStartHandler::new();
        let session_output = session_handler
            .handle("")
            .expect("SessionStart hook should execute");
        serde_json::from_str::<serde_json::Value>(&session_output)
            .expect("SessionStart should return valid JSON");

        // UserPromptSubmit
        let prompt_handler = UserPromptHandler::new();
        let prompt_output = prompt_handler
            .handle(r#"{"prompt": "test"}"#)
            .expect("UserPromptSubmit hook should execute");
        serde_json::from_str::<serde_json::Value>(&prompt_output)
            .expect("UserPromptSubmit should return valid JSON");

        // PostToolUse
        let tool_handler = PostToolUseHandler::new();
        let tool_output = tool_handler
            .handle(r#"{"tool_name": "Test", "tool_input": {}, "tool_output": ""}"#)
            .expect("PostToolUse hook should execute");
        serde_json::from_str::<serde_json::Value>(&tool_output)
            .expect("PostToolUse should return valid JSON");

        // PreCompact
        let compact_handler = PreCompactHandler::new();
        let compact_output = compact_handler
            .handle(r#"{"sections": []}"#)
            .expect("PreCompact hook should execute");
        serde_json::from_str::<serde_json::Value>(&compact_output)
            .expect("PreCompact should return valid JSON");

        // Stop
        let stop_handler = StopHandler::new();
        let stop_output = stop_handler
            .handle(r"{}")
            .expect("Stop hook should execute");
        serde_json::from_str::<serde_json::Value>(&stop_output)
            .expect("Stop should return valid JSON");
    }

    #[test]
    fn test_hook_event_types() {
        assert_eq!(SessionStartHandler::new().event_type(), "SessionStart");
        assert_eq!(UserPromptHandler::new().event_type(), "UserPromptSubmit");
        assert_eq!(PostToolUseHandler::new().event_type(), "PostToolUse");
        assert_eq!(PreCompactHandler::new().event_type(), "PreCompact");
        assert_eq!(StopHandler::new().event_type(), "Stop");
    }
}
