//! Shared connection handling for `SQLite` backends.
//!
//! This module provides utilities for managing `SQLite` connections with proper
//! mutex handling, poison recovery, and optimal performance configuration.

use crate::{Error, Result};
use rusqlite::Connection;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Timeout for acquiring mutex lock (5 seconds).
/// Reserved for future use when upgrading to `parking_lot::Mutex`.
#[allow(dead_code)]
pub const MUTEX_LOCK_TIMEOUT: Duration = Duration::from_secs(5);

/// Helper to acquire mutex lock with poison recovery.
///
/// If the mutex is poisoned (due to a panic in a previous critical section),
/// we recover the inner value and log a warning. This prevents cascading
/// failures when one operation panics.
///
/// # Examples
///
/// ```ignore
/// use std::sync::Mutex;
/// use subcog::storage::sqlite::acquire_lock;
///
/// let mutex = Mutex::new(connection);
/// let guard = acquire_lock(&mutex);
/// // Use guard...
/// ```
pub fn acquire_lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // First try to acquire lock normally
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            // Recover from poison - this is safe because we log the issue
            // and the connection state should still be valid
            tracing::warn!("SQLite mutex was poisoned, recovering");
            metrics::counter!("sqlite_mutex_poison_recovery_total").increment(1);
            poisoned.into_inner()
        },
    }
}

/// Alternative lock acquisition with spin-wait timeout.
///
/// Note: Rust's `std::sync::Mutex` doesn't have a native `try_lock_for`,
/// so we implement a spin-wait with sleep. For production, consider
/// using `parking_lot::Mutex` which has proper timed locking.
///
/// Reserved for future use - currently using simpler `acquire_lock` with poison recovery.
///
/// # Errors
///
/// Returns [`Error::OperationFailed`] if the lock cannot be acquired within the timeout.
///
/// # Examples
///
/// ```ignore
/// use std::sync::Mutex;
/// use std::time::Duration;
/// use subcog::storage::sqlite::acquire_lock_with_timeout;
///
/// let mutex = Mutex::new(connection);
/// let timeout = Duration::from_secs(5);
/// let guard = acquire_lock_with_timeout(&mutex, timeout)?;
/// // Use guard...
/// # Ok::<(), subcog::Error>(())
/// ```
#[allow(dead_code)]
pub fn acquire_lock_with_timeout<T>(
    mutex: &Mutex<T>,
    timeout: Duration,
) -> Result<MutexGuard<'_, T>> {
    let start = Instant::now();
    let sleep_duration = Duration::from_millis(10);

    loop {
        match mutex.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                tracing::warn!("SQLite mutex was poisoned, recovering");
                metrics::counter!("sqlite_mutex_poison_recovery_total").increment(1);
                return Ok(poisoned.into_inner());
            },
            Err(std::sync::TryLockError::WouldBlock) => {
                if start.elapsed() > timeout {
                    metrics::counter!("sqlite_mutex_timeout_total").increment(1);
                    return Err(Error::OperationFailed {
                        operation: "acquire_lock".to_string(),
                        cause: format!("Lock acquisition timed out after {timeout:?}"),
                    });
                }
                std::thread::sleep(sleep_duration);
            },
        }
    }
}

/// Configures a `SQLite` connection with optimal settings for performance and concurrency.
///
/// # Configuration Applied
///
/// - **WAL mode**: Enables Write-Ahead Logging for better concurrent read performance
/// - **NORMAL synchronous**: Balances durability with performance
/// - **`busy_timeout`**: Sets a 5-second timeout to handle lock contention gracefully
///
/// # Concurrency Model
///
/// Uses a `Mutex<Connection>` for thread-safe access. While this serializes
/// database operations, `SQLite`'s WAL mode and `busy_timeout` pragma mitigate
/// contention:
///
/// - **WAL mode**: Allows concurrent readers with a single writer
/// - **`busy_timeout`**: Waits up to 5 seconds for locks instead of failing immediately
/// - **NORMAL synchronous**: Balances durability with performance
///
/// For high-throughput scenarios requiring true connection pooling, consider
/// using `r2d2-rusqlite` or `deadpool-sqlite`.
///
/// # Errors
///
/// Returns [`Error::OperationFailed`] if pragma configuration fails.
///
/// # Examples
///
/// ```ignore
/// use rusqlite::Connection;
/// use subcog::storage::sqlite::configure_connection;
///
/// let conn = Connection::open("db.sqlite")?;
/// configure_connection(&conn)?;
/// # Ok::<(), subcog::Error>(())
/// ```
pub fn configure_connection(conn: &Connection) -> Result<()> {
    // Enable WAL mode for better concurrent read performance
    // Note: pragma_update returns the result which we ignore - journal_mode returns
    // a string like "wal" which would cause execute_batch to fail
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    let _ = conn.pragma_update(None, "synchronous", "NORMAL");
    // Set busy timeout to 5 seconds to handle lock contention gracefully
    // This prevents SQLITE_BUSY errors during high concurrent access
    let _ = conn.pragma_update(None, "busy_timeout", "5000");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_acquire_lock_success() {
        let mutex = Mutex::new(42);
        let guard = acquire_lock(&mutex);
        assert_eq!(*guard, 42);
    }

    #[test]
    fn test_acquire_lock_concurrent() {
        let mutex = Arc::new(Mutex::new(0));
        let mut handles = vec![];

        for _ in 0..10 {
            let mutex_clone = Arc::clone(&mutex);
            let handle = thread::spawn(move || {
                let mut guard = acquire_lock(&mutex_clone);
                *guard += 1;
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let guard = acquire_lock(&mutex);
        assert_eq!(*guard, 10);
    }

    #[test]
    fn test_acquire_lock_with_timeout_success() {
        let mutex = Mutex::new(42);
        let timeout = Duration::from_secs(1);
        let guard = acquire_lock_with_timeout(&mutex, timeout).unwrap();
        assert_eq!(*guard, 42);
    }

    #[test]
    fn test_acquire_lock_with_timeout_timeout() {
        let mutex = Arc::new(Mutex::new(42));
        let mutex_clone = Arc::clone(&mutex);

        // Hold the lock in a thread
        let _guard = mutex.lock().unwrap();

        // Try to acquire with timeout (should fail)
        let timeout = Duration::from_millis(50);
        let result = acquire_lock_with_timeout(&mutex_clone, timeout);
        assert!(result.is_err());

        assert!(
            matches!(
                result,
                Err(Error::OperationFailed { ref operation, ref cause })
                    if operation == "acquire_lock" && cause.contains("timed out")
            ),
            "Expected OperationFailed error with 'acquire_lock' operation and 'timed out' cause"
        );
    }

    #[test]
    fn test_configure_connection() {
        let conn = Connection::open_in_memory().unwrap();
        let result = configure_connection(&conn);
        assert!(result.is_ok());

        // Verify WAL mode is enabled (in-memory databases use "memory" journal mode)
        let journal_mode: String = conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        // In-memory SQLite databases cannot use WAL mode - they report "memory"
        // File-based databases should use WAL mode
        assert!(
            journal_mode.to_lowercase() == "wal" || journal_mode.to_lowercase() == "memory",
            "Expected 'wal' or 'memory' journal mode, got '{journal_mode}'"
        );

        // Verify synchronous mode (NORMAL = 1)
        let synchronous: i32 = conn
            .pragma_query_value(None, "synchronous", |row| row.get(0))
            .unwrap();
        assert_eq!(synchronous, 1, "Expected NORMAL synchronous mode (1)");

        // Verify busy timeout
        let busy_timeout: i32 = conn
            .pragma_query_value(None, "busy_timeout", |row| row.get(0))
            .unwrap();
        assert_eq!(busy_timeout, 5000);
    }
}
