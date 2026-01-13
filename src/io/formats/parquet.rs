//! Apache Parquet format adapter for export.
//!
//! Provides columnar storage format for efficient analytics queries.
//! Requires the `parquet-export` feature.
//!
//! Note: Parquet import is not supported as it's primarily an analytics format.

use crate::io::traits::{ExportSink, ExportableMemory};
use crate::{Error, Result};
use arrow::array::{ArrayRef, StringArray, UInt64Array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
use std::io::Write;
use std::sync::Arc;

/// Parquet export sink.
///
/// Buffers memories and writes them as a Parquet file on finalize.
/// Uses columnar storage with Snappy compression.
pub struct ParquetExportSink<W: Write + Send> {
    writer: Option<W>,
    /// Buffered memories for batch writing.
    memories: Vec<ExportableMemory>,
}

impl<W: Write + Send> ParquetExportSink<W> {
    /// Creates a new Parquet export sink.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
    pub fn new(writer: W) -> Result<Self> {
        Ok(Self {
            writer: Some(writer),
            memories: Vec::new(),
        })
    }

    /// Creates the Arrow schema for memories.
    fn schema() -> Schema {
        Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("content", DataType::Utf8, false),
            Field::new("namespace", DataType::Utf8, false),
            Field::new("domain", DataType::Utf8, false),
            Field::new("project_id", DataType::Utf8, true),
            Field::new("branch", DataType::Utf8, true),
            Field::new("file_path", DataType::Utf8, true),
            Field::new("status", DataType::Utf8, false),
            Field::new("created_at", DataType::UInt64, false),
            Field::new("updated_at", DataType::UInt64, false),
            Field::new("tags", DataType::Utf8, false), // Stored as comma-separated
            Field::new("source", DataType::Utf8, true),
        ])
    }

    /// Converts buffered memories to a record batch.
    fn to_record_batch(&self) -> Result<RecordBatch> {
        let schema = Arc::new(Self::schema());

        let ids: StringArray = self.memories.iter().map(|m| Some(m.id.as_str())).collect();
        let contents: StringArray = self
            .memories
            .iter()
            .map(|m| Some(m.content.as_str()))
            .collect();
        let namespaces: StringArray = self
            .memories
            .iter()
            .map(|m| Some(m.namespace.as_str()))
            .collect();
        let domains: StringArray = self
            .memories
            .iter()
            .map(|m| Some(m.domain.as_str()))
            .collect();
        let project_ids: StringArray = self
            .memories
            .iter()
            .map(|m| m.project_id.as_deref())
            .collect();
        let branches: StringArray = self.memories.iter().map(|m| m.branch.as_deref()).collect();
        let file_paths: StringArray = self
            .memories
            .iter()
            .map(|m| m.file_path.as_deref())
            .collect();
        let statuses: StringArray = self
            .memories
            .iter()
            .map(|m| Some(m.status.as_str()))
            .collect();
        let created_ats: UInt64Array = self.memories.iter().map(|m| Some(m.created_at)).collect();
        let updated_ats: UInt64Array = self.memories.iter().map(|m| Some(m.updated_at)).collect();
        let tags: StringArray = self
            .memories
            .iter()
            .map(|m| Some(m.tags.join(",")))
            .collect();
        let sources: StringArray = self.memories.iter().map(|m| m.source.as_deref()).collect();

        let columns: Vec<ArrayRef> = vec![
            Arc::new(ids),
            Arc::new(contents),
            Arc::new(namespaces),
            Arc::new(domains),
            Arc::new(project_ids),
            Arc::new(branches),
            Arc::new(file_paths),
            Arc::new(statuses),
            Arc::new(created_ats),
            Arc::new(updated_ats),
            Arc::new(tags),
            Arc::new(sources),
        ];

        RecordBatch::try_new(schema, columns)
            .map_err(|e| Error::Internal(format!("Failed to create record batch: {e}")))
    }
}

impl<W: Write + Send + 'static> ExportSink for ParquetExportSink<W> {
    fn write(&mut self, memory: &ExportableMemory) -> Result<()> {
        self.memories.push(memory.clone());
        Ok(())
    }

    fn finalize(mut self: Box<Self>) -> Result<()> {
        if self.memories.is_empty() {
            return Ok(());
        }

        let writer = self
            .writer
            .take()
            .ok_or_else(|| Error::Internal("Writer already consumed".to_string()))?;

        let schema = Arc::new(Self::schema());
        let props = WriterProperties::builder()
            .set_compression(Compression::SNAPPY)
            .build();

        let mut arrow_writer = ArrowWriter::try_new(writer, schema, Some(props))
            .map_err(|e| Error::Internal(format!("Failed to create Parquet writer: {e}")))?;

        let batch = self.to_record_batch()?;
        arrow_writer
            .write(&batch)
            .map_err(|e| Error::Internal(format!("Failed to write Parquet batch: {e}")))?;

        arrow_writer
            .close()
            .map_err(|e| Error::Internal(format!("Failed to close Parquet writer: {e}")))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parquet_export() {
        let mut output = Cursor::new(Vec::new());
        {
            let mut sink = ParquetExportSink::new(&mut output).unwrap();
            sink.write(&ExportableMemory {
                id: "test-1".to_string(),
                content: "Test memory content".to_string(),
                namespace: "decisions".to_string(),
                domain: "project".to_string(),
                project_id: Some("test-repo".to_string()),
                branch: Some("main".to_string()),
                file_path: None,
                status: "active".to_string(),
                created_at: 1234567890,
                updated_at: 1234567890,
                tags: vec!["rust".to_string(), "test".to_string()],
                source: Some("test.rs".to_string()),
            })
            .unwrap();
            Box::new(sink).finalize().unwrap();
        }

        // Verify Parquet magic bytes (PAR1)
        let data = output.into_inner();
        assert!(!data.is_empty());
        assert_eq!(&data[0..4], b"PAR1");
    }

    #[test]
    fn test_parquet_empty_export() {
        let mut output = Cursor::new(Vec::new());
        {
            let sink = ParquetExportSink::new(&mut output).unwrap();
            Box::new(sink).finalize().unwrap();
        }

        // Empty export should produce empty output
        let data = output.into_inner();
        assert!(data.is_empty());
    }

    #[test]
    fn test_schema_fields() {
        let schema = ParquetExportSink::<Vec<u8>>::schema();
        assert_eq!(schema.fields().len(), 12);

        // Verify required fields are non-nullable
        let id_field = schema.field_with_name("id").unwrap();
        assert!(!id_field.is_nullable());

        // Verify optional fields are nullable
        let project_id_field = schema.field_with_name("project_id").unwrap();
        assert!(project_id_field.is_nullable());
    }
}
