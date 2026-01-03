//! Garbage collection module.
//!
//! This module provides garbage collection utilities for cleaning up stale
//! memories, particularly those associated with deleted git branches.
//!
//! # Overview
//!
//! The garbage collector identifies memories associated with branches that
//! no longer exist in the git repository and marks them as tombstoned.
//! This helps keep the memory index clean and relevant.
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
//! # Lazy GC
//!
//! The garbage collector can be integrated into the recall path for lazy,
//! opportunistic cleanup. When memories are retrieved, the system can check
//! if any are from stale branches and mark them accordingly.

mod branch;

pub use branch::{BranchGarbageCollector, GcResult, branch_exists};
