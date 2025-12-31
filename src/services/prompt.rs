//! Prompt template storage and management service.
//!
//! Provides CRUD operations for user-defined prompt templates, storing them
//! as memories in the `prompts` namespace with JSON-serialized content.
//!
//! # Domain Hierarchy
//!
//! Prompts are searched in priority order:
//! 1. **Project** - Repository-specific prompts in `.git/notes/subcog`
//! 2. **User** - User-wide prompts in `~/.subcog/`
//! 3. **Org** - Organization-wide prompts (if configured)
//!
//! # Storage Format
//!
//! Prompts are stored as memories with this structure:
//! ```yaml
//! ---
//! id: prompt-code-review-1234567890
//! namespace: prompts
//! domain: project
//! tags: [coding, review]
//! ---
//! {"name":"code-review","description":"...","content":"...","variables":[...]}
//! ```

use crate::config::Config;
use crate::git::{NotesManager, YamlFrontMatterParser};
use crate::models::{MemoryStatus, Namespace, PromptTemplate};
use crate::storage::index::DomainScope;
use crate::{Error, Result};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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
pub struct PromptService {
    /// Configuration.
    config: Config,
    /// Repository path for git notes storage.
    repo_path: Option<PathBuf>,
}

impl PromptService {
    /// Creates a new prompt service.
    #[must_use]
    pub const fn new(config: Config) -> Self {
        Self {
            config,
            repo_path: None,
        }
    }

    /// Creates a prompt service with a repository path.
    #[must_use]
    pub fn with_repo_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.repo_path = Some(path.into());
        self
    }

    /// Sets the repository path.
    pub fn set_repo_path(&mut self, path: impl Into<PathBuf>) {
        self.repo_path = Some(path.into());
    }

    /// Gets the effective repository path.
    fn get_repo_path(&self) -> Result<&PathBuf> {
        self.repo_path
            .as_ref()
            .or(self.config.repo_path.as_ref())
            .ok_or_else(|| Error::InvalidInput("Repository path not configured".to_string()))
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
    /// let service = PromptService::new(Default::default());
    /// let template = PromptTemplate::new("code-review", "Review {{code}}");
    /// let id = service.save(template, DomainScope::Project)?;
    /// # Ok::<(), subcog::Error>(())
    /// ```
    pub fn save(&self, mut template: PromptTemplate, domain: DomainScope) -> Result<String> {
        // Validate name
        validate_prompt_name(&template.name)?;

        // Set timestamps if not already set
        let now = current_timestamp();
        if template.created_at == 0 {
            template.created_at = now;
        }
        template.updated_at = now;

        // Serialize template to JSON for storage
        let json_content =
            serde_json::to_string(&template).map_err(|e| Error::OperationFailed {
                operation: "serialize_prompt".to_string(),
                cause: e.to_string(),
            })?;

        // Generate memory ID
        let memory_id = format!("prompt_{}_{}", template.name, now);

        // Store to git notes
        let repo_path = self.get_repo_path()?;
        let notes = NotesManager::new(repo_path);

        // Build metadata
        let metadata = serde_json::json!({
            "id": memory_id,
            "namespace": Namespace::Prompts.as_str(),
            "domain": domain_scope_to_string(domain),
            "status": MemoryStatus::Active.as_str(),
            "created_at": template.created_at,
            "updated_at": template.updated_at,
            "tags": template.tags,
            "prompt_name": template.name,
        });

        let note_content = YamlFrontMatterParser::serialize(&metadata, &json_content)?;

        // Check if prompt already exists - if so, delete old and add new
        // (NotesManager.add with force=true will overwrite)
        if let Some(existing_id) = self.find_prompt_note_id(&template.name, domain)? {
            // Remove old note, then add new one
            notes.remove(&existing_id)?;
        }
        notes.add_to_head(&note_content)?;

        Ok(memory_id)
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
    pub fn get(&self, name: &str, domain: Option<DomainScope>) -> Result<Option<PromptTemplate>> {
        let repo_path = self.get_repo_path()?;
        let notes = NotesManager::new(repo_path);

        // Get all notes and search for the prompt
        let all_notes = notes.list()?;

        // Search order based on domain parameter
        let scopes = match domain {
            Some(scope) => vec![scope],
            None => vec![DomainScope::Project, DomainScope::User, DomainScope::Org],
        };

        for scope in scopes {
            if let Some(template) = self.find_prompt_in_notes(&all_notes, name, scope)? {
                return Ok(Some(template));
            }
        }

        Ok(None)
    }

    /// Searches for a prompt in notes for a specific domain scope.
    fn find_prompt_in_notes(
        &self,
        notes: &[(String, String)],
        name: &str,
        scope: DomainScope,
    ) -> Result<Option<PromptTemplate>> {
        for (note_id, content) in notes {
            let result = self.parse_prompt_note(note_id, content, name, scope)?;
            if result.is_some() {
                return Ok(result);
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
    pub fn list(&self, filter: &PromptFilter) -> Result<Vec<PromptTemplate>> {
        let repo_path = self.get_repo_path()?;
        let notes = NotesManager::new(repo_path);

        let all_notes = notes.list()?;
        let mut results = self.collect_matching_prompts(&all_notes, filter)?;

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

    /// Collects prompts from notes that match the filter.
    fn collect_matching_prompts(
        &self,
        notes: &[(String, String)],
        filter: &PromptFilter,
    ) -> Result<Vec<PromptTemplate>> {
        let mut results = Vec::new();
        for (_note_id, content) in notes {
            let template = self.try_parse_prompt(content)?;
            if let Some(t) = template.filter(|t| self.matches_filter(t, filter)) {
                results.push(t);
            }
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
    pub fn delete(&self, name: &str, domain: DomainScope) -> Result<bool> {
        let repo_path = self.get_repo_path()?;
        let notes = NotesManager::new(repo_path);

        // Find the note containing this prompt
        if let Some(note_id) = self.find_prompt_note_id(name, domain)? {
            notes.remove(&note_id)?;
            return Ok(true);
        }

        Ok(false)
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
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<PromptTemplate>> {
        let repo_path = self.get_repo_path()?;
        let notes = NotesManager::new(repo_path);

        let all_notes = notes.list()?;
        let query_lower = query.to_lowercase();
        let mut results = self.score_prompts(&all_notes, &query_lower)?;

        // Sort by score descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top results
        Ok(results.into_iter().take(limit).map(|(t, _)| t).collect())
    }

    /// Scores prompts against a search query.
    fn score_prompts(
        &self,
        notes: &[(String, String)],
        query_lower: &str,
    ) -> Result<Vec<(PromptTemplate, f32)>> {
        let mut results = Vec::new();
        for (_note_id, content) in notes {
            self.add_scored_prompt(&mut results, content, query_lower)?;
        }
        Ok(results)
    }

    /// Calculates and adds a scored prompt if it matches the query.
    fn add_scored_prompt(
        &self,
        results: &mut Vec<(PromptTemplate, f32)>,
        content: &str,
        query_lower: &str,
    ) -> Result<()> {
        let Some(template) = self.try_parse_prompt(content)? else {
            return Ok(());
        };
        let score = self.calculate_relevance(&template, query_lower);
        if score > 0.0 {
            results.push((template, score));
        }
        Ok(())
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
    pub fn increment_usage(&self, name: &str, domain: DomainScope) -> Result<()> {
        // Get the existing prompt
        let mut template = self
            .get(name, Some(domain))?
            .ok_or_else(|| Error::OperationFailed {
                operation: "increment_usage".to_string(),
                cause: format!("Prompt not found: {name}"),
            })?;

        // Increment usage count
        template.usage_count = template.usage_count.saturating_add(1);

        // Save back
        self.save(template, domain)?;

        Ok(())
    }

    /// Finds the git note ID for a prompt by name and domain.
    fn find_prompt_note_id(&self, name: &str, domain: DomainScope) -> Result<Option<String>> {
        let repo_path = self.get_repo_path()?;
        let notes = NotesManager::new(repo_path);

        let all_notes = notes.list()?;

        for (note_id, content) in &all_notes {
            if self.note_matches_prompt(content, name, domain) {
                return Ok(Some(note_id.clone()));
            }
        }

        Ok(None)
    }

    /// Checks if a note matches a prompt by name and domain.
    fn note_matches_prompt(&self, content: &str, name: &str, domain: DomainScope) -> bool {
        let Ok((metadata, _)) = YamlFrontMatterParser::parse(content) else {
            return false;
        };

        // Check if this is a prompt note
        let is_prompt = metadata
            .get("namespace")
            .and_then(|v| v.as_str())
            .is_some_and(|ns| ns == Namespace::Prompts.as_str());

        if !is_prompt {
            return false;
        }

        // Check name match
        let name_matches = metadata
            .get("prompt_name")
            .and_then(|v| v.as_str())
            .is_some_and(|n| n == name);

        if !name_matches {
            return false;
        }

        // Check domain match
        metadata
            .get("domain")
            .and_then(|v| v.as_str())
            .is_some_and(|d| d == domain_scope_to_string(domain))
    }

    /// Parses a prompt note if it matches the name and domain.
    fn parse_prompt_note(
        &self,
        _note_id: &str,
        content: &str,
        name: &str,
        domain: DomainScope,
    ) -> Result<Option<PromptTemplate>> {
        let (metadata, body) = YamlFrontMatterParser::parse(content)?;

        // Check if this is a prompt note
        let is_prompt = metadata
            .get("namespace")
            .and_then(|v| v.as_str())
            .is_some_and(|ns| ns == Namespace::Prompts.as_str());

        if !is_prompt {
            return Ok(None);
        }

        // Check domain
        let domain_str = domain_scope_to_string(domain);
        let domain_matches = metadata
            .get("domain")
            .and_then(|v| v.as_str())
            .is_some_and(|d| d == domain_str);

        if !domain_matches {
            return Ok(None);
        }

        // Parse the JSON body
        let template: PromptTemplate =
            serde_json::from_str(&body).map_err(|e| Error::OperationFailed {
                operation: "parse_prompt_json".to_string(),
                cause: e.to_string(),
            })?;

        // Check name match
        if template.name != name {
            return Ok(None);
        }

        Ok(Some(template))
    }

    /// Tries to parse a note as a prompt template.
    fn try_parse_prompt(&self, content: &str) -> Result<Option<PromptTemplate>> {
        let parse_result = YamlFrontMatterParser::parse(content);
        let (metadata, body) = match parse_result {
            Ok(result) => result,
            Err(_) => return Ok(None),
        };

        // Check if this is a prompt note
        let is_prompt = metadata
            .get("namespace")
            .and_then(|v| v.as_str())
            .is_some_and(|ns| ns == Namespace::Prompts.as_str());

        if !is_prompt {
            return Ok(None);
        }

        // Parse the JSON body
        let template: PromptTemplate = match serde_json::from_str(&body) {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };

        Ok(Some(template))
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
fn validate_prompt_name(name: &str) -> Result<()> {
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

/// Converts domain scope to string for storage.
const fn domain_scope_to_string(scope: DomainScope) -> &'static str {
    match scope {
        DomainScope::Project => "project",
        DomainScope::User => "user",
        DomainScope::Org => "org",
    }
}

/// Gets current Unix timestamp.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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

    #[test]
    fn test_domain_scope_to_string() {
        assert_eq!(domain_scope_to_string(DomainScope::Project), "project");
        assert_eq!(domain_scope_to_string(DomainScope::User), "user");
        assert_eq!(domain_scope_to_string(DomainScope::Org), "org");
    }
}
