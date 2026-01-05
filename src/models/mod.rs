//! Data models for subcog.
//!
//! This module contains all the core data structures used throughout the system.

mod capture;
mod consolidation;
mod domain;
mod events;
mod memory;
mod prompt;
mod search;

pub use capture::{CaptureRequest, CaptureResult};
pub use consolidation::{EdgeType, MemoryTier, RetentionScore};
pub use domain::{Domain, MemoryStatus, Namespace};
pub use events::{EventMeta, MemoryEvent};
pub use memory::{Memory, MemoryId, MemoryResult};
pub use prompt::{
    ExtractedVariable, IssueSeverity, MAX_VARIABLE_VALUE_LENGTH, PromptTemplate, PromptVariable,
    ValidationIssue, ValidationResult, extract_variables, is_reserved_variable_name,
    sanitize_variable_value, substitute_variables, validate_prompt_content,
};
pub use search::{DetailLevel, SearchFilter, SearchHit, SearchMode, SearchResult};
