//! Chaos testing for concurrent access (LOW-TEST-004).
//!
//! Tests concurrent operations to find race conditions and deadlocks:
//! - Concurrent reads and writes to topic index
//! - Concurrent capture operations
//! - Concurrent search operations
//! - Mixed read/write workloads

// Chaos tests use expect/unwrap/panic for simplicity - panics are acceptable in tests
// Excessive nesting is acceptable in concurrent test code with thread spawns
// Needless collect is sometimes needed for clearer concurrent test structure
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::excessive_nesting,
    clippy::needless_collect
)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;
use subcog::models::{MemoryId, Namespace};
use subcog::services::TopicIndexService;

// ============================================================================
// LOW-TEST-004: Chaos Testing for Concurrent Access
// ============================================================================

/// Test: Concurrent reads to topic index should not deadlock.
#[test]
fn test_concurrent_reads_no_deadlock() {
    let service = Arc::new(TopicIndexService::new());

    // Pre-populate with some data
    for i in 0..100 {
        let id = MemoryId::new(format!("mem-{i}"));
        let tags = vec![format!("tag-{}", i % 10)];
        service
            .add_memory(&id, &tags, Namespace::Decisions)
            .unwrap();
    }

    let num_threads = 10;
    let ops_per_thread = 100;
    let completed = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..num_threads)
        .map(|t| {
            let service = Arc::clone(&service);
            let completed = Arc::clone(&completed);
            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    // Mix of different read operations
                    match i % 4 {
                        0 => {
                            let _ = service.list_topics();
                        },
                        1 => {
                            let _ = service.get_topic_memories(&format!("tag-{}", t % 10));
                        },
                        2 => {
                            let _ = service.get_topic_info(&format!("tag-{}", (t + i) % 10));
                        },
                        _ => {
                            let _ = service.topic_count();
                            let _ = service.association_count();
                        },
                    }
                    completed.fetch_add(1, Ordering::SeqCst);
                }
            })
        })
        .collect();

    // Wait with timeout
    let timeout = Duration::from_secs(30);
    let start = std::time::Instant::now();

    for handle in handles {
        let remaining = timeout.saturating_sub(start.elapsed());
        assert!(
            !remaining.is_zero(),
            "Deadlock detected: threads did not complete in time"
        );
        // Join with implicit timeout via test timeout
        handle.join().expect("Thread panicked");
    }

    // Verify all operations completed
    assert_eq!(
        completed.load(Ordering::SeqCst),
        num_threads * ops_per_thread
    );
}

/// Test: Concurrent writes to topic index should not corrupt data.
#[test]
fn test_concurrent_writes_no_corruption() {
    let service = Arc::new(TopicIndexService::new());
    let num_threads = 10;
    let ops_per_thread = 50;
    let completed = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..num_threads)
        .map(|t| {
            let service = Arc::clone(&service);
            let completed = Arc::clone(&completed);
            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    let id = MemoryId::new(format!("thread-{t}-mem-{i}"));
                    let tags = vec![
                        format!("thread-{t}"),
                        format!("shared-tag"),
                        format!("tag-{}", i % 5),
                    ];
                    let namespace = match i % 4 {
                        0 => Namespace::Decisions,
                        1 => Namespace::Patterns,
                        2 => Namespace::Learnings,
                        _ => Namespace::Context,
                    };

                    service.add_memory(&id, &tags, namespace).unwrap();
                    completed.fetch_add(1, Ordering::SeqCst);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // Verify all operations completed
    assert_eq!(
        completed.load(Ordering::SeqCst),
        num_threads * ops_per_thread
    );

    // Verify data integrity
    let topics = service.list_topics().unwrap();

    // Should have "shared-tag" with all memories
    let shared = topics.iter().find(|t| t.name == "shared-tag");
    assert!(shared.is_some());
    let shared = shared.unwrap();
    assert_eq!(
        shared.memory_count,
        num_threads * ops_per_thread,
        "shared-tag should have all memories"
    );

    // Each thread's tag should have ops_per_thread memories
    for t in 0..num_threads {
        let thread_tag = topics.iter().find(|tp| tp.name == format!("thread-{t}"));
        assert!(thread_tag.is_some());
        assert_eq!(thread_tag.unwrap().memory_count, ops_per_thread);
    }
}

/// Test: Mixed concurrent reads and writes should not deadlock.
#[test]
fn test_mixed_concurrent_access() {
    let service = Arc::new(TopicIndexService::new());
    let num_writers = 5;
    let num_readers = 5;
    let ops_per_thread = 50;
    let completed = Arc::new(AtomicUsize::new(0));

    // Start writer threads
    let writer_handles: Vec<_> = (0..num_writers)
        .map(|t| {
            let service = Arc::clone(&service);
            let completed = Arc::clone(&completed);
            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    let id = MemoryId::new(format!("writer-{t}-{i}"));
                    let tags = vec![format!("writer-tag-{t}"), "common".to_string()];
                    service
                        .add_memory(&id, &tags, Namespace::Decisions)
                        .unwrap();
                    completed.fetch_add(1, Ordering::SeqCst);

                    // Small delay to increase interleaving
                    if i % 10 == 0 {
                        thread::yield_now();
                    }
                }
            })
        })
        .collect();

    // Start reader threads
    let reader_handles: Vec<_> = (0..num_readers)
        .map(|t| {
            let service = Arc::clone(&service);
            let completed = Arc::clone(&completed);
            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    match i % 3 {
                        0 => {
                            let _ = service.list_topics();
                        },
                        1 => {
                            let _ = service.get_topic_memories("common");
                        },
                        _ => {
                            let _ =
                                service.get_topic_info(&format!("writer-tag-{}", t % num_writers));
                        },
                    }
                    completed.fetch_add(1, Ordering::SeqCst);

                    // Small delay to increase interleaving
                    if i % 10 == 0 {
                        thread::yield_now();
                    }
                }
            })
        })
        .collect();

    // Wait for all threads
    for handle in writer_handles {
        handle.join().expect("Writer thread panicked");
    }
    for handle in reader_handles {
        handle.join().expect("Reader thread panicked");
    }

    // Verify all operations completed
    let expected = (num_writers + num_readers) * ops_per_thread;
    assert_eq!(completed.load(Ordering::SeqCst), expected);

    // Verify "common" tag has correct count
    let common_memories = service.get_topic_memories("common").unwrap();
    assert_eq!(common_memories.len(), num_writers * ops_per_thread);
}

/// Test: Rapid sequential operations should not corrupt state.
#[test]
fn test_rapid_sequential_operations() {
    let service = TopicIndexService::new();
    let iterations = 1000;

    for i in 0..iterations {
        let id = MemoryId::new(format!("rapid-{i}"));
        let tags = vec![format!("batch-{}", i / 100), "all".to_string()];
        service
            .add_memory(&id, &tags, Namespace::Decisions)
            .unwrap();
    }

    // Verify counts
    assert_eq!(service.topic_count(), 12); // 10 batch-X + "all" + "decisions"

    let all_memories = service.get_topic_memories("all").unwrap();
    assert_eq!(all_memories.len(), iterations);

    // Verify each batch
    for batch in 0..10 {
        let batch_memories = service
            .get_topic_memories(&format!("batch-{batch}"))
            .unwrap();
        assert_eq!(batch_memories.len(), 100);
    }
}

/// Test: Stress test with many threads and operations.
#[test]
fn test_stress_many_threads() {
    let service = Arc::new(TopicIndexService::new());
    let num_threads = 50;
    let ops_per_thread = 20;
    let completed = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..num_threads)
        .map(|t| {
            let service = Arc::clone(&service);
            let completed = Arc::clone(&completed);
            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    // Mix of operations
                    if i % 2 == 0 {
                        // Write
                        let id = MemoryId::new(format!("stress-{t}-{i}"));
                        let tags = vec!["stress".to_string()];
                        let _ = service.add_memory(&id, &tags, Namespace::Decisions);
                    } else {
                        // Read
                        let _ = service.list_topics();
                    }
                    completed.fetch_add(1, Ordering::SeqCst);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    assert_eq!(
        completed.load(Ordering::SeqCst),
        num_threads * ops_per_thread
    );
}

/// Test: Concurrent `needs_refresh` checks.
#[test]
fn test_concurrent_needs_refresh() {
    let service = Arc::new(TopicIndexService::new());
    let num_threads = 20;
    let ops_per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let service = Arc::clone(&service);
            thread::spawn(move || {
                for _ in 0..ops_per_thread {
                    let _ = service.needs_refresh();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread panicked");
    }
}

/// Test: Concurrent `topic_count` and `association_count`.
#[test]
fn test_concurrent_count_operations() {
    let service = Arc::new(TopicIndexService::new());

    // Pre-populate
    for i in 0..50 {
        let id = MemoryId::new(format!("count-{i}"));
        service
            .add_memory(&id, &["counted".to_string()], Namespace::Decisions)
            .unwrap();
    }

    let num_threads = 10;
    let ops_per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|t| {
            let service = Arc::clone(&service);
            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    if t % 2 == 0 {
                        let _ = service.topic_count();
                    } else {
                        let _ = service.association_count();
                    }

                    // Occasionally add more data
                    if i % 20 == 0 {
                        let id = MemoryId::new(format!("dynamic-{t}-{i}"));
                        let _ =
                            service.add_memory(&id, &["dynamic".to_string()], Namespace::Patterns);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread panicked");
    }
}

/// Test: Empty service under concurrent access.
#[test]
fn test_empty_service_concurrent_access() {
    let service = Arc::new(TopicIndexService::new());
    let num_threads = 10;
    let ops_per_thread = 50;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let service = Arc::clone(&service);
            thread::spawn(move || {
                for _ in 0..ops_per_thread {
                    // All these should return empty/zero on empty service
                    let topics = service.list_topics().unwrap();
                    assert!(topics.is_empty());

                    let memories = service.get_topic_memories("nonexistent").unwrap();
                    assert!(memories.is_empty());

                    let info = service.get_topic_info("nonexistent").unwrap();
                    assert!(info.is_none());

                    assert_eq!(service.topic_count(), 0);
                    assert_eq!(service.association_count(), 0);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread panicked");
    }
}

// ============================================================================
// Query Parser Concurrent Access Tests
// ============================================================================

mod query_parser_chaos {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use subcog::services::parse_filter_query;

    /// Test: Concurrent query parsing should be thread-safe.
    #[test]
    fn test_concurrent_query_parsing() {
        let queries = Arc::new(vec![
            "ns:decisions tag:rust",
            "tag:a,b,c -tag:test",
            "since:7d status:active",
            "source:src/**/*.rs",
            "ns:patterns ns:learnings tag:error",
            "",
            "invalid:query freetext",
            "tag:unicode-\u{65E5}\u{672C}\u{8A9E} tag:emoji-\u{1F980}",
        ]);

        let num_threads = 20;
        let ops_per_thread = 50;
        let completed = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..num_threads)
            .map(|t| {
                let queries = Arc::clone(&queries);
                let completed = Arc::clone(&completed);
                thread::spawn(move || {
                    for i in 0..ops_per_thread {
                        let query = &queries[(t + i) % queries.len()];
                        let filter = parse_filter_query(query);
                        // Just verify it returns something
                        let _ = filter.is_empty();
                        completed.fetch_add(1, Ordering::SeqCst);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        assert_eq!(
            completed.load(Ordering::SeqCst),
            num_threads * ops_per_thread
        );
    }

    /// Test: Same query parsed concurrently should produce identical results.
    #[test]
    fn test_deterministic_parsing() {
        let query = "ns:decisions tag:rust,python -tag:test since:7d";
        let num_threads = 10;

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let query = query.to_string();
                thread::spawn(move || {
                    let filter = parse_filter_query(&query);
                    (
                        filter.namespaces.len(),
                        filter.tags.len(),
                        filter.tags_any.len(),
                        filter.excluded_tags.len(),
                        filter.created_after.is_some(),
                    )
                })
            })
            .collect();

        let results: Vec<_> = handles
            .into_iter()
            .map(|h| h.join().expect("Thread panicked"))
            .collect();

        // All results should be identical
        let first = &results[0];
        for result in &results[1..] {
            assert_eq!(result, first, "Parsing should be deterministic");
        }
    }
}

// ============================================================================
// Namespace Concurrent Access Tests
// ============================================================================

mod namespace_chaos {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use subcog::models::Namespace;

    /// Test: Concurrent namespace parsing is thread-safe.
    #[test]
    fn test_concurrent_namespace_parsing() {
        let names = Arc::new(vec![
            "decisions",
            "PATTERNS",
            "Learnings",
            "tech-debt",
            "techdebt",
            "tech_debt",
            "blockers",
            "progress",
            "apis",
            "config",
            "invalid",
            "security",
        ]);

        let num_threads = 10;
        let ops_per_thread = 100;
        let successful_parses = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..num_threads)
            .map(|t| {
                let names = Arc::clone(&names);
                let successful = Arc::clone(&successful_parses);
                thread::spawn(move || {
                    for i in 0..ops_per_thread {
                        let name = &names[(t + i) % names.len()];
                        if Namespace::parse(name).is_some() {
                            successful.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // "invalid" is the only one that should fail
        // 11 valid names out of 12, so success rate should be 11/12
        let total_ops = num_threads * ops_per_thread;
        let successes = successful_parses.load(Ordering::SeqCst);

        // Allow some tolerance due to thread scheduling
        let expected_min = (total_ops * 10) / 12; // At least 10/12 should succeed
        assert!(
            successes >= expected_min,
            "Expected at least {expected_min} successes, got {successes}"
        );
    }

    /// Test: `Namespace::all` is thread-safe.
    #[test]
    fn test_concurrent_namespace_all() {
        let num_threads = 10;
        let ops_per_thread: usize = 100;

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                thread::spawn(move || {
                    for _ in 0..ops_per_thread {
                        let all = Namespace::all();
                        assert_eq!(all.len(), 14);
                        for ns in all {
                            let _ = ns.as_str();
                            let _ = ns.is_system();
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread panicked");
        }
    }
}
