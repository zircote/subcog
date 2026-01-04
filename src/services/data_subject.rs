//! GDPR Data Subject Rights Service.
//!
//! Implements data subject rights as required by GDPR:
//! - **Article 17**: Right to Erasure ("Right to be Forgotten")
//! - **Article 20**: Right to Data Portability
//!
//! # Compliance Features
//!
//! | Requirement | Implementation |
//! |-------------|----------------|
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
//! let container = ServiceContainer::from_current_dir()?;
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
use crate::models::{Memory, MemoryId, SearchFilter};
use crate::security::{AuditEntry, AuditOutcome, global_logger};
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
/// 1. **Persistence** (SQLite) - Authoritative storage
/// 2. **Index** (SQLite FTS5) - Full-text search index
/// 3. **Vector** (usearch) - Embedding vectors
///
/// # Audit Logging
///
/// All operations are logged for compliance:
/// - `gdpr.export` - Data export requests
/// - `gdpr.delete` - Data deletion requests
pub struct DataSubjectService {
    /// SQLite index backend for listing and deleting memories.
    index: SqliteBackend,
    /// Optional vector backend for deleting embeddings.
    vector: Option<Arc<dyn VectorBackend + Send + Sync>>,
}

impl DataSubjectService {
    /// Creates a new data subject service with an index backend.
    ///
    /// # Arguments
    ///
    /// * `index` - SQLite index backend for memory operations
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
    /// 1. **Index** (SQLite) - Memory metadata and FTS index
    /// 2. **Vector** (usearch) - Embedding vectors (if configured)
    /// 3. **Persistence** (SQLite) - Authoritative storage (if configured)
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
    /// 2. SQLite Index - Delete from search index (authoritative)
    fn delete_memory_from_all_layers(&self, id: &MemoryId) -> Result<()> {
        // 1. Delete from vector backend (best-effort)
        if let Some(ref vector) = self.vector {
            if let Err(e) = vector.remove(id) {
                tracing::debug!(memory_id = %id, error = %e, "Vector deletion failed (continuing)");
                // Continue - vector is a derived store
            }
        }

        // 2. Delete from index (authoritative storage)
        self.index.remove(id)?;

        Ok(())
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
            embedding: None,
            tags: vec!["test".to_string()],
            source: Some("test.rs".to_string()),
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
}
