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

use crate::models::{EventMeta, MemoryEvent};
use crate::observability::{
    RequestContext, current_request_id, global_event_bus, scope_request_context,
};
use crate::{Error, Result};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;
use tracing::Instrument;

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

        if entry.sign(key, &last)
            && let Some(ref sig) = entry.hmac_signature
        {
            last.clone_from(sig);
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

    /// Logs a PII disclosure event for GDPR/SOC2 compliance.
    ///
    /// Records when data containing PII is disclosed to external systems such as:
    /// - LLM providers (Anthropic, `OpenAI`, Ollama, etc.)
    /// - Remote sync destinations
    /// - External APIs
    ///
    /// The actual PII values are NOT logged to avoid storing sensitive data.
    /// Only metadata about the disclosure is recorded.
    ///
    /// # Arguments
    ///
    /// * `destination` - The external system receiving the data (e.g., "anthropic", "openai")
    /// * `pii_types` - Types of PII being disclosed
    /// * `data_subject_id` - Optional identifier of the data subject (anonymized)
    /// * `purpose` - The purpose of the disclosure
    /// * `legal_basis` - Legal basis for disclosure (e.g., "consent", "`legitimate_interest`")
    pub fn log_pii_disclosure(
        &self,
        destination: &str,
        pii_types: &[String],
        data_subject_id: Option<&str>,
        purpose: &str,
        legal_basis: &str,
    ) {
        let entry = AuditEntry::new("security.pii_disclosure", "disclose").with_metadata(
            serde_json::json!({
                "destination": destination,
                "pii_types": pii_types,
                "pii_count": pii_types.len(),
                "data_subject_id": data_subject_id,
                "purpose": purpose,
                "legal_basis": legal_basis,
                "timestamp_utc": Utc::now().to_rfc3339()
            }),
        );
        self.log_entry(entry);
    }

    /// Logs a bulk PII disclosure event for batch operations.
    ///
    /// Similar to `log_pii_disclosure` but for bulk operations involving multiple
    /// data subjects or records.
    ///
    /// # Arguments
    ///
    /// * `destination` - The external system receiving the data
    /// * `record_count` - Number of records being disclosed
    /// * `pii_categories` - Categories of PII being disclosed
    /// * `purpose` - The purpose of the disclosure
    /// * `legal_basis` - Legal basis for disclosure
    pub fn log_bulk_pii_disclosure(
        &self,
        destination: &str,
        record_count: usize,
        pii_categories: &[String],
        purpose: &str,
        legal_basis: &str,
    ) {
        let entry = AuditEntry::new("security.pii_bulk_disclosure", "bulk_disclose").with_metadata(
            serde_json::json!({
                "destination": destination,
                "record_count": record_count,
                "pii_categories": pii_categories,
                "purpose": purpose,
                "legal_basis": legal_basis,
                "timestamp_utc": Utc::now().to_rfc3339()
            }),
        );
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
    #[allow(clippy::too_many_lines)]
    fn event_to_entry(&self, event: &MemoryEvent) -> AuditEntry {
        fn base_metadata(meta: &EventMeta) -> serde_json::Map<String, serde_json::Value> {
            let mut metadata = serde_json::Map::new();
            metadata.insert(
                "event_id".to_string(),
                serde_json::Value::String(meta.event_id.clone()),
            );
            metadata.insert(
                "correlation_id".to_string(),
                meta.correlation_id
                    .clone()
                    .map_or(serde_json::Value::Null, serde_json::Value::String),
            );
            metadata.insert(
                "source".to_string(),
                serde_json::Value::String(meta.source.to_string()),
            );
            metadata.insert(
                "event_timestamp".to_string(),
                serde_json::Value::Number(meta.timestamp.into()),
            );
            metadata
        }

        match event {
            MemoryEvent::Captured {
                meta,
                memory_id,
                namespace,
                domain,
                content_length,
            } => {
                let mut metadata = base_metadata(meta);
                metadata.insert(
                    "namespace".to_string(),
                    serde_json::Value::String(namespace.as_str().to_string()),
                );
                metadata.insert(
                    "domain".to_string(),
                    serde_json::Value::String(domain.to_string()),
                );
                metadata.insert(
                    "content_length".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(*content_length as u64)),
                );

                AuditEntry::new("memory.captured", "create")
                    .with_resource(memory_id.to_string())
                    .with_metadata(serde_json::Value::Object(metadata))
            },

            MemoryEvent::Retrieved {
                meta,
                memory_id,
                query,
                score,
            } => {
                let mut metadata = base_metadata(meta);
                metadata.insert(
                    "query_length".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(query.len() as u64)),
                );
                metadata.insert(
                    "score".to_string(),
                    serde_json::Value::Number(
                        serde_json::Number::from_f64(f64::from(*score))
                            .unwrap_or_else(|| serde_json::Number::from(0_u64)),
                    ),
                );

                AuditEntry::new("memory.retrieved", "read")
                    .with_resource(memory_id.to_string())
                    .with_metadata(serde_json::Value::Object(metadata))
            },

            MemoryEvent::Updated {
                meta,
                memory_id,
                modified_fields,
            } => {
                let mut metadata = base_metadata(meta);
                metadata.insert(
                    "modified_fields".to_string(),
                    serde_json::Value::Array(
                        modified_fields
                            .iter()
                            .cloned()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );

                AuditEntry::new("memory.updated", "update")
                    .with_resource(memory_id.to_string())
                    .with_metadata(serde_json::Value::Object(metadata))
            },

            MemoryEvent::Archived {
                meta,
                memory_id,
                reason,
            } => {
                let mut metadata = base_metadata(meta);
                metadata.insert(
                    "reason".to_string(),
                    serde_json::Value::String(reason.clone()),
                );

                AuditEntry::new("memory.archived", "archive")
                    .with_resource(memory_id.to_string())
                    .with_metadata(serde_json::Value::Object(metadata))
            },

            MemoryEvent::Deleted {
                meta,
                memory_id,
                reason,
            } => {
                let mut metadata = base_metadata(meta);
                metadata.insert(
                    "reason".to_string(),
                    serde_json::Value::String(reason.clone()),
                );

                AuditEntry::new("memory.deleted", "delete")
                    .with_resource(memory_id.to_string())
                    .with_metadata(serde_json::Value::Object(metadata))
            },

            MemoryEvent::Redacted {
                meta,
                memory_id,
                redaction_type,
            } => {
                let mut metadata = base_metadata(meta);
                metadata.insert(
                    "redaction_type".to_string(),
                    serde_json::Value::String(redaction_type.clone()),
                );

                AuditEntry::new("security.redacted", "redact")
                    .with_resource(memory_id.to_string())
                    .with_metadata(serde_json::Value::Object(metadata))
            },

            MemoryEvent::Synced {
                meta,
                pushed,
                pulled,
                conflicts,
            } => {
                let mut metadata = base_metadata(meta);
                metadata.insert(
                    "pushed".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(*pushed as u64)),
                );
                metadata.insert(
                    "pulled".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(*pulled as u64)),
                );
                metadata.insert(
                    "conflicts".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(*conflicts as u64)),
                );

                AuditEntry::new("memory.synced", "sync")
                    .with_metadata(serde_json::Value::Object(metadata))
            },

            MemoryEvent::Consolidated {
                meta,
                processed,
                archived,
                merged,
            } => {
                let mut metadata = base_metadata(meta);
                metadata.insert(
                    "processed".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(*processed as u64)),
                );
                metadata.insert(
                    "archived".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(*archived as u64)),
                );
                metadata.insert(
                    "merged".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(*merged as u64)),
                );

                AuditEntry::new("memory.consolidated", "consolidate")
                    .with_metadata(serde_json::Value::Object(metadata))
            },

            MemoryEvent::McpStarted {
                meta,
                transport,
                port,
            } => {
                let mut metadata = base_metadata(meta);
                metadata.insert(
                    "transport".to_string(),
                    serde_json::Value::String(transport.clone()),
                );
                metadata.insert(
                    "port".to_string(),
                    port.map_or(serde_json::Value::Null, |p| {
                        serde_json::Value::Number(p.into())
                    }),
                );

                AuditEntry::new("mcp.started", "start")
                    .with_metadata(serde_json::Value::Object(metadata))
            },

            MemoryEvent::McpAuthFailed {
                meta,
                client_id,
                reason,
            } => {
                let mut metadata = base_metadata(meta);
                metadata.insert(
                    "client_id".to_string(),
                    client_id
                        .clone()
                        .map_or(serde_json::Value::Null, serde_json::Value::String),
                );
                metadata.insert(
                    "reason".to_string(),
                    serde_json::Value::String(reason.clone()),
                );

                AuditEntry::new("mcp.auth_failed", "authenticate")
                    .with_outcome(AuditOutcome::Denied)
                    .with_metadata(serde_json::Value::Object(metadata))
            },

            MemoryEvent::McpToolExecuted {
                meta,
                tool_name,
                status,
                duration_ms,
                error,
            } => {
                let mut metadata = base_metadata(meta);
                metadata.insert(
                    "tool_name".to_string(),
                    serde_json::Value::String(tool_name.clone()),
                );
                metadata.insert(
                    "status".to_string(),
                    serde_json::Value::String(status.clone()),
                );
                metadata.insert(
                    "duration_ms".to_string(),
                    serde_json::Value::Number((*duration_ms).into()),
                );
                metadata.insert(
                    "error".to_string(),
                    error
                        .clone()
                        .map_or(serde_json::Value::Null, serde_json::Value::String),
                );

                let outcome = if status == "success" {
                    AuditOutcome::Success
                } else {
                    AuditOutcome::Failure
                };

                AuditEntry::new("mcp.tool_executed", "execute")
                    .with_outcome(outcome)
                    .with_metadata(serde_json::Value::Object(metadata))
            },

            MemoryEvent::McpRequestError {
                meta,
                operation,
                error,
            } => {
                let mut metadata = base_metadata(meta);
                metadata.insert(
                    "operation".to_string(),
                    serde_json::Value::String(operation.clone()),
                );
                metadata.insert(
                    "error".to_string(),
                    serde_json::Value::String(error.clone()),
                );

                AuditEntry::new("mcp.request_error", "request")
                    .with_outcome(AuditOutcome::Failure)
                    .with_metadata(serde_json::Value::Object(metadata))
            },

            MemoryEvent::HookInvoked { meta, hook } => {
                let mut metadata = base_metadata(meta);
                metadata.insert("hook".to_string(), serde_json::Value::String(hook.clone()));

                AuditEntry::new("hook.invoked", "invoke")
                    .with_metadata(serde_json::Value::Object(metadata))
            },

            MemoryEvent::HookClassified {
                meta,
                hook,
                classification,
                classifier,
                confidence,
            } => {
                let mut metadata = base_metadata(meta);
                metadata.insert("hook".to_string(), serde_json::Value::String(hook.clone()));
                metadata.insert(
                    "classification".to_string(),
                    serde_json::Value::String(classification.clone()),
                );
                metadata.insert(
                    "classifier".to_string(),
                    serde_json::Value::String(classifier.clone()),
                );
                metadata.insert(
                    "confidence".to_string(),
                    serde_json::Value::Number(
                        serde_json::Number::from_f64(f64::from(*confidence))
                            .unwrap_or_else(|| serde_json::Number::from(0_u64)),
                    ),
                );

                AuditEntry::new("hook.classified", "classify")
                    .with_metadata(serde_json::Value::Object(metadata))
            },

            MemoryEvent::HookCaptureDecision {
                meta,
                hook,
                decision,
                namespace,
                memory_id,
            } => {
                let mut metadata = base_metadata(meta);
                metadata.insert("hook".to_string(), serde_json::Value::String(hook.clone()));
                metadata.insert(
                    "decision".to_string(),
                    serde_json::Value::String(decision.clone()),
                );
                metadata.insert(
                    "namespace".to_string(),
                    namespace
                        .clone()
                        .map_or(serde_json::Value::Null, serde_json::Value::String),
                );
                metadata.insert(
                    "memory_id".to_string(),
                    memory_id.as_ref().map_or(serde_json::Value::Null, |id| {
                        serde_json::Value::String(id.to_string())
                    }),
                );

                AuditEntry::new("hook.capture_decision", "decision")
                    .with_metadata(serde_json::Value::Object(metadata))
            },

            MemoryEvent::HookFailed { meta, hook, error } => {
                let mut metadata = base_metadata(meta);
                metadata.insert("hook".to_string(), serde_json::Value::String(hook.clone()));
                metadata.insert(
                    "error".to_string(),
                    serde_json::Value::String(error.clone()),
                );

                AuditEntry::new("hook.failed", "invoke")
                    .with_outcome(AuditOutcome::Failure)
                    .with_metadata(serde_json::Value::Object(metadata))
            },
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
    if let Some(ref path) = config.log_path
        && let Some(parent) = path.parent()
    {
        std::fs::create_dir_all(parent).map_err(|e| Error::OperationFailed {
            operation: "init_audit_logger".to_string(),
            cause: e.to_string(),
        })?;
    }

    GLOBAL_AUDIT_LOGGER
        .set(AuditLogger::with_config(config))
        .map_err(|_logger| Error::OperationFailed {
            operation: "init_audit_logger".to_string(),
            cause: "audit logger already initialized".to_string(),
        })?;

    start_audit_subscription();

    Ok(())
}

/// Returns the global audit logger, if initialized.
#[must_use]
pub fn global_logger() -> Option<&'static AuditLogger> {
    GLOBAL_AUDIT_LOGGER.get()
}

fn log_event_if_configured(event: &MemoryEvent) {
    if let Some(logger) = global_logger() {
        logger.log(event);
    }
}

fn start_audit_subscription() {
    if tokio::runtime::Handle::try_current().is_err() {
        tracing::warn!("Audit event subscription requires a Tokio runtime");
        return;
    }

    let mut receiver = global_event_bus().subscribe();
    let span = tracing::Span::current();
    let request_context = current_request_id().map(RequestContext::from_id);
    tokio::spawn(
        async move {
            let run = async move {
                while let Ok(event) = receiver.recv().await {
                    log_event_if_configured(&event);
                }
            };

            if let Some(context) = request_context {
                scope_request_context(context, run).await;
            } else {
                run.await;
            }
        }
        .instrument(span),
    );
}

/// Records a memory event through the global audit logger.
pub fn record_event(event: MemoryEvent) {
    global_event_bus().publish(event);
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Access Review Reports (COMP-HIGH-004)
// ============================================================================

/// Access review report for SOC2 compliance.
///
/// Aggregates audit entries into a structured report showing:
/// - Who accessed what resources
/// - When accesses occurred
/// - What actions were taken
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessReviewReport {
    /// Report generation timestamp.
    pub generated_at: DateTime<Utc>,
    /// Start of the review period.
    pub period_start: DateTime<Utc>,
    /// End of the review period.
    pub period_end: DateTime<Utc>,
    /// Total number of access events.
    pub total_events: usize,
    /// Summary by actor (user or system).
    pub by_actor: std::collections::HashMap<String, ActorAccessSummary>,
    /// Summary by resource type.
    pub by_resource_type: std::collections::HashMap<String, usize>,
    /// Summary by action type.
    pub by_action: std::collections::HashMap<String, usize>,
    /// Summary by outcome.
    pub by_outcome: OutcomeSummary,
}

/// Summary of access events for a single actor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorAccessSummary {
    /// Total events for this actor.
    pub event_count: usize,
    /// Distinct resources accessed.
    pub resources_accessed: std::collections::HashSet<String>,
    /// Actions performed.
    pub actions: std::collections::HashMap<String, usize>,
    /// First access timestamp.
    pub first_access: DateTime<Utc>,
    /// Last access timestamp.
    pub last_access: DateTime<Utc>,
}

/// Summary of outcomes across all events.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutcomeSummary {
    /// Count of successful operations.
    pub success: usize,
    /// Count of failed operations.
    pub failure: usize,
    /// Count of denied operations.
    pub denied: usize,
}

impl AccessReviewReport {
    /// Generates an access review report from audit entries.
    ///
    /// # Arguments
    ///
    /// * `entries` - The audit entries to analyze
    /// * `period_start` - Start of the review period
    /// * `period_end` - End of the review period
    #[must_use]
    pub fn generate(
        entries: &[AuditEntry],
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> Self {
        let mut by_actor: std::collections::HashMap<String, ActorAccessSummary> =
            std::collections::HashMap::new();
        let mut by_resource_type: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut by_action: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut by_outcome = OutcomeSummary::default();

        // Filter entries within the period
        let filtered: Vec<_> = entries
            .iter()
            .filter(|e| e.timestamp >= period_start && e.timestamp <= period_end)
            .collect();

        for entry in &filtered {
            // Update actor summary
            let actor_summary =
                by_actor
                    .entry(entry.actor.clone())
                    .or_insert_with(|| ActorAccessSummary {
                        event_count: 0,
                        resources_accessed: std::collections::HashSet::new(),
                        actions: std::collections::HashMap::new(),
                        first_access: entry.timestamp,
                        last_access: entry.timestamp,
                    });

            actor_summary.event_count += 1;
            if let Some(ref resource) = entry.resource {
                actor_summary.resources_accessed.insert(resource.clone());
            }
            *actor_summary
                .actions
                .entry(entry.action.clone())
                .or_insert(0) += 1;
            if entry.timestamp < actor_summary.first_access {
                actor_summary.first_access = entry.timestamp;
            }
            if entry.timestamp > actor_summary.last_access {
                actor_summary.last_access = entry.timestamp;
            }

            // Update resource type count
            *by_resource_type
                .entry(entry.event_type.clone())
                .or_insert(0) += 1;

            // Update action count
            *by_action.entry(entry.action.clone()).or_insert(0) += 1;

            // Update outcome count
            match entry.outcome {
                AuditOutcome::Success => by_outcome.success += 1,
                AuditOutcome::Failure => by_outcome.failure += 1,
                AuditOutcome::Denied => by_outcome.denied += 1,
            }
        }

        Self {
            generated_at: Utc::now(),
            period_start,
            period_end,
            total_events: filtered.len(),
            by_actor,
            by_resource_type,
            by_action,
            by_outcome,
        }
    }
}

impl AuditLogger {
    /// Generates an access review report for the specified period.
    ///
    /// # Arguments
    ///
    /// * `period_start` - Start of the review period
    /// * `period_end` - End of the review period
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use chrono::{Duration, Utc};
    /// use subcog::security::AuditLogger;
    ///
    /// let logger = AuditLogger::new();
    /// let end = Utc::now();
    /// let start = end - Duration::days(30);
    ///
    /// let report = logger.generate_access_review(start, end);
    /// println!("Total events: {}", report.total_events);
    /// ```
    #[must_use]
    pub fn generate_access_review(
        &self,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> AccessReviewReport {
        let entries = self.entries_since(period_start);
        AccessReviewReport::generate(&entries, period_start, period_end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, EventMeta, MemoryId, Namespace};

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
            meta: EventMeta::with_timestamp("test", None, 1_234_567_890),
            memory_id: MemoryId::new("test_id"),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            content_length: 100,
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

    // Access Review Report Tests

    #[test]
    fn test_access_review_report_empty() {
        let entries: Vec<AuditEntry> = vec![];
        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now();

        let report = AccessReviewReport::generate(&entries, start, end);

        assert_eq!(report.total_events, 0);
        assert!(report.by_actor.is_empty());
        assert!(report.by_resource_type.is_empty());
        assert!(report.by_action.is_empty());
        assert_eq!(report.by_outcome.success, 0);
        assert_eq!(report.by_outcome.failure, 0);
        assert_eq!(report.by_outcome.denied, 0);
    }

    #[test]
    fn test_access_review_report_single_entry() {
        let entry = AuditEntry::new("memory.capture", "capture")
            .with_actor("user1")
            .with_resource("mem_123")
            .with_outcome(AuditOutcome::Success);

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        let report = AccessReviewReport::generate(&[entry], start, end);

        assert_eq!(report.total_events, 1);
        assert_eq!(report.by_actor.len(), 1);
        assert!(report.by_actor.contains_key("user1"));

        let actor_summary = report.by_actor.get("user1").unwrap();
        assert_eq!(actor_summary.event_count, 1);
        assert!(actor_summary.resources_accessed.contains("mem_123"));
        assert_eq!(actor_summary.actions.get("capture"), Some(&1));

        assert_eq!(report.by_outcome.success, 1);
        assert_eq!(report.by_outcome.failure, 0);
        assert_eq!(report.by_outcome.denied, 0);
    }

    #[test]
    fn test_access_review_report_multiple_actors() {
        let entry1 = AuditEntry::new("memory.capture", "capture")
            .with_actor("user1")
            .with_outcome(AuditOutcome::Success);
        let entry2 = AuditEntry::new("memory.recall", "recall")
            .with_actor("user2")
            .with_outcome(AuditOutcome::Success);
        let entry3 = AuditEntry::new("memory.capture", "capture")
            .with_actor("user1")
            .with_outcome(AuditOutcome::Denied);

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        let report = AccessReviewReport::generate(&[entry1, entry2, entry3], start, end);

        assert_eq!(report.total_events, 3);
        assert_eq!(report.by_actor.len(), 2);

        let user1_summary = report.by_actor.get("user1").unwrap();
        assert_eq!(user1_summary.event_count, 2);
        assert_eq!(user1_summary.actions.get("capture"), Some(&2));

        let user2_summary = report.by_actor.get("user2").unwrap();
        assert_eq!(user2_summary.event_count, 1);
        assert_eq!(user2_summary.actions.get("recall"), Some(&1));
    }

    #[test]
    fn test_access_review_report_outcome_counting() {
        let entry1 = AuditEntry::new("test.event", "action").with_outcome(AuditOutcome::Success);
        let entry2 = AuditEntry::new("test.event", "action").with_outcome(AuditOutcome::Success);
        let entry3 = AuditEntry::new("test.event", "action").with_outcome(AuditOutcome::Failure);
        let entry4 = AuditEntry::new("test.event", "action").with_outcome(AuditOutcome::Denied);
        let entry5 = AuditEntry::new("test.event", "action").with_outcome(AuditOutcome::Denied);

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        let report =
            AccessReviewReport::generate(&[entry1, entry2, entry3, entry4, entry5], start, end);

        assert_eq!(report.by_outcome.success, 2);
        assert_eq!(report.by_outcome.failure, 1);
        assert_eq!(report.by_outcome.denied, 2);
    }

    #[test]
    fn test_access_review_report_filters_by_period() {
        let now = Utc::now();

        // Create entries at different times
        let mut entry_old = AuditEntry::new("memory.capture", "capture");
        entry_old.timestamp = now - chrono::Duration::days(10);

        let mut entry_in_range = AuditEntry::new("memory.recall", "recall");
        entry_in_range.timestamp = now - chrono::Duration::hours(12);

        let mut entry_future = AuditEntry::new("memory.delete", "delete");
        entry_future.timestamp = now + chrono::Duration::days(10);

        let start = now - chrono::Duration::days(1);
        let end = now + chrono::Duration::days(1);

        let report =
            AccessReviewReport::generate(&[entry_old, entry_in_range, entry_future], start, end);

        // Only entry_in_range should be included
        assert_eq!(report.total_events, 1);
        assert_eq!(report.by_action.get("recall"), Some(&1));
        assert!(!report.by_action.contains_key("capture"));
        assert!(!report.by_action.contains_key("delete"));
    }

    #[test]
    fn test_access_review_report_resource_aggregation() {
        let entry1 = AuditEntry::new("memory.capture", "capture")
            .with_actor("user1")
            .with_resource("mem_1");
        let entry2 = AuditEntry::new("memory.capture", "capture")
            .with_actor("user1")
            .with_resource("mem_2");
        let entry3 = AuditEntry::new("memory.recall", "recall")
            .with_actor("user1")
            .with_resource("mem_1"); // Same resource as entry1

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        let report = AccessReviewReport::generate(&[entry1, entry2, entry3], start, end);

        let user1_summary = report.by_actor.get("user1").unwrap();
        assert_eq!(user1_summary.event_count, 3);
        // resources_accessed should dedupe: mem_1, mem_2
        assert_eq!(user1_summary.resources_accessed.len(), 2);
        assert!(user1_summary.resources_accessed.contains("mem_1"));
        assert!(user1_summary.resources_accessed.contains("mem_2"));
    }

    #[test]
    fn test_access_review_report_action_counting() {
        let entry1 = AuditEntry::new("memory.capture", "capture");
        let entry2 = AuditEntry::new("memory.capture", "capture");
        let entry3 = AuditEntry::new("memory.recall", "recall");
        let entry4 = AuditEntry::new("memory.delete", "delete");

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        let report = AccessReviewReport::generate(&[entry1, entry2, entry3, entry4], start, end);

        assert_eq!(report.by_action.get("capture"), Some(&2));
        assert_eq!(report.by_action.get("recall"), Some(&1));
        assert_eq!(report.by_action.get("delete"), Some(&1));
    }

    #[test]
    fn test_access_review_report_resource_type_counting() {
        let entry1 = AuditEntry::new("memory.capture", "action");
        let entry2 = AuditEntry::new("memory.capture", "action");
        let entry3 = AuditEntry::new("memory.recall", "action");
        let entry4 = AuditEntry::new("security.pii_detection", "action");

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        let report = AccessReviewReport::generate(&[entry1, entry2, entry3, entry4], start, end);

        assert_eq!(report.by_resource_type.get("memory.capture"), Some(&2));
        assert_eq!(report.by_resource_type.get("memory.recall"), Some(&1));
        assert_eq!(
            report.by_resource_type.get("security.pii_detection"),
            Some(&1)
        );
    }

    #[test]
    fn test_audit_logger_generate_access_review() {
        let logger = AuditLogger::new();

        // Log some events
        logger.log_capture("mem_1", "decisions");
        logger.log_recall("test query", 5);
        logger.log_denied("capture", "secrets detected");

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        let report = logger.generate_access_review(start, end);

        assert_eq!(report.total_events, 3);
        assert_eq!(report.by_outcome.success, 2);
        assert_eq!(report.by_outcome.denied, 1);
    }

    #[test]
    fn test_access_review_report_serialization() {
        let entry = AuditEntry::new("memory.capture", "capture")
            .with_actor("user1")
            .with_outcome(AuditOutcome::Success);

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        let report = AccessReviewReport::generate(&[entry], start, end);

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("generated_at"));
        assert!(json.contains("period_start"));
        assert!(json.contains("period_end"));
        assert!(json.contains("total_events"));
        assert!(json.contains("by_actor"));
        assert!(json.contains("by_outcome"));

        // Verify it can be deserialized
        let deserialized: AccessReviewReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total_events, report.total_events);
    }

    // PII Disclosure Logging Tests

    #[test]
    fn test_log_pii_disclosure() {
        let logger = AuditLogger::new();
        let pii_types = vec!["Email Address".to_string(), "Name".to_string()];

        logger.log_pii_disclosure(
            "anthropic",
            &pii_types,
            Some("user_hash_123"),
            "llm_processing",
            "consent",
        );

        let entries = logger.recent_entries(10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, "security.pii_disclosure");
        assert_eq!(entries[0].action, "disclose");

        let metadata = &entries[0].metadata;
        assert_eq!(metadata["destination"], "anthropic");
        assert_eq!(metadata["pii_count"], 2);
        assert_eq!(metadata["purpose"], "llm_processing");
        assert_eq!(metadata["legal_basis"], "consent");
        assert_eq!(metadata["data_subject_id"], "user_hash_123");
    }

    #[test]
    fn test_log_pii_disclosure_without_data_subject() {
        let logger = AuditLogger::new();
        let pii_types = vec!["IP Address".to_string()];

        logger.log_pii_disclosure(
            "openai",
            &pii_types,
            None,
            "embedding",
            "legitimate_interest",
        );

        let entries = logger.recent_entries(10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].metadata["destination"], "openai");
        assert!(entries[0].metadata["data_subject_id"].is_null());
        assert_eq!(entries[0].metadata["purpose"], "embedding");
        assert_eq!(entries[0].metadata["legal_basis"], "legitimate_interest");
    }

    #[test]
    fn test_log_pii_disclosure_multiple_destinations() {
        let logger = AuditLogger::new();
        let pii_types = vec!["Name".to_string()];

        logger.log_pii_disclosure("anthropic", &pii_types, None, "enrichment", "consent");
        logger.log_pii_disclosure("openai", &pii_types, None, "enrichment", "consent");
        logger.log_pii_disclosure("ollama", &pii_types, None, "enrichment", "consent");

        let entries = logger.recent_entries(10);
        assert_eq!(entries.len(), 3);

        // Verify different destinations
        let destinations: Vec<_> = entries
            .iter()
            .map(|e| e.metadata["destination"].as_str().unwrap())
            .collect();
        assert!(destinations.contains(&"anthropic"));
        assert!(destinations.contains(&"openai"));
        assert!(destinations.contains(&"ollama"));
    }

    #[test]
    fn test_log_bulk_pii_disclosure() {
        let logger = AuditLogger::new();
        let pii_categories = vec![
            "Personal Identifiers".to_string(),
            "Contact Information".to_string(),
        ];

        logger.log_bulk_pii_disclosure(
            "remote_sync",
            100,
            &pii_categories,
            "backup",
            "legitimate_interest",
        );

        let entries = logger.recent_entries(10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, "security.pii_bulk_disclosure");
        assert_eq!(entries[0].action, "bulk_disclose");

        let metadata = &entries[0].metadata;
        assert_eq!(metadata["destination"], "remote_sync");
        assert_eq!(metadata["record_count"], 100);
        assert_eq!(metadata["purpose"], "backup");
        assert_eq!(metadata["legal_basis"], "legitimate_interest");
    }

    #[test]
    fn test_log_bulk_pii_disclosure_zero_records() {
        let logger = AuditLogger::new();
        let pii_categories = vec!["Names".to_string()];

        logger.log_bulk_pii_disclosure("api_export", 0, &pii_categories, "export", "consent");

        let entries = logger.recent_entries(10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].metadata["record_count"], 0);
    }

    #[test]
    fn test_pii_disclosure_timestamp_included() {
        let logger = AuditLogger::new();
        let pii_types = vec!["Email".to_string()];

        let before = Utc::now();
        logger.log_pii_disclosure("provider", &pii_types, None, "purpose", "basis");
        let after = Utc::now();

        let entries = logger.recent_entries(10);
        assert_eq!(entries.len(), 1);

        // Verify timestamp_utc is in metadata
        let timestamp_str = entries[0].metadata["timestamp_utc"].as_str().unwrap();
        let timestamp: DateTime<Utc> = timestamp_str.parse().unwrap();
        assert!(timestamp >= before && timestamp <= after);
    }

    #[test]
    fn test_pii_disclosure_in_access_review() {
        let logger = AuditLogger::new();
        let pii_types = vec!["SSN".to_string()];

        logger.log_pii_disclosure("external_api", &pii_types, None, "verification", "consent");
        logger.log_bulk_pii_disclosure("backup", 50, &pii_types, "archival", "legitimate_interest");

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        let report = logger.generate_access_review(start, end);

        assert_eq!(report.total_events, 2);
        assert_eq!(
            report.by_resource_type.get("security.pii_disclosure"),
            Some(&1)
        );
        assert_eq!(
            report.by_resource_type.get("security.pii_bulk_disclosure"),
            Some(&1)
        );
    }
}
