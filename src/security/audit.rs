//! Audit logging.

use crate::models::MemoryEvent;

/// Audit logger for SOC2/GDPR compliance.
pub struct AuditLogger {
    // TODO: Add audit storage
}

impl AuditLogger {
    /// Creates a new audit logger.
    #[must_use]
    pub const fn new() -> Self {
        Self {}
    }

    /// Logs an audit event.
    pub fn log(&self, _event: &MemoryEvent) {
        // TODO: Implement audit logging
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new()
    }
}
