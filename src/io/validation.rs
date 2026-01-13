//! Import validation and normalization.
//!
//! Validates imported memory data and applies defaults before storage.

use crate::models::{CaptureRequest, Domain, Namespace};
use crate::services::deduplication::ContentHasher;

use super::traits::ImportedMemory;

/// Severity of a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationSeverity {
    /// Warning: issue noted but import can proceed.
    Warning,
    /// Error: import of this record should be skipped.
    Error,
}

/// A validation issue found during import.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// The field that has an issue.
    pub field: String,
    /// Description of the issue.
    pub message: String,
    /// Severity of the issue.
    pub severity: ValidationSeverity,
}

impl ValidationIssue {
    /// Creates a warning issue.
    #[must_use]
    pub fn warning(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            severity: ValidationSeverity::Warning,
        }
    }

    /// Creates an error issue.
    #[must_use]
    pub fn error(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            severity: ValidationSeverity::Error,
        }
    }
}

/// Result of validating an imported memory.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the memory is valid for import.
    pub is_valid: bool,
    /// Issues found during validation.
    pub issues: Vec<ValidationIssue>,
    /// Content hash for deduplication.
    pub content_hash: String,
}

impl ValidationResult {
    /// Creates a successful validation result.
    #[must_use]
    pub fn valid(content_hash: String) -> Self {
        Self {
            is_valid: true,
            issues: Vec::new(),
            content_hash,
        }
    }

    /// Creates a failed validation result.
    #[must_use]
    pub fn invalid(issues: Vec<ValidationIssue>) -> Self {
        Self {
            is_valid: false,
            issues,
            content_hash: String::new(),
        }
    }

    /// Adds a warning to the result.
    #[must_use]
    pub fn with_warning(mut self, field: impl Into<String>, message: impl Into<String>) -> Self {
        self.issues.push(ValidationIssue::warning(field, message));
        self
    }
}

/// Validates and normalizes imported memory data.
///
/// Applies defaults for missing fields and validates required fields.
///
/// # Defaults
///
/// - `namespace`: Configurable, defaults to `Namespace::Decisions`
/// - `domain`: Context-dependent (project if in git repo, else user)
/// - `tags`: Empty vector
/// - `source`: None
pub struct ImportValidator {
    /// Default namespace for memories without one.
    default_namespace: Namespace,
    /// Default domain for memories without one.
    default_domain: Domain,
    /// Maximum content length (bytes).
    max_content_length: usize,
}

impl Default for ImportValidator {
    fn default() -> Self {
        Self {
            default_namespace: Namespace::Decisions,
            default_domain: Domain::new(),
            max_content_length: 500_000, // 500KB, same as CaptureService
        }
    }
}

impl ImportValidator {
    /// Creates a new validator with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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

    /// Validates an imported memory.
    ///
    /// # Returns
    ///
    /// A [`ValidationResult`] indicating whether the memory is valid
    /// and any issues found.
    #[must_use]
    pub fn validate(&self, imported: &ImportedMemory) -> ValidationResult {
        let mut issues = Vec::new();

        // Content is required and must not be empty
        let trimmed = imported.content.trim();
        if trimmed.is_empty() {
            issues.push(ValidationIssue::error("content", "Content cannot be empty"));
            return ValidationResult::invalid(issues);
        }

        // Check content length
        if imported.content.len() > self.max_content_length {
            issues.push(ValidationIssue::error(
                "content",
                format!(
                    "Content exceeds maximum size of {} bytes (got {} bytes)",
                    self.max_content_length,
                    imported.content.len()
                ),
            ));
            return ValidationResult::invalid(issues);
        }

        // Validate namespace if provided
        if let Some(ref ns) = imported.namespace {
            if Namespace::parse(ns).is_none() {
                issues.push(ValidationIssue::warning(
                    "namespace",
                    format!("Unknown namespace '{}', using default", ns),
                ));
            }
        } else {
            issues.push(ValidationIssue::warning(
                "namespace",
                "Namespace not specified, using default",
            ));
        }

        // Compute content hash for deduplication
        let content_hash = ContentHasher::hash(&imported.content);

        let mut result = ValidationResult::valid(content_hash);
        result.issues = issues;
        result
    }

    /// Converts an imported memory to a capture request.
    ///
    /// Applies defaults for missing fields.
    #[must_use]
    pub fn to_capture_request(&self, imported: ImportedMemory) -> CaptureRequest {
        let namespace = imported
            .namespace
            .as_ref()
            .and_then(|ns| Namespace::parse(ns))
            .unwrap_or(self.default_namespace);

        let domain = imported
            .domain
            .as_ref()
            .map(|d| parse_domain(d))
            .unwrap_or_else(|| self.default_domain.clone());

        CaptureRequest {
            content: imported.content,
            namespace,
            domain,
            tags: imported.tags,
            source: imported.source,
            skip_security_check: false,
            ttl_seconds: imported.ttl_seconds,
            scope: None,
        }
    }

    /// Returns the content hash tag for an imported memory.
    ///
    /// Used for duplicate detection before capture.
    #[must_use]
    pub fn content_hash_tag(&self, imported: &ImportedMemory) -> String {
        ContentHasher::content_to_tag(&imported.content)
    }
}

/// Parses a domain string into a Domain.
fn parse_domain(s: &str) -> Domain {
    match s.to_lowercase().as_str() {
        "user" => Domain::for_user(),
        "org" => Domain::for_org(),
        _ => Domain::new(), // Default to project-scoped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_memory() {
        let validator = ImportValidator::new();
        let imported = ImportedMemory::new("Valid content")
            .with_namespace("decisions")
            .with_tag("test");

        let result = validator.validate(&imported);
        assert!(result.is_valid);
        assert!(!result.content_hash.is_empty());
    }

    #[test]
    fn test_validate_empty_content() {
        let validator = ImportValidator::new();
        let imported = ImportedMemory::new("   ");

        let result = validator.validate(&imported);
        assert!(!result.is_valid);
        assert!(result.issues.iter().any(|i| i.field == "content"));
    }

    #[test]
    fn test_validate_content_too_long() {
        let validator = ImportValidator::new();
        let imported = ImportedMemory::new("x".repeat(600_000));

        let result = validator.validate(&imported);
        assert!(!result.is_valid);
        assert!(result.issues.iter().any(|i| i.message.contains("maximum size")));
    }

    #[test]
    fn test_validate_unknown_namespace() {
        let validator = ImportValidator::new();
        let imported = ImportedMemory::new("Content").with_namespace("unknown-ns");

        let result = validator.validate(&imported);
        assert!(result.is_valid); // Warning, not error
        assert!(result
            .issues
            .iter()
            .any(|i| i.severity == ValidationSeverity::Warning));
    }

    #[test]
    fn test_to_capture_request() {
        let validator = ImportValidator::new()
            .with_default_namespace(Namespace::Learnings);

        let imported = ImportedMemory::new("Test content")
            .with_tag("rust")
            .with_source("test.rs");

        let request = validator.to_capture_request(imported);
        assert_eq!(request.content, "Test content");
        assert_eq!(request.namespace, Namespace::Learnings);
        assert_eq!(request.tags, vec!["rust"]);
        assert_eq!(request.source, Some("test.rs".to_string()));
    }

    #[test]
    fn test_content_hash_tag() {
        let validator = ImportValidator::new();
        let imported = ImportedMemory::new("Test content");

        let tag = validator.content_hash_tag(&imported);
        assert!(tag.starts_with("hash:sha256:"));
    }
}
