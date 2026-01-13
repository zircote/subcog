//! Context template storage trait definition.

use crate::Result;
use crate::models::ContextTemplate;

/// Trait for context template storage backends.
///
/// Each domain scope (project, user, org) can have its own storage backend.
/// Unlike prompt storage, context templates support versioning with auto-increment.
pub trait ContextTemplateStorage: Send + Sync {
    /// Saves a context template, auto-incrementing the version.
    ///
    /// If a template with the same name exists, creates a new version.
    /// If this is a new template, creates version 1.
    ///
    /// # Arguments
    ///
    /// * `template` - The context template to save
    ///
    /// # Returns
    ///
    /// A tuple of (name, version) for the saved template.
    ///
    /// # Errors
    ///
    /// Returns an error if the template cannot be saved.
    fn save(&self, template: &ContextTemplate) -> Result<(String, u32)>;

    /// Gets a context template by name and optional version.
    ///
    /// # Arguments
    ///
    /// * `name` - The template name
    /// * `version` - Optional version number. If None, returns the latest version.
    ///
    /// # Returns
    ///
    /// The context template if found, None otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage cannot be accessed.
    fn get(&self, name: &str, version: Option<u32>) -> Result<Option<ContextTemplate>>;

    /// Lists all context templates, optionally filtered.
    ///
    /// Returns only the latest version of each template.
    ///
    /// # Arguments
    ///
    /// * `tags` - Optional tags to filter by (AND logic)
    /// * `name_pattern` - Optional glob pattern for name matching
    ///
    /// # Returns
    ///
    /// List of matching context templates (latest versions only).
    ///
    /// # Errors
    ///
    /// Returns an error if the storage cannot be accessed.
    fn list(
        &self,
        tags: Option<&[String]>,
        name_pattern: Option<&str>,
    ) -> Result<Vec<ContextTemplate>>;

    /// Deletes a context template by name and optional version.
    ///
    /// # Arguments
    ///
    /// * `name` - The template name
    /// * `version` - Optional version. If None, deletes all versions.
    ///
    /// # Returns
    ///
    /// True if any versions were deleted, false if not found.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage cannot be accessed.
    fn delete(&self, name: &str, version: Option<u32>) -> Result<bool>;

    /// Gets all available versions for a template.
    ///
    /// # Arguments
    ///
    /// * `name` - The template name
    ///
    /// # Returns
    ///
    /// List of version numbers in descending order (newest first).
    ///
    /// # Errors
    ///
    /// Returns an error if the storage cannot be accessed.
    fn get_versions(&self, name: &str) -> Result<Vec<u32>>;

    /// Gets the latest version number for a template.
    ///
    /// # Arguments
    ///
    /// * `name` - The template name
    ///
    /// # Returns
    ///
    /// The latest version number, or None if template doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage cannot be accessed.
    fn get_latest_version(&self, name: &str) -> Result<Option<u32>>;
}
