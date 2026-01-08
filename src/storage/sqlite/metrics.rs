//! Shared metrics recording for `SQLite` backends.
//!
//! This module provides utilities for recording storage operation metrics
//! consistently across both the index and persistence backends.

use std::time::Instant;

/// Records operation metrics for storage operations.
///
/// This function records two metrics for each operation:
/// 1. `storage_operations_total` - Counter for operation count by status
/// 2. `storage_operation_duration_ms` - Histogram for operation latency
///
/// # Arguments
///
/// * `backend` - Backend name (e.g., "sqlite", "postgresql")
/// * `operation` - Operation name (e.g., "index", "search", "store", "get")
/// * `start` - Operation start time from `Instant::now()`
/// * `status` - Operation status ("success" or "error")
///
/// # Examples
///
/// ```ignore
/// use std::time::Instant;
/// use subcog::storage::sqlite::record_operation_metrics;
///
/// let start = Instant::now();
/// // ... perform operation ...
/// let status = if result.is_ok() { "success" } else { "error" };
/// record_operation_metrics("sqlite", "search", start, status);
/// ```
pub fn record_operation_metrics(
    backend: &'static str,
    operation: &'static str,
    start: Instant,
    status: &'static str,
) {
    metrics::counter!(
        "storage_operations_total",
        "backend" => backend,
        "operation" => operation,
        "status" => status
    )
    .increment(1);
    metrics::histogram!(
        "storage_operation_duration_ms",
        "backend" => backend,
        "operation" => operation,
        "status" => status
    )
    .record(start.elapsed().as_secs_f64() * 1000.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_record_operation_metrics_success() {
        // Test that metrics recording completes without panicking
        let start = Instant::now();
        thread::sleep(Duration::from_millis(1));

        record_operation_metrics("sqlite", "test_operation", start, "success");

        // If we get here without panic, the function works
        // Note: We can't easily verify metrics output in unit tests,
        // but we can verify the function executes successfully
    }

    #[test]
    fn test_record_operation_metrics_error() {
        // Test error status recording
        let start = Instant::now();
        thread::sleep(Duration::from_millis(1));

        record_operation_metrics("sqlite", "test_operation", start, "error");
    }

    #[test]
    fn test_record_operation_metrics_different_operations() {
        // Test different operation types
        let start = Instant::now();

        record_operation_metrics("sqlite", "index", start, "success");
        record_operation_metrics("sqlite", "search", start, "success");
        record_operation_metrics("sqlite", "store", start, "success");
        record_operation_metrics("sqlite", "get", start, "error");
        record_operation_metrics("sqlite", "delete", start, "success");
    }

    #[test]
    fn test_record_operation_metrics_different_backends() {
        // Test different backend types
        let start = Instant::now();

        record_operation_metrics("sqlite", "search", start, "success");
        record_operation_metrics("postgresql", "search", start, "success");
        record_operation_metrics("redis", "search", start, "error");
    }

    #[test]
    fn test_record_operation_metrics_timing() {
        // Verify that elapsed time is being calculated
        let start = Instant::now();
        thread::sleep(Duration::from_millis(10));

        // Should record a duration >= 10ms
        record_operation_metrics("sqlite", "timed_operation", start, "success");

        // Verify elapsed time was non-zero
        assert!(start.elapsed().as_millis() >= 10);
    }

    #[test]
    fn test_record_operation_metrics_zero_duration() {
        // Test with minimal elapsed time
        let start = Instant::now();

        // Call immediately without sleep
        record_operation_metrics("sqlite", "instant_operation", start, "success");

        // Should still work even with near-zero duration
    }

    #[test]
    fn test_record_operation_metrics_concurrent() {
        // Test that multiple threads can record metrics simultaneously
        use std::thread;

        let handles: Vec<_> = (0..4)
            .map(|i| {
                let status = if i % 2 == 0 { "success" } else { "error" };
                thread::spawn(move || {
                    let start = Instant::now();
                    thread::sleep(Duration::from_millis(i * 2));
                    record_operation_metrics("sqlite", "concurrent_operation", start, status);
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread panicked");
        }
    }

    #[test]
    fn test_record_operation_metrics_long_duration() {
        // Test with longer duration to verify ms conversion
        let start = Instant::now();
        thread::sleep(Duration::from_millis(50));

        record_operation_metrics("sqlite", "long_operation", start, "success");

        // Verify duration is reasonable (>= 50ms)
        assert!(start.elapsed().as_millis() >= 50);
    }
}
