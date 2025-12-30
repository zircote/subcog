//! Audit logging.
//!
//! Provides SOC2/GDPR compliant audit logging for memory operations.

use crate::models::MemoryEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

/// Audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique entry ID.
    pub id: String,
    /// Timestamp of the event.
    pub timestamp: DateTime<Utc>,
    /// Event type.
    pub event_type: String,
    /// Actor (user or system).
    pub actor: String,
    /// Resource affected.
    pub resource: Option<String>,
    /// Action taken.
    pub action: String,
    /// Outcome (success/failure).
    pub outcome: AuditOutcome,
    /// Additional metadata.
    pub metadata: serde_json::Value,
}

/// Outcome of an audited action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditOutcome {
    /// Action succeeded.
    Success,
    /// Action failed.
    Failure,
    /// Action was denied.
    Denied,
}

impl AuditEntry {
    /// Creates a new audit entry for the current time.
    #[must_use]
    pub fn new(event_type: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            event_type: event_type.into(),
            actor: "system".to_string(),
            resource: None,
            action: action.into(),
            outcome: AuditOutcome::Success,
            metadata: serde_json::Value::Null,
        }
    }

    /// Sets the actor.
    #[must_use]
    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = actor.into();
        self
    }

    /// Sets the resource.
    #[must_use]
    pub fn with_resource(mut self, resource: impl Into<String>) -> Self {
        self.resource = Some(resource.into());
        self
    }

    /// Sets the outcome.
    #[must_use]
    pub const fn with_outcome(mut self, outcome: AuditOutcome) -> Self {
        self.outcome = outcome;
        self
    }

    /// Sets metadata.
    #[must_use]
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Audit logger configuration.
#[derive(Debug, Clone)]
pub struct AuditConfig {
    /// Path to audit log file.
    pub log_path: Option<PathBuf>,
    /// Whether to also log to stderr.
    pub log_stderr: bool,
    /// Minimum retention period in days.
    pub retention_days: u32,
    /// Include memory content in logs (may contain sensitive data).
    pub include_content: bool,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            log_path: None,
            log_stderr: false,
            retention_days: 90,
            include_content: false,
        }
    }
}

impl AuditConfig {
    /// Creates a new audit config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the log path.
    #[must_use]
    pub fn with_log_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.log_path = Some(path.into());
        self
    }

    /// Enables stderr logging.
    #[must_use]
    pub const fn with_stderr(mut self) -> Self {
        self.log_stderr = true;
        self
    }

    /// Sets retention period.
    #[must_use]
    pub const fn with_retention(mut self, days: u32) -> Self {
        self.retention_days = days;
        self
    }
}

/// Audit logger for SOC2/GDPR compliance.
pub struct AuditLogger {
    config: AuditConfig,
    entries: Mutex<Vec<AuditEntry>>,
}

impl AuditLogger {
    /// Creates a new audit logger with default config.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: AuditConfig::default(),
            entries: Mutex::new(Vec::new()),
        }
    }

    /// Creates a new audit logger with custom config.
    #[must_use]
    pub fn with_config(config: AuditConfig) -> Self {
        Self {
            config,
            entries: Mutex::new(Vec::new()),
        }
    }

    /// Logs an audit event.
    pub fn log(&self, event: &MemoryEvent) {
        let entry = self.event_to_entry(event);
        self.log_entry(entry);
    }

    /// Logs a custom audit entry.
    pub fn log_entry(&self, entry: AuditEntry) {
        // Store in memory
        if let Ok(mut entries) = self.entries.lock() {
            entries.push(entry.clone());
        }

        // Optionally write to file
        if let Some(ref path) = self.config.log_path {
            let _ = self.append_to_file(path, &entry);
        }
    }

    /// Logs a capture event.
    pub fn log_capture(&self, memory_id: &str, namespace: &str) {
        let entry = AuditEntry::new("memory.capture", "create")
            .with_resource(memory_id)
            .with_metadata(serde_json::json!({
                "namespace": namespace
            }));
        self.log_entry(entry);
    }

    /// Logs a search/recall event.
    pub fn log_recall(&self, query: &str, result_count: usize) {
        let entry = AuditEntry::new("memory.recall", "search")
            .with_metadata(serde_json::json!({
                "query_length": query.len(),
                "result_count": result_count
            }));
        self.log_entry(entry);
    }

    /// Logs a sync event.
    pub fn log_sync(&self, pushed: usize, pulled: usize) {
        let entry = AuditEntry::new("memory.sync", "sync")
            .with_metadata(serde_json::json!({
                "pushed": pushed,
                "pulled": pulled
            }));
        self.log_entry(entry);
    }

    /// Logs a redaction event.
    pub fn log_redaction(&self, memory_id: &str, redaction_types: &[String]) {
        let entry = AuditEntry::new("security.redaction", "redact")
            .with_resource(memory_id)
            .with_metadata(serde_json::json!({
                "redaction_types": redaction_types
            }));
        self.log_entry(entry);
    }

    /// Logs an access denied event.
    pub fn log_denied(&self, action: &str, reason: &str) {
        let entry = AuditEntry::new("security.denied", action)
            .with_outcome(AuditOutcome::Denied)
            .with_metadata(serde_json::json!({
                "reason": reason
            }));
        self.log_entry(entry);
    }

    /// Returns recent audit entries.
    #[must_use]
    pub fn recent_entries(&self, limit: usize) -> Vec<AuditEntry> {
        if let Ok(entries) = self.entries.lock() {
            entries.iter().rev().take(limit).cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Returns all entries since a given timestamp.
    #[must_use]
    pub fn entries_since(&self, since: DateTime<Utc>) -> Vec<AuditEntry> {
        if let Ok(entries) = self.entries.lock() {
            entries
                .iter()
                .filter(|e| e.timestamp >= since)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Clears old entries beyond retention period.
    pub fn cleanup(&self) {
        let cutoff = Utc::now() - chrono::Duration::days(i64::from(self.config.retention_days));
        if let Ok(mut entries) = self.entries.lock() {
            entries.retain(|e| e.timestamp >= cutoff);
        }
    }

    /// Converts a `MemoryEvent` to an `AuditEntry`.
    fn event_to_entry(&self, event: &MemoryEvent) -> AuditEntry {
        match event {
            MemoryEvent::Captured {
                memory_id,
                namespace,
                domain,
                content_length,
                timestamp,
            } => AuditEntry::new("memory.captured", "create")
                .with_resource(memory_id.to_string())
                .with_metadata(serde_json::json!({
                    "namespace": namespace.as_str(),
                    "domain": domain.to_string(),
                    "content_length": content_length,
                    "event_timestamp": timestamp
                })),

            MemoryEvent::Retrieved {
                memory_id,
                query,
                score,
                timestamp,
            } => AuditEntry::new("memory.retrieved", "read")
                .with_resource(memory_id.to_string())
                .with_metadata(serde_json::json!({
                    "query_length": query.len(),
                    "score": score,
                    "event_timestamp": timestamp
                })),

            MemoryEvent::Updated {
                memory_id,
                modified_fields,
                timestamp,
            } => AuditEntry::new("memory.updated", "update")
                .with_resource(memory_id.to_string())
                .with_metadata(serde_json::json!({
                    "modified_fields": modified_fields,
                    "event_timestamp": timestamp
                })),

            MemoryEvent::Archived {
                memory_id,
                reason,
                timestamp,
            } => AuditEntry::new("memory.archived", "archive")
                .with_resource(memory_id.to_string())
                .with_metadata(serde_json::json!({
                    "reason": reason,
                    "event_timestamp": timestamp
                })),

            MemoryEvent::Deleted {
                memory_id,
                reason,
                timestamp,
            } => AuditEntry::new("memory.deleted", "delete")
                .with_resource(memory_id.to_string())
                .with_metadata(serde_json::json!({
                    "reason": reason,
                    "event_timestamp": timestamp
                })),

            MemoryEvent::Redacted {
                memory_id,
                redaction_type,
                timestamp,
            } => AuditEntry::new("security.redacted", "redact")
                .with_resource(memory_id.to_string())
                .with_metadata(serde_json::json!({
                    "redaction_type": redaction_type,
                    "event_timestamp": timestamp
                })),

            MemoryEvent::Synced {
                pushed,
                pulled,
                conflicts,
                timestamp,
            } => AuditEntry::new("memory.synced", "sync").with_metadata(serde_json::json!({
                "pushed": pushed,
                "pulled": pulled,
                "conflicts": conflicts,
                "event_timestamp": timestamp
            })),

            MemoryEvent::Consolidated {
                processed,
                archived,
                merged,
                timestamp,
            } => AuditEntry::new("memory.consolidated", "consolidate").with_metadata(serde_json::json!({
                "processed": processed,
                "archived": archived,
                "merged": merged,
                "event_timestamp": timestamp
            })),
        }
    }

    /// Appends an entry to the log file.
    fn append_to_file(&self, path: &PathBuf, entry: &AuditEntry) -> std::io::Result<()> {
        use std::fs::OpenOptions;
        use std::io::Write;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        let json = serde_json::to_string(entry).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;

        writeln!(file, "{json}")?;
        Ok(())
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, MemoryId, Namespace};

    #[test]
    fn test_audit_entry_creation() {
        let entry = AuditEntry::new("test.event", "test_action")
            .with_actor("test_user")
            .with_resource("resource_id")
            .with_outcome(AuditOutcome::Success);

        assert_eq!(entry.event_type, "test.event");
        assert_eq!(entry.action, "test_action");
        assert_eq!(entry.actor, "test_user");
        assert_eq!(entry.resource, Some("resource_id".to_string()));
        assert_eq!(entry.outcome, AuditOutcome::Success);
    }

    #[test]
    fn test_log_capture() {
        let logger = AuditLogger::new();
        logger.log_capture("mem_123", "decisions");

        let entries = logger.recent_entries(10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, "memory.capture");
    }

    #[test]
    fn test_log_recall() {
        let logger = AuditLogger::new();
        logger.log_recall("test query", 5);

        let entries = logger.recent_entries(10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, "memory.recall");
    }

    #[test]
    fn test_log_denied() {
        let logger = AuditLogger::new();
        logger.log_denied("capture", "secrets detected");

        let entries = logger.recent_entries(10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].outcome, AuditOutcome::Denied);
    }

    #[test]
    fn test_log_memory_event() {
        let logger = AuditLogger::new();
        let event = MemoryEvent::Captured {
            memory_id: MemoryId::new("test_id"),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            content_length: 100,
            timestamp: 1234567890,
        };

        logger.log(&event);

        let entries = logger.recent_entries(10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, "memory.captured");
    }

    #[test]
    fn test_entries_since() {
        let logger = AuditLogger::new();

        // Log some entries
        logger.log_capture("mem_1", "decisions");
        logger.log_capture("mem_2", "learnings");

        let since = Utc::now() - chrono::Duration::hours(1);
        let entries = logger.entries_since(since);

        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_cleanup() {
        let config = AuditConfig::new().with_retention(0); // 0 days = immediate cleanup
        let logger = AuditLogger::with_config(config);

        logger.log_capture("mem_1", "decisions");

        // Wait a tiny bit and cleanup
        std::thread::sleep(std::time::Duration::from_millis(10));
        logger.cleanup();

        let entries = logger.recent_entries(10);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_audit_entry_serialization() {
        let entry = AuditEntry::new("test.event", "action")
            .with_metadata(serde_json::json!({"key": "value"}));

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("test.event"));

        let deserialized: AuditEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type, entry.event_type);
    }
}
