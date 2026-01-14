//! Memory export service.
//!
//! Orchestrates bulk memory export to various formats.

use crate::io::formats::{Format, create_export_sink};
use crate::io::traits::{ExportField, ExportSink, ExportableMemory};
use crate::models::{Memory, SearchFilter};
use crate::services::parse_filter_query;
use crate::storage::IndexBackend;
use crate::storage::index::SqliteBackend;
use crate::{Error, Result};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

/// Options for memory export.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// File format to export to.
    pub format: Format,
    /// Filter query string (GitHub-style syntax).
    pub filter: Option<String>,
    /// Maximum number of memories to export.
    pub limit: Option<usize>,
    /// Fields to include in export.
    pub fields: Option<Vec<ExportField>>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: Format::Json,
            filter: None,
            limit: None,
            fields: None, // All fields
        }
    }
}

impl ExportOptions {
    /// Creates export options with the given format.
    #[must_use]
    pub const fn with_format(mut self, format: Format) -> Self {
        self.format = format;
        self
    }

    /// Sets the filter query string.
    #[must_use]
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// Sets the maximum number of memories to export.
    #[must_use]
    pub const fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the fields to include in export.
    #[must_use]
    pub fn with_fields(mut self, fields: Vec<ExportField>) -> Self {
        self.fields = Some(fields);
        self
    }

    /// Parses the filter query into a SearchFilter.
    #[must_use]
    pub fn parse_filter(&self) -> SearchFilter {
        self.filter
            .as_ref()
            .map(|f| parse_filter_query(f))
            .unwrap_or_default()
    }
}

/// Result of an export operation.
#[derive(Debug, Clone)]
pub struct ExportResult {
    /// Number of memories exported.
    pub exported: usize,
    /// Total memories that matched the filter.
    pub total_matched: usize,
    /// Format used for export.
    pub format: Format,
    /// Output path (if file export).
    pub output_path: Option<String>,
}

impl ExportResult {
    /// Creates a new export result.
    #[must_use]
    pub const fn new(format: Format) -> Self {
        Self {
            exported: 0,
            total_matched: 0,
            format,
            output_path: None,
        }
    }

    /// Returns whether any memories were exported.
    #[must_use]
    pub const fn has_exports(&self) -> bool {
        self.exported > 0
    }
}

/// Progress callback for export operations.
pub type ExportProgressCallback = Box<dyn Fn(usize, Option<usize>) + Send>;

/// Service for exporting memories to external formats.
pub struct ExportService {
    /// Index backend for querying memories.
    index: Arc<SqliteBackend>,
}

impl ExportService {
    /// Creates a new export service.
    #[must_use]
    pub fn new(index: Arc<SqliteBackend>) -> Self {
        Self { index }
    }

    /// Exports memories to a file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written or export fails.
    pub fn export_to_file(
        &self,
        path: &Path,
        options: ExportOptions,
        progress: Option<ExportProgressCallback>,
    ) -> Result<ExportResult> {
        let format = if options.format == Format::Json {
            // Auto-detect from extension if using default
            Format::from_path(path).unwrap_or(Format::Json)
        } else {
            options.format
        };

        if !format.supports_export() {
            return Err(Error::InvalidInput(format!(
                "Format {} does not support export",
                format
            )));
        }

        let file = std::fs::File::create(path).map_err(|e| Error::OperationFailed {
            operation: "create_export_file".to_string(),
            cause: e.to_string(),
        })?;
        let writer = std::io::BufWriter::new(file);

        let mut result = self.export_to_writer(writer, options.with_format(format), progress)?;
        result.output_path = Some(path.display().to_string());
        Ok(result)
    }

    /// Exports memories to a writer.
    ///
    /// # Errors
    ///
    /// Returns an error if writing fails.
    pub fn export_to_writer<W: Write + Send + 'static>(
        &self,
        writer: W,
        options: ExportOptions,
        progress: Option<ExportProgressCallback>,
    ) -> Result<ExportResult> {
        let mut sink = create_export_sink(writer, options.format)?;
        let result = self.export_to_sink(sink.as_mut(), &options, progress)?;
        sink.finalize()?;
        Ok(result)
    }

    /// Exports memories to a sink.
    ///
    /// # Errors
    ///
    /// Returns an error if export fails.
    pub fn export_to_sink(
        &self,
        sink: &mut dyn ExportSink,
        options: &ExportOptions,
        progress: Option<ExportProgressCallback>,
    ) -> Result<ExportResult> {
        let filter = options.parse_filter();
        let limit = options.limit.unwrap_or(usize::MAX);

        // Query memories from index
        let memory_ids = self.index.list_all(&filter, limit)?;
        let total_matched = memory_ids.len();

        let mut result = ExportResult::new(options.format);
        result.total_matched = total_matched;

        // Batch fetch memories
        let ids: Vec<_> = memory_ids.iter().map(|(id, _)| id.clone()).collect();
        let memories = self.index.get_memories_batch(&ids)?;

        for memory in memories.into_iter().flatten() {
            let exportable = ExportableMemory::from(&memory);
            sink.write(&exportable)?;
            result.exported += 1;

            if let Some(ref cb) = progress {
                cb(result.exported, Some(total_matched));
            }
        }

        Ok(result)
    }

    /// Exports memories directly from an iterator.
    ///
    /// Useful when memories are already loaded.
    ///
    /// # Errors
    ///
    /// Returns an error if export fails.
    #[allow(clippy::excessive_nesting)]
    pub fn export_memories<'a, I>(
        &self,
        memories: I,
        sink: &mut dyn ExportSink,
        progress: Option<ExportProgressCallback>,
    ) -> Result<ExportResult>
    where
        I: IntoIterator<Item = &'a Memory>,
    {
        let mut result = ExportResult::new(Format::Json);

        for memory in memories {
            let exportable = ExportableMemory::from(memory);
            sink.write(&exportable)?;
            result.exported += 1;

            if let Some(ref cb) = progress {
                cb(result.exported, None);
            }
        }

        result.total_matched = result.exported;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::formats::json::JsonExportSink;
    use crate::models::{Domain, MemoryId, MemoryStatus, Namespace};

    fn test_memory(id: &str, content: &str) -> Memory {
        Memory {
            id: MemoryId::new(id),
            content: content.to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: 0,
            updated_at: 0,
            tombstoned_at: None,
            expires_at: None,
            embedding: None,
            tags: vec![],
            source: None,
            is_summary: false,
            source_memory_ids: None,
            consolidation_timestamp: None,
        }
    }

    #[test]
    fn test_export_options_defaults() {
        let options = ExportOptions::default();
        assert_eq!(options.format, Format::Json);
        assert!(options.filter.is_none());
        assert!(options.limit.is_none());
    }

    #[test]
    fn test_export_options_with_filter() {
        let options = ExportOptions::default().with_filter("ns:decisions tag:rust");
        assert!(options.filter.is_some());

        let filter = options.parse_filter();
        assert_eq!(filter.namespaces.len(), 1);
        assert_eq!(filter.tags.len(), 1);
    }

    #[test]
    fn test_export_result_has_exports() {
        let mut result = ExportResult::new(Format::Json);
        assert!(!result.has_exports());

        result.exported = 1;
        assert!(result.has_exports());
    }

    #[test]
    fn test_export_memories_to_json() {
        let index = Arc::new(SqliteBackend::in_memory().unwrap());
        let service = ExportService::new(index);

        let memories = [
            test_memory("1", "First memory"),
            test_memory("2", "Second memory"),
        ];

        let mut output = Vec::new();
        {
            let mut sink = JsonExportSink::new(&mut output);
            service
                .export_memories(memories.iter(), &mut sink, None)
                .unwrap();
            Box::new(sink).finalize().unwrap();
        }

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("First memory"));
        assert!(output_str.contains("Second memory"));
    }
}
