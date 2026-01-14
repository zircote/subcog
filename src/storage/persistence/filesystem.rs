//! Filesystem-based persistence backend.
//!
//! A fallback backend that stores memories as individual JSON files.
//! Useful for testing and environments without git.
//!
//! # Security
//!
//! This module includes protections against filesystem-based attacks:
//! - **Path traversal**: Memory IDs are validated to prevent directory escape
//! - **File size limits**: Maximum file size enforced to prevent memory exhaustion
//! - **Encryption at rest**: Optional AES-256-GCM encryption (CRIT-005)
//!
//! # Encryption
//!
//! When the `encryption` feature is enabled and `SUBCOG_ENCRYPTION_KEY` is set,
//! all memory files are encrypted with AES-256-GCM before writing to disk.
//!
//! ```bash
//! # Generate a key
//! openssl rand -base64 32
//!
//! # Enable encryption
//! export SUBCOG_ENCRYPTION_KEY="your-base64-encoded-key"
//! ```

use crate::models::{Memory, MemoryId};
use crate::security::encryption::is_encrypted;
#[cfg(feature = "encryption")]
use crate::security::encryption::{EncryptionConfig, Encryptor};
use crate::storage::traits::PersistenceBackend;
use crate::{Error, Result};
use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Maximum file size for memory files (1MB).
/// Prevents memory exhaustion from maliciously large files.
const MAX_FILE_SIZE: u64 = 1024 * 1024;

/// Serializable memory format for filesystem storage.
#[derive(Debug, Serialize, Deserialize)]
struct StoredMemory {
    id: String,
    content: String,
    namespace: String,
    domain_org: Option<String>,
    domain_project: Option<String>,
    domain_repo: Option<String>,
    project_id: Option<String>,
    branch: Option<String>,
    file_path: Option<String>,
    status: String,
    created_at: u64,
    updated_at: u64,
    #[serde(default)]
    tombstoned_at: Option<u64>,
    /// Expiration timestamp (Unix epoch seconds).
    #[serde(default)]
    expires_at: Option<u64>,
    embedding: Option<Vec<f32>>,
    tags: Vec<String>,
    source: Option<String>,
    /// Whether this memory is a consolidation summary node.
    #[serde(default)]
    is_summary: bool,
    /// IDs of memories that were consolidated into this summary.
    #[serde(default)]
    source_memory_ids: Option<Vec<String>>,
    /// Timestamp when this memory was consolidated.
    #[serde(default)]
    consolidation_timestamp: Option<u64>,
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
            project_id: m.project_id.clone(),
            branch: m.branch.clone(),
            file_path: m.file_path.clone(),
            status: m.status.as_str().to_string(),
            created_at: m.created_at,
            updated_at: m.updated_at,
            tombstoned_at: m
                .tombstoned_at
                .and_then(|ts| u64::try_from(ts.timestamp()).ok()),
            expires_at: m.expires_at,
            embedding: m.embedding.clone(),
            tags: m.tags.clone(),
            source: m.source.clone(),
            is_summary: m.is_summary,
            source_memory_ids: m
                .source_memory_ids
                .as_ref()
                .map(|ids| ids.iter().map(|id| id.as_str().to_string()).collect()),
            consolidation_timestamp: m.consolidation_timestamp,
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
            "tombstoned" => MemoryStatus::Tombstoned,
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
            project_id: self.project_id.clone(),
            branch: self.branch.clone(),
            file_path: self.file_path.clone(),
            status,
            created_at: self.created_at,
            updated_at: self.updated_at,
            tombstoned_at: self.tombstoned_at.and_then(|ts| {
                let ts_i64 = i64::try_from(ts).unwrap_or(i64::MAX);
                Utc.timestamp_opt(ts_i64, 0).single()
            }),
            expires_at: self.expires_at,
            embedding: self.embedding.clone(),
            tags: self.tags.clone(),
            #[cfg(feature = "group-scope")]
            group_id: None, // TODO: Add group_id to filesystem persistence
            source: self.source.clone(),
            is_summary: self.is_summary,
            source_memory_ids: self
                .source_memory_ids
                .as_ref()
                .map(|ids| ids.iter().map(MemoryId::new).collect()),
            consolidation_timestamp: self.consolidation_timestamp,
        }
    }
}

/// Filesystem-based persistence backend.
pub struct FilesystemBackend {
    /// Base directory for storage.
    base_path: PathBuf,
    /// Optional encryptor for encryption at rest.
    #[cfg(feature = "encryption")]
    encryptor: Option<Encryptor>,
}

impl FilesystemBackend {
    /// Creates a new filesystem backend.
    ///
    /// If the `encryption` feature is enabled and `SUBCOG_ENCRYPTION_KEY` is set,
    /// encryption at rest is automatically enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        let path = base_path.into();

        // Try to create directory, ignore errors for now
        let _ = fs::create_dir_all(&path);

        #[cfg(feature = "encryption")]
        let encryptor = Self::try_create_encryptor();

        Self {
            base_path: path,
            #[cfg(feature = "encryption")]
            encryptor,
        }
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

        #[cfg(feature = "encryption")]
        let encryptor = Self::try_create_encryptor();

        Ok(Self {
            base_path,
            #[cfg(feature = "encryption")]
            encryptor,
        })
    }

    /// Tries to create an encryptor from environment configuration.
    #[cfg(feature = "encryption")]
    fn try_create_encryptor() -> Option<Encryptor> {
        EncryptionConfig::try_from_env().map_or_else(
            || {
                tracing::debug!("Encryption key not configured, storing files unencrypted");
                None
            },
            |config| match Encryptor::new(config) {
                Ok(enc) => {
                    tracing::info!("Encryption at rest enabled for filesystem backend");
                    Some(enc)
                },
                Err(e) => {
                    tracing::warn!("Failed to create encryptor: {e}");
                    None
                },
            },
        )
    }

    /// Decrypts data if it's encrypted, returns as-is otherwise.
    ///
    /// This helper reduces nesting in the `get` method (`clippy::excessive_nesting` fix).
    #[cfg(feature = "encryption")]
    fn decrypt_if_needed(&self, raw_data: Vec<u8>) -> Result<Vec<u8>> {
        if !is_encrypted(&raw_data) {
            return Ok(raw_data);
        }
        self.encryptor.as_ref().map_or_else(
            || {
                Err(Error::OperationFailed {
                    operation: "decrypt_memory".to_string(),
                    cause: "File is encrypted but no encryption key configured".to_string(),
                })
            },
            |encryptor| encryptor.decrypt(&raw_data),
        )
    }

    /// Decrypts data if it's encrypted, returns as-is otherwise.
    ///
    /// Non-encryption version - returns error if data is encrypted.
    #[cfg(not(feature = "encryption"))]
    fn decrypt_if_needed(&self, raw_data: Vec<u8>) -> Result<Vec<u8>> {
        if is_encrypted(&raw_data) {
            return Err(Error::OperationFailed {
                operation: "decrypt_memory".to_string(),
                cause: "File is encrypted but encryption feature not enabled".to_string(),
            });
        }
        Ok(raw_data)
    }

    /// Returns whether encryption is enabled.
    #[cfg(feature = "encryption")]
    #[must_use]
    pub const fn encryption_enabled(&self) -> bool {
        self.encryptor.is_some()
    }

    /// Returns whether encryption is enabled.
    #[cfg(not(feature = "encryption"))]
    #[must_use]
    pub const fn encryption_enabled(&self) -> bool {
        false
    }

    /// Returns the path for a memory file.
    ///
    /// # Security
    ///
    /// The memory ID is sanitized to prevent path traversal attacks.
    /// Only alphanumeric characters, dashes, and underscores are allowed.
    fn memory_path(&self, id: &MemoryId) -> Result<PathBuf> {
        let id_str = id.as_str();

        // Validate ID to prevent path traversal attacks (PEN-H2)
        if !Self::is_safe_filename(id_str) {
            return Err(Error::InvalidInput(format!(
                "Memory ID contains invalid characters: {id_str}",
            )));
        }

        let path = self.base_path.join(format!("{id_str}.json"));

        // Double-check: ensure the resulting path is under base_path
        // Note: We compare the non-canonical paths because:
        // 1. The ID validation above prevents ".." and "/" in the filename
        // 2. The file may not exist yet (for store operations)
        // 3. Canonicalization would fail for non-existent files
        // The is_safe_filename check is the primary security barrier
        if !path.starts_with(&self.base_path) {
            return Err(Error::InvalidInput(format!(
                "Path traversal attempt detected for ID: {id_str}",
            )));
        }

        Ok(path)
    }

    /// Checks if a filename is safe (no path traversal).
    fn is_safe_filename(name: &str) -> bool {
        // Only allow alphanumeric, dash, underscore
        // Reject: .. / \ NUL and other special chars
        !name.is_empty()
            && name.len() <= 255
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    }

    /// Returns the base path.
    #[must_use]
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }
}

impl PersistenceBackend for FilesystemBackend {
    fn store(&self, memory: &Memory) -> Result<()> {
        // Ensure directory exists before storing
        let _ = fs::create_dir_all(&self.base_path);

        let path = self.memory_path(&memory.id)?;
        let stored = StoredMemory::from(memory);

        let json = serde_json::to_string_pretty(&stored).map_err(|e| Error::OperationFailed {
            operation: "serialize_memory".to_string(),
            cause: e.to_string(),
        })?;

        // CRIT-005: Encrypt if encryption is enabled
        #[cfg(feature = "encryption")]
        let data = if let Some(ref encryptor) = self.encryptor {
            encryptor.encrypt(json.as_bytes())?
        } else {
            json.into_bytes()
        };

        #[cfg(not(feature = "encryption"))]
        let data = json.into_bytes();

        fs::write(&path, data).map_err(|e| Error::OperationFailed {
            operation: "write_memory_file".to_string(),
            cause: e.to_string(),
        })?;

        Ok(())
    }

    fn get(&self, id: &MemoryId) -> Result<Option<Memory>> {
        let path = match self.memory_path(id) {
            Ok(p) => p,
            Err(_) => return Ok(None), // Invalid ID means no memory
        };

        if !path.exists() {
            return Ok(None);
        }

        // PEN-H4: Validate file size before reading to prevent memory exhaustion
        let metadata = fs::metadata(&path).map_err(|e| Error::OperationFailed {
            operation: "read_file_metadata".to_string(),
            cause: e.to_string(),
        })?;

        if metadata.len() > MAX_FILE_SIZE {
            return Err(Error::InvalidInput(format!(
                "Memory file exceeds maximum size of {MAX_FILE_SIZE} bytes: {}",
                path.display()
            )));
        }

        // Read raw bytes first to detect encryption
        let raw_data = fs::read(&path).map_err(|e| Error::OperationFailed {
            operation: "read_memory_file".to_string(),
            cause: e.to_string(),
        })?;

        // CRIT-005: Decrypt if file is encrypted (uses helper to reduce nesting)
        let json_bytes = self.decrypt_if_needed(raw_data)?;

        let json = String::from_utf8(json_bytes).map_err(|e| Error::OperationFailed {
            operation: "decode_memory_file".to_string(),
            cause: e.to_string(),
        })?;

        let stored: StoredMemory =
            serde_json::from_str(&json).map_err(|e| Error::OperationFailed {
                operation: "deserialize_memory".to_string(),
                cause: e.to_string(),
            })?;

        Ok(Some(stored.to_memory()))
    }

    fn delete(&self, id: &MemoryId) -> Result<bool> {
        let path = match self.memory_path(id) {
            Ok(p) => p,
            Err(_) => return Ok(false), // Invalid ID means nothing to delete
        };

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

            if let Some(id) = extract_memory_id_from_path(&entry.path()) {
                ids.push(id);
            }
        }

        Ok(ids)
    }
}

/// Extracts a memory ID from a JSON file path.
fn extract_memory_id_from_path(path: &Path) -> Option<MemoryId> {
    // Check if it's a JSON file
    if path.extension().is_none_or(|ext| ext != "json") {
        return None;
    }

    // Get the file stem (name without extension) and convert to string
    let stem = path.file_stem()?;
    let id_str = stem.to_str()?;

    Some(MemoryId::new(id_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, MemoryStatus, Namespace};
    use serde_json;
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
            created_at: 1_234_567_890,
            updated_at: 1_234_567_890,
            tombstoned_at: None,
            expires_at: None,
            embedding: None,
            tags: vec!["test".to_string()],
            #[cfg(feature = "group-scope")]
            group_id: None,
            source: Some("test.rs".to_string()),
            is_summary: false,
            source_memory_ids: None,
            consolidation_timestamp: None,
        }
    }

    #[test]
    fn test_store_and_get() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemBackend::new(dir.path());

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
        let backend = FilesystemBackend::new(dir.path());

        let memory = create_test_memory("to_delete");
        backend.store(&memory).unwrap();

        let deleted = backend.delete(&MemoryId::new("to_delete")).unwrap();
        assert!(deleted);

        let retrieved = backend.get(&MemoryId::new("to_delete")).unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_deserialize_without_tombstoned_at() {
        let json = r#"{
            "id": "legacy-id",
            "content": "Legacy content",
            "namespace": "decisions",
            "domain_org": null,
            "domain_project": null,
            "domain_repo": null,
            "project_id": null,
            "branch": null,
            "file_path": null,
            "status": "active",
            "created_at": 123,
            "updated_at": 123,
            "embedding": null,
            "tags": [],
            "source": null
        }"#;

        let stored: StoredMemory = serde_json::from_str(json).unwrap();
        let memory = stored.to_memory();
        assert!(memory.tombstoned_at.is_none());
    }

    #[test]
    fn test_delete_nonexistent() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemBackend::new(dir.path());

        let deleted = backend.delete(&MemoryId::new("nonexistent")).unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_list_ids() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemBackend::new(dir.path());

        backend.store(&create_test_memory("id1")).unwrap();
        backend.store(&create_test_memory("id2")).unwrap();
        backend.store(&create_test_memory("id3")).unwrap();

        let ids = backend.list_ids().unwrap();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn test_count() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemBackend::new(dir.path());

        assert_eq!(backend.count().unwrap(), 0);

        backend.store(&create_test_memory("id1")).unwrap();
        backend.store(&create_test_memory("id2")).unwrap();

        assert_eq!(backend.count().unwrap(), 2);
    }

    #[test]
    fn test_exists() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemBackend::new(dir.path());

        backend.store(&create_test_memory("exists")).unwrap();

        assert!(backend.exists(&MemoryId::new("exists")).unwrap());
        assert!(!backend.exists(&MemoryId::new("not_exists")).unwrap());
    }

    #[test]
    fn test_update_memory() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemBackend::new(dir.path());

        let mut memory = create_test_memory("update_test");
        backend.store(&memory).unwrap();

        memory.content = "Updated content".to_string();
        memory.updated_at = 9_999_999_999;
        backend.store(&memory).unwrap();

        let retrieved = backend.get(&MemoryId::new("update_test")).unwrap().unwrap();
        assert_eq!(retrieved.content, "Updated content");
        assert_eq!(retrieved.updated_at, 9_999_999_999);
    }

    #[test]
    fn test_path_traversal_protection() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemBackend::new(dir.path());

        // Attempt path traversal with ".."
        let result = backend.memory_path(&MemoryId::new("../../../etc/passwd"));
        assert!(result.is_err());

        // Attempt with forward slash
        let result = backend.memory_path(&MemoryId::new("dir/subdir/file"));
        assert!(result.is_err());

        // Attempt with backslash
        let result = backend.memory_path(&MemoryId::new("dir\\subdir\\file"));
        assert!(result.is_err());
    }

    #[test]
    fn test_safe_filename_validation() {
        // Valid filenames
        assert!(FilesystemBackend::is_safe_filename("valid_id"));
        assert!(FilesystemBackend::is_safe_filename("valid-id-123"));
        assert!(FilesystemBackend::is_safe_filename("abc123"));
        assert!(FilesystemBackend::is_safe_filename("UPPERCASE"));

        // Invalid filenames
        assert!(!FilesystemBackend::is_safe_filename(""));
        assert!(!FilesystemBackend::is_safe_filename("../path"));
        assert!(!FilesystemBackend::is_safe_filename("path/to/file"));
        assert!(!FilesystemBackend::is_safe_filename("path\\to\\file"));
        assert!(!FilesystemBackend::is_safe_filename("file.json"));
        assert!(!FilesystemBackend::is_safe_filename("file with space"));
    }

    #[test]
    fn test_with_create_success() {
        let dir = TempDir::new().unwrap();
        let subdir = dir.path().join("subdir");

        let backend = FilesystemBackend::with_create(&subdir);
        assert!(backend.is_ok());
        assert!(subdir.exists());
    }

    #[test]
    fn test_base_path_accessor() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemBackend::new(dir.path());

        assert_eq!(backend.base_path(), dir.path());
    }

    #[test]
    fn test_memory_roundtrip_all_namespaces() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemBackend::new(dir.path());

        let namespaces = [
            Namespace::Decisions,
            Namespace::Patterns,
            Namespace::Learnings,
            Namespace::Context,
            Namespace::TechDebt,
            Namespace::Apis,
            Namespace::Config,
            Namespace::Security,
            Namespace::Performance,
            Namespace::Testing,
        ];

        for (i, ns) in namespaces.iter().enumerate() {
            let id = format!("ns_test_{i}");
            let mut memory = create_test_memory(&id);
            memory.namespace = *ns;

            backend.store(&memory).unwrap();
            let retrieved = backend.get(&MemoryId::new(&id)).unwrap().unwrap();
            assert_eq!(retrieved.namespace, *ns);
        }
    }
}
