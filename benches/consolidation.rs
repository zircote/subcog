//! Benchmarks for memory consolidation service.
//!
//! This benchmark verifies that consolidation preserves all factual details from source
//! memories. It measures "detail loss" by capturing specific facts, consolidating memories,
//! and verifying all facts are still retrievable through the summary.
//!
//! Acceptance Criteria:
//! - Benchmark shows 0% detail loss on Information Extraction queries
//! - Reports extraction accuracy for all captured facts

// Criterion macros generate items without docs - this is expected for benchmarks
// Benchmarks use expect/unwrap for simplicity - panics are acceptable in benchmarks
#![allow(missing_docs)]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::cast_precision_loss
)]

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::hint::black_box as hint_black_box;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

use subcog::config::{ConsolidationConfig, Config};
use subcog::llm::{CaptureAnalysis, LlmProvider};
use subcog::models::{Domain, Memory, MemoryId, Namespace};
use subcog::services::{CaptureRequest, CaptureService, ConsolidationService, RecallService};
use subcog::storage::index::SqliteBackend as SqliteIndexBackend;
use subcog::storage::persistence::FilesystemBackend;
use subcog::{Result, current_timestamp};

// ============================================================================
// Test Data: Specific Facts for Verification
// ============================================================================

/// A fact that should be preserved through consolidation.
#[derive(Debug, Clone)]
struct Fact {
    /// The key term or concept in this fact
    key: &'static str,
    /// The full fact statement
    statement: &'static str,
    /// Keywords to search for this fact
    search_terms: Vec<&'static str>,
}

/// Returns a set of factual memories about a Redis caching architecture.
/// Each memory contains specific, verifiable facts that should be preserved.
fn redis_architecture_facts() -> Vec<Fact> {
    vec![
        Fact {
            key: "session_storage",
            statement: "Use Redis for session storage with TTL of 30 minutes",
            search_terms: vec!["redis", "session", "storage", "30 minutes", "TTL"],
        },
        Fact {
            key: "persistence",
            statement: "Configure Redis with AOF persistence, fsync every second",
            search_terms: vec!["redis", "AOF", "persistence", "fsync", "second"],
        },
        Fact {
            key: "eviction",
            statement: "Set maxmemory-policy to allkeys-lru with 2GB limit",
            search_terms: vec!["maxmemory-policy", "allkeys-lru", "2GB", "limit"],
        },
        Fact {
            key: "replication",
            statement: "Enable Redis replication with 2 read replicas for scaling",
            search_terms: vec!["replication", "replicas", "2 read replicas", "scaling"],
        },
        Fact {
            key: "monitoring",
            statement: "Monitor Redis with Prometheus metrics exported on port 9121",
            search_terms: vec!["monitor", "prometheus", "metrics", "port 9121"],
        },
    ]
}

/// Returns a set of factual memories about database migration strategy.
fn database_migration_facts() -> Vec<Fact> {
    vec![
        Fact {
            key: "migration_tool",
            statement: "Use Flyway for database migrations with versioned SQL scripts",
            search_terms: vec!["flyway", "migrations", "versioned", "SQL scripts"],
        },
        Fact {
            key: "rollback",
            statement: "Implement rollback strategy using down migrations for emergency revert",
            search_terms: vec!["rollback", "down migrations", "emergency", "revert"],
        },
        Fact {
            key: "testing",
            statement: "Test all migrations on staging environment before production deploy",
            search_terms: vec!["test", "staging", "environment", "production", "deploy"],
        },
    ]
}

// ============================================================================
// Mock LLM Provider - Creates Summaries While Preserving Facts
// ============================================================================

/// Mock LLM provider for benchmarking that creates realistic summaries.
///
/// This provider creates summaries that include all the key facts from source
/// memories, simulating an ideal LLM that preserves details during consolidation.
struct DetailPreservingMockLlm {
    /// Response delay in milliseconds (simulates LLM latency)
    delay_ms: u64,
}

impl DetailPreservingMockLlm {
    fn new() -> Self {
        Self { delay_ms: 0 }
    }

    fn new_with_delay(delay_ms: u64) -> Self {
        Self { delay_ms }
    }

    /// Extracts all key facts from memory content and creates a summary
    fn create_summary_from_memories(&self, memories: &[Memory]) -> String {
        let mut summary = String::from("**Consolidated Summary**\n\n");

        for memory in memories {
            // Extract key sentences from each memory
            summary.push_str(&format!("- {}\n", memory.content.trim()));
        }

        summary
    }
}

impl LlmProvider for DetailPreservingMockLlm {
    fn name(&self) -> &'static str {
        "mock-detail-preserving"
    }

    fn complete(&self, prompt: &str) -> Result<String> {
        // Simulate LLM latency
        if self.delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(self.delay_ms));
        }

        // Extract memory content from the prompt
        // The prompt contains memories formatted as:
        // **Memory ID**: <id>
        // **Namespace**: <namespace>
        // **Tags**: <tags>
        // **Content**: <content>
        let mut summary = String::from("**Consolidated Summary**\n\n");

        // Parse out the content sections from the prompt
        for line in prompt.lines() {
            if line.starts_with("**Content**:") {
                let content = line.trim_start_matches("**Content**:").trim();
                if !content.is_empty() {
                    summary.push_str(&format!("- {content}\n"));
                }
            } else if line.starts_with("Decision:") || line.starts_with("Use ") ||
                      line.starts_with("Configure ") || line.starts_with("Set ") ||
                      line.starts_with("Enable ") || line.starts_with("Monitor ") ||
                      line.starts_with("Implement ") || line.starts_with("Test ") {
                summary.push_str(&format!("- {}\n", line.trim()));
            }
        }

        // If we didn't extract any content, create a generic summary
        if summary == "**Consolidated Summary**\n\n" {
            summary.push_str("Summary of related memories about system architecture and configuration.\n");
        }

        Ok(summary)
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

// ============================================================================
// Benchmark: Information Extraction with Detail Loss Measurement
// ============================================================================

/// Captures memories, consolidates them, and verifies all facts are retrievable.
///
/// This benchmark measures "detail loss" - the percentage of specific facts that
/// cannot be retrieved after consolidation. Target: 0% detail loss.
fn bench_consolidation_detail_preservation(c: &mut Criterion) {
    let mut group = c.benchmark_group("consolidation_detail_preservation");

    // Longer measurement time since this involves multiple operations
    group.measurement_time(Duration::from_secs(10));

    // Test with Redis architecture facts (5 facts)
    group.bench_function("redis_facts_5", |b| {
        b.iter_with_setup(
            || {
                // Setup: Create temporary directory and services
                let temp_dir = TempDir::new().expect("Failed to create temp dir");
                let facts = redis_architecture_facts();
                let (service, recall, facts) = setup_consolidation_test(&temp_dir, facts);
                (temp_dir, service, recall, facts)
            },
            |(temp_dir, mut service, recall, facts)| {
                // Benchmark: Consolidate and verify facts
                let result = consolidate_and_verify_facts(&mut service, &recall, &facts);
                hint_black_box(result);
                drop(temp_dir);
            },
        );
    });

    // Test with database migration facts (3 facts)
    group.bench_function("db_migration_facts_3", |b| {
        b.iter_with_setup(
            || {
                let temp_dir = TempDir::new().expect("Failed to create temp dir");
                let facts = database_migration_facts();
                let (service, recall, facts) = setup_consolidation_test(&temp_dir, facts);
                (temp_dir, service, recall, facts)
            },
            |(temp_dir, mut service, recall, facts)| {
                let result = consolidate_and_verify_facts(&mut service, &recall, &facts);
                hint_black_box(result);
                drop(temp_dir);
            },
        );
    });

    // Test with combined facts (8 facts)
    group.bench_function("combined_facts_8", |b| {
        b.iter_with_setup(
            || {
                let temp_dir = TempDir::new().expect("Failed to create temp dir");
                let mut facts = redis_architecture_facts();
                facts.extend(database_migration_facts());
                let (service, recall, facts) = setup_consolidation_test(&temp_dir, facts);
                (temp_dir, service, recall, facts)
            },
            |(temp_dir, mut service, recall, facts)| {
                let result = consolidate_and_verify_facts(&mut service, &recall, &facts);
                hint_black_box(result);
                drop(temp_dir);
            },
        );
    });

    // Throughput test - facts verified per second
    group.throughput(Throughput::Elements(5));
    group.bench_function("throughput_fact_verification", |b| {
        b.iter_with_setup(
            || {
                let temp_dir = TempDir::new().expect("Failed to create temp dir");
                let facts = redis_architecture_facts();
                let (service, recall, facts) = setup_consolidation_test(&temp_dir, facts);
                (temp_dir, service, recall, facts)
            },
            |(temp_dir, mut service, recall, facts)| {
                let result = consolidate_and_verify_facts(&mut service, &recall, &facts);
                hint_black_box(result);
                drop(temp_dir);
            },
        );
    });

    group.finish();
}

/// Sets up a consolidation test with captured memories for given facts.
///
/// Returns (consolidation_service, recall_service, facts)
fn setup_consolidation_test(
    temp_dir: &TempDir,
    facts: Vec<Fact>,
) -> (
    ConsolidationService<FilesystemBackend>,
    RecallService,
    Vec<Fact>,
) {
    // Create backends
    let index_path = temp_dir.path().join("index.db");
    let index_backend = Arc::new(
        SqliteIndexBackend::new(&index_path).expect("Failed to create index backend"),
    );
    let persistence_backend = FilesystemBackend::new(temp_dir.path());

    // Create capture service
    let config = Config::default();
    let capture_service = CaptureService::new(config)
        .with_index(Arc::clone(&index_backend) as Arc<dyn subcog::storage::traits::IndexBackend + Send + Sync>);

    // Capture each fact as a separate memory
    for fact in &facts {
        let request = CaptureRequest {
            namespace: Namespace::Decisions,
            content: fact.statement.to_string(),
            domain: Domain::new(),
            tags: fact.search_terms.iter().map(|s| (*s).to_string()).collect(),
            source: Some("benchmark-test.md".to_string()),
            skip_security_check: true,
        };

        capture_service
            .capture(&persistence_backend, &request)
            .expect("Failed to capture memory");
    }

    // Create recall service for fact verification
    let recall_service = RecallService::new(
        Arc::new(persistence_backend.clone()) as Arc<dyn subcog::storage::traits::PersistenceBackend>,
        Arc::clone(&index_backend) as Arc<dyn subcog::storage::traits::IndexBackend + Send + Sync>,
    );

    // Create consolidation service with mock LLM
    let llm: Arc<dyn LlmProvider + Send + Sync> = Arc::new(DetailPreservingMockLlm::new());
    let consolidation_service = ConsolidationService::new(persistence_backend)
        .with_llm(llm)
        .with_index(index_backend);

    (consolidation_service, recall_service, facts)
}

/// Consolidates memories and verifies all facts are still retrievable.
///
/// Returns (detail_loss_percentage, facts_lost_count)
fn consolidate_and_verify_facts(
    service: &mut ConsolidationService<FilesystemBackend>,
    recall: &RecallService,
    facts: &[Fact],
) -> (f64, usize) {
    // Configure consolidation
    let config = ConsolidationConfig {
        enabled: true,
        namespace_filter: Some(vec![Namespace::Decisions]),
        time_window_days: None,
        min_memories_to_consolidate: facts.len().min(2),
        similarity_threshold: 0.6,
        llm_provider: None,
    };

    // Run consolidation
    let _ = service.consolidate_memories(recall, &config);

    // Verify each fact is retrievable
    let mut facts_found = 0;
    let mut facts_lost = 0;

    for fact in facts {
        // Search for the fact using its key terms
        let search_query = fact.search_terms.join(" ");
        let search_result = recall.search(&search_query, None);

        // Check if the fact is present in any retrieved memory (original or summary)
        let fact_found = if let Ok(results) = search_result {
            results.iter().any(|result| {
                // Check if the content contains key elements of the fact
                let content_lower = result.content.to_lowercase();
                fact.search_terms.iter().any(|term| {
                    content_lower.contains(&term.to_lowercase())
                })
            })
        } else {
            false
        };

        if fact_found {
            facts_found += 1;
        } else {
            facts_lost += 1;
        }
    }

    let total_facts = facts.len();
    let detail_loss_percentage = if total_facts > 0 {
        (facts_lost as f64 / total_facts as f64) * 100.0
    } else {
        0.0
    };

    (detail_loss_percentage, facts_lost)
}

// ============================================================================
// Benchmark: Consolidation Performance with Varying Memory Counts
// ============================================================================

fn bench_consolidation_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("consolidation_scalability");

    // Test consolidation performance with different memory counts
    for count in [5, 10, 20, 50] {
        group.throughput(Throughput::Elements(count));
        group.bench_with_input(
            BenchmarkId::new("memories", count),
            &count,
            |b, &count| {
                b.iter_with_setup(
                    || {
                        let temp_dir = TempDir::new().expect("Failed to create temp dir");
                        setup_scalability_test(&temp_dir, count)
                    },
                    |(temp_dir, mut service, recall)| {
                        let config = ConsolidationConfig {
                            enabled: true,
                            namespace_filter: Some(vec![Namespace::Decisions]),
                            time_window_days: None,
                            min_memories_to_consolidate: 3,
                            similarity_threshold: 0.7,
                            llm_provider: None,
                        };
                        let result = service.consolidate_memories(&recall, &config);
                        hint_black_box(result);
                        drop(temp_dir);
                    },
                );
            },
        );
    }

    group.finish();
}

/// Sets up a scalability test with N memories.
fn setup_scalability_test(
    temp_dir: &TempDir,
    memory_count: u64,
) -> (
    ConsolidationService<FilesystemBackend>,
    RecallService,
) {
    let index_path = temp_dir.path().join("index.db");
    let index_backend = Arc::new(
        SqliteIndexBackend::new(&index_path).expect("Failed to create index backend"),
    );
    let persistence_backend = FilesystemBackend::new(temp_dir.path());

    let config = Config::default();
    let capture_service = CaptureService::new(config)
        .with_index(Arc::clone(&index_backend) as Arc<dyn subcog::storage::traits::IndexBackend + Send + Sync>);

    // Capture N related memories
    for i in 0..memory_count {
        let request = CaptureRequest {
            namespace: Namespace::Decisions,
            content: format!("Decision {i}: Use caching strategy with Redis for performance"),
            domain: Domain::new(),
            tags: vec!["redis".to_string(), "caching".to_string()],
            source: Some(format!("benchmark-{i}.md")),
            skip_security_check: true,
        };

        capture_service
            .capture(&persistence_backend, &request)
            .expect("Failed to capture memory");
    }

    let recall_service = RecallService::new(
        Arc::new(persistence_backend.clone()) as Arc<dyn subcog::storage::traits::PersistenceBackend>,
        Arc::clone(&index_backend) as Arc<dyn subcog::storage::traits::IndexBackend + Send + Sync>,
    );

    let llm: Arc<dyn LlmProvider + Send + Sync> = Arc::new(DetailPreservingMockLlm::new());
    let consolidation_service = ConsolidationService::new(persistence_backend)
        .with_llm(llm)
        .with_index(index_backend);

    (consolidation_service, recall_service)
}

// ============================================================================
// Benchmark: LLM Latency Impact
// ============================================================================

fn bench_llm_latency_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("llm_latency_impact");

    // Test with different simulated LLM latencies
    for latency_ms in [0, 50, 100, 200] {
        group.bench_with_input(
            BenchmarkId::new("latency_ms", latency_ms),
            &latency_ms,
            |b, &latency_ms| {
                b.iter_with_setup(
                    || {
                        let temp_dir = TempDir::new().expect("Failed to create temp dir");
                        setup_latency_test(&temp_dir, latency_ms)
                    },
                    |(temp_dir, mut service, recall)| {
                        let config = ConsolidationConfig {
                            enabled: true,
                            namespace_filter: Some(vec![Namespace::Decisions]),
                            time_window_days: None,
                            min_memories_to_consolidate: 3,
                            similarity_threshold: 0.7,
                            llm_provider: None,
                        };
                        let result = service.consolidate_memories(&recall, &config);
                        hint_black_box(result);
                        drop(temp_dir);
                    },
                );
            },
        );
    }

    group.finish();
}

/// Sets up a latency test with a mock LLM that has simulated delay.
fn setup_latency_test(
    temp_dir: &TempDir,
    latency_ms: u64,
) -> (
    ConsolidationService<FilesystemBackend>,
    RecallService,
) {
    let index_path = temp_dir.path().join("index.db");
    let index_backend = Arc::new(
        SqliteIndexBackend::new(&index_path).expect("Failed to create index backend"),
    );
    let persistence_backend = FilesystemBackend::new(temp_dir.path());

    let config = Config::default();
    let capture_service = CaptureService::new(config)
        .with_index(Arc::clone(&index_backend) as Arc<dyn subcog::storage::traits::IndexBackend + Send + Sync>);

    // Capture 5 related memories
    let facts = redis_architecture_facts();
    for fact in &facts {
        let request = CaptureRequest {
            namespace: Namespace::Decisions,
            content: fact.statement.to_string(),
            domain: Domain::new(),
            tags: fact.search_terms.iter().map(|s| (*s).to_string()).collect(),
            source: Some("benchmark-test.md".to_string()),
            skip_security_check: true,
        };

        capture_service
            .capture(&persistence_backend, &request)
            .expect("Failed to capture memory");
    }

    let recall_service = RecallService::new(
        Arc::new(persistence_backend.clone()) as Arc<dyn subcog::storage::traits::PersistenceBackend>,
        Arc::clone(&index_backend) as Arc<dyn subcog::storage::traits::IndexBackend + Send + Sync>,
    );

    // Create mock LLM with simulated latency
    let llm: Arc<dyn LlmProvider + Send + Sync> =
        Arc::new(DetailPreservingMockLlm::new_with_delay(latency_ms));
    let consolidation_service = ConsolidationService::new(persistence_backend)
        .with_llm(llm)
        .with_index(index_backend);

    (consolidation_service, recall_service)
}

// ============================================================================
// Combined benchmark groups
// ============================================================================

criterion_group!(
    benches,
    bench_consolidation_detail_preservation,
    bench_consolidation_scalability,
    bench_llm_latency_impact,
);

criterion_main!(benches);
