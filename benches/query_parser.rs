//! Benchmarks for query parser and filter operations (LOW-TEST-003).
//!
//! Benchmark targets:
//! - Simple query parsing: <1ms
//! - Complex query parsing: <5ms
//! - Filter serialization: <1ms
//! - Namespace parsing: <100us

// Criterion macros generate items without docs - this is expected for benchmarks
#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::time::Duration;

use subcog::models::Namespace;
use subcog::services::parse_filter_query;

// ============================================================================
// Query Parser Benchmarks
// ============================================================================

/// Sample queries of varying complexity.
const EMPTY_QUERY: &str = "";
const SIMPLE_QUERY: &str = "tag:rust";
const MEDIUM_QUERY: &str = "ns:decisions tag:rust,python status:active";
const COMPLEX_QUERY: &str = "ns:decisions ns:patterns tag:rust,python,typescript -tag:test -tag:draft since:7d source:src/**/*.rs status:active";
const VERY_COMPLEX_QUERY: &str = "ns:decisions ns:patterns ns:learnings ns:context \
    tag:rust,python,typescript,javascript,go -tag:test -tag:draft -tag:wip \
    since:30d source:src/**/*.rs status:active status:pending";

fn bench_query_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_parsing");
    group.measurement_time(Duration::from_secs(5));

    // Test parsing empty query
    group.bench_function("empty", |b| {
        b.iter(|| parse_filter_query(black_box(EMPTY_QUERY)));
    });

    // Test simple single-token query
    group.bench_function("simple", |b| {
        b.iter(|| parse_filter_query(black_box(SIMPLE_QUERY)));
    });

    // Test medium complexity query
    group.bench_function("medium", |b| {
        b.iter(|| parse_filter_query(black_box(MEDIUM_QUERY)));
    });

    // Test complex query with many tokens
    group.bench_function("complex", |b| {
        b.iter(|| parse_filter_query(black_box(COMPLEX_QUERY)));
    });

    // Test very complex query
    group.bench_function("very_complex", |b| {
        b.iter(|| parse_filter_query(black_box(VERY_COMPLEX_QUERY)));
    });

    // Throughput test
    group.throughput(Throughput::Elements(1));
    group.bench_function("throughput", |b| {
        b.iter(|| {
            let _ = parse_filter_query(black_box(MEDIUM_QUERY));
        });
    });

    group.finish();
}

fn bench_query_token_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_token_types");

    // Benchmark each token type
    let token_queries = [
        ("namespace", "ns:decisions"),
        ("tag_single", "tag:rust"),
        ("tag_multi", "tag:rust,python,typescript,go,java"),
        ("excluded_tag", "-tag:test"),
        ("status", "status:active"),
        ("source", "source:src/**/*.rs"),
        ("since_days", "since:7d"),
        ("since_hours", "since:24h"),
        ("since_minutes", "since:30m"),
    ];

    for (name, query) in token_queries {
        group.bench_with_input(BenchmarkId::new("parse", name), &query, |b, query| {
            b.iter(|| parse_filter_query(black_box(query)));
        });
    }

    group.finish();
}

fn bench_query_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_scaling");

    // Test how parsing scales with number of tokens
    for count in [1u64, 5, 10, 20, 50] {
        let query: String = (0..count)
            .map(|i| format!("tag:tag{i}"))
            .collect::<Vec<_>>()
            .join(" ");

        group.throughput(Throughput::Elements(count));
        group.bench_with_input(
            BenchmarkId::new("token_count", count),
            &query,
            |b, query| {
                b.iter(|| parse_filter_query(black_box(query)));
            },
        );
    }

    // Test how parsing scales with tag count in OR expressions
    for count in [2, 5, 10, 20, 50] {
        let tags: String = (0..count)
            .map(|i| format!("tag{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let query = format!("tag:{tags}");

        group.bench_with_input(
            BenchmarkId::new("or_tags_count", count),
            &query,
            |b, query| {
                b.iter(|| parse_filter_query(black_box(query)));
            },
        );
    }

    group.finish();
}

// ============================================================================
// Namespace Parsing Benchmarks
// ============================================================================

fn bench_namespace_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("namespace_parsing");

    // Test valid namespace parsing
    let valid_namespaces = [
        "decisions",
        "patterns",
        "learnings",
        "context",
        "tech-debt",
        "blockers",
        "progress",
        "apis",
        "config",
        "security",
        "performance",
    ];

    for ns in valid_namespaces {
        group.bench_with_input(BenchmarkId::new("valid", ns), &ns, |b, ns| {
            b.iter(|| Namespace::parse(black_box(ns)));
        });
    }

    // Test case-insensitive parsing
    group.bench_function("case_insensitive_upper", |b| {
        b.iter(|| Namespace::parse(black_box("DECISIONS")));
    });

    group.bench_function("case_insensitive_mixed", |b| {
        b.iter(|| Namespace::parse(black_box("DeCiSiOnS")));
    });

    // Test invalid namespace parsing
    group.bench_function("invalid", |b| {
        b.iter(|| Namespace::parse(black_box("invalid-namespace")));
    });

    // Benchmark Namespace::all()
    group.bench_function("all", |b| {
        b.iter(Namespace::all);
    });

    // Benchmark Namespace::user_namespaces()
    group.bench_function("user_namespaces", |b| {
        b.iter(Namespace::user_namespaces);
    });

    // Benchmark is_system check
    group.bench_function("is_system_check", |b| {
        let namespaces = Namespace::all();
        b.iter(|| {
            for ns in namespaces {
                black_box(ns.is_system());
            }
        });
    });

    group.finish();
}

// ============================================================================
// Filter Operations Benchmarks
// ============================================================================

fn bench_filter_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_operations");

    // Benchmark filter creation
    group.bench_function("create_empty", |b| {
        b.iter(subcog::SearchFilter::new);
    });

    // Benchmark filter with_namespace
    group.bench_function("with_namespace", |b| {
        b.iter(|| subcog::SearchFilter::new().with_namespace(black_box(Namespace::Decisions)));
    });

    // Benchmark filter with_tag
    group.bench_function("with_tag", |b| {
        b.iter(|| subcog::SearchFilter::new().with_tag(black_box("rust")));
    });

    // Benchmark chained filter operations
    group.bench_function("chained_operations", |b| {
        b.iter(|| {
            subcog::SearchFilter::new()
                .with_namespace(black_box(Namespace::Decisions))
                .with_namespace(black_box(Namespace::Patterns))
                .with_tag(black_box("rust"))
                .with_tag(black_box("python"))
        });
    });

    // Benchmark is_empty check
    group.bench_function("is_empty_true", |b| {
        let filter = subcog::SearchFilter::new();
        b.iter(|| black_box(&filter).is_empty());
    });

    group.bench_function("is_empty_false", |b| {
        let filter = subcog::SearchFilter::new().with_tag("test");
        b.iter(|| black_box(&filter).is_empty());
    });

    group.finish();
}

// ============================================================================
// Memory ID Benchmarks
// ============================================================================

fn bench_memory_id(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_id");

    // Benchmark MemoryId creation
    group.bench_function("new_short", |b| {
        b.iter(|| subcog::MemoryId::new(black_box("abc123")));
    });

    group.bench_function("new_long", |b| {
        b.iter(|| subcog::MemoryId::new(black_box("abc123def456ghi789jkl012mno345")));
    });

    // Benchmark MemoryId from String
    group.bench_function("from_string", |b| {
        let s = "test-memory-id-12345".to_string();
        b.iter(|| subcog::MemoryId::from(black_box(s.clone())));
    });

    // Benchmark as_str
    group.bench_function("as_str", |b| {
        let id = subcog::MemoryId::new("test-123");
        b.iter(|| black_box(&id).as_str());
    });

    // Benchmark comparison
    group.bench_function("eq", |b| {
        let id1 = subcog::MemoryId::new("test-123");
        let id2 = subcog::MemoryId::new("test-123");
        b.iter(|| black_box(&id1) == black_box(&id2));
    });

    // Benchmark hashing (for HashSet/HashMap use)
    group.bench_function("hash", |b| {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let id = subcog::MemoryId::new("test-123");
        b.iter(|| {
            let mut hasher = DefaultHasher::new();
            black_box(&id).hash(&mut hasher);
            hasher.finish()
        });
    });

    group.finish();
}

// ============================================================================
// Domain Benchmarks
// ============================================================================

fn bench_domain(c: &mut Criterion) {
    let mut group = c.benchmark_group("domain");

    // Benchmark Domain creation
    group.bench_function("new_global", |b| {
        b.iter(subcog::Domain::new);
    });

    group.bench_function("for_repository", |b| {
        b.iter(|| subcog::Domain::for_repository(black_box("zircote"), black_box("subcog")));
    });

    group.bench_function("for_user", |b| {
        b.iter(subcog::Domain::for_user);
    });

    // Benchmark is_project_scoped check
    group.bench_function("is_project_scoped_true", |b| {
        let domain = subcog::Domain::new();
        b.iter(|| black_box(&domain).is_project_scoped());
    });

    group.bench_function("is_project_scoped_false", |b| {
        let domain = subcog::Domain::for_repository("org", "repo");
        b.iter(|| black_box(&domain).is_project_scoped());
    });

    // Benchmark to_string
    group.bench_function("to_string_project", |b| {
        let domain = subcog::Domain::new();
        b.iter(|| black_box(&domain).to_string());
    });

    group.bench_function("to_string_repo", |b| {
        let domain = subcog::Domain::for_repository("zircote", "subcog");
        b.iter(|| black_box(&domain).to_string());
    });

    group.finish();
}

// ============================================================================
// Combined benchmark groups
// ============================================================================

criterion_group!(
    benches,
    bench_query_parsing,
    bench_query_token_types,
    bench_query_scaling,
    bench_namespace_parsing,
    bench_filter_operations,
    bench_memory_id,
    bench_domain,
);

criterion_main!(benches);
