//! Security features.
//!
//! Secret detection, PII filtering, and audit logging.

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
mod pii;
mod redactor;
mod secrets;

pub use audit::{AuditConfig, AuditEntry, AuditLogger, AuditOutcome};
pub use pii::{PiiDetector, PiiMatch};
pub use redactor::{ContentRedactor, RedactionConfig, RedactionMode};
pub use secrets::{SecretDetector, SecretMatch};
