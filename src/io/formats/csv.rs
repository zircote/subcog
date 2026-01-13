//! CSV format adapter for import/export.
//!
//! Supports configurable column mapping with sensible defaults.

use crate::io::traits::{ExportSink, ExportableMemory, ImportSource, ImportedMemory};
use crate::{Error, Result};
use std::io::{BufRead, Write};

/// CSV import source.
///
/// Reads CSV files with configurable column mapping.
/// First row is expected to be headers unless configured otherwise.
pub struct CsvImportSource<R: BufRead> {
    /// CSV reader.
    reader: csv::Reader<R>,
    /// Column indices for each field.
    column_map: ColumnMap,
}

/// Maps CSV column indices to memory fields.
#[derive(Debug, Default)]
struct ColumnMap {
    content: Option<usize>,
    namespace: Option<usize>,
    domain: Option<usize>,
    tags: Option<usize>,
    source: Option<usize>,
    created_at: Option<usize>,
    ttl_seconds: Option<usize>,
}

impl ColumnMap {
    /// Creates a column map from CSV headers.
    fn from_headers(headers: &csv::StringRecord) -> Result<Self> {
        let mut map = Self::default();

        for (i, header) in headers.iter().enumerate() {
            match header.to_lowercase().as_str() {
                "content" | "text" | "memory" | "body" => map.content = Some(i),
                "namespace" | "ns" | "category" | "type" => map.namespace = Some(i),
                "domain" | "scope" => map.domain = Some(i),
                "tags" | "labels" | "keywords" => map.tags = Some(i),
                "source" | "src" | "origin" | "file" => map.source = Some(i),
                "created_at" | "created" | "timestamp" | "date" => map.created_at = Some(i),
                "ttl" | "ttl_seconds" | "expiry" => map.ttl_seconds = Some(i),
                _ => {} // Ignore unknown columns
            }
        }

        // Content is required
        if map.content.is_none() {
            return Err(Error::InvalidInput(
                "CSV must have a 'content' column (or 'text', 'memory', 'body')".to_string(),
            ));
        }

        Ok(map)
    }
}

impl<R: BufRead> CsvImportSource<R> {
    /// Creates a new CSV import source.
    ///
    /// # Errors
    ///
    /// Returns an error if headers cannot be read or 'content' column is missing.
    pub fn new(reader: R) -> Result<Self> {
        let mut csv_reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true) // Allow varying number of fields
            .trim(csv::Trim::All)
            .from_reader(reader);

        let headers = csv_reader
            .headers()
            .map_err(|e| Error::OperationFailed {
                operation: "read_csv_headers".to_string(),
                cause: e.to_string(),
            })?
            .clone();
        let column_map = ColumnMap::from_headers(&headers)?;

        Ok(Self {
            reader: csv_reader,
            column_map,
        })
    }

    /// Parses a record into an imported memory.
    fn parse_record(&self, record: &csv::StringRecord) -> Result<ImportedMemory> {
        let get_field = |idx: Option<usize>| -> Option<String> {
            idx.and_then(|i| record.get(i))
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
        };

        let content = get_field(self.column_map.content)
            .ok_or_else(|| Error::InvalidInput("Missing content field".to_string()))?;

        let tags = get_field(self.column_map.tags)
            .map(|t| {
                t.split(|c| c == ',' || c == ';' || c == '|')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        let created_at = get_field(self.column_map.created_at)
            .and_then(|s| s.parse::<u64>().ok());

        let ttl_seconds = get_field(self.column_map.ttl_seconds)
            .and_then(|s| s.parse::<u64>().ok());

        Ok(ImportedMemory {
            content,
            namespace: get_field(self.column_map.namespace),
            domain: get_field(self.column_map.domain),
            tags,
            source: get_field(self.column_map.source),
            created_at,
            ttl_seconds,
        })
    }
}

impl<R: BufRead> ImportSource for CsvImportSource<R> {
    fn next(&mut self) -> Result<Option<ImportedMemory>> {
        let mut record = csv::StringRecord::new();

        let has_record = self
            .reader
            .read_record(&mut record)
            .map_err(|e| Error::OperationFailed {
                operation: "read_csv".to_string(),
                cause: e.to_string(),
            })?;
        if !has_record {
            return Ok(None);
        }

        let memory = self.parse_record(&record)?;
        Ok(Some(memory))
    }
}

/// CSV export sink.
///
/// Writes memories as CSV with headers.
pub struct CsvExportSink<W: Write> {
    writer: csv::Writer<W>,
    /// Whether headers have been written.
    headers_written: bool,
}

impl<W: Write> CsvExportSink<W> {
    /// Creates a new CSV export sink.
    ///
    /// # Errors
    ///
    /// Returns an error if the writer cannot be created.
    pub fn new(writer: W) -> Result<Self> {
        let csv_writer = csv::WriterBuilder::new()
            .has_headers(false) // We write headers manually
            .from_writer(writer);

        Ok(Self {
            writer: csv_writer,
            headers_written: false,
        })
    }

    /// Writes headers if not already written.
    fn ensure_headers(&mut self) -> Result<()> {
        if !self.headers_written {
            self.writer
                .write_record([
                    "id",
                    "content",
                    "namespace",
                    "domain",
                    "project_id",
                    "branch",
                    "file_path",
                    "status",
                    "created_at",
                    "updated_at",
                    "tags",
                    "source",
                ])
                .map_err(|e| Error::OperationFailed {
                    operation: "write_csv_headers".to_string(),
                    cause: e.to_string(),
                })?;
            self.headers_written = true;
        }
        Ok(())
    }
}

impl<W: Write + Send> ExportSink for CsvExportSink<W> {
    fn write(&mut self, memory: &ExportableMemory) -> Result<()> {
        self.ensure_headers()?;

        self.writer
            .write_record([
                &memory.id,
                &memory.content,
                &memory.namespace,
                &memory.domain,
                memory.project_id.as_deref().unwrap_or(""),
                memory.branch.as_deref().unwrap_or(""),
                memory.file_path.as_deref().unwrap_or(""),
                &memory.status,
                &memory.created_at.to_string(),
                &memory.updated_at.to_string(),
                &memory.tags.join(","),
                memory.source.as_deref().unwrap_or(""),
            ])
            .map_err(|e| Error::OperationFailed {
                operation: "write_csv".to_string(),
                cause: e.to_string(),
            })?;

        Ok(())
    }

    fn finalize(mut self: Box<Self>) -> Result<()> {
        self.writer.flush().map_err(|e| Error::OperationFailed {
            operation: "flush_csv".to_string(),
            cause: e.to_string(),
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_import_basic_csv() {
        let input = r#"content,namespace,tags
"First memory",decisions,"rust,test"
"Second memory",learnings,""
"#;
        let mut source = CsvImportSource::new(Cursor::new(input)).unwrap();

        let first = source.next().unwrap().unwrap();
        assert_eq!(first.content, "First memory");
        assert_eq!(first.namespace, Some("decisions".to_string()));
        assert_eq!(first.tags, vec!["rust", "test"]);

        let second = source.next().unwrap().unwrap();
        assert_eq!(second.content, "Second memory");
        assert_eq!(second.namespace, Some("learnings".to_string()));
        assert!(second.tags.is_empty());

        assert!(source.next().unwrap().is_none());
    }

    #[test]
    fn test_import_alternative_headers() {
        let input = r#"text,category,labels
"Memory content",decisions,"tag1|tag2"
"#;
        let mut source = CsvImportSource::new(Cursor::new(input)).unwrap();

        let memory = source.next().unwrap().unwrap();
        assert_eq!(memory.content, "Memory content");
        assert_eq!(memory.namespace, Some("decisions".to_string()));
        assert_eq!(memory.tags, vec!["tag1", "tag2"]);
    }

    #[test]
    fn test_import_missing_content_column() {
        let input = "namespace,tags\ndecisions,test\n";
        let result = CsvImportSource::new(Cursor::new(input));
        assert!(result.is_err());
    }

    #[test]
    fn test_export_csv() {
        let mut output = Vec::new();
        {
            let mut sink = CsvExportSink::new(&mut output).unwrap();
            sink.write(&ExportableMemory {
                id: "1".to_string(),
                content: "Test memory".to_string(),
                namespace: "decisions".to_string(),
                domain: "project".to_string(),
                project_id: Some("repo".to_string()),
                branch: None,
                file_path: None,
                status: "active".to_string(),
                created_at: 1234567890,
                updated_at: 1234567890,
                tags: vec!["rust".to_string(), "test".to_string()],
                source: None,
            })
            .unwrap();
            Box::new(sink).finalize().unwrap();
        }

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("id,content,namespace"));
        assert!(output_str.contains("Test memory"));
        assert!(output_str.contains("rust,test"));
    }

    #[test]
    fn test_tag_delimiters() {
        // Test various tag delimiters
        let input = r#"content,tags
"Memory 1","a,b,c"
"Memory 2","x;y;z"
"Memory 3","p|q|r"
"#;
        let mut source = CsvImportSource::new(Cursor::new(input)).unwrap();

        let m1 = source.next().unwrap().unwrap();
        assert_eq!(m1.tags, vec!["a", "b", "c"]);

        let m2 = source.next().unwrap().unwrap();
        assert_eq!(m2.tags, vec!["x", "y", "z"]);

        let m3 = source.next().unwrap().unwrap();
        assert_eq!(m3.tags, vec!["p", "q", "r"]);
    }
}
