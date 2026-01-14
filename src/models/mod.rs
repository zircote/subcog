//! Data models for subcog.
//!
//! This module contains all the core data structures used throughout the system.

mod capture;
mod consolidation;
mod context_template;
mod domain;
mod events;
pub mod graph;
pub mod group;
mod memory;
mod prompt;
mod search;
pub mod temporal;

pub use capture::{CaptureRequest, CaptureResult};
pub use consolidation::{EdgeType, MemoryTier, RetentionScore};
pub use context_template::{
    AUTO_VARIABLE_PREFIXES, AUTO_VARIABLES, ContextTemplate, OutputFormat, TemplateVariable,
    TemplateVersion, VariableType, is_auto_variable,
};
pub use domain::{Domain, MemoryStatus, Namespace};
pub use events::{EventMeta, MemoryEvent};
pub use memory::{Memory, MemoryId, MemoryResult};
pub use prompt::{
    ExtractedVariable, IssueSeverity, MAX_VARIABLE_VALUE_LENGTH, PromptTemplate, PromptVariable,
    ValidationIssue, ValidationResult, extract_variables, is_reserved_variable_name,
    sanitize_variable_value, substitute_variables, validate_prompt_content,
};
pub use search::{DetailLevel, SearchFilter, SearchHit, SearchMode, SearchResult};

// Group types (feature-gated)
#[cfg(feature = "group-scope")]
pub use group::{
    AddMemberRequest, CreateGroupRequest, CreateInviteRequest, Group, GroupId, GroupInvite,
    GroupMember, GroupMembership, GroupRole, is_valid_email, normalize_email,
};
