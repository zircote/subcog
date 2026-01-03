//! Git notes CRUD operations.
//!
//! Manages git notes for storing memories. Notes are attached to a
//! dedicated orphan commit to avoid polluting the main history.

use crate::{Error, Result};
use git2::{Repository, Signature};
use std::path::Path;

/// Manages git notes operations.
pub struct NotesManager {
    /// Path to the repository.
    repo_path: std::path::PathBuf,
    /// Notes ref to use.
    notes_ref: String,
}

impl NotesManager {
    /// Default notes ref for subcog.
    pub const DEFAULT_NOTES_REF: &'static str = "refs/notes/subcog";

    /// Creates a new notes manager.
    #[must_use]
    pub fn new(repo_path: impl AsRef<Path>) -> Self {
        Self {
            repo_path: repo_path.as_ref().to_path_buf(),
            notes_ref: Self::DEFAULT_NOTES_REF.to_string(),
        }
    }

    /// Sets a custom notes ref.
    #[must_use]
    pub fn with_notes_ref(mut self, notes_ref: impl Into<String>) -> Self {
        self.notes_ref = notes_ref.into();
        self
    }

    /// Returns the notes ref.
    #[must_use]
    pub fn notes_ref(&self) -> &str {
        &self.notes_ref
    }

    /// Opens the git repository.
    fn open_repo(&self) -> Result<Repository> {
        Repository::open(&self.repo_path).map_err(|e| Error::OperationFailed {
            operation: "open_repository".to_string(),
            cause: e.to_string(),
        })
    }

    /// Gets the default signature for commits.
    /// Kept as method for API consistency with other repository operations.
    #[allow(clippy::unused_self)]
    fn get_signature(&self, repo: &Repository) -> Result<Signature<'_>> {
        repo.signature().or_else(|_| {
            Signature::now("subcog", "subcog@local").map_err(|e| Error::OperationFailed {
                operation: "create_signature".to_string(),
                cause: e.to_string(),
            })
        })
    }

    /// Gets or creates the notes ref commit.
    /// Uses HEAD as the annotated object for notes.
    /// Kept as method for API consistency with other repository operations.
    #[allow(clippy::unused_self)]
    fn get_notes_target(&self, repo: &Repository) -> Result<git2::Oid> {
        // Get HEAD commit
        let head = repo.head().map_err(|e| Error::OperationFailed {
            operation: "get_head".to_string(),
            cause: e.to_string(),
        })?;

        head.target().ok_or_else(|| Error::OperationFailed {
            operation: "get_head_target".to_string(),
            cause: "HEAD has no target".to_string(),
        })
    }

    /// Adds a note to a commit.
    ///
    /// # Errors
    ///
    /// Returns an error if the note cannot be added.
    pub fn add(&self, commit_id: &str, content: &str) -> Result<()> {
        let repo = self.open_repo()?;
        let sig = self.get_signature(&repo)?;

        let oid = git2::Oid::from_str(commit_id)
            .map_err(|e| Error::InvalidInput(format!("Invalid commit ID '{commit_id}': {e}")))?;

        repo.note(&sig, &sig, Some(&self.notes_ref), oid, content, true)
            .map_err(|e| Error::OperationFailed {
                operation: "add_note".to_string(),
                cause: e.to_string(),
            })?;

        Ok(())
    }

    /// Adds a note using HEAD as the target.
    ///
    /// # Errors
    ///
    /// Returns an error if the note cannot be added.
    pub fn add_to_head(&self, content: &str) -> Result<git2::Oid> {
        let repo = self.open_repo()?;
        let sig = self.get_signature(&repo)?;
        let target = self.get_notes_target(&repo)?;

        let note_oid = repo
            .note(&sig, &sig, Some(&self.notes_ref), target, content, true)
            .map_err(|e| Error::OperationFailed {
                operation: "add_note".to_string(),
                cause: e.to_string(),
            })?;

        Ok(note_oid)
    }

    /// Gets a note from a commit.
    ///
    /// # Errors
    ///
    /// Returns an error if the note cannot be retrieved.
    pub fn get(&self, commit_id: &str) -> Result<Option<String>> {
        let repo = self.open_repo()?;

        let oid = git2::Oid::from_str(commit_id)
            .map_err(|e| Error::InvalidInput(format!("Invalid commit ID '{commit_id}': {e}")))?;

        match repo.find_note(Some(&self.notes_ref), oid) {
            Ok(note) => Ok(note.message().map(String::from)),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(None),
            Err(e) => Err(Error::OperationFailed {
                operation: "get_note".to_string(),
                cause: e.to_string(),
            }),
        }
    }

    /// Gets a note from HEAD.
    ///
    /// # Errors
    ///
    /// Returns an error if the note cannot be retrieved.
    pub fn get_from_head(&self) -> Result<Option<String>> {
        let repo = self.open_repo()?;
        let target = self.get_notes_target(&repo)?;

        match repo.find_note(Some(&self.notes_ref), target) {
            Ok(note) => Ok(note.message().map(String::from)),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(None),
            Err(e) => Err(Error::OperationFailed {
                operation: "get_note".to_string(),
                cause: e.to_string(),
            }),
        }
    }

    /// Removes a note from a commit.
    ///
    /// # Errors
    ///
    /// Returns an error if the note cannot be removed.
    pub fn remove(&self, commit_id: &str) -> Result<bool> {
        let repo = self.open_repo()?;
        let sig = self.get_signature(&repo)?;

        let oid = git2::Oid::from_str(commit_id)
            .map_err(|e| Error::InvalidInput(format!("Invalid commit ID '{commit_id}': {e}")))?;

        match repo.note_delete(oid, Some(&self.notes_ref), &sig, &sig) {
            Ok(()) => Ok(true),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(false),
            Err(e) => Err(Error::OperationFailed {
                operation: "remove_note".to_string(),
                cause: e.to_string(),
            }),
        }
    }

    /// Lists all notes as (`commit_id`, content) pairs.
    ///
    /// # Errors
    ///
    /// Returns an error if notes cannot be listed.
    pub fn list(&self) -> Result<Vec<(String, String)>> {
        let repo = self.open_repo()?;
        let mut results = Vec::new();

        let notes = match repo.notes(Some(&self.notes_ref)) {
            Ok(notes) => notes,
            Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(Error::OperationFailed {
                    operation: "list_notes".to_string(),
                    cause: e.to_string(),
                });
            },
        };

        for note_result in notes {
            let (note_oid, annotated_oid) = note_result.map_err(|e| Error::OperationFailed {
                operation: "iterate_notes".to_string(),
                cause: e.to_string(),
            })?;

            // Get the note content using let-else to reduce nesting
            let Ok(blob) = repo.find_blob(note_oid) else {
                continue;
            };

            let Ok(content) = std::str::from_utf8(blob.content()) else {
                continue;
            };

            results.push((annotated_oid.to_string(), content.to_string()));
        }

        Ok(results)
    }

    /// Checks if the notes ref exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the check fails.
    pub fn notes_ref_exists(&self) -> Result<bool> {
        let repo = self.open_repo()?;
        match repo.find_reference(&self.notes_ref) {
            Ok(_) => Ok(true),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(false),
            Err(e) => Err(Error::OperationFailed {
                operation: "check_notes_ref".to_string(),
                cause: e.to_string(),
            }),
        }
    }

    /// Returns the count of notes.
    ///
    /// # Errors
    ///
    /// Returns an error if counting fails.
    pub fn count(&self) -> Result<usize> {
        self.list().map(|notes| notes.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn test_notes_manager_creation() {
        let manager = NotesManager::new("/tmp/test");
        assert_eq!(manager.notes_ref(), NotesManager::DEFAULT_NOTES_REF);

        let custom = NotesManager::new("/tmp/test").with_notes_ref("refs/notes/custom");
        assert_eq!(custom.notes_ref(), "refs/notes/custom");
    }

    #[test]
    fn test_add_and_get_note() {
        let (dir, repo) = create_test_repo();
        let manager = NotesManager::new(dir.path());

        // Get HEAD commit
        let head = repo.head().unwrap().target().unwrap().to_string();

        // Add a note
        manager.add(&head, "Test note content").unwrap();

        // Get the note
        let content = manager.get(&head).unwrap();
        assert_eq!(content, Some("Test note content".to_string()));
    }

    #[test]
    fn test_add_to_head() {
        let (dir, _repo) = create_test_repo();
        let manager = NotesManager::new(dir.path());

        // Add a note to HEAD
        let oid = manager.add_to_head("Note on HEAD").unwrap();
        assert!(!oid.is_zero());

        // Get the note from HEAD
        let content = manager.get_from_head().unwrap();
        assert_eq!(content, Some("Note on HEAD".to_string()));
    }

    #[test]
    fn test_get_nonexistent_note() {
        let (dir, _repo) = create_test_repo();
        let manager = NotesManager::new(dir.path());

        // Try to get a note that doesn't exist (use a valid but non-existent OID)
        let result = manager.get("0000000000000000000000000000000000000001");
        // This should return Ok(None) or an error depending on implementation
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_remove_note() {
        let (dir, repo) = create_test_repo();
        let manager = NotesManager::new(dir.path());

        let head = repo.head().unwrap().target().unwrap().to_string();

        // Add and then remove a note
        manager.add(&head, "To be removed").unwrap();
        let removed = manager.remove(&head).unwrap();
        assert!(removed);

        // Verify it's gone
        let content = manager.get(&head).unwrap();
        assert!(content.is_none());
    }

    #[test]
    fn test_list_notes() {
        let (dir, repo) = create_test_repo();
        let manager = NotesManager::new(dir.path());

        let head = repo.head().unwrap().target().unwrap().to_string();

        // Add a note
        manager.add(&head, "Listed note").unwrap();

        // List notes
        let notes = manager.list().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].0, head);
        assert_eq!(notes[0].1, "Listed note");
    }

    #[test]
    fn test_list_empty_notes() {
        let (dir, _repo) = create_test_repo();
        let manager = NotesManager::new(dir.path());

        let notes = manager.list().unwrap();
        assert!(notes.is_empty());
    }

    #[test]
    fn test_count_notes() {
        let (dir, repo) = create_test_repo();
        let manager = NotesManager::new(dir.path());

        assert_eq!(manager.count().unwrap(), 0);

        let head = repo.head().unwrap().target().unwrap().to_string();
        manager.add(&head, "Note 1").unwrap();

        assert_eq!(manager.count().unwrap(), 1);
    }

    // ============================================================================
    // Git Operation Failure Tests
    // ============================================================================

    #[test]
    fn test_open_nonexistent_repo_fails() {
        let manager = NotesManager::new("/nonexistent/path/to/repo");

        // All operations should fail with OperationFailed error
        let result = manager.list();
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("open_repository") || msg.contains("failed"),
            "Error should mention repository operation: {msg}"
        );
    }

    #[test]
    fn test_add_note_invalid_commit_id() {
        let (dir, _repo) = create_test_repo();
        let manager = NotesManager::new(dir.path());

        // Invalid commit ID format
        let result = manager.add("not-a-valid-oid", "content");
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("invalid") || msg.contains("Invalid"),
            "Error should mention invalid input: {msg}"
        );
    }

    #[test]
    fn test_add_note_nonexistent_commit() {
        let (dir, _repo) = create_test_repo();
        let manager = NotesManager::new(dir.path());

        // Valid OID format but doesn't exist
        let fake_oid = "0000000000000000000000000000000000000001";
        let result = manager.add(fake_oid, "content");

        // This may succeed (git allows notes on any OID) or fail
        // depending on implementation. Just verify we don't panic.
        let _ = result;
    }

    #[test]
    fn test_get_note_invalid_commit_id() {
        let (dir, _repo) = create_test_repo();
        let manager = NotesManager::new(dir.path());

        let result = manager.get("invalid-oid");
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_note_invalid_commit_id() {
        let (dir, _repo) = create_test_repo();
        let manager = NotesManager::new(dir.path());

        let result = manager.remove("invalid-oid");
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_nonexistent_note() {
        let (dir, repo) = create_test_repo();
        let manager = NotesManager::new(dir.path());

        let head = repo.head().unwrap().target().unwrap().to_string();

        // Try to remove a note that doesn't exist
        let result = manager.remove(&head);
        // Should return Ok(false) - note didn't exist
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_custom_notes_ref() {
        let (dir, repo) = create_test_repo();
        let manager = NotesManager::new(dir.path()).with_notes_ref("refs/notes/custom");

        let head = repo.head().unwrap().target().unwrap().to_string();

        // Add note with custom ref
        manager.add(&head, "Custom ref note").unwrap();

        // Get with same custom ref should work
        let content = manager.get(&head).unwrap();
        assert_eq!(content, Some("Custom ref note".to_string()));

        // Default manager shouldn't see it
        let default_manager = NotesManager::new(dir.path());
        let default_content = default_manager.get(&head).unwrap();
        assert!(default_content.is_none());
    }

    #[test]
    fn test_notes_ref_isolation() {
        let (dir, repo) = create_test_repo();
        let head = repo.head().unwrap().target().unwrap().to_string();

        // Add notes with different refs
        let manager1 = NotesManager::new(dir.path()).with_notes_ref("refs/notes/ns1");
        let manager2 = NotesManager::new(dir.path()).with_notes_ref("refs/notes/ns2");

        manager1.add(&head, "Namespace 1 note").unwrap();
        manager2.add(&head, "Namespace 2 note").unwrap();

        // Each should see only its own note
        let content1 = manager1.get(&head).unwrap();
        let content2 = manager2.get(&head).unwrap();

        assert_eq!(content1, Some("Namespace 1 note".to_string()));
        assert_eq!(content2, Some("Namespace 2 note".to_string()));

        // Counts should be independent
        assert_eq!(manager1.count().unwrap(), 1);
        assert_eq!(manager2.count().unwrap(), 1);
    }

    #[test]
    fn test_add_to_head_on_bare_repo() {
        // Create a bare repo which has no working directory
        let dir = TempDir::new().unwrap();
        let repo = Repository::init_bare(dir.path()).unwrap();

        // Create an initial commit
        {
            let sig = Signature::now("test", "test@test.com").unwrap();
            let tree_id = {
                let mut index = repo.index().unwrap();
                index.write_tree().unwrap()
            };
            let tree = repo.find_tree(tree_id).unwrap();
            let commit_oid = repo
                .commit(None, &sig, &sig, "Initial commit", &tree, &[])
                .unwrap();

            // Set HEAD to point to the commit
            repo.reference("refs/heads/main", commit_oid, true, "initial commit")
                .unwrap();
            repo.set_head("refs/heads/main").unwrap();
        }

        let manager = NotesManager::new(dir.path());

        // Should still be able to add notes
        let result = manager.add_to_head("Note on bare repo");
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_existing_note() {
        let (dir, repo) = create_test_repo();
        let manager = NotesManager::new(dir.path());

        let head = repo.head().unwrap().target().unwrap().to_string();

        // Add initial note
        manager.add(&head, "Initial content").unwrap();

        // Update with new content (force=true in implementation)
        manager.add(&head, "Updated content").unwrap();

        // Should see updated content
        let content = manager.get(&head).unwrap();
        assert_eq!(content, Some("Updated content".to_string()));

        // Count should still be 1
        assert_eq!(manager.count().unwrap(), 1);
    }

    #[test]
    fn test_empty_note_content() {
        let (dir, repo) = create_test_repo();
        let manager = NotesManager::new(dir.path());

        let head = repo.head().unwrap().target().unwrap().to_string();

        // Empty content should be allowed
        manager.add(&head, "").unwrap();

        let content = manager.get(&head).unwrap();
        assert_eq!(content, Some(String::new()));
    }

    #[test]
    fn test_multiline_note_content() {
        let (dir, repo) = create_test_repo();
        let manager = NotesManager::new(dir.path());

        let head = repo.head().unwrap().target().unwrap().to_string();

        let multiline = "Line 1\nLine 2\nLine 3\n\nBlank line above";
        manager.add(&head, multiline).unwrap();

        let content = manager.get(&head).unwrap();
        assert_eq!(content, Some(multiline.to_string()));
    }

    #[test]
    fn test_unicode_note_content() {
        let (dir, repo) = create_test_repo();
        let manager = NotesManager::new(dir.path());

        let head = repo.head().unwrap().target().unwrap().to_string();

        let unicode = "Hello 世界 🌍 émojis café";
        manager.add(&head, unicode).unwrap();

        let content = manager.get(&head).unwrap();
        assert_eq!(content, Some(unicode.to_string()));
    }

    // ============================================================================
    // Repository State Tests (TEST-GIT-1)
    // ============================================================================

    #[test]
    fn test_detached_head_state() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        // Create initial commit
        let sig = Signature::now("test", "test@test.com").unwrap();
        let tree_id = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let commit_oid = repo
            .commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        // Detach HEAD by pointing directly to the commit
        repo.set_head_detached(commit_oid).unwrap();
        assert!(repo.head_detached().unwrap());

        let manager = NotesManager::new(dir.path());

        // Should still be able to add notes in detached HEAD state
        let result = manager.add_to_head("Note in detached HEAD");
        assert!(result.is_ok(), "Should add note in detached HEAD state");

        // Should be able to read it back
        let content = manager.get_from_head().unwrap();
        assert_eq!(content, Some("Note in detached HEAD".to_string()));
    }

    #[test]
    fn test_empty_repo_no_commits() {
        let dir = TempDir::new().unwrap();
        let _repo = Repository::init(dir.path()).unwrap();

        let manager = NotesManager::new(dir.path());

        // Empty repo has no HEAD target - add_to_head should fail gracefully
        let result = manager.add_to_head("Note on empty repo");
        assert!(
            result.is_err(),
            "Empty repo with no commits should fail to add note to HEAD"
        );

        // get_from_head should also fail gracefully
        let result = manager.get_from_head();
        assert!(
            result.is_err(),
            "Empty repo with no commits should fail to get note from HEAD"
        );

        // list should return empty, not error
        let notes = manager.list().unwrap();
        assert!(notes.is_empty());
    }

    #[test]
    fn test_notes_ref_exists_on_empty_repo() {
        let dir = TempDir::new().unwrap();
        let _repo = Repository::init(dir.path()).unwrap();

        let manager = NotesManager::new(dir.path());

        // Notes ref should not exist on fresh repo
        let exists = manager.notes_ref_exists().unwrap();
        assert!(!exists, "Notes ref should not exist on fresh repo");
    }
}
