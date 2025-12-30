//! Security features.
//!
//! Secret detection, PII filtering, and audit logging.

mod audit;
mod pii;
mod redactor;
mod secrets;

pub use audit::{AuditConfig, AuditEntry, AuditLogger, AuditOutcome};
pub use pii::{PiiDetector, PiiMatch};
pub use redactor::{ContentRedactor, RedactionConfig, RedactionMode};
pub use secrets::{SecretDetector, SecretMatch};
