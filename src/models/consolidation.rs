//! Memory consolidation types for lifecycle management.
//!
//! This module provides types for managing memory retention, tiering, and
//! relationships between memories during consolidation operations.
//!
//! # Memory Tiers
//!
//! Memories are organized into tiers based on access patterns and importance:
//!
//! | Tier | Score Range | Behavior |
//! |------|-------------|----------|
//! | Hot | ≥ 0.7 | Frequently accessed, high priority |
//! | Warm | 0.4 - 0.7 | Moderately accessed (default) |
//! | Cold | 0.2 - 0.4 | Rarely accessed, low priority |
//! | Archive | < 0.2 | Long-term storage only |
//!
//! # Edge Types
//!
//! Relationships between memories are modeled as directed edges:
//!
//! - `Contradicts` - Memory A conflicts with memory B
//! - `Supersedes` - Memory A replaces memory B
//! - `RelatedTo` - Memory A is contextually related to memory B
//! - `Refines` - Memory A adds detail to memory B
//! - `ParentOf` / `ChildOf` - Hierarchical relationships
//! - `SummarizedBy` / `SourceOf` - Consolidation relationships (original → summary, summary → originals)
//!
//! # Retention Scoring
//!
//! [`RetentionScore`] calculates a composite score from:
//! - **Access frequency** (20% weight) - How often the memory is retrieved
//! - **Recency** (30% weight) - When the memory was last accessed
//! - **Importance** (50% weight) - LLM-assessed significance
//!
//! # Example
//!
//! ```rust
//! use subcog::models::{MemoryTier, RetentionScore};
//!
//! // Create a retention score for a frequently-accessed, recent, important memory
//! let score = RetentionScore::new(0.8, 0.9, 0.95);
//! assert_eq!(score.suggested_tier(), MemoryTier::Hot);
//!
//! // A rarely-accessed, old, low-importance memory
//! let cold_score = RetentionScore::new(0.1, 0.2, 0.1);
//! assert_eq!(cold_score.suggested_tier(), MemoryTier::Archive);
//! ```

use std::fmt;

/// Memory tier for retention management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MemoryTier {
    /// Hot tier: frequently accessed, high priority.
    Hot,
    /// Warm tier: moderately accessed (default).
    #[default]
    Warm,
    /// Cold tier: rarely accessed, low priority.
    Cold,
    /// Archive tier: long-term storage only.
    Archive,
}

impl MemoryTier {
    /// Returns the tier as a string slice.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Hot => "hot",
            Self::Warm => "warm",
            Self::Cold => "cold",
            Self::Archive => "archive",
        }
    }
}

impl fmt::Display for MemoryTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Type of relationship edge between memories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeType {
    /// Memory A contradicts memory B.
    Contradicts,
    /// Memory A supersedes memory B.
    Supersedes,
    /// Memory A is related to memory B.
    RelatedTo,
    /// Memory A refines memory B.
    Refines,
    /// Memory A is a parent of memory B (hierarchy).
    ParentOf,
    /// Memory A is a child of memory B (hierarchy).
    ChildOf,
    /// Memory A is summarized by memory B (A is an original, B is the summary).
    SummarizedBy,
    /// Memory A is a source of memory B (A is a summary, B is an original).
    SourceOf,
}

impl EdgeType {
    /// Returns the edge type as a string slice.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Contradicts => "contradicts",
            Self::Supersedes => "supersedes",
            Self::RelatedTo => "related_to",
            Self::Refines => "refines",
            Self::ParentOf => "parent_of",
            Self::ChildOf => "child_of",
            Self::SummarizedBy => "summarized_by",
            Self::SourceOf => "source_of",
        }
    }

    /// Returns the inverse edge type.
    #[must_use]
    pub const fn inverse(&self) -> Self {
        match self {
            Self::Contradicts => Self::Contradicts,
            Self::Supersedes => Self::Supersedes, // No inverse defined
            Self::RelatedTo => Self::RelatedTo,
            Self::Refines => Self::Refines, // No inverse defined
            Self::ParentOf => Self::ChildOf,
            Self::ChildOf => Self::ParentOf,
            Self::SummarizedBy => Self::SourceOf,
            Self::SourceOf => Self::SummarizedBy,
        }
    }
}

impl fmt::Display for EdgeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Retention score for memory lifecycle management.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RetentionScore {
    /// Overall score (0.0 to 1.0, higher = keep longer).
    score: f32,
    /// Access frequency component.
    access_frequency: f32,
    /// Recency component.
    recency: f32,
    /// Importance component (from LLM analysis).
    importance: f32,
}

impl RetentionScore {
    /// Creates a new retention score.
    ///
    /// # Panics
    ///
    /// Panics if any component is outside the 0.0 to 1.0 range.
    #[must_use]
    pub fn new(access_frequency: f32, recency: f32, importance: f32) -> Self {
        debug_assert!(
            (0.0..=1.0).contains(&access_frequency),
            "access_frequency must be 0.0 to 1.0"
        );
        debug_assert!((0.0..=1.0).contains(&recency), "recency must be 0.0 to 1.0");
        debug_assert!(
            (0.0..=1.0).contains(&importance),
            "importance must be 0.0 to 1.0"
        );

        // Weighted average: importance > recency > frequency
        let score = access_frequency.mul_add(0.2, importance.mul_add(0.5, recency * 0.3));

        Self {
            score,
            access_frequency,
            recency,
            importance,
        }
    }

    /// Returns the overall score.
    #[must_use]
    pub const fn score(&self) -> f32 {
        self.score
    }

    /// Returns the suggested tier based on score.
    #[must_use]
    pub fn suggested_tier(&self) -> MemoryTier {
        if self.score >= 0.7 {
            MemoryTier::Hot
        } else if self.score >= 0.4 {
            MemoryTier::Warm
        } else if self.score >= 0.2 {
            MemoryTier::Cold
        } else {
            MemoryTier::Archive
        }
    }
}

impl Default for RetentionScore {
    fn default() -> Self {
        Self::new(0.5, 0.5, 0.5)
    }
}
