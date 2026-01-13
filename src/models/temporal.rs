//! Bitemporal types for knowledge graph time tracking.
//!
//! This module provides types for implementing bitemporal data models,
//! enabling queries like "what did we know at time T?" and "when was this fact true?"
//!
//! # Bitemporal Concepts
//!
//! Bitemporal data tracking uses two independent time dimensions:
//!
//! | Dimension | Question Answered | Example |
//! |-----------|-------------------|---------|
//! | **Valid Time** | When was this fact true in the real world? | "Alice worked at Acme from 2020-2023" |
//! | **Transaction Time** | When was this fact recorded in the system? | "We learned this on 2024-01-15" |
//!
//! # Valid Time Semantics
//!
//! Valid time represents when a fact was/is/will be true in the real world:
//!
//! - `ValidTimeRange::unbounded()` - Always true (default for most entities)
//! - `ValidTimeRange::from(start)` - True from `start` onwards
//! - `ValidTimeRange::until(end)` - True until `end`
//! - `ValidTimeRange::between(start, end)` - True during the interval
//!
//! # Transaction Time Semantics
//!
//! Transaction time is automatically set when a record is created and never modified.
//! This enables auditing and point-in-time queries ("what did the system know at time T?").
//!
//! # Example
//!
//! ```rust
//! use subcog::models::temporal::{ValidTimeRange, TransactionTime};
//!
//! // Entity was valid from Jan 1, 2024 onwards
//! let valid_time = ValidTimeRange::from(1704067200);
//!
//! // Check if valid at a specific point
//! assert!(valid_time.contains(1704153600)); // Jan 2, 2024
//! assert!(!valid_time.contains(1703980800)); // Dec 31, 2023
//!
//! // Transaction time is auto-set to now
//! let tx_time = TransactionTime::now();
//! assert!(tx_time.timestamp() > 0);
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents when a fact was true in the real world (valid time).
///
/// This is a half-open interval `[start, end)` where:
/// - `start` is inclusive (None means unbounded past)
/// - `end` is exclusive (None means unbounded future)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ValidTimeRange {
    /// Start of validity (inclusive), None for unbounded past.
    pub start: Option<i64>,
    /// End of validity (exclusive), None for unbounded future.
    pub end: Option<i64>,
}

impl ValidTimeRange {
    /// Creates an unbounded time range (always valid).
    #[must_use]
    pub const fn unbounded() -> Self {
        Self {
            start: None,
            end: None,
        }
    }

    /// Creates a time range starting from a given timestamp.
    #[must_use]
    pub const fn from(start: i64) -> Self {
        Self {
            start: Some(start),
            end: None,
        }
    }

    /// Creates a time range ending at a given timestamp.
    #[must_use]
    pub const fn until(end: i64) -> Self {
        Self {
            start: None,
            end: Some(end),
        }
    }

    /// Creates a bounded time range.
    #[must_use]
    pub const fn between(start: i64, end: i64) -> Self {
        Self {
            start: Some(start),
            end: Some(end),
        }
    }

    /// Creates a time range starting from now.
    #[must_use]
    pub fn from_now() -> Self {
        Self::from(current_timestamp())
    }

    /// Creates a time range ending now.
    #[must_use]
    pub fn until_now() -> Self {
        Self::until(current_timestamp())
    }

    /// Checks if the given timestamp falls within this range.
    ///
    /// Uses half-open interval semantics: `[start, end)`.
    #[must_use]
    pub const fn contains(&self, timestamp: i64) -> bool {
        let after_start = match self.start {
            Some(s) => timestamp >= s,
            None => true,
        };
        let before_end = match self.end {
            Some(e) => timestamp < e,
            None => true,
        };
        after_start && before_end
    }

    /// Checks if this range is currently valid.
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.contains(current_timestamp())
    }

    /// Checks if this range is unbounded on both ends.
    #[must_use]
    pub const fn is_unbounded(&self) -> bool {
        self.start.is_none() && self.end.is_none()
    }

    /// Checks if this range has started (start is in the past or unbounded).
    #[must_use]
    pub fn has_started(&self) -> bool {
        self.start.is_none_or(|s| current_timestamp() >= s)
    }

    /// Checks if this range has ended (end is in the past).
    #[must_use]
    pub fn has_ended(&self) -> bool {
        self.end.is_some_and(|e| current_timestamp() >= e)
    }

    /// Returns the overlap of this range with another, if any.
    #[must_use]
    pub fn overlap(&self, other: &Self) -> Option<Self> {
        let start = match (self.start, other.start) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };

        let end = match (self.end, other.end) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };

        // Check if the interval is valid (start < end)
        if let (Some(s), Some(e)) = (start, end)
            && s >= e
        {
            return None;
        }

        Some(Self { start, end })
    }

    /// Ends this range at the given timestamp.
    ///
    /// Useful for "closing" an open-ended range when a fact becomes invalid.
    #[must_use]
    pub const fn close_at(self, end: i64) -> Self {
        Self {
            start: self.start,
            end: Some(end),
        }
    }

    /// Ends this range now.
    #[must_use]
    pub fn close_now(self) -> Self {
        self.close_at(current_timestamp())
    }
}

impl Default for ValidTimeRange {
    fn default() -> Self {
        Self::unbounded()
    }
}

impl fmt::Display for ValidTimeRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.start, self.end) {
            (None, None) => write!(f, "[∞, ∞)"),
            (Some(s), None) => write!(f, "[{s}, ∞)"),
            (None, Some(e)) => write!(f, "[∞, {e})"),
            (Some(s), Some(e)) => write!(f, "[{s}, {e})"),
        }
    }
}

/// Represents when a fact was recorded in the system (transaction time).
///
/// Transaction time is immutable after creation - it captures the moment
/// when the system learned about a fact. This enables point-in-time queries
/// and auditing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionTime {
    /// Unix timestamp when the record was created.
    timestamp: i64,
}

impl TransactionTime {
    /// Creates a transaction time at the current moment.
    #[must_use]
    pub fn now() -> Self {
        Self {
            timestamp: current_timestamp(),
        }
    }

    /// Creates a transaction time at a specific timestamp.
    ///
    /// This should only be used for importing historical data or testing.
    #[must_use]
    pub const fn at(timestamp: i64) -> Self {
        Self { timestamp }
    }

    /// Returns the timestamp.
    #[must_use]
    pub const fn timestamp(&self) -> i64 {
        self.timestamp
    }

    /// Checks if this transaction time is before another.
    #[must_use]
    pub const fn is_before(&self, other: &Self) -> bool {
        self.timestamp < other.timestamp
    }

    /// Checks if this transaction time is after another.
    #[must_use]
    pub const fn is_after(&self, other: &Self) -> bool {
        self.timestamp > other.timestamp
    }

    /// Checks if this transaction occurred before or at the given timestamp.
    #[must_use]
    pub const fn was_known_at(&self, timestamp: i64) -> bool {
        self.timestamp <= timestamp
    }
}

impl Default for TransactionTime {
    fn default() -> Self {
        Self::now()
    }
}

impl fmt::Display for TransactionTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tx@{}", self.timestamp)
    }
}

impl From<i64> for TransactionTime {
    fn from(timestamp: i64) -> Self {
        Self::at(timestamp)
    }
}

/// A bitemporal point representing both valid and transaction time.
///
/// This is useful for querying "what was known to be true at time T1,
/// as of system time T2?"
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BitemporalPoint {
    /// Point in valid time to query.
    pub valid_at: i64,
    /// Point in transaction time to query.
    pub as_of: i64,
}

impl BitemporalPoint {
    /// Creates a bitemporal point.
    #[must_use]
    pub const fn new(valid_at: i64, as_of: i64) -> Self {
        Self { valid_at, as_of }
    }

    /// Creates a bitemporal point for "now" in both dimensions.
    #[must_use]
    pub fn now() -> Self {
        let ts = current_timestamp();
        Self {
            valid_at: ts,
            as_of: ts,
        }
    }

    /// Checks if a record with given temporal metadata is visible at this point.
    #[must_use]
    pub const fn is_visible(&self, valid_time: &ValidTimeRange, tx_time: &TransactionTime) -> bool {
        valid_time.contains(self.valid_at) && tx_time.was_known_at(self.as_of)
    }
}

impl Default for BitemporalPoint {
    fn default() -> Self {
        Self::now()
    }
}

impl fmt::Display for BitemporalPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "valid@{}, as_of@{}", self.valid_at, self.as_of)
    }
}

/// Returns the current Unix timestamp in seconds.
#[must_use]
#[allow(clippy::cast_possible_wrap)]
pub fn current_timestamp() -> i64 {
    // Cast is safe: u64::MAX seconds won't occur until year 292277026596
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_time_range_unbounded() {
        let range = ValidTimeRange::unbounded();
        assert!(range.is_unbounded());
        assert!(range.contains(0));
        assert!(range.contains(i64::MAX));
        assert!(range.contains(i64::MIN));
    }

    #[test]
    fn test_valid_time_range_from() {
        let range = ValidTimeRange::from(100);
        assert!(!range.is_unbounded());
        assert!(!range.contains(99));
        assert!(range.contains(100));
        assert!(range.contains(101));
        assert!(range.contains(i64::MAX));
    }

    #[test]
    fn test_valid_time_range_until() {
        let range = ValidTimeRange::until(100);
        assert!(!range.is_unbounded());
        assert!(range.contains(99));
        assert!(!range.contains(100)); // End is exclusive
        assert!(range.contains(i64::MIN));
    }

    #[test]
    fn test_valid_time_range_between() {
        let range = ValidTimeRange::between(100, 200);
        assert!(!range.is_unbounded());
        assert!(!range.contains(99));
        assert!(range.contains(100));
        assert!(range.contains(150));
        assert!(range.contains(199));
        assert!(!range.contains(200)); // End is exclusive
    }

    #[test]
    fn test_valid_time_range_overlap() {
        let r1 = ValidTimeRange::between(100, 200);
        let r2 = ValidTimeRange::between(150, 250);

        let overlap = r1.overlap(&r2);
        assert!(overlap.is_some());
        let overlap = overlap.unwrap();
        assert_eq!(overlap.start, Some(150));
        assert_eq!(overlap.end, Some(200));
    }

    #[test]
    fn test_valid_time_range_no_overlap() {
        let r1 = ValidTimeRange::between(100, 200);
        let r2 = ValidTimeRange::between(200, 300);

        let overlap = r1.overlap(&r2);
        assert!(overlap.is_none());
    }

    #[test]
    fn test_valid_time_range_close() {
        let range = ValidTimeRange::from(100);
        assert!(range.end.is_none());

        let closed = range.close_at(200);
        assert_eq!(closed.start, Some(100));
        assert_eq!(closed.end, Some(200));
    }

    #[test]
    fn test_transaction_time() {
        let tx = TransactionTime::now();
        assert!(tx.timestamp() > 0);

        let tx2 = TransactionTime::at(100);
        assert_eq!(tx2.timestamp(), 100);
        assert!(tx2.is_before(&tx));
        assert!(tx.is_after(&tx2));
    }

    #[test]
    fn test_transaction_time_was_known_at() {
        let tx = TransactionTime::at(100);
        assert!(tx.was_known_at(100));
        assert!(tx.was_known_at(101));
        assert!(!tx.was_known_at(99));
    }

    #[test]
    fn test_bitemporal_point() {
        let point = BitemporalPoint::new(150, 200);

        // Record was valid from 100-200, created at tx_time 50
        let valid_time = ValidTimeRange::between(100, 200);
        let tx_time = TransactionTime::at(50);

        // At point (valid_at=150, as_of=200):
        // - 150 is in [100, 200) ✓
        // - tx_time 50 <= as_of 200 ✓
        assert!(point.is_visible(&valid_time, &tx_time));

        // Record created after our as_of point
        let tx_time_future = TransactionTime::at(250);
        assert!(!point.is_visible(&valid_time, &tx_time_future));

        // Record not valid at our valid_at point
        let valid_time_past = ValidTimeRange::between(50, 100);
        assert!(!point.is_visible(&valid_time_past, &tx_time));
    }

    #[test]
    fn test_display_formats() {
        assert_eq!(ValidTimeRange::unbounded().to_string(), "[∞, ∞)");
        assert_eq!(ValidTimeRange::from(100).to_string(), "[100, ∞)");
        assert_eq!(ValidTimeRange::until(200).to_string(), "[∞, 200)");
        assert_eq!(ValidTimeRange::between(100, 200).to_string(), "[100, 200)");

        assert_eq!(TransactionTime::at(100).to_string(), "tx@100");
        assert_eq!(
            BitemporalPoint::new(100, 200).to_string(),
            "valid@100, as_of@200"
        );
    }

    #[test]
    fn test_defaults() {
        let valid_time = ValidTimeRange::default();
        assert!(valid_time.is_unbounded());

        let tx_time = TransactionTime::default();
        assert!(tx_time.timestamp() > 0);

        let point = BitemporalPoint::default();
        assert!(point.valid_at > 0);
        assert!(point.as_of > 0);
    }
}
