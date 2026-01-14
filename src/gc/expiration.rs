//! TTL-based expiration garbage collector implementation.
//!
//! Identifies and tombstones memories that have exceeded their TTL (`expires_at` timestamp).
//!
//! Unlike retention-based GC which calculates expiration from `created_at + retention_days`,
//! expiration GC uses the explicit `expires_at` field set at capture time.
//!
//! # Configuration
//!
//! TTL can be set at capture time via:
//! - CLI: `subcog capture --ttl 7d "content"`
//! - MCP: `{ "ttl": "30d" }` in capture arguments
//! - Config file: `[memory.ttl]` section with per-namespace defaults
//!
//! # Example
//!
//! ```rust,ignore
//! use subcog::gc::{ExpirationConfig, ExpirationService};
//! use subcog::storage::index::SqliteBackend;
//! use std::sync::Arc;
//!
//! // Create expiration service with index backend
//! let backend = Arc::new(SqliteBackend::new("memories.db")?);
//! let config = ExpirationConfig::default();
//! let service = ExpirationService::new(backend, config);
//!
//! // Dry run to see what would be cleaned up
//! let result = service.gc_expired_memories(true)?;
//! println!("Would tombstone {} expired memories", result.memories_tombstoned);
//!
//! // Actually perform the cleanup
//! let result = service.gc_expired_memories(false)?;
//! println!("Tombstoned {} memories", result.memories_tombstoned);
//! ```
//!
//! # Probabilistic Cleanup
//!
//! To avoid expensive full scans on every operation, the expiration service
//! can be triggered probabilistically during capture operations:
//!
//! ```rust,ignore
//! use subcog::gc::ExpirationService;
//!
//! // 5% chance to trigger cleanup after a capture
//! if service.should_run_cleanup() {
//!     let _ = service.gc_expired_memories(false);
//! }
//! ```

use crate::Result;
use crate::models::SearchFilter;
use crate::storage::traits::IndexBackend;
use chrono::{TimeZone, Utc};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, instrument, warn};

/// Environment variable for cleanup probability (0.0 to 1.0).
pub const EXPIRATION_CLEANUP_PROBABILITY_ENV: &str = "SUBCOG_EXPIRATION_CLEANUP_PROBABILITY";

/// Default probability of running cleanup after a capture (5%).
pub const DEFAULT_CLEANUP_PROBABILITY: f64 = 0.05;

/// Safely converts Duration to milliseconds as u64, capping at `u64::MAX`.
#[inline]
fn duration_to_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Converts usize to f64 for metrics, capping at `u32::MAX`.
#[inline]
fn usize_to_f64(value: usize) -> f64 {
    let capped = u32::try_from(value).unwrap_or(u32::MAX);
    f64::from(capped)
}

/// Converts u64 to f64 for metrics, capping at `u32::MAX`.
#[inline]
fn u64_to_f64(value: u64) -> f64 {
    let capped = u32::try_from(value).unwrap_or(u32::MAX);
    f64::from(capped)
}

/// Configuration for expiration-based garbage collection.
#[derive(Debug, Clone)]
pub struct ExpirationConfig {
    /// Maximum memories to process in a single GC run.
    ///
    /// Prevents long-running GC operations.
    pub batch_limit: usize,

    /// Probability of running cleanup after a capture (0.0 to 1.0).
    ///
    /// Set to 0.0 to disable probabilistic cleanup.
    /// Set to 1.0 to always run cleanup after capture.
    pub cleanup_probability: f64,
}

impl Default for ExpirationConfig {
    fn default() -> Self {
        Self {
            batch_limit: 10000,
            cleanup_probability: DEFAULT_CLEANUP_PROBABILITY,
        }
    }
}

impl ExpirationConfig {
    /// Creates a new expiration config with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an expiration config from environment variables.
    ///
    /// Reads:
    /// - `SUBCOG_EXPIRATION_BATCH_LIMIT`: Batch limit for GC runs
    /// - `SUBCOG_EXPIRATION_CLEANUP_PROBABILITY`: Probability of cleanup (0.0-1.0)
    #[must_use]
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Batch limit
        if let Some(limit) = std::env::var("SUBCOG_EXPIRATION_BATCH_LIMIT")
            .ok()
            .and_then(|l| l.parse::<usize>().ok())
        {
            config.batch_limit = limit;
        }

        // Cleanup probability
        if let Some(prob) = std::env::var(EXPIRATION_CLEANUP_PROBABILITY_ENV)
            .ok()
            .and_then(|p| p.parse::<f64>().ok())
        {
            config.cleanup_probability = prob.clamp(0.0, 1.0);
        }

        config
    }

    /// Sets the batch limit.
    #[must_use]
    pub const fn with_batch_limit(mut self, limit: usize) -> Self {
        self.batch_limit = limit;
        self
    }

    /// Sets the cleanup probability.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // clamp() is not const fn
    pub fn with_cleanup_probability(mut self, probability: f64) -> Self {
        self.cleanup_probability = probability.clamp(0.0, 1.0);
        self
    }
}

/// Result of an expiration garbage collection operation.
#[derive(Debug, Clone, Default)]
pub struct ExpirationGcResult {
    /// Total number of memories checked.
    pub memories_checked: usize,

    /// Number of memories that were (or would be) tombstoned.
    pub memories_tombstoned: usize,

    /// Whether this was a dry run (no actual changes made).
    pub dry_run: bool,

    /// Duration of the GC operation in milliseconds.
    pub duration_ms: u64,
}

impl ExpirationGcResult {
    /// Returns `true` if any memories were tombstoned.
    #[must_use]
    pub const fn has_expired_memories(&self) -> bool {
        self.memories_tombstoned > 0
    }

    /// Returns a human-readable summary of the GC result.
    #[must_use]
    pub fn summary(&self) -> String {
        let action = if self.dry_run {
            "would tombstone"
        } else {
            "tombstoned"
        };

        if self.memories_tombstoned == 0 {
            format!(
                "No TTL-expired memories found ({} memories checked in {}ms)",
                self.memories_checked, self.duration_ms
            )
        } else {
            format!(
                "{} {} TTL-expired memories - checked {} in {}ms",
                action, self.memories_tombstoned, self.memories_checked, self.duration_ms
            )
        }
    }
}

/// Service for garbage collecting memories that have exceeded their TTL.
///
/// Unlike `RetentionGarbageCollector` which uses retention policies based on age,
/// this service checks the explicit `expires_at` timestamp set at capture time.
///
/// # Thread Safety
///
/// The service holds an `Arc` reference to the index backend,
/// making it safe to share across threads.
pub struct ExpirationService {
    /// Reference to the index backend for querying and updating memories.
    index: Arc<dyn IndexBackend + Send + Sync>,

    /// Expiration configuration.
    config: ExpirationConfig,
}

impl ExpirationService {
    /// Creates a new expiration service.
    ///
    /// # Arguments
    ///
    /// * `index` - Shared reference to the index backend.
    /// * `config` - Expiration configuration.
    #[must_use]
    pub fn new(index: Arc<dyn IndexBackend + Send + Sync>, config: ExpirationConfig) -> Self {
        // Arc::strong_count prevents clippy::missing_const_for_fn false positive
        let _ = Arc::strong_count(&index);
        Self { index, config }
    }

    /// Determines whether cleanup should run based on configured probability.
    ///
    /// Uses a random number generator to decide probabilistically.
    /// This enables lazy, opportunistic cleanup without expensive full scans
    /// on every operation.
    ///
    /// # Returns
    ///
    /// `true` if cleanup should run, `false` otherwise.
    #[must_use]
    pub fn should_run_cleanup(&self) -> bool {
        if self.config.cleanup_probability <= 0.0 {
            return false;
        }
        if self.config.cleanup_probability >= 1.0 {
            return true;
        }

        // Use a simple random check
        let random: f64 = rand_float();
        random < self.config.cleanup_probability
    }

    /// Performs garbage collection on TTL-expired memories.
    ///
    /// This method:
    /// 1. Lists all active (non-tombstoned) memories
    /// 2. Checks each memory for TTL expiration (`expires_at < now`)
    /// 3. Tombstones expired memories (unless `dry_run`)
    ///
    /// # Arguments
    ///
    /// * `dry_run` - If true, only report what would be done without making changes
    ///
    /// # Returns
    ///
    /// An `ExpirationGcResult` containing statistics about the operation.
    ///
    /// # Errors
    ///
    /// Returns an error if index backend operations fail.
    #[instrument(
        name = "subcog.gc.expiration",
        skip(self),
        fields(
            request_id = tracing::field::Empty,
            component = "gc",
            operation = "expiration",
            dry_run = dry_run,
            batch_limit = self.config.batch_limit
        )
    )]
    pub fn gc_expired_memories(&self, dry_run: bool) -> Result<ExpirationGcResult> {
        let start = Instant::now();
        if let Some(request_id) = crate::observability::current_request_id() {
            tracing::Span::current().record("request_id", request_id.as_str());
        }

        let mut result = ExpirationGcResult {
            dry_run,
            ..Default::default()
        };

        let now = crate::current_timestamp();

        // Query all active memories (not tombstoned)
        let filter = SearchFilter::new().with_include_tombstoned(false);
        let memories = self.index.list_all(&filter, self.config.batch_limit)?;

        debug!(
            memory_count = memories.len(),
            now, "Checking memories for TTL expiration"
        );

        for (id, _score) in memories {
            result.memories_checked += 1;

            // Get the full memory to check expires_at
            let Some(memory) = self.index.get_memory(&id)? else {
                continue;
            };

            // Check if memory has an expiration time and has expired
            let Some(expires_at) = memory.expires_at else {
                // No TTL set, skip
                continue;
            };

            if expires_at >= now {
                // Not yet expired
                continue;
            }

            // Memory has expired
            debug!(
                memory_id = %id.as_str(),
                expires_at,
                now,
                expired_ago_secs = now.saturating_sub(expires_at),
                "Memory TTL expired"
            );

            if dry_run {
                result.memories_tombstoned += 1;
                continue;
            }

            // Tombstone the memory
            let mut updated = memory.clone();
            let now_i64 = i64::try_from(now).unwrap_or(i64::MAX);
            let now_dt = Utc
                .timestamp_opt(now_i64, 0)
                .single()
                .unwrap_or_else(Utc::now);
            updated.tombstoned_at = Some(now_dt);

            let Err(e) = self.index.index(&updated) else {
                result.memories_tombstoned += 1;
                continue;
            };

            warn!(
                memory_id = %id.as_str(),
                error = %e,
                "Failed to tombstone TTL-expired memory"
            );
        }

        result.duration_ms = duration_to_millis(start.elapsed());

        // Record metrics
        metrics::counter!(
            "gc_expiration_runs_total",
            "dry_run" => dry_run.to_string()
        )
        .increment(1);
        metrics::gauge!("gc_expiration_tombstoned").set(usize_to_f64(result.memories_tombstoned));
        metrics::histogram!("gc_expiration_duration_ms").record(u64_to_f64(result.duration_ms));
        metrics::histogram!(
            "memory_lifecycle_duration_ms",
            "component" => "gc",
            "operation" => "expiration"
        )
        .record(u64_to_f64(result.duration_ms));

        info!(
            memories_checked = result.memories_checked,
            memories_tombstoned = result.memories_tombstoned,
            duration_ms = result.duration_ms,
            dry_run,
            "Expiration GC completed"
        );

        Ok(result)
    }

    /// Returns the current expiration configuration.
    #[must_use]
    pub const fn config(&self) -> &ExpirationConfig {
        &self.config
    }
}

/// Generates a random float between 0.0 and 1.0.
///
/// Uses a simple xorshift-based PRNG seeded from the current time.
/// Not cryptographically secure, but sufficient for probabilistic cleanup.
#[allow(clippy::cast_precision_loss)] // Intentional for random float generation
fn rand_float() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Seed from current time in nanoseconds (truncate to u64, which is fine for PRNG seeding)
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(42, |d| {
            let nanos = d.as_nanos();
            // Truncate to u64 - this is acceptable for PRNG seeding
            #[allow(clippy::cast_possible_truncation)]
            let result = nanos as u64;
            result
        });

    // Simple xorshift64
    let mut x = seed;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;

    // Convert to 0.0..1.0 range
    (x as f64) / (u64::MAX as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, Memory, MemoryId, MemoryStatus, Namespace};
    use crate::storage::index::SqliteBackend;

    fn create_test_memory(id: &str, namespace: Namespace, expires_at: Option<u64>) -> Memory {
        let now = crate::current_timestamp();
        Memory {
            id: MemoryId::new(id),
            content: format!("Test memory {id}"),
            namespace,
            domain: Domain::new(),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: now,
            updated_at: now,
            tombstoned_at: None,
            expires_at,
            embedding: None,
            tags: vec!["test".to_string()],
            #[cfg(feature = "group-scope")]
            group_id: None,
            source: None,
            is_summary: false,
            source_memory_ids: None,
            consolidation_timestamp: None,
        }
    }

    #[test]
    fn test_expiration_config_default() {
        let config = ExpirationConfig::default();
        assert_eq!(config.batch_limit, 10000);
        assert!((config.cleanup_probability - 0.05).abs() < f64::EPSILON);
    }

    #[test]
    fn test_expiration_config_builders() {
        let config = ExpirationConfig::new()
            .with_batch_limit(5000)
            .with_cleanup_probability(0.10);

        assert_eq!(config.batch_limit, 5000);
        assert!((config.cleanup_probability - 0.10).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cleanup_probability_clamping() {
        let config = ExpirationConfig::new().with_cleanup_probability(1.5);
        assert!((config.cleanup_probability - 1.0).abs() < f64::EPSILON);

        let config = ExpirationConfig::new().with_cleanup_probability(-0.5);
        assert!(config.cleanup_probability.abs() < f64::EPSILON);
    }

    #[test]
    fn test_expiration_gc_result_summary() {
        let result = ExpirationGcResult {
            memories_checked: 100,
            memories_tombstoned: 5,
            dry_run: false,
            duration_ms: 50,
        };
        assert!(result.summary().contains("tombstoned 5"));

        let result = ExpirationGcResult {
            memories_checked: 100,
            memories_tombstoned: 0,
            dry_run: false,
            duration_ms: 50,
        };
        assert!(result.summary().contains("No TTL-expired"));
    }

    #[test]
    fn test_expiration_gc_result_has_expired() {
        let result = ExpirationGcResult {
            memories_tombstoned: 0,
            ..Default::default()
        };
        assert!(!result.has_expired_memories());

        let result = ExpirationGcResult {
            memories_tombstoned: 1,
            ..Default::default()
        };
        assert!(result.has_expired_memories());
    }

    #[test]
    fn test_should_run_cleanup_always() {
        let config = ExpirationConfig::new().with_cleanup_probability(1.0);
        let backend: Arc<dyn IndexBackend + Send + Sync> =
            Arc::new(SqliteBackend::in_memory().expect("in-memory backend"));
        let service = ExpirationService::new(backend, config);

        // With probability 1.0, should always return true
        assert!(service.should_run_cleanup());
    }

    #[test]
    fn test_should_run_cleanup_never() {
        let config = ExpirationConfig::new().with_cleanup_probability(0.0);
        let backend: Arc<dyn IndexBackend + Send + Sync> =
            Arc::new(SqliteBackend::in_memory().expect("in-memory backend"));
        let service = ExpirationService::new(backend, config);

        // With probability 0.0, should always return false
        assert!(!service.should_run_cleanup());
    }

    #[test]
    fn test_gc_expired_memories_dry_run() {
        let backend: Arc<dyn IndexBackend + Send + Sync> =
            Arc::new(SqliteBackend::in_memory().expect("in-memory backend"));
        let config = ExpirationConfig::default();
        let service = ExpirationService::new(Arc::clone(&backend), config);

        // Create a memory that has already expired
        let now = crate::current_timestamp();
        let expired_memory = create_test_memory(
            "expired-1",
            Namespace::Decisions,
            Some(now.saturating_sub(3600)), // Expired 1 hour ago
        );
        backend.index(&expired_memory).expect("index memory");

        // Create a memory that hasn't expired yet
        let future_memory = create_test_memory(
            "future-1",
            Namespace::Decisions,
            Some(now.saturating_add(3600)), // Expires in 1 hour
        );
        backend.index(&future_memory).expect("index memory");

        // Create a memory with no TTL
        let no_ttl_memory = create_test_memory("no-ttl-1", Namespace::Learnings, None);
        backend.index(&no_ttl_memory).expect("index memory");

        // Dry run
        let result = service
            .gc_expired_memories(true)
            .expect("gc should succeed");

        assert_eq!(result.memories_checked, 3);
        assert_eq!(result.memories_tombstoned, 1);
        assert!(result.dry_run);

        // Verify memory is NOT actually tombstoned (dry run)
        let memory = backend
            .get_memory(&MemoryId::new("expired-1"))
            .expect("get memory")
            .expect("memory exists");
        assert!(memory.tombstoned_at.is_none());
    }

    #[test]
    fn test_gc_expired_memories_actual() {
        let backend: Arc<dyn IndexBackend + Send + Sync> =
            Arc::new(SqliteBackend::in_memory().expect("in-memory backend"));
        let config = ExpirationConfig::default();
        let service = ExpirationService::new(Arc::clone(&backend), config);

        // Create a memory that has already expired
        let now = crate::current_timestamp();
        let expired_memory = create_test_memory(
            "expired-2",
            Namespace::Decisions,
            Some(now.saturating_sub(3600)), // Expired 1 hour ago
        );
        backend.index(&expired_memory).expect("index memory");

        // Actual run
        let result = service
            .gc_expired_memories(false)
            .expect("gc should succeed");

        assert_eq!(result.memories_checked, 1);
        assert_eq!(result.memories_tombstoned, 1);
        assert!(!result.dry_run);

        // Verify memory IS actually tombstoned
        let memory = backend
            .get_memory(&MemoryId::new("expired-2"))
            .expect("get memory")
            .expect("memory exists");
        assert!(memory.tombstoned_at.is_some());
    }

    #[test]
    fn test_gc_no_expired_memories() {
        let backend: Arc<dyn IndexBackend + Send + Sync> =
            Arc::new(SqliteBackend::in_memory().expect("in-memory backend"));
        let config = ExpirationConfig::default();
        let service = ExpirationService::new(Arc::clone(&backend), config);

        // Create only non-expired memories
        let now = crate::current_timestamp();
        let future_memory = create_test_memory(
            "future-2",
            Namespace::Decisions,
            Some(now.saturating_add(86400)), // Expires in 1 day
        );
        backend.index(&future_memory).expect("index memory");

        let no_ttl_memory = create_test_memory("no-ttl-2", Namespace::Learnings, None);
        backend.index(&no_ttl_memory).expect("index memory");

        let result = service
            .gc_expired_memories(false)
            .expect("gc should succeed");

        assert_eq!(result.memories_checked, 2);
        assert_eq!(result.memories_tombstoned, 0);
    }

    #[test]
    fn test_rand_float_in_range() {
        // Run multiple times to verify range
        for _ in 0..100 {
            let value = rand_float();
            assert!((0.0..=1.0).contains(&value), "rand_float() = {value}");
        }
    }
}
