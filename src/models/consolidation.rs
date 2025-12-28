//! Memory consolidation types.

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
        let score = importance * 0.5 + recency * 0.3 + access_frequency * 0.2;

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
