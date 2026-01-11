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

/// Consolidation service integration tests with LLM providers.
///
/// Tests verify that consolidation works with different LLM providers:
/// - OpenAI (API key required, skipped if not available)
/// - Ollama (local server required, skipped if not running)
/// - Mock providers (always run)
mod consolidation_integration_tests {
    use std::sync::Arc;
    use subcog::config::ConsolidationConfig;
    use subcog::llm::{LlmProvider, OllamaClient, OpenAiClient};
    use subcog::models::{Domain, Memory, MemoryId, MemoryStatus, Namespace};
    use subcog::services::ConsolidationService;
    use subcog::storage::index::SqliteBackend as SqliteIndexBackend;
    use subcog::storage::persistence::FilesystemBackend;
    use subcog::Result;

    /// Helper to create a test memory with specified ID and content.
    fn create_test_memory(id: &str, content: &str, embedding: Option<Vec<f32>>) -> Memory {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Memory {
            id: MemoryId::new(id),
            content: content.to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: now,
            updated_at: now,
            tombstoned_at: None,
            embedding,
            tags: vec!["test".to_string()],
            source: None,
            is_summary: false,
            source_memory_ids: None,
            consolidation_timestamp: None,
        }
    }

    /// Mock LLM provider that returns predefined summaries.
    struct MockLlmProvider {
        summary: String,
    }

    impl MockLlmProvider {
        fn new(summary: impl Into<String>) -> Self {
            Self {
                summary: summary.into(),
            }
        }
    }

    impl LlmProvider for MockLlmProvider {
        fn name(&self) -> &'static str {
            "mock"
        }

        fn complete(&self, _prompt: &str) -> Result<String> {
            Ok(self.summary.clone())
        }

        fn analyze_for_capture(
            &self,
            _content: &str,
        ) -> Result<subcog::llm::CaptureAnalysis> {
            Ok(subcog::llm::CaptureAnalysis {
                should_capture: true,
                confidence: 0.8,
                suggested_namespace: Some("decisions".to_string()),
                suggested_tags: vec![],
                reasoning: "Mock analysis".to_string(),
            })
        }
    }

    #[test]
    fn test_consolidation_with_mock_provider() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let backend = FilesystemBackend::new(temp_dir.path());

        // Create and store test memories with embeddings for similarity matching
        let memory1 = create_test_memory(
            "mem_1",
            "Use PostgreSQL for primary storage with connection pooling",
            Some(vec![0.1, 0.2, 0.3]),
        );
        let memory2 = create_test_memory(
            "mem_2",
            "Enable JSONB support for flexible schema",
            Some(vec![0.1, 0.2, 0.35]),
        );
        let memory3 = create_test_memory(
            "mem_3",
            "Configure pgbouncer for connection management",
            Some(vec![0.15, 0.2, 0.3]),
        );

        backend
            .store(&memory1)
            .expect("Failed to store memory1");
        backend
            .store(&memory2)
            .expect("Failed to store memory2");
        backend
            .store(&memory3)
            .expect("Failed to store memory3");

        // Create consolidation service with mock LLM
        let llm: Arc<dyn LlmProvider + Send + Sync> = Arc::new(MockLlmProvider::new(
            "Summary: Database architecture using PostgreSQL with JSONB support and connection pooling via pgbouncer.",
        ));
        let service = ConsolidationService::new(backend).with_llm(llm);

        // Create recall service for finding related memories
        let recall = subcog::services::RecallService::new();

        // Use default config with low similarity threshold for testing
        let config = ConsolidationConfig::new()
            .with_similarity_threshold(0.7)
            .with_min_memories_to_consolidate(2);

        // Find related memories
        let groups = service
            .find_related_memories(&recall, &config)
            .expect("Failed to find related memories");

        assert!(
            !groups.is_empty(),
            "Should find at least one group of related memories"
        );

        // Summarize the first group
        if let Some(group) = groups.first() {
            let summary = service
                .summarize_group(group)
                .expect("Failed to summarize group");

            assert!(
                summary.contains("PostgreSQL") || summary.contains("database"),
                "Summary should contain relevant terms: {summary}"
            );
        }
    }

    #[test]
    fn test_consolidation_with_openai_provider() {
        // Skip if OPENAI_API_KEY not set
        if std::env::var("OPENAI_API_KEY").is_err() {
            eprintln!("Skipping OpenAI consolidation test - OPENAI_API_KEY not set");
            return;
        }

        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let backend = FilesystemBackend::new(temp_dir.path());

        // Create test memories about database decisions with embeddings
        let memory1 = create_test_memory(
            "openai_mem_1",
            "Decision: Use PostgreSQL as the primary database for better JSONB support and ACID compliance",
            Some(vec![0.5, 0.6, 0.7]),
        );
        let memory2 = create_test_memory(
            "openai_mem_2",
            "Decision: Enable pgbouncer for connection pooling to handle high concurrency",
            Some(vec![0.5, 0.6, 0.75]),
        );

        backend
            .store(&memory1)
            .expect("Failed to store memory1");
        backend
            .store(&memory2)
            .expect("Failed to store memory2");

        // Create OpenAI client with default configuration
        let openai_client = OpenAiClient::new();
        let llm: Arc<dyn LlmProvider + Send + Sync> = Arc::new(openai_client);

        let service = ConsolidationService::new(backend).with_llm(llm);

        // Create test memories for summarization
        let memories = vec![memory1, memory2];

        // Test summarization with OpenAI
        let result = service.summarize_group(&memories);

        match result {
            Ok(summary) => {
                assert!(!summary.is_empty(), "Summary should not be empty");
                assert!(
                    summary.len() > 20,
                    "Summary should be reasonably detailed"
                );
                // Check that summary contains relevant terms
                let summary_lower = summary.to_lowercase();
                assert!(
                    summary_lower.contains("postgresql")
                        || summary_lower.contains("database")
                        || summary_lower.contains("connection"),
                    "Summary should contain relevant database terms: {summary}"
                );
                println!("OpenAI consolidation test passed. Summary: {summary}");
            },
            Err(e) => {
                panic!("OpenAI consolidation failed: {e}");
            },
        }
    }

    #[test]
    fn test_consolidation_end_to_end_with_openai() {
        // Skip if OPENAI_API_KEY not set
        if std::env::var("OPENAI_API_KEY").is_err() {
            eprintln!("Skipping OpenAI end-to-end consolidation test - OPENAI_API_KEY not set");
            return;
        }

        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let backend = FilesystemBackend::new(temp_dir.path());

        // Create SQLite index backend for edge storage
        let index_path = temp_dir.path().join("index.db");
        let index_backend = SqliteIndexBackend::new(&index_path).expect("Failed to create index backend");

        // Create and store test memories with embeddings
        let memory1 = create_test_memory(
            "e2e_mem_1",
            "Use Redis for caching to reduce database load",
            Some(vec![0.8, 0.9, 1.0]),
        );
        let memory2 = create_test_memory(
            "e2e_mem_2",
            "Configure Redis with persistence enabled for durability",
            Some(vec![0.8, 0.9, 1.05]),
        );
        let memory3 = create_test_memory(
            "e2e_mem_3",
            "Set up Redis sentinel for high availability",
            Some(vec![0.85, 0.9, 1.0]),
        );

        backend
            .store(&memory1)
            .expect("Failed to store memory1");
        backend
            .store(&memory2)
            .expect("Failed to store memory2");
        backend
            .store(&memory3)
            .expect("Failed to store memory3");

        // Create consolidation service with OpenAI and index backend
        let openai_client = OpenAiClient::new();
        let llm: Arc<dyn LlmProvider + Send + Sync> = Arc::new(openai_client);
        let service = ConsolidationService::new(backend)
            .with_llm(llm)
            .with_index(Arc::new(index_backend));

        // Create recall service
        let recall = subcog::services::RecallService::new();

        // Configure consolidation
        let config = ConsolidationConfig::new()
            .with_similarity_threshold(0.7)
            .with_min_memories_to_consolidate(2)
            .with_namespace_filter(vec![Namespace::Decisions]);

        // Run end-to-end consolidation
        let result = service.consolidate_memories(&recall, &config);

        match result {
            Ok(stats) => {
                println!("Consolidation stats: {}", stats.summary());
                assert!(
                    stats.processed > 0,
                    "Should have processed some memories"
                );
                // Note: summaries_created may be 0 if LLM fails or similarity threshold not met
                println!("OpenAI end-to-end consolidation test passed");
            },
            Err(e) => {
                panic!("End-to-end consolidation failed: {e}");
            },
        }
    }

    #[test]
    fn test_consolidation_creates_summary_with_source_ids() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let backend = FilesystemBackend::new(temp_dir.path());

        // Create test memories
        let memory1 = create_test_memory("sum_mem_1", "First decision", None);
        let memory2 = create_test_memory("sum_mem_2", "Second decision", None);

        let mem1_id = memory1.id.clone();
        let mem2_id = memory2.id.clone();

        backend
            .store(&memory1)
            .expect("Failed to store memory1");
        backend
            .store(&memory2)
            .expect("Failed to store memory2");

        // Create mock LLM
        let llm: Arc<dyn LlmProvider + Send + Sync> = Arc::new(MockLlmProvider::new(
            "Combined summary of both decisions",
        ));
        let service = ConsolidationService::new(backend.clone()).with_llm(llm);

        // Create summary node
        let summary_content = "This is a test summary";
        let source_memories = vec![memory1, memory2];

        let result = service.create_summary_node(summary_content, &source_memories);

        assert!(result.is_ok(), "Failed to create summary node");

        let summary = result.expect("Should have summary");

        // Verify summary fields
        assert!(summary.is_summary, "Should be marked as summary");
        assert_eq!(summary.content, summary_content);
        assert!(
            summary.source_memory_ids.is_some(),
            "Should have source memory IDs"
        );

        let source_ids = summary.source_memory_ids.expect("Should have source IDs");
        assert_eq!(source_ids.len(), 2, "Should have 2 source IDs");
        assert!(
            source_ids.contains(&mem1_id),
            "Should contain first memory ID"
        );
        assert!(
            source_ids.contains(&mem2_id),
            "Should contain second memory ID"
        );

        // Verify it was stored
        let retrieved = backend.get(&summary.id);
        assert!(retrieved.is_ok(), "Failed to retrieve summary from backend");
        let retrieved_summary = retrieved.expect("Should retrieve summary");
        assert_eq!(retrieved_summary.id, summary.id);
        assert!(retrieved_summary.is_summary);
    }

    #[test]
    fn test_consolidation_with_ollama_provider() {
        // Create Ollama client and check if available
        let ollama_client = OllamaClient::new();
        if !ollama_client.is_available() {
            eprintln!("Skipping Ollama consolidation test - Ollama server not running");
            eprintln!("To run this test: start Ollama server with 'ollama serve' or Docker");
            return;
        }

        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let backend = FilesystemBackend::new(temp_dir.path());

        // Create test memories about caching decisions with embeddings
        let memory1 = create_test_memory(
            "ollama_mem_1",
            "Decision: Use Redis for caching with LRU eviction policy for better memory management",
            Some(vec![0.3, 0.4, 0.5]),
        );
        let memory2 = create_test_memory(
            "ollama_mem_2",
            "Decision: Configure Redis with maxmemory-policy allkeys-lru for automatic eviction",
            Some(vec![0.3, 0.4, 0.55]),
        );

        backend
            .store(&memory1)
            .expect("Failed to store memory1");
        backend
            .store(&memory2)
            .expect("Failed to store memory2");

        // Create Ollama client
        let llm: Arc<dyn LlmProvider + Send + Sync> = Arc::new(ollama_client);

        let service = ConsolidationService::new(backend).with_llm(llm);

        // Create test memories for summarization
        let memories = vec![memory1, memory2];

        // Test summarization with Ollama
        let result = service.summarize_group(&memories);

        match result {
            Ok(summary) => {
                assert!(!summary.is_empty(), "Summary should not be empty");
                assert!(
                    summary.len() > 20,
                    "Summary should be reasonably detailed"
                );
                // Check that summary contains relevant terms
                let summary_lower = summary.to_lowercase();
                assert!(
                    summary_lower.contains("redis")
                        || summary_lower.contains("cache")
                        || summary_lower.contains("lru")
                        || summary_lower.contains("memory"),
                    "Summary should contain relevant caching terms: {summary}"
                );
                println!("Ollama consolidation test passed. Summary: {summary}");
            },
            Err(e) => {
                panic!("Ollama consolidation failed: {e}");
            },
        }
    }

    #[test]
    fn test_consolidation_end_to_end_with_ollama() {
        // Create Ollama client and check if available
        let ollama_client = OllamaClient::new();
        if !ollama_client.is_available() {
            eprintln!("Skipping Ollama end-to-end consolidation test - Ollama server not running");
            eprintln!("To run this test: start Ollama server with 'ollama serve' or Docker");
            return;
        }

        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let backend = FilesystemBackend::new(temp_dir.path());

        // Create SQLite index backend for edge storage
        let index_path = temp_dir.path().join("index.db");
        let index_backend = SqliteIndexBackend::new(&index_path).expect("Failed to create index backend");

        // Create and store test memories with embeddings
        let memory1 = create_test_memory(
            "e2e_ollama_mem_1",
            "Use in-memory caching to reduce API latency",
            Some(vec![0.6, 0.7, 0.8]),
        );
        let memory2 = create_test_memory(
            "e2e_ollama_mem_2",
            "Configure cache TTL to 300 seconds for optimal performance",
            Some(vec![0.6, 0.7, 0.85]),
        );
        let memory3 = create_test_memory(
            "e2e_ollama_mem_3",
            "Implement cache warming on service startup",
            Some(vec![0.65, 0.7, 0.8]),
        );

        backend
            .store(&memory1)
            .expect("Failed to store memory1");
        backend
            .store(&memory2)
            .expect("Failed to store memory2");
        backend
            .store(&memory3)
            .expect("Failed to store memory3");

        // Create consolidation service with Ollama and index backend
        let llm: Arc<dyn LlmProvider + Send + Sync> = Arc::new(ollama_client);
        let service = ConsolidationService::new(backend)
            .with_llm(llm)
            .with_index(Arc::new(index_backend));

        // Create recall service
        let recall = subcog::services::RecallService::new();

        // Configure consolidation
        let config = ConsolidationConfig::new()
            .with_similarity_threshold(0.7)
            .with_min_memories_to_consolidate(2)
            .with_namespace_filter(vec![Namespace::Decisions]);

        // Run end-to-end consolidation
        let result = service.consolidate_memories(&recall, &config);

        match result {
            Ok(stats) => {
                println!("Consolidation stats: {}", stats.summary());
                assert!(
                    stats.processed > 0,
                    "Should have processed some memories"
                );
                // Note: summaries_created may be 0 if LLM fails or similarity threshold not met
                println!("Ollama end-to-end consolidation test passed");
            },
            Err(e) => {
                panic!("End-to-end consolidation with Ollama failed: {e}");
            },
        }
    }
}
