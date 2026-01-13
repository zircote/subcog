//! Format adapters for import/export.
//!
//! Each format implements [`ImportSource`] and/or [`ExportSink`] traits.

pub mod csv;
pub mod json;
#[cfg(feature = "parquet-export")]
pub mod parquet;
pub mod yaml;

use crate::{Error, Result};
use std::io::{BufRead, Write};
use std::path::Path;
use std::str::FromStr;

use super::traits::{ExportSink, ImportSource};

/// Supported file formats for import/export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// JSON format (newline-delimited or array).
    Json,
    /// YAML format (document stream).
    Yaml,
    /// CSV format with configurable column mapping.
    Csv,
    /// Apache Parquet columnar format (export only).
    #[cfg(feature = "parquet-export")]
    Parquet,
}

impl Format {
    /// Returns all available formats for import.
    #[must_use]
    pub fn import_formats() -> Vec<Self> {
        vec![Self::Json, Self::Yaml, Self::Csv]
    }

    /// Returns all available formats for export.
    #[must_use]
    pub fn export_formats() -> Vec<Self> {
        let formats = vec![Self::Json, Self::Yaml, Self::Csv];
        #[cfg(feature = "parquet-export")]
        formats.push(Self::Parquet);
        formats
    }

    /// Returns the file extension for this format.
    #[must_use]
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Csv => "csv",
            #[cfg(feature = "parquet-export")]
            Self::Parquet => "parquet",
        }
    }

    /// Returns the MIME type for this format.
    #[must_use]
    pub const fn mime_type(&self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Yaml => "application/x-yaml",
            Self::Csv => "text/csv",
            #[cfg(feature = "parquet-export")]
            Self::Parquet => "application/vnd.apache.parquet",
        }
    }

    /// Detects format from file extension.
    ///
    /// # Errors
    ///
    /// Returns an error if the extension is not recognized.
    pub fn from_path(path: &Path) -> Result<Self> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase);

        match ext.as_deref() {
            Some("json" | "ndjson" | "jsonl") => Ok(Self::Json),
            Some("yaml" | "yml") => Ok(Self::Yaml),
            Some("csv" | "tsv") => Ok(Self::Csv),
            #[cfg(feature = "parquet-export")]
            Some("parquet" | "pq") => Ok(Self::Parquet),
            Some(ext) => Err(Error::InvalidInput(format!(
                "Unsupported file extension: .{ext}"
            ))),
            None => Err(Error::InvalidInput(
                "Cannot determine format: file has no extension".to_string(),
            )),
        }
    }

    /// Returns whether this format supports import.
    #[must_use]
    pub const fn supports_import(&self) -> bool {
        match self {
            Self::Json | Self::Yaml | Self::Csv => true,
            #[cfg(feature = "parquet-export")]
            Self::Parquet => false,
        }
    }

    /// Returns whether this format supports export.
    #[must_use]
    pub const fn supports_export(&self) -> bool {
        true // All formats support export
    }
}

impl FromStr for Format {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "json" | "ndjson" | "jsonl" => Ok(Self::Json),
            "yaml" | "yml" => Ok(Self::Yaml),
            "csv" | "tsv" => Ok(Self::Csv),
            #[cfg(feature = "parquet-export")]
            "parquet" | "pq" => Ok(Self::Parquet),
            _ => Err(Error::InvalidInput(format!("Unknown format: {s}"))),
        }
    }
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json => write!(f, "json"),
            Self::Yaml => write!(f, "yaml"),
            Self::Csv => write!(f, "csv"),
            #[cfg(feature = "parquet-export")]
            Self::Parquet => write!(f, "parquet"),
        }
    }
}

/// Creates an import source for the given format and reader.
///
/// # Errors
///
/// Returns an error if the format doesn't support import.
pub fn create_import_source<R: BufRead + 'static>(
    reader: R,
    format: Format,
) -> Result<Box<dyn ImportSource>> {
    match format {
        Format::Json => Ok(Box::new(json::JsonImportSource::new(reader))),
        Format::Yaml => Ok(Box::new(yaml::YamlImportSource::new(reader)?)),
        Format::Csv => Ok(Box::new(csv::CsvImportSource::new(reader)?)),
        #[cfg(feature = "parquet-export")]
        Format::Parquet => Err(Error::NotImplemented(
            "Parquet import is not supported".to_string(),
        )),
    }
}

/// Creates an export sink for the given format and writer.
///
/// # Errors
///
/// Returns an error if sink creation fails.
pub fn create_export_sink<W: Write + Send + 'static>(
    writer: W,
    format: Format,
) -> Result<Box<dyn ExportSink>> {
    match format {
        Format::Json => Ok(Box::new(json::JsonExportSink::new(writer))),
        Format::Yaml => Ok(Box::new(yaml::YamlExportSink::new(writer))),
        Format::Csv => Ok(Box::new(csv::CsvExportSink::new(writer)?)),
        #[cfg(feature = "parquet-export")]
        Format::Parquet => Ok(Box::new(parquet::ParquetExportSink::new(writer)?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_from_str() {
        assert_eq!(Format::from_str("json").unwrap(), Format::Json);
        assert_eq!(Format::from_str("YAML").unwrap(), Format::Yaml);
        assert_eq!(Format::from_str("csv").unwrap(), Format::Csv);
        assert!(Format::from_str("unknown").is_err());
    }

    #[test]
    fn test_format_from_path() {
        assert_eq!(
            Format::from_path(Path::new("test.json")).unwrap(),
            Format::Json
        );
        assert_eq!(
            Format::from_path(Path::new("test.yml")).unwrap(),
            Format::Yaml
        );
        assert_eq!(
            Format::from_path(Path::new("test.csv")).unwrap(),
            Format::Csv
        );
        assert!(Format::from_path(Path::new("test.txt")).is_err());
    }

    #[test]
    fn test_format_extension() {
        assert_eq!(Format::Json.extension(), "json");
        assert_eq!(Format::Yaml.extension(), "yaml");
        assert_eq!(Format::Csv.extension(), "csv");
    }

    #[test]
    fn test_format_supports() {
        assert!(Format::Json.supports_import());
        assert!(Format::Json.supports_export());
        assert!(Format::Yaml.supports_import());
        assert!(Format::Csv.supports_import());
    }
}
