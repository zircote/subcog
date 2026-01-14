//! JSON format adapter for import/export.
//!
//! Supports both newline-delimited JSON (NDJSON/JSONL) and JSON arrays.

use crate::io::traits::{ExportSink, ExportableMemory, ImportSource, ImportedMemory};
use crate::{Error, Result};
use std::io::{BufRead, Write};

/// JSON import source.
///
/// Automatically detects and handles both formats:
/// - **NDJSON/JSONL**: One JSON object per line
/// - **Array**: A JSON array of objects `[{...}, {...}]`
pub struct JsonImportSource<R: BufRead> {
    reader: R,
    /// Buffered memories when parsing array format.
    buffer: Vec<ImportedMemory>,
    /// Current index into buffer (for array format).
    buffer_index: usize,
    /// Whether we've detected and started parsing.
    started: bool,
    /// Whether we're in array mode.
    array_mode: bool,
    /// Line number for error reporting.
    line_number: usize,
}

impl<R: BufRead> JsonImportSource<R> {
    /// Creates a new JSON import source.
    #[must_use]
    pub const fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
            buffer_index: 0,
            started: false,
            array_mode: false,
            line_number: 0,
        }
    }

    /// Peeks at the first non-whitespace character to detect format.
    fn detect_format(&mut self) -> Result<bool> {
        // Read first line to detect format
        let mut first_line = String::new();
        let bytes_read =
            self.reader
                .read_line(&mut first_line)
                .map_err(|e| Error::OperationFailed {
                    operation: "read_json".to_string(),
                    cause: e.to_string(),
                })?;
        if bytes_read == 0 {
            return Ok(false); // Empty file
        }
        self.line_number = 1;

        let trimmed = first_line.trim();
        if trimmed.is_empty() {
            return Ok(false);
        }

        // If starts with '[', it's an array
        if trimmed.starts_with('[') {
            self.array_mode = true;
            // Read remaining content and prepend first line
            let mut remaining = String::new();
            self.reader
                .read_to_string(&mut remaining)
                .map_err(|e| Error::OperationFailed {
                    operation: "read_json".to_string(),
                    cause: e.to_string(),
                })?;
            let full_content = format!("{first_line}{remaining}");

            let memories: Vec<ImportedMemory> = serde_json::from_str(&full_content)
                .map_err(|e| Error::InvalidInput(format!("Failed to parse JSON array: {e}")))?;

            self.buffer = memories;
        } else {
            // NDJSON mode - parse first line as object
            let memory: ImportedMemory = serde_json::from_str(trimmed).map_err(|e| {
                Error::InvalidInput(format!("Line 1: Failed to parse JSON object: {e}"))
            })?;
            self.buffer.push(memory);
        }

        self.buffer_index = 0;
        self.started = true;
        Ok(true)
    }
}

impl<R: BufRead> ImportSource for JsonImportSource<R> {
    fn next(&mut self) -> Result<Option<ImportedMemory>> {
        // First call: detect format
        if !self.started && !self.detect_format()? {
            return Ok(None);
        }

        // Array mode: return from buffer
        if self.array_mode {
            if self.buffer_index < self.buffer.len() {
                let memory = self.buffer[self.buffer_index].clone();
                self.buffer_index += 1;
                return Ok(Some(memory));
            }
            return Ok(None);
        }

        // NDJSON mode: return buffered first, then read lines
        if self.buffer_index < self.buffer.len() {
            let memory = self.buffer[self.buffer_index].clone();
            self.buffer_index += 1;
            return Ok(Some(memory));
        }

        // Read next line
        let mut line = String::new();
        loop {
            line.clear();
            let bytes_read =
                self.reader
                    .read_line(&mut line)
                    .map_err(|e| Error::OperationFailed {
                        operation: "read_json".to_string(),
                        cause: e.to_string(),
                    })?;
            if bytes_read == 0 {
                return Ok(None);
            }
            self.line_number += 1;

            let trimmed = line.trim();
            if !trimmed.is_empty() {
                break;
            }
        }

        let memory: ImportedMemory = serde_json::from_str(line.trim()).map_err(|e| {
            Error::InvalidInput(format!(
                "Line {}: Failed to parse JSON: {e}",
                self.line_number
            ))
        })?;

        Ok(Some(memory))
    }

    fn size_hint(&self) -> Option<usize> {
        if self.array_mode {
            Some(self.buffer.len())
        } else {
            None
        }
    }
}

/// JSON export sink.
///
/// Writes memories as newline-delimited JSON (NDJSON).
pub struct JsonExportSink<W: Write> {
    writer: W,
    /// Number of records written.
    count: usize,
}

impl<W: Write> JsonExportSink<W> {
    /// Creates a new JSON export sink.
    #[must_use]
    pub const fn new(writer: W) -> Self {
        Self { writer, count: 0 }
    }
}

impl<W: Write + Send> ExportSink for JsonExportSink<W> {
    fn write(&mut self, memory: &ExportableMemory) -> Result<()> {
        serde_json::to_writer(&mut self.writer, memory).map_err(|e| Error::OperationFailed {
            operation: "write_json".to_string(),
            cause: e.to_string(),
        })?;
        writeln!(self.writer).map_err(|e| Error::OperationFailed {
            operation: "write_json".to_string(),
            cause: e.to_string(),
        })?;
        self.count += 1;
        Ok(())
    }

    fn finalize(mut self: Box<Self>) -> Result<()> {
        self.writer.flush().map_err(|e| Error::OperationFailed {
            operation: "flush_json".to_string(),
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
    fn test_import_ndjson() {
        let input = r#"{"content": "First memory", "namespace": "decisions"}
{"content": "Second memory", "tags": ["rust", "test"]}
"#;
        let mut source = JsonImportSource::new(Cursor::new(input));

        let first = source.next().unwrap().unwrap();
        assert_eq!(first.content, "First memory");
        assert_eq!(first.namespace, Some("decisions".to_string()));

        let second = source.next().unwrap().unwrap();
        assert_eq!(second.content, "Second memory");
        assert_eq!(second.tags, vec!["rust", "test"]);

        assert!(source.next().unwrap().is_none());
    }

    #[test]
    fn test_import_array() {
        let input = r#"[
            {"content": "First memory"},
            {"content": "Second memory"}
        ]"#;
        let mut source = JsonImportSource::new(Cursor::new(input));

        let first = source.next().unwrap().unwrap();
        assert_eq!(first.content, "First memory");

        let second = source.next().unwrap().unwrap();
        assert_eq!(second.content, "Second memory");

        assert!(source.next().unwrap().is_none());
    }

    #[test]
    fn test_export_ndjson() {
        let mut output = Vec::new();
        {
            let mut sink = JsonExportSink::new(&mut output);
            sink.write(&ExportableMemory {
                id: "1".to_string(),
                content: "Test".to_string(),
                namespace: "decisions".to_string(),
                domain: "project".to_string(),
                project_id: None,
                branch: None,
                file_path: None,
                status: "active".to_string(),
                created_at: 0,
                updated_at: 0,
                tags: vec![],
                source: None,
            })
            .unwrap();
            Box::new(sink).finalize().unwrap();
        }

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("\"content\":\"Test\""));
        assert!(output_str.ends_with('\n'));
    }
}
