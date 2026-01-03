//! Memory event types for audit and observability.

use std::sync::Arc;

use super::{Domain, MemoryId, Namespace};

/// Events emitted during memory operations.
#[derive(Debug, Clone)]
pub enum MemoryEvent {
    /// A memory was captured.
    Captured {
        /// The ID of the captured memory.
        memory_id: MemoryId,
        /// The namespace.
        namespace: Namespace,
        /// The domain.
        domain: Domain,
        /// Content length in bytes.
        content_length: usize,
        /// Timestamp (Unix epoch seconds).
        timestamp: u64,
    },
    /// A memory was retrieved via search.
    Retrieved {
        /// The ID of the retrieved memory.
        memory_id: MemoryId,
        /// The search query that matched (`Arc<str>` for zero-copy sharing, PERF-C1).
        query: Arc<str>,
        /// The similarity score.
        score: f32,
        /// Timestamp (Unix epoch seconds).
        timestamp: u64,
    },
    /// A memory was updated.
    Updated {
        /// The ID of the updated memory.
        memory_id: MemoryId,
        /// Fields that were modified.
        modified_fields: Vec<String>,
        /// Timestamp (Unix epoch seconds).
        timestamp: u64,
    },
    /// A memory was archived.
    Archived {
        /// The ID of the archived memory.
        memory_id: MemoryId,
        /// Reason for archiving.
        reason: String,
        /// Timestamp (Unix epoch seconds).
        timestamp: u64,
    },
    /// A memory was deleted.
    Deleted {
        /// The ID of the deleted memory.
        memory_id: MemoryId,
        /// Reason for deletion.
        reason: String,
        /// Timestamp (Unix epoch seconds).
        timestamp: u64,
    },
    /// Content was redacted for security.
    Redacted {
        /// The ID of the affected memory.
        memory_id: MemoryId,
        /// Type of content redacted.
        redaction_type: String,
        /// Timestamp (Unix epoch seconds).
        timestamp: u64,
    },
    /// Memories were synchronized with remote.
    Synced {
        /// Number of memories pushed.
        pushed: usize,
        /// Number of memories pulled.
        pulled: usize,
        /// Number of conflicts resolved.
        conflicts: usize,
        /// Timestamp (Unix epoch seconds).
        timestamp: u64,
    },
    /// Consolidation occurred.
    Consolidated {
        /// Number of memories processed.
        processed: usize,
        /// Number of memories archived.
        archived: usize,
        /// Number of memories merged.
        merged: usize,
        /// Timestamp (Unix epoch seconds).
        timestamp: u64,
    },
}

impl MemoryEvent {
    /// Returns the event type name.
    #[must_use]
    pub const fn event_type(&self) -> &'static str {
        match self {
            Self::Captured { .. } => "captured",
            Self::Retrieved { .. } => "retrieved",
            Self::Updated { .. } => "updated",
            Self::Archived { .. } => "archived",
            Self::Deleted { .. } => "deleted",
            Self::Redacted { .. } => "redacted",
            Self::Synced { .. } => "synced",
            Self::Consolidated { .. } => "consolidated",
        }
    }

    /// Returns the timestamp of the event.
    #[must_use]
    pub const fn timestamp(&self) -> u64 {
        match self {
            Self::Captured { timestamp, .. }
            | Self::Retrieved { timestamp, .. }
            | Self::Updated { timestamp, .. }
            | Self::Archived { timestamp, .. }
            | Self::Deleted { timestamp, .. }
            | Self::Redacted { timestamp, .. }
            | Self::Synced { timestamp, .. }
            | Self::Consolidated { timestamp, .. } => *timestamp,
        }
    }
}
