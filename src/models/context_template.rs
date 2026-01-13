//! Context template models.
//!
//! Provides data structures for user-defined context templates that format memories
//! and statistics for hooks and MCP tool responses.
//!
//! # Variable Types
//!
//! Context templates support two types of variables:
//!
//! 1. **Auto-variables**: Automatically populated from memory context
//!    - `{{memories}}` - List of memories for iteration
//!    - `{{memory.id}}`, `{{memory.content}}`, etc. - Individual memory fields (in iteration)
//!    - `{{statistics}}` - Memory statistics object
//!    - `{{total_count}}`, `{{namespace_counts}}` - Statistics fields
//!
//! 2. **User-variables**: Custom variables provided at render time
//!    - Any `{{variable}}` not in the auto-variable list
//!
//! # Iteration Syntax
//!
//! Templates support iteration over collections using `{{#each}}...{{/each}}`:
//!
//! ```text
//! {{#each memories}}
//! - **{{memory.namespace}}**: {{memory.content}}
//! {{/each}}
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::models::prompt::{ExtractedVariable, PromptVariable, extract_variables};

/// Auto-variable names recognized by the system.
///
/// These variables are automatically populated from the render context
/// and do not need to be provided by the user.
pub const AUTO_VARIABLES: &[&str] = &[
    // Iteration collection
    "memories",
    // Individual memory fields (used inside {{#each memories}})
    "memory.id",
    "memory.content",
    "memory.namespace",
    "memory.tags",
    "memory.score",
    "memory.created_at",
    "memory.updated_at",
    "memory.domain",
    // Statistics
    "statistics",
    "total_count",
    "namespace_counts",
];

/// Auto-variable prefixes for iteration context.
///
/// Variables starting with these prefixes are auto-variables when inside iteration blocks.
pub const AUTO_VARIABLE_PREFIXES: &[&str] = &["memory."];

/// Checks if a variable name is an auto-variable.
#[must_use]
pub fn is_auto_variable(name: &str) -> bool {
    AUTO_VARIABLES.contains(&name)
        || AUTO_VARIABLE_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

/// Output format for rendered templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Markdown output (default).
    #[default]
    Markdown,
    /// JSON output.
    Json,
    /// XML output.
    Xml,
}

impl OutputFormat {
    /// Returns the format as a string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Json => "json",
            Self::Xml => "xml",
        }
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for OutputFormat {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "markdown" | "md" => Ok(Self::Markdown),
            "json" => Ok(Self::Json),
            "xml" => Ok(Self::Xml),
            _ => Err(crate::Error::InvalidInput(format!(
                "Invalid output format: {s}. Expected: markdown, json, or xml"
            ))),
        }
    }
}

/// Type of template variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VariableType {
    /// User-provided custom variable.
    #[default]
    User,
    /// Auto-populated from memory context.
    Auto,
}

impl VariableType {
    /// Returns the type as a string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Auto => "auto",
        }
    }
}

impl fmt::Display for VariableType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A variable in a context template.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TemplateVariable {
    /// Variable name (without braces).
    pub name: String,
    /// Type of variable (user or auto).
    #[serde(default)]
    pub var_type: VariableType,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// Default value if not provided.
    #[serde(default)]
    pub default: Option<String>,
    /// Whether the variable is required.
    #[serde(default = "default_required")]
    pub required: bool,
}

/// Default value for `required` field (true for user variables, false for auto).
const fn default_required() -> bool {
    true
}

impl TemplateVariable {
    /// Creates a new user variable.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let var_type = if is_auto_variable(&name) {
            VariableType::Auto
        } else {
            VariableType::User
        };
        let required = var_type == VariableType::User;

        Self {
            name,
            var_type,
            description: None,
            default: None,
            required,
        }
    }

    /// Creates a new auto-variable.
    #[must_use]
    pub fn auto(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            var_type: VariableType::Auto,
            description: None,
            default: None,
            required: false,
        }
    }

    /// Sets the description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the default value.
    #[must_use]
    pub fn with_default(mut self, default: impl Into<String>) -> Self {
        self.default = Some(default.into());
        self.required = false;
        self
    }
}

impl Default for TemplateVariable {
    fn default() -> Self {
        Self {
            name: String::new(),
            var_type: VariableType::User,
            description: None,
            default: None,
            required: true,
        }
    }
}

impl From<ExtractedVariable> for TemplateVariable {
    fn from(extracted: ExtractedVariable) -> Self {
        Self::new(extracted.name)
    }
}

impl From<PromptVariable> for TemplateVariable {
    fn from(prompt_var: PromptVariable) -> Self {
        Self {
            name: prompt_var.name,
            var_type: VariableType::User,
            description: prompt_var.description,
            default: prompt_var.default,
            required: prompt_var.required,
        }
    }
}

impl From<TemplateVariable> for PromptVariable {
    fn from(template_var: TemplateVariable) -> Self {
        Self {
            name: template_var.name,
            description: template_var.description,
            default: template_var.default,
            required: template_var.required,
        }
    }
}

/// A user-defined context template.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextTemplate {
    /// Unique template name (kebab-case).
    pub name: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: String,
    /// The template content with `{{variable}}` placeholders.
    pub content: String,
    /// Extracted variables with metadata.
    #[serde(default)]
    pub variables: Vec<TemplateVariable>,
    /// Categorization tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Default output format for rendering.
    #[serde(default)]
    pub output_format: OutputFormat,
    /// Author identifier.
    #[serde(default)]
    pub author: Option<String>,
    /// Version number (auto-incremented on save).
    #[serde(default = "default_version")]
    pub version: u32,
    /// Creation timestamp (Unix epoch seconds).
    #[serde(default)]
    pub created_at: u64,
    /// Last update timestamp (Unix epoch seconds).
    #[serde(default)]
    pub updated_at: u64,
}

/// Default version number.
const fn default_version() -> u32 {
    1
}

impl ContextTemplate {
    /// Creates a new context template with the given name and content.
    ///
    /// Variables are automatically extracted from the content and classified
    /// as user or auto variables.
    #[must_use]
    pub fn new(name: impl Into<String>, content: impl Into<String>) -> Self {
        let content = content.into();
        let variables = extract_variables(&content)
            .into_iter()
            .map(TemplateVariable::from)
            .collect();

        Self {
            name: name.into(),
            description: String::new(),
            content,
            variables,
            tags: Vec::new(),
            output_format: OutputFormat::default(),
            author: None,
            version: 1,
            created_at: 0,
            updated_at: 0,
        }
    }

    /// Sets the description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Sets the tags.
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Sets the author.
    #[must_use]
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Sets explicit variable definitions, overriding auto-detected ones.
    #[must_use]
    pub fn with_variables(mut self, variables: Vec<TemplateVariable>) -> Self {
        self.variables = variables;
        self
    }

    /// Sets the version.
    #[must_use]
    pub const fn with_version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    /// Returns the list of variable names in this template.
    #[must_use]
    pub fn variable_names(&self) -> Vec<&str> {
        self.variables.iter().map(|v| v.name.as_str()).collect()
    }

    /// Returns only user-defined variables (not auto-variables).
    #[must_use]
    pub fn user_variables(&self) -> Vec<&TemplateVariable> {
        self.variables
            .iter()
            .filter(|v| v.var_type == VariableType::User)
            .collect()
    }

    /// Returns only auto-variables.
    #[must_use]
    pub fn auto_variables(&self) -> Vec<&TemplateVariable> {
        self.variables
            .iter()
            .filter(|v| v.var_type == VariableType::Auto)
            .collect()
    }

    /// Checks if the template uses iteration (has `{{#each}}` blocks).
    #[must_use]
    pub fn has_iteration(&self) -> bool {
        self.content.contains("{{#each")
    }

    /// Returns the iteration collection names used in the template.
    #[must_use]
    pub fn iteration_collections(&self) -> Vec<&str> {
        let mut collections = Vec::new();
        let mut search_start = 0;

        while let Some(start) = self.content[search_start..].find("{{#each ") {
            let abs_start = search_start + start + 8; // Skip "{{#each "
            let Some(end) = self.content[abs_start..].find("}}") else {
                break;
            };
            let collection = self.content[abs_start..abs_start + end].trim();
            if !collections.contains(&collection) {
                collections.push(collection);
            }
            search_start = abs_start + end;
        }

        collections
    }
}

/// Version metadata for a context template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVersion {
    /// Version number.
    pub version: u32,
    /// Creation timestamp.
    pub created_at: u64,
    /// Author of this version.
    pub author: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_auto_variable() {
        // Direct matches
        assert!(is_auto_variable("memories"));
        assert!(is_auto_variable("memory.id"));
        assert!(is_auto_variable("memory.content"));
        assert!(is_auto_variable("statistics"));
        assert!(is_auto_variable("total_count"));

        // Prefix matches
        assert!(is_auto_variable("memory.custom_field"));

        // User variables
        assert!(!is_auto_variable("user_name"));
        assert!(!is_auto_variable("custom_var"));
        assert!(!is_auto_variable("my_memories")); // Not a prefix match
    }

    #[test]
    fn test_output_format_parsing() {
        assert_eq!(
            "markdown".parse::<OutputFormat>().unwrap(),
            OutputFormat::Markdown
        );
        assert_eq!(
            "md".parse::<OutputFormat>().unwrap(),
            OutputFormat::Markdown
        );
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!("xml".parse::<OutputFormat>().unwrap(), OutputFormat::Xml);
        assert!("invalid".parse::<OutputFormat>().is_err());
    }

    #[test]
    fn test_output_format_display() {
        assert_eq!(OutputFormat::Markdown.to_string(), "markdown");
        assert_eq!(OutputFormat::Json.to_string(), "json");
        assert_eq!(OutputFormat::Xml.to_string(), "xml");
    }

    #[test]
    fn test_template_variable_new() {
        // User variable
        let user_var = TemplateVariable::new("user_name");
        assert_eq!(user_var.var_type, VariableType::User);
        assert!(user_var.required);

        // Auto variable
        let auto_var = TemplateVariable::new("memory.id");
        assert_eq!(auto_var.var_type, VariableType::Auto);
        assert!(!auto_var.required);
    }

    #[test]
    fn test_template_variable_builders() {
        let var = TemplateVariable::new("name")
            .with_description("User's name")
            .with_default("Anonymous");

        assert_eq!(var.description, Some("User's name".to_string()));
        assert_eq!(var.default, Some("Anonymous".to_string()));
        assert!(!var.required); // Setting default makes it optional
    }

    #[test]
    fn test_context_template_new() {
        let template = ContextTemplate::new(
            "test-template",
            "Hello {{user_name}}, you have {{total_count}} memories.",
        );

        assert_eq!(template.name, "test-template");
        assert_eq!(template.variables.len(), 2);

        // Check variable classification
        let user_var = template
            .variables
            .iter()
            .find(|v| v.name == "user_name")
            .unwrap();
        assert_eq!(user_var.var_type, VariableType::User);

        let auto_var = template
            .variables
            .iter()
            .find(|v| v.name == "total_count")
            .unwrap();
        assert_eq!(auto_var.var_type, VariableType::Auto);
    }

    #[test]
    fn test_context_template_builders() {
        let template = ContextTemplate::new("test", "{{var}}")
            .with_description("A test template")
            .with_tags(vec!["test".to_string(), "example".to_string()])
            .with_author("test-user")
            .with_version(5);

        assert_eq!(template.description, "A test template");
        assert_eq!(template.tags, vec!["test", "example"]);
        assert_eq!(template.author, Some("test-user".to_string()));
        assert_eq!(template.version, 5);
    }

    #[test]
    fn test_context_template_variable_helpers() {
        let template = ContextTemplate::new("test", "{{user_var}} {{memory.id}} {{total_count}}");

        let user_vars = template.user_variables();
        assert_eq!(user_vars.len(), 1);
        assert_eq!(user_vars[0].name, "user_var");

        let auto_vars = template.auto_variables();
        assert_eq!(auto_vars.len(), 2);
    }

    #[test]
    fn test_context_template_has_iteration() {
        let with_iteration =
            ContextTemplate::new("test", "{{#each memories}}{{memory.id}}{{/each}}");
        assert!(with_iteration.has_iteration());

        let without_iteration = ContextTemplate::new("test", "{{total_count}}");
        assert!(!without_iteration.has_iteration());
    }

    #[test]
    fn test_context_template_iteration_collections() {
        let template = ContextTemplate::new(
            "test",
            "{{#each memories}}{{memory.id}}{{/each}} and {{#each items}}{{item.name}}{{/each}}",
        );

        let collections = template.iteration_collections();
        assert_eq!(collections.len(), 2);
        assert!(collections.contains(&"memories"));
        assert!(collections.contains(&"items"));
    }

    #[test]
    fn test_context_template_serialization() {
        let template = ContextTemplate::new("test", "{{var}}")
            .with_description("A test")
            .with_tags(vec!["tag1".to_string()]);

        let json = serde_json::to_string(&template).unwrap();
        let parsed: ContextTemplate = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, "test");
        assert_eq!(parsed.description, "A test");
        assert_eq!(parsed.tags, vec!["tag1"]);
    }

    #[test]
    fn test_template_variable_from_extracted() {
        let extracted = ExtractedVariable {
            name: "memory.content".to_string(),
            position: 0,
        };
        let var: TemplateVariable = extracted.into();

        assert_eq!(var.name, "memory.content");
        assert_eq!(var.var_type, VariableType::Auto);
    }

    #[test]
    fn test_template_variable_from_prompt_variable() {
        let prompt_var = PromptVariable {
            name: "user_input".to_string(),
            description: Some("User input".to_string()),
            default: Some("default".to_string()),
            required: false,
        };
        let var: TemplateVariable = prompt_var.into();

        assert_eq!(var.name, "user_input");
        assert_eq!(var.var_type, VariableType::User);
        assert_eq!(var.description, Some("User input".to_string()));
        assert_eq!(var.default, Some("default".to_string()));
    }

    #[test]
    fn test_template_variable_to_prompt_variable() {
        let var = TemplateVariable::new("test")
            .with_description("Test var")
            .with_default("default");

        let prompt_var: PromptVariable = var.into();

        assert_eq!(prompt_var.name, "test");
        assert_eq!(prompt_var.description, Some("Test var".to_string()));
        assert_eq!(prompt_var.default, Some("default".to_string()));
    }
}
