//! Prompt template models.
//!
//! Provides data structures for user-defined prompt templates with variable substitution.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;
use std::sync::LazyLock;

use crate::{Error, Result};

/// Regex pattern for extracting template variables: `{{variable_name}}`.
static VARIABLE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // SAFETY: This regex is compile-time verified and cannot fail
    Regex::new(r"\{\{(\w+)\}\}").unwrap_or_else(|_| unreachable!())
});

/// Regex pattern for detecting any content between `{{` and `}}`.
static VALIDATION_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // SAFETY: This regex is compile-time verified and cannot fail
    Regex::new(r"\{\{([^}]*)\}\}").unwrap_or_else(|_| unreachable!())
});

/// A user-defined prompt template.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptTemplate {
    /// Unique prompt name (kebab-case).
    pub name: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: String,
    /// The prompt content with `{{variable}}` placeholders.
    pub content: String,
    /// Extracted variables with optional metadata.
    #[serde(default)]
    pub variables: Vec<PromptVariable>,
    /// Categorization tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Author identifier.
    #[serde(default)]
    pub author: Option<String>,
    /// Usage count for popularity ranking.
    #[serde(default)]
    pub usage_count: u64,
    /// Creation timestamp (Unix epoch seconds).
    #[serde(default)]
    pub created_at: u64,
    /// Last update timestamp (Unix epoch seconds).
    #[serde(default)]
    pub updated_at: u64,
}

impl PromptTemplate {
    /// Creates a new prompt template with the given name and content.
    #[must_use]
    pub fn new(name: impl Into<String>, content: impl Into<String>) -> Self {
        let content = content.into();
        let variables = extract_variables(&content)
            .into_iter()
            .map(|v| PromptVariable {
                name: v.name,
                description: None,
                default: None,
                required: true,
            })
            .collect();

        Self {
            name: name.into(),
            description: String::new(),
            content,
            variables,
            tags: Vec::new(),
            author: None,
            usage_count: 0,
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
    pub fn with_variables(mut self, variables: Vec<PromptVariable>) -> Self {
        self.variables = variables;
        self
    }

    /// Returns the list of variable names in this template.
    #[must_use]
    pub fn variable_names(&self) -> Vec<&str> {
        self.variables.iter().map(|v| v.name.as_str()).collect()
    }

    /// Populates the template with the given variable values.
    ///
    /// # Errors
    ///
    /// Returns an error if a required variable is missing and has no default.
    pub fn populate(&self, values: &HashMap<String, String>) -> Result<String> {
        substitute_variables(&self.content, values, &self.variables)
    }
}

/// A template variable definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromptVariable {
    /// Variable name (without braces).
    pub name: String,
    /// Human-readable description for elicitation.
    #[serde(default)]
    pub description: Option<String>,
    /// Default value if not provided.
    #[serde(default)]
    pub default: Option<String>,
    /// Whether the variable is required.
    #[serde(default = "default_required")]
    pub required: bool,
}

/// Default value for `required` field (true).
const fn default_required() -> bool {
    true
}

impl PromptVariable {
    /// Creates a new required variable.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            default: None,
            required: true,
        }
    }

    /// Creates a new optional variable with a default value.
    #[must_use]
    pub fn optional(name: impl Into<String>, default: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            default: Some(default.into()),
            required: false,
        }
    }

    /// Sets the description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

impl Default for PromptVariable {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: None,
            default: None,
            required: true,
        }
    }
}

/// Result of extracting a variable from prompt content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedVariable {
    /// Variable name (without braces).
    pub name: String,
    /// Byte position in the content where the variable starts.
    pub position: usize,
}

/// Extracts variables from prompt content.
///
/// Variables are identified by the pattern `{{variable_name}}` where `variable_name`
/// consists of alphanumeric characters and underscores.
///
/// # Returns
///
/// A list of extracted variables in order of first appearance, deduplicated.
#[must_use]
pub fn extract_variables(content: &str) -> Vec<ExtractedVariable> {
    let mut seen = HashSet::new();
    let mut variables = Vec::new();

    for cap in VARIABLE_PATTERN.captures_iter(content) {
        if let Some(name_match) = cap.get(1) {
            let name = name_match.as_str().to_string();
            if seen.insert(name.clone()) {
                variables.push(ExtractedVariable {
                    name,
                    position: cap.get(0).map_or(0, |m| m.start()),
                });
            }
        }
    }

    variables
}

/// Substitutes variables in prompt content.
///
/// # Arguments
///
/// * `content` - The template content with `{{variable}}` placeholders.
/// * `values` - A map of variable names to their values.
/// * `variables` - Variable definitions for defaults and required checks.
///
/// # Errors
///
/// Returns an error if a required variable is missing and has no default.
pub fn substitute_variables<S: BuildHasher>(
    content: &str,
    values: &HashMap<String, String, S>,
    variables: &[PromptVariable],
) -> Result<String> {
    // Build effective values map with defaults
    let mut effective_values: HashMap<String, String> = HashMap::new();

    // Add provided values
    for (k, v) in values {
        effective_values.insert(k.clone(), v.clone());
    }

    // Apply defaults and check required
    for var in variables {
        if !effective_values.contains_key(&var.name) {
            if let Some(default) = &var.default {
                effective_values.insert(var.name.clone(), default.clone());
            } else if var.required {
                return Err(Error::InvalidInput(format!(
                    "Missing required variable '{}'. Provide it with: --var {}=VALUE",
                    var.name, var.name
                )));
            }
        }
    }

    // Check for variables in content that aren't in the variables list
    for extracted in extract_variables(content) {
        if !effective_values.contains_key(&extracted.name) {
            // Variable found in content but not provided and not in definitions
            // For flexibility, we'll just leave it unreplaced or error
            return Err(Error::InvalidInput(format!(
                "Missing variable '{}'. Provide it with: --var {}=VALUE",
                extracted.name, extracted.name
            )));
        }
    }

    // Perform substitution
    let result = VARIABLE_PATTERN
        .replace_all(content, |caps: &regex::Captures| {
            caps.get(1)
                .and_then(|m| effective_values.get(m.as_str()))
                .map_or_else(|| caps[0].to_string(), String::clone)
        })
        .to_string();

    Ok(result)
}

/// Validation result for prompt content.
#[derive(Debug, Clone, Default)]
pub struct ValidationResult {
    /// Whether the prompt is valid.
    pub is_valid: bool,
    /// List of issues found.
    pub issues: Vec<ValidationIssue>,
}

impl ValidationResult {
    /// Creates a valid result with no issues.
    #[must_use]
    pub const fn valid() -> Self {
        Self {
            is_valid: true,
            issues: Vec::new(),
        }
    }

    /// Creates an invalid result with the given issues.
    #[must_use]
    pub const fn invalid(issues: Vec<ValidationIssue>) -> Self {
        Self {
            is_valid: false,
            issues,
        }
    }

    /// Adds an issue and marks the result as invalid.
    pub fn add_issue(&mut self, issue: ValidationIssue) {
        self.is_valid = false;
        self.issues.push(issue);
    }
}

/// A validation issue found in prompt content.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// Severity of the issue.
    pub severity: IssueSeverity,
    /// Description of the issue.
    pub message: String,
    /// Byte position in the content where the issue was found.
    pub position: Option<usize>,
}

impl ValidationIssue {
    /// Creates a new error-level issue.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: IssueSeverity::Error,
            message: message.into(),
            position: None,
        }
    }

    /// Creates a new warning-level issue.
    #[must_use]
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: IssueSeverity::Warning,
            message: message.into(),
            position: None,
        }
    }

    /// Sets the position of the issue.
    #[must_use]
    pub const fn at_position(mut self, position: usize) -> Self {
        self.position = Some(position);
        self
    }
}

/// Severity level for validation issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueSeverity {
    /// Critical issue that must be fixed.
    Error,
    /// Non-critical issue that should be addressed.
    Warning,
}

/// Reserved variable name prefixes that cannot be used.
const RESERVED_PREFIXES: &[&str] = &["subcog_", "system_", "__"];

/// Checks if a variable name uses a reserved prefix.
#[must_use]
pub fn is_reserved_variable_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    RESERVED_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

/// Validates prompt content for common issues.
///
/// Checks for:
/// - Unclosed braces (e.g., `{{var` without closing `}}`)
/// - Invalid variable names (non-alphanumeric characters)
/// - Reserved variable names (e.g., `subcog_*`, `system_*`, `__*`)
/// - Duplicate variable definitions
///
/// # Returns
///
/// A validation result indicating whether the content is valid.
#[must_use]
pub fn validate_prompt_content(content: &str) -> ValidationResult {
    let mut result = ValidationResult::valid();
    let mut seen_names: HashSet<String> = HashSet::new();

    // Check for unclosed braces
    let open_count = content.matches("{{").count();
    let close_count = content.matches("}}").count();

    if open_count != close_count {
        result.add_issue(ValidationIssue::error(format!(
            "Unbalanced braces: {open_count} opening '{{{{' vs {close_count} closing '}}}}'"
        )));
    }

    // Check for single braces that might indicate typos
    // Pattern: single { not followed by { or single } not followed by }
    let mut i = 0;
    let bytes = content.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'{' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                // Valid opening {{, skip both
                i += 2;
                continue;
            }
            // Single { - might be intentional (like in code blocks)
            // Only warn if it looks like a malformed variable
            if i + 1 < bytes.len() && bytes[i + 1].is_ascii_alphabetic() {
                result.add_issue(
                    ValidationIssue::warning("Single '{' found - did you mean '{{'?")
                        .at_position(i),
                );
            }
        } else if bytes[i] == b'}' {
            // Check if this is the first } of a }} pair
            if i + 1 < bytes.len() && bytes[i + 1] == b'}' {
                // Valid closing }}, skip both
                i += 2;
                continue;
            }
            // Single } - warn if preceded by alphanumeric (likely typo)
            if i > 0 && bytes[i - 1].is_ascii_alphanumeric() {
                result.add_issue(
                    ValidationIssue::warning("Single '}' found - did you mean '}}'?")
                        .at_position(i),
                );
            }
        }
        i += 1;
    }

    // Check for invalid variable names (variables extracted but with issues)
    // The regex only matches valid names, so this catches edge cases
    // like {{123}} which wouldn't match \w+ starting with digit
    for cap in VALIDATION_PATTERN.captures_iter(content) {
        if let Some(inner) = cap.get(1) {
            let name = inner.as_str();
            if name.is_empty() {
                result.add_issue(
                    ValidationIssue::error("Empty variable name: {{}}").at_position(inner.start()),
                );
            } else if !name
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            {
                result.add_issue(
                    ValidationIssue::error(format!(
                        "Invalid variable name '{name}': must start with letter or underscore"
                    ))
                    .at_position(inner.start()),
                );
            } else if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                result.add_issue(
                    ValidationIssue::error(format!(
                        "Invalid variable name '{name}': contains invalid characters"
                    ))
                    .at_position(inner.start()),
                );
            } else if is_reserved_variable_name(name) {
                result.add_issue(
                    ValidationIssue::error(format!(
                        "Reserved variable name '{name}': cannot use 'subcog_', 'system_', or '__' prefix"
                    ))
                    .at_position(inner.start()),
                );
            } else if !seen_names.insert(name.to_lowercase()) {
                // Note: This is a warning, not an error, as duplicate variables
                // are functionally valid (just redundant)
                result.add_issue(
                    ValidationIssue::warning(format!("Duplicate variable name: '{name}'"))
                        .at_position(inner.start()),
                );
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_variables_simple() {
        let content = "Hello {{name}}, your {{item}} is ready.";
        let vars = extract_variables(content);

        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].name, "name");
        assert_eq!(vars[1].name, "item");
    }

    #[test]
    fn test_extract_variables_deduplicates() {
        let content = "{{name}} and {{name}} again, plus {{other}}.";
        let vars = extract_variables(content);

        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].name, "name");
        assert_eq!(vars[1].name, "other");
    }

    #[test]
    fn test_extract_variables_underscores() {
        let content = "{{user_name}} and {{item_count}}.";
        let vars = extract_variables(content);

        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].name, "user_name");
        assert_eq!(vars[1].name, "item_count");
    }

    #[test]
    fn test_extract_variables_empty() {
        let content = "No variables here.";
        let vars = extract_variables(content);

        assert!(vars.is_empty());
    }

    #[test]
    fn test_substitute_variables_complete() {
        let content = "Hello {{name}}, your {{item}} is ready.";
        let mut values = HashMap::new();
        values.insert("name".to_string(), "Alice".to_string());
        values.insert("item".to_string(), "order".to_string());

        let result = substitute_variables(content, &values, &[]).unwrap();
        assert_eq!(result, "Hello Alice, your order is ready.");
    }

    #[test]
    fn test_substitute_variables_with_defaults() {
        let content = "Hello {{name}}, status: {{status}}.";
        let mut values = HashMap::new();
        values.insert("name".to_string(), "Bob".to_string());

        let variables = vec![
            PromptVariable::new("name"),
            PromptVariable::optional("status", "pending"),
        ];

        let result = substitute_variables(content, &values, &variables).unwrap();
        assert_eq!(result, "Hello Bob, status: pending.");
    }

    #[test]
    fn test_substitute_variables_missing_required() {
        let content = "Hello {{name}}.";
        let values = HashMap::new();

        let variables = vec![PromptVariable::new("name")];

        let result = substitute_variables(content, &values, &variables);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Missing required variable"));
        assert!(err_msg.contains("--var name=VALUE"));
    }

    #[test]
    fn test_prompt_template_new() {
        let template = PromptTemplate::new("greeting", "Hello {{name}}!");

        assert_eq!(template.name, "greeting");
        assert_eq!(template.content, "Hello {{name}}!");
        assert_eq!(template.variables.len(), 1);
        assert_eq!(template.variables[0].name, "name");
    }

    #[test]
    fn test_prompt_template_populate() {
        let template = PromptTemplate::new("greeting", "Hello {{name}}!");

        let mut values = HashMap::new();
        values.insert("name".to_string(), "World".to_string());

        let result = template.populate(&values).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_prompt_template_serialization() {
        let template = PromptTemplate::new("test", "{{var}}")
            .with_description("A test prompt")
            .with_tags(vec!["test".to_string()]);

        let json = serde_json::to_string(&template).unwrap();
        let parsed: PromptTemplate = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, "test");
        assert_eq!(parsed.description, "A test prompt");
        assert_eq!(parsed.tags, vec!["test"]);
    }

    #[test]
    fn test_validate_prompt_content_valid() {
        let content = "Hello {{name}}, your {{item}} is ready.";
        let result = validate_prompt_content(content);

        assert!(result.is_valid);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_validate_prompt_content_unclosed_braces() {
        let content = "Hello {{name}, missing close.";
        let result = validate_prompt_content(content);

        assert!(!result.is_valid);
        assert!(
            result
                .issues
                .iter()
                .any(|i| i.message.contains("Unbalanced"))
        );
    }

    #[test]
    fn test_validate_prompt_content_empty_variable() {
        let content = "Hello {{}}, empty.";
        let result = validate_prompt_content(content);

        assert!(!result.is_valid);
        assert!(
            result
                .issues
                .iter()
                .any(|i| i.message.contains("Empty variable"))
        );
    }

    #[test]
    fn test_validate_prompt_content_invalid_name() {
        let content = "Hello {{123bad}}, invalid.";
        let result = validate_prompt_content(content);

        assert!(!result.is_valid);
        assert!(
            result
                .issues
                .iter()
                .any(|i| i.message.contains("must start with letter"))
        );
    }

    #[test]
    fn test_prompt_variable_builders() {
        let required = PromptVariable::new("name").with_description("User's name");
        assert!(required.required);
        assert_eq!(required.description, Some("User's name".to_string()));

        let optional = PromptVariable::optional("status", "pending");
        assert!(!optional.required);
        assert_eq!(optional.default, Some("pending".to_string()));
    }

    #[test]
    fn test_is_reserved_variable_name() {
        // Reserved prefixes
        assert!(is_reserved_variable_name("subcog_version"));
        assert!(is_reserved_variable_name("SUBCOG_CONFIG"));
        assert!(is_reserved_variable_name("system_path"));
        assert!(is_reserved_variable_name("System_User"));
        assert!(is_reserved_variable_name("__private"));
        assert!(is_reserved_variable_name("__init"));

        // Valid names
        assert!(!is_reserved_variable_name("name"));
        assert!(!is_reserved_variable_name("user_name"));
        assert!(!is_reserved_variable_name("mySubcog"));
        assert!(!is_reserved_variable_name("_underscore"));
    }

    #[test]
    fn test_validate_prompt_content_reserved_name() {
        let content = "Config: {{subcog_config}}";
        let result = validate_prompt_content(content);

        assert!(!result.is_valid);
        assert!(result.issues.iter().any(|i| i.message.contains("Reserved")));
    }

    #[test]
    fn test_validate_prompt_content_duplicate_variable() {
        let content = "Hello {{name}} and {{name}} again";
        let result = validate_prompt_content(content);

        // Should have a warning for duplicate, but still be functionally valid
        // (warnings don't make it invalid, only errors do)
        assert!(
            result
                .issues
                .iter()
                .any(|i| i.message.contains("Duplicate"))
        );
    }

    #[test]
    fn test_validate_prompt_content_system_prefix() {
        let content = "Path: {{system_path}}";
        let result = validate_prompt_content(content);

        assert!(!result.is_valid);
        assert!(result.issues.iter().any(|i| i.message.contains("system_")));
    }

    #[test]
    fn test_validate_prompt_content_double_underscore() {
        let content = "Private: {{__internal}}";
        let result = validate_prompt_content(content);

        assert!(!result.is_valid);
        assert!(result.issues.iter().any(|i| i.message.contains("__")));
    }
}
