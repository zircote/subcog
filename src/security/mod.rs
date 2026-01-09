//! Security features.
//!
//! Secret detection, PII filtering, content redaction, and audit logging.
//!
//! # Overview
//!
//! This module provides security features for protecting sensitive content:
//!
//! - **Secret Detection**: Identifies API keys, tokens, credentials in content
//! - **PII Detection**: Finds personally identifiable information
//! - **Content Redaction**: Masks or removes sensitive content before storage
//! - **Audit Logging**: SOC2/GDPR-compliant event logging
//!
//! # Redaction Patterns
//!
//! ## Secret Patterns
//!
//! | Pattern | Regex | Example |
//! |---------|-------|---------|
//! | AWS Access Key | `AKIA[0-9A-Z]{16}` | `AKIAIOSFODNN7EXAMPLE` |
//! | AWS Secret Key | `aws_secret_access_key\s*=\s*[A-Za-z0-9/+=]{40}` | `aws_secret_key = wJalrXUtnFEMI...` |
//! | GitHub Token | `gh[pousr]_[A-Za-z0-9_]{36,}` | `ghp_xxxxxxxxxxxx...` |
//! | GitHub PAT | `github_pat_[A-Za-z0-9_]{22,}` | `github_pat_xxxxx...` |
//! | Generic API Key | `api[_-]?key\s*[=:]\s*[A-Za-z0-9_\-]{24,}` | `api_key = sk-xxxxx...` |
//! | Generic Secret | `(?:secret\|password)\s*[=:]\s*[^\s]{8,}` | `password = mypassword` |
//! | Private Key | `-----BEGIN (?:RSA )?PRIVATE KEY-----` | PEM headers |
//! | JWT | `eyJ[...].eyJ[...].[...]` | Base64-encoded JWT |
//! | Slack Token | `xox[baprs]-[0-9]{10,13}-...` | `xoxb-123456789012-...` |
//! | Slack Webhook | `https://hooks.slack.com/services/T.../B.../...` | Webhook URLs |
//! | Google API Key | `AIza[0-9A-Za-z_-]{35}` | `AIzaSyC...` |
//! | Stripe Key | `(?:sk\|pk)_(?:live\|test)_[A-Za-z0-9]{24,}` | `sk_live_xxxxx...` |
//! | Database URL | `(?:postgres\|mysql)://user:pass@host` | Connection strings |
//! | Bearer Token | `bearer\s+[A-Za-z0-9_\-.]{20,}` | `Bearer eyJhbGci...` |
//! | `OpenAI` API Key | `sk-[A-Za-z0-9]{48}` | `sk-xxxxxxxx...` |
//! | Anthropic API Key | `sk-ant-api[A-Za-z0-9_-]{90,}` | `sk-ant-api03-xxx...` |
//!
//! ## PII Patterns
//!
//! | Pattern | Regex | Example |
//! |---------|-------|---------|
//! | Email Address | `[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}` | `user@example.com` |
//! | SSN | `\d{3}-\d{2}-\d{4}` | `123-45-6789` |
//! | Phone Number | `\(?[2-9]\d{2}\)?[-.\s]?\d{3}[-.\s]?\d{4}` | `(555) 123-4567` |
//! | Credit Card | `4[0-9]{12}(?:[0-9]{3})?` (Visa) | `4111111111111111` |
//! | IP Address | `(?:\d{1,3}\.){3}\d{1,3}` | `192.168.1.1` |
//! | Date of Birth | `(?:dob\|birth\s*date)\s*[:=]?\s*\d{1,2}[/-]\d{1,2}[/-]\d{2,4}` | `DOB: 01/15/1990` |
//! | ZIP Code | `\d{5}(?:-\d{4})?` | `90210` |
//! | Driver's License | `(?:driver's?\s*license\|dl)\s*#?\s*[A-Z0-9]{6,12}` | `DL: D12345678` |
//! | Passport | `passport\s*#?\s*[A-Z0-9]{6,9}` | `Passport: AB123456` |
//!
//! ## Redaction Modes
//!
//! | Mode | Output | Example |
//! |------|--------|---------|
//! | `Mask` (default) | `[REDACTED]` | `Key: [REDACTED]` |
//! | `TypedMask` | `[REDACTED:TYPE]` | `Key: [REDACTED:AWS_ACCESS_KEY_ID]` |
//! | `Asterisks` | `****...` | `Key: ********************` |
//! | `Remove` | (empty) | `Key: ` |
//!
//! # Usage
//!
//! ```rust
//! use subcog::security::{ContentRedactor, RedactionConfig, RedactionMode, SecretDetector};
//!
//! // Basic secret detection
//! let detector = SecretDetector::new();
//! assert!(detector.contains_secrets("AKIAIOSFODNN7EXAMPLE"));
//!
//! // Redact secrets with custom mode
//! let config = RedactionConfig::new()
//!     .with_mode(RedactionMode::TypedMask)
//!     .with_pii();  // Also redact PII
//! let redactor = ContentRedactor::with_config(config);
//! let redacted = redactor.redact("Key: AKIAIOSFODNN7EXAMPLE");
//! assert!(redacted.contains("[REDACTED:AWS_ACCESS_KEY_ID]"));
//! ```
//!
//! # False Positive Prevention
//!
//! Generic patterns (API keys, secrets, bearer tokens) include placeholder filtering:
//!
//! ```rust
//! use subcog::security::SecretDetector;
//!
//! let detector = SecretDetector::new();
//! // Placeholders are NOT flagged
//! assert!(!detector.contains_secrets("api_key = your_api_key_here"));
//! assert!(!detector.contains_secrets("api_key = example_key_12345678"));
//! ```
//!
//! Filtered placeholder patterns: `example`, `test`, `demo`, `your_`, `placeholder`,
//! `changeme`, `xxx`, `foo`, `bar`, `sample`, `fake`, `dummy`, `mock`.
//!
//! # Graceful Degradation
//!
//! All detection is performed locally without external dependencies:
//!
//! - No network calls required
//! - No LLM fallback needed
//! - Regex patterns are statically compiled at startup
//! - Detection completes in <5ms for typical content

// Allow cast_possible_truncation for timestamp conversions.
#![allow(clippy::cast_possible_truncation)]
// Allow match_same_arms for explicit enum handling.
#![allow(clippy::match_same_arms)]
// Allow clone_on_ref_ptr - Arc clones are intentional.
#![allow(clippy::clone_on_ref_ptr)]
// Allow option_if_let_else for clearer if-let patterns with mutex locks.
#![allow(clippy::option_if_let_else)]
// Allow needless_pass_by_value for entry types passed by value for cloning.
#![allow(clippy::needless_pass_by_value)]
// Allow unused_self for methods kept for API consistency.
#![allow(clippy::unused_self)]

mod audit;
pub mod encryption;
mod pii;
pub mod rbac;
mod redactor;
mod secrets;

pub use audit::{
    AccessReviewReport, ActorAccessSummary, AuditConfig, AuditEntry, AuditLogger, AuditOutcome,
    OutcomeSummary, global_logger, init_global, record_event,
};
pub use encryption::{EncryptionConfig, Encryptor, is_encrypted};
pub use pii::{PiiDetector, PiiMatch};
pub use rbac::{
    AccessControl, AccessResult, Permission, PermissionCategory, RbacSummary, Role, RoleSummary,
};
pub use redactor::{ContentRedactor, RedactionConfig, RedactionMode};
pub use secrets::{SecretDetector, SecretMatch};
