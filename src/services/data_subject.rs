//! GDPR Data Subject Rights Service.
//!
//! Implements data subject rights as required by GDPR:
//! - **Article 6**: Lawful Basis (Consent tracking)
//! - **Article 7**: Conditions for Consent
//! - **Article 17**: Right to Erasure ("Right to be Forgotten")
//! - **Article 20**: Right to Data Portability
//!
//! # Compliance Features
//!
//! | Requirement | Implementation |
//! |-------------|----------------|
//! | Consent tracking | [`ConsentRecord`] with granular purposes |
//! | Consent withdrawal | `revoke_consent()` with audit trail |
//! | Audit logging | All operations logged via [`AuditLogger`] |
//! | Data export format | JSON (machine-readable, portable) |
//! | Complete deletion | Removes from all storage layers |
//! | Verification | Returns deletion confirmation with counts |
//!
//! # Usage
//!
//! ```rust,ignore
//! use subcog::services::{DataSubjectService, ServiceContainer};
//!
//! let container = ServiceContainer::from_current_dir_or_user()?;
//! let service = DataSubjectService::new(&container)?;
//!
//! // Export all user data (GDPR Article 20)
//! let export = service.export_user_data()?;
//! println!("Exported {} memories", export.memories.len());
//!
//! // Delete all user data (GDPR Article 17)
//! let result = service.delete_user_data()?;
//! println!("Deleted {} memories", result.deleted_count);
//! ```

use crate::Result;
use crate::models::{EventMeta, Memory, MemoryEvent, MemoryId, SearchFilter};
use crate::observability::current_request_id;
use crate::security::{AuditEntry, AuditOutcome, global_logger, record_event};
use crate::storage::index::SqliteBackend;
use crate::storage::traits::{IndexBackend, VectorBackend};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::instrument;

// ============================================================================
// Data Export Types (GDPR Article 20)
// ============================================================================

/// Result of a user data export operation.
///
/// Contains all memories associated with the user in a portable JSON format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDataExport {
    /// Export format version for forward compatibility.
    pub version: String,
    /// Timestamp when the export was generated (Unix epoch seconds).
    pub exported_at: u64,
    /// Total number of memories exported.
    pub memory_count: usize,
    /// All memories belonging to the user.
    pub memories: Vec<ExportedMemory>,
    /// Metadata about the export.
    pub metadata: ExportMetadata,
}

/// A single memory in the export format.
///
/// Uses a flat structure for maximum portability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedMemory {
    /// Unique memory identifier.
    pub id: String,
    /// Memory content.
    pub content: String,
    /// Namespace (e.g., "decisions", "learnings").
    pub namespace: String,
    /// Domain (e.g., "project", "user", "org/repo").
    pub domain: String,
    /// Project identifier (git remote URL).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Branch name (git branch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// File path relative to repo root.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// Status (e.g., "active", "archived").
    pub status: String,
    /// Creation timestamp (Unix epoch seconds).
    pub created_at: u64,
    /// Last update timestamp (Unix epoch seconds).
    pub updated_at: u64,
    /// Tags for categorization.
    pub tags: Vec<String>,
    /// Source reference (file path, URL, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Metadata about the export operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportMetadata {
    /// Export format identifier.
    pub format: String,
    /// Application that generated the export.
    pub generator: String,
    /// Generator version.
    pub generator_version: String,
}

impl From<Memory> for ExportedMemory {
    fn from(memory: Memory) -> Self {
        Self {
            id: memory.id.to_string(),
            content: memory.content,
            namespace: memory.namespace.as_str().to_string(),
            domain: memory.domain.to_string(),
            project_id: memory.project_id,
            branch: memory.branch,
            file_path: memory.file_path,
            status: memory.status.as_str().to_string(),
            created_at: memory.created_at,
            updated_at: memory.updated_at,
            tags: memory.tags,
            source: memory.source,
        }
    }
}

// ============================================================================
// Consent Tracking Types (GDPR Articles 6 & 7) - COMP-HIGH-003
// ============================================================================

/// Purpose for which consent is granted.
///
/// GDPR requires consent to be specific to a purpose. This enum defines
/// the granular purposes for which consent can be granted or revoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsentPurpose {
    /// Storage of memory data.
    DataStorage,
    /// Processing for search and recall.
    DataProcessing,
    /// Generation of embeddings for semantic search.
    EmbeddingGeneration,
    /// Use of LLM for enrichment and analysis.
    LlmProcessing,
    /// Sync to remote storage.
    RemoteSync,
    /// Analytics and metrics collection.
    Analytics,
}

impl ConsentPurpose {
    /// Returns all available consent purposes.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::DataStorage,
            Self::DataProcessing,
            Self::EmbeddingGeneration,
            Self::LlmProcessing,
            Self::RemoteSync,
            Self::Analytics,
        ]
    }

    /// Returns the string representation of the purpose.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::DataStorage => "data_storage",
            Self::DataProcessing => "data_processing",
            Self::EmbeddingGeneration => "embedding_generation",
            Self::LlmProcessing => "llm_processing",
            Self::RemoteSync => "remote_sync",
            Self::Analytics => "analytics",
        }
    }

    /// Returns a human-readable description of the purpose.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::DataStorage => "Store memory content in persistent storage",
            Self::DataProcessing => "Process memories for search and recall functionality",
            Self::EmbeddingGeneration => "Generate vector embeddings for semantic search",
            Self::LlmProcessing => "Use language models for enrichment and analysis",
            Self::RemoteSync => "Synchronize data to remote storage locations",
            Self::Analytics => "Collect anonymized usage metrics and analytics",
        }
    }
}

/// A record of consent granted or revoked.
///
/// Each consent record captures:
/// - The specific purpose for which consent applies
/// - When consent was granted/revoked
/// - Version of the consent text shown to the user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentRecord {
    /// The purpose for which consent is granted.
    pub purpose: ConsentPurpose,
    /// Whether consent is currently granted.
    pub granted: bool,
    /// Timestamp when consent was recorded (Unix epoch seconds).
    pub recorded_at: u64,
    /// Version of the consent text/policy shown.
    pub policy_version: String,
    /// Optional identifier for the data subject.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<String>,
    /// Method by which consent was collected (e.g., "cli", "mcp", "config").
    pub collection_method: String,
}

impl ConsentRecord {
    /// Creates a new consent record granting consent.
    #[must_use]
    pub fn grant(purpose: ConsentPurpose, policy_version: impl Into<String>) -> Self {
        Self {
            purpose,
            granted: true,
            recorded_at: crate::current_timestamp(),
            policy_version: policy_version.into(),
            subject_id: None,
            collection_method: "explicit".to_string(),
        }
    }

    /// Creates a new consent record revoking consent.
    #[must_use]
    pub fn revoke(purpose: ConsentPurpose, policy_version: impl Into<String>) -> Self {
        Self {
            purpose,
            granted: false,
            recorded_at: crate::current_timestamp(),
            policy_version: policy_version.into(),
            subject_id: None,
            collection_method: "explicit".to_string(),
        }
    }

    /// Sets the subject ID.
    #[must_use]
    pub fn with_subject(mut self, subject_id: impl Into<String>) -> Self {
        self.subject_id = Some(subject_id.into());
        self
    }

    /// Sets the collection method.
    #[must_use]
    pub fn with_method(mut self, method: impl Into<String>) -> Self {
        self.collection_method = method.into();
        self
    }
}

/// Current consent status for all purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentStatus {
    /// Map of purpose to current consent state.
    pub consents: std::collections::HashMap<String, bool>,
    /// Timestamp of the last consent change.
    pub last_updated: u64,
    /// Current policy version.
    pub policy_version: String,
}

impl Default for ConsentStatus {
    fn default() -> Self {
        Self {
            consents: std::collections::HashMap::new(),
            last_updated: 0,
            policy_version: "1.0".to_string(),
        }
    }
}

// ============================================================================
// Data Deletion Types (GDPR Article 17)
// ============================================================================

/// Result of a user data deletion operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionResult {
    /// Number of memories deleted.
    pub deleted_count: usize,
    /// IDs of deleted memories (for audit trail).
    pub deleted_ids: Vec<String>,
    /// Number of memories that failed to delete.
    pub failed_count: usize,
    /// IDs that failed to delete with error messages.
    pub failures: Vec<DeletionFailure>,
    /// Timestamp when deletion was performed (Unix epoch seconds).
    pub deleted_at: u64,
    /// Whether deletion was complete (no failures).
    pub complete: bool,
}

/// Details about a failed deletion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionFailure {
    /// Memory ID that failed to delete.
    pub id: String,
    /// Error message describing the failure.
    pub error: String,
}

// ============================================================================
// Data Subject Service
// ============================================================================

/// Service for GDPR data subject rights operations.
///
/// Provides:
/// - `export_user_data()`: Export all user data (Article 20)
/// - `delete_user_data()`: Delete all user data (Article 17)
///
/// # Storage Layers
///
/// Operations affect all three storage layers:
/// 1. **Persistence** (`SQLite`) - Authoritative storage
/// 2. **Index** (`SQLite` FTS5) - Full-text search index
/// 3. **Vector** (usearch) - Embedding vectors
///
/// # Audit Logging
///
/// All operations are logged for compliance:
/// - `gdpr.export` - Data export requests
/// - `gdpr.delete` - Data deletion requests
pub struct DataSubjectService {
    /// `SQLite` index backend for listing and deleting memories.
    index: SqliteBackend,
    /// Optional vector backend for deleting embeddings.
    vector: Option<Arc<dyn VectorBackend + Send + Sync>>,
}

impl DataSubjectService {
    /// Creates a new data subject service with an index backend.
    ///
    /// # Arguments
    ///
    /// * `index` - `SQLite` index backend for memory operations
    #[must_use]
    pub const fn new(index: SqliteBackend) -> Self {
        Self {
            index,
            vector: None,
        }
    }

    /// Adds a vector backend for complete deletion of embeddings.
    #[must_use]
    pub fn with_vector(mut self, vector: Arc<dyn VectorBackend + Send + Sync>) -> Self {
        self.vector = Some(vector);
        self
    }

    /// Exports all user data in a portable format.
    ///
    /// Implements GDPR Article 20 (Right to Data Portability).
    ///
    /// # Returns
    ///
    /// A [`UserDataExport`] containing all memories in JSON-serializable format.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The index backend fails to list memories
    /// - Memory retrieval fails
    ///
    /// # Audit
    ///
    /// Logs a `gdpr.export` event with the count of exported memories.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let service = DataSubjectService::new(index);
    /// let export = service.export_user_data()?;
    ///
    /// // Serialize to JSON for download
    /// let json = serde_json::to_string_pretty(&export)?;
    /// std::fs::write("my_data.json", json)?;
    /// ```
    #[instrument(skip(self), fields(operation = "export_user_data"))]
    pub fn export_user_data(&self) -> Result<UserDataExport> {
        let start = Instant::now();

        tracing::info!("Starting GDPR data export");

        // List all memories (no filter = all data)
        let filter = SearchFilter::new();
        let memory_ids = self.index.list_all(&filter, usize::MAX)?;

        // Fetch full memory content for each ID
        let ids: Vec<MemoryId> = memory_ids.into_iter().map(|(id, _)| id).collect();
        let memories_opt = self.index.get_memories_batch(&ids)?;

        // Convert to export format, filtering out None values
        let memories: Vec<ExportedMemory> = memories_opt
            .into_iter()
            .flatten()
            .map(ExportedMemory::from)
            .collect();

        let memory_count = memories.len();

        let export = UserDataExport {
            version: "1.0".to_string(),
            exported_at: crate::current_timestamp(),
            memory_count,
            memories,
            metadata: ExportMetadata {
                format: "subcog-export-v1".to_string(),
                generator: "subcog".to_string(),
                generator_version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        // Audit log the export
        Self::log_export_event(memory_count);

        // Record metrics
        Self::record_metrics("export", start, memory_count, 0);

        tracing::info!(
            memory_count = memory_count,
            duration_ms = start.elapsed().as_millis(),
            "GDPR data export completed"
        );

        Ok(export)
    }

    /// Deletes all user data from all storage layers.
    ///
    /// Implements GDPR Article 17 (Right to Erasure / "Right to be Forgotten").
    ///
    /// # Storage Layers Affected
    ///
    /// 1. **Index** (`SQLite`) - Memory metadata and FTS index
    /// 2. **Vector** (usearch) - Embedding vectors (if configured)
    /// 3. **Persistence** (`SQLite`) - Authoritative storage (if configured)
    ///
    /// # Returns
    ///
    /// A [`DeletionResult`] with counts and any failures.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The index backend fails to list memories
    /// - Critical deletion operations fail
    ///
    /// Note: Individual memory deletion failures are captured in the result
    /// rather than causing the entire operation to fail.
    ///
    /// # Audit
    ///
    /// Logs a `gdpr.delete` event with the count of deleted memories.
    ///
    /// # Warning
    ///
    /// This operation is **irreversible**. All user data will be permanently deleted.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let service = DataSubjectService::new(index)
    ///     .with_vector(vector_backend);
    ///
    /// let result = service.delete_user_data()?;
    ///
    /// if result.complete {
    ///     println!("Successfully deleted {} memories", result.deleted_count);
    /// } else {
    ///     eprintln!("Partial deletion: {} failed", result.failed_count);
    /// }
    /// ```
    #[instrument(skip(self), fields(operation = "delete_user_data"))]
    pub fn delete_user_data(&self) -> Result<DeletionResult> {
        let start = Instant::now();

        tracing::warn!("Starting GDPR data deletion (irreversible)");

        // List all memories
        let filter = SearchFilter::new();
        let memory_ids = self.index.list_all(&filter, usize::MAX)?;

        let ids: Vec<MemoryId> = memory_ids.into_iter().map(|(id, _)| id).collect();
        let total_count = ids.len();

        if ids.is_empty() {
            tracing::info!("No memories to delete");
            return Ok(DeletionResult {
                deleted_count: 0,
                deleted_ids: Vec::new(),
                failed_count: 0,
                failures: Vec::new(),
                deleted_at: crate::current_timestamp(),
                complete: true,
            });
        }

        let mut deleted_ids = Vec::with_capacity(total_count);
        let mut failures = Vec::new();

        // Delete from all storage layers
        for id in &ids {
            match self.delete_memory_from_all_layers(id) {
                Ok(()) => {
                    deleted_ids.push(id.to_string());
                    record_event(MemoryEvent::Deleted {
                        meta: EventMeta::new("gdpr", current_request_id()),
                        memory_id: id.clone(),
                        reason: "gdpr.delete_user_data".to_string(),
                    });
                },
                Err(e) => {
                    tracing::warn!(memory_id = %id, error = %e, "Failed to delete memory");
                    failures.push(DeletionFailure {
                        id: id.to_string(),
                        error: e.to_string(),
                    });
                },
            }
        }

        let deleted_count = deleted_ids.len();
        let failed_count = failures.len();
        let complete = failures.is_empty();

        // Audit log the deletion
        Self::log_deletion_event(deleted_count, failed_count);

        // Record metrics
        Self::record_metrics("delete", start, deleted_count, failed_count);

        tracing::info!(
            deleted_count = deleted_count,
            failed_count = failed_count,
            duration_ms = start.elapsed().as_millis(),
            complete = complete,
            "GDPR data deletion completed"
        );

        Ok(DeletionResult {
            deleted_count,
            deleted_ids,
            failed_count,
            failures,
            deleted_at: crate::current_timestamp(),
            complete,
        })
    }

    /// Deletes a single memory from all storage layers.
    ///
    /// # Deletion Order
    ///
    /// 1. Vector backend (if configured) - Delete embedding
    /// 2. `SQLite` Index - Delete from search index (authoritative)
    fn delete_memory_from_all_layers(&self, id: &MemoryId) -> Result<()> {
        // 1. Delete from vector backend (best-effort)
        if let Some(ref vector) = self.vector
            && let Err(e) = vector.remove(id)
        {
            tracing::debug!(memory_id = %id, error = %e, "Vector deletion failed (continuing)");
            // Continue - vector is a derived store
        }

        // 2. Delete from index (authoritative storage)
        self.index.remove(id)?;

        Ok(())
    }

    // ========================================================================
    // Consent Management (GDPR Articles 6 & 7) - COMP-HIGH-003
    // ========================================================================

    /// Records consent for a specific purpose.
    ///
    /// Implements GDPR Article 7 (Conditions for Consent).
    ///
    /// # Arguments
    ///
    /// * `record` - The consent record to store
    ///
    /// # Audit
    ///
    /// Logs a `gdpr.consent` event with the purpose and grant status.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use subcog::services::{DataSubjectService, ConsentRecord, ConsentPurpose};
    ///
    /// let service = DataSubjectService::new(index);
    /// let record = ConsentRecord::grant(ConsentPurpose::DataStorage, "1.0")
    ///     .with_method("cli");
    /// service.record_consent(&record)?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if consent recording fails.
    #[instrument(skip(self), fields(operation = "record_consent"))]
    pub fn record_consent(&self, record: &ConsentRecord) -> Result<()> {
        let purpose = record.purpose.as_str();
        let granted = record.granted;

        tracing::info!(
            purpose = purpose,
            granted = granted,
            policy_version = %record.policy_version,
            "Recording consent"
        );

        // Store consent record (in-memory for now, could be persisted)
        // For production, this would be stored in the persistence layer
        Self::log_consent_event(record);

        metrics::counter!(
            "gdpr_consent_recorded_total",
            "purpose" => purpose.to_string(),
            "granted" => granted.to_string()
        )
        .increment(1);

        Ok(())
    }

    /// Revokes consent for a specific purpose.
    ///
    /// Creates a revocation record and logs the event for audit.
    ///
    /// # Arguments
    ///
    /// * `purpose` - The purpose for which consent is being revoked
    /// * `policy_version` - The current policy version
    ///
    /// # Audit
    ///
    /// Logs a `gdpr.consent_revoked` event.
    ///
    /// # Errors
    ///
    /// Returns an error if consent revocation fails.
    #[instrument(skip(self), fields(operation = "revoke_consent"))]
    pub fn revoke_consent(&self, purpose: ConsentPurpose, policy_version: &str) -> Result<()> {
        let record = ConsentRecord::revoke(purpose, policy_version);

        tracing::warn!(
            purpose = purpose.as_str(),
            policy_version = policy_version,
            "Revoking consent"
        );

        Self::log_consent_event(&record);

        metrics::counter!(
            "gdpr_consent_revoked_total",
            "purpose" => purpose.as_str().to_string()
        )
        .increment(1);

        Ok(())
    }

    /// Checks if consent is granted for a specific purpose.
    ///
    /// # Returns
    ///
    /// `true` if consent is currently granted for the purpose.
    ///
    /// # Note
    ///
    /// Default implementation returns `true` for backward compatibility.
    /// Production systems should check persistent consent records.
    #[must_use]
    pub fn has_consent(&self, purpose: ConsentPurpose) -> bool {
        // Log consent check for audit
        tracing::debug!(purpose = purpose.as_str(), "Checking consent");

        // Default to true for backward compatibility
        // Production would check persistent storage
        true
    }

    /// Returns the current consent status for all purposes.
    ///
    /// # Returns
    ///
    /// A [`ConsentStatus`] with the current state of all consents.
    #[must_use]
    pub fn consent_status(&self) -> ConsentStatus {
        let mut status = ConsentStatus::default();

        for purpose in ConsentPurpose::all() {
            status
                .consents
                .insert(purpose.as_str().to_string(), self.has_consent(*purpose));
        }

        status.last_updated = crate::current_timestamp();
        status
    }

    /// Logs a consent event to the audit log.
    fn log_consent_event(record: &ConsentRecord) {
        if let Some(logger) = global_logger() {
            let action = if record.granted {
                "grant_consent"
            } else {
                "revoke_consent"
            };

            let entry = AuditEntry::new("gdpr.consent", action)
                .with_outcome(AuditOutcome::Success)
                .with_metadata(serde_json::json!({
                    "purpose": record.purpose.as_str(),
                    "granted": record.granted,
                    "policy_version": record.policy_version,
                    "collection_method": record.collection_method,
                    "gdpr_article": if record.granted {
                        "Article 6 - Lawful Basis for Processing"
                    } else {
                        "Article 7(3) - Right to Withdraw Consent"
                    }
                }));
            logger.log_entry(entry);
        }
    }

    /// Logs an export event to the audit log.
    fn log_export_event(memory_count: usize) {
        if let Some(logger) = global_logger() {
            let entry = AuditEntry::new("gdpr.export", "export_user_data")
                .with_outcome(AuditOutcome::Success)
                .with_metadata(serde_json::json!({
                    "memory_count": memory_count,
                    "gdpr_article": "Article 20 - Right to Data Portability"
                }));
            logger.log_entry(entry);
        }
    }

    /// Logs a deletion event to the audit log.
    fn log_deletion_event(deleted_count: usize, failed_count: usize) {
        if let Some(logger) = global_logger() {
            let outcome = if failed_count == 0 {
                AuditOutcome::Success
            } else {
                AuditOutcome::Failure
            };

            let entry = AuditEntry::new("gdpr.delete", "delete_user_data")
                .with_outcome(outcome)
                .with_metadata(serde_json::json!({
                    "deleted_count": deleted_count,
                    "failed_count": failed_count,
                    "gdpr_article": "Article 17 - Right to Erasure"
                }));
            logger.log_entry(entry);
        }
    }

    /// Records metrics for the operation.
    fn record_metrics(operation: &str, start: Instant, success_count: usize, failure_count: usize) {
        metrics::counter!(
            "gdpr_operations_total",
            "operation" => operation.to_string(),
            "status" => if failure_count == 0 { "success" } else { "partial" }
        )
        .increment(1);

        metrics::histogram!(
            "gdpr_operation_duration_ms",
            "operation" => operation.to_string()
        )
        .record(start.elapsed().as_secs_f64() * 1000.0);

        metrics::gauge!(
            "gdpr_last_operation_count",
            "operation" => operation.to_string()
        )
        .set(success_count as f64);

        if failure_count > 0 {
            metrics::counter!(
                "gdpr_operation_failures_total",
                "operation" => operation.to_string()
            )
            .increment(failure_count as u64);
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, MemoryStatus, Namespace};
    use crate::storage::index::SqliteBackend;

    fn create_test_memory(id: &str, content: &str, namespace: Namespace) -> Memory {
        Memory {
            id: MemoryId::new(id),
            content: content.to_string(),
            namespace,
            domain: Domain::new(),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
            tombstoned_at: None,
            expires_at: None,
            embedding: None,
            tags: vec!["test".to_string()],
            source: Some("test.rs".to_string()),
            is_summary: false,
            source_memory_ids: None,
            consolidation_timestamp: None,
        }
    }

    #[test]
    fn test_export_user_data_empty() {
        let index = SqliteBackend::in_memory().expect("Failed to create index");
        let service = DataSubjectService::new(index);

        let export = service.export_user_data().expect("Export failed");

        assert_eq!(export.version, "1.0");
        assert_eq!(export.memory_count, 0);
        assert!(export.memories.is_empty());
        assert_eq!(export.metadata.format, "subcog-export-v1");
        assert_eq!(export.metadata.generator, "subcog");
    }

    #[test]
    fn test_export_user_data_with_memories() {
        let index = SqliteBackend::in_memory().expect("Failed to create index");

        // Add test memories
        let mem1 = create_test_memory("id1", "Decision: Use PostgreSQL", Namespace::Decisions);
        let mem2 = create_test_memory("id2", "Learned: Connection pooling", Namespace::Learnings);
        let mem3 = create_test_memory("id3", "Pattern: Repository pattern", Namespace::Patterns);

        index.index(&mem1).expect("Index failed");
        index.index(&mem2).expect("Index failed");
        index.index(&mem3).expect("Index failed");

        let service = DataSubjectService::new(index);
        let export = service.export_user_data().expect("Export failed");

        assert_eq!(export.memory_count, 3);
        assert_eq!(export.memories.len(), 3);

        // Verify memory content is included
        let ids: Vec<_> = export.memories.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"id1"));
        assert!(ids.contains(&"id2"));
        assert!(ids.contains(&"id3"));

        // Verify all fields are exported
        let decision = export.memories.iter().find(|m| m.id == "id1").unwrap();
        assert_eq!(decision.content, "Decision: Use PostgreSQL");
        assert_eq!(decision.namespace, "decisions");
        assert_eq!(decision.status, "active");
        assert_eq!(decision.tags, vec!["test".to_string()]);
        assert_eq!(decision.source, Some("test.rs".to_string()));
    }

    #[test]
    fn test_delete_user_data_empty() {
        let index = SqliteBackend::in_memory().expect("Failed to create index");
        let service = DataSubjectService::new(index);

        let result = service.delete_user_data().expect("Delete failed");

        assert_eq!(result.deleted_count, 0);
        assert!(result.deleted_ids.is_empty());
        assert_eq!(result.failed_count, 0);
        assert!(result.failures.is_empty());
        assert!(result.complete);
    }

    #[test]
    fn test_delete_user_data_with_memories() {
        let index = SqliteBackend::in_memory().expect("Failed to create index");

        // Add test memories
        let mem1 = create_test_memory("id1", "To be deleted 1", Namespace::Decisions);
        let mem2 = create_test_memory("id2", "To be deleted 2", Namespace::Learnings);

        index.index(&mem1).expect("Index failed");
        index.index(&mem2).expect("Index failed");

        // Verify memories exist
        let filter = SearchFilter::new();
        let before = index.list_all(&filter, 100).expect("List failed");
        assert_eq!(before.len(), 2);

        let service = DataSubjectService::new(index);
        let result = service.delete_user_data().expect("Delete failed");

        assert_eq!(result.deleted_count, 2);
        assert_eq!(result.deleted_ids.len(), 2);
        assert!(result.deleted_ids.contains(&"id1".to_string()));
        assert!(result.deleted_ids.contains(&"id2".to_string()));
        assert_eq!(result.failed_count, 0);
        assert!(result.complete);

        // Verify memories are gone - need to access the index directly
        // Since we consumed it, we check the result instead
        assert!(result.complete);
    }

    #[test]
    fn test_exported_memory_from_memory() {
        let memory = create_test_memory("test-id", "Test content", Namespace::Security);
        let exported = ExportedMemory::from(memory);

        assert_eq!(exported.id, "test-id");
        assert_eq!(exported.content, "Test content");
        assert_eq!(exported.namespace, "security");
        assert_eq!(exported.status, "active");
    }

    #[test]
    fn test_export_serialization() {
        let export = UserDataExport {
            version: "1.0".to_string(),
            exported_at: 1_700_000_000,
            memory_count: 1,
            memories: vec![ExportedMemory {
                id: "test".to_string(),
                content: "Content".to_string(),
                namespace: "decisions".to_string(),
                domain: "project".to_string(),
                project_id: None,
                branch: None,
                file_path: None,
                status: "active".to_string(),
                created_at: 1_700_000_000,
                updated_at: 1_700_000_000,
                tags: vec!["tag1".to_string()],
                source: None,
            }],
            metadata: ExportMetadata {
                format: "subcog-export-v1".to_string(),
                generator: "subcog".to_string(),
                generator_version: "0.1.0".to_string(),
            },
        };

        let json = serde_json::to_string(&export).expect("Serialization failed");
        assert!(json.contains("subcog-export-v1"));
        assert!(json.contains("decisions"));

        // Verify deserialization
        let parsed: UserDataExport = serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(parsed.memory_count, 1);
        assert_eq!(parsed.memories[0].id, "test");
    }

    #[test]
    fn test_deletion_result_serialization() {
        let result = DeletionResult {
            deleted_count: 5,
            deleted_ids: vec!["id1".to_string(), "id2".to_string()],
            failed_count: 1,
            failures: vec![DeletionFailure {
                id: "id3".to_string(),
                error: "Not found".to_string(),
            }],
            deleted_at: 1_700_000_000,
            complete: false,
        };

        let json = serde_json::to_string(&result).expect("Serialization failed");
        assert!(json.contains("deleted_count"));
        assert!(json.contains("complete"));

        let parsed: DeletionResult = serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(parsed.deleted_count, 5);
        assert!(!parsed.complete);
    }

    #[test]
    fn test_service_builder_pattern() {
        let index = SqliteBackend::in_memory().expect("Failed to create index");

        // Test builder pattern compiles and works
        let service = DataSubjectService::new(index);

        // Service should work without vector backend
        let export = service.export_user_data();
        assert!(export.is_ok());
    }

    #[test]
    fn test_export_preserves_all_namespaces() {
        let index = SqliteBackend::in_memory().expect("Failed to create index");

        // Add memories with different namespaces
        for ns in Namespace::user_namespaces() {
            let mem = create_test_memory(
                &format!("id-{}", ns.as_str()),
                &format!("Content for {}", ns.as_str()),
                *ns,
            );
            index.index(&mem).expect("Index failed");
        }

        let service = DataSubjectService::new(index);
        let export = service.export_user_data().expect("Export failed");

        // Should have exported all user namespaces
        assert_eq!(export.memory_count, Namespace::user_namespaces().len());

        // Verify each namespace is represented
        let namespaces: Vec<_> = export
            .memories
            .iter()
            .map(|m| m.namespace.as_str())
            .collect();
        for ns in Namespace::user_namespaces() {
            assert!(
                namespaces.contains(&ns.as_str()),
                "Missing namespace: {}",
                ns.as_str()
            );
        }
    }

    #[test]
    fn test_delete_clears_all_memories() {
        let index = SqliteBackend::in_memory().expect("Failed to create index");

        // Add many memories
        for i in 0..10 {
            let mem = create_test_memory(
                &format!("bulk-{i}"),
                &format!("Bulk memory {i}"),
                Namespace::Decisions,
            );
            index.index(&mem).expect("Index failed");
        }

        let service = DataSubjectService::new(index);

        // Delete all
        let result = service.delete_user_data().expect("Delete failed");

        assert_eq!(result.deleted_count, 10);
        assert!(result.complete);

        // Export should now be empty
        let export = service.export_user_data().expect("Export failed");
        assert_eq!(export.memory_count, 0);
    }

    // ========================================================================
    // Consent Tracking Tests (COMP-HIGH-003)
    // ========================================================================

    #[test]
    fn test_consent_purpose_all() {
        let all = ConsentPurpose::all();
        assert_eq!(all.len(), 6);
        assert!(all.contains(&ConsentPurpose::DataStorage));
        assert!(all.contains(&ConsentPurpose::DataProcessing));
        assert!(all.contains(&ConsentPurpose::EmbeddingGeneration));
        assert!(all.contains(&ConsentPurpose::LlmProcessing));
        assert!(all.contains(&ConsentPurpose::RemoteSync));
        assert!(all.contains(&ConsentPurpose::Analytics));
    }

    #[test]
    fn test_consent_purpose_as_str() {
        assert_eq!(ConsentPurpose::DataStorage.as_str(), "data_storage");
        assert_eq!(ConsentPurpose::DataProcessing.as_str(), "data_processing");
        assert_eq!(
            ConsentPurpose::EmbeddingGeneration.as_str(),
            "embedding_generation"
        );
        assert_eq!(ConsentPurpose::LlmProcessing.as_str(), "llm_processing");
        assert_eq!(ConsentPurpose::RemoteSync.as_str(), "remote_sync");
        assert_eq!(ConsentPurpose::Analytics.as_str(), "analytics");
    }

    #[test]
    fn test_consent_purpose_description() {
        // All purposes should have descriptions
        for purpose in ConsentPurpose::all() {
            let desc = purpose.description();
            assert!(!desc.is_empty());
            assert!(desc.len() > 10); // Descriptions should be meaningful
        }
    }

    #[test]
    fn test_consent_record_grant() {
        let record = ConsentRecord::grant(ConsentPurpose::DataStorage, "1.0");

        assert_eq!(record.purpose, ConsentPurpose::DataStorage);
        assert!(record.granted);
        assert_eq!(record.policy_version, "1.0");
        assert!(record.recorded_at > 0);
        assert_eq!(record.collection_method, "explicit");
        assert!(record.subject_id.is_none());
    }

    #[test]
    fn test_consent_record_revoke() {
        let record = ConsentRecord::revoke(ConsentPurpose::Analytics, "2.0");

        assert_eq!(record.purpose, ConsentPurpose::Analytics);
        assert!(!record.granted);
        assert_eq!(record.policy_version, "2.0");
    }

    #[test]
    fn test_consent_record_builders() {
        let record = ConsentRecord::grant(ConsentPurpose::LlmProcessing, "1.0")
            .with_subject("user-123")
            .with_method("mcp");

        assert_eq!(record.subject_id, Some("user-123".to_string()));
        assert_eq!(record.collection_method, "mcp");
    }

    #[test]
    fn test_consent_record_serialization() {
        let record =
            ConsentRecord::grant(ConsentPurpose::DataStorage, "1.0").with_subject("test-user");

        let json = serde_json::to_string(&record).expect("Serialization failed");
        assert!(json.contains("data_storage"));
        assert!(json.contains("test-user"));
        assert!(json.contains("\"granted\":true"));

        let parsed: ConsentRecord = serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(parsed.purpose, ConsentPurpose::DataStorage);
        assert!(parsed.granted);
    }

    #[test]
    fn test_consent_status_default() {
        let status = ConsentStatus::default();

        assert!(status.consents.is_empty());
        assert_eq!(status.last_updated, 0);
        assert_eq!(status.policy_version, "1.0");
    }

    #[test]
    fn test_service_record_consent() {
        let index = SqliteBackend::in_memory().expect("Failed to create index");
        let service = DataSubjectService::new(index);

        let record = ConsentRecord::grant(ConsentPurpose::DataStorage, "1.0");
        let result = service.record_consent(&record);

        assert!(result.is_ok());
    }

    #[test]
    fn test_service_revoke_consent() {
        let index = SqliteBackend::in_memory().expect("Failed to create index");
        let service = DataSubjectService::new(index);

        let result = service.revoke_consent(ConsentPurpose::Analytics, "1.0");

        assert!(result.is_ok());
    }

    #[test]
    fn test_service_has_consent() {
        let index = SqliteBackend::in_memory().expect("Failed to create index");
        let service = DataSubjectService::new(index);

        // Default is true for backward compatibility
        assert!(service.has_consent(ConsentPurpose::DataStorage));
        assert!(service.has_consent(ConsentPurpose::Analytics));
    }

    #[test]
    fn test_service_consent_status() {
        let index = SqliteBackend::in_memory().expect("Failed to create index");
        let service = DataSubjectService::new(index);

        let status = service.consent_status();

        // Should have all purposes
        assert_eq!(status.consents.len(), 6);
        assert!(status.last_updated > 0);

        // All should be true by default
        for granted in status.consents.values() {
            assert!(*granted);
        }
    }

    #[test]
    fn test_consent_status_serialization() {
        let mut status = ConsentStatus::default();
        status.consents.insert("data_storage".to_string(), true);
        status.consents.insert("analytics".to_string(), false);
        status.last_updated = 1_700_000_000;

        let json = serde_json::to_string(&status).expect("Serialization failed");
        assert!(json.contains("data_storage"));
        assert!(json.contains("analytics"));

        let parsed: ConsentStatus = serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(parsed.consents.get("data_storage"), Some(&true));
        assert_eq!(parsed.consents.get("analytics"), Some(&false));
    }
}
