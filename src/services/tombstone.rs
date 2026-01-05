//! Tombstone operations for soft-delete functionality (ADR-0053).

use crate::models::{EventMeta, MemoryEvent, MemoryId, MemoryStatus};
use crate::observability::current_request_id;
use crate::security::record_event;
use crate::storage::traits::PersistenceBackend;
use crate::{Error, Result};
use chrono::{TimeZone, Utc};
use std::sync::Arc;
use std::time::Duration;
use tracing::instrument;

/// Service for tombstone operations (soft deletes).
pub struct TombstoneService {
    persistence: Arc<dyn PersistenceBackend>,
}

impl TombstoneService {
    /// Creates a new tombstone service.
    #[must_use]
    pub fn new(persistence: Arc<dyn PersistenceBackend>) -> Self {
        Self { persistence }
    }

    /// Tombstones a memory (soft delete).
    ///
    /// Sets status to Tombstoned and records the timestamp.
    ///
    /// # Errors
    ///
    /// Returns an error if the memory cannot be found or updated.
    #[instrument(skip(self), fields(memory_id = %id.as_str()))]
    pub fn tombstone_memory(&self, id: &MemoryId) -> Result<()> {
        // Get the current memory
        let mut memory = self
            .persistence
            .get(id)?
            .ok_or_else(|| Error::OperationFailed {
                operation: "tombstone_memory".to_string(),
                cause: format!("Memory not found: {}", id.as_str()),
            })?;

        // Set tombstone status and timestamp
        let now = crate::current_timestamp();
        let now_i64 = i64::try_from(now).unwrap_or(i64::MAX);
        let now_dt = Utc
            .timestamp_opt(now_i64, 0)
            .single()
            .unwrap_or_else(Utc::now);

        memory.status = MemoryStatus::Tombstoned;
        memory.tombstoned_at = Some(now_dt);
        memory.updated_at = now;

        // Update in persistence
        self.persistence.store(&memory)?;

        record_event(MemoryEvent::Updated {
            meta: EventMeta::with_timestamp("tombstone", current_request_id(), now),
            memory_id: memory.id.clone(),
            modified_fields: vec!["status".to_string(), "tombstoned_at".to_string()],
        });

        tracing::info!(
            memory_id = %id.as_str(),
            tombstoned_at = now,
            "Tombstoned memory"
        );

        metrics::counter!("tombstone_memory_total").increment(1);
        Ok(())
    }

    /// Untombstones a memory (restore from soft delete).
    ///
    /// Sets status back to Active and clears the tombstone timestamp.
    ///
    /// # Errors
    ///
    /// Returns an error if the memory cannot be found or updated.
    #[instrument(skip(self), fields(memory_id = %id.as_str()))]
    pub fn untombstone_memory(&self, id: &MemoryId) -> Result<()> {
        // Get the current memory
        let mut memory = self
            .persistence
            .get(id)?
            .ok_or_else(|| Error::OperationFailed {
                operation: "untombstone_memory".to_string(),
                cause: format!("Memory not found: {}", id.as_str()),
            })?;

        // Clear tombstone status and timestamp
        memory.status = MemoryStatus::Active;
        memory.tombstoned_at = None;
        memory.updated_at = crate::current_timestamp();

        // Update in persistence
        self.persistence.store(&memory)?;

        record_event(MemoryEvent::Updated {
            meta: EventMeta::with_timestamp("tombstone", current_request_id(), memory.updated_at),
            memory_id: memory.id.clone(),
            modified_fields: vec!["status".to_string(), "tombstoned_at".to_string()],
        });

        tracing::info!(
            memory_id = %id.as_str(),
            "Untombstoned memory"
        );

        metrics::counter!("untombstone_memory_total").increment(1);
        Ok(())
    }

    /// Purges tombstoned memories older than the specified duration.
    ///
    /// Permanently deletes memories that have been tombstoned for longer
    /// than the threshold.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion operation fails.
    #[instrument(skip(self), fields(older_than_secs = older_than.as_secs()))]
    pub fn purge_tombstoned(&self, older_than: Duration) -> Result<usize> {
        let threshold = crate::current_timestamp().saturating_sub(older_than.as_secs());
        let threshold_i64 = i64::try_from(threshold).unwrap_or(i64::MAX);

        // List all memory IDs and check each
        let all_ids = self.persistence.list_ids()?;

        let mut purged = 0;
        for id in all_ids {
            if let Some(memory) = self.persistence.get(&id)? {
                if memory.status == MemoryStatus::Tombstoned {
                    if let Some(ts) = memory.tombstoned_at {
                        if ts.timestamp() < threshold_i64 {
                            self.persistence.delete(&memory.id)?;
                            record_event(MemoryEvent::Deleted {
                                meta: EventMeta::new("tombstone", current_request_id()),
                                memory_id: memory.id.clone(),
                                reason: "purge_tombstoned".to_string(),
                            });
                            purged += 1;
                        }
                    }
                }
            }
        }

        tracing::info!(
            purged,
            threshold,
            older_than_secs = older_than.as_secs(),
            "Purged tombstoned memories"
        );

        metrics::counter!("purge_tombstoned_total").increment(purged as u64);
        Ok(purged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, Memory, Namespace};
    use crate::storage::persistence::FilesystemBackend;
    use tempfile::TempDir;

    fn create_test_memory(id: &str) -> Memory {
        Memory {
            id: MemoryId::new(id),
            content: "Test content".to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: 1_000_000,
            updated_at: 1_000_000,
            tombstoned_at: None,
            embedding: None,
            tags: vec![],
            source: None,
        }
    }

    #[test]
    fn test_tombstone_memory() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemBackend::new(dir.path());
        let service = TombstoneService::new(Arc::new(backend));

        // Create and store a memory
        let memory = create_test_memory("test-1");
        service.persistence.store(&memory).unwrap();

        // Tombstone it
        service.tombstone_memory(&memory.id).unwrap();

        // Verify status and timestamp
        let retrieved = service.persistence.get(&memory.id).unwrap().unwrap();
        assert_eq!(retrieved.status, MemoryStatus::Tombstoned);
        assert!(retrieved.tombstoned_at.is_some());
    }

    #[test]
    fn test_untombstone_memory() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemBackend::new(dir.path());
        let service = TombstoneService::new(Arc::new(backend));

        // Create, store, and tombstone
        let memory = create_test_memory("test-2");
        service.persistence.store(&memory).unwrap();
        service.tombstone_memory(&memory.id).unwrap();

        // Untombstone
        service.untombstone_memory(&memory.id).unwrap();

        // Verify status and timestamp cleared
        let retrieved = service.persistence.get(&memory.id).unwrap().unwrap();
        assert_eq!(retrieved.status, MemoryStatus::Active);
        assert_eq!(retrieved.tombstoned_at, None);
    }

    #[test]
    fn test_purge_tombstoned() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemBackend::new(dir.path());
        let service = TombstoneService::new(Arc::new(backend));

        // Create memories with different tombstone times
        let old_memory = Memory {
            id: MemoryId::new("old"),
            status: MemoryStatus::Tombstoned,
            tombstoned_at: Some(Utc.timestamp_opt(100, 0).unwrap()), // Very old
            ..create_test_memory("old")
        };

        let recent_memory = Memory {
            id: MemoryId::new("recent"),
            status: MemoryStatus::Tombstoned,
            tombstoned_at: Some(Utc::now() - chrono::Duration::seconds(1)),
            ..create_test_memory("recent")
        };

        service.persistence.store(&old_memory).unwrap();
        service.persistence.store(&recent_memory).unwrap();

        // Purge memories older than 30 days
        let purged = service
            .purge_tombstoned(Duration::from_secs(30 * 24 * 60 * 60))
            .unwrap();

        // Old should be purged, recent should remain
        assert_eq!(purged, 1);
        assert!(service.persistence.get(&old_memory.id).unwrap().is_none());
        assert!(
            service
                .persistence
                .get(&recent_memory.id)
                .unwrap()
                .is_some()
        );
    }
}
