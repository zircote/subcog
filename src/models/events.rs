//! Memory event types for audit and observability.

use std::sync::Arc;

use super::{Domain, MemoryId, Namespace};
use crate::current_timestamp;
use uuid::Uuid;

/// Shared event metadata required for observability.
#[derive(Debug, Clone)]
pub struct EventMeta {
    /// Unique identifier for this event.
    pub event_id: String,
    /// Optional correlation identifier for request/trace linking.
    pub correlation_id: Option<String>,
    /// Event source component.
    pub source: &'static str,
    /// Timestamp (Unix epoch seconds).
    pub timestamp: u64,
}

impl EventMeta {
    /// Creates new event metadata using the current timestamp.
    #[must_use]
    pub fn new(source: &'static str, correlation_id: Option<String>) -> Self {
        Self::with_timestamp(source, correlation_id, current_timestamp())
    }

    /// Creates new event metadata with a specified timestamp.
    #[must_use]
    pub fn with_timestamp(
        source: &'static str,
        correlation_id: Option<String>,
        timestamp: u64,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            correlation_id,
            source,
            timestamp,
        }
    }
}

/// Events emitted during memory operations.
#[derive(Debug, Clone)]
pub enum MemoryEvent {
    /// A memory was captured.
    Captured {
        /// Event metadata.
        meta: EventMeta,
        /// The ID of the captured memory.
        memory_id: MemoryId,
        /// The namespace.
        namespace: Namespace,
        /// The domain.
        domain: Domain,
        /// Content length in bytes.
        content_length: usize,
    },
    /// A memory was retrieved via search.
    Retrieved {
        /// Event metadata.
        meta: EventMeta,
        /// The ID of the retrieved memory.
        memory_id: MemoryId,
        /// The search query that matched (`Arc<str>` for zero-copy sharing, PERF-C1).
        query: Arc<str>,
        /// The similarity score.
        score: f32,
    },
    /// A memory was updated.
    Updated {
        /// Event metadata.
        meta: EventMeta,
        /// The ID of the updated memory.
        memory_id: MemoryId,
        /// Fields that were modified.
        modified_fields: Vec<String>,
    },
    /// A memory was archived.
    Archived {
        /// Event metadata.
        meta: EventMeta,
        /// The ID of the archived memory.
        memory_id: MemoryId,
        /// Reason for archiving.
        reason: String,
    },
    /// A memory was deleted.
    Deleted {
        /// Event metadata.
        meta: EventMeta,
        /// The ID of the deleted memory.
        memory_id: MemoryId,
        /// Reason for deletion.
        reason: String,
    },
    /// Content was redacted for security.
    Redacted {
        /// Event metadata.
        meta: EventMeta,
        /// The ID of the affected memory.
        memory_id: MemoryId,
        /// Type of content redacted.
        redaction_type: String,
    },
    /// Memories were synchronized with remote.
    Synced {
        /// Event metadata.
        meta: EventMeta,
        /// Number of memories pushed.
        pushed: usize,
        /// Number of memories pulled.
        pulled: usize,
        /// Number of conflicts resolved.
        conflicts: usize,
    },
    /// Consolidation occurred.
    Consolidated {
        /// Event metadata.
        meta: EventMeta,
        /// Number of memories processed.
        processed: usize,
        /// Number of memories archived.
        archived: usize,
        /// Number of memories merged.
        merged: usize,
    },
    /// MCP server started.
    McpStarted {
        /// Event metadata.
        meta: EventMeta,
        /// Transport type (stdio/http).
        transport: String,
        /// HTTP port if applicable.
        port: Option<u16>,
    },
    /// MCP authentication failed.
    McpAuthFailed {
        /// Event metadata.
        meta: EventMeta,
        /// Optional client identifier.
        client_id: Option<String>,
        /// Failure reason.
        reason: String,
    },
    /// MCP tool execution completed.
    McpToolExecuted {
        /// Event metadata.
        meta: EventMeta,
        /// Tool name.
        tool_name: String,
        /// Execution status (success/error).
        status: String,
        /// Execution duration in milliseconds.
        duration_ms: u64,
        /// Optional error string.
        error: Option<String>,
    },
    /// MCP request error (invalid params, tool failure, etc.).
    McpRequestError {
        /// Event metadata.
        meta: EventMeta,
        /// Operation name.
        operation: String,
        /// Error message.
        error: String,
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
            Self::McpStarted { .. } => "mcp.started",
            Self::McpAuthFailed { .. } => "mcp.auth_failed",
            Self::McpToolExecuted { .. } => "mcp.tool_executed",
            Self::McpRequestError { .. } => "mcp.request_error",
        }
    }

    /// Returns the timestamp of the event.
    #[must_use]
    pub const fn timestamp(&self) -> u64 {
        match self {
            Self::Captured { meta, .. }
            | Self::Retrieved { meta, .. }
            | Self::Updated { meta, .. }
            | Self::Archived { meta, .. }
            | Self::Deleted { meta, .. }
            | Self::Redacted { meta, .. }
            | Self::Synced { meta, .. }
            | Self::Consolidated { meta, .. }
            | Self::McpStarted { meta, .. }
            | Self::McpAuthFailed { meta, .. }
            | Self::McpToolExecuted { meta, .. }
            | Self::McpRequestError { meta, .. } => meta.timestamp,
        }
    }

    /// Returns the event metadata.
    #[must_use]
    pub const fn meta(&self) -> &EventMeta {
        match self {
            Self::Captured { meta, .. }
            | Self::Retrieved { meta, .. }
            | Self::Updated { meta, .. }
            | Self::Archived { meta, .. }
            | Self::Deleted { meta, .. }
            | Self::Redacted { meta, .. }
            | Self::Synced { meta, .. }
            | Self::Consolidated { meta, .. }
            | Self::McpStarted { meta, .. }
            | Self::McpAuthFailed { meta, .. }
            | Self::McpToolExecuted { meta, .. }
            | Self::McpRequestError { meta, .. } => meta,
        }
    }
}
