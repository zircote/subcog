//! Recent capture deduplication checker.
//!
//! Detects duplicates by tracking recently captured content hashes
//! in an in-memory LRU cache with TTL-based expiration.

use crate::models::{MemoryId, Namespace};
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use tracing::instrument;

use super::hasher::ContentHasher;

/// Entry in the recent capture cache.
#[derive(Debug, Clone)]
struct CacheEntry {
    /// The memory ID of the captured content.
    memory_id: MemoryId,
    /// The namespace of the captured content.
    namespace: Namespace,
    /// The domain of the captured content.
    domain: String,
    /// When this entry was recorded.
    captured_at: Instant,
}

/// Checker for recently captured content.
///
/// # How it works
///
/// 1. Maintains an LRU cache mapping content hashes to capture info
/// 2. When checking, looks up the hash in the cache
/// 3. Returns a match if found and not expired (within TTL window)
/// 4. Automatically evicts expired entries and maintains LRU ordering
///
/// # Thread Safety
///
/// Uses `RwLock` for interior mutability, allowing concurrent reads
/// and exclusive writes. Safe for use across async tasks.
///
/// # Lock Poisoning
///
/// Lock poisoning is handled with fail-open semantics: if the lock is
/// poisoned (due to a panic in another thread), operations return `None`
/// (for checks) or silently skip (for records). This is intentional:
/// - Deduplication is a performance optimization, not a correctness requirement
/// - Failing to detect a duplicate just means we capture twice (safe)
/// - Blocking all captures due to a transient panic would be worse
///
/// # Example
///
/// ```rust,ignore
/// use subcog::services::deduplication::RecentCaptureChecker;
/// use std::time::Duration;
///
/// let checker = RecentCaptureChecker::new(1000, Duration::from_secs(300));
///
/// // Record a capture
/// checker.record("content", MemoryId::new("id1"), Namespace::Decisions, "project");
///
/// // Check if same content was recently captured
/// let result = checker.check("content", Namespace::Decisions);
/// assert!(result.is_some());
/// ```
pub struct RecentCaptureChecker {
    /// LRU cache mapping content hash to capture entry.
    /// Uses `RwLock` for thread-safe interior mutability.
    cache: RwLock<LruCache<String, CacheEntry>>,
    /// Time-to-live for cache entries.
    ttl: Duration,
}

impl RecentCaptureChecker {
    /// Creates a new recent capture checker.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of entries in the cache
    /// * `ttl` - How long entries remain valid
    ///
    /// # Panics
    ///
    /// Panics if capacity is 0.
    #[must_use]
    #[allow(clippy::expect_used)] // Documented panic for invalid input
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        let cap = NonZeroUsize::new(capacity).expect("capacity must be > 0");
        Self {
            cache: RwLock::new(LruCache::new(cap)),
            ttl,
        }
    }

    /// Creates a checker with default settings.
    ///
    /// Default: 1000 entries, 5 minute TTL.
    #[must_use]
    pub fn default_settings() -> Self {
        Self::new(1000, Duration::from_secs(300))
    }

    /// Checks if content was recently captured in the given namespace.
    ///
    /// # Arguments
    ///
    /// * `content` - The content to check
    /// * `namespace` - The namespace to check within
    ///
    /// # Returns
    ///
    /// Returns `Some((MemoryId, URN))` if content was recently captured,
    /// `None` otherwise.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = checker.check("content", Namespace::Decisions);
    /// match result {
    ///     Some((id, urn)) => println!("Recently captured: {}", urn),
    ///     None => println!("Not recently captured"),
    /// }
    /// ```
    #[instrument(
        skip(self, content),
        fields(
            operation = "recent_capture_check",
            namespace = %namespace.as_str(),
            content_length = content.len()
        )
    )]
    pub fn check(&self, content: &str, namespace: Namespace) -> Option<(MemoryId, String)> {
        let start = Instant::now();
        let hash = ContentHasher::hash(content);

        // Try to get from cache (read lock)
        let result = {
            let cache = self.cache.read().ok()?;
            cache.peek(&hash).cloned()
        };

        let duration_ms = start.elapsed().as_millis();

        match result {
            Some(entry) => {
                // Check if entry is still valid (not expired)
                if entry.captured_at.elapsed() <= self.ttl {
                    // Check namespace matches
                    if entry.namespace == namespace {
                        let urn = format!(
                            "subcog://{}/{}/{}",
                            entry.domain,
                            namespace.as_str(),
                            entry.memory_id
                        );

                        tracing::debug!(
                            memory_id = %entry.memory_id,
                            urn = %urn,
                            age_ms = %entry.captured_at.elapsed().as_millis(),
                            duration_ms = %duration_ms,
                            "Recent capture found"
                        );

                        metrics::histogram!(
                            "deduplication_check_duration_ms",
                            "checker" => "recent_capture",
                            "found" => "true"
                        )
                        .record(duration_ms as f64);

                        return Some((entry.memory_id, urn));
                    }
                }

                // Entry expired or wrong namespace - will be cleaned up lazily
                tracing::debug!(
                    duration_ms = %duration_ms,
                    "Cache entry expired or wrong namespace"
                );
            },
            None => {
                tracing::debug!(
                    duration_ms = %duration_ms,
                    "No recent capture found"
                );
            },
        }

        metrics::histogram!(
            "deduplication_check_duration_ms",
            "checker" => "recent_capture",
            "found" => "false"
        )
        .record(duration_ms as f64);

        None
    }

    /// Records a successful capture for future duplicate detection.
    ///
    /// # Arguments
    ///
    /// * `content` - The captured content
    /// * `memory_id` - The ID assigned to the captured memory
    /// * `namespace` - The namespace the content was captured to
    /// * `domain` - The domain the content was captured to
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// checker.record(
    ///     "Use PostgreSQL for storage",
    ///     MemoryId::new("mem-123"),
    ///     Namespace::Decisions,
    ///     "project",
    /// );
    /// ```
    #[instrument(
        skip(self, content),
        fields(
            operation = "record_capture",
            memory_id = %memory_id,
            namespace = %namespace.as_str(),
            content_length = content.len()
        )
    )]
    pub fn record(&self, content: &str, memory_id: &MemoryId, namespace: Namespace, domain: &str) {
        let hash = ContentHasher::hash(content);

        let entry = CacheEntry {
            memory_id: memory_id.clone(),
            namespace,
            domain: domain.to_string(),
            captured_at: Instant::now(),
        };

        // Acquire write lock and insert
        if let Ok(mut cache) = self.cache.write() {
            cache.put(hash, entry);

            tracing::debug!(
                memory_id = %memory_id,
                "Recorded capture in recent cache"
            );

            metrics::gauge!("deduplication_recent_cache_size").set(cache.len() as f64);
        }
    }

    /// Records a capture using just the content hash.
    ///
    /// Useful when the hash has already been computed.
    ///
    /// # Arguments
    ///
    /// * `content_hash` - The pre-computed content hash
    /// * `memory_id` - The ID assigned to the captured memory
    /// * `namespace` - The namespace the content was captured to
    /// * `domain` - The domain the content was captured to
    pub fn record_by_hash(
        &self,
        content_hash: &str,
        memory_id: &MemoryId,
        namespace: Namespace,
        domain: &str,
    ) {
        let entry = CacheEntry {
            memory_id: memory_id.clone(),
            namespace,
            domain: domain.to_string(),
            captured_at: Instant::now(),
        };

        if let Ok(mut cache) = self.cache.write() {
            cache.put(content_hash.to_string(), entry);

            tracing::debug!(
                memory_id = %memory_id,
                hash = %content_hash,
                "Recorded capture by hash in recent cache"
            );

            metrics::gauge!("deduplication_recent_cache_size").set(cache.len() as f64);
        }
    }

    /// Clears all entries from the cache.
    #[cfg(test)]
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();

            tracing::debug!("Cleared recent capture cache");

            metrics::gauge!("deduplication_recent_cache_size").set(0.0);
        }
    }

    /// Returns the current number of entries in the cache.
    ///
    /// Note: This includes potentially expired entries that haven't
    /// been cleaned up yet.
    #[cfg(test)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.cache.read().map(|c| c.len()).unwrap_or(0)
    }

    /// Returns true if the cache is empty.
    #[cfg(test)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the configured TTL.
    #[cfg(test)]
    #[must_use]
    pub const fn ttl(&self) -> Duration {
        self.ttl
    }
}

impl Default for RecentCaptureChecker {
    fn default() -> Self {
        Self::default_settings()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_new_checker() {
        let checker = RecentCaptureChecker::new(100, Duration::from_secs(60));
        assert_eq!(checker.len(), 0);
        assert!(checker.is_empty());
        assert_eq!(checker.ttl(), Duration::from_secs(60));
    }

    #[test]
    fn test_default_settings() {
        let checker = RecentCaptureChecker::default_settings();
        assert_eq!(checker.ttl(), Duration::from_secs(300));
    }

    #[test]
    fn test_record_and_check() {
        let checker = RecentCaptureChecker::new(100, Duration::from_secs(60));

        let content = "Use PostgreSQL for storage";
        let memory_id = MemoryId::new("mem-123");

        // Record the capture
        checker.record(content, &memory_id, Namespace::Decisions, "project");

        assert_eq!(checker.len(), 1);

        // Check should find it
        let result = checker.check(content, Namespace::Decisions);
        assert!(result.is_some());

        let (id, urn) = result.unwrap();
        assert_eq!(id.as_str(), "mem-123");
        assert_eq!(urn, "subcog://project/decisions/mem-123");
    }

    #[test]
    fn test_check_not_found() {
        let checker = RecentCaptureChecker::new(100, Duration::from_secs(60));

        let result = checker.check("non-existent content", Namespace::Decisions);
        assert!(result.is_none());
    }

    #[test]
    fn test_check_wrong_namespace() {
        let checker = RecentCaptureChecker::new(100, Duration::from_secs(60));

        let content = "Use PostgreSQL for storage";
        checker.record(
            content,
            &MemoryId::new("mem-123"),
            Namespace::Decisions,
            "project",
        );

        // Check in different namespace should not find it
        let result = checker.check(content, Namespace::Patterns);
        assert!(result.is_none());
    }

    #[test]
    fn test_check_expired() {
        // Create checker with very short TTL
        let checker = RecentCaptureChecker::new(100, Duration::from_millis(50));

        let content = "Use PostgreSQL for storage";
        checker.record(
            content,
            &MemoryId::new("mem-123"),
            Namespace::Decisions,
            "project",
        );

        // Wait for expiration
        thread::sleep(Duration::from_millis(100));

        // Check should not find it (expired)
        let result = checker.check(content, Namespace::Decisions);
        assert!(result.is_none());
    }

    #[test]
    fn test_normalized_content_matches() {
        let checker = RecentCaptureChecker::new(100, Duration::from_secs(60));

        // Record with normalized content
        checker.record(
            "Use PostgreSQL",
            &MemoryId::new("mem-123"),
            Namespace::Decisions,
            "project",
        );

        // Check with whitespace/case variations should still match
        let result = checker.check("  use  postgresql  ", Namespace::Decisions);
        assert!(result.is_some());
    }

    #[test]
    fn test_lru_eviction() {
        // Create checker with capacity of 2
        let checker = RecentCaptureChecker::new(2, Duration::from_secs(60));

        checker.record(
            "content1",
            &MemoryId::new("mem-1"),
            Namespace::Decisions,
            "project",
        );
        checker.record(
            "content2",
            &MemoryId::new("mem-2"),
            Namespace::Decisions,
            "project",
        );

        assert_eq!(checker.len(), 2);

        // Add a third - should evict content1 (least recently used)
        checker.record(
            "content3",
            &MemoryId::new("mem-3"),
            Namespace::Decisions,
            "project",
        );

        assert_eq!(checker.len(), 2);

        // content1 should be evicted
        let result = checker.check("content1", Namespace::Decisions);
        assert!(result.is_none());

        // content2 and content3 should still be there
        let result = checker.check("content2", Namespace::Decisions);
        assert!(result.is_some());

        let result = checker.check("content3", Namespace::Decisions);
        assert!(result.is_some());
    }

    #[test]
    fn test_clear() {
        let checker = RecentCaptureChecker::new(100, Duration::from_secs(60));

        checker.record(
            "content1",
            &MemoryId::new("mem-1"),
            Namespace::Decisions,
            "project",
        );
        checker.record(
            "content2",
            &MemoryId::new("mem-2"),
            Namespace::Decisions,
            "project",
        );

        assert_eq!(checker.len(), 2);

        checker.clear();

        assert_eq!(checker.len(), 0);
        assert!(checker.is_empty());
    }

    #[test]
    fn test_record_by_hash() {
        let checker = RecentCaptureChecker::new(100, Duration::from_secs(60));

        let content = "Use PostgreSQL for storage";
        let hash = ContentHasher::hash(content);

        // Record by hash
        checker.record_by_hash(
            &hash,
            &MemoryId::new("mem-123"),
            Namespace::Decisions,
            "project",
        );

        // Check with content should find it
        let result = checker.check(content, Namespace::Decisions);
        assert!(result.is_some());
    }

    #[test]
    fn test_update_existing() {
        let checker = RecentCaptureChecker::new(100, Duration::from_secs(60));

        let content = "Use PostgreSQL for storage";

        // Record first time
        checker.record(
            content,
            &MemoryId::new("mem-old"),
            Namespace::Decisions,
            "project",
        );

        // Record again with different ID
        checker.record(
            content,
            &MemoryId::new("mem-new"),
            Namespace::Decisions,
            "project",
        );

        assert_eq!(checker.len(), 1);

        // Should find the new ID
        let result = checker.check(content, Namespace::Decisions);
        assert!(result.is_some());

        let (id, _) = result.unwrap();
        assert_eq!(id.as_str(), "mem-new");
    }

    #[test]
    fn test_thread_safety() {
        use std::sync::Arc;

        let checker = Arc::new(RecentCaptureChecker::new(100, Duration::from_secs(60)));

        let checker1 = checker.clone();
        let checker2 = checker.clone();

        let t1 = thread::spawn(move || {
            for i in 0..50 {
                checker1.record(
                    &format!("content-t1-{i}"),
                    &MemoryId::new(format!("mem-t1-{i}")),
                    Namespace::Decisions,
                    "project",
                );
            }
        });

        let t2 = thread::spawn(move || {
            for i in 0..50 {
                checker2.record(
                    &format!("content-t2-{i}"),
                    &MemoryId::new(format!("mem-t2-{i}")),
                    Namespace::Patterns,
                    "project",
                );
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();

        // Should have 100 entries
        assert_eq!(checker.len(), 100);
    }
}
