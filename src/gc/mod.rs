//! Garbage collection module.
//!
//! This module provides garbage collection utilities for cleaning up stale
//! memories, particularly those associated with deleted git branches,
//! memories that have exceeded their retention period, or memories that
//! have exceeded their explicit TTL (`expires_at` timestamp).
//!
//! # Overview
//!
//! The garbage collector identifies memories associated with branches that
//! no longer exist in the git repository and marks them as tombstoned.
//! It also enforces retention policies to clean up old memories.
//!
//! # Example
//!
//! ```rust,ignore
//! use subcog::gc::BranchGarbageCollector;
//! use subcog::storage::index::SqliteBackend;
//! use std::sync::Arc;
//!
//! let backend = Arc::new(SqliteBackend::new("memories.db")?);
//! let gc = BranchGarbageCollector::new(backend);
//!
//! // Dry run to see what would be cleaned up
//! let result = gc.gc_stale_branches("github.com/org/repo", true)?;
//! println!("Would tombstone {} memories from {} stale branches",
//!          result.memories_tombstoned, result.stale_branches.len());
//!
//! // Actually perform the cleanup
//! let result = gc.gc_stale_branches("github.com/org/repo", false)?;
//! println!("Tombstoned {} memories", result.memories_tombstoned);
//! ```
//!
//! # Retention Policy
//!
//! Memories can be automatically cleaned up based on age using the retention
//! garbage collector:
//!
//! ```rust,ignore
//! use subcog::gc::{RetentionConfig, RetentionGarbageCollector};
//!
//! // Load retention config from environment (default: 365 days)
//! let config = RetentionConfig::from_env();
//!
//! // Create retention GC with index backend
//! let gc = RetentionGarbageCollector::new(backend, config);
//!
//! // Clean up expired memories
//! let result = gc.gc_expired_memories(false)?;
//! println!("Tombstoned {} expired memories", result.memories_tombstoned);
//! ```
//!
//! # Lazy GC
//!
//! The garbage collector can be integrated into the recall path for lazy,
//! opportunistic cleanup. When memories are retrieved, the system can check
//! if any are from stale branches and mark them accordingly.

mod branch;
mod expiration;
mod retention;

pub use branch::{BranchGarbageCollector, GcResult, branch_exists};
pub use expiration::{
    DEFAULT_CLEANUP_PROBABILITY, EXPIRATION_CLEANUP_PROBABILITY_ENV, ExpirationConfig,
    ExpirationGcResult, ExpirationService,
};
pub use retention::{
    DEFAULT_RETENTION_DAYS, RETENTION_DAYS_ENV, RetentionConfig, RetentionGarbageCollector,
    RetentionGcResult, retention_days,
};
