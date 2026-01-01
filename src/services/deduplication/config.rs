//! Deduplication configuration.
//!
//! This module defines configuration for the deduplication service,
//! including per-namespace similarity thresholds and cache settings.

use crate::models::Namespace;
use std::collections::HashMap;
use std::time::Duration;

/// Configuration for the deduplication service.
///
/// # Environment Variables
///
/// | Variable | Type | Default | Description |
/// |----------|------|---------|-------------|
/// | `SUBCOG_DEDUP_ENABLED` | bool | `true` | Enable deduplication |
/// | `SUBCOG_DEDUP_THRESHOLD_DECISIONS` | f32 | `0.92` | Threshold for decisions namespace |
/// | `SUBCOG_DEDUP_THRESHOLD_PATTERNS` | f32 | `0.90` | Threshold for patterns namespace |
/// | `SUBCOG_DEDUP_THRESHOLD_LEARNINGS` | f32 | `0.88` | Threshold for learnings namespace |
/// | `SUBCOG_DEDUP_THRESHOLD_DEFAULT` | f32 | `0.90` | Default threshold |
/// | `SUBCOG_DEDUP_TIME_WINDOW_SECS` | u64 | `300` | Recent capture window |
/// | `SUBCOG_DEDUP_CACHE_CAPACITY` | usize | `1000` | LRU cache size |
/// | `SUBCOG_DEDUP_MIN_SEMANTIC_LENGTH` | usize | `50` | Min content length for semantic check |
///
/// # Example
///
/// ```rust
/// use subcog::services::deduplication::DeduplicationConfig;
/// use subcog::models::Namespace;
///
/// let config = DeduplicationConfig::default();
/// assert!(config.enabled);
/// assert_eq!(config.default_threshold, 0.90);
/// assert_eq!(config.get_threshold(Namespace::Decisions), 0.92);
/// ```
#[derive(Debug, Clone)]
pub struct DeduplicationConfig {
    /// Enable/disable entire deduplication.
    pub enabled: bool,

    /// Per-namespace similarity thresholds.
    pub similarity_thresholds: HashMap<Namespace, f32>,

    /// Default threshold when namespace not configured.
    pub default_threshold: f32,

    /// Recent capture time window.
    pub recent_window: Duration,

    /// Recent capture cache capacity.
    pub cache_capacity: usize,

    /// Minimum content length for semantic check.
    ///
    /// Content shorter than this will skip semantic similarity checking
    /// and rely only on exact match and recent capture detection.
    pub min_semantic_length: usize,
}

impl DeduplicationConfig {
    /// Creates a new configuration from environment variables.
    ///
    /// Falls back to defaults for any unset variables.
    ///
    /// # Example
    ///
    /// ```rust
    /// use subcog::services::deduplication::DeduplicationConfig;
    ///
    /// let config = DeduplicationConfig::from_env();
    /// // Config is populated from environment with defaults
    /// ```
    #[must_use]
    pub fn from_env() -> Self {
        let enabled = std::env::var("SUBCOG_DEDUP_ENABLED")
            .map(|v| v.to_lowercase() != "false" && v != "0")
            .unwrap_or(true);

        let default_threshold = std::env::var("SUBCOG_DEDUP_THRESHOLD_DEFAULT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.90);

        let recent_window_secs = std::env::var("SUBCOG_DEDUP_TIME_WINDOW_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300);

        let cache_capacity = std::env::var("SUBCOG_DEDUP_CACHE_CAPACITY")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1000);

        let min_semantic_length = std::env::var("SUBCOG_DEDUP_MIN_SEMANTIC_LENGTH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50);

        let mut thresholds = HashMap::new();

        // Load per-namespace thresholds
        if let Ok(v) = std::env::var("SUBCOG_DEDUP_THRESHOLD_DECISIONS") {
            if let Ok(threshold) = v.parse() {
                thresholds.insert(Namespace::Decisions, threshold);
            }
        }

        if let Ok(v) = std::env::var("SUBCOG_DEDUP_THRESHOLD_PATTERNS") {
            if let Ok(threshold) = v.parse() {
                thresholds.insert(Namespace::Patterns, threshold);
            }
        }

        if let Ok(v) = std::env::var("SUBCOG_DEDUP_THRESHOLD_LEARNINGS") {
            if let Ok(threshold) = v.parse() {
                thresholds.insert(Namespace::Learnings, threshold);
            }
        }

        if let Ok(v) = std::env::var("SUBCOG_DEDUP_THRESHOLD_BLOCKERS") {
            if let Ok(threshold) = v.parse() {
                thresholds.insert(Namespace::Blockers, threshold);
            }
        }

        if let Ok(v) = std::env::var("SUBCOG_DEDUP_THRESHOLD_TECHDEBT") {
            if let Ok(threshold) = v.parse() {
                thresholds.insert(Namespace::TechDebt, threshold);
            }
        }

        if let Ok(v) = std::env::var("SUBCOG_DEDUP_THRESHOLD_CONTEXT") {
            if let Ok(threshold) = v.parse() {
                thresholds.insert(Namespace::Context, threshold);
            }
        }

        Self {
            enabled,
            similarity_thresholds: thresholds,
            default_threshold,
            recent_window: Duration::from_secs(recent_window_secs),
            cache_capacity,
            min_semantic_length,
        }
    }

    /// Gets the similarity threshold for a namespace.
    ///
    /// Returns the namespace-specific threshold if configured,
    /// otherwise returns the default threshold.
    ///
    /// # Arguments
    ///
    /// * `namespace` - The namespace to get the threshold for
    ///
    /// # Example
    ///
    /// ```rust
    /// use subcog::services::deduplication::DeduplicationConfig;
    /// use subcog::models::Namespace;
    ///
    /// let config = DeduplicationConfig::default();
    /// assert_eq!(config.get_threshold(Namespace::Decisions), 0.92);
    /// assert_eq!(config.get_threshold(Namespace::Patterns), 0.90);
    /// ```
    #[must_use]
    pub fn get_threshold(&self, namespace: Namespace) -> f32 {
        self.similarity_thresholds
            .get(&namespace)
            .copied()
            .unwrap_or(self.default_threshold)
    }

    /// Builder method to set enabled state.
    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Builder method to set a namespace threshold.
    #[must_use]
    pub fn with_threshold(mut self, namespace: Namespace, threshold: f32) -> Self {
        self.similarity_thresholds.insert(namespace, threshold);
        self
    }

    /// Builder method to set the default threshold.
    #[must_use]
    pub const fn with_default_threshold(mut self, threshold: f32) -> Self {
        self.default_threshold = threshold;
        self
    }

    /// Builder method to set the recent window duration.
    #[must_use]
    pub const fn with_recent_window(mut self, duration: Duration) -> Self {
        self.recent_window = duration;
        self
    }

    /// Builder method to set the cache capacity.
    #[must_use]
    pub const fn with_cache_capacity(mut self, capacity: usize) -> Self {
        self.cache_capacity = capacity;
        self
    }

    /// Builder method to set the minimum semantic length.
    #[must_use]
    pub const fn with_min_semantic_length(mut self, length: usize) -> Self {
        self.min_semantic_length = length;
        self
    }
}

impl Default for DeduplicationConfig {
    fn default() -> Self {
        let mut thresholds = HashMap::new();

        // Per ADR-003: Per-Namespace Similarity Thresholds
        thresholds.insert(Namespace::Decisions, 0.92); // High value, avoid losing unique decisions
        thresholds.insert(Namespace::Patterns, 0.90); // Standard threshold
        thresholds.insert(Namespace::Learnings, 0.88); // Learnings often phrased differently

        Self {
            enabled: true,
            similarity_thresholds: thresholds,
            default_threshold: 0.90,
            recent_window: Duration::from_secs(300), // 5 minutes
            cache_capacity: 1000,
            min_semantic_length: 50,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper for float comparisons in tests.
    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < f32::EPSILON
    }

    #[test]
    fn test_default_config() {
        let config = DeduplicationConfig::default();

        assert!(config.enabled);
        assert!(approx_eq(config.default_threshold, 0.90));
        assert_eq!(config.recent_window, Duration::from_secs(300));
        assert_eq!(config.cache_capacity, 1000);
        assert_eq!(config.min_semantic_length, 50);
    }

    #[test]
    fn test_namespace_thresholds() {
        let config = DeduplicationConfig::default();

        // Configured namespaces return their specific thresholds
        assert!(approx_eq(config.get_threshold(Namespace::Decisions), 0.92));
        assert!(approx_eq(config.get_threshold(Namespace::Patterns), 0.90));
        assert!(approx_eq(config.get_threshold(Namespace::Learnings), 0.88));

        // Unconfigured namespaces return the default
        assert!(approx_eq(config.get_threshold(Namespace::Blockers), 0.90));
        assert!(approx_eq(config.get_threshold(Namespace::TechDebt), 0.90));
    }

    #[test]
    fn test_builder_methods() {
        let config = DeduplicationConfig::default()
            .with_enabled(false)
            .with_default_threshold(0.85)
            .with_threshold(Namespace::Context, 0.95)
            .with_recent_window(Duration::from_secs(600))
            .with_cache_capacity(500)
            .with_min_semantic_length(100);

        assert!(!config.enabled);
        assert!(approx_eq(config.default_threshold, 0.85));
        assert!(approx_eq(config.get_threshold(Namespace::Context), 0.95));
        assert_eq!(config.recent_window, Duration::from_secs(600));
        assert_eq!(config.cache_capacity, 500);
        assert_eq!(config.min_semantic_length, 100);
    }
}
