//! Benchmarks for search intent detection and related functionality.
//!
//! Benchmark targets:
//! - Keyword detection: <10ms
//! - LLM classification: <200ms (with mock)
//! - Memory retrieval with weights: <50ms
//! - Topic index building: <100ms for 1000 memories

// Criterion macros generate items without docs - this is expected for benchmarks
#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;

use subcog::Result;
use subcog::config::SearchIntentConfig;
use subcog::hooks::{
    AdaptiveContextConfig, MemoryContext, NamespaceWeights, SearchContextBuilder, SearchIntent,
    SearchIntentType, detect_search_intent, detect_search_intent_hybrid,
    detect_search_intent_with_timeout,
};
use subcog::llm::{CaptureAnalysis, LlmProvider};
use subcog::models::{MemoryId, Namespace};
use subcog::services::TopicIndexService;

// ============================================================================
// Task 6.6: Keyword Detection Benchmark - Target <10ms
// ============================================================================

/// Sample prompts of varying complexity for benchmark.
const SHORT_PROMPT: &str = "how to implement auth?";
const MEDIUM_PROMPT: &str =
    "How do I implement user authentication with OAuth2 for my web application?";
const LONG_PROMPT: &str = "I'm trying to figure out how to implement user authentication \
    in my application. I've been looking at OAuth2 and JWT options, and I'm not sure \
    which approach would be better for a microservices architecture. Can you help me \
    understand the differences and guide me through the implementation steps?";

fn bench_keyword_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("keyword_detection");

    // Set target time for quick iterations
    group.measurement_time(Duration::from_secs(5));

    // Test different prompt lengths
    group.bench_function("short_prompt", |b| {
        b.iter(|| detect_search_intent(black_box(SHORT_PROMPT)));
    });

    group.bench_function("medium_prompt", |b| {
        b.iter(|| detect_search_intent(black_box(MEDIUM_PROMPT)));
    });

    group.bench_function("long_prompt", |b| {
        b.iter(|| detect_search_intent(black_box(LONG_PROMPT)));
    });

    // Test different intent types
    let intent_prompts = [
        ("howto", "How do I create a new module?"),
        ("location", "Where is the database configuration?"),
        (
            "explanation",
            "What is the purpose of the ServiceContainer?",
        ),
        (
            "comparison",
            "What's the difference between git notes and SQLite?",
        ),
        (
            "troubleshoot",
            "Why is the authentication failing with this error?",
        ),
        ("general", "Search for memory implementations"),
    ];

    for (intent_name, prompt) in intent_prompts {
        group.bench_with_input(
            BenchmarkId::new("intent_type", intent_name),
            &prompt,
            |b, prompt| b.iter(|| detect_search_intent(black_box(prompt))),
        );
    }

    // Throughput test - prompts per second
    group.throughput(Throughput::Elements(1));
    group.bench_function("throughput", |b| {
        b.iter(|| {
            let _ = detect_search_intent(black_box(MEDIUM_PROMPT));
        });
    });

    group.finish();
}

// ============================================================================
// Task 6.7: LLM Classification Benchmark - Target <200ms
// ============================================================================

/// Mock LLM provider for benchmarking.
struct MockLlmProvider {
    response: String,
    delay_ms: u64,
}

impl MockLlmProvider {
    fn new_fast() -> Self {
        Self {
            response: r#"{"intent_type": "howto", "confidence": 0.9, "topics": ["auth", "oauth"]}"#
                .to_string(),
            delay_ms: 0,
        }
    }

    fn new_with_delay(delay_ms: u64) -> Self {
        Self {
            response: r#"{"intent_type": "howto", "confidence": 0.9, "topics": ["auth"]}"#
                .to_string(),
            delay_ms,
        }
    }
}

impl LlmProvider for MockLlmProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn complete(&self, _prompt: &str) -> Result<String> {
        if self.delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(self.delay_ms));
        }
        Ok(self.response.clone())
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

fn bench_llm_classification(c: &mut Criterion) {
    let mut group = c.benchmark_group("llm_classification");

    // Test with fast mock (measures overhead only)
    group.bench_function("mock_fast", |b| {
        let provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider::new_fast());
        let config = SearchIntentConfig::default().with_llm_timeout_ms(500);

        b.iter(|| {
            detect_search_intent_with_timeout(
                Some(Arc::clone(&provider)),
                black_box(MEDIUM_PROMPT),
                black_box(&config),
            )
        });
    });

    // Test hybrid detection with fast mock
    group.bench_function("hybrid_fast", |b| {
        let provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider::new_fast());
        let config = SearchIntentConfig::default().with_llm_timeout_ms(500);

        b.iter(|| {
            detect_search_intent_hybrid(
                Some(Arc::clone(&provider)),
                black_box(MEDIUM_PROMPT),
                black_box(&config),
            )
        });
    });

    // Test timeout behavior (should fall back to keyword detection quickly)
    group.measurement_time(Duration::from_secs(10));
    group.bench_function("timeout_fallback", |b| {
        let provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider::new_with_delay(500)); // Slow provider
        let config = SearchIntentConfig::default().with_llm_timeout_ms(50); // Short timeout

        b.iter(|| {
            detect_search_intent_with_timeout(
                Some(Arc::clone(&provider)),
                black_box(MEDIUM_PROMPT),
                black_box(&config),
            )
        });
    });

    // Test with LLM disabled (keyword only path)
    group.bench_function("llm_disabled", |b| {
        let provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider::new_fast());
        let config = SearchIntentConfig::default().with_use_llm(false);

        b.iter(|| {
            detect_search_intent_with_timeout(
                Some(Arc::clone(&provider)),
                black_box(MEDIUM_PROMPT),
                black_box(&config),
            )
        });
    });

    group.finish();
}

// ============================================================================
// Task 6.8: Memory Retrieval Benchmark - Target <50ms
// ============================================================================

fn bench_namespace_weights(c: &mut Criterion) {
    let mut group = c.benchmark_group("namespace_weights");

    // Benchmark weight creation for each intent type
    let intent_types = [
        SearchIntentType::HowTo,
        SearchIntentType::Location,
        SearchIntentType::Explanation,
        SearchIntentType::Comparison,
        SearchIntentType::Troubleshoot,
        SearchIntentType::General,
    ];

    for intent_type in intent_types {
        group.bench_with_input(
            BenchmarkId::new("create_weights", intent_type.as_str()),
            &intent_type,
            |b, &intent_type| b.iter(|| NamespaceWeights::for_intent(black_box(intent_type))),
        );
    }

    // Benchmark weight application
    group.bench_function("apply_weights", |b| {
        let weights = NamespaceWeights::for_intent(SearchIntentType::HowTo);
        let namespaces = [
            Namespace::Patterns,
            Namespace::Learnings,
            Namespace::Decisions,
            Namespace::Apis,
            Namespace::Config,
        ];

        b.iter(|| {
            for ns in &namespaces {
                black_box(weights.apply(ns, 0.75));
            }
        });
    });

    group.finish();
}

fn bench_context_building(c: &mut Criterion) {
    let mut group = c.benchmark_group("context_building");

    // Create test intent
    let intent = SearchIntent::new(SearchIntentType::HowTo, 0.85)
        .with_keywords(vec!["how to".to_string(), "implement".to_string()])
        .with_topics(vec![
            "authentication".to_string(),
            "oauth".to_string(),
            "security".to_string(),
        ]);

    // Benchmark context creation from intent
    group.bench_function("from_intent", |b| {
        b.iter(|| MemoryContext::from_intent(black_box(&intent)));
    });

    // Benchmark full context building (without recall service)
    group.bench_function("build_context_no_recall", |b| {
        let builder = SearchContextBuilder::new().with_config(AdaptiveContextConfig::default());

        b.iter(|| builder.build_context(black_box(&intent)));
    });

    // Benchmark with different confidence levels
    for confidence in [0.3, 0.5, 0.7, 0.9] {
        let test_intent = SearchIntent::new(SearchIntentType::HowTo, confidence)
            .with_topics(vec!["test".to_string()]);

        group.bench_with_input(
            BenchmarkId::new("confidence_level", format!("{confidence:.1}")),
            &test_intent,
            |b, intent| {
                let builder = SearchContextBuilder::new();
                b.iter(|| builder.build_context(black_box(intent)));
            },
        );
    }

    group.finish();
}

// ============================================================================
// Task 6.9: Topic Index Benchmark - Target <100ms for 1000 memories
// ============================================================================

/// Helper function to add memories to a topic index service.
fn populate_topic_index(service: &TopicIndexService, count: u64, tag_pattern: &str) {
    for i in 0..count {
        let id = MemoryId::new(format!("mem-{i}"));
        let tags = vec![format!("{tag_pattern}-{}", i % 20)];
        let _ = service.add_memory(&id, &tags, Namespace::Decisions);
    }
}

/// Helper function to add memories with varying tags and namespaces.
fn populate_topic_index_varied(service: &TopicIndexService, count: u64) {
    for i in 0..count {
        let id = MemoryId::new(format!("mem-{i}"));
        let tags = vec![format!("tag-{}", i % 10), format!("category-{}", i % 5)];
        let namespace = match i % 5 {
            0 => Namespace::Decisions,
            1 => Namespace::Patterns,
            2 => Namespace::Learnings,
            3 => Namespace::Context,
            _ => Namespace::Apis,
        };
        let _ = service.add_memory(&id, &tags, namespace);
    }
}

fn bench_topic_index(c: &mut Criterion) {
    let mut group = c.benchmark_group("topic_index");

    // Benchmark topic index creation
    group.bench_function("create_service", |b| {
        b.iter(TopicIndexService::new);
    });

    // Benchmark add_memory for varying counts
    for count in [10, 100, 1000] {
        group.throughput(Throughput::Elements(count));
        group.bench_with_input(
            BenchmarkId::new("add_memories", count),
            &count,
            |b, &count| {
                b.iter_with_setup(TopicIndexService::new, |service| {
                    populate_topic_index_varied(&service, count);
                });
            },
        );
    }

    // Benchmark list_topics after populating
    group.bench_function("list_topics_100", |b| {
        b.iter_with_setup(
            || {
                let service = TopicIndexService::new();
                populate_topic_index(&service, 100, "topic");
                service
            },
            |service| black_box(service.list_topics()),
        );
    });

    // Benchmark get_topic_memories
    group.bench_function("get_topic_memories", |b| {
        let service = TopicIndexService::new();
        for i in 0..100 {
            let id = MemoryId::new(format!("mem-{i}"));
            let tags = vec!["common-topic".to_string()];
            let _ = service.add_memory(&id, &tags, Namespace::Decisions);
        }

        b.iter(|| service.get_topic_memories(black_box("common-topic")));
    });

    // Benchmark get_topic_info
    group.bench_function("get_topic_info", |b| {
        let service = TopicIndexService::new();
        for i in 0..50 {
            let id = MemoryId::new(format!("mem-{i}"));
            let tags = vec!["test-topic".to_string()];
            let _ = service.add_memory(&id, &tags, Namespace::Patterns);
        }

        b.iter(|| service.get_topic_info(black_box("test-topic")));
    });

    group.finish();
}

// ============================================================================
// Combined benchmark groups
// ============================================================================

criterion_group!(
    benches,
    bench_keyword_detection,
    bench_llm_classification,
    bench_namespace_weights,
    bench_context_building,
    bench_topic_index,
);

criterion_main!(benches);
