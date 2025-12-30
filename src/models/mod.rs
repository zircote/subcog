//! Data models for subcog.
//!
//! This module contains all the core data structures used throughout the system.

mod capture;
mod consolidation;
mod domain;
mod events;
mod memory;
mod search;

pub use capture::{CaptureRequest, CaptureResult};
pub use consolidation::{EdgeType, MemoryTier, RetentionScore};
pub use domain::{Domain, MemoryStatus, Namespace};
pub use events::MemoryEvent;
pub use memory::{Memory, MemoryId, MemoryResult};
pub use search::{DetailLevel, SearchFilter, SearchHit, SearchMode, SearchResult};
