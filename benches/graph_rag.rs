//! Benchmarks for Graph RAG service.
//!
//! Benchmark targets:
//! - Config creation: <1µs
//! - Score calculation: <10µs
//! - Search provenance tracking: <1µs
//! - Result filtering: <100µs for 100 results
//! - Graph expansion scoring: <50µs per entity
//!
//! These benchmarks focus on the computational overhead of the Graph RAG
//! service rather than I/O-bound operations like database queries.

// Criterion macros generate items without docs - this is expected for benchmarks
// Benchmarks use expect/unwrap for simplicity - panics are acceptable in benchmarks
#![allow(missing_docs)]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::excessive_nesting,
    clippy::doc_markdown,
    clippy::explicit_iter_loop
)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::collections::HashMap;
use std::hint::black_box;
use std::time::Duration;

use subcog::models::MemoryId;
use subcog::models::graph::EntityId;
use subcog::services::{ExpansionConfig, GraphRAGConfig, SearchProvenance};

// ============================================================================
// Configuration Benchmarks
// ============================================================================

/// Benchmarks configuration creation and modification.
fn bench_config_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_rag_config");
    group.measurement_time(Duration::from_secs(3));

    // Benchmark default config creation
    group.bench_function("default_config", |b| {
        b.iter(|| black_box(GraphRAGConfig::default()));
    });

    // Benchmark config with builder pattern
    group.bench_function("builder_pattern", |b| {
        b.iter(|| {
            black_box(
                GraphRAGConfig::new()
                    .with_max_depth(3)
                    .with_expansion_boost(1.5)
                    .with_max_query_entities(10)
                    .with_max_expansion_results(20),
            )
        });
    });

    // Benchmark expansion config creation
    group.bench_function("expansion_config", |b| {
        b.iter(|| black_box(ExpansionConfig::default()));
    });

    // Benchmark config from environment (with no vars set)
    group.bench_function("from_env", |b| {
        b.iter(|| black_box(GraphRAGConfig::from_env()));
    });

    group.finish();
}

// ============================================================================
// Provenance Tracking Benchmarks
// ============================================================================

/// Benchmarks provenance creation and pattern matching.
fn bench_provenance_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_rag_provenance");
    group.measurement_time(Duration::from_secs(3));

    // Benchmark semantic provenance creation
    group.bench_function("semantic_create", |b| {
        b.iter(|| black_box(SearchProvenance::Semantic));
    });

    // Benchmark graph expansion provenance creation
    group.bench_function("graph_expansion_create", |b| {
        let entity_id = EntityId::new("entity_123");
        b.iter(|| {
            black_box(SearchProvenance::GraphExpansion {
                source_entity: entity_id.clone(),
                hop_count: 2,
            })
        });
    });

    // Benchmark both provenance creation
    group.bench_function("both_create", |b| {
        let entity_id = EntityId::new("entity_456");
        b.iter(|| {
            black_box(SearchProvenance::Both {
                semantic_score: 850,
                source_entity: entity_id.clone(),
            })
        });
    });

    // Benchmark provenance pattern matching
    group.bench_function("pattern_match", |b| {
        let provenances = vec![
            SearchProvenance::Semantic,
            SearchProvenance::GraphExpansion {
                source_entity: EntityId::new("e1"),
                hop_count: 1,
            },
            SearchProvenance::Both {
                semantic_score: 900,
                source_entity: EntityId::new("e2"),
            },
        ];

        b.iter(|| {
            for p in &provenances {
                black_box(matches!(p, SearchProvenance::Semantic));
                black_box(matches!(p, SearchProvenance::GraphExpansion { .. }));
                black_box(matches!(p, SearchProvenance::Both { .. }));
            }
        });
    });

    group.finish();
}

// ============================================================================
// Score Calculation Benchmarks
// ============================================================================

/// Benchmarks score calculation for graph expansion.
fn bench_score_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_rag_scoring");
    group.measurement_time(Duration::from_secs(3));

    // Benchmark base score calculation
    group.bench_function("base_score", |b| {
        b.iter(|| {
            for hop_count in 0..10usize {
                #[allow(clippy::cast_precision_loss)]
                let score = 1.0 / (1.0 + hop_count as f32);
                black_box(score);
            }
        });
    });

    // Benchmark score with boost
    group.bench_function("boosted_score", |b| {
        let boost = 1.2f32;
        b.iter(|| {
            for hop_count in 0..10usize {
                #[allow(clippy::cast_precision_loss)]
                let base_score = 1.0 / (1.0 + hop_count as f32);
                let score = base_score * boost;
                black_box(score);
            }
        });
    });

    // Benchmark midpoint calculation (used for hybrid scoring)
    group.bench_function("midpoint_score", |b| {
        let scores: Vec<(f32, f32)> = (0..100)
            .map(|i| (i as f32 / 100.0, (100 - i) as f32 / 100.0))
            .collect();

        b.iter(|| {
            for (semantic, graph) in &scores {
                let combined = f32::midpoint(*semantic, *graph);
                black_box(combined);
            }
        });
    });

    // Benchmark score conversion to u32 (for provenance tracking)
    group.bench_function("score_to_int", |b| {
        let scores: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();

        b.iter(|| {
            for score in &scores {
                #[allow(clippy::cast_possible_truncation)]
                let int_score = (score.abs() * 1000.0) as u32;
                black_box(int_score);
            }
        });
    });

    group.finish();
}

// ============================================================================
// HashMap Operations Benchmarks
// ============================================================================

/// Benchmarks HashMap operations used in result merging.
fn bench_hashmap_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_rag_hashmap");
    group.measurement_time(Duration::from_secs(3));

    // Pre-generate memory IDs for benchmarks
    let memory_ids: Vec<MemoryId> = (0..100)
        .map(|i| MemoryId::new(format!("mem_{i:04}")))
        .collect();

    let entity_ids: Vec<EntityId> = (0..100)
        .map(|i| EntityId::new(format!("entity_{i:04}")))
        .collect();

    // Benchmark entry API with or_insert
    group.bench_with_input(
        BenchmarkId::new("entry_or_insert", 100),
        &(&memory_ids, &entity_ids),
        |b, (mem_ids, ent_ids)| {
            b.iter(|| {
                let mut map: HashMap<String, (f32, String, usize)> = HashMap::new();
                for (i, mem_id) in mem_ids.iter().enumerate() {
                    let key = mem_id.as_str().to_string();
                    let entity = ent_ids[i % ent_ids.len()].as_ref().to_string();
                    map.entry(key).or_insert((0.5, entity, 1));
                }
                black_box(map)
            });
        },
    );

    // Benchmark entry API with and_modify
    group.bench_with_input(
        BenchmarkId::new("entry_and_modify", 100),
        &(&memory_ids, &entity_ids),
        |b, (mem_ids, ent_ids)| {
            b.iter(|| {
                let mut map: HashMap<String, (f32, String, usize)> = HashMap::new();
                // First pass - insert all
                for (i, mem_id) in mem_ids.iter().enumerate() {
                    let key = mem_id.as_str().to_string();
                    let entity = ent_ids[i % ent_ids.len()].as_ref().to_string();
                    map.insert(key, (0.5, entity, 2));
                }
                // Second pass - modify existing
                for mem_id in mem_ids.iter() {
                    let key = mem_id.as_str().to_string();
                    map.entry(key).and_modify(|(score, _, hops)| {
                        if 1 < *hops {
                            *score = 0.8;
                            *hops = 1;
                        }
                    });
                }
                black_box(map)
            });
        },
    );

    // Benchmark get_mut operations
    group.bench_with_input(
        BenchmarkId::new("get_mut", 100),
        &memory_ids,
        |b, mem_ids| {
            b.iter(|| {
                let mut map: HashMap<String, f32> = HashMap::new();
                for mem_id in mem_ids {
                    map.insert(mem_id.as_str().to_string(), 0.5);
                }
                for mem_id in mem_ids {
                    if let Some(score) = map.get_mut(mem_id.as_str()) {
                        *score += 0.1;
                    }
                }
                black_box(map)
            });
        },
    );

    group.finish();
}

// ============================================================================
// Result Filtering Benchmarks
// ============================================================================

/// Benchmarks result filtering by provenance.
fn bench_result_filtering(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_rag_filtering");
    group.measurement_time(Duration::from_secs(3));

    // Generate mixed provenances
    let provenances: Vec<SearchProvenance> = (0..100)
        .map(|i| match i % 3 {
            0 => SearchProvenance::Semantic,
            1 => SearchProvenance::GraphExpansion {
                source_entity: EntityId::new(format!("e{i}")),
                hop_count: (i % 5) + 1,
            },
            _ => SearchProvenance::Both {
                semantic_score: (i * 10) as u32,
                source_entity: EntityId::new(format!("e{i}")),
            },
        })
        .collect();

    // Benchmark filtering for semantic only
    group.throughput(Throughput::Elements(100));
    group.bench_function("filter_semantic", |b| {
        b.iter(|| {
            let filtered: Vec<_> = provenances
                .iter()
                .filter(|p| matches!(p, SearchProvenance::Semantic))
                .collect();
            black_box(filtered)
        });
    });

    // Benchmark filtering for graph expansion only
    group.bench_function("filter_graph", |b| {
        b.iter(|| {
            let filtered: Vec<_> = provenances
                .iter()
                .filter(|p| matches!(p, SearchProvenance::GraphExpansion { .. }))
                .collect();
            black_box(filtered)
        });
    });

    // Benchmark filtering for hybrid (both)
    group.bench_function("filter_hybrid", |b| {
        b.iter(|| {
            let filtered: Vec<_> = provenances
                .iter()
                .filter(|p| matches!(p, SearchProvenance::Both { .. }))
                .collect();
            black_box(filtered)
        });
    });

    group.finish();
}

// ============================================================================
// Sorting Benchmarks
// ============================================================================

/// Benchmarks result sorting by score.
fn bench_result_sorting(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_rag_sorting");
    group.measurement_time(Duration::from_secs(3));

    // Generate scores for sorting
    let scores: Vec<f32> = (0..100).map(|i| (i as f32 / 100.0).sin().abs()).collect();

    // Benchmark sorting with partial_cmp
    group.throughput(Throughput::Elements(100));
    group.bench_function("sort_by_score", |b| {
        b.iter(|| {
            let mut sorted = scores.clone();
            sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
            black_box(sorted)
        });
    });

    // Benchmark sorting with total_cmp (alternative approach)
    group.bench_function("sort_by_total_cmp", |b| {
        b.iter(|| {
            let mut sorted = scores.clone();
            sorted.sort_by(|a, b| b.total_cmp(a));
            black_box(sorted)
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_config_creation,
    bench_provenance_tracking,
    bench_score_calculation,
    bench_hashmap_operations,
    bench_result_filtering,
    bench_result_sorting,
);

criterion_main!(benches);
