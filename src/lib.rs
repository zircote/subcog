//! # Subcog
//!
//! A persistent memory system for AI coding assistants.
//! Persistent memory system for AI coding assistants.
//!
//! Subcog captures decisions, learnings, and context from coding sessions
//! and surfaces them when relevant through semantic search.
//!
//! ## Features
//!
//! - Single-binary distribution (<100MB, <10ms cold start)
//! - Three-layer storage architecture (Persistence, Index, Vector)
//! - Pluggable backends (SQLite+usearch, PostgreSQL+pgvector)
//! - MCP server integration for AI agent interoperability
//! - Claude Code hooks for seamless IDE integration
//! - Semantic search with hybrid vector + BM25 ranking
//!
//! ## Example
//!
//! ```rust,ignore
//! use subcog::{CaptureService, CaptureRequest, Namespace};
//!
//! let service = CaptureService::new(config)?;
//! let result = service.capture(CaptureRequest {
//!     namespace: Namespace::Decisions,
//!     content: "Use PostgreSQL for primary storage".to_string(),
//!     ..Default::default()
//! })?;
//! ```

#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(missing_docs)]
#![forbid(unsafe_code)]
// multiple_crate_versions is inherently crate-level (detects duplicate transitive dependencies).
// Cannot be moved to function level. Current duplicates: fastembed→ort transitive deps.
#![allow(clippy::multiple_crate_versions)]

use thiserror::Error as ThisError;

// Module declarations
pub mod cli;
pub mod config;
pub mod context;
pub mod embedding;
pub mod gc;
pub mod git;
pub mod hooks;
pub mod io;
pub mod llm;
pub mod mcp;
pub mod models;
pub mod observability;
pub mod rendering;
pub mod security;
pub mod services;
pub mod storage;
pub mod webhooks;

// Re-exports for convenience
pub use config::{FeatureFlags, OperationTimeoutConfig, OperationType, SubcogConfig};
pub use embedding::Embedder;
pub use llm::LlmProvider;
pub use models::{
    CaptureRequest, CaptureResult, DetailLevel, Domain, Memory, MemoryId, MemoryStatus, Namespace,
    SearchFilter, SearchMode, SearchResult,
};
pub use services::{
    CaptureService, ConsolidationService, ContextBuilderService, RecallService, SyncService,
};
pub use storage::{CompositeStorage, IndexBackend, PersistenceBackend, VectorBackend};

/// Error type for subcog operations.
///
/// Uses `thiserror` for automatic `Display` and `Error` trait implementations.
///
/// # Error Variant Triggers
///
/// | Variant | Raised When |
/// |---------|-------------|
/// | `InvalidInput` | Missing required parameters, malformed JSON, invalid namespace names |
/// | `OperationFailed` | I/O errors, git operations fail, database queries fail |
/// | `ContentBlocked` | Secret patterns detected (API keys, tokens), PII detected |
/// | `NotImplemented` | Calling unfinished features (e.g., PostgreSQL consolidation) |
/// | `FeatureNotEnabled` | Using features requiring compile-time flags |
/// | `Unauthorized` | Invalid/missing JWT token in MCP HTTP transport |
#[derive(Debug, ThisError)]
pub enum Error {
    /// Invalid input was provided.
    ///
    /// Raised when:
    /// - Required parameters are missing (e.g., empty content in capture)
    /// - JSON deserialization fails in MCP tool handlers
    /// - Invalid namespace string is provided
    /// - Prompt template has invalid variable syntax
    /// - Search query is empty or malformed
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// An operation failed.
    ///
    /// Raised when:
    /// - `SQLite` database operations fail
    /// - Filesystem I/O errors occur
    /// - Index backend is not configured
    /// - Service container initialization fails
    #[error("operation '{operation}' failed: {cause}")]
    OperationFailed {
        /// The operation that failed.
        operation: String,
        /// The underlying cause.
        cause: String,
    },

    /// Content was blocked due to security concerns.
    ///
    /// Raised when:
    /// - Secret detection finds API keys, tokens, or credentials
    /// - PII patterns are detected (configurable)
    /// - Content fails security policy checks
    ///
    /// See `security::secrets` for pattern definitions.
    #[error("content blocked: {reason}")]
    ContentBlocked {
        /// The reason the content was blocked.
        reason: String,
    },

    /// Feature not yet implemented.
    ///
    /// Raised when:
    /// - PostgreSQL consolidation is called
    /// - Redis consolidation is called
    /// - Other stub implementations are invoked
    #[error("not implemented: {0}")]
    NotImplemented(String),

    /// Feature not enabled (requires feature flag).
    ///
    /// Raised when:
    /// - Optional Cargo features are not compiled in
    /// - Currently unused but reserved for future gated features
    #[error("feature not enabled: {0} (compile with --features {0})")]
    FeatureNotEnabled(String),

    /// Authentication failed (SEC-H1).
    ///
    /// Raised when:
    /// - JWT token is missing in HTTP transport requests
    /// - JWT token is expired or invalid
    /// - JWT signature verification fails
    /// - Insufficient entropy in JWT secret
    #[error("unauthorized: {0}")]
    Unauthorized(String),
}

/// Result type alias for subcog operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Returns the current Unix timestamp in seconds.
///
/// This is a centralized utility function to avoid duplicate implementations
/// across the codebase (CQ-H1). Uses `SystemTime::now()` with fallback to 0
/// if the system clock is before the Unix epoch.
///
/// # Examples
///
/// ```rust
/// use subcog::current_timestamp;
///
/// let ts = current_timestamp();
/// assert!(ts > 0); // Should be a reasonable Unix timestamp
/// ```
#[must_use]
pub fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// Placeholder functions (add, divide) and Config struct removed in code review cleanup.
// Use SubcogConfig for configuration instead.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::InvalidInput("test error".to_string());
        assert_eq!(err.to_string(), "invalid input: test error");

        let err = Error::OperationFailed {
            operation: "test".to_string(),
            cause: "failed".to_string(),
        };
        assert_eq!(err.to_string(), "operation 'test' failed: failed");

        let err = Error::ContentBlocked {
            reason: "secrets detected".to_string(),
        };
        assert_eq!(err.to_string(), "content blocked: secrets detected");
    }
}
