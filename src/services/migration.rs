//! Migration service.
//!
//! Provides functionality for migrating existing memories to use new features,
//! primarily generating embeddings for memories that lack them.

use std::sync::Arc;

use crate::Result;
use crate::embedding::Embedder;
use crate::models::{Memory, MemoryId};
use crate::storage::{IndexBackend, VectorBackend};

/// Statistics from a migration operation.
#[derive(Debug, Clone, Default)]
pub struct MigrationStats {
    /// Number of memories that were migrated.
    pub migrated: usize,
    /// Number of memories that were skipped.
    pub skipped: usize,
    /// Number of errors encountered.
    pub errors: usize,
    /// Total memories processed.
    pub total: usize,
}

impl MigrationStats {
    /// Creates a new empty migration stats instance.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            migrated: 0,
            skipped: 0,
            errors: 0,
            total: 0,
        }
    }
}

/// Options for migration.
#[derive(Debug, Clone, Default)]
pub struct MigrationOptions {
    /// If true, don't actually modify anything.
    pub dry_run: bool,
    /// If true, re-generate embeddings even for memories that already have them.
    pub force: bool,
    /// Maximum number of memories to process.
    pub limit: usize,
}

impl MigrationOptions {
    /// Creates a new migration options instance with defaults.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            dry_run: false,
            force: false,
            limit: 10000,
        }
    }

    /// Sets the `dry_run` option.
    #[must_use]
    pub const fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Sets the force option.
    #[must_use]
    pub const fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }

    /// Sets the limit option.
    #[must_use]
    pub const fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

/// Migration service for generating embeddings for existing memories.
pub struct MigrationService<I, E, V>
where
    I: IndexBackend,
    E: Embedder,
    V: VectorBackend,
{
    index: Arc<I>,
    embedder: Arc<E>,
    vector: Arc<V>,
}

impl<I, E, V> MigrationService<I, E, V>
where
    I: IndexBackend,
    E: Embedder,
    V: VectorBackend,
{
    /// Creates a new migration service.
    ///
    /// # Arguments
    ///
    /// * `index` - The index backend to read memories from
    /// * `embedder` - The embedder to generate embeddings
    /// * `vector` - The vector backend to store embeddings
    pub const fn new(index: Arc<I>, embedder: Arc<E>, vector: Arc<V>) -> Self {
        Self {
            index,
            embedder,
            vector,
        }
    }

    /// Migrates embeddings for all memories in the index.
    ///
    /// # Arguments
    ///
    /// * `options` - Migration options
    ///
    /// # Returns
    ///
    /// Statistics about the migration.
    ///
    /// # Errors
    ///
    /// Returns an error if the migration fails.
    pub fn migrate_embeddings(&self, options: &MigrationOptions) -> Result<MigrationStats> {
        let filter = crate::SearchFilter::new();
        let memories = self.index.list_all(&filter, options.limit)?;

        let mut stats = MigrationStats {
            total: memories.len(),
            ..Default::default()
        };

        for (memory_id, _score) in memories {
            let result = self.migrate_single(&memory_id, options);
            match result {
                Ok(true) => stats.migrated += 1,
                Ok(false) => stats.skipped += 1,
                Err(e) => {
                    tracing::warn!("Failed to migrate memory {}: {e}", memory_id.as_str());
                    stats.errors += 1;
                },
            }
        }

        Ok(stats)
    }

    /// Migrates a single memory.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - The ID of the memory to migrate
    /// * `options` - Migration options
    ///
    /// # Returns
    ///
    /// `true` if the memory was migrated, `false` if it was skipped.
    ///
    /// # Errors
    ///
    /// Returns an error if the migration fails.
    pub fn migrate_single(&self, memory_id: &MemoryId, options: &MigrationOptions) -> Result<bool> {
        // Get the full memory
        let memory = match self.index.get_memory(memory_id)? {
            Some(m) => m,
            None => return Ok(false),
        };

        // Check if already has embedding (unless force)
        if !options.force && memory.embedding.is_some() {
            return Ok(false);
        }

        if options.dry_run {
            tracing::debug!("Would migrate: {}", memory_id.as_str());
            return Ok(true);
        }

        // Generate embedding
        let embedding = self.embedder.embed(&memory.content)?;

        // Store in vector backend
        self.vector.upsert(memory_id, &embedding)?;

        Ok(true)
    }

    /// Checks if a memory needs migration.
    ///
    /// # Arguments
    ///
    /// * `memory` - The memory to check
    /// * `force` - Whether to force re-migration
    ///
    /// # Returns
    ///
    /// `true` if the memory needs migration.
    #[must_use]
    pub const fn needs_migration(memory: &Memory, force: bool) -> bool {
        force || memory.embedding.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_stats_new() {
        let stats = MigrationStats::new();
        assert_eq!(stats.migrated, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.errors, 0);
        assert_eq!(stats.total, 0);
    }

    #[test]
    fn test_migration_stats_default() {
        let stats = MigrationStats::default();
        assert_eq!(stats.migrated, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.errors, 0);
        assert_eq!(stats.total, 0);
    }

    #[test]
    fn test_migration_options_new() {
        let options = MigrationOptions::new();
        assert!(!options.dry_run);
        assert!(!options.force);
        assert_eq!(options.limit, 10000);
    }

    #[test]
    fn test_migration_options_default() {
        let options = MigrationOptions::default();
        assert!(!options.dry_run);
        assert!(!options.force);
        assert_eq!(options.limit, 0);
    }

    #[test]
    fn test_migration_options_with_dry_run() {
        let options = MigrationOptions::new().with_dry_run(true);
        assert!(options.dry_run);
        assert!(!options.force);
    }

    #[test]
    fn test_migration_options_with_force() {
        let options = MigrationOptions::new().with_force(true);
        assert!(!options.dry_run);
        assert!(options.force);
    }

    #[test]
    fn test_migration_options_with_limit() {
        let options = MigrationOptions::new().with_limit(100);
        assert_eq!(options.limit, 100);
    }

    #[test]
    fn test_migration_options_chaining() {
        let options = MigrationOptions::new()
            .with_dry_run(true)
            .with_force(true)
            .with_limit(50);
        assert!(options.dry_run);
        assert!(options.force);
        assert_eq!(options.limit, 50);
    }

    fn create_test_memory(id: &str, with_embedding: bool) -> Memory {
        Memory {
            id: MemoryId::new(id),
            namespace: crate::Namespace::Decisions,
            content: "test content".to_string(),
            domain: crate::Domain::new(),
            project_id: None,
            branch: None,
            file_path: None,
            status: crate::MemoryStatus::Active,
            created_at: 0,
            updated_at: 0,
            tombstoned_at: None,
            expires_at: None,
            embedding: if with_embedding {
                Some(vec![0.1, 0.2, 0.3])
            } else {
                None
            },
            tags: vec![],
            source: None,
            is_summary: false,
            source_memory_ids: None,
            consolidation_timestamp: None,
        }
    }

    #[test]
    fn test_needs_migration_no_embedding() {
        let memory = create_test_memory("test", false);
        assert!(MigrationService::<
            crate::storage::index::SqliteBackend,
            crate::embedding::FastEmbedEmbedder,
            crate::storage::vector::UsearchBackend,
        >::needs_migration(&memory, false));
    }

    #[test]
    fn test_needs_migration_has_embedding() {
        let memory = create_test_memory("test", true);
        assert!(!MigrationService::<
            crate::storage::index::SqliteBackend,
            crate::embedding::FastEmbedEmbedder,
            crate::storage::vector::UsearchBackend,
        >::needs_migration(&memory, false));
    }

    #[test]
    fn test_needs_migration_force() {
        let memory = create_test_memory("test", true);
        assert!(MigrationService::<
            crate::storage::index::SqliteBackend,
            crate::embedding::FastEmbedEmbedder,
            crate::storage::vector::UsearchBackend,
        >::needs_migration(&memory, true));
    }
}
