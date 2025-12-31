//! Git notes-based prompt storage for project scope.
//!
//! Stores prompts as git notes in the `refs/notes/subcog/prompts` ref.
//! Each prompt is stored as a note attached to a separate empty commit.

use super::PromptStorage;
use crate::git::{NotesManager, YamlFrontMatterParser};
use crate::models::PromptTemplate;
use crate::{Error, Result};
use git2::{Repository, Signature};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Notes ref for storing prompts.
const PROMPTS_NOTES_REF: &str = "refs/notes/subcog/prompts";

/// Git notes-based prompt storage.
pub struct GitNotesPromptStorage {
    /// Path to the git repository.
    repo_path: PathBuf,
    /// Notes manager instance.
    notes_manager: NotesManager,
    /// In-memory cache of prompt name to commit ID.
    cache: Mutex<HashMap<String, String>>,
}

impl GitNotesPromptStorage {
    /// Creates a new git notes prompt storage.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the git repository
    #[must_use]
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        let path = repo_path.into();
        let notes_manager = NotesManager::new(&path).with_notes_ref(PROMPTS_NOTES_REF);
        Self {
            repo_path: path,
            notes_manager,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Returns the repository path.
    #[must_use]
    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    /// Returns the notes ref being used.
    #[must_use]
    pub const fn notes_ref(&self) -> &str {
        PROMPTS_NOTES_REF
    }

    /// Builds the cache of prompt names to commit IDs.
    ///
    /// # Errors
    ///
    /// Returns an error if notes cannot be read.
    pub fn build_cache(&self) -> Result<()> {
        let mut cache = self.cache.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_cache".to_string(),
            cause: e.to_string(),
        })?;

        cache.clear();

        let notes = self.notes_manager.list()?;

        for (commit_id, content) in notes {
            if let Some(name) = extract_prompt_name(&content) {
                cache.insert(name, commit_id);
            }
        }

        Ok(())
    }

    /// Creates an empty commit for attaching a prompt note.
    ///
    /// This allows each prompt to have its own note without overwriting others.
    fn create_prompt_commit(&self, prompt_name: &str) -> Result<git2::Oid> {
        let repo = Repository::open(&self.repo_path).map_err(|e| Error::OperationFailed {
            operation: "open_repository".to_string(),
            cause: e.to_string(),
        })?;

        let sig = repo.signature().or_else(|_| {
            Signature::now("subcog", "subcog@local").map_err(|e| Error::OperationFailed {
                operation: "create_signature".to_string(),
                cause: e.to_string(),
            })
        })?;

        // Get HEAD commit for parent
        let head = repo.head().map_err(|e| Error::OperationFailed {
            operation: "get_head".to_string(),
            cause: e.to_string(),
        })?;

        let parent_commit = head.peel_to_commit().map_err(|e| Error::OperationFailed {
            operation: "peel_to_commit".to_string(),
            cause: e.to_string(),
        })?;

        // Get the tree from the parent (we're creating an empty commit)
        let tree = parent_commit.tree().map_err(|e| Error::OperationFailed {
            operation: "get_tree".to_string(),
            cause: e.to_string(),
        })?;

        // Create commit with message indicating it's a prompt marker
        let commit_oid = repo
            .commit(
                None, // Don't update any ref
                &sig,
                &sig,
                &format!("subcog: prompt marker for '{prompt_name}'"),
                &tree,
                &[&parent_commit],
            )
            .map_err(|e| Error::OperationFailed {
                operation: "create_commit".to_string(),
                cause: e.to_string(),
            })?;

        Ok(commit_oid)
    }

    /// Adds a note to a specific commit.
    fn add_note_to_commit(&self, commit_oid: git2::Oid, content: &str) -> Result<()> {
        let repo = Repository::open(&self.repo_path).map_err(|e| Error::OperationFailed {
            operation: "open_repository".to_string(),
            cause: e.to_string(),
        })?;

        let sig = repo.signature().or_else(|_| {
            Signature::now("subcog", "subcog@local").map_err(|e| Error::OperationFailed {
                operation: "create_signature".to_string(),
                cause: e.to_string(),
            })
        })?;

        repo.note(
            &sig,
            &sig,
            Some(PROMPTS_NOTES_REF),
            commit_oid,
            content,
            true,
        )
        .map_err(|e| Error::OperationFailed {
            operation: "add_note".to_string(),
            cause: e.to_string(),
        })?;

        Ok(())
    }

    /// Serializes a prompt to git notes format with YAML front matter.
    fn serialize_prompt(template: &PromptTemplate) -> Result<String> {
        let metadata = serde_json::json!({
            "prompt_name": template.name,
            "description": template.description,
            "tags": template.tags,
            "author": template.author,
            "usage_count": template.usage_count,
            "created_at": template.created_at,
            "updated_at": template.updated_at,
            "variables": template.variables,
        });

        YamlFrontMatterParser::serialize(&metadata, &template.content)
    }

    /// Deserializes a prompt from git notes format.
    fn deserialize_prompt(content: &str) -> Result<PromptTemplate> {
        let (metadata, body) = YamlFrontMatterParser::parse(content)?;

        let name = metadata
            .get("prompt_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing prompt name in metadata".to_string()))?
            .to_string();

        let description = metadata
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let tags: Vec<String> = metadata
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let author = metadata
            .get("author")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);

        let usage_count = metadata
            .get("usage_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        let created_at = metadata
            .get("created_at")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        let updated_at = metadata
            .get("updated_at")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(created_at);

        let variables = metadata
            .get("variables")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        Ok(PromptTemplate {
            name,
            description,
            content: body,
            variables,
            tags,
            author,
            usage_count,
            created_at,
            updated_at,
        })
    }

    /// Finds a prompt note by name.
    fn find_prompt_note(&self, name: &str) -> Result<Option<(String, String)>> {
        let notes = self.notes_manager.list()?;

        for (commit_id, content) in notes {
            let prompt_name = extract_prompt_name(&content);
            if prompt_name.as_deref() == Some(name) {
                return Ok(Some((commit_id, content)));
            }
        }

        Ok(None)
    }
}

/// Extracts the prompt name from note content.
fn extract_prompt_name(content: &str) -> Option<String> {
    let (metadata, _) = YamlFrontMatterParser::parse(content).ok()?;
    metadata
        .get("prompt_name")
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Gets current Unix timestamp.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Simple glob pattern matching.
fn matches_glob(pattern: &str, text: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == text;
    }

    let parts: Vec<&str> = pattern.split('*').collect();

    if parts.is_empty() {
        return true;
    }

    // Check prefix
    if !parts[0].is_empty() && !text.starts_with(parts[0]) {
        return false;
    }

    // Check suffix
    let last = parts.last().unwrap_or(&"");
    if !last.is_empty() && !text.ends_with(last) {
        return false;
    }

    // Check all parts exist in order
    let mut remaining = text;
    for part in &parts {
        if part.is_empty() {
            continue;
        }
        if let Some(pos) = remaining.find(part) {
            remaining = &remaining[pos + part.len()..];
        } else {
            return false;
        }
    }

    true
}

impl PromptStorage for GitNotesPromptStorage {
    fn save(&self, template: &PromptTemplate) -> Result<String> {
        // Create mutable copy with updated timestamp
        let mut template = template.clone();
        let now = current_timestamp();
        if template.created_at == 0 {
            template.created_at = now;
        }
        template.updated_at = now;

        let content = Self::serialize_prompt(&template)?;

        // Check if prompt exists - if so, update the existing note
        if let Some((commit_id, _)) = self.find_prompt_note(&template.name)? {
            // Update existing note on the same commit
            let oid = git2::Oid::from_str(&commit_id).map_err(|e| Error::OperationFailed {
                operation: "parse_commit_id".to_string(),
                cause: e.to_string(),
            })?;
            self.add_note_to_commit(oid, &content)?;

            return Ok(format!("prompt_git_{}", template.name));
        }

        // Create a new commit for this prompt and attach the note
        let commit_oid = self.create_prompt_commit(&template.name)?;
        self.add_note_to_commit(commit_oid, &content)?;

        // Update cache
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(template.name.clone(), commit_oid.to_string());
        }

        Ok(format!("prompt_git_{}", template.name))
    }

    fn get(&self, name: &str) -> Result<Option<PromptTemplate>> {
        match self.find_prompt_note(name)? {
            Some((_, content)) => {
                let template = Self::deserialize_prompt(&content)?;
                Ok(Some(template))
            },
            None => Ok(None),
        }
    }

    fn list(
        &self,
        tags: Option<&[String]>,
        name_pattern: Option<&str>,
    ) -> Result<Vec<PromptTemplate>> {
        let notes = self.notes_manager.list()?;
        let mut results = Vec::new();

        for (_, content) in notes {
            // Try to deserialize
            let template = match Self::deserialize_prompt(&content) {
                Ok(t) => t,
                Err(_) => continue,
            };

            // Check tag filter (AND logic)
            let has_all_tags = tags.is_none_or(|required_tags| {
                required_tags.iter().all(|rt| template.tags.contains(rt))
            });
            if !has_all_tags {
                continue;
            }

            // Check name pattern
            let matches_pattern =
                name_pattern.is_none_or(|pattern| matches_glob(pattern, &template.name));
            if !matches_pattern {
                continue;
            }

            results.push(template);
        }

        // Sort by usage count (descending) then name
        results.sort_by(|a, b| {
            b.usage_count
                .cmp(&a.usage_count)
                .then_with(|| a.name.cmp(&b.name))
        });

        Ok(results)
    }

    fn delete(&self, name: &str) -> Result<bool> {
        match self.find_prompt_note(name)? {
            Some((commit_id, _)) => {
                self.notes_manager.remove(&commit_id)?;

                // Update cache
                if let Ok(mut cache) = self.cache.lock() {
                    cache.remove(name);
                }

                Ok(true)
            },
            None => Ok(false),
        }
    }

    fn increment_usage(&self, name: &str) -> Result<u64> {
        // Get existing prompt
        let (_, content) = self
            .find_prompt_note(name)?
            .ok_or_else(|| Error::OperationFailed {
                operation: "increment_usage".to_string(),
                cause: format!("Prompt not found: {name}"),
            })?;

        let mut template = Self::deserialize_prompt(&content)?;
        template.usage_count = template.usage_count.saturating_add(1);

        // Save back (which updates timestamp and removes old note)
        self.save(&template)?;

        Ok(template.usage_count)
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

        // Create an initial commit
        {
            let sig = Signature::now("test", "test@test.com").unwrap();
            let tree_id = repo.index().unwrap().write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                .unwrap();
        }

        (dir, repo)
    }

    #[test]
    fn test_git_notes_prompt_storage_creation() {
        let (dir, _repo) = create_test_repo();
        let storage = GitNotesPromptStorage::new(dir.path());
        assert_eq!(storage.repo_path(), dir.path());
        assert_eq!(storage.notes_ref(), PROMPTS_NOTES_REF);
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let template = PromptTemplate::new("test-prompt", "Hello {{name}}!")
            .with_description("A test prompt")
            .with_tags(vec!["test".to_string(), "greeting".to_string()]);

        let serialized = GitNotesPromptStorage::serialize_prompt(&template).unwrap();
        let deserialized = GitNotesPromptStorage::deserialize_prompt(&serialized).unwrap();

        assert_eq!(deserialized.name, template.name);
        assert_eq!(deserialized.content, template.content);
        assert_eq!(deserialized.description, template.description);
        assert_eq!(deserialized.tags, template.tags);
    }

    #[test]
    fn test_save_and_get_prompt() {
        let (dir, _repo) = create_test_repo();
        let storage = GitNotesPromptStorage::new(dir.path());

        let template = PromptTemplate::new("my-prompt", "Content here");

        // Save
        let id = storage.save(&template).unwrap();
        assert!(id.contains("my-prompt"));

        // Get
        let retrieved = storage.get("my-prompt").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "my-prompt");
    }

    #[test]
    fn test_list_prompts() {
        let (dir, _repo) = create_test_repo();
        let storage = GitNotesPromptStorage::new(dir.path());

        // Save multiple prompts
        storage
            .save(&PromptTemplate::new("alpha", "A").with_tags(vec!["common".to_string()]))
            .unwrap();
        storage
            .save(
                &PromptTemplate::new("beta", "B")
                    .with_tags(vec!["common".to_string(), "special".to_string()]),
            )
            .unwrap();
        storage.save(&PromptTemplate::new("gamma", "C")).unwrap();

        // List all
        let all = storage.list(None, None).unwrap();
        assert_eq!(all.len(), 3);

        // Filter by tag
        let with_common = storage.list(Some(&["common".to_string()]), None).unwrap();
        assert_eq!(with_common.len(), 2);

        // Filter by name pattern
        let alpha_pattern = storage.list(None, Some("a*")).unwrap();
        assert_eq!(alpha_pattern.len(), 1);
        assert_eq!(alpha_pattern[0].name, "alpha");
    }

    #[test]
    fn test_delete_prompt() {
        let (dir, _repo) = create_test_repo();
        let storage = GitNotesPromptStorage::new(dir.path());

        storage
            .save(&PromptTemplate::new("to-delete", "Content"))
            .unwrap();

        assert!(storage.get("to-delete").unwrap().is_some());
        assert!(storage.delete("to-delete").unwrap());
        assert!(storage.get("to-delete").unwrap().is_none());
        assert!(!storage.delete("to-delete").unwrap()); // Already deleted
    }

    #[test]
    fn test_increment_usage() {
        let (dir, _repo) = create_test_repo();
        let storage = GitNotesPromptStorage::new(dir.path());

        storage
            .save(&PromptTemplate::new("used-prompt", "Content"))
            .unwrap();

        let count1 = storage.increment_usage("used-prompt").unwrap();
        assert_eq!(count1, 1);

        let count2 = storage.increment_usage("used-prompt").unwrap();
        assert_eq!(count2, 2);

        let prompt = storage.get("used-prompt").unwrap().unwrap();
        assert_eq!(prompt.usage_count, 2);
    }

    #[test]
    fn test_update_existing_prompt() {
        let (dir, _repo) = create_test_repo();
        let storage = GitNotesPromptStorage::new(dir.path());

        // Save initial version
        storage
            .save(&PromptTemplate::new("update-me", "Version 1"))
            .unwrap();

        // Increment usage
        storage.increment_usage("update-me").unwrap();

        // Update with new content
        let mut updated =
            PromptTemplate::new("update-me", "Version 2").with_description("Updated description");
        updated.usage_count = 1; // Preserve usage count

        storage.save(&updated).unwrap();

        // Verify update
        let prompt = storage.get("update-me").unwrap().unwrap();
        assert_eq!(prompt.content, "Version 2");
        assert_eq!(prompt.description, "Updated description");
    }

    #[test]
    fn test_matches_glob() {
        assert!(matches_glob("test", "test"));
        assert!(!matches_glob("test", "other"));

        assert!(matches_glob("test-*", "test-prompt"));
        assert!(!matches_glob("test-*", "other-prompt"));

        assert!(matches_glob("*-prompt", "test-prompt"));
        assert!(matches_glob("*test*", "my-test-prompt"));
    }

    #[test]
    fn test_extract_prompt_name() {
        let content = r"---
prompt_name: my-prompt
description: Test
---
Content here";

        let name = extract_prompt_name(content);
        assert_eq!(name, Some("my-prompt".to_string()));
    }
}
