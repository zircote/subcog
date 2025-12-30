//! PII detection.
//!
//! Detects personally identifiable information in content.

use once_cell::sync::Lazy;
use regex::Regex;

/// A detected PII match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiiMatch {
    /// Type of PII detected.
    pub pii_type: String,
    /// Start position in content.
    pub start: usize,
    /// End position in content.
    pub end: usize,
    /// The matched text.
    pub matched_text: String,
}

/// Pattern for detecting PII.
struct PiiPattern {
    name: &'static str,
    regex: &'static Lazy<Regex>,
}

// Define regex patterns as separate statics
static EMAIL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap_or_else(|_| Regex::new(".^").unwrap())
});

static SSN_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap_or_else(|_| Regex::new(".^").unwrap())
});

static PHONE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:\+?1[-.\s]?)?\(?[2-9]\d{2}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b").unwrap_or_else(|_| Regex::new(".^").unwrap())
});

static CREDIT_CARD_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:4[0-9]{12}(?:[0-9]{3})?|5[1-5][0-9]{14}|3[47][0-9]{13}|6(?:011|5[0-9]{2})[0-9]{12})\b").unwrap_or_else(|_| Regex::new(".^").unwrap())
});

static IP_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b").unwrap_or_else(|_| Regex::new(".^").unwrap())
});

static DOB_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(?:dob|date\s*of\s*birth|birth\s*date)\s*[:=]?\s*\d{1,2}[/\-]\d{1,2}[/\-]\d{2,4}\b").unwrap_or_else(|_| Regex::new(".^").unwrap())
});

static ZIP_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b\d{5}(?:-\d{4})?\b").unwrap_or_else(|_| Regex::new(".^").unwrap())
});

static DL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(?:driver'?s?\s*license|dl)\s*#?\s*[:=]?\s*[A-Z0-9]{6,12}\b").unwrap_or_else(|_| Regex::new(".^").unwrap())
});

static PASSPORT_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\bpassport\s*#?\s*[:=]?\s*[A-Z0-9]{6,9}\b").unwrap_or_else(|_| Regex::new(".^").unwrap())
});

/// Returns the list of PII patterns to check.
fn pii_patterns() -> Vec<PiiPattern> {
    vec![
        PiiPattern {
            name: "Email Address",
            regex: &EMAIL_REGEX,
        },
        PiiPattern {
            name: "SSN",
            regex: &SSN_REGEX,
        },
        PiiPattern {
            name: "Phone Number",
            regex: &PHONE_REGEX,
        },
        PiiPattern {
            name: "Credit Card Number",
            regex: &CREDIT_CARD_REGEX,
        },
        PiiPattern {
            name: "IP Address",
            regex: &IP_REGEX,
        },
        PiiPattern {
            name: "Date of Birth",
            regex: &DOB_REGEX,
        },
        PiiPattern {
            name: "ZIP Code",
            regex: &ZIP_REGEX,
        },
        PiiPattern {
            name: "Driver's License",
            regex: &DL_REGEX,
        },
        PiiPattern {
            name: "Passport Number",
            regex: &PASSPORT_REGEX,
        },
    ]
}

/// Detector for personally identifiable information.
pub struct PiiDetector {
    /// Skip common non-PII patterns (like local IPs).
    skip_local: bool,
}

impl PiiDetector {
    /// Creates a new PII detector.
    #[must_use]
    pub const fn new() -> Self {
        Self { skip_local: true }
    }

    /// Disables skipping of local/non-sensitive patterns.
    #[must_use]
    pub const fn include_local(mut self) -> Self {
        self.skip_local = false;
        self
    }

    /// Checks if content contains PII.
    #[must_use]
    pub fn contains_pii(&self, content: &str) -> bool {
        !self.detect(content).is_empty()
    }

    /// Returns all detected PII matches.
    #[must_use]
    pub fn detect(&self, content: &str) -> Vec<PiiMatch> {
        let mut matches = Vec::new();

        for pattern in pii_patterns() {
            for m in pattern.regex.find_iter(content) {
                let matched = m.as_str();

                // Skip local IP addresses if configured
                if self.skip_local && pattern.name == "IP Address" {
                    if matched.starts_with("127.")
                        || matched.starts_with("10.")
                        || matched.starts_with("192.168.")
                        || matched.starts_with("172.16.")
                        || matched == "0.0.0.0"
                    {
                        continue;
                    }
                }

                // Skip common non-PII ZIP codes (very short, likely not actual addresses)
                if pattern.name == "ZIP Code" && matched.len() == 5 {
                    // Only flag ZIP codes if they appear in an address context
                    let before = if m.start() >= 20 {
                        &content[m.start() - 20..m.start()]
                    } else {
                        &content[..m.start()]
                    };
                    if !before.to_lowercase().contains("address")
                        && !before.to_lowercase().contains("zip")
                        && !before.contains(',')
                    {
                        continue;
                    }
                }

                matches.push(PiiMatch {
                    pii_type: pattern.name.to_string(),
                    start: m.start(),
                    end: m.end(),
                    matched_text: matched.to_string(),
                });
            }
        }

        // Sort by position
        matches.sort_by_key(|m| m.start);

        // Remove overlapping matches
        let mut result = Vec::new();
        let mut last_end = 0;
        for m in matches {
            if m.start >= last_end {
                last_end = m.end;
                result.push(m);
            }
        }

        result
    }

    /// Returns the types of PII detected.
    #[must_use]
    pub fn detect_types(&self, content: &str) -> Vec<String> {
        self.detect(content)
            .into_iter()
            .map(|m| m.pii_type)
            .collect()
    }

    /// Returns the count of PII detected.
    #[must_use]
    pub fn count(&self, content: &str) -> usize {
        self.detect(content).len()
    }
}

impl Default for PiiDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_email() {
        let detector = PiiDetector::new();
        let content = "Contact me at john.doe@example.com";
        let matches = detector.detect(content);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pii_type, "Email Address");
        assert_eq!(matches[0].matched_text, "john.doe@example.com");
    }

    #[test]
    fn test_detect_ssn() {
        let detector = PiiDetector::new();
        let content = "SSN: 123-45-6789";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.pii_type == "SSN"));
    }

    #[test]
    fn test_detect_phone() {
        let detector = PiiDetector::new();
        let content = "Call me at (555) 123-4567";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.pii_type == "Phone Number"));
    }

    #[test]
    fn test_detect_credit_card() {
        let detector = PiiDetector::new();
        // Visa test number
        let content = "Card: 4111111111111111";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.pii_type == "Credit Card Number"));
    }

    #[test]
    fn test_detect_ip_address() {
        let detector = PiiDetector::new();
        let content = "Server IP: 203.0.113.42";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.pii_type == "IP Address"));
    }

    #[test]
    fn test_skip_local_ip() {
        let detector = PiiDetector::new();
        let content = "Localhost: 127.0.0.1";
        let matches = detector.detect(content);

        assert!(matches.iter().all(|m| m.pii_type != "IP Address"));
    }

    #[test]
    fn test_include_local_ip() {
        let detector = PiiDetector::new().include_local();
        let content = "Localhost: 127.0.0.1";
        let matches = detector.detect(content);

        assert!(matches.iter().any(|m| m.pii_type == "IP Address"));
    }

    #[test]
    fn test_no_pii() {
        let detector = PiiDetector::new();
        let content = "This is just regular text without PII.";
        assert!(!detector.contains_pii(content));
    }

    #[test]
    fn test_multiple_pii() {
        let detector = PiiDetector::new();
        let content = "Email: test@example.com, Phone: 555-123-4567";
        let matches = detector.detect(content);

        assert!(matches.len() >= 2);
    }

    #[test]
    fn test_detect_types() {
        let detector = PiiDetector::new();
        let content = "test@example.com";
        let types = detector.detect_types(content);

        assert!(types.contains(&"Email Address".to_string()));
    }
}
