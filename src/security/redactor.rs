//! Content redaction.
//!
//! Redacts sensitive content (secrets and PII) from text.

use super::audit::global_logger;
use super::{PiiDetector, SecretDetector};

/// Redaction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RedactionMode {
    /// Replace with `[REDACTED]`.
    #[default]
    Mask,
    /// Replace with type-specific placeholder (e.g., `SECRET:AWS_KEY`).
    TypedMask,
    /// Replace with asterisks of same length.
    Asterisks,
    /// Remove entirely.
    Remove,
}

/// Configuration for content redaction.
#[derive(Debug, Clone)]
pub struct RedactionConfig {
    /// Redaction mode.
    pub mode: RedactionMode,
    /// Redact secrets.
    pub redact_secrets: bool,
    /// Redact PII.
    pub redact_pii: bool,
    /// Placeholder for redacted content.
    pub placeholder: String,
}

impl Default for RedactionConfig {
    fn default() -> Self {
        Self {
            mode: RedactionMode::Mask,
            redact_secrets: true,
            redact_pii: false,
            placeholder: "[REDACTED]".to_string(),
        }
    }
}

impl RedactionConfig {
    /// Creates a new redaction config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the redaction mode.
    #[must_use]
    pub const fn with_mode(mut self, mode: RedactionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Enables PII redaction.
    #[must_use]
    pub const fn with_pii(mut self) -> Self {
        self.redact_pii = true;
        self
    }

    /// Disables secret redaction.
    #[must_use]
    pub const fn without_secrets(mut self) -> Self {
        self.redact_secrets = false;
        self
    }

    /// Sets a custom placeholder.
    #[must_use]
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }
}

/// Redacts sensitive content from text.
pub struct ContentRedactor {
    secret_detector: SecretDetector,
    pii_detector: PiiDetector,
    config: RedactionConfig,
}

impl ContentRedactor {
    /// Creates a new content redactor with default config.
    #[must_use]
    pub fn new() -> Self {
        Self {
            secret_detector: SecretDetector::new(),
            pii_detector: PiiDetector::new(),
            config: RedactionConfig::default(),
        }
    }

    /// Creates a new content redactor with custom config.
    #[must_use]
    pub const fn with_config(config: RedactionConfig) -> Self {
        Self {
            secret_detector: SecretDetector::new(),
            pii_detector: PiiDetector::new(),
            config,
        }
    }

    /// Returns the configuration.
    #[must_use]
    pub const fn config(&self) -> &RedactionConfig {
        &self.config
    }

    /// Redacts sensitive content, returning the redacted text.
    #[must_use]
    pub fn redact(&self, content: &str) -> String {
        // Collect all matches to redact
        let mut ranges: Vec<(usize, usize, String)> = Vec::new();

        if self.config.redact_secrets {
            for m in self.secret_detector.detect(content) {
                let replacement = self.get_replacement(&m.secret_type, m.end - m.start);
                ranges.push((m.start, m.end, replacement));
            }
        }

        if self.config.redact_pii {
            let pii_matches = self.pii_detector.detect(content);

            // Log PII detection for audit (GDPR/SOC2 compliance)
            self.log_pii_detection_if_any(&pii_matches);

            for m in pii_matches {
                let replacement = self.get_replacement(&m.pii_type, m.end - m.start);
                ranges.push((m.start, m.end, replacement));
            }
        }

        // Sort by start position (reverse order for replacement)
        ranges.sort_by(|a, b| b.0.cmp(&a.0));

        // Remove overlapping ranges (keep earliest)
        let mut filtered: Vec<(usize, usize, String)> = Vec::new();
        for range in ranges {
            if filtered.iter().all(|f| range.1 <= f.0 || range.0 >= f.1) {
                filtered.push(range);
            }
        }

        // Apply replacements (in reverse order to preserve positions)
        let mut result = content.to_string();
        for (start, end, replacement) in filtered {
            result.replace_range(start..end, &replacement);
        }

        result
    }

    /// Returns the redacted content and a flag indicating if anything was redacted.
    #[must_use]
    pub fn redact_with_flag(&self, content: &str) -> (String, bool) {
        let redacted = self.redact(content);
        let was_redacted = redacted != content;
        (redacted, was_redacted)
    }

    /// Checks if content needs redaction.
    #[must_use]
    pub fn needs_redaction(&self, content: &str) -> bool {
        if self.config.redact_secrets && self.secret_detector.contains_secrets(content) {
            return true;
        }
        if self.config.redact_pii && self.pii_detector.contains_pii(content) {
            return true;
        }
        false
    }

    /// Returns the types of sensitive content found.
    #[must_use]
    pub fn detected_types(&self, content: &str) -> Vec<String> {
        let mut types = Vec::new();

        if self.config.redact_secrets {
            types.extend(self.secret_detector.detect_types(content));
        }

        if self.config.redact_pii {
            types.extend(self.pii_detector.detect_types(content));
        }

        types
    }

    /// Logs PII detection events for audit compliance (GDPR/SOC2).
    ///
    /// Only logs when matches are found to avoid noise.
    fn log_pii_detection_if_any(&self, pii_matches: &[super::PiiMatch]) {
        if !pii_matches.is_empty() {
            if let Some(logger) = global_logger() {
                let pii_types: Vec<&str> =
                    pii_matches.iter().map(|m| m.pii_type.as_str()).collect();
                let mut entry = super::audit::AuditEntry::new("security", "pii_detected");
                entry.metadata = serde_json::json!({
                    "pii_count": pii_matches.len(),
                    "pii_types": pii_types,
                });
                logger.log_entry(entry);
            }
        }
    }

    /// Gets the replacement string based on mode.
    fn get_replacement(&self, type_name: &str, length: usize) -> String {
        match self.config.mode {
            RedactionMode::Mask => self.config.placeholder.clone(),
            RedactionMode::TypedMask => {
                format!("[REDACTED:{}]", type_name.to_uppercase().replace(' ', "_"))
            },
            RedactionMode::Asterisks => "*".repeat(length),
            RedactionMode::Remove => String::new(),
        }
    }
}

impl Default for ContentRedactor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_aws_key() {
        let redactor = ContentRedactor::new();
        let content = "AWS_KEY=AKIAIOSFODNN7EXAMPLE";
        let redacted = redactor.redact(content);

        assert!(!redacted.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn test_redact_multiple_secrets() {
        let redactor = ContentRedactor::new();
        let content = "AKIAIOSFODNN7EXAMPLE and ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        let redacted = redactor.redact(content);

        assert!(!redacted.contains("AKIA"));
        assert!(!redacted.contains("ghp_"));
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn test_typed_mask_mode() {
        let config = RedactionConfig::new().with_mode(RedactionMode::TypedMask);
        let redactor = ContentRedactor::with_config(config);
        let content = "Key: AKIAIOSFODNN7EXAMPLE";
        let redacted = redactor.redact(content);

        assert!(redacted.contains("[REDACTED:AWS_ACCESS_KEY_ID]"));
    }

    #[test]
    fn test_asterisks_mode() {
        let config = RedactionConfig::new().with_mode(RedactionMode::Asterisks);
        let redactor = ContentRedactor::with_config(config);
        let content = "Key: AKIAIOSFODNN7EXAMPLE";
        let redacted = redactor.redact(content);

        // The asterisks should be the same length as the matched text
        assert!(redacted.contains("****"));
        assert!(!redacted.contains("AKIA"));
    }

    #[test]
    fn test_remove_mode() {
        let config = RedactionConfig::new().with_mode(RedactionMode::Remove);
        let redactor = ContentRedactor::with_config(config);
        let content = "Key: AKIAIOSFODNN7EXAMPLE here";
        let redacted = redactor.redact(content);

        assert!(!redacted.contains("AKIA"));
        assert!(redacted.contains("Key:  here"));
    }

    #[test]
    fn test_redact_pii() {
        let config = RedactionConfig::new().with_pii();
        let redactor = ContentRedactor::with_config(config);
        let content = "Email: test@example.com";
        let redacted = redactor.redact(content);

        assert!(!redacted.contains("test@example.com"));
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn test_no_redaction_needed() {
        let redactor = ContentRedactor::new();
        let content = "Just regular text";
        let redacted = redactor.redact(content);

        assert_eq!(redacted, content);
    }

    #[test]
    fn test_redact_with_flag() {
        let redactor = ContentRedactor::new();

        let (redacted, was_redacted) = redactor.redact_with_flag("AKIAIOSFODNN7EXAMPLE");
        assert!(was_redacted);
        assert!(redacted.contains("[REDACTED]"));

        let (redacted, was_redacted) = redactor.redact_with_flag("Just text");
        assert!(!was_redacted);
        assert_eq!(redacted, "Just text");
    }

    #[test]
    fn test_needs_redaction() {
        let redactor = ContentRedactor::new();

        assert!(redactor.needs_redaction("AKIAIOSFODNN7EXAMPLE"));
        assert!(!redactor.needs_redaction("Just text"));
    }

    #[test]
    fn test_detected_types() {
        let config = RedactionConfig::new().with_pii();
        let redactor = ContentRedactor::with_config(config);
        let content = "AKIAIOSFODNN7EXAMPLE and test@example.com";
        let types = redactor.detected_types(content);

        assert!(types.contains(&"AWS Access Key ID".to_string()));
        assert!(types.contains(&"Email Address".to_string()));
    }

    #[test]
    fn test_custom_placeholder() {
        let config = RedactionConfig::new().with_placeholder("***HIDDEN***");
        let redactor = ContentRedactor::with_config(config);
        let content = "Key: AKIAIOSFODNN7EXAMPLE";
        let redacted = redactor.redact(content);

        assert!(redacted.contains("***HIDDEN***"));
    }

    #[test]
    fn test_pii_only() {
        let config = RedactionConfig::new().without_secrets().with_pii();
        let redactor = ContentRedactor::with_config(config);
        let content = "AKIAIOSFODNN7EXAMPLE and test@example.com";
        let redacted = redactor.redact(content);

        // Secret should remain, PII should be redacted
        assert!(redacted.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(!redacted.contains("test@example.com"));
    }
}
