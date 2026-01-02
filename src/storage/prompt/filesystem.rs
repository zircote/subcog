//! Filesystem-based prompt storage fallback.
//!
//! Stores prompts as JSON files in a directory structure.

use super::PromptStorage;
use crate::current_timestamp;
use crate::models::PromptTemplate;
use crate::{Error, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Filesystem-based prompt storage.
///
/// Stores each prompt as a JSON file: `{base_path}/{prompt_name}.json`
pub struct FilesystemPromptStorage {
    /// Base directory for prompt files.
    base_path: PathBuf,
}

impl FilesystemPromptStorage {
    /// Creates a new filesystem prompt storage.
    ///
    /// # Arguments
    ///
    /// * `base_path` - Directory to store prompt files
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    pub fn new(base_path: impl Into<PathBuf>) -> Result<Self> {
        let path = base_path.into();

        // Ensure directory exists
        fs::create_dir_all(&path).map_err(|e| Error::OperationFailed {
            operation: "create_prompt_dir".to_string(),
            cause: e.to_string(),
        })?;

        Ok(Self { base_path: path })
    }

    /// Returns the default user-scope path.
    ///
    /// Returns `~/.config/subcog/prompts/`.
    #[must_use]
    pub fn default_user_path() -> Option<PathBuf> {
        directories::BaseDirs::new()
            .map(|d| d.home_dir().join(".config").join("subcog").join("prompts"))
    }

    /// Returns the default org-scope path.
    ///
    /// Returns `~/.config/subcog/orgs/{org}/prompts/`.
    #[must_use]
    pub fn default_org_path(org: &str) -> Option<PathBuf> {
        directories::BaseDirs::new().map(|d| {
            d.home_dir()
                .join(".config")
                .join("subcog")
                .join("orgs")
                .join(org)
                .join("prompts")
        })
    }

    /// Returns the base path.
    #[must_use]
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    /// Gets the file path for a prompt.
    fn prompt_path(&self, name: &str) -> PathBuf {
        self.base_path.join(format!("{name}.json"))
    }

    /// Reads a prompt from a file.
    fn read_prompt_file(&self, path: &Path) -> Result<PromptTemplate> {
        let content = fs::read_to_string(path).map_err(|e| Error::OperationFailed {
            operation: "read_prompt_file".to_string(),
            cause: e.to_string(),
        })?;

        serde_json::from_str(&content).map_err(|e| Error::OperationFailed {
            operation: "parse_prompt_json".to_string(),
            cause: e.to_string(),
        })
    }

    /// Writes a prompt to a file.
    fn write_prompt_file(&self, path: &Path, template: &PromptTemplate) -> Result<()> {
        let content =
            serde_json::to_string_pretty(template).map_err(|e| Error::OperationFailed {
                operation: "serialize_prompt".to_string(),
                cause: e.to_string(),
            })?;

        fs::write(path, content).map_err(|e| Error::OperationFailed {
            operation: "write_prompt_file".to_string(),
            cause: e.to_string(),
        })
    }
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

impl PromptStorage for FilesystemPromptStorage {
    fn save(&self, template: &PromptTemplate) -> Result<String> {
        let path = self.prompt_path(&template.name);

        // Create mutable copy with updated timestamp
        let mut template = template.clone();
        let now = current_timestamp();
        if template.created_at == 0 {
            template.created_at = now;
        }
        template.updated_at = now;

        self.write_prompt_file(&path, &template)?;

        Ok(format!("prompt_fs_{}", template.name))
    }

    fn get(&self, name: &str) -> Result<Option<PromptTemplate>> {
        let path = self.prompt_path(name);

        if !path.exists() {
            return Ok(None);
        }

        let template = self.read_prompt_file(&path)?;
        Ok(Some(template))
    }

    fn list(
        &self,
        tags: Option<&[String]>,
        name_pattern: Option<&str>,
    ) -> Result<Vec<PromptTemplate>> {
        let entries = fs::read_dir(&self.base_path).map_err(|e| Error::OperationFailed {
            operation: "list_prompt_dir".to_string(),
            cause: e.to_string(),
        })?;

        let mut results = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();

            // Only process .json files
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            // Try to read the prompt
            let template = match self.read_prompt_file(&path) {
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
        let path = self.prompt_path(name);

        if !path.exists() {
            return Ok(false);
        }

        fs::remove_file(&path).map_err(|e| Error::OperationFailed {
            operation: "delete_prompt_file".to_string(),
            cause: e.to_string(),
        })?;

        Ok(true)
    }

    fn increment_usage(&self, name: &str) -> Result<u64> {
        let path = self.prompt_path(name);

        if !path.exists() {
            return Err(Error::OperationFailed {
                operation: "increment_usage".to_string(),
                cause: format!("Prompt not found: {name}"),
            });
        }

        let mut template = self.read_prompt_file(&path)?;
        template.usage_count = template.usage_count.saturating_add(1);
        template.updated_at = current_timestamp();

        self.write_prompt_file(&path, &template)?;

        Ok(template.usage_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_filesystem_prompt_storage_creation() {
        let dir = TempDir::new().unwrap();
        let storage = FilesystemPromptStorage::new(dir.path()).unwrap();
        assert_eq!(storage.base_path(), dir.path());
    }

    #[test]
    fn test_save_and_get_prompt() {
        let dir = TempDir::new().unwrap();
        let storage = FilesystemPromptStorage::new(dir.path()).unwrap();

        let template =
            PromptTemplate::new("test-prompt", "Hello {{name}}!").with_description("A test prompt");

        let id = storage.save(&template).unwrap();
        assert!(id.contains("test-prompt"));

        let retrieved = storage.get("test-prompt").unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.name, "test-prompt");
        assert_eq!(retrieved.content, "Hello {{name}}!");
    }

    #[test]
    fn test_list_prompts() {
        let dir = TempDir::new().unwrap();
        let storage = FilesystemPromptStorage::new(dir.path()).unwrap();

        storage
            .save(&PromptTemplate::new("alpha", "A").with_tags(vec!["tag1".to_string()]))
            .unwrap();
        storage
            .save(
                &PromptTemplate::new("beta", "B")
                    .with_tags(vec!["tag1".to_string(), "tag2".to_string()]),
            )
            .unwrap();
        storage.save(&PromptTemplate::new("gamma", "C")).unwrap();

        // List all
        let all = storage.list(None, None).unwrap();
        assert_eq!(all.len(), 3);

        // Filter by tag
        let with_tag1 = storage.list(Some(&["tag1".to_string()]), None).unwrap();
        assert_eq!(with_tag1.len(), 2);

        // Filter by name pattern
        let alpha_pattern = storage.list(None, Some("a*")).unwrap();
        assert_eq!(alpha_pattern.len(), 1);
        assert_eq!(alpha_pattern[0].name, "alpha");
    }

    #[test]
    fn test_delete_prompt() {
        let dir = TempDir::new().unwrap();
        let storage = FilesystemPromptStorage::new(dir.path()).unwrap();

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
        let dir = TempDir::new().unwrap();
        let storage = FilesystemPromptStorage::new(dir.path()).unwrap();

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
    fn test_default_user_path() {
        let path = FilesystemPromptStorage::default_user_path();
        // Should return Some on most systems
        if let Some(p) = path {
            assert!(p.to_string_lossy().contains("subcog"));
            assert!(p.to_string_lossy().ends_with("prompts"));
        }
    }

    #[test]
    fn test_default_org_path() {
        let path = FilesystemPromptStorage::default_org_path("test-org");
        // Should return Some on most systems
        if let Some(p) = path {
            assert!(p.to_string_lossy().contains("subcog"));
            assert!(p.to_string_lossy().contains("orgs"));
            assert!(p.to_string_lossy().contains("test-org"));
            assert!(p.to_string_lossy().ends_with("prompts"));
        }
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
}
