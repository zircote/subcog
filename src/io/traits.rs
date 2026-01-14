//! Core traits for import/export operations.
//!
//! Defines the [`ImportSource`] and [`ExportSink`] traits that format adapters
//! implement to support different file formats.

use crate::Result;
use crate::models::Memory;
use serde::{Deserialize, Serialize};

/// Intermediate representation for imported memory data.
///
/// This struct captures the fields that can be imported from external formats.
/// Optional fields allow partial data to be imported with defaults applied
/// during validation.
///
/// # Field Mapping
///
/// | Field | Required | Default |
/// |-------|----------|---------|
/// | `content` | Yes | - |
/// | `namespace` | No | `decisions` |
/// | `domain` | No | Context-dependent |
/// | `tags` | No | `[]` |
/// | `source` | No | `None` |
/// | `created_at` | No | Current time |
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedMemory {
    /// The memory content (required).
    pub content: String,

    /// Namespace for categorization.
    #[serde(default)]
    pub namespace: Option<String>,

    /// Domain scope (project, user, org).
    #[serde(default)]
    pub domain: Option<String>,

    /// Tags for categorization.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Source reference (file path, URL).
    #[serde(default)]
    pub source: Option<String>,

    /// Original creation timestamp (Unix epoch seconds).
    ///
    /// If provided, preserves the original creation time during import.
    #[serde(default)]
    pub created_at: Option<u64>,

    /// TTL in seconds for automatic expiration.
    #[serde(default)]
    pub ttl_seconds: Option<u64>,
}

impl ImportedMemory {
    /// Creates a new imported memory with just content.
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            namespace: None,
            domain: None,
            tags: Vec::new(),
            source: None,
            created_at: None,
            ttl_seconds: None,
        }
    }

    /// Sets the namespace.
    #[must_use]
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// Sets the domain.
    #[must_use]
    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
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
}

/// Source of imported memories.
///
/// Implementations read memories from a specific format (JSON, YAML, CSV, etc.)
/// and yield them one at a time for processing.
///
/// # Streaming
///
/// Sources should read data incrementally where possible to support large files
/// without loading everything into memory.
///
/// # Example Implementation
///
/// ```rust,ignore
/// impl ImportSource for JsonSource {
///     fn next(&mut self) -> Result<Option<ImportedMemory>> {
///         // Read next line, parse JSON, return memory
///     }
///
///     fn size_hint(&self) -> Option<usize> {
///         None // Unknown for streaming
///     }
/// }
/// ```
pub trait ImportSource {
    /// Reads the next memory from the source.
    ///
    /// Returns `Ok(None)` when the source is exhausted.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing fails or I/O errors occur.
    fn next(&mut self) -> Result<Option<ImportedMemory>>;

    /// Returns an estimate of the total number of records.
    ///
    /// Used for progress reporting. Returns `None` if unknown.
    fn size_hint(&self) -> Option<usize> {
        None
    }
}

/// Memory representation for export.
///
/// A subset of [`Memory`] fields that are meaningful for external consumption.
/// Excludes internal fields like embeddings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportableMemory {
    /// Unique memory identifier.
    pub id: String,
    /// Memory content.
    pub content: String,
    /// Namespace (e.g., "decisions", "learnings").
    pub namespace: String,
    /// Domain (e.g., "project", "user").
    pub domain: String,
    /// Project identifier (git remote URL).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Branch name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// File path relative to repo root.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// Status (e.g., "active", "archived").
    pub status: String,
    /// Creation timestamp (Unix epoch seconds).
    pub created_at: u64,
    /// Last update timestamp (Unix epoch seconds).
    pub updated_at: u64,
    /// Tags for categorization.
    pub tags: Vec<String>,
    /// Source reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl From<Memory> for ExportableMemory {
    fn from(m: Memory) -> Self {
        Self {
            id: m.id.to_string(),
            content: m.content,
            namespace: m.namespace.as_str().to_string(),
            domain: m.domain.to_string(),
            project_id: m.project_id,
            branch: m.branch,
            file_path: m.file_path,
            status: m.status.as_str().to_string(),
            created_at: m.created_at,
            updated_at: m.updated_at,
            tags: m.tags,
            source: m.source,
        }
    }
}

impl From<&Memory> for ExportableMemory {
    fn from(m: &Memory) -> Self {
        Self {
            id: m.id.to_string(),
            content: m.content.clone(),
            namespace: m.namespace.as_str().to_string(),
            domain: m.domain.to_string(),
            project_id: m.project_id.clone(),
            branch: m.branch.clone(),
            file_path: m.file_path.clone(),
            status: m.status.as_str().to_string(),
            created_at: m.created_at,
            updated_at: m.updated_at,
            tags: m.tags.clone(),
            source: m.source.clone(),
        }
    }
}

/// Sink for exported memories.
///
/// Implementations write memories to a specific format (JSON, YAML, CSV, etc.).
///
/// # Lifecycle
///
/// 1. Create sink with output destination
/// 2. Call `write()` for each memory
/// 3. Call `finalize()` to complete the export
///
/// # Example Implementation
///
/// ```rust,ignore
/// impl ExportSink for JsonSink {
///     fn write(&mut self, memory: &ExportableMemory) -> Result<()> {
///         serde_json::to_writer(&mut self.writer, memory)?;
///         writeln!(self.writer)?;
///         Ok(())
///     }
///
///     fn finalize(self: Box<Self>) -> Result<()> {
///         self.writer.flush()?;
///         Ok(())
///     }
/// }
/// ```
pub trait ExportSink {
    /// Writes a single memory to the sink.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or I/O fails.
    fn write(&mut self, memory: &ExportableMemory) -> Result<()>;

    /// Finalizes the export, writing any footers and flushing buffers.
    ///
    /// This method consumes the sink.
    ///
    /// # Errors
    ///
    /// Returns an error if I/O fails.
    fn finalize(self: Box<Self>) -> Result<()>;
}

/// Fields that can be selected for export.
///
/// Used with [`crate::io::services::export::ExportOptions::fields`] to customize output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportField {
    /// Memory ID.
    Id,
    /// Memory content.
    Content,
    /// Namespace.
    Namespace,
    /// Domain.
    Domain,
    /// Project ID.
    ProjectId,
    /// Branch.
    Branch,
    /// File path.
    FilePath,
    /// Status.
    Status,
    /// Creation timestamp.
    CreatedAt,
    /// Update timestamp.
    UpdatedAt,
    /// Tags.
    Tags,
    /// Source reference.
    Source,
}

impl ExportField {
    /// Returns all available fields.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Id,
            Self::Content,
            Self::Namespace,
            Self::Domain,
            Self::ProjectId,
            Self::Branch,
            Self::FilePath,
            Self::Status,
            Self::CreatedAt,
            Self::UpdatedAt,
            Self::Tags,
            Self::Source,
        ]
    }

    /// Returns the string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Content => "content",
            Self::Namespace => "namespace",
            Self::Domain => "domain",
            Self::ProjectId => "project_id",
            Self::Branch => "branch",
            Self::FilePath => "file_path",
            Self::Status => "status",
            Self::CreatedAt => "created_at",
            Self::UpdatedAt => "updated_at",
            Self::Tags => "tags",
            Self::Source => "source",
        }
    }

    /// Parses a field name string.
    ///
    /// Returns `None` if the field name is not recognized.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "id" => Some(Self::Id),
            "content" => Some(Self::Content),
            "namespace" | "ns" => Some(Self::Namespace),
            "domain" => Some(Self::Domain),
            "project_id" | "project" => Some(Self::ProjectId),
            "branch" => Some(Self::Branch),
            "file_path" | "file" | "path" => Some(Self::FilePath),
            "status" => Some(Self::Status),
            "created_at" | "created" => Some(Self::CreatedAt),
            "updated_at" | "updated" => Some(Self::UpdatedAt),
            "tags" => Some(Self::Tags),
            "source" => Some(Self::Source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_imported_memory_builder() {
        let mem = ImportedMemory::new("Test content")
            .with_namespace("decisions")
            .with_domain("project")
            .with_tag("rust")
            .with_tag("test")
            .with_source("test.rs");

        assert_eq!(mem.content, "Test content");
        assert_eq!(mem.namespace, Some("decisions".to_string()));
        assert_eq!(mem.domain, Some("project".to_string()));
        assert_eq!(mem.tags, vec!["rust", "test"]);
        assert_eq!(mem.source, Some("test.rs".to_string()));
    }

    #[test]
    fn test_export_field_parsing() {
        assert_eq!(ExportField::parse("id"), Some(ExportField::Id));
        assert_eq!(ExportField::parse("content"), Some(ExportField::Content));
        assert_eq!(ExportField::parse("ns"), Some(ExportField::Namespace));
        assert_eq!(
            ExportField::parse("namespace"),
            Some(ExportField::Namespace)
        );
        assert_eq!(ExportField::parse("unknown"), None);
    }

    #[test]
    fn test_export_field_all() {
        let all = ExportField::all();
        assert_eq!(all.len(), 12);
        assert!(all.contains(&ExportField::Id));
        assert!(all.contains(&ExportField::Content));
    }
}
