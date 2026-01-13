//! Capture request and result types.

use super::{Domain, MemoryId, Namespace};
use crate::storage::index::DomainScope;

/// Request to capture a new memory.
#[derive(Debug, Clone, Default)]
pub struct CaptureRequest {
    /// The content to capture.
    pub content: String,
    /// Target namespace for the memory.
    pub namespace: Namespace,
    /// Target domain for the memory.
    pub domain: Domain,
    /// Optional tags for categorization.
    pub tags: Vec<String>,
    /// Optional source reference.
    pub source: Option<String>,
    /// Whether to skip security filtering.
    pub skip_security_check: bool,
    /// Optional time-to-live in seconds.
    ///
    /// When set, `expires_at` is calculated as `created_at + ttl_seconds`.
    /// `None` means no expiration (memory lives until manually deleted).
    pub ttl_seconds: Option<u64>,
    /// Target storage scope for the memory.
    ///
    /// - `Project`/`User`: Stored in user-local index (default)
    /// - `Org`: Stored in organization-shared index (requires org feature enabled)
    ///
    /// Default: `None` (uses context-appropriate scope based on git status)
    pub scope: Option<DomainScope>,
}

impl CaptureRequest {
    /// Creates a new capture request with the given content.
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            ..Default::default()
        }
    }

    /// Sets the namespace.
    #[must_use]
    pub const fn with_namespace(mut self, namespace: Namespace) -> Self {
        self.namespace = namespace;
        self
    }

    /// Sets the domain.
    #[must_use]
    pub fn with_domain(mut self, domain: Domain) -> Self {
        self.domain = domain;
        self
    }

    /// Adds a tag.
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Sets the source reference.
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Sets the time-to-live in seconds.
    ///
    /// The memory will expire after this duration from creation time.
    /// `None` means no expiration.
    #[must_use]
    pub const fn with_ttl(mut self, ttl_seconds: u64) -> Self {
        self.ttl_seconds = Some(ttl_seconds);
        self
    }

    /// Sets the storage scope for the memory.
    ///
    /// - `Project`/`User`: Stored in user-local index
    /// - `Org`: Stored in organization-shared index
    #[must_use]
    pub const fn with_scope(mut self, scope: DomainScope) -> Self {
        self.scope = Some(scope);
        self
    }
}

/// Result of a capture operation.
#[derive(Debug, Clone)]
pub struct CaptureResult {
    /// The ID of the captured memory.
    pub memory_id: MemoryId,
    /// The URN of the captured memory.
    pub urn: String,
    /// Whether the content was modified (e.g., redacted).
    pub content_modified: bool,
    /// Any warnings generated during capture.
    pub warnings: Vec<String>,
}

impl CaptureResult {
    /// Creates a new capture result.
    #[must_use]
    pub const fn new(memory_id: MemoryId, urn: String) -> Self {
        Self {
            memory_id,
            urn,
            content_modified: false,
            warnings: Vec::new(),
        }
    }
}
