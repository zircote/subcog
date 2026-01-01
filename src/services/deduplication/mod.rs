//! Deduplication service for pre-compact hook.
//!
//! This module provides three-tier deduplication checking:
//! 1. **Exact match**: SHA256 hash comparison via tag search
//! 2. **Semantic similarity**: `FastEmbed` embeddings with cosine similarity threshold
//! 3. **Recent capture**: In-memory LRU cache with TTL-based expiration
//!
//! The service implements short-circuit evaluation, exiting early on first match.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    DeduplicationService                         │
//! │  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐ │
//! │  │ ExactMatch   │  │ Semantic     │  │ RecentCapture          │ │
//! │  │ Checker      │  │ Checker      │  │ Checker                │ │
//! │  │              │  │              │  │                        │ │
//! │  │ SHA256 hash  │  │ Embedding    │  │ LRU Cache with TTL     │ │
//! │  │ comparison   │  │ similarity   │  │ (5 min window)         │ │
//! │  └──────────────┘  └──────────────┘  └────────────────────────┘ │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use subcog::services::deduplication::{DeduplicationService, DeduplicationConfig};
//!
//! let config = DeduplicationConfig::default();
//! let service = DeduplicationService::new(recall, embedder, config);
//!
//! let result = service.check_duplicate("Use PostgreSQL for primary storage", Namespace::Decisions)?;
//! if result.is_duplicate {
//!     println!("Skipping duplicate: {:?}", result.reason);
//! }
//! ```

mod config;
mod exact_match;
mod hasher;
mod recent;
mod semantic;
mod service;
mod types;

pub use config::DeduplicationConfig;
pub use exact_match::ExactMatchChecker;
pub use hasher::ContentHasher;
pub use recent::RecentCaptureChecker;
pub use semantic::{SemanticSimilarityChecker, cosine_similarity};
pub use service::DeduplicationService;
pub use types::{Deduplicator, DuplicateCheckResult, DuplicateReason};
