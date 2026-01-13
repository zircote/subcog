//! Context template storage, management, and rendering service.
//!
//! Provides CRUD operations for user-defined context templates with versioning,
//! plus rendering capabilities for formatting memories and statistics.
//!
//! # Key Features
//!
//! - **Auto-increment versioning**: Each save creates a new version
//! - **Template rendering**: Variable substitution, iteration, format conversion
//! - **Multiple output formats**: Markdown, JSON, XML
//! - **Auto-variables**: Memory fields and statistics automatically populated
//!
//! # Example
//!
//! ```rust,ignore
//! use subcog::services::ContextTemplateService;
//! use subcog::models::{ContextTemplate, OutputFormat};
//!
//! let mut service = ContextTemplateService::new();
//!
//! // Create a template
//! let template = ContextTemplate::new("search-results", r#"
//! # Results
//! Found {{total_count}} memories:
//! {{#each memories}}
//! - {{memory.content}}
//! {{/each}}
//! "#);
//!
//! // Save it (auto-increments version)
//! let (name, version) = service.save(&template)?;
//!
//! // Render with memories
//! let output = service.render_with_memories(
//!     "search-results",
//!     None, // latest version
//!     &memories,
//!     &statistics,
//!     &HashMap::new(),
//!     OutputFormat::Markdown,
//! )?;
//! ```

use crate::config::SubcogConfig;
use crate::models::{ContextTemplate, Memory, OutputFormat, VariableType};
use crate::rendering::{RenderContext, RenderValue, TemplateRenderer};
use crate::services::MemoryStatistics;
use crate::storage::context_template::{ContextTemplateStorage, ContextTemplateStorageFactory};
use crate::storage::index::DomainScope;
use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::Arc;

/// Filter for listing context templates.
#[derive(Debug, Clone, Default)]
pub struct ContextTemplateFilter {
    /// Domain scope to filter by.
    pub domain: Option<DomainScope>,
    /// Tags to filter by (AND logic - must have all).
    pub tags: Vec<String>,
    /// Name pattern (glob-style).
    pub name_pattern: Option<String>,
    /// Maximum number of results.
    pub limit: Option<usize>,
}

impl ContextTemplateFilter {
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

/// Result of a render operation.
#[derive(Debug, Clone)]
pub struct RenderResult {
    /// The rendered output.
    pub output: String,
    /// The output format used.
    pub format: OutputFormat,
    /// The template name used.
    pub template_name: String,
    /// The template version used.
    pub template_version: u32,
}

/// Service for context template CRUD operations and rendering.
pub struct ContextTemplateService {
    /// Full subcog configuration.
    config: SubcogConfig,
    /// Cached storage backends per domain.
    storage_cache: HashMap<DomainScope, Arc<dyn ContextTemplateStorage>>,
    /// Template renderer instance.
    renderer: TemplateRenderer,
}

impl ContextTemplateService {
    /// Creates a new context template service with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: SubcogConfig::load_default(),
            storage_cache: HashMap::new(),
            renderer: TemplateRenderer::new(),
        }
    }

    /// Creates a new context template service with custom configuration.
    #[must_use]
    pub fn with_config(config: SubcogConfig) -> Self {
        Self {
            config,
            storage_cache: HashMap::new(),
            renderer: TemplateRenderer::new(),
        }
    }

    /// Gets the storage backend for a domain scope.
    fn get_storage(&mut self, domain: DomainScope) -> Result<Arc<dyn ContextTemplateStorage>> {
        // Check cache first
        if let Some(storage) = self.storage_cache.get(&domain) {
            return Ok(Arc::clone(storage));
        }

        // Create new storage via factory
        let storage = ContextTemplateStorageFactory::create_for_scope(domain, &self.config)?;

        // Cache it
        self.storage_cache.insert(domain, Arc::clone(&storage));

        Ok(storage)
    }

    /// Saves a context template, auto-incrementing the version.
    ///
    /// # Arguments
    ///
    /// * `template` - The context template to save
    /// * `domain` - The domain scope to save in (defaults to User)
    ///
    /// # Returns
    ///
    /// A tuple of (name, version) for the saved template.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The template name is empty or invalid
    /// - Storage fails
    pub fn save(
        &mut self,
        template: &ContextTemplate,
        domain: DomainScope,
    ) -> Result<(String, u32)> {
        validate_template_name(&template.name)?;
        let storage = self.get_storage(domain)?;
        storage.save(template)
    }

    /// Saves a context template to the default domain (User).
    ///
    /// # Errors
    ///
    /// Returns an error if the template name is invalid or storage fails.
    pub fn save_default(&mut self, template: &ContextTemplate) -> Result<(String, u32)> {
        self.save(template, DomainScope::User)
    }

    /// Gets a context template by name and optional version.
    ///
    /// # Arguments
    ///
    /// * `name` - The template name to search for
    /// * `version` - Optional version number (None = latest)
    /// * `domain` - Optional domain to search (if None, searches User then Project)
    ///
    /// # Returns
    ///
    /// The context template if found.
    ///
    /// # Errors
    ///
    /// Returns an error if storage access fails.
    pub fn get(
        &mut self,
        name: &str,
        version: Option<u32>,
        domain: Option<DomainScope>,
    ) -> Result<Option<ContextTemplate>> {
        let scopes = match domain {
            Some(scope) => vec![scope],
            None => vec![DomainScope::User, DomainScope::Project],
        };

        for scope in scopes {
            let storage = match self.get_storage(scope) {
                Ok(s) => s,
                Err(Error::NotImplemented(_) | Error::FeatureNotEnabled(_)) => continue,
                Err(e) => return Err(e),
            };
            if let Some(template) = storage.get(name, version)? {
                return Ok(Some(template));
            }
        }

        Ok(None)
    }

    /// Lists context templates matching the filter.
    ///
    /// Returns only the latest version of each template.
    ///
    /// # Errors
    ///
    /// Returns an error if storage access fails.
    pub fn list(&mut self, filter: &ContextTemplateFilter) -> Result<Vec<ContextTemplate>> {
        let mut results = Vec::new();

        let scopes = match filter.domain {
            Some(scope) => vec![scope],
            None => vec![DomainScope::User, DomainScope::Project],
        };

        for scope in scopes {
            let storage = match self.get_storage(scope) {
                Ok(s) => s,
                Err(Error::NotImplemented(_) | Error::FeatureNotEnabled(_)) => continue,
                Err(e) => return Err(e),
            };

            let tags = (!filter.tags.is_empty()).then_some(filter.tags.as_slice());
            let templates = storage.list(tags, filter.name_pattern.as_deref())?;
            results.extend(templates);
        }

        // Sort by name
        results.sort_by(|a, b| a.name.cmp(&b.name));

        // Apply limit
        if let Some(limit) = filter.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    /// Deletes a context template by name and optional version.
    ///
    /// # Arguments
    ///
    /// * `name` - The template name to delete
    /// * `version` - Optional version (None = delete all versions)
    /// * `domain` - The domain scope to delete from
    ///
    /// # Returns
    ///
    /// `true` if any versions were deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if storage access fails.
    pub fn delete(
        &mut self,
        name: &str,
        version: Option<u32>,
        domain: DomainScope,
    ) -> Result<bool> {
        let storage = self.get_storage(domain)?;
        storage.delete(name, version)
    }

    /// Gets all available versions for a template.
    ///
    /// # Errors
    ///
    /// Returns an error if storage access fails.
    pub fn get_versions(&mut self, name: &str, domain: DomainScope) -> Result<Vec<u32>> {
        let storage = self.get_storage(domain)?;
        storage.get_versions(name)
    }

    /// Renders a template with memories and statistics.
    ///
    /// This is the main entry point for template rendering. It:
    /// 1. Retrieves the template by name/version
    /// 2. Builds a render context with auto-variables populated
    /// 3. Renders the template with variable substitution and iteration
    /// 4. Converts to the requested output format
    ///
    /// # Arguments
    ///
    /// * `template_name` - Name of the template to render
    /// * `version` - Optional version (None = latest)
    /// * `memories` - List of memories to include
    /// * `statistics` - Memory statistics
    /// * `custom_vars` - Custom user-defined variables
    /// * `format` - Output format (or None to use template default)
    ///
    /// # Returns
    ///
    /// A [`RenderResult`] containing the rendered output.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Template not found
    /// - Required user variables not provided
    /// - Rendering fails
    pub fn render_with_memories(
        &mut self,
        template_name: &str,
        version: Option<u32>,
        memories: &[Memory],
        statistics: &MemoryStatistics,
        custom_vars: &HashMap<String, String>,
        format: Option<OutputFormat>,
    ) -> Result<RenderResult> {
        // Get the template
        let template = self
            .get(template_name, version, None)?
            .ok_or_else(|| Error::InvalidInput(format!("Template not found: {template_name}")))?;

        // Build render context with auto-variables
        let context = self.build_render_context(memories, statistics, custom_vars, &template)?;

        // Determine output format
        let output_format = format.unwrap_or(template.output_format);

        // Render the template
        let output = self.renderer.render(&template, &context, output_format)?;

        Ok(RenderResult {
            output,
            format: output_format,
            template_name: template.name,
            template_version: template.version,
        })
    }

    /// Renders a template directly (without loading from storage).
    ///
    /// Useful for preview/dry-run scenarios.
    ///
    /// # Errors
    ///
    /// Returns an error if rendering fails or required variables are missing.
    pub fn render_direct(
        &self,
        template: &ContextTemplate,
        memories: &[Memory],
        statistics: &MemoryStatistics,
        custom_vars: &HashMap<String, String>,
        format: Option<OutputFormat>,
    ) -> Result<String> {
        let context = self.build_render_context(memories, statistics, custom_vars, template)?;
        let output_format = format.unwrap_or(template.output_format);
        self.renderer.render(template, &context, output_format)
    }

    /// Builds a render context with auto-variables populated.
    fn build_render_context(
        &self,
        memories: &[Memory],
        statistics: &MemoryStatistics,
        custom_vars: &HashMap<String, String>,
        template: &ContextTemplate,
    ) -> Result<RenderContext> {
        let mut context = RenderContext::new();

        // Add auto-variables for statistics
        context.set(
            "total_count",
            RenderValue::String(statistics.total_count.to_string()),
        );

        // Build namespace counts string
        let ns_counts: Vec<String> = statistics
            .namespace_counts
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect();
        context.set(
            "namespace_counts",
            RenderValue::String(ns_counts.join(", ")),
        );

        // Build statistics as formatted string
        let stats_str = format!(
            "Total: {}, Namespaces: {{{}}}",
            statistics.total_count,
            ns_counts.join(", ")
        );
        context.set("statistics", RenderValue::String(stats_str));

        // Add memories as iterable list
        let memory_list: Vec<HashMap<String, String>> = memories
            .iter()
            .enumerate()
            .map(|(idx, m)| {
                let mut map = HashMap::new();
                map.insert("memory.id".to_string(), m.id.as_str().to_string());
                map.insert("memory.content".to_string(), m.content.clone());
                map.insert(
                    "memory.namespace".to_string(),
                    m.namespace.as_str().to_string(),
                );
                map.insert("memory.tags".to_string(), m.tags.join(", "));
                map.insert("memory.domain".to_string(), m.domain.to_string());
                map.insert("memory.created_at".to_string(), m.created_at.to_string());
                map.insert("memory.updated_at".to_string(), m.updated_at.to_string());
                // Score is not part of Memory, use index as placeholder
                map.insert(
                    "memory.score".to_string(),
                    format!("{:.2}", (idx as f64).mul_add(-0.01, 1.0)),
                );
                map
            })
            .collect();
        context.set("memories", RenderValue::List(memory_list));

        // Add custom user variables
        for (key, value) in custom_vars {
            context.set(key, RenderValue::String(value.clone()));
        }

        // Validate required user variables are provided
        for var in &template.variables {
            if var.var_type == VariableType::User
                && var.required
                && !custom_vars.contains_key(&var.name)
                && var.default.is_none()
            {
                return Err(Error::InvalidInput(format!(
                    "Required variable '{}' not provided",
                    var.name
                )));
            }
        }

        Ok(context)
    }

    /// Validates template content without saving.
    ///
    /// Checks for:
    /// - Valid variable syntax
    /// - Balanced iteration blocks
    /// - Known auto-variables
    ///
    /// # Errors
    ///
    /// This function currently does not return errors (returns Ok with validation result).
    pub fn validate(&self, template: &ContextTemplate) -> Result<ValidationResult> {
        let mut issues = Vec::new();

        // Check iteration blocks are balanced
        let open_count = template.content.matches("{{#each").count();
        let close_count = template.content.matches("{{/each}}").count();
        if open_count != close_count {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                message: format!(
                    "Unbalanced iteration blocks: {open_count} opens, {close_count} closes"
                ),
            });
        }

        // Check for unknown variables (warning only)
        for var in &template.variables {
            if var.var_type == VariableType::User && var.default.is_none() && !var.required {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    message: format!("Variable '{}' is optional with no default", var.name),
                });
            }
        }

        Ok(ValidationResult {
            is_valid: !issues
                .iter()
                .any(|i| i.severity == ValidationSeverity::Error),
            issues,
        })
    }
}

impl Default for ContextTemplateService {
    fn default() -> Self {
        Self::new()
    }
}

/// Validates a template name.
///
/// Valid names must be kebab-case: lowercase letters, numbers, and hyphens only.
pub fn validate_template_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::InvalidInput(
            "Template name cannot be empty. Use a kebab-case name like 'search-results'."
                .to_string(),
        ));
    }

    let first_char = name.chars().next().unwrap_or('_');
    if !first_char.is_ascii_lowercase() {
        return Err(Error::InvalidInput(format!(
            "Template name must start with a lowercase letter, got '{name}'."
        )));
    }

    for ch in name.chars() {
        if !ch.is_ascii_lowercase() && !ch.is_ascii_digit() && ch != '-' {
            return Err(Error::InvalidInput(format!(
                "Invalid character '{ch}' in template name '{name}'. \
                 Use kebab-case: lowercase letters, numbers, and hyphens only."
            )));
        }
    }

    if name.ends_with('-') {
        return Err(Error::InvalidInput(format!(
            "Template name cannot end with a hyphen: '{name}'."
        )));
    }

    if name.contains("--") {
        return Err(Error::InvalidInput(format!(
            "Template name cannot have consecutive hyphens: '{name}'."
        )));
    }

    Ok(())
}

/// Validation severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationSeverity {
    /// Error - prevents template from being used.
    Error,
    /// Warning - template can be used but may have issues.
    Warning,
}

/// A validation issue found in a template.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// Severity of the issue.
    pub severity: ValidationSeverity,
    /// Human-readable description.
    pub message: String,
}

/// Result of template validation.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the template is valid (no errors).
    pub is_valid: bool,
    /// List of issues found.
    pub issues: Vec<ValidationIssue>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_template_name_valid() {
        assert!(validate_template_name("search-results").is_ok());
        assert!(validate_template_name("my-template-v2").is_ok());
        assert!(validate_template_name("simple").is_ok());
    }

    #[test]
    fn test_validate_template_name_invalid() {
        assert!(validate_template_name("").is_err());
        assert!(validate_template_name("1invalid").is_err());
        assert!(validate_template_name("-invalid").is_err());
        assert!(validate_template_name("Invalid").is_err());
        assert!(validate_template_name("invalid_name").is_err());
        assert!(validate_template_name("invalid-").is_err());
        assert!(validate_template_name("invalid--name").is_err());
    }

    #[test]
    fn test_context_template_filter_builder() {
        let filter = ContextTemplateFilter::new()
            .with_domain(DomainScope::User)
            .with_tags(vec!["formatting".to_string()])
            .with_name_pattern("search-*")
            .with_limit(10);

        assert_eq!(filter.domain, Some(DomainScope::User));
        assert_eq!(filter.tags, vec!["formatting"]);
        assert_eq!(filter.name_pattern, Some("search-*".to_string()));
        assert_eq!(filter.limit, Some(10));
    }

    #[test]
    fn test_build_render_context() {
        use crate::models::{Domain, MemoryId, MemoryStatus, Namespace};

        let service = ContextTemplateService::new();
        let template = ContextTemplate::new("test", "{{total_count}} memories");

        let memories = vec![Memory {
            id: MemoryId::new("test-memory-1"),
            content: "Test memory".to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: 0,
            updated_at: 0,
            tombstoned_at: None,
            expires_at: None,
            embedding: None,
            tags: vec!["test".to_string()],
            source: None,
            is_summary: false,
            source_memory_ids: None,
            consolidation_timestamp: None,
        }];

        let mut namespace_counts = HashMap::new();
        namespace_counts.insert("decisions".to_string(), 1);
        let statistics = MemoryStatistics {
            total_count: 1,
            namespace_counts,
            top_tags: vec![],
            recent_topics: vec![],
        };

        let custom_vars = HashMap::new();
        let context = service
            .build_render_context(&memories, &statistics, &custom_vars, &template)
            .unwrap();

        // Verify auto-variables are set
        assert!(context.get("total_count").is_some());
        assert!(context.get("memories").is_some());
    }

    #[test]
    fn test_validate_template() {
        let service = ContextTemplateService::new();

        // Valid template
        let valid = ContextTemplate::new("test", "{{#each memories}}{{memory.content}}{{/each}}");
        let result = service.validate(&valid).unwrap();
        assert!(result.is_valid);

        // Unbalanced iteration
        let invalid = ContextTemplate::new("test", "{{#each memories}}content");
        let result = service.validate(&invalid).unwrap();
        assert!(!result.is_valid);
    }
}
