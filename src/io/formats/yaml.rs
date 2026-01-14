//! YAML format adapter for import/export.
//!
//! Supports YAML document streams (multiple documents separated by `---`).

use crate::io::traits::{ExportSink, ExportableMemory, ImportSource, ImportedMemory};
use crate::{Error, Result};
use std::io::{BufRead, Write};

/// YAML import source.
///
/// Reads YAML document streams where each document is a memory object.
/// Documents are separated by `---` markers.
pub struct YamlImportSource {
    /// Pre-parsed memories from the YAML stream.
    memories: Vec<ImportedMemory>,
    /// Current index.
    index: usize,
}

impl YamlImportSource {
    /// Creates a new YAML import source.
    ///
    /// Parses all documents upfront since YAML requires full parsing.
    ///
    /// # Errors
    ///
    /// Returns an error if YAML parsing fails.
    pub fn new<R: BufRead>(mut reader: R) -> Result<Self> {
        let mut content = String::new();
        reader
            .read_to_string(&mut content)
            .map_err(|e| Error::OperationFailed {
                operation: "read_yaml".to_string(),
                cause: e.to_string(),
            })?;

        if content.trim().is_empty() {
            return Ok(Self {
                memories: Vec::new(),
                index: 0,
            });
        }

        // Try parsing as a sequence first (array of memories)
        if let Ok(memories) = serde_yaml_ng::from_str::<Vec<ImportedMemory>>(&content) {
            return Ok(Self { memories, index: 0 });
        }

        // Try parsing as multi-document stream
        let mut memories = Vec::new();
        for (doc_index, document) in serde_yaml_ng::Deserializer::from_str(&content).enumerate() {
            let memory: ImportedMemory =
                serde::Deserialize::deserialize(document).map_err(|e| {
                    Error::InvalidInput(format!(
                        "Document {}: Failed to parse YAML: {e}",
                        doc_index + 1
                    ))
                })?;
            memories.push(memory);
        }

        Ok(Self { memories, index: 0 })
    }
}

impl ImportSource for YamlImportSource {
    fn next(&mut self) -> Result<Option<ImportedMemory>> {
        if self.index < self.memories.len() {
            let memory = self.memories[self.index].clone();
            self.index += 1;
            Ok(Some(memory))
        } else {
            Ok(None)
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.memories.len())
    }
}

/// YAML export sink.
///
/// Writes memories as a YAML document stream with `---` separators.
pub struct YamlExportSink<W: Write> {
    writer: W,
    /// Number of records written.
    count: usize,
}

impl<W: Write> YamlExportSink<W> {
    /// Creates a new YAML export sink.
    #[must_use]
    pub const fn new(writer: W) -> Self {
        Self { writer, count: 0 }
    }
}

impl<W: Write + Send> ExportSink for YamlExportSink<W> {
    fn write(&mut self, memory: &ExportableMemory) -> Result<()> {
        // Write document separator (except for first document)
        if self.count > 0 {
            writeln!(self.writer, "---").map_err(|e| Error::OperationFailed {
                operation: "write_yaml".to_string(),
                cause: e.to_string(),
            })?;
        }

        serde_yaml_ng::to_writer(&mut self.writer, memory).map_err(|e| Error::OperationFailed {
            operation: "write_yaml".to_string(),
            cause: e.to_string(),
        })?;
        self.count += 1;
        Ok(())
    }

    fn finalize(mut self: Box<Self>) -> Result<()> {
        self.writer.flush().map_err(|e| Error::OperationFailed {
            operation: "flush_yaml".to_string(),
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
    fn test_import_single_document() {
        let input = r"
content: Test memory
namespace: decisions
tags:
  - rust
  - test
";
        let mut source = YamlImportSource::new(Cursor::new(input)).unwrap();

        let memory = source.next().unwrap().unwrap();
        assert_eq!(memory.content, "Test memory");
        assert_eq!(memory.namespace, Some("decisions".to_string()));
        assert_eq!(memory.tags, vec!["rust", "test"]);

        assert!(source.next().unwrap().is_none());
    }

    #[test]
    fn test_import_multi_document() {
        let input = r"---
content: First memory
---
content: Second memory
namespace: learnings
";
        let mut source = YamlImportSource::new(Cursor::new(input)).unwrap();

        let first = source.next().unwrap().unwrap();
        assert_eq!(first.content, "First memory");

        let second = source.next().unwrap().unwrap();
        assert_eq!(second.content, "Second memory");
        assert_eq!(second.namespace, Some("learnings".to_string()));

        assert!(source.next().unwrap().is_none());
    }

    #[test]
    fn test_import_array_format() {
        let input = r"
- content: First memory
- content: Second memory
  tags:
    - test
";
        let mut source = YamlImportSource::new(Cursor::new(input)).unwrap();

        let first = source.next().unwrap().unwrap();
        assert_eq!(first.content, "First memory");

        let second = source.next().unwrap().unwrap();
        assert_eq!(second.content, "Second memory");

        assert!(source.next().unwrap().is_none());
    }

    #[test]
    fn test_export_yaml() {
        let mut output = Vec::new();
        {
            let mut sink = YamlExportSink::new(&mut output);
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
            sink.write(&ExportableMemory {
                id: "2".to_string(),
                content: "Second".to_string(),
                namespace: "learnings".to_string(),
                domain: "user".to_string(),
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
        assert!(output_str.contains("content: Test"));
        assert!(output_str.contains("---"));
        assert!(output_str.contains("content: Second"));
    }

    #[test]
    fn test_empty_input() {
        let input = "";
        let mut source = YamlImportSource::new(Cursor::new(input)).unwrap();
        assert!(source.next().unwrap().is_none());
    }
}
