//! Filesystem-based persistence backend.
//!
//! A fallback backend that stores memories as individual JSON files.
//! Useful for testing and environments without git.

use crate::models::{Memory, MemoryId};
use crate::storage::traits::PersistenceBackend;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Serializable memory format for filesystem storage.
#[derive(Debug, Serialize, Deserialize)]
struct StoredMemory {
    id: String,
    content: String,
    namespace: String,
    domain_org: Option<String>,
    domain_project: Option<String>,
    domain_repo: Option<String>,
    status: String,
    created_at: u64,
    updated_at: u64,
    embedding: Option<Vec<f32>>,
    tags: Vec<String>,
    source: Option<String>,
}

impl From<&Memory> for StoredMemory {
    fn from(m: &Memory) -> Self {
        Self {
            id: m.id.as_str().to_string(),
            content: m.content.clone(),
            namespace: m.namespace.as_str().to_string(),
            domain_org: m.domain.organization.clone(),
            domain_project: m.domain.project.clone(),
            domain_repo: m.domain.repository.clone(),
            status: m.status.as_str().to_string(),
            created_at: m.created_at,
            updated_at: m.updated_at,
            embedding: m.embedding.clone(),
            tags: m.tags.clone(),
            source: m.source.clone(),
        }
    }
}

impl StoredMemory {
    fn to_memory(&self) -> Memory {
        use crate::models::{Domain, MemoryStatus, Namespace};

        let namespace = match self.namespace.as_str() {
            "decisions" => Namespace::Decisions,
            "patterns" => Namespace::Patterns,
            "learnings" => Namespace::Learnings,
            "context" => Namespace::Context,
            "tech-debt" => Namespace::TechDebt,
            "apis" => Namespace::Apis,
            "config" => Namespace::Config,
            "security" => Namespace::Security,
            "performance" => Namespace::Performance,
            "testing" => Namespace::Testing,
            _ => Namespace::Decisions,
        };

        let status = match self.status.as_str() {
            "active" => MemoryStatus::Active,
            "archived" => MemoryStatus::Archived,
            "superseded" => MemoryStatus::Superseded,
            "pending" => MemoryStatus::Pending,
            "deleted" => MemoryStatus::Deleted,
            _ => MemoryStatus::Active,
        };

        Memory {
            id: MemoryId::new(&self.id),
            content: self.content.clone(),
            namespace,
            domain: Domain {
                organization: self.domain_org.clone(),
                project: self.domain_project.clone(),
                repository: self.domain_repo.clone(),
            },
            status,
            created_at: self.created_at,
            updated_at: self.updated_at,
            embedding: self.embedding.clone(),
            tags: self.tags.clone(),
            source: self.source.clone(),
        }
    }
}

/// Filesystem-based persistence backend.
pub struct FilesystemBackend {
    /// Base directory for storage.
    base_path: PathBuf,
}

impl FilesystemBackend {
    /// Creates a new filesystem backend.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        let path = base_path.into();

        // Try to create directory, ignore errors for now
        let _ = fs::create_dir_all(&path);

        Self { base_path: path }
    }

    /// Creates a new filesystem backend with checked directory creation.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    pub fn with_create(base_path: impl Into<PathBuf>) -> Result<Self> {
        let base_path = base_path.into();

        // Ensure directory exists
        fs::create_dir_all(&base_path).map_err(|e| Error::OperationFailed {
            operation: "create_storage_dir".to_string(),
            cause: e.to_string(),
        })?;

        Ok(Self { base_path })
    }

    /// Returns the path for a memory file.
    fn memory_path(&self, id: &MemoryId) -> PathBuf {
        self.base_path.join(format!("{}.json", id.as_str()))
    }

    /// Returns the base path.
    #[must_use]
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }
}

impl PersistenceBackend for FilesystemBackend {
    fn store(&mut self, memory: &Memory) -> Result<()> {
        // Ensure directory exists before storing
        let _ = fs::create_dir_all(&self.base_path);

        let path = self.memory_path(&memory.id);
        let stored = StoredMemory::from(memory);

        let json = serde_json::to_string_pretty(&stored).map_err(|e| Error::OperationFailed {
            operation: "serialize_memory".to_string(),
            cause: e.to_string(),
        })?;

        fs::write(&path, json).map_err(|e| Error::OperationFailed {
            operation: "write_memory_file".to_string(),
            cause: e.to_string(),
        })?;

        Ok(())
    }

    fn get(&self, id: &MemoryId) -> Result<Option<Memory>> {
        let path = self.memory_path(id);

        if !path.exists() {
            return Ok(None);
        }

        let json = fs::read_to_string(&path).map_err(|e| Error::OperationFailed {
            operation: "read_memory_file".to_string(),
            cause: e.to_string(),
        })?;

        let stored: StoredMemory =
            serde_json::from_str(&json).map_err(|e| Error::OperationFailed {
                operation: "deserialize_memory".to_string(),
                cause: e.to_string(),
            })?;

        Ok(Some(stored.to_memory()))
    }

    fn delete(&mut self, id: &MemoryId) -> Result<bool> {
        let path = self.memory_path(id);

        if !path.exists() {
            return Ok(false);
        }

        fs::remove_file(&path).map_err(|e| Error::OperationFailed {
            operation: "delete_memory_file".to_string(),
            cause: e.to_string(),
        })?;

        Ok(true)
    }

    fn list_ids(&self) -> Result<Vec<MemoryId>> {
        let mut ids = Vec::new();

        // If directory doesn't exist, return empty list
        if !self.base_path.exists() {
            return Ok(ids);
        }

        let entries = fs::read_dir(&self.base_path).map_err(|e| Error::OperationFailed {
            operation: "read_storage_dir".to_string(),
            cause: e.to_string(),
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| Error::OperationFailed {
                operation: "read_dir_entry".to_string(),
                cause: e.to_string(),
            })?;

            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Some(stem) = path.file_stem() {
                    if let Some(id_str) = stem.to_str() {
                        ids.push(MemoryId::new(id_str));
                    }
                }
            }
        }

        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, MemoryStatus, Namespace};
    use tempfile::TempDir;

    fn create_test_memory(id: &str) -> Memory {
        Memory {
            id: MemoryId::new(id),
            content: "Test content".to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            status: MemoryStatus::Active,
            created_at: 1234567890,
            updated_at: 1234567890,
            embedding: None,
            tags: vec!["test".to_string()],
            source: Some("test.rs".to_string()),
        }
    }

    #[test]
    fn test_store_and_get() {
        let dir = TempDir::new().unwrap();
        let mut backend = FilesystemBackend::new(dir.path());

        let memory = create_test_memory("test_id");
        backend.store(&memory).unwrap();

        let retrieved = backend.get(&MemoryId::new("test_id")).unwrap();
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id.as_str(), "test_id");
        assert_eq!(retrieved.content, "Test content");
        assert_eq!(retrieved.namespace, Namespace::Decisions);
    }

    #[test]
    fn test_get_nonexistent() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemBackend::new(dir.path());

        let result = backend.get(&MemoryId::new("nonexistent")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_delete() {
        let dir = TempDir::new().unwrap();
        let mut backend = FilesystemBackend::new(dir.path());

        let memory = create_test_memory("to_delete");
        backend.store(&memory).unwrap();

        let deleted = backend.delete(&MemoryId::new("to_delete")).unwrap();
        assert!(deleted);

        let retrieved = backend.get(&MemoryId::new("to_delete")).unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_delete_nonexistent() {
        let dir = TempDir::new().unwrap();
        let mut backend = FilesystemBackend::new(dir.path());

        let deleted = backend.delete(&MemoryId::new("nonexistent")).unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_list_ids() {
        let dir = TempDir::new().unwrap();
        let mut backend = FilesystemBackend::new(dir.path());

        backend.store(&create_test_memory("id1")).unwrap();
        backend.store(&create_test_memory("id2")).unwrap();
        backend.store(&create_test_memory("id3")).unwrap();

        let ids = backend.list_ids().unwrap();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn test_count() {
        let dir = TempDir::new().unwrap();
        let mut backend = FilesystemBackend::new(dir.path());

        assert_eq!(backend.count().unwrap(), 0);

        backend.store(&create_test_memory("id1")).unwrap();
        backend.store(&create_test_memory("id2")).unwrap();

        assert_eq!(backend.count().unwrap(), 2);
    }

    #[test]
    fn test_exists() {
        let dir = TempDir::new().unwrap();
        let mut backend = FilesystemBackend::new(dir.path());

        backend.store(&create_test_memory("exists")).unwrap();

        assert!(backend.exists(&MemoryId::new("exists")).unwrap());
        assert!(!backend.exists(&MemoryId::new("not_exists")).unwrap());
    }

    #[test]
    fn test_update_memory() {
        let dir = TempDir::new().unwrap();
        let mut backend = FilesystemBackend::new(dir.path());

        let mut memory = create_test_memory("update_test");
        backend.store(&memory).unwrap();

        memory.content = "Updated content".to_string();
        memory.updated_at = 9999999999;
        backend.store(&memory).unwrap();

        let retrieved = backend.get(&MemoryId::new("update_test")).unwrap().unwrap();
        assert_eq!(retrieved.content, "Updated content");
        assert_eq!(retrieved.updated_at, 9999999999);
    }
}
