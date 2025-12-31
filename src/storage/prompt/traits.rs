//! Prompt storage trait definition.

use crate::Result;
use crate::models::PromptTemplate;

/// Trait for prompt storage backends.
///
/// Each domain scope (project, user, org) can have its own storage backend.
pub trait PromptStorage: Send + Sync {
    /// Saves a prompt template.
    ///
    /// # Arguments
    ///
    /// * `template` - The prompt template to save
    ///
    /// # Returns
    ///
    /// The unique ID of the saved prompt.
    ///
    /// # Errors
    ///
    /// Returns an error if the prompt cannot be saved.
    fn save(&self, template: &PromptTemplate) -> Result<String>;

    /// Gets a prompt by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The prompt name
    ///
    /// # Returns
    ///
    /// The prompt template if found, None otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage cannot be accessed.
    fn get(&self, name: &str) -> Result<Option<PromptTemplate>>;

    /// Lists all prompts, optionally filtered.
    ///
    /// # Arguments
    ///
    /// * `tags` - Optional tags to filter by (AND logic)
    /// * `name_pattern` - Optional glob pattern for name matching
    ///
    /// # Returns
    ///
    /// List of matching prompt templates.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage cannot be accessed.
    fn list(
        &self,
        tags: Option<&[String]>,
        name_pattern: Option<&str>,
    ) -> Result<Vec<PromptTemplate>>;

    /// Deletes a prompt by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The prompt name
    ///
    /// # Returns
    ///
    /// True if deleted, false if not found.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage cannot be accessed.
    fn delete(&self, name: &str) -> Result<bool>;

    /// Updates a prompt's usage count.
    ///
    /// # Arguments
    ///
    /// * `name` - The prompt name
    ///
    /// # Returns
    ///
    /// The new usage count.
    ///
    /// # Errors
    ///
    /// Returns an error if the prompt is not found or storage cannot be accessed.
    fn increment_usage(&self, name: &str) -> Result<u64>;
}
