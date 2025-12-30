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

static GENERIC_API_KEY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:api[_-]?key|apikey)\s*[=:]\s*['"]?([A-Za-z0-9_\-]{20,})['"]?"#)
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

static BEARER_TOKEN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)bearer\s+[A-Za-z0-9_\-.]+").expect("static regex: bearer token pattern")
});

static OPENAI_API_KEY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"sk-[A-Za-z0-9]{48}").expect("static regex: OpenAI API key pattern")
});

static ANTHROPIC_API_KEY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"sk-ant-api[A-Za-z0-9_-]{90,}").expect("static regex: Anthropic API key pattern")
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
            for m in pattern.regex.find_iter(content) {
                matches.push(SecretMatch {
                    secret_type: pattern.name.to_string(),
                    start: m.start(),
                    end: m.end(),
                    matched_text: m.as_str().to_string(),
                });
            }
        }

        // Sort by position
        matches.sort_by_key(|m| m.start);

        // Remove overlapping matches (keep the first one)
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
        // Test Bearer token detection
        let content = "Authorization: Bearer abc123xyz.token.value";
        let matches = detector.detect(content);

        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.secret_type == "Bearer Token"));
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
}
