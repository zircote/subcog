//! Benchmarks for search operations.
//!
//! Benchmark targets:
//! - 100 memories: <20ms
//! - 1,000 memories: <50ms
//! - 10,000 memories: <100ms
//!
//! These benchmarks test the full search pipeline including:
//! - Query embedding generation
//! - Vector similarity search
//! - Text (BM25) search
//! - RRF fusion
//! - Score normalization

// Criterion macros generate items without docs - this is expected for benchmarks
// Benchmarks use expect/unwrap for simplicity - panics are acceptable in benchmarks
#![allow(missing_docs)]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::print_stderr)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

use subcog::embedding::FastEmbedEmbedder;
use subcog::storage::index::SqliteBackend;
use subcog::storage::vector::UsearchBackend;
use subcog::{
    CaptureRequest, CaptureService, Domain, Namespace, RecallService, SearchFilter, SearchMode,
};

// ============================================================================
// Helper Functions
// ============================================================================

/// Creates a capture service with all backends.
fn create_capture_service(temp_dir: &TempDir) -> CaptureService {
    let config = subcog::config::Config::default();
    let embedder: Arc<dyn subcog::Embedder> = Arc::new(FastEmbedEmbedder::new());

    let index_path = temp_dir.path().join("bench_index.db");
    let index: Arc<dyn subcog::IndexBackend + Send + Sync> =
        Arc::new(SqliteBackend::new(&index_path).expect("Failed to create SQLite index"));

    let vector_path = temp_dir.path().join("bench_vectors");
    #[cfg(feature = "usearch-hnsw")]
    let vector: Arc<dyn subcog::VectorBackend + Send + Sync> = Arc::new(
        UsearchBackend::new(&vector_path, FastEmbedEmbedder::DEFAULT_DIMENSIONS)
            .expect("Failed to create vector backend"),
    );
    #[cfg(not(feature = "usearch-hnsw"))]
    let vector: Arc<dyn subcog::VectorBackend + Send + Sync> = Arc::new(UsearchBackend::new(
        &vector_path,
        FastEmbedEmbedder::DEFAULT_DIMENSIONS,
    ));

    CaptureService::with_backends(config, embedder, index, vector)
}

/// Creates a recall service with the same backends.
fn create_recall_service(temp_dir: &TempDir) -> RecallService {
    let embedder: Arc<dyn subcog::Embedder> = Arc::new(FastEmbedEmbedder::new());

    let index_path = temp_dir.path().join("bench_index.db");
    let index = SqliteBackend::new(&index_path).expect("Failed to create SQLite index");

    let vector_path = temp_dir.path().join("bench_vectors");
    #[cfg(feature = "usearch-hnsw")]
    let vector: Arc<dyn subcog::VectorBackend + Send + Sync> = Arc::new(
        UsearchBackend::new(&vector_path, FastEmbedEmbedder::DEFAULT_DIMENSIONS)
            .expect("Failed to create vector backend"),
    );
    #[cfg(not(feature = "usearch-hnsw"))]
    let vector: Arc<dyn subcog::VectorBackend + Send + Sync> = Arc::new(UsearchBackend::new(
        &vector_path,
        FastEmbedEmbedder::DEFAULT_DIMENSIONS,
    ));

    RecallService::with_backends(index, embedder, vector)
}

/// Sample technical content for populating the index.
const SAMPLE_CONTENT: &[&str] = &[
    "PostgreSQL database configuration with connection pooling",
    "Redis caching layer implementation with TTL",
    "JWT authentication token validation flow",
    "Microservices architecture with event sourcing",
    "Kubernetes deployment configuration with autoscaling",
    "GraphQL API design patterns and best practices",
    "Docker container orchestration strategies",
    "CI/CD pipeline with GitHub Actions",
    "Performance optimization for Node.js applications",
    "Security audit checklist for web applications",
];

/// Populates the index with the specified number of memories.
fn populate_index(capture_service: &CaptureService, count: usize) {
    let namespaces = [
        Namespace::Decisions,
        Namespace::Patterns,
        Namespace::Learnings,
    ];

    for i in 0..count {
        let content_idx = i % SAMPLE_CONTENT.len();
        let namespace = namespaces[i % namespaces.len()];

        let request = CaptureRequest {
            namespace,
            content: format!("{} - instance {}", SAMPLE_CONTENT[content_idx], i),
            domain: Domain::new(),
            tags: vec!["benchmark".to_string()],
            source: None,
            skip_security_check: true,
            ttl_seconds: None,
            scope: None,
            #[cfg(feature = "group-scope")]
            group_id: None,
        };

        if let Err(e) = capture_service.capture(request) {
            eprintln!("Warning: Failed to capture memory {i}: {e}");
        }
    }
}

// ============================================================================
// Search Benchmarks
// ============================================================================

fn bench_search_100(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);
    let recall_service = create_recall_service(&temp_dir);

    // Populate with 100 memories
    populate_index(&capture_service, 100);

    let mut group = c.benchmark_group("search_100_memories");
    group.measurement_time(Duration::from_secs(10));

    // Text search
    group.bench_function("text_search", |b| {
        let filter = SearchFilter::default();
        b.iter(|| {
            recall_service
                .search("database configuration", SearchMode::Text, &filter, 10)
                .expect("Search should succeed")
        });
    });

    // Hybrid search
    group.bench_function("hybrid_search", |b| {
        let filter = SearchFilter::default();
        b.iter(|| {
            recall_service
                .search("database configuration", SearchMode::Hybrid, &filter, 10)
                .expect("Search should succeed")
        });
    });

    group.finish();
}

fn bench_search_1000(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);
    let recall_service = create_recall_service(&temp_dir);

    // Populate with 1000 memories
    populate_index(&capture_service, 1000);

    let mut group = c.benchmark_group("search_1000_memories");
    group.measurement_time(Duration::from_secs(15));

    // Text search
    group.bench_function("text_search", |b| {
        let filter = SearchFilter::default();
        b.iter(|| {
            recall_service
                .search("authentication security", SearchMode::Text, &filter, 10)
                .expect("Search should succeed")
        });
    });

    // Hybrid search
    group.bench_function("hybrid_search", |b| {
        let filter = SearchFilter::default();
        b.iter(|| {
            recall_service
                .search("authentication security", SearchMode::Hybrid, &filter, 10)
                .expect("Search should succeed")
        });
    });

    group.finish();
}

fn bench_search_10000(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);
    let recall_service = create_recall_service(&temp_dir);

    // Populate with 10000 memories - this takes a while
    populate_index(&capture_service, 10000);

    let mut group = c.benchmark_group("search_10000_memories");
    group.measurement_time(Duration::from_secs(20));

    // Text search
    group.bench_function("text_search", |b| {
        let filter = SearchFilter::default();
        b.iter(|| {
            recall_service
                .search("microservices architecture", SearchMode::Text, &filter, 10)
                .expect("Search should succeed")
        });
    });

    // Hybrid search - this is the critical path
    group.bench_function("hybrid_search", |b| {
        let filter = SearchFilter::default();
        b.iter(|| {
            recall_service
                .search(
                    "microservices architecture",
                    SearchMode::Hybrid,
                    &filter,
                    10,
                )
                .expect("Search should succeed")
        });
    });

    // Vector search
    group.bench_function("vector_search", |b| {
        let filter = SearchFilter::default();
        b.iter(|| {
            recall_service
                .search(
                    "microservices architecture",
                    SearchMode::Vector,
                    &filter,
                    10,
                )
                .expect("Search should succeed")
        });
    });

    group.finish();
}

fn bench_search_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_scaling");
    group.measurement_time(Duration::from_secs(10));

    for count in &[10, 50, 100, 500, 1000] {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let capture_service = create_capture_service(&temp_dir);
        let recall_service = create_recall_service(&temp_dir);

        populate_index(&capture_service, *count);

        group.bench_with_input(BenchmarkId::new("text_search", count), count, |b, _| {
            let filter = SearchFilter::default();
            b.iter(|| {
                recall_service
                    .search("kubernetes deployment", SearchMode::Text, &filter, 10)
                    .expect("Search should succeed")
            });
        });

        group.bench_with_input(BenchmarkId::new("hybrid_search", count), count, |b, _| {
            let filter = SearchFilter::default();
            b.iter(|| {
                recall_service
                    .search("kubernetes deployment", SearchMode::Hybrid, &filter, 10)
                    .expect("Search should succeed")
            });
        });
    }

    group.finish();
}

fn bench_search_modes(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let capture_service = create_capture_service(&temp_dir);
    let recall_service = create_recall_service(&temp_dir);

    // Populate with a moderate number of memories
    populate_index(&capture_service, 200);

    let mut group = c.benchmark_group("search_modes");
    group.measurement_time(Duration::from_secs(10));

    let filter = SearchFilter::default();

    group.bench_function("text_mode", |b| {
        b.iter(|| {
            recall_service
                .search("API design patterns", SearchMode::Text, &filter, 10)
                .expect("Search should succeed")
        });
    });

    group.bench_function("vector_mode", |b| {
        b.iter(|| {
            recall_service
                .search("API design patterns", SearchMode::Vector, &filter, 10)
                .expect("Search should succeed")
        });
    });

    group.bench_function("hybrid_mode", |b| {
        b.iter(|| {
            recall_service
                .search("API design patterns", SearchMode::Hybrid, &filter, 10)
                .expect("Search should succeed")
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_search_100,
    bench_search_1000,
    bench_search_10000,
    bench_search_scaling,
    bench_search_modes,
);
criterion_main!(benches);
