//! Benchmarks for embedding generation.
//!
//! Benchmark targets:
//! - Cold start (first embed): <2s
//! - Warm embed (subsequent): <50ms
//! - Batch embed (10 texts): <200ms
//!
//! These benchmarks require the `fastembed-embeddings` feature to be enabled
//! for real semantic embedding performance testing. Without the feature,
//! the fallback hash-based implementation is benchmarked instead.

// Criterion macros generate items without docs - this is expected for benchmarks
// Benchmarks use expect/unwrap for simplicity - panics are acceptable in benchmarks
#![allow(missing_docs)]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::time::Duration;

use subcog::embedding::{Embedder, FastEmbedEmbedder, cosine_similarity};

// ============================================================================
// Test Data
// ============================================================================

/// Short text for quick embedding tests.
const SHORT_TEXT: &str = "database storage";

/// Medium text - typical query length.
const MEDIUM_TEXT: &str = "How do I implement user authentication with OAuth2?";

/// Long text - longer content for embedding.
const LONG_TEXT: &str = "I'm building a new web application that needs to handle \
    user authentication securely. The application will need to support multiple \
    OAuth2 providers including Google, GitHub, and Microsoft. I want to make sure \
    the implementation follows best practices for security and handles edge cases \
    like token expiration and refresh properly.";

/// Technical text for semantic similarity testing.
const TECH_TEXT_1: &str = "PostgreSQL database connection pooling with PgBouncer";
const TECH_TEXT_2: &str = "MySQL connection pool configuration";
const TECH_TEXT_UNRELATED: &str = "cat and dog are common household pets";

// ============================================================================
// Cold Start Benchmark - Target <2s
// ============================================================================

fn bench_cold_start(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedding_cold_start");

    // Use longer measurement time for cold start since it involves model loading
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(10); // Fewer samples for slow cold start

    // Note: We can't truly test "cold start" in criterion because the model
    // gets cached after first use. This test measures the first embed call
    // performance which includes model loading only on the very first run.
    group.bench_function("first_embed", |b| {
        b.iter_with_setup(FastEmbedEmbedder::new, |embedder| {
            let _ = embedder.embed(black_box(SHORT_TEXT));
        });
    });

    group.finish();
}

// ============================================================================
// Warm Embed Benchmark - Target <50ms
// ============================================================================

fn bench_warm_embed(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedding_warm");

    // Initialize embedder and warm up the model
    let embedder = FastEmbedEmbedder::new();
    let _ = embedder.embed("warmup text"); // Force model load

    group.measurement_time(Duration::from_secs(10));

    // Benchmark different text lengths
    group.bench_function("short_text", |b| {
        b.iter(|| embedder.embed(black_box(SHORT_TEXT)));
    });

    group.bench_function("medium_text", |b| {
        b.iter(|| embedder.embed(black_box(MEDIUM_TEXT)));
    });

    group.bench_function("long_text", |b| {
        b.iter(|| embedder.embed(black_box(LONG_TEXT)));
    });

    // Throughput test
    group.throughput(Throughput::Elements(1));
    group.bench_function("throughput", |b| {
        b.iter(|| {
            let _ = embedder.embed(black_box(MEDIUM_TEXT));
        });
    });

    group.finish();
}

// ============================================================================
// Batch Embed Benchmark - Target <200ms for 10 texts
// ============================================================================

fn bench_batch_embed(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedding_batch");

    // Initialize embedder
    let embedder = FastEmbedEmbedder::new();
    let _ = embedder.embed("warmup"); // Warm up model

    // Test different batch sizes
    let batch_sizes = [1, 5, 10, 20, 50];

    for size in batch_sizes {
        let texts: Vec<&str> = (0..size)
            .map(|i| match i % 3 {
                0 => SHORT_TEXT,
                1 => MEDIUM_TEXT,
                _ => LONG_TEXT,
            })
            .collect();

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("batch_size", size), &texts, |b, texts| {
            b.iter(|| embedder.embed_batch(black_box(texts)));
        });
    }

    // Batch vs sequential comparison (10 texts)
    let ten_texts: Vec<&str> = (0..10).map(|_| MEDIUM_TEXT).collect();

    group.bench_function("batch_10_sequential", |b| {
        b.iter(|| {
            for text in &ten_texts {
                let _ = embedder.embed(black_box(text));
            }
        });
    });

    group.bench_function("batch_10_batched", |b| {
        b.iter(|| embedder.embed_batch(black_box(&ten_texts)));
    });

    group.finish();
}

// ============================================================================
// Cosine Similarity Benchmark
// ============================================================================

fn bench_cosine_similarity(c: &mut Criterion) {
    let mut group = c.benchmark_group("cosine_similarity");

    // Create test embeddings
    let embedder = FastEmbedEmbedder::new();
    let emb1 = embedder.embed(TECH_TEXT_1).expect("embed failed");
    let emb2 = embedder.embed(TECH_TEXT_2).expect("embed failed");

    group.bench_function("compute_similarity", |b| {
        b.iter(|| cosine_similarity(black_box(&emb1), black_box(&emb2)));
    });

    // Throughput - similarities per second
    group.throughput(Throughput::Elements(1));
    group.bench_function("throughput", |b| {
        b.iter(|| {
            let _ = cosine_similarity(black_box(&emb1), black_box(&emb2));
        });
    });

    group.finish();
}

// ============================================================================
// Semantic Similarity Benchmark (end-to-end)
// ============================================================================

fn bench_semantic_similarity(c: &mut Criterion) {
    let mut group = c.benchmark_group("semantic_similarity");

    let embedder = FastEmbedEmbedder::new();
    let _ = embedder.embed("warmup");

    // End-to-end: embed two texts and compute similarity
    group.bench_function("embed_and_compare", |b| {
        b.iter(|| {
            let emb1 = embedder
                .embed(black_box(TECH_TEXT_1))
                .expect("embed failed");
            let emb2 = embedder
                .embed(black_box(TECH_TEXT_2))
                .expect("embed failed");
            cosine_similarity(&emb1, &emb2)
        });
    });

    // Compare related vs unrelated (quality check, not speed)
    group.bench_function("related_vs_unrelated", |b| {
        let emb_base = embedder.embed(TECH_TEXT_1).expect("embed failed");

        b.iter(|| {
            let emb_related = embedder
                .embed(black_box(TECH_TEXT_2))
                .expect("embed failed");
            let emb_unrelated = embedder
                .embed(black_box(TECH_TEXT_UNRELATED))
                .expect("embed failed");

            let sim_related = cosine_similarity(&emb_base, &emb_related);
            let sim_unrelated = cosine_similarity(&emb_base, &emb_unrelated);

            // Sanity check: related should be more similar
            assert!(
                sim_related > sim_unrelated,
                "Related ({sim_related}) should be > unrelated ({sim_unrelated})"
            );

            (sim_related, sim_unrelated)
        });
    });

    group.finish();
}

// ============================================================================
// Dimension Check Benchmark
// ============================================================================

fn bench_dimensions(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedding_dimensions");

    let embedder = FastEmbedEmbedder::new();

    group.bench_function("dimensions_call", |b| {
        b.iter(|| embedder.dimensions());
    });

    // Verify dimensions are correct (384 for all-MiniLM-L6-v2)
    group.bench_function("dimensions_verify", |b| {
        b.iter(|| {
            let dims = embedder.dimensions();
            assert_eq!(dims, 384, "Expected 384 dimensions for all-MiniLM-L6-v2");
            dims
        });
    });

    group.finish();
}

// ============================================================================
// Combined benchmark groups
// ============================================================================

criterion_group!(
    benches,
    bench_cold_start,
    bench_warm_embed,
    bench_batch_embed,
    bench_cosine_similarity,
    bench_semantic_similarity,
    bench_dimensions,
);

criterion_main!(benches);
