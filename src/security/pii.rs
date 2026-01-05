//! PII detection.
// Allow expect() on static regex patterns - these are guaranteed to compile
#![allow(clippy::expect_used)]
//!
//! Detects personally identifiable information in content.

use regex::Regex;
use std::sync::LazyLock;

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
    regex: &'static LazyLock<Regex>,
}

// Define regex patterns as separate statics
// Note: These patterns are static and guaranteed to compile, so expect() is safe
static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}")
        .expect("static regex: email pattern")
});

static SSN_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").expect("static regex: SSN pattern"));

static PHONE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:\+?1[-.\s]?)?\(?[2-9]\d{2}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b")
        .expect("static regex: phone pattern")
});

static CREDIT_CARD_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b(?:4[0-9]{12}(?:[0-9]{3})?|5[1-5][0-9]{14}|3[47][0-9]{13}|6(?:011|5[0-9]{2})[0-9]{12})\b",
    )
    .expect("static regex: credit card pattern")
});

static IP_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b",
    )
    .expect("static regex: IP address pattern")
});

static DOB_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:dob|date\s*of\s*birth|birth\s*date)\s*[:=]?\s*\d{1,2}[/\-]\d{1,2}[/\-]\d{2,4}\b",
    )
    .expect("static regex: date of birth pattern")
});

static ZIP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{5}(?:-\d{4})?\b").expect("static regex: ZIP code pattern"));

static DL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:driver'?s?\s*license|dl)\s*#?\s*[:=]?\s*[A-Z0-9]{6,12}\b")
        .expect("static regex: driver's license pattern")
});

static PASSPORT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bpassport\s*#?\s*[:=]?\s*[A-Z0-9]{6,9}\b")
        .expect("static regex: passport pattern")
});

// HIGH-SEC: International tax/ID patterns

/// UK National Insurance Number: 2 letters + 6 digits + 1 letter (e.g., AB123456C)
/// Letters may be separated by spaces or dashes
static UK_NIN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-CEGHJ-PR-TW-Z]{2}[\s\-]?\d{2}[\s\-]?\d{2}[\s\-]?\d{2}[\s\-]?[A-D]\b")
        .expect("static regex: UK NIN pattern")
});

/// Canada Social Insurance Number: 9 digits, often XXX-XXX-XXX or XXX XXX XXX
static CA_SIN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b\d{3}[\s\-]?\d{3}[\s\-]?\d{3}\b").expect("static regex: Canada SIN pattern")
});

/// EU VAT Number: Country prefix (2 letters) + country-specific format
/// Common formats: AT + U + 8 digits, BE + 10 digits, DE + 9 digits, etc.
static EU_VAT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:ATU\d{8}|BE[01]\d{9}|DE\d{9}|DK\d{8}|EE\d{9}|EL\d{9}|ES[A-Z]\d{7}[A-Z0-9]|FI\d{8}|FR[A-Z0-9]{2}\d{9}|HR\d{11}|HU\d{8}|IE\d{7}[A-Z]{1,2}|IT\d{11}|LT\d{9,12}|LU\d{8}|LV\d{11}|MT\d{8}|NL\d{9}B\d{2}|PL\d{10}|PT\d{9}|RO\d{2,10}|SE\d{12}|SI\d{8}|SK\d{10}|CY\d{8}[A-Z]|CZ\d{8,10}|BG\d{9,10})\b",
    )
    .expect("static regex: EU VAT pattern")
});

/// Australian Tax File Number (TFN): 8-9 digits
static AU_TFN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:tfn|tax\s*file\s*number)\s*[:=]?\s*\d{3}[\s\-]?\d{3}[\s\-]?\d{2,3}\b")
        .expect("static regex: Australia TFN pattern")
});

/// Indian Aadhaar Number: 12 digits, often formatted as XXXX XXXX XXXX
static IN_AADHAAR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[2-9]\d{3}[\s\-]?\d{4}[\s\-]?\d{4}\b")
        .expect("static regex: India Aadhaar pattern")
});

/// Indian PAN (Permanent Account Number): 5 letters + 4 digits + 1 letter
static IN_PAN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Z]{5}\d{4}[A-Z]\b").expect("static regex: India PAN pattern")
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
        // International tax/ID patterns
        PiiPattern {
            name: "UK National Insurance Number",
            regex: &UK_NIN_REGEX,
        },
        PiiPattern {
            name: "Canada SIN",
            regex: &CA_SIN_REGEX,
        },
        PiiPattern {
            name: "EU VAT Number",
            regex: &EU_VAT_REGEX,
        },
        PiiPattern {
            name: "Australia TFN",
            regex: &AU_TFN_REGEX,
        },
        PiiPattern {
            name: "India Aadhaar",
            regex: &IN_AADHAAR_REGEX,
        },
        PiiPattern {
            name: "India PAN",
            regex: &IN_PAN_REGEX,
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
        let mut found_matches = Vec::new();

        for pattern in pii_patterns() {
            self.collect_pattern_matches(pattern.name, pattern.regex, content, &mut found_matches);
        }

        // Sort by position
        found_matches.sort_by_key(|m| m.start);

        // Remove overlapping matches
        deduplicate_overlapping(found_matches)
    }

    /// Collects matches for a single pattern into the result vector.
    fn collect_pattern_matches(
        &self,
        pattern_name: &str,
        regex: &Regex,
        content: &str,
        matches: &mut Vec<PiiMatch>,
    ) {
        for m in regex.find_iter(content) {
            if let Some(pii_match) = self.process_match(pattern_name, &m, content) {
                matches.push(pii_match);
            }
        }
    }

    /// Processes a match and returns a `PiiMatch` if it should be included.
    fn process_match(
        &self,
        pattern_name: &str,
        m: &regex::Match<'_>,
        content: &str,
    ) -> Option<PiiMatch> {
        let match_str = m.as_str();

        // Skip local IP addresses if configured
        if self.skip_local && pattern_name == "IP Address" && is_local_ip(match_str) {
            return None;
        }

        // Skip common non-PII ZIP codes (very short, likely not actual addresses)
        if pattern_name == "ZIP Code"
            && match_str.len() == 5
            && !is_zip_in_address_context(content, m.start())
        {
            return None;
        }

        Some(PiiMatch {
            pii_type: pattern_name.to_string(),
            start: m.start(),
            end: m.end(),
            matched_text: match_str.to_string(),
        })
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

/// Removes overlapping matches, keeping the first occurrence.
fn deduplicate_overlapping(sorted_matches: Vec<PiiMatch>) -> Vec<PiiMatch> {
    let mut result = Vec::new();
    let mut last_end = 0;

    for m in sorted_matches {
        if m.start >= last_end {
            last_end = m.end;
            result.push(m);
        }
    }

    result
}

/// Checks if an IP address is a local/private address.
fn is_local_ip(ip: &str) -> bool {
    ip.starts_with("127.")
        || ip.starts_with("10.")
        || ip.starts_with("192.168.")
        || ip.starts_with("172.16.")
        || ip == "0.0.0.0"
}

/// Checks if a ZIP code appears in an address context.
fn is_zip_in_address_context(content: &str, match_start: usize) -> bool {
    let before = if match_start >= 20 {
        &content[match_start - 20..match_start]
    } else {
        &content[..match_start]
    };
    let before_lower = before.to_lowercase();
    before_lower.contains("address") || before_lower.contains("zip") || before.contains(',')
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

    // ============================================================================
    // International Tax/ID Pattern Tests
    // ============================================================================

    #[test]
    fn test_detect_uk_nin() {
        let detector = PiiDetector::new();
        // Valid UK NIN format: 2 letters + 6 digits + 1 letter
        let content = "NIN: AB123456C";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(
            matches
                .iter()
                .any(|m| m.pii_type == "UK National Insurance Number")
        );
    }

    #[test]
    fn test_detect_uk_nin_with_spaces() {
        let detector = PiiDetector::new();
        let content = "National Insurance: AB 12 34 56 C";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(
            matches
                .iter()
                .any(|m| m.pii_type == "UK National Insurance Number")
        );
    }

    #[test]
    fn test_detect_canada_sin() {
        let detector = PiiDetector::new();
        // Canadian SIN: 9 digits, often XXX-XXX-XXX
        let content = "SIN: 123-456-789";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.pii_type == "Canada SIN"));
    }

    #[test]
    fn test_detect_canada_sin_no_dashes() {
        let detector = PiiDetector::new();
        let content = "SIN: 123456789";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.pii_type == "Canada SIN"));
    }

    #[test]
    fn test_detect_eu_vat_german() {
        let detector = PiiDetector::new();
        // German VAT: DE + 9 digits
        let content = "VAT: DE123456789";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.pii_type == "EU VAT Number"));
    }

    #[test]
    fn test_detect_eu_vat_french() {
        let detector = PiiDetector::new();
        // French VAT: FR + 2 chars + 9 digits
        let content = "VAT: FR12123456789";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.pii_type == "EU VAT Number"));
    }

    #[test]
    fn test_detect_eu_vat_dutch() {
        let detector = PiiDetector::new();
        // Dutch VAT: NL + 9 digits + B + 2 digits
        let content = "VAT: NL123456789B01";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.pii_type == "EU VAT Number"));
    }

    #[test]
    fn test_detect_australia_tfn() {
        let detector = PiiDetector::new();
        // Australian TFN: 8-9 digits with context
        let content = "TFN: 123-456-789";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.pii_type == "Australia TFN"));
    }

    #[test]
    fn test_detect_india_aadhaar() {
        let detector = PiiDetector::new();
        // Aadhaar: 12 digits, first digit 2-9
        let content = "Aadhaar: 2345 6789 0123";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.pii_type == "India Aadhaar"));
    }

    #[test]
    fn test_detect_india_pan() {
        let detector = PiiDetector::new();
        // PAN: 5 letters + 4 digits + 1 letter
        let content = "PAN: ABCDE1234F";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.pii_type == "India PAN"));
    }

    #[test]
    fn test_international_ids_case_insensitive() {
        let detector = PiiDetector::new();

        // EU VAT lowercase
        let content = "vat: de123456789";
        let matches = detector.detect(content);
        assert!(matches.iter().any(|m| m.pii_type == "EU VAT Number"));
    }
}
