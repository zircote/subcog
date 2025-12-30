//! Git notes persistence backend.
//!
//! This is the primary persistence backend for subcog.
//! Memories are stored as git notes attached to a dedicated ref.

use crate::git::{NotesManager, YamlFrontMatterParser};
use crate::models::{Domain, Memory, MemoryId, MemoryStatus, Namespace};
use crate::storage::traits::PersistenceBackend;
use crate::{Error, Result};
use std::collections::HashMap;
use std::path::PathBuf;

/// Git notes-based persistence backend.
pub struct GitNotesBackend {
    /// Path to the git repository.
    repo_path: PathBuf,
    /// Git notes ref (e.g., "refs/notes/subcog").
    notes_ref: String,
    /// Notes manager instance.
    notes_manager: NotesManager,
    /// In-memory index of memory ID to commit ID mappings.
    id_mapping: HashMap<String, String>,
}

impl GitNotesBackend {
    /// Creates a new git notes backend.
    #[must_use]
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        let path = repo_path.into();
        let notes_manager = NotesManager::new(&path);
        Self {
            repo_path: path,
            notes_ref: NotesManager::DEFAULT_NOTES_REF.to_string(),
            notes_manager,
            id_mapping: HashMap::new(),
        }
    }

    /// Sets a custom notes ref.
    #[must_use]
    pub fn with_notes_ref(mut self, notes_ref: impl Into<String>) -> Self {
        let ref_str = notes_ref.into();
        self.notes_ref = ref_str.clone();
        self.notes_manager = NotesManager::new(&self.repo_path).with_notes_ref(ref_str);
        self
    }

    /// Returns the repository path.
    #[must_use]
    pub fn repo_path(&self) -> &PathBuf {
        &self.repo_path
    }

    /// Returns the notes ref.
    #[must_use]
    pub fn notes_ref(&self) -> &str {
        &self.notes_ref
    }

    /// Builds the index of memory IDs from existing notes.
    ///
    /// # Errors
    ///
    /// Returns an error if notes cannot be read.
    pub fn build_index(&mut self) -> Result<()> {
        self.id_mapping.clear();

        let notes = self.notes_manager.list()?;

        for (commit_id, content) in notes {
            if let Ok((metadata, _)) = YamlFrontMatterParser::parse(&content) {
                if let Some(id) = metadata.get("id").and_then(|v| v.as_str()) {
                    self.id_mapping.insert(id.to_string(), commit_id);
                }
            }
        }

        Ok(())
    }

    /// Serializes a memory to YAML front matter format.
    fn serialize_memory(memory: &Memory) -> Result<String> {
        let metadata = serde_json::json!({
            "id": memory.id.as_str(),
            "namespace": memory.namespace.as_str(),
            "domain": memory.domain.to_string(),
            "status": memory.status.as_str(),
            "created_at": memory.created_at,
            "updated_at": memory.updated_at,
            "tags": memory.tags
        });

        YamlFrontMatterParser::serialize(&metadata, &memory.content)
    }

    /// Deserializes a memory from YAML front matter format.
    fn deserialize_memory(content: &str) -> Result<Memory> {
        let (metadata, body) = YamlFrontMatterParser::parse(content)?;

        let id = metadata
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing memory ID in metadata".to_string()))?;

        let namespace_str = metadata
            .get("namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("decisions");

        let namespace = parse_namespace(namespace_str);

        let domain_str = metadata
            .get("domain")
            .and_then(|v| v.as_str())
            .unwrap_or("global");

        let domain = parse_domain(domain_str);

        let status_str = metadata
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("active");

        let status = parse_status(status_str);

        let created_at = metadata
            .get("created_at")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let updated_at = metadata
            .get("updated_at")
            .and_then(|v| v.as_u64())
            .unwrap_or(created_at);

        let tags: Vec<String> = metadata
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let source = metadata
            .get("source")
            .and_then(|v| v.as_str())
            .map(String::from);

        Ok(Memory {
            id: MemoryId::new(id),
            content: body,
            namespace,
            domain,
            status,
            created_at,
            updated_at,
            embedding: None,
            tags,
            source,
        })
    }
}

/// Parses a namespace string to Namespace enum.
fn parse_namespace(s: &str) -> Namespace {
    match s.to_lowercase().as_str() {
        "decisions" => Namespace::Decisions,
        "patterns" => Namespace::Patterns,
        "learnings" => Namespace::Learnings,
        "context" => Namespace::Context,
        "tech-debt" | "techdebt" => Namespace::TechDebt,
        "apis" => Namespace::Apis,
        "config" => Namespace::Config,
        "security" => Namespace::Security,
        "performance" => Namespace::Performance,
        "testing" => Namespace::Testing,
        _ => Namespace::Decisions,
    }
}

/// Parses a status string to MemoryStatus enum.
fn parse_status(s: &str) -> MemoryStatus {
    match s.to_lowercase().as_str() {
        "active" => MemoryStatus::Active,
        "archived" => MemoryStatus::Archived,
        "superseded" => MemoryStatus::Superseded,
        "pending" => MemoryStatus::Pending,
        "deleted" => MemoryStatus::Deleted,
        _ => MemoryStatus::Active,
    }
}

/// Parses a domain string to Domain struct.
fn parse_domain(s: &str) -> Domain {
    if s == "global" || s.is_empty() {
        return Domain::new();
    }

    let parts: Vec<&str> = s.split('/').collect();
    match parts.len() {
        1 => Domain {
            organization: Some(parts[0].to_string()),
            project: None,
            repository: None,
        },
        2 => Domain {
            organization: Some(parts[0].to_string()),
            project: None,
            repository: Some(parts[1].to_string()),
        },
        3 => Domain {
            organization: Some(parts[0].to_string()),
            project: Some(parts[1].to_string()),
            repository: Some(parts[2].to_string()),
        },
        _ => Domain::new(),
    }
}

impl PersistenceBackend for GitNotesBackend {
    fn store(&mut self, memory: &Memory) -> Result<()> {
        let content = Self::serialize_memory(memory)?;

        // Add note to HEAD
        let _note_oid = self.notes_manager.add_to_head(&content)?;

        // Update our in-memory mapping
        // For simplicity, we use the memory ID as the key and store a placeholder
        // In production, we would track which commit each memory is attached to
        self.id_mapping
            .insert(memory.id.as_str().to_string(), "HEAD".to_string());

        Ok(())
    }

    fn get(&self, id: &MemoryId) -> Result<Option<Memory>> {
        // First check our mapping
        if !self.id_mapping.contains_key(id.as_str()) {
            // Try to find by scanning all notes
            let notes = self.notes_manager.list()?;

            for (_, content) in notes {
                if let Ok((metadata, _)) = YamlFrontMatterParser::parse(&content) {
                    if let Some(note_id) = metadata.get("id").and_then(|v| v.as_str()) {
                        if note_id == id.as_str() {
                            return Self::deserialize_memory(&content).map(Some);
                        }
                    }
                }
            }

            return Ok(None);
        }

        // Get from HEAD (simplified - in production we'd use the actual commit ID)
        let content = self.notes_manager.get_from_head()?;

        match content {
            Some(c) => {
                let memory = Self::deserialize_memory(&c)?;
                if memory.id.as_str() == id.as_str() {
                    Ok(Some(memory))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    fn delete(&mut self, id: &MemoryId) -> Result<bool> {
        // For git notes, we don't actually delete - we mark as deleted
        // A proper implementation would need to track the commit ID
        if self.id_mapping.remove(id.as_str()).is_some() {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn list_ids(&self) -> Result<Vec<MemoryId>> {
        let notes = self.notes_manager.list()?;
        let mut ids = Vec::new();

        for (_, content) in notes {
            if let Ok((metadata, _)) = YamlFrontMatterParser::parse(&content) {
                if let Some(id) = metadata.get("id").and_then(|v| v.as_str()) {
                    ids.push(MemoryId::new(id));
                }
            }
        }

        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Repository, Signature};
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, Repository) {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        // Create an initial commit in a separate scope so tree is dropped before returning
        {
            let sig = Signature::now("test", "test@test.com").unwrap();
            let tree_id = repo.index().unwrap().write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                .unwrap();
        }

        (dir, repo)
    }

    fn create_test_memory() -> Memory {
        Memory {
            id: MemoryId::new("test_memory_123"),
            content: "Use PostgreSQL for primary storage".to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::for_repository("zircote", "subcog"),
            status: MemoryStatus::Active,
            created_at: 1234567890,
            updated_at: 1234567890,
            embedding: None,
            tags: vec!["database".to_string(), "architecture".to_string()],
            source: Some("src/main.rs".to_string()),
        }
    }

    #[test]
    fn test_git_notes_backend_creation() {
        let backend = GitNotesBackend::new("/tmp/test");
        assert_eq!(backend.notes_ref(), NotesManager::DEFAULT_NOTES_REF);

        let custom = GitNotesBackend::new("/tmp/test").with_notes_ref("refs/notes/custom");
        assert_eq!(custom.notes_ref(), "refs/notes/custom");
    }

    #[test]
    fn test_serialize_memory() {
        let memory = create_test_memory();
        let serialized = GitNotesBackend::serialize_memory(&memory).unwrap();

        assert!(serialized.contains("---"));
        assert!(serialized.contains("namespace: decisions"));
        assert!(serialized.contains("Use PostgreSQL"));
    }

    #[test]
    fn test_deserialize_memory() {
        let content = r#"---
id: test_123
namespace: decisions
domain: zircote/subcog
status: active
created_at: 1234567890
updated_at: 1234567890
tags:
  - rust
  - memory
---
This is the memory content."#;

        let memory = GitNotesBackend::deserialize_memory(content).unwrap();
        assert_eq!(memory.id.as_str(), "test_123");
        assert_eq!(memory.namespace, Namespace::Decisions);
        assert_eq!(memory.content, "This is the memory content.");
        assert_eq!(memory.tags.len(), 2);
    }

    #[test]
    fn test_store_and_list() {
        let (dir, _repo) = create_test_repo();
        let mut backend = GitNotesBackend::new(dir.path());

        let memory = create_test_memory();
        backend.store(&memory).unwrap();

        let ids = backend.list_ids().unwrap();
        assert!(!ids.is_empty());
    }

    #[test]
    fn test_parse_namespace() {
        assert_eq!(parse_namespace("decisions"), Namespace::Decisions);
        assert_eq!(parse_namespace("Patterns"), Namespace::Patterns);
        assert_eq!(parse_namespace("TECH-DEBT"), Namespace::TechDebt);
        assert_eq!(parse_namespace("techdebt"), Namespace::TechDebt);
        assert_eq!(parse_namespace("unknown"), Namespace::Decisions);
    }

    #[test]
    fn test_parse_status() {
        assert_eq!(parse_status("active"), MemoryStatus::Active);
        assert_eq!(parse_status("Archived"), MemoryStatus::Archived);
        assert_eq!(parse_status("SUPERSEDED"), MemoryStatus::Superseded);
        assert_eq!(parse_status("unknown"), MemoryStatus::Active);
    }

    #[test]
    fn test_parse_domain() {
        let global = parse_domain("global");
        assert!(global.is_global());

        let org_repo = parse_domain("zircote/subcog");
        assert_eq!(org_repo.organization, Some("zircote".to_string()));
        assert_eq!(org_repo.repository, Some("subcog".to_string()));

        let full = parse_domain("org/proj/repo");
        assert_eq!(full.organization, Some("org".to_string()));
        assert_eq!(full.project, Some("proj".to_string()));
        assert_eq!(full.repository, Some("repo".to_string()));
    }

    #[test]
    fn test_roundtrip() {
        let memory = create_test_memory();
        let serialized = GitNotesBackend::serialize_memory(&memory).unwrap();
        let deserialized = GitNotesBackend::deserialize_memory(&serialized).unwrap();

        assert_eq!(memory.id.as_str(), deserialized.id.as_str());
        assert_eq!(memory.namespace, deserialized.namespace);
        assert_eq!(memory.content, deserialized.content);
        assert_eq!(memory.status, deserialized.status);
        assert_eq!(memory.created_at, deserialized.created_at);
    }
}
