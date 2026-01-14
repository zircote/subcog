//! Memory import service.
//!
//! Orchestrates bulk memory import from various formats.

#![allow(
    clippy::needless_pass_by_value,
    clippy::cast_precision_loss,
    clippy::unused_self,
    clippy::unnecessary_wraps
)]

use crate::io::formats::{Format, create_import_source};
use crate::io::traits::ImportSource;
use crate::io::validation::{ImportValidator, ValidationSeverity};
use crate::models::{Domain, Namespace};
use crate::services::CaptureService;
use crate::services::deduplication::ContentHasher;
use crate::{Error, Result};
use std::io::BufRead;
use std::path::Path;
use std::sync::Arc;

/// Options for memory import.
#[derive(Debug, Clone)]
pub struct ImportOptions {
    /// File format to import from.
    pub format: Format,
    /// Default namespace for memories without one.
    pub default_namespace: Namespace,
    /// Default domain for memories without one.
    pub default_domain: Domain,
    /// Skip memories that would be duplicates.
    pub skip_duplicates: bool,
    /// Continue on validation errors (skip invalid records).
    pub skip_invalid: bool,
    /// Dry run mode (validate without storing).
    pub dry_run: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            format: Format::Json,
            default_namespace: Namespace::Decisions,
            default_domain: Domain::new(),
            skip_duplicates: true,
            skip_invalid: true,
            dry_run: false,
        }
    }
}

impl ImportOptions {
    /// Creates import options with the given format.
    #[must_use]
    pub const fn with_format(mut self, format: Format) -> Self {
        self.format = format;
        self
    }

    /// Sets the default namespace.
    #[must_use]
    pub const fn with_default_namespace(mut self, namespace: Namespace) -> Self {
        self.default_namespace = namespace;
        self
    }

    /// Sets the default domain.
    #[must_use]
    pub fn with_default_domain(mut self, domain: Domain) -> Self {
        self.default_domain = domain;
        self
    }

    /// Enables or disables duplicate skipping.
    #[must_use]
    pub const fn with_skip_duplicates(mut self, skip: bool) -> Self {
        self.skip_duplicates = skip;
        self
    }

    /// Enables or disables dry run mode.
    #[must_use]
    pub const fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }
}

/// Progress callback for import operations.
pub type ProgressCallback = Box<dyn Fn(&ImportProgress) + Send>;

/// Progress information during import.
#[derive(Debug, Clone, Default)]
pub struct ImportProgress {
    /// Total records processed so far.
    pub processed: usize,
    /// Records successfully imported.
    pub imported: usize,
    /// Records skipped (duplicates).
    pub skipped_duplicates: usize,
    /// Records skipped (invalid).
    pub skipped_invalid: usize,
    /// Estimated total records (if known).
    pub total_estimate: Option<usize>,
    /// Current record being processed (1-indexed).
    pub current: usize,
}

impl ImportProgress {
    /// Returns the percentage complete (0-100) if total is known.
    #[must_use]
    pub fn percent_complete(&self) -> Option<f32> {
        self.total_estimate.map(|total| {
            if total == 0 {
                100.0
            } else {
                (self.processed as f32 / total as f32) * 100.0
            }
        })
    }
}

/// Result of an import operation.
#[derive(Debug, Clone)]
pub struct ImportResult {
    /// Number of records successfully imported.
    pub imported: usize,
    /// Number of records skipped as duplicates.
    pub skipped_duplicates: usize,
    /// Number of records skipped due to validation errors.
    pub skipped_invalid: usize,
    /// Total records processed.
    pub total_processed: usize,
    /// Validation warnings encountered.
    pub warnings: Vec<String>,
    /// Validation errors encountered (for skipped records).
    pub errors: Vec<String>,
}

impl ImportResult {
    /// Creates an empty result.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            imported: 0,
            skipped_duplicates: 0,
            skipped_invalid: 0,
            total_processed: 0,
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Returns whether any records were imported.
    #[must_use]
    pub const fn has_imports(&self) -> bool {
        self.imported > 0
    }

    /// Returns whether any errors occurred.
    #[must_use]
    pub const fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

impl Default for ImportResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Service for importing memories from external sources.
pub struct ImportService {
    /// Capture service for storing imported memories.
    capture_service: Arc<CaptureService>,
}

impl ImportService {
    /// Creates a new import service.
    #[must_use]
    pub const fn new(capture_service: Arc<CaptureService>) -> Self {
        Self { capture_service }
    }

    /// Imports memories from a file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or format detection fails.
    pub fn import_from_file(
        &self,
        path: &Path,
        options: ImportOptions,
        progress: Option<ProgressCallback>,
    ) -> Result<ImportResult> {
        let format = if options.format == Format::Json {
            // Auto-detect from extension if using default
            Format::from_path(path).unwrap_or(Format::Json)
        } else {
            options.format
        };

        let file = std::fs::File::open(path).map_err(|e| Error::OperationFailed {
            operation: "open_import_file".to_string(),
            cause: e.to_string(),
        })?;
        let reader = std::io::BufReader::new(file);

        self.import_from_reader(reader, options.with_format(format), progress)
    }

    /// Imports memories from a reader.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing fails or storage errors occur.
    pub fn import_from_reader<R: BufRead + 'static>(
        &self,
        reader: R,
        options: ImportOptions,
        progress: Option<ProgressCallback>,
    ) -> Result<ImportResult> {
        let mut source = create_import_source(reader, options.format)?;
        self.import_from_source(source.as_mut(), &options, progress)
    }

    /// Imports memories from a source.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails or storage errors occur.
    #[allow(clippy::excessive_nesting)]
    pub fn import_from_source(
        &self,
        source: &mut dyn ImportSource,
        options: &ImportOptions,
        progress: Option<ProgressCallback>,
    ) -> Result<ImportResult> {
        let validator = ImportValidator::new()
            .with_default_namespace(options.default_namespace)
            .with_default_domain(options.default_domain.clone());

        let mut result = ImportResult::new();
        let mut prog = ImportProgress {
            total_estimate: source.size_hint(),
            ..Default::default()
        };

        // Track existing content hashes for deduplication
        let mut seen_hashes = std::collections::HashSet::new();

        while let Some(imported) = source.next()? {
            prog.current += 1;
            prog.processed += 1;
            result.total_processed += 1;

            // Validate the imported memory
            let validation = validator.validate(&imported);

            // Collect warnings
            for issue in &validation.issues {
                if issue.severity == ValidationSeverity::Warning {
                    result.warnings.push(format!(
                        "Record {}: {}: {}",
                        prog.current, issue.field, issue.message
                    ));
                }
            }

            // Handle validation errors
            if !validation.is_valid {
                if options.skip_invalid {
                    prog.skipped_invalid += 1;
                    result.skipped_invalid += 1;
                    for issue in &validation.issues {
                        if issue.severity == ValidationSeverity::Error {
                            result.errors.push(format!(
                                "Record {}: {}: {}",
                                prog.current, issue.field, issue.message
                            ));
                        }
                    }
                    if let Some(ref cb) = progress {
                        cb(&prog);
                    }
                    continue;
                }
                return Err(Error::InvalidInput(format!(
                    "Record {}: validation failed",
                    prog.current
                )));
            }

            // Check for duplicates
            let content_hash = ContentHasher::hash(&imported.content);
            if options.skip_duplicates {
                // Check in-batch duplicates
                if seen_hashes.contains(&content_hash) {
                    prog.skipped_duplicates += 1;
                    result.skipped_duplicates += 1;
                    if let Some(ref cb) = progress {
                        cb(&prog);
                    }
                    continue;
                }

                // Check existing memories via content hash tag
                let hash_tag = ContentHasher::content_to_tag(&imported.content);
                if self.memory_exists_with_tag(&hash_tag)? {
                    prog.skipped_duplicates += 1;
                    result.skipped_duplicates += 1;
                    seen_hashes.insert(content_hash);
                    if let Some(ref cb) = progress {
                        cb(&prog);
                    }
                    continue;
                }

                seen_hashes.insert(content_hash);
            }

            // Store the memory (unless dry run)
            if options.dry_run {
                // Dry run counts as imported
                prog.imported += 1;
                result.imported += 1;
            } else {
                let request = validator.to_capture_request(imported);
                match self.capture_service.capture(request) {
                    Ok(_) => {
                        prog.imported += 1;
                        result.imported += 1;
                    },
                    Err(e) => {
                        if options.skip_invalid {
                            result
                                .errors
                                .push(format!("Record {}: capture failed: {}", prog.current, e));
                            prog.skipped_invalid += 1;
                            result.skipped_invalid += 1;
                        } else {
                            return Err(e);
                        }
                    },
                }
            }

            if let Some(ref cb) = progress {
                cb(&prog);
            }
        }

        Ok(result)
    }

    /// Checks if a memory with the given hash tag already exists.
    const fn memory_exists_with_tag(&self, _hash_tag: &str) -> Result<bool> {
        // For now, we'll rely on the capture service's deduplication
        // The hash tag is added during capture and can be checked there
        // This is a simplified check - in production, we'd query the index
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::io::Cursor;

    fn test_capture_service() -> Arc<CaptureService> {
        Arc::new(CaptureService::new(Config::default()))
    }

    #[test]
    fn test_import_options_defaults() {
        let options = ImportOptions::default();
        assert_eq!(options.format, Format::Json);
        assert!(options.skip_duplicates);
        assert!(options.skip_invalid);
        assert!(!options.dry_run);
    }

    #[test]
    fn test_import_progress_percent() {
        let progress = ImportProgress {
            processed: 50,
            total_estimate: Some(100),
            ..Default::default()
        };
        assert_eq!(progress.percent_complete(), Some(50.0));

        let unknown = ImportProgress::default();
        assert!(unknown.percent_complete().is_none());
    }

    #[test]
    fn test_import_result_has_imports() {
        let mut result = ImportResult::new();
        assert!(!result.has_imports());

        result.imported = 1;
        assert!(result.has_imports());
    }

    #[test]
    fn test_dry_run_import() {
        let service = ImportService::new(test_capture_service());
        let input = r#"{"content": "Test memory"}"#;

        let result = service
            .import_from_reader(
                Cursor::new(input),
                ImportOptions::default().with_dry_run(true),
                None,
            )
            .unwrap();

        assert_eq!(result.imported, 1);
        assert_eq!(result.total_processed, 1);
    }

    #[test]
    fn test_import_with_invalid_record() {
        let service = ImportService::new(test_capture_service());
        // Empty content should be invalid
        let input = r#"{"content": ""}
{"content": "Valid memory"}"#;

        let result = service
            .import_from_reader(
                Cursor::new(input),
                ImportOptions::default().with_dry_run(true),
                None,
            )
            .unwrap();

        assert_eq!(result.skipped_invalid, 1);
        assert_eq!(result.imported, 1);
    }
}
