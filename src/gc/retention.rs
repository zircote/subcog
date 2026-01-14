//! Retention policy garbage collector implementation.
//!
//! Identifies and tombstones memories that have exceeded their retention period.
//!
//! # Configuration
//!
//! Retention can be configured via:
//! - Environment variable: `SUBCOG_RETENTION_DAYS` (default: 365)
//! - Config file: `[gc] retention_days = 365`
//! - Per-namespace overrides: `[gc.retention] decisions = 730`
//!
//! # Example
//!
//! ```rust,ignore
//! use subcog::gc::{RetentionConfig, RetentionGarbageCollector};
//! use subcog::storage::index::SqliteBackend;
//! use std::sync::Arc;
//!
//! // Load retention config from environment
//! let config = RetentionConfig::from_env();
//! assert_eq!(config.default_days, 365);
//!
//! // Create retention GC with index backend
//! let backend = Arc::new(SqliteBackend::new("memories.db")?);
//! let gc = RetentionGarbageCollector::new(backend, config);
//!
//! // Dry run to see what would be cleaned up
//! let result = gc.gc_expired_memories(true)?;
//! println!("Would tombstone {} expired memories", result.memories_tombstoned);
//!
//! // Actually perform the cleanup
//! let result = gc.gc_expired_memories(false)?;
//! println!("Tombstoned {} memories", result.memories_tombstoned);
//! ```

use crate::Result;
use crate::models::{Namespace, SearchFilter};
use crate::storage::traits::IndexBackend;
use chrono::{TimeZone, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, info_span, instrument, warn};

/// Environment variable for default retention period in days.
pub const RETENTION_DAYS_ENV: &str = "SUBCOG_RETENTION_DAYS";

/// Default retention period in days (1 year).
pub const DEFAULT_RETENTION_DAYS: u32 = 365;

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

/// Returns the configured retention period in days.
///
/// Reads from `SUBCOG_RETENTION_DAYS` environment variable, defaulting to 365.
#[must_use]
pub fn retention_days() -> u32 {
    std::env::var(RETENTION_DAYS_ENV)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_RETENTION_DAYS)
}

/// Retention policy configuration.
///
/// Supports a default retention period and per-namespace overrides.
#[derive(Debug, Clone)]
pub struct RetentionConfig {
    /// Default retention period in days.
    pub default_days: u32,

    /// Per-namespace retention overrides.
    ///
    /// Namespaces not in this map use `default_days`.
    pub namespace_days: HashMap<Namespace, u32>,

    /// Minimum retention period in days (cannot go below this).
    ///
    /// Provides a safety floor to prevent accidental data loss.
    pub minimum_days: u32,

    /// Maximum memories to process in a single GC run.
    ///
    /// Prevents long-running GC operations.
    pub batch_limit: usize,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            default_days: DEFAULT_RETENTION_DAYS,
            namespace_days: HashMap::new(),
            minimum_days: 30,   // At least 30 days
            batch_limit: 10000, // Process up to 10k memories per run
        }
    }
}

impl RetentionConfig {
    /// Creates a new retention config with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a retention config from environment variables.
    ///
    /// Reads:
    /// - `SUBCOG_RETENTION_DAYS`: Default retention period
    /// - `SUBCOG_RETENTION_MIN_DAYS`: Minimum retention period
    /// - `SUBCOG_RETENTION_BATCH_LIMIT`: Batch limit for GC runs
    /// - `SUBCOG_RETENTION_<NAMESPACE>_DAYS`: Per-namespace overrides
    #[must_use]
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Default retention
        if let Some(d) = std::env::var(RETENTION_DAYS_ENV)
            .ok()
            .and_then(|days| days.parse::<u32>().ok())
        {
            config.default_days = d;
        }

        // Minimum retention
        if let Some(d) = std::env::var("SUBCOG_RETENTION_MIN_DAYS")
            .ok()
            .and_then(|days| days.parse::<u32>().ok())
        {
            config.minimum_days = d;
        }

        // Batch limit
        if let Some(l) = std::env::var("SUBCOG_RETENTION_BATCH_LIMIT")
            .ok()
            .and_then(|limit| limit.parse::<usize>().ok())
        {
            config.batch_limit = l;
        }

        // Per-namespace overrides
        for ns in Namespace::all().iter().copied() {
            let env_key = format!(
                "SUBCOG_RETENTION_{}_DAYS",
                ns.as_str().to_uppercase().replace('-', "_")
            );
            if let Some(d) = std::env::var(&env_key)
                .ok()
                .and_then(|days| days.parse::<u32>().ok())
            {
                config.namespace_days.insert(ns, d);
            }
        }

        config
    }

    /// Sets the default retention period.
    #[must_use]
    pub const fn with_default_days(mut self, days: u32) -> Self {
        self.default_days = days;
        self
    }

    /// Sets the minimum retention period.
    #[must_use]
    pub const fn with_minimum_days(mut self, days: u32) -> Self {
        self.minimum_days = days;
        self
    }

    /// Sets the batch limit.
    #[must_use]
    pub const fn with_batch_limit(mut self, limit: usize) -> Self {
        self.batch_limit = limit;
        self
    }

    /// Sets a per-namespace retention override.
    #[must_use]
    pub fn with_namespace_days(mut self, namespace: Namespace, days: u32) -> Self {
        self.namespace_days.insert(namespace, days);
        self
    }

    /// Gets the effective retention period for a namespace.
    ///
    /// Returns the namespace-specific override if set, otherwise the default.
    /// The result is clamped to be at least `minimum_days`.
    #[must_use]
    pub fn effective_days(&self, namespace: Namespace) -> u32 {
        let days = self
            .namespace_days
            .get(&namespace)
            .copied()
            .unwrap_or(self.default_days);

        // Enforce minimum retention
        days.max(self.minimum_days)
    }

    /// Returns the cutoff timestamp for expired memories in a namespace.
    ///
    /// Memories with `created_at` before this timestamp are considered expired.
    #[must_use]
    pub fn cutoff_timestamp(&self, namespace: Namespace) -> u64 {
        let days = self.effective_days(namespace);
        let now = crate::current_timestamp();
        let seconds_per_day: u64 = 86400;
        now.saturating_sub(u64::from(days) * seconds_per_day)
    }
}

/// Result of a retention garbage collection operation.
#[derive(Debug, Clone, Default)]
pub struct RetentionGcResult {
    /// Total number of memories checked.
    pub memories_checked: usize,

    /// Number of memories that were (or would be) tombstoned.
    pub memories_tombstoned: usize,

    /// Breakdown of tombstoned memories by namespace.
    pub by_namespace: HashMap<String, usize>,

    /// Whether this was a dry run (no actual changes made).
    pub dry_run: bool,

    /// Duration of the GC operation in milliseconds.
    pub duration_ms: u64,
}

impl RetentionGcResult {
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
                "No expired memories found ({} memories checked in {}ms)",
                self.memories_checked, self.duration_ms
            )
        } else {
            let ns_breakdown: Vec<String> = self
                .by_namespace
                .iter()
                .map(|(ns, count)| format!("{ns}: {count}"))
                .collect();

            format!(
                "{} {} expired memories ({}) - checked {} in {}ms",
                action,
                self.memories_tombstoned,
                ns_breakdown.join(", "),
                self.memories_checked,
                self.duration_ms
            )
        }
    }
}

/// Garbage collector for expired memories based on retention policy.
///
/// Identifies memories that have exceeded their retention period and marks
/// them as tombstoned. Supports per-namespace retention policies.
///
/// # Thread Safety
///
/// The garbage collector holds an `Arc` reference to the index backend,
/// making it safe to share across threads.
pub struct RetentionGarbageCollector<I: IndexBackend> {
    /// Reference to the index backend for querying and updating memories.
    index: Arc<I>,

    /// Retention policy configuration.
    config: RetentionConfig,
}

impl<I: IndexBackend> RetentionGarbageCollector<I> {
    /// Creates a new retention garbage collector.
    ///
    /// # Arguments
    ///
    /// * `index` - Shared reference to the index backend.
    /// * `config` - Retention policy configuration.
    #[must_use]
    pub fn new(index: Arc<I>, config: RetentionConfig) -> Self {
        // Arc::strong_count prevents clippy::missing_const_for_fn false positive
        let _ = Arc::strong_count(&index);
        Self { index, config }
    }

    /// Performs garbage collection on expired memories.
    ///
    /// This method:
    /// 1. Iterates through all namespaces
    /// 2. For each namespace, calculates the retention cutoff
    /// 3. Queries for memories older than the cutoff
    /// 4. Tombstones expired memories (unless `dry_run`)
    ///
    /// # Arguments
    ///
    /// * `dry_run` - If true, only report what would be done without making changes
    ///
    /// # Returns
    ///
    /// A `RetentionGcResult` containing statistics about the operation.
    ///
    /// # Errors
    ///
    /// Returns an error if index backend operations fail.
    #[instrument(
        name = "subcog.gc.retention",
        skip(self),
        fields(
            request_id = tracing::field::Empty,
            component = "gc",
            operation = "retention",
            dry_run = dry_run,
            default_retention_days = self.config.default_days
        )
    )]
    pub fn gc_expired_memories(&self, dry_run: bool) -> Result<RetentionGcResult> {
        let start = Instant::now();
        if let Some(request_id) = crate::observability::current_request_id() {
            tracing::Span::current().record("request_id", request_id.as_str());
        }
        let mut result = RetentionGcResult {
            dry_run,
            ..Default::default()
        };

        let now = crate::current_timestamp();

        // Process each namespace with its specific retention policy
        for namespace in Namespace::user_namespaces().iter().copied() {
            let cutoff = self.config.cutoff_timestamp(namespace);
            let retention_days = self.config.effective_days(namespace);
            let _span = info_span!(
                "subcog.gc.retention.namespace",
                namespace = %namespace.as_str(),
                retention_days = retention_days
            )
            .entered();

            debug!(
                namespace = namespace.as_str(),
                retention_days, cutoff, "Processing namespace for retention GC"
            );

            let count = self.process_namespace(namespace, cutoff, now, dry_run, &mut result)?;

            if count > 0 {
                result
                    .by_namespace
                    .insert(namespace.as_str().to_string(), count);
            }
        }

        result.duration_ms = duration_to_millis(start.elapsed());

        // Record metrics
        metrics::counter!(
            "gc_retention_runs_total",
            "dry_run" => dry_run.to_string()
        )
        .increment(1);
        metrics::gauge!("gc_retention_tombstoned").set(usize_to_f64(result.memories_tombstoned));
        metrics::histogram!("gc_retention_duration_ms").record(u64_to_f64(result.duration_ms));
        metrics::histogram!(
            "memory_lifecycle_duration_ms",
            "component" => "gc",
            "operation" => "retention"
        )
        .record(u64_to_f64(result.duration_ms));

        info!(
            memories_checked = result.memories_checked,
            memories_tombstoned = result.memories_tombstoned,
            duration_ms = result.duration_ms,
            dry_run,
            "Retention GC completed"
        );

        Ok(result)
    }

    /// Processes a single namespace for expired memories.
    fn process_namespace(
        &self,
        namespace: Namespace,
        cutoff: u64,
        now: u64,
        dry_run: bool,
        result: &mut RetentionGcResult,
    ) -> Result<usize> {
        let filter = SearchFilter::new()
            .with_namespace(namespace)
            .with_include_tombstoned(false);

        let memories = self.index.list_all(&filter, self.config.batch_limit)?;
        let mut tombstoned = 0;

        for (id, _score) in memories {
            result.memories_checked += 1;

            // Get the full memory to check created_at
            let Some(memory) = self.index.get_memory(&id)? else {
                continue;
            };

            // Check if memory has expired
            if memory.created_at >= cutoff {
                continue;
            }

            // Memory has expired
            if dry_run {
                tombstoned += 1;
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
                tombstoned += 1;
                continue;
            };

            warn!(
                memory_id = %id.as_str(),
                error = %e,
                "Failed to tombstone expired memory"
            );
        }

        result.memories_tombstoned += tombstoned;
        Ok(tombstoned)
    }

    /// Returns the current retention configuration.
    #[must_use]
    pub const fn config(&self) -> &RetentionConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, Memory, MemoryId, MemoryStatus};
    use crate::storage::index::SqliteBackend;

    fn create_test_memory(id: &str, namespace: Namespace, created_at: u64) -> Memory {
        Memory {
            id: MemoryId::new(id),
            content: format!("Test memory {id}"),
            namespace,
            domain: Domain::new(),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at,
            updated_at: created_at,
            tombstoned_at: None,
            expires_at: None,
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
    fn test_retention_config_default() {
        let config = RetentionConfig::default();
        assert_eq!(config.default_days, 365);
        assert_eq!(config.minimum_days, 30);
        assert_eq!(config.batch_limit, 10000);
    }

    #[test]
    fn test_retention_config_builders() {
        let config = RetentionConfig::new()
            .with_default_days(180)
            .with_minimum_days(7)
            .with_batch_limit(5000)
            .with_namespace_days(Namespace::Decisions, 730);

        assert_eq!(config.default_days, 180);
        assert_eq!(config.minimum_days, 7);
        assert_eq!(config.batch_limit, 5000);
        assert_eq!(config.namespace_days.get(&Namespace::Decisions), Some(&730));
    }

    #[test]
    fn test_effective_days_with_override() {
        let config = RetentionConfig::new()
            .with_default_days(365)
            .with_namespace_days(Namespace::Decisions, 730);

        // Namespace with override
        assert_eq!(config.effective_days(Namespace::Decisions), 730);

        // Namespace without override uses default
        assert_eq!(config.effective_days(Namespace::Learnings), 365);
    }

    #[test]
    fn test_effective_days_minimum_enforced() {
        let config = RetentionConfig::new()
            .with_default_days(10) // Below minimum
            .with_minimum_days(30);

        // Should be clamped to minimum
        assert_eq!(config.effective_days(Namespace::Patterns), 30);
    }

    #[test]
    fn test_cutoff_timestamp() {
        let config = RetentionConfig::new().with_default_days(30);

        let cutoff = config.cutoff_timestamp(Namespace::Decisions);
        let now = crate::current_timestamp();
        let expected = now - (30 * 86400);

        // Allow 1 second tolerance for test timing
        assert!(cutoff.abs_diff(expected) <= 1);
    }

    #[test]
    fn test_retention_gc_result_summary_no_expired() {
        let result = RetentionGcResult {
            memories_checked: 100,
            memories_tombstoned: 0,
            by_namespace: HashMap::new(),
            dry_run: false,
            duration_ms: 50,
        };

        assert!(!result.has_expired_memories());
        assert!(result.summary().contains("No expired memories"));
        assert!(result.summary().contains("100 memories checked"));
    }

    #[test]
    fn test_retention_gc_result_summary_with_expired() {
        let mut by_namespace = HashMap::new();
        by_namespace.insert("decisions".to_string(), 5);
        by_namespace.insert("learnings".to_string(), 3);

        let result = RetentionGcResult {
            memories_checked: 100,
            memories_tombstoned: 8,
            by_namespace,
            dry_run: false,
            duration_ms: 75,
        };

        assert!(result.has_expired_memories());
        let summary = result.summary();
        assert!(summary.contains("tombstoned 8 expired memories"));
    }

    #[test]
    fn test_retention_gc_result_summary_dry_run() {
        let mut by_namespace = HashMap::new();
        by_namespace.insert("decisions".to_string(), 5);

        let result = RetentionGcResult {
            memories_checked: 50,
            memories_tombstoned: 5,
            by_namespace,
            dry_run: true,
            duration_ms: 25,
        };

        let summary = result.summary();
        assert!(summary.contains("would tombstone"));
    }

    #[test]
    fn test_gc_no_expired_memories() {
        let backend = Arc::new(SqliteBackend::in_memory().expect("Failed to create backend"));

        // Create a recent memory
        let now = crate::current_timestamp();
        let memory = create_test_memory("mem1", Namespace::Decisions, now);
        backend.index(&memory).expect("Failed to index memory");

        let config = RetentionConfig::new().with_default_days(30);
        let gc = RetentionGarbageCollector::new(Arc::clone(&backend), config);

        let result = gc.gc_expired_memories(false).expect("GC should succeed");

        assert!(!result.has_expired_memories());
        assert_eq!(result.memories_checked, 1);
        assert_eq!(result.memories_tombstoned, 0);
    }

    #[test]
    fn test_gc_expired_memory_dry_run() {
        let backend = Arc::new(SqliteBackend::in_memory().expect("Failed to create backend"));

        // Create an old memory (400 days ago)
        let now = crate::current_timestamp();
        let old_timestamp = now - (400 * 86400);
        let memory = create_test_memory("mem1", Namespace::Decisions, old_timestamp);
        backend.index(&memory).expect("Failed to index memory");

        let config = RetentionConfig::new().with_default_days(365);
        let gc = RetentionGarbageCollector::new(Arc::clone(&backend), config);

        let result = gc.gc_expired_memories(true).expect("GC should succeed");

        assert!(result.has_expired_memories());
        assert_eq!(result.memories_tombstoned, 1);
        assert!(result.dry_run);

        // Memory should NOT be tombstoned in dry run
        let memory = backend
            .get_memory(&MemoryId::new("mem1"))
            .expect("Failed to get memory")
            .expect("Memory should exist");
        assert!(memory.tombstoned_at.is_none());
    }

    #[test]
    fn test_gc_expired_memory_actual() {
        let backend = Arc::new(SqliteBackend::in_memory().expect("Failed to create backend"));

        // Create an old memory (400 days ago)
        let now = crate::current_timestamp();
        let old_timestamp = now - (400 * 86400);
        let memory = create_test_memory("mem1", Namespace::Decisions, old_timestamp);
        backend.index(&memory).expect("Failed to index memory");

        let config = RetentionConfig::new().with_default_days(365);
        let gc = RetentionGarbageCollector::new(Arc::clone(&backend), config);

        let result = gc.gc_expired_memories(false).expect("GC should succeed");

        assert!(result.has_expired_memories());
        assert_eq!(result.memories_tombstoned, 1);
        assert!(!result.dry_run);

        // Memory SHOULD be tombstoned
        let memory = backend
            .get_memory(&MemoryId::new("mem1"))
            .expect("Failed to get memory")
            .expect("Memory should exist");
        assert!(memory.tombstoned_at.is_some());
    }

    #[test]
    fn test_gc_per_namespace_retention() {
        let backend = Arc::new(SqliteBackend::in_memory().expect("Failed to create backend"));

        let now = crate::current_timestamp();

        // Create a memory 100 days old in decisions
        let decisions_mem =
            create_test_memory("decisions1", Namespace::Decisions, now - (100 * 86400));
        backend
            .index(&decisions_mem)
            .expect("Failed to index memory");

        // Create a memory 100 days old in learnings
        let learnings_mem =
            create_test_memory("learnings1", Namespace::Learnings, now - (100 * 86400));
        backend
            .index(&learnings_mem)
            .expect("Failed to index memory");

        // Config: decisions retained 730 days, learnings 90 days
        let config = RetentionConfig::new()
            .with_default_days(90)
            .with_namespace_days(Namespace::Decisions, 730);

        let gc = RetentionGarbageCollector::new(Arc::clone(&backend), config);

        let result = gc.gc_expired_memories(false).expect("GC should succeed");

        // Only learnings should be expired (100 > 90)
        // Decisions should NOT be expired (100 < 730)
        assert_eq!(result.memories_tombstoned, 1);

        // Verify decisions is not tombstoned
        let decisions = backend
            .get_memory(&MemoryId::new("decisions1"))
            .expect("Failed to get memory")
            .expect("Memory should exist");
        assert!(decisions.tombstoned_at.is_none());

        // Verify learnings is tombstoned
        let learnings = backend
            .get_memory(&MemoryId::new("learnings1"))
            .expect("Failed to get memory")
            .expect("Memory should exist");
        assert!(learnings.tombstoned_at.is_some());
    }

    #[test]
    fn test_retention_days_from_env() {
        // Default when env not set
        let days = retention_days();
        assert_eq!(days, DEFAULT_RETENTION_DAYS);
    }
}
