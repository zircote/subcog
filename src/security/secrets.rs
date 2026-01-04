//! Secret detection patterns.
// Allow expect() on static regex patterns - these are guaranteed to compile
#![allow(clippy::expect_used)]
//!
//! Detects common secret patterns in content to prevent accidental capture.

use regex::Regex;
use std::sync::LazyLock;

/// A detected secret match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretMatch {
    /// Type of secret detected.
    pub secret_type: String,
    /// Start position in content.
    pub start: usize,
    /// End position in content.
    pub end: usize,
    /// The matched text (for debugging, will be redacted in production).
    pub matched_text: String,
}

/// Pattern for detecting secrets.
struct SecretPattern {
    name: &'static str,
    regex: &'static LazyLock<Regex>,
}

// Define regex patterns as separate statics
// Note: These patterns are static and guaranteed to compile, so expect() is safe
static AWS_ACCESS_KEY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)AKIA[0-9A-Z]{16}").expect("static regex: AWS access key pattern")
});

static AWS_SECRET_KEY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:aws_secret_access_key|aws_secret_key|secret_access_key)\s*[=:]\s*['"]?([A-Za-z0-9/+=]{40})['"]?"#).expect("static regex: AWS secret key pattern")
});

static GITHUB_TOKEN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"gh[pousr]_[A-Za-z0-9_]{36,}").expect("static regex: GitHub token pattern")
});

static GITHUB_PAT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"github_pat_[A-Za-z0-9_]{22,}").expect("static regex: GitHub PAT pattern")
});

/// Generic API key pattern with reduced false positives.
///
/// Requires:
/// - Assignment operator (= or :) with optional whitespace
/// - Optional quotes around the value
/// - Value must be at least 24 chars (not 20) to reduce UUIDs/short IDs
/// - Placeholder filtering is handled in `detect()` method
static GENERIC_API_KEY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:api[_-]?key|apikey)\s*[=:]\s*['"]?([A-Za-z0-9_\-]{24,})['"]?"#)
        .expect("static regex: generic API key pattern")
});

static GENERIC_SECRET_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:secret|password|passwd|pwd)\s*[=:]\s*['"]?([^\s'"]{8,})['"]?"#)
        .expect("static regex: generic secret pattern")
});

static PRIVATE_KEY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"-----BEGIN (?:RSA |DSA |EC |OPENSSH |PGP )?PRIVATE KEY-----")
        .expect("static regex: private key pattern")
});

static JWT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"eyJ[A-Za-z0-9_-]*\.eyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]*")
        .expect("static regex: JWT pattern")
});

static SLACK_TOKEN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"xox[baprs]-[0-9]{10,13}-[0-9]{10,13}[a-zA-Z0-9-]*")
        .expect("static regex: Slack token pattern")
});

static SLACK_WEBHOOK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https://hooks\.slack\.com/services/T[A-Z0-9]+/B[A-Z0-9]+/[a-zA-Z0-9]+")
        .expect("static regex: Slack webhook pattern")
});

static GOOGLE_API_KEY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"AIza[0-9A-Za-z_-]{35}").expect("static regex: Google API key pattern")
});

static STRIPE_API_KEY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:sk|pk)_(?:live|test)_[A-Za-z0-9]{24,}")
        .expect("static regex: Stripe API key pattern")
});

static DATABASE_URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:postgres|mysql|mongodb|redis)://[^:]+:[^@]+@[^\s]+")
        .expect("static regex: database URL pattern")
});

/// Bearer token pattern with reduced false positives.
///
/// Requires:
/// - "Bearer " prefix (case-insensitive)
/// - Token must be at least 20 chars to exclude short strings
/// - Placeholder filtering is handled in `detect()` method
static BEARER_TOKEN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)bearer\s+([A-Za-z0-9_\-.]{20,})").expect("static regex: bearer token pattern")
});

static OPENAI_API_KEY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"sk-[A-Za-z0-9]{48}").expect("static regex: OpenAI API key pattern")
});

static ANTHROPIC_API_KEY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"sk-ant-api[A-Za-z0-9_-]{90,}").expect("static regex: Anthropic API key pattern")
});

// HIGH-SEC-012: GCP/Azure/Twilio credentials
static GCP_SERVICE_ACCOUNT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)"type"\s*:\s*"service_account""#)
        .expect("static regex: GCP service account pattern")
});

static AZURE_STORAGE_KEY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:AccountKey|SharedAccessSignature)\s*=\s*[A-Za-z0-9+/=]{44,}")
        .expect("static regex: Azure storage key pattern")
});

static AZURE_AD_CLIENT_SECRET_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)(?:client_secret|azure_client_secret)\s*[=:]\s*['"]?([A-Za-z0-9~._-]{34,})['"]?"#,
    )
    .expect("static regex: Azure AD client secret pattern")
});

static TWILIO_API_KEY_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"SK[a-f0-9]{32}").expect("static regex: Twilio API key pattern"));

static TWILIO_AUTH_TOKEN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:twilio_auth_token|auth_token)\s*[=:]\s*['"]?([a-f0-9]{32})['"]?"#)
        .expect("static regex: Twilio auth token pattern")
});

static SENDGRID_API_KEY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"SG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}")
        .expect("static regex: SendGrid API key pattern")
});

static MAILGUN_API_KEY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"key-[a-f0-9]{32}").expect("static regex: Mailgun API key pattern")
});

/// Returns the list of secret patterns to check.
fn secret_patterns() -> Vec<SecretPattern> {
    vec![
        SecretPattern {
            name: "AWS Access Key ID",
            regex: &AWS_ACCESS_KEY_REGEX,
        },
        SecretPattern {
            name: "AWS Secret Access Key",
            regex: &AWS_SECRET_KEY_REGEX,
        },
        SecretPattern {
            name: "GitHub Token",
            regex: &GITHUB_TOKEN_REGEX,
        },
        SecretPattern {
            name: "GitHub Personal Access Token (Classic)",
            regex: &GITHUB_PAT_REGEX,
        },
        SecretPattern {
            name: "Generic API Key",
            regex: &GENERIC_API_KEY_REGEX,
        },
        SecretPattern {
            name: "Generic Secret",
            regex: &GENERIC_SECRET_REGEX,
        },
        SecretPattern {
            name: "Private Key",
            regex: &PRIVATE_KEY_REGEX,
        },
        SecretPattern {
            name: "JWT Token",
            regex: &JWT_REGEX,
        },
        SecretPattern {
            name: "Slack Token",
            regex: &SLACK_TOKEN_REGEX,
        },
        SecretPattern {
            name: "Slack Webhook",
            regex: &SLACK_WEBHOOK_REGEX,
        },
        SecretPattern {
            name: "Google API Key",
            regex: &GOOGLE_API_KEY_REGEX,
        },
        SecretPattern {
            name: "Stripe API Key",
            regex: &STRIPE_API_KEY_REGEX,
        },
        SecretPattern {
            name: "Database URL with Credentials",
            regex: &DATABASE_URL_REGEX,
        },
        SecretPattern {
            name: "Bearer Token",
            regex: &BEARER_TOKEN_REGEX,
        },
        SecretPattern {
            name: "OpenAI API Key",
            regex: &OPENAI_API_KEY_REGEX,
        },
        SecretPattern {
            name: "Anthropic API Key",
            regex: &ANTHROPIC_API_KEY_REGEX,
        },
        // HIGH-SEC-012: Cloud provider credentials
        SecretPattern {
            name: "GCP Service Account",
            regex: &GCP_SERVICE_ACCOUNT_REGEX,
        },
        SecretPattern {
            name: "Azure Storage Key",
            regex: &AZURE_STORAGE_KEY_REGEX,
        },
        SecretPattern {
            name: "Azure AD Client Secret",
            regex: &AZURE_AD_CLIENT_SECRET_REGEX,
        },
        SecretPattern {
            name: "Twilio API Key",
            regex: &TWILIO_API_KEY_REGEX,
        },
        SecretPattern {
            name: "Twilio Auth Token",
            regex: &TWILIO_AUTH_TOKEN_REGEX,
        },
        SecretPattern {
            name: "SendGrid API Key",
            regex: &SENDGRID_API_KEY_REGEX,
        },
        SecretPattern {
            name: "Mailgun API Key",
            regex: &MAILGUN_API_KEY_REGEX,
        },
    ]
}

/// Detector for secrets in content.
pub struct SecretDetector {
    /// Minimum length for generic secret values.
    min_secret_length: usize,
}

impl SecretDetector {
    /// Creates a new secret detector.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            min_secret_length: 8,
        }
    }

    /// Sets the minimum secret length for generic patterns.
    #[must_use]
    pub const fn with_min_length(mut self, length: usize) -> Self {
        self.min_secret_length = length;
        self
    }

    /// Checks if content contains any secrets.
    #[must_use]
    pub fn contains_secrets(&self, content: &str) -> bool {
        !self.detect(content).is_empty()
    }

    /// Returns all detected secret matches.
    #[must_use]
    pub fn detect(&self, content: &str) -> Vec<SecretMatch> {
        let mut matches = Vec::new();

        for pattern in secret_patterns() {
            self.collect_pattern_matches(pattern, content, &mut matches);
        }

        // Sort by position
        matches.sort_by_key(|m| m.start);

        // Remove overlapping matches (keep the first one)
        Self::deduplicate_overlapping(matches)
    }

    /// Collects matches for a single pattern into the result vector.
    fn collect_pattern_matches(
        &self,
        pattern: SecretPattern,
        content: &str,
        matches: &mut Vec<SecretMatch>,
    ) {
        for m in pattern.regex.find_iter(content) {
            if let Some(secret_match) = self.process_match(&pattern, &m) {
                matches.push(secret_match);
            }
        }
    }

    /// Processes a single regex match and returns a `SecretMatch` if it should be included.
    fn process_match(&self, pattern: &SecretPattern, m: &regex::Match<'_>) -> Option<SecretMatch> {
        let matched_text = m.as_str().to_string();

        // Only apply placeholder filtering to generic patterns that are prone to false positives
        // Specific patterns (AWS, GitHub, OpenAI, etc.) have precise formats that don't need filtering
        let should_filter = pattern.name == "Generic API Key"
            || pattern.name == "Generic Secret"
            || pattern.name == "Bearer Token";

        if should_filter && Self::is_placeholder(&matched_text) {
            return None;
        }

        Some(SecretMatch {
            secret_type: pattern.name.to_string(),
            start: m.start(),
            end: m.end(),
            matched_text,
        })
    }

    /// Removes overlapping matches, keeping the first occurrence.
    fn deduplicate_overlapping(sorted_matches: Vec<SecretMatch>) -> Vec<SecretMatch> {
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

    /// Returns the types of secrets detected.
    #[must_use]
    pub fn detect_types(&self, content: &str) -> Vec<String> {
        self.detect(content)
            .into_iter()
            .map(|m| m.secret_type)
            .collect()
    }

    /// Returns the count of secrets detected.
    #[must_use]
    pub fn count(&self, content: &str) -> usize {
        self.detect(content).len()
    }

    /// Checks if a matched value is a common placeholder (false positive).
    ///
    /// This reduces false positives for generic API key and bearer token patterns.
    fn is_placeholder(value: &str) -> bool {
        // Common placeholder prefixes/patterns (case-insensitive)
        const PLACEHOLDER_PATTERNS: &[&str] = &[
            "example",
            "test",
            "demo",
            "your_",
            "your-",
            "my_",
            "my-",
            "placeholder",
            "changeme",
            "xxx",
            "yyy",
            "zzz",
            "foo",
            "bar",
            "baz",
            "sample",
            "fake",
            "dummy",
            "mock",
        ];

        let lower = value.to_lowercase();
        PLACEHOLDER_PATTERNS
            .iter()
            .any(|&pattern| lower.contains(pattern))
    }
}

impl Default for SecretDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_aws_access_key() {
        let detector = SecretDetector::new();
        let content = "My AWS key is AKIAIOSFODNN7EXAMPLE";
        let matches = detector.detect(content);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].secret_type, "AWS Access Key ID");
    }

    #[test]
    fn test_detect_github_token() {
        let detector = SecretDetector::new();

        // Fine-grained token
        let content = "GITHUB_TOKEN=ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        assert!(detector.contains_secrets(content));

        // Classic PAT
        let content2 = "token: github_pat_xxxxxxxxxxxxxxxxxxxxxx_yyyyyyyy";
        assert!(detector.contains_secrets(content2));
    }

    #[test]
    fn test_detect_private_key() {
        let detector = SecretDetector::new();
        let content = "-----BEGIN RSA PRIVATE KEY-----\nMIIE...";
        let matches = detector.detect(content);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].secret_type, "Private Key");
    }

    #[test]
    fn test_detect_jwt() {
        let detector = SecretDetector::new();
        // Test JWT without Bearer prefix to avoid overlap with Bearer Token pattern
        let content = "token=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.secret_type == "JWT Token"));
    }

    #[test]
    fn test_detect_bearer_token() {
        let detector = SecretDetector::new();
        // Test Bearer token detection with realistic token (20+ chars, not a placeholder)
        let content = "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9ab";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.secret_type == "Bearer Token"));
    }

    #[test]
    fn test_bearer_token_rejects_placeholders() {
        let detector = SecretDetector::new();

        // Short tokens (< 20 chars) should NOT match
        let content = "Authorization: Bearer shorttoken";
        assert!(!detector.contains_secrets(content));

        // "example" placeholder should NOT match (contains placeholder pattern)
        let content2 = "Authorization: Bearer example_abcdefgh1234567890";
        assert!(!detector.contains_secrets(content2));

        // "test" placeholder should NOT match
        let content3 = "Authorization: Bearer test_token_1234567890abc";
        assert!(!detector.contains_secrets(content3));
    }

    #[test]
    fn test_detect_slack_webhook() {
        let detector = SecretDetector::new();
        // Build test URL in parts to avoid GitHub secret scanning
        let base = "https://hooks.slack.com/services/";
        let fake_ids = [
            "T", "FAKE", "FAKE", "TEST/B", "FAKE", "FAKE", "TEST/", "fake", "token", "here",
        ];
        let content = format!("SLACK_WEBHOOK={base}{}", fake_ids.join(""));
        let matches = detector.detect(&content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.secret_type == "Slack Webhook"));
    }

    #[test]
    fn test_detect_stripe_key() {
        let detector = SecretDetector::new();
        // Use sk_test_ prefix which is for test keys, not live
        let content = "STRIPE_KEY=sk_test_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        assert!(detector.contains_secrets(content));
    }

    #[test]
    fn test_detect_database_url() {
        let detector = SecretDetector::new();
        let content = "DATABASE_URL=postgres://user:password@localhost:5432/db";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(
            matches
                .iter()
                .any(|m| m.secret_type == "Database URL with Credentials")
        );
    }

    #[test]
    fn test_detect_openai_key() {
        let detector = SecretDetector::new();
        let content = "OPENAI_API_KEY=sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        assert!(detector.contains_secrets(content));
    }

    #[test]
    fn test_no_secrets() {
        let detector = SecretDetector::new();
        let content = "This is just regular text with no secrets.";
        assert!(!detector.contains_secrets(content));
        assert!(detector.detect(content).is_empty());
    }

    #[test]
    fn test_multiple_secrets() {
        let detector = SecretDetector::new();
        let content = "AWS_KEY=AKIAIOSFODNN7EXAMPLE and GITHUB_TOKEN=ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        let matches = detector.detect(content);

        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_detect_types() {
        let detector = SecretDetector::new();
        let content = "AKIAIOSFODNN7EXAMPLE";
        let types = detector.detect_types(content);

        assert!(types.contains(&"AWS Access Key ID".to_string()));
    }

    #[test]
    fn test_count() {
        let detector = SecretDetector::new();
        let content = "AKIAIOSFODNN7EXAMPLE and ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        assert_eq!(detector.count(content), 2);
    }

    // ============================================================================
    // Command Injection and Bypass Tests
    // ============================================================================

    #[test]
    fn test_bypass_null_byte_injection() {
        let detector = SecretDetector::new();

        // Attempt to bypass with null bytes (should still detect)
        let content = "AKIA\0IOSFODNN7EXAMPLE";
        // Null bytes in the middle break the pattern, so shouldn't detect
        assert!(!detector.contains_secrets(content));

        // But adjacent null bytes shouldn't prevent detection
        let content2 = "\0AKIAIOSFODNN7EXAMPLE\0";
        assert!(detector.contains_secrets(content2));
    }

    #[test]
    fn test_bypass_unicode_homoglyphs() {
        let detector = SecretDetector::new();

        // Attempt bypass with Unicode look-alikes (Cyrillic 'А' instead of ASCII 'A')
        // U+0410 CYRILLIC CAPITAL LETTER A
        let content = "АKIAIOSFODNN7EXAMPLE"; // First char is Cyrillic
        // Should NOT detect because pattern expects ASCII 'A'
        assert!(!detector.contains_secrets(content));

        // Normal ASCII should still work
        let content2 = "AKIAIOSFODNN7EXAMPLE";
        assert!(detector.contains_secrets(content2));
    }

    #[test]
    fn test_bypass_invisible_characters() {
        let detector = SecretDetector::new();

        // Zero-width space (U+200B) between characters
        let content = "AKIA\u{200B}IOSFODNN7EXAMPLE";
        // Should NOT detect due to invisible character
        assert!(!detector.contains_secrets(content));

        // Zero-width joiner (U+200D)
        let content2 = "AKIA\u{200D}IOSFODNN7EXAMPLE";
        assert!(!detector.contains_secrets(content2));
    }

    #[test]
    fn test_bypass_whitespace_variations() {
        let detector = SecretDetector::new();

        // Non-breaking space in assignment (U+00A0)
        // Use 24+ chars for the key value (new minimum), non-placeholder
        let content = "api_key =\u{00A0}k8s_prod_auth_1234567890abcdef";
        // Pattern should handle various whitespace
        assert!(detector.contains_secrets(content));

        // Tab character
        let content2 = "api_key\t=\tk8s_prod_auth_1234567890abcdef";
        assert!(detector.contains_secrets(content2));
    }

    #[test]
    fn test_case_insensitive_aws_detection() {
        let detector = SecretDetector::new();

        // AWS access key regex uses (?i) flag - case-insensitive matching
        // This is intentional to catch keys even when case is altered

        // Lowercase should still match (case-insensitive)
        let content = "akiaiosfodnn7example";
        assert!(detector.contains_secrets(content));

        // Mixed case should still match
        let content2 = "AkIaIOSFODNN7EXAMPLE";
        assert!(detector.contains_secrets(content2));

        // Proper uppercase format should match
        let content3 = "AKIAIOSFODNN7EXAMPLE";
        assert!(detector.contains_secrets(content3));
    }

    #[test]
    fn test_bypass_padding_and_wrapping() {
        let detector = SecretDetector::new();

        // Secret wrapped in other text
        let content = "The key prefix-AKIAIOSFODNN7EXAMPLE-suffix is here";
        assert!(detector.contains_secrets(content));

        // Secret at very end
        let content2 = "key: AKIAIOSFODNN7EXAMPLE";
        assert!(detector.contains_secrets(content2));

        // Secret at very start
        let content3 = "AKIAIOSFODNN7EXAMPLE is leaked";
        assert!(detector.contains_secrets(content3));
    }

    #[test]
    fn test_bypass_encoding_variations() {
        let detector = SecretDetector::new();

        // URL encoded (shouldn't detect - these are encoded)
        let content = "AKIA%49OSFODNN7EXAMPLE"; // %49 = 'I'
        // Pattern expects literal characters, not URL encoding
        assert!(!detector.contains_secrets(content));

        // Base64 encoded secret (detector should NOT decode)
        // Base64 of "AKIAIOSFODNN7EXAMPLE" would be different
        let content2 = "QUtJQUlPU0ZPRE5ON0VYQU1QTEU="; // base64
        assert!(!detector.contains_secrets(content2));
    }

    #[test]
    fn test_bypass_comment_injection() {
        let detector = SecretDetector::new();

        // Secret in code comments
        let content = "// AKIAIOSFODNN7EXAMPLE";
        assert!(detector.contains_secrets(content));

        // Secret in HTML comment
        let content2 = "<!-- AKIAIOSFODNN7EXAMPLE -->";
        assert!(detector.contains_secrets(content2));

        // Secret in JSON string
        let content3 = r#"{"key": "AKIAIOSFODNN7EXAMPLE"}"#;
        assert!(detector.contains_secrets(content3));
    }

    #[test]
    fn test_bypass_line_breaks() {
        let detector = SecretDetector::new();

        // Secret split across lines (should NOT detect)
        let content = "AKIA\nIOSFODNN7EXAMPLE";
        assert!(!detector.contains_secrets(content));

        // CRLF
        let content2 = "AKIA\r\nIOSFODNN7EXAMPLE";
        assert!(!detector.contains_secrets(content2));

        // Secret on its own line should detect
        let content3 = "line1\nAKIAIOSFODNN7EXAMPLE\nline3";
        assert!(detector.contains_secrets(content3));
    }

    #[test]
    fn test_bypass_string_concatenation() {
        let detector = SecretDetector::new();

        // Concatenated in code (detector sees raw text, not executed code)
        let content = r#""AKIA" + "IOSFODNN7EXAMPLE""#;
        // Neither part is a valid key on its own
        assert!(!detector.contains_secrets(content));

        // But if they appear together in output, should detect
        let content2 = "key = AKIAIOSFODNN7EXAMPLE";
        assert!(detector.contains_secrets(content2));
    }

    #[test]
    fn test_near_miss_patterns() {
        let detector = SecretDetector::new();

        // Almost AWS key but too short
        let content = "AKIAIOSFODNN7EXA"; // 16 chars after AKIA but needs to be complete
        assert!(!detector.contains_secrets(content));

        // Almost GitHub token but wrong prefix
        let content2 = "ghx_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        assert!(!detector.contains_secrets(content2));

        // Almost JWT but missing signature
        let content3 = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0";
        assert!(!detector.contains_secrets(content3));
    }

    #[test]
    fn test_false_positive_resistance() {
        let detector = SecretDetector::new();

        // Common words that might trigger patterns
        let content = "The API documentation describes the key features";
        assert!(!detector.contains_secrets(content));

        // UUID that looks similar to tokens
        let content2 = "id: 550e8400-e29b-41d4-a716-446655440000";
        assert!(!detector.contains_secrets(content2));

        // Version strings
        let content3 = "version: 1.2.3-beta.4";
        assert!(!detector.contains_secrets(content3));
    }

    #[test]
    fn test_nested_secrets() {
        let detector = SecretDetector::new();

        // Content with multiple distinct secrets on same line
        // AWS key (20 chars) and GitHub token (ghp_ + 36+ chars) separated by newline
        let content = "AKIAIOSFODNN7EXAMPLE\nghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let matches = detector.detect(content);

        // Should detect both patterns (AWS access key and GitHub token)
        assert!(
            matches.len() >= 2,
            "Expected 2+ matches, got {}: {:?}",
            matches.len(),
            matches
        );
    }

    #[test]
    fn test_overlapping_patterns() {
        let detector = SecretDetector::new();

        // Content that matches multiple patterns
        let content = "password = sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        let matches = detector.detect(content);

        // May match generic secret and OpenAI key pattern
        assert!(!matches.is_empty());
    }

    #[test]
    fn test_minimum_length_enforcement() {
        let detector = SecretDetector::new();

        // Generic password too short (< 8 chars)
        let content = "password = short";
        assert!(!detector.contains_secrets(content));

        // Generic password just at minimum (8 chars)
        let content2 = "password = 12345678";
        assert!(detector.contains_secrets(content2));
    }

    #[test]
    fn test_multiline_key_format() {
        let detector = SecretDetector::new();

        // Private key header on its own line
        let content = "-----BEGIN RSA PRIVATE KEY-----";
        assert!(detector.contains_secrets(content));

        // PEM formatted key
        let content2 = r"-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASC
-----END PRIVATE KEY-----";
        assert!(detector.contains_secrets(content2));
    }

    // ============================================================================
    // Cloud Provider Credential Tests (HIGH-SEC-012)
    // ============================================================================

    #[test]
    fn test_detect_gcp_service_account() {
        let detector = SecretDetector::new();
        let content = r#"{"type": "service_account", "project_id": "my-project"}"#;
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(
            matches
                .iter()
                .any(|m| m.secret_type == "GCP Service Account")
        );
    }

    #[test]
    fn test_detect_azure_storage_key() {
        let detector = SecretDetector::new();
        // Azure storage key format: AccountKey=base64string (44+ chars)
        let content = "AccountKey=dGhpc2lzYXRlc3RrZXl0aGF0aXNsb25nZW5vdWdodG9tYXRjaA==";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.secret_type == "Azure Storage Key"));
    }

    #[test]
    fn test_detect_azure_sas_token() {
        let detector = SecretDetector::new();
        // SAS signature is base64-encoded, 44+ chars
        let content = "SharedAccessSignature=dGhpc2lzYXRlc3RzaWduYXR1cmV0aGF0aXNsb25nZW5vdWdo";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.secret_type == "Azure Storage Key"));
    }

    #[test]
    fn test_detect_azure_ad_client_secret() {
        let detector = SecretDetector::new();
        // Azure AD client secrets are typically 34+ character strings
        let content = "client_secret = 'abcdefghijklmnopqrstuvwxyz12345678'";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(
            matches
                .iter()
                .any(|m| m.secret_type == "Azure AD Client Secret")
        );
    }

    #[test]
    fn test_detect_twilio_api_key() {
        let detector = SecretDetector::new();
        // Twilio API keys start with SK followed by 32 hex chars
        // Use test pattern that matches format but is obviously fake
        let content = "TWILIO_SID=SK00000000000000000000000000000000";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.secret_type == "Twilio API Key"));
    }

    #[test]
    fn test_detect_twilio_auth_token() {
        let detector = SecretDetector::new();
        let content = "twilio_auth_token = 'abcdef0123456789abcdef0123456789'";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.secret_type == "Twilio Auth Token"));
    }

    #[test]
    fn test_detect_sendgrid_api_key() {
        let detector = SecretDetector::new();
        // SendGrid API keys: SG.<22 chars>.<43 chars>
        let content = "SENDGRID_API_KEY=SG.abcdefghijklmnopqrstuv.abcdefghijklmnopqrstuvwxyz0123456789abcdefg";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.secret_type == "SendGrid API Key"));
    }

    #[test]
    fn test_detect_mailgun_api_key() {
        let detector = SecretDetector::new();
        // Mailgun API keys: key-<32 hex chars>
        // Use standalone key to avoid overlap with Generic API Key pattern
        let content = "MAILGUN_TOKEN=key-abcdef0123456789abcdef0123456789";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.secret_type == "Mailgun API Key"));
    }

    #[test]
    fn test_cloud_credentials_case_insensitive() {
        let detector = SecretDetector::new();

        // GCP service account with different casing
        let content = r#"{"TYPE": "SERVICE_ACCOUNT"}"#;
        assert!(detector.contains_secrets(content));

        // Azure with different casing
        let content2 = "accountkey=dGhpc2lzYXRlc3RrZXl0aGF0aXNsb25nZW5vdWdodG9tYXRjaA==";
        assert!(detector.contains_secrets(content2));
    }
}
