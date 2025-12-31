//! Prompt template storage and management service.
//!
//! Provides CRUD operations for user-defined prompt templates using
//! domain-scoped storage backends via [`PromptStorageFactory`].
//!
//! # Domain Hierarchy
//!
//! Prompts are searched in priority order:
//! 1. **Project** - Repository-specific prompts (git notes at `refs/notes/_prompts`)
//! 2. **User** - User-wide prompts (`~/.config/subcog/_prompts/`)
//! 3. **Org** - Organization-wide prompts (deferred)
//!
//! # Storage Backends
//!
//! | Domain | Backend | Location |
//! |--------|---------|----------|
//! | Project | Git Notes | `.git/refs/notes/_prompts` |
//! | User | SQLite | `~/.config/subcog/_prompts/prompts.db` |
//! | User | Filesystem | `~/.config/subcog/_prompts/` (fallback) |
//! | Org | Deferred | Not yet implemented |

use crate::config::Config;
use crate::models::PromptTemplate;
use crate::storage::index::DomainScope;
use crate::storage::prompt::{PromptStorage, PromptStorageFactory};
use crate::{Error, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Filter for listing prompts.
#[derive(Debug, Clone, Default)]
pub struct PromptFilter {
    /// Domain scope to filter by.
    pub domain: Option<DomainScope>,
    /// Tags to filter by (AND logic - must have all).
    pub tags: Vec<String>,
    /// Name pattern (glob-style).
    pub name_pattern: Option<String>,
    /// Maximum number of results.
    pub limit: Option<usize>,
}

impl PromptFilter {
    /// Creates a new empty filter.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            domain: None,
            tags: Vec::new(),
            name_pattern: None,
            limit: None,
        }
    }

    /// Filters by domain scope.
    #[must_use]
    pub const fn with_domain(mut self, domain: DomainScope) -> Self {
        self.domain = Some(domain);
        self
    }

    /// Filters by tags (AND logic).
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Filters by name pattern.
    #[must_use]
    pub fn with_name_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.name_pattern = Some(pattern.into());
        self
    }

    /// Limits results.
    #[must_use]
    pub const fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Service for prompt template CRUD operations.
///
/// Uses [`PromptStorageFactory`] to get domain-scoped storage backends.
pub struct PromptService {
    /// Configuration.
    config: Config,
    /// Cached storage backends per domain.
    storage_cache: HashMap<DomainScope, Arc<dyn PromptStorage>>,
}

impl PromptService {
    /// Creates a new prompt service.
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            storage_cache: HashMap::new(),
        }
    }

    /// Creates a prompt service with a repository path.
    #[must_use]
    pub fn with_repo_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.repo_path = Some(path.into());
        self
    }

    /// Sets the repository path.
    pub fn set_repo_path(&mut self, path: impl Into<PathBuf>) {
        self.config.repo_path = Some(path.into());
        // Clear cache since repo path changed
        self.storage_cache.clear();
    }

    /// Gets the storage backend for a domain scope.
    fn get_storage(&mut self, domain: DomainScope) -> Result<Arc<dyn PromptStorage>> {
        // Check cache first
        if let Some(storage) = self.storage_cache.get(&domain) {
            return Ok(Arc::clone(storage));
        }

        // Create new storage via factory
        let storage = PromptStorageFactory::create_for_scope(domain, &self.config)?;

        // Cache it
        self.storage_cache.insert(domain, Arc::clone(&storage));

        Ok(storage)
    }

    /// Saves or updates a prompt template.
    ///
    /// # Arguments
    ///
    /// * `template` - The prompt template to save
    /// * `domain` - The domain scope to save in (defaults to Project)
    ///
    /// # Returns
    ///
    /// The unique ID of the saved prompt.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The template name is empty or invalid
    /// - Storage fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use subcog::services::PromptService;
    /// use subcog::models::PromptTemplate;
    /// use subcog::storage::index::DomainScope;
    ///
    /// let mut service = PromptService::new(Default::default());
    /// let template = PromptTemplate::new("code-review", "Review {{code}}");
    /// let id = service.save(&template, DomainScope::Project)?;
    /// # Ok::<(), subcog::Error>(())
    /// ```
    pub fn save(&mut self, template: &PromptTemplate, domain: DomainScope) -> Result<String> {
        // Validate name
        validate_prompt_name(&template.name)?;

        // Get storage for domain
        let storage = self.get_storage(domain)?;

        // Delegate to storage backend
        storage.save(template)
    }

    /// Gets a prompt by name, searching domain hierarchy.
    ///
    /// Searches in priority order: Project → User → Org
    ///
    /// # Arguments
    ///
    /// * `name` - The prompt name to search for
    /// * `domain` - Optional domain to search (if None, searches all)
    ///
    /// # Returns
    ///
    /// The prompt template if found.
    ///
    /// # Errors
    ///
    /// Returns an error if storage operations fail.
    pub fn get(
        &mut self,
        name: &str,
        domain: Option<DomainScope>,
    ) -> Result<Option<PromptTemplate>> {
        // Search order based on domain parameter
        let scopes = match domain {
            Some(scope) => vec![scope],
            None => vec![DomainScope::Project, DomainScope::User],
        };

        for scope in scopes {
            // Get storage, skipping unimplemented domains (e.g., Org)
            let storage = match self.get_storage(scope) {
                Ok(s) => s,
                Err(Error::NotImplemented(_)) => continue,
                Err(e) => return Err(e),
            };
            if let Some(template) = storage.get(name)? {
                return Ok(Some(template));
            }
        }

        Ok(None)
    }

    /// Lists prompts matching the filter.
    ///
    /// # Arguments
    ///
    /// * `filter` - Filter criteria
    ///
    /// # Returns
    ///
    /// List of matching prompt templates.
    ///
    /// # Errors
    ///
    /// Returns an error if storage operations fail.
    pub fn list(&mut self, filter: &PromptFilter) -> Result<Vec<PromptTemplate>> {
        let mut results = Vec::new();

        // Determine which domains to search
        let scopes = match filter.domain {
            Some(scope) => vec![scope],
            None => vec![DomainScope::Project, DomainScope::User],
        };

        // Collect from all relevant domains
        for scope in scopes {
            // Get storage, skipping unimplemented domains
            let storage = match self.get_storage(scope) {
                Ok(s) => s,
                Err(Error::NotImplemented(_)) => continue,
                Err(e) => return Err(e),
            };

            let tags = (!filter.tags.is_empty()).then_some(filter.tags.as_slice());
            let prompts = storage.list(tags, filter.name_pattern.as_deref())?;

            // Filter and collect matching templates
            results.extend(
                prompts
                    .into_iter()
                    .filter(|t| self.matches_filter(t, filter)),
            );
        }

        // Sort by usage count (descending) then name
        results.sort_by(|a, b| {
            b.usage_count
                .cmp(&a.usage_count)
                .then_with(|| a.name.cmp(&b.name))
        });

        // Apply limit
        if let Some(limit) = filter.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    /// Deletes a prompt by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The prompt name to delete
    /// * `domain` - The domain scope to delete from
    ///
    /// # Returns
    ///
    /// `true` if the prompt was found and deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if storage operations fail.
    pub fn delete(&mut self, name: &str, domain: DomainScope) -> Result<bool> {
        let storage = self.get_storage(domain)?;
        storage.delete(name)
    }

    /// Searches prompts semantically by query.
    ///
    /// # Arguments
    ///
    /// * `query` - The search query
    /// * `limit` - Maximum results
    ///
    /// # Returns
    ///
    /// List of matching prompt templates, ordered by relevance.
    ///
    /// # Errors
    ///
    /// Returns an error if storage operations fail.
    pub fn search(&mut self, query: &str, limit: usize) -> Result<Vec<PromptTemplate>> {
        // Get all prompts from all domains
        let all_prompts = self.list(&PromptFilter::new())?;

        let query_lower = query.to_lowercase();
        let mut scored: Vec<(PromptTemplate, f32)> = all_prompts
            .into_iter()
            .map(|t| {
                let score = self.calculate_relevance(&t, &query_lower);
                (t, score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top results
        Ok(scored.into_iter().take(limit).map(|(t, _)| t).collect())
    }

    /// Increments the usage count for a prompt.
    ///
    /// # Arguments
    ///
    /// * `name` - The prompt name
    /// * `domain` - The domain scope
    ///
    /// # Errors
    ///
    /// Returns an error if the prompt is not found or storage fails.
    pub fn increment_usage(&mut self, name: &str, domain: DomainScope) -> Result<()> {
        let storage = self.get_storage(domain)?;
        storage.increment_usage(name)?;
        Ok(())
    }

    /// Checks if a template matches the filter.
    fn matches_filter(&self, template: &PromptTemplate, filter: &PromptFilter) -> bool {
        // Check tags (AND logic)
        for tag in &filter.tags {
            if !template.tags.iter().any(|t| t == tag) {
                return false;
            }
        }

        // Check name pattern (simple glob with * wildcard)
        if let Some(ref pattern) = filter.name_pattern {
            if !matches_glob(pattern, &template.name) {
                return false;
            }
        }

        true
    }

    /// Calculates relevance score for search.
    fn calculate_relevance(&self, template: &PromptTemplate, query: &str) -> f32 {
        let mut score = 0.0f32;

        // Exact name match
        if template.name.to_lowercase() == query {
            score += 10.0;
        } else if template.name.to_lowercase().contains(query) {
            score += 5.0;
        }

        // Description match
        if template.description.to_lowercase().contains(query) {
            score += 3.0;
        }

        // Content match
        if template.content.to_lowercase().contains(query) {
            score += 1.0;
        }

        // Tag match
        for tag in &template.tags {
            if tag.to_lowercase().contains(query) {
                score += 2.0;
            }
        }

        // Boost by usage
        score *= 1.0 + (template.usage_count as f32 / 100.0).min(0.5);

        score
    }
}

impl Default for PromptService {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

/// Validates a prompt name.
///
/// Valid names must be kebab-case: lowercase letters, numbers, and hyphens only.
/// Must start with a letter, cannot end with a hyphen, and cannot have consecutive hyphens.
///
/// Examples of valid names: `code-review`, `api-design-v2`, `weekly-report`
pub fn validate_prompt_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::InvalidInput(
            "Prompt name cannot be empty. Use a kebab-case name like 'code-review' or 'api-design'."
                .to_string(),
        ));
    }

    // Check for valid kebab-case: lowercase letters, numbers, hyphens
    // Must start with a letter
    let first_char = name.chars().next().unwrap_or('_');
    if !first_char.is_ascii_lowercase() {
        return Err(Error::InvalidInput(format!(
            "Prompt name must start with a lowercase letter, got '{name}'. \
             Example: 'code-review' instead of 'Code-Review' or '1-review'."
        )));
    }

    for ch in name.chars() {
        if !ch.is_ascii_lowercase() && !ch.is_ascii_digit() && ch != '-' {
            return Err(Error::InvalidInput(format!(
                "Invalid character '{ch}' in prompt name '{name}'. \
                 Use kebab-case: lowercase letters, numbers, and hyphens only. \
                 Example: 'my-prompt-v2' instead of 'My_Prompt v2'."
            )));
        }
    }

    // Cannot end with hyphen
    if name.ends_with('-') {
        return Err(Error::InvalidInput(format!(
            "Prompt name cannot end with a hyphen: '{name}'. \
             Remove the trailing hyphen or add a suffix like '{}-final'.",
            name.trim_end_matches('-')
        )));
    }

    // Cannot have consecutive hyphens
    if name.contains("--") {
        return Err(Error::InvalidInput(format!(
            "Prompt name cannot have consecutive hyphens: '{name}'. \
             Use single hyphens between words, e.g., 'my-prompt' instead of 'my--prompt'."
        )));
    }

    Ok(())
}

/// Simple glob pattern matching (* only).
fn matches_glob(pattern: &str, text: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == text;
    }

    let parts: Vec<&str> = pattern.split('*').collect();

    // Handle empty pattern
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_prompt_name_valid() {
        assert!(validate_prompt_name("code-review").is_ok());
        assert!(validate_prompt_name("my-prompt-v2").is_ok());
        assert!(validate_prompt_name("simple").is_ok());
        assert!(validate_prompt_name("a1b2c3").is_ok());
    }

    #[test]
    fn test_validate_prompt_name_invalid() {
        // Empty
        assert!(validate_prompt_name("").is_err());

        // Starts with number
        assert!(validate_prompt_name("1invalid").is_err());

        // Starts with hyphen
        assert!(validate_prompt_name("-invalid").is_err());

        // Contains uppercase
        assert!(validate_prompt_name("Invalid").is_err());

        // Contains underscore
        assert!(validate_prompt_name("invalid_name").is_err());

        // Ends with hyphen
        assert!(validate_prompt_name("invalid-").is_err());

        // Consecutive hyphens
        assert!(validate_prompt_name("invalid--name").is_err());

        // Contains spaces
        assert!(validate_prompt_name("invalid name").is_err());
    }

    #[test]
    fn test_matches_glob() {
        // Exact match
        assert!(matches_glob("code-review", "code-review"));
        assert!(!matches_glob("code-review", "other"));

        // Prefix match
        assert!(matches_glob("code-*", "code-review"));
        assert!(matches_glob("code-*", "code-fix"));
        assert!(!matches_glob("code-*", "other-review"));

        // Suffix match
        assert!(matches_glob("*-review", "code-review"));
        assert!(matches_glob("*-review", "quick-review"));
        assert!(!matches_glob("*-review", "code-fix"));

        // Contains match
        assert!(matches_glob("*code*", "my-code-review"));
        assert!(!matches_glob("*code*", "my-review"));

        // Multiple wildcards
        assert!(matches_glob("*code*review*", "my-code-review-v2"));
    }

    #[test]
    fn test_prompt_filter_builder() {
        let filter = PromptFilter::new()
            .with_domain(DomainScope::Project)
            .with_tags(vec!["coding".to_string()])
            .with_name_pattern("code-*")
            .with_limit(10);

        assert_eq!(filter.domain, Some(DomainScope::Project));
        assert_eq!(filter.tags, vec!["coding"]);
        assert_eq!(filter.name_pattern, Some("code-*".to_string()));
        assert_eq!(filter.limit, Some(10));
    }

    #[test]
    fn test_matches_filter_tags() {
        let service = PromptService::default();

        let template = PromptTemplate::new("test", "content")
            .with_tags(vec!["coding".to_string(), "rust".to_string()]);

        // Matches all tags
        let filter = PromptFilter::new().with_tags(vec!["coding".to_string()]);
        assert!(service.matches_filter(&template, &filter));

        // Matches multiple tags
        let filter = PromptFilter::new().with_tags(vec!["coding".to_string(), "rust".to_string()]);
        assert!(service.matches_filter(&template, &filter));

        // Doesn't match missing tag
        let filter = PromptFilter::new().with_tags(vec!["python".to_string()]);
        assert!(!service.matches_filter(&template, &filter));
    }

    #[test]
    fn test_matches_filter_name_pattern() {
        let service = PromptService::default();

        let template = PromptTemplate::new("code-review", "content");

        let filter = PromptFilter::new().with_name_pattern("code-*");
        assert!(service.matches_filter(&template, &filter));

        let filter = PromptFilter::new().with_name_pattern("*-review");
        assert!(service.matches_filter(&template, &filter));

        let filter = PromptFilter::new().with_name_pattern("other-*");
        assert!(!service.matches_filter(&template, &filter));
    }

    #[test]
    fn test_calculate_relevance() {
        let service = PromptService::default();

        let template = PromptTemplate::new("code-review", "Review code for issues")
            .with_description("A helpful code review prompt")
            .with_tags(vec!["coding".to_string(), "review".to_string()]);

        // Exact name match should score highest
        let exact_score = service.calculate_relevance(&template, "code-review");
        let partial_score = service.calculate_relevance(&template, "code");
        let desc_score = service.calculate_relevance(&template, "helpful");
        let no_match_score = service.calculate_relevance(&template, "xyz123");

        assert!(exact_score > partial_score);
        assert!(partial_score > desc_score);
        assert!(no_match_score == 0.0);
    }
}
