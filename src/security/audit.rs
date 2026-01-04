//! Audit logging.
//!
//! Provides SOC2/GDPR compliant audit logging for memory operations.
//!
//! # HMAC Chain Integrity
//!
//! Audit entries are cryptographically chained using HMAC-SHA256.
//! Each entry includes the HMAC of the previous entry, creating
//! an append-only chain that detects tampering or deletion.
//!
//! To verify chain integrity, use [`AuditLogger::verify_chain`].

use crate::models::MemoryEvent;
use crate::{Error, Result};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;

/// HMAC-SHA256 type alias.
type HmacSha256 = Hmac<Sha256>;

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
    /// HMAC signature of this entry (hex-encoded).
    ///
    /// Computed as: `HMAC-SHA256(key, id || timestamp || event_type || action || previous_hmac)`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hmac_signature: Option<String>,
    /// HMAC of the previous entry in the chain (hex-encoded).
    ///
    /// First entry in chain has `previous_hmac` = "genesis".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_hmac: Option<String>,
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
            hmac_signature: None,
            previous_hmac: None,
        }
    }

    /// Computes the canonical string for HMAC signing.
    ///
    /// Format: `id|timestamp|event_type|action|previous_hmac`
    #[must_use]
    pub fn canonical_string(&self, previous_hmac: &str) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.id,
            self.timestamp.to_rfc3339(),
            self.event_type,
            self.action,
            previous_hmac
        )
    }

    /// Computes the HMAC signature for this entry.
    ///
    /// Returns `None` if the HMAC key is invalid (should not happen with valid 32-byte keys).
    #[must_use]
    pub fn compute_hmac(&self, key: &[u8], previous_hmac: &str) -> Option<String> {
        let canonical = self.canonical_string(previous_hmac);
        let mut mac = HmacSha256::new_from_slice(key).ok()?;
        mac.update(canonical.as_bytes());
        let result = mac.finalize();
        Some(hex::encode(result.into_bytes()))
    }

    /// Signs this entry with HMAC, setting both signature and previous hash.
    ///
    /// Returns `false` if the HMAC key is invalid.
    pub fn sign(&mut self, key: &[u8], previous_hmac: &str) -> bool {
        if let Some(sig) = self.compute_hmac(key, previous_hmac) {
            self.previous_hmac = Some(previous_hmac.to_string());
            self.hmac_signature = Some(sig);
            true
        } else {
            false
        }
    }

    /// Verifies this entry's HMAC signature.
    ///
    /// Returns `true` if the signature is valid, `false` otherwise.
    #[must_use]
    pub fn verify(&self, key: &[u8]) -> bool {
        let Some(ref signature) = self.hmac_signature else {
            return false;
        };
        let Some(ref previous) = self.previous_hmac else {
            return false;
        };

        self.compute_hmac(key, previous)
            .is_some_and(|computed| computed == *signature)
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

/// Genesis hash for the first entry in an HMAC chain.
pub const GENESIS_HMAC: &str = "genesis";

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
    /// HMAC key for chain integrity (32 bytes recommended).
    ///
    /// If `None`, entries are not signed.
    pub hmac_key: Option<Vec<u8>>,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            log_path: None,
            log_stderr: false,
            retention_days: 90,
            include_content: false,
            hmac_key: None,
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

    /// Sets the HMAC key for chain integrity.
    ///
    /// The key should be at least 32 bytes for security.
    #[must_use]
    pub fn with_hmac_key(mut self, key: Vec<u8>) -> Self {
        self.hmac_key = Some(key);
        self
    }
}

/// Audit logger for SOC2/GDPR compliance.
///
/// # HMAC Chain Integrity
///
/// When configured with an HMAC key, the logger maintains a cryptographic
/// chain where each entry's signature includes the previous entry's signature.
/// This creates an append-only log that detects tampering or deletion.
///
/// The chain starts with `GENESIS_HMAC` as the "previous" value for the
/// first entry. Each subsequent entry includes the HMAC of the previous entry.
pub struct AuditLogger {
    config: AuditConfig,
    entries: Mutex<Vec<AuditEntry>>,
    /// Last HMAC in the chain (for signing new entries).
    last_hmac: Mutex<String>,
}

static GLOBAL_AUDIT_LOGGER: OnceLock<AuditLogger> = OnceLock::new();

impl AuditLogger {
    /// Creates a new audit logger with default config.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: AuditConfig::default(),
            entries: Mutex::new(Vec::new()),
            last_hmac: Mutex::new(GENESIS_HMAC.to_string()),
        }
    }

    /// Creates a new audit logger with custom config.
    #[must_use]
    pub fn with_config(config: AuditConfig) -> Self {
        Self {
            config,
            entries: Mutex::new(Vec::new()),
            last_hmac: Mutex::new(GENESIS_HMAC.to_string()),
        }
    }

    /// Logs an audit event.
    pub fn log(&self, event: &MemoryEvent) {
        let entry = self.event_to_entry(event);
        self.log_entry(entry);
    }

    /// Logs a custom audit entry.
    ///
    /// If an HMAC key is configured, the entry is signed and chained
    /// to the previous entry before storage.
    pub fn log_entry(&self, entry: AuditEntry) {
        let signed_entry = self.sign_entry(entry);

        // Store in memory
        if let Ok(mut entries) = self.entries.lock() {
            entries.push(signed_entry.clone());
        }

        // Optionally write to file
        if let Some(ref path) = self.config.log_path {
            let _ = self.append_to_file(path, &signed_entry);
        }
    }

    /// Signs an entry with HMAC if configured, updating chain state.
    fn sign_entry(&self, mut entry: AuditEntry) -> AuditEntry {
        let Some(ref key) = self.config.hmac_key else {
            return entry;
        };
        let Ok(mut last) = self.last_hmac.lock() else {
            return entry;
        };

        if entry.sign(key, &last) {
            if let Some(ref sig) = entry.hmac_signature {
                last.clone_from(sig);
            }
        }
        entry
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
        let entry = AuditEntry::new("memory.recall", "search").with_metadata(serde_json::json!({
            "query_length": query.len(),
            "result_count": result_count
        }));
        self.log_entry(entry);
    }

    /// Logs a sync event.
    pub fn log_sync(&self, pushed: usize, pulled: usize) {
        let entry = AuditEntry::new("memory.sync", "sync").with_metadata(serde_json::json!({
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

    /// Logs a PII detection event for GDPR/SOC2 compliance.
    ///
    /// Records when PII is detected in content, including the types of PII found
    /// and the count. The actual PII values are NOT logged to avoid storing
    /// sensitive data in audit logs.
    pub fn log_pii_detection(&self, pii_types: &[String], context: Option<&str>) {
        let entry =
            AuditEntry::new("security.pii_detection", "detect").with_metadata(serde_json::json!({
                "pii_types": pii_types,
                "pii_count": pii_types.len(),
                "context": context
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

    /// Verifies the HMAC chain integrity of all entries.
    ///
    /// Returns `Ok(())` if all entries are valid and properly chained,
    /// or an error describing the first invalid entry found.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No HMAC key is configured
    /// - An entry has an invalid or missing signature
    /// - The chain is broken (entry's `previous_hmac` doesn't match prior signature)
    pub fn verify_chain(&self) -> Result<()> {
        let key = self
            .config
            .hmac_key
            .as_ref()
            .ok_or_else(|| Error::OperationFailed {
                operation: "verify_chain".to_string(),
                cause: "no HMAC key configured".to_string(),
            })?;

        // Clone entries to release lock early and avoid significant_drop_tightening
        let entries: Vec<AuditEntry> = self
            .entries
            .lock()
            .map_err(|_| Error::OperationFailed {
                operation: "verify_chain".to_string(),
                cause: "failed to acquire lock".to_string(),
            })?
            .clone();

        let mut expected_previous = GENESIS_HMAC.to_string();

        for (i, entry) in entries.iter().enumerate() {
            // Check that entry has HMAC fields
            let Some(ref signature) = entry.hmac_signature else {
                return Err(Error::OperationFailed {
                    operation: "verify_chain".to_string(),
                    cause: format!("entry {i} missing hmac_signature"),
                });
            };
            let Some(ref previous) = entry.previous_hmac else {
                return Err(Error::OperationFailed {
                    operation: "verify_chain".to_string(),
                    cause: format!("entry {i} missing previous_hmac"),
                });
            };

            // Verify chain linkage
            if *previous != expected_previous {
                return Err(Error::OperationFailed {
                    operation: "verify_chain".to_string(),
                    cause: format!(
                        "entry {i} chain broken: expected previous '{expected_previous}', got '{previous}'"
                    ),
                });
            }

            // Verify signature
            if !entry.verify(key) {
                return Err(Error::OperationFailed {
                    operation: "verify_chain".to_string(),
                    cause: format!("entry {i} has invalid signature"),
                });
            }

            // Update expected previous for next iteration
            expected_previous.clone_from(signature);
        }

        Ok(())
    }

    /// Returns whether HMAC signing is enabled.
    #[must_use]
    pub const fn is_signing_enabled(&self) -> bool {
        self.config.hmac_key.is_some()
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
            } => AuditEntry::new("memory.consolidated", "consolidate").with_metadata(
                serde_json::json!({
                    "processed": processed,
                    "archived": archived,
                    "merged": merged,
                    "event_timestamp": timestamp
                }),
            ),
        }
    }

    /// Appends an entry to the log file.
    ///
    /// # Security
    ///
    /// - Path canonicalization is performed to prevent TOCTOU race conditions
    ///   where a symlink could be modified between path validation and file open.
    /// - On Unix, file permissions are set atomically to 0o600 (owner read/write only)
    ///   at file creation time using `OpenOptionsExt::mode()` to prevent race conditions.
    fn append_to_file(&self, path: &std::path::Path, entry: &AuditEntry) -> std::io::Result<()> {
        use std::fs::OpenOptions;
        use std::io::Write;

        // Canonicalize path to resolve symlinks and prevent TOCTOU attacks.
        // If the file doesn't exist yet, canonicalize the parent directory instead.
        let canonical_path = Self::canonicalize_path(path)?;

        // Use OpenOptionsExt::mode() on Unix to set permissions atomically at creation time.
        // This prevents the TOCTOU race where the file could be accessed with default
        // permissions before set_permissions() is called.
        #[cfg(unix)]
        let mut file = {
            use std::os::unix::fs::OpenOptionsExt;
            OpenOptions::new()
                .create(true)
                .append(true)
                .mode(0o600) // Atomic permission setting at creation
                .open(&canonical_path)?
        };

        #[cfg(not(unix))]
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&canonical_path)?;

        let json = serde_json::to_string(entry)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        writeln!(file, "{json}")?;
        Ok(())
    }

    /// Canonicalizes a path, handling non-existent files by canonicalizing the parent.
    fn canonicalize_path(path: &std::path::Path) -> std::io::Result<PathBuf> {
        if path.exists() {
            return path.canonicalize();
        }

        let Some(parent) = path.parent() else {
            return Ok(path.to_path_buf());
        };

        if !parent.exists() {
            // Parent doesn't exist - return as-is, let OpenOptions handle the error
            return Ok(path.to_path_buf());
        }

        let file_name = path.file_name().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid file name")
        })?;

        Ok(parent.canonicalize()?.join(file_name))
    }
}

/// Initializes the global audit logger.
///
/// # Errors
///
/// Returns an error if the log directory cannot be created or if the global
/// audit logger has already been initialized.
pub fn init_global(config: AuditConfig) -> Result<()> {
    if let Some(ref path) = config.log_path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::OperationFailed {
                operation: "init_audit_logger".to_string(),
                cause: e.to_string(),
            })?;
        }
    }

    GLOBAL_AUDIT_LOGGER
        .set(AuditLogger::with_config(config))
        .map_err(|_logger| Error::OperationFailed {
            operation: "init_audit_logger".to_string(),
            cause: "audit logger already initialized".to_string(),
        })?;

    Ok(())
}

/// Returns the global audit logger, if initialized.
#[must_use]
pub fn global_logger() -> Option<&'static AuditLogger> {
    GLOBAL_AUDIT_LOGGER.get()
}

/// Records a memory event through the global audit logger.
pub fn record_event(event: MemoryEvent) {
    if let Some(logger) = global_logger() {
        logger.log(&event);
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
            timestamp: 1_234_567_890,
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

    // HMAC Chain Tests

    #[test]
    fn test_hmac_sign_and_verify() {
        let key = b"test_key_32_bytes_long_xxxxxxxx";
        let mut entry = AuditEntry::new("test.event", "action");

        assert!(entry.sign(key, GENESIS_HMAC));
        assert!(entry.hmac_signature.is_some());
        assert_eq!(entry.previous_hmac, Some(GENESIS_HMAC.to_string()));
        assert!(entry.verify(key));
    }

    #[test]
    fn test_hmac_verify_fails_with_wrong_key() {
        let key = b"test_key_32_bytes_long_xxxxxxxx";
        let wrong_key = b"wrong_key_32_bytes_long_xxxxxxx";
        let mut entry = AuditEntry::new("test.event", "action");

        assert!(entry.sign(key, GENESIS_HMAC));
        assert!(!entry.verify(wrong_key));
    }

    #[test]
    fn test_hmac_verify_fails_with_tampered_content() {
        let key = b"test_key_32_bytes_long_xxxxxxxx";
        let mut entry = AuditEntry::new("test.event", "action");

        assert!(entry.sign(key, GENESIS_HMAC));
        entry.action = "tampered_action".to_string();
        assert!(!entry.verify(key));
    }

    #[test]
    fn test_hmac_chain_signing() {
        let key = vec![0u8; 32]; // 32-byte key
        let config = AuditConfig::new().with_hmac_key(key.clone());
        let logger = AuditLogger::with_config(config);

        // Log multiple entries
        logger.log_capture("mem_1", "decisions");
        logger.log_capture("mem_2", "learnings");
        logger.log_capture("mem_3", "patterns");

        let entries = logger.recent_entries(10);
        assert_eq!(entries.len(), 3);

        // All entries should be signed
        for entry in &entries {
            assert!(entry.hmac_signature.is_some());
            assert!(entry.previous_hmac.is_some());
            assert!(entry.verify(&key));
        }
    }

    #[test]
    fn test_hmac_chain_verification() {
        let key = vec![0u8; 32];
        let config = AuditConfig::new().with_hmac_key(key);
        let logger = AuditLogger::with_config(config);

        logger.log_capture("mem_1", "decisions");
        logger.log_capture("mem_2", "learnings");

        assert!(logger.verify_chain().is_ok());
    }

    #[test]
    fn test_hmac_chain_verification_no_key() {
        let logger = AuditLogger::new();
        logger.log_capture("mem_1", "decisions");

        // Should fail because no HMAC key is configured
        assert!(logger.verify_chain().is_err());
    }

    #[test]
    fn test_is_signing_enabled() {
        let logger_without_key = AuditLogger::new();
        assert!(!logger_without_key.is_signing_enabled());

        let config = AuditConfig::new().with_hmac_key(vec![0u8; 32]);
        let logger_with_key = AuditLogger::with_config(config);
        assert!(logger_with_key.is_signing_enabled());
    }

    #[test]
    fn test_hmac_entry_serialization_with_signature() {
        let key = b"test_key_32_bytes_long_xxxxxxxx";
        let mut entry = AuditEntry::new("test.event", "action");
        assert!(entry.sign(key, GENESIS_HMAC));

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("hmac_signature"));
        assert!(json.contains("previous_hmac"));
        assert!(json.contains("genesis"));

        let deserialized: AuditEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.hmac_signature, entry.hmac_signature);
        assert_eq!(deserialized.previous_hmac, entry.previous_hmac);
        assert!(deserialized.verify(key));
    }

    #[test]
    fn test_unsigned_entry_omits_hmac_fields() {
        let entry = AuditEntry::new("test.event", "action");

        let json = serde_json::to_string(&entry).unwrap();
        // Fields with skip_serializing_if = "Option::is_none" should be omitted
        assert!(!json.contains("hmac_signature"));
        assert!(!json.contains("previous_hmac"));
    }

    #[test]
    fn test_log_pii_detection() {
        let logger = AuditLogger::new();
        let pii_types = vec!["Email Address".to_string(), "SSN".to_string()];
        logger.log_pii_detection(&pii_types, Some("content_redaction"));

        let entries = logger.recent_entries(10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, "security.pii_detection");
        assert_eq!(entries[0].action, "detect");

        // Verify metadata contains expected fields
        let metadata = &entries[0].metadata;
        assert_eq!(metadata["pii_count"], 2);
        assert_eq!(metadata["context"], "content_redaction");
    }

    #[test]
    fn test_log_pii_detection_without_context() {
        let logger = AuditLogger::new();
        let pii_types = vec!["Phone Number".to_string()];
        logger.log_pii_detection(&pii_types, None);

        let entries = logger.recent_entries(10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].metadata["pii_count"], 1);
        assert!(entries[0].metadata["context"].is_null());
    }
}
