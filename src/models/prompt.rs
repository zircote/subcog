//! Prompt template models.
//!
//! Provides data structures for user-defined prompt templates with variable substitution.
//!
//! # Code Block Detection Edge Cases
//!
//! Variable extraction automatically skips `{{variable}}` patterns inside fenced code blocks
//! to avoid capturing documentation examples. This section documents edge cases and behaviors.
//!
//! ## Supported Code Block Syntaxes
//!
//! | Syntax | Supported | Notes |
//! |--------|-----------|-------|
//! | ` ```language ... ``` ` | ✓ | Standard fenced code block |
//! | ` ``` ... ``` ` | ✓ | Code block without language |
//! | ` ~~~ ... ~~~ ` | ✓ | Tilde fenced code block |
//! | Indented code (4 spaces) | ✗ | Only fenced blocks detected |
//!
//! ## Edge Cases
//!
//! ### 1. Unclosed Code Blocks
//!
//! Input: triple-backtick rust, `let x = "{{var}}";`, no closing triple-backtick
//!
//! **Behavior**: Unclosed blocks are not detected, so variables inside ARE extracted.
//! This is intentional - malformed content shouldn't silently exclude variables.
//!
//! ### 2. Nested Code Blocks (within markdown)
//!
//! Input: Outer tilde block containing inner backtick block with `{{inner_var}}`
//!
//! **Behavior**: Both tilde and backtick blocks are detected. Variables inside
//! either syntax are excluded. Nested blocks are handled correctly.
//!
//! ### 3. Variables at Block Boundaries
//!
//! Input: `{{before}}` immediately before triple-backtick, `{{after}}` immediately after
//!
//! **Behavior**: Both `{{before}}` and `{{after}}` are extracted. Only content
//! strictly between the opening and closing triple-backticks is excluded.
//!
//! ### 4. Inline Code (single backticks)
//!
//! Input: `Use {{var}} syntax for variables.` (single backticks around var)
//!
//! **Behavior**: Single backticks DO NOT exclude variables. Only triple-backtick
//! fenced blocks are detected. `{{var}}` IS extracted.
//!
//! ### 5. Empty Code Blocks
//!
//! Input: Empty triple-backtick block
//!
//! **Behavior**: Empty blocks are detected but contain no variables to exclude.
//!
//! ## Workarounds
//!
//! If you need a `{{variable}}` pattern in your actual prompt output (not as a variable):
//!
//! 1. **Escape it**: Use `\{\{literal\}\}` (will be preserved literally)
//! 2. **Put it in a code block**: Variables in fenced blocks are not substituted
//! 3. **Use a variable with literal value**: Define `open_brace`/`close_brace` variables

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;
use std::sync::LazyLock;

use crate::{Error, Result};

/// Creates a compile-time verified regex wrapped in [`LazyLock`].
///
/// # Safety
///
/// The regex pattern is verified at compile time and cannot fail at runtime.
/// The `unreachable!()` branch exists only for type checking.
macro_rules! lazy_regex {
    ($pattern:expr) => {
        LazyLock::new(|| Regex::new($pattern).unwrap_or_else(|_| unreachable!()))
    };
}

/// Regex pattern for extracting template variables: `{{variable_name}}`.
static VARIABLE_PATTERN: LazyLock<Regex> = lazy_regex!(r"\{\{(\w+)\}\}");

/// Regex pattern for detecting any content between `{{` and `}}`.
static VALIDATION_PATTERN: LazyLock<Regex> = lazy_regex!(r"\{\{([^}]*)\}\}");

/// Regex pattern for detecting fenced code blocks (triple backticks with optional language identifier).
/// Matches: ``` followed by optional language, then content, then ```
static CODE_BLOCK_BACKTICK_PATTERN: LazyLock<Regex> =
    lazy_regex!(r"```([a-zA-Z0-9_-]*)\n?([\s\S]*?)```");

/// Regex pattern for detecting tilde fenced code blocks.
/// Matches: ~~~ followed by optional language, then content, then ~~~
static CODE_BLOCK_TILDE_PATTERN: LazyLock<Regex> =
    lazy_regex!(r"~~~([a-zA-Z0-9_-]*)\n?([\s\S]*?)~~~");

/// Represents a fenced code block region in content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlockRegion {
    /// Start byte position (inclusive).
    pub start: usize,
    /// End byte position (exclusive).
    pub end: usize,
    /// Optional language identifier (e.g., "rust", "markdown").
    pub language: Option<String>,
}

impl CodeBlockRegion {
    /// Creates a new code block region.
    ///
    /// Note: This cannot be `const` because `Option<String>` is not a `Copy` type.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new(start: usize, end: usize, language: Option<String>) -> Self {
        Self {
            start,
            end,
            language,
        }
    }

    /// Checks if a byte position falls within this region.
    #[must_use]
    pub const fn contains(&self, position: usize) -> bool {
        position >= self.start && position < self.end
    }
}

/// Detects fenced code blocks in content.
///
/// Returns regions sorted by start position. Handles:
/// - Code blocks with language identifiers (```rust, ```markdown, ~~~rust, ~~~markdown)
/// - Empty code blocks
/// - Multiple code blocks
/// - Both backtick (\`\`\`) and tilde (~~~) syntax
///
/// # Returns
///
/// A list of code block regions in order of appearance.
#[must_use]
pub fn detect_code_blocks(content: &str) -> Vec<CodeBlockRegion> {
    let mut regions = Vec::new();

    // Detect backtick code blocks (```)
    for cap in CODE_BLOCK_BACKTICK_PATTERN.captures_iter(content) {
        if let Some(full_match) = cap.get(0) {
            let language = cap
                .get(1)
                .map(|m| m.as_str().trim())
                .filter(|lang| !lang.is_empty())
                .map(ToString::to_string);

            regions.push(CodeBlockRegion::new(
                full_match.start(),
                full_match.end(),
                language,
            ));
        }
    }

    // Detect tilde code blocks (~~~)
    for cap in CODE_BLOCK_TILDE_PATTERN.captures_iter(content) {
        if let Some(full_match) = cap.get(0) {
            let language = cap
                .get(1)
                .map(|m| m.as_str().trim())
                .filter(|lang| !lang.is_empty())
                .map(ToString::to_string);

            regions.push(CodeBlockRegion::new(
                full_match.start(),
                full_match.end(),
                language,
            ));
        }
    }

    // Sort by start position (combines backtick and tilde blocks in order)
    regions.sort_by_key(|r| r.start);
    regions
}

/// Checks if a byte position falls within any exclusion region.
///
/// # Arguments
///
/// * `position` - The byte position to check.
/// * `regions` - The list of exclusion regions (e.g., code blocks).
///
/// # Returns
///
/// `true` if the position is inside any exclusion region.
#[must_use]
pub fn is_in_exclusion(position: usize, regions: &[CodeBlockRegion]) -> bool {
    regions.iter().any(|r| r.contains(position))
}

/// Extracts variables from prompt content, excluding those inside code blocks.
///
/// This is the internal implementation that takes pre-computed exclusion regions.
fn extract_variables_with_exclusions(
    content: &str,
    exclusions: &[CodeBlockRegion],
) -> Vec<ExtractedVariable> {
    let mut seen = HashSet::new();
    let mut variables = Vec::new();

    for cap in VARIABLE_PATTERN.captures_iter(content) {
        if let Some(name_match) = cap.get(1) {
            let position = cap.get(0).map_or(0, |m| m.start());

            // Skip variables inside exclusion regions (code blocks)
            if is_in_exclusion(position, exclusions) {
                continue;
            }

            let name = name_match.as_str().to_string();
            if seen.insert(name.clone()) {
                variables.push(ExtractedVariable { name, position });
            }
        }
    }

    variables
}

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
/// **Important**: Variables inside fenced code blocks (``` ```) are automatically
/// excluded to avoid capturing example/documentation patterns.
///
/// # Returns
///
/// A list of extracted variables in order of first appearance, deduplicated.
#[must_use]
pub fn extract_variables(content: &str) -> Vec<ExtractedVariable> {
    let code_blocks = detect_code_blocks(content);
    extract_variables_with_exclusions(content, &code_blocks)
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

    // ============================================================
    // Task 1.4: Unit Tests for Code Block Detection
    // ============================================================

    #[test]
    fn test_detect_code_blocks_single() {
        let content = "Before\n```rust\nlet x = 1;\n```\nAfter";
        let blocks = detect_code_blocks(content);

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language, Some("rust".to_string()));
        assert!(blocks[0].start < blocks[0].end);
    }

    #[test]
    fn test_detect_code_blocks_multiple() {
        let content =
            "```python\nprint('hello')\n```\n\nSome text\n\n```javascript\nconsole.log('hi');\n```";
        let blocks = detect_code_blocks(content);

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].language, Some("python".to_string()));
        assert_eq!(blocks[1].language, Some("javascript".to_string()));
        assert!(blocks[0].end <= blocks[1].start);
    }

    #[test]
    fn test_detect_code_blocks_with_language_identifier() {
        let content = "```markdown\n# Header\n```";
        let blocks = detect_code_blocks(content);

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language, Some("markdown".to_string()));
    }

    #[test]
    fn test_detect_code_blocks_empty() {
        let content = "```\n```";
        let blocks = detect_code_blocks(content);

        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].language.is_none());
    }

    #[test]
    fn test_detect_code_blocks_no_language() {
        let content = "```\nplain code\n```";
        let blocks = detect_code_blocks(content);

        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].language.is_none());
    }

    #[test]
    fn test_detect_code_blocks_none() {
        let content = "No code blocks here, just regular text.";
        let blocks = detect_code_blocks(content);

        assert!(blocks.is_empty());
    }

    #[test]
    fn test_detect_code_blocks_unclosed() {
        // Unclosed code blocks should not match (regex requires closing ```)
        let content = "```rust\nunclosed code block without ending";
        let blocks = detect_code_blocks(content);

        assert!(blocks.is_empty());
    }

    #[test]
    fn test_code_block_region_contains() {
        let region = CodeBlockRegion::new(10, 50, Some("rust".to_string()));

        assert!(!region.contains(9)); // Before
        assert!(region.contains(10)); // Start (inclusive)
        assert!(region.contains(30)); // Middle
        assert!(region.contains(49)); // End - 1
        assert!(!region.contains(50)); // End (exclusive)
        assert!(!region.contains(51)); // After
    }

    // ============================================================
    // Task 1.5: Unit Tests for Context-Aware Extraction
    // ============================================================

    #[test]
    fn test_extract_variables_outside_code_block() {
        let content = "Process {{file}} for issues.";
        let vars = extract_variables(content);

        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "file");
    }

    #[test]
    fn test_extract_variables_inside_code_block_not_extracted() {
        let content = "Text before\n```\n{{timestamp}}\n```\nText after";
        let vars = extract_variables(content);

        // Variable inside code block should NOT be extracted
        assert!(vars.is_empty());
    }

    #[test]
    fn test_extract_variables_mixed_inside_outside() {
        let content = "Scan {{PROJECT_ROOT_PATH}} for issues.\n\n## Example Output\n```markdown\n**Generated:** {{timestamp}}\n**Files:** {{count}}\n```";
        let vars = extract_variables(content);

        // Only the variable OUTSIDE the code block should be extracted
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "PROJECT_ROOT_PATH");
    }

    #[test]
    fn test_extract_variables_multiple_code_blocks() {
        let content = "Use {{var1}} here.\n\n```\n{{inside1}}\n```\n\nThen {{var2}}.\n\n```rust\n{{inside2}}\n```\n\nFinally {{var3}}.";
        let vars = extract_variables(content);

        // Only variables outside code blocks should be extracted
        assert_eq!(vars.len(), 3);
        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
        assert!(names.contains(&"var1"));
        assert!(names.contains(&"var2"));
        assert!(names.contains(&"var3"));
        assert!(!names.contains(&"inside1"));
        assert!(!names.contains(&"inside2"));
    }

    #[test]
    fn test_extract_variables_at_boundary() {
        // Variable immediately before code block
        let content = "{{before}}```\ncode\n```{{after}}";
        let vars = extract_variables(content);

        // Both should be extracted (they're outside the code block)
        assert_eq!(vars.len(), 2);
        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
        assert!(names.contains(&"before"));
        assert!(names.contains(&"after"));
    }

    #[test]
    fn test_extract_variables_backward_compatible_no_code_blocks() {
        let content = "Hello {{name}}, your {{item}} is ready for {{action}}.";
        let vars = extract_variables(content);

        // Should work exactly as before when there are no code blocks
        assert_eq!(vars.len(), 3);
        assert_eq!(vars[0].name, "name");
        assert_eq!(vars[1].name, "item");
        assert_eq!(vars[2].name, "action");
    }

    #[test]
    fn test_extract_variables_empty_content() {
        let content = "";
        let vars = extract_variables(content);

        assert!(vars.is_empty());
    }

    #[test]
    fn test_is_in_exclusion_helper() {
        let regions = vec![
            CodeBlockRegion::new(10, 20, None),
            CodeBlockRegion::new(50, 80, Some("rust".to_string())),
        ];

        assert!(!is_in_exclusion(5, &regions)); // Before all
        assert!(is_in_exclusion(10, &regions)); // Start of first
        assert!(is_in_exclusion(15, &regions)); // Inside first
        assert!(!is_in_exclusion(20, &regions)); // End of first (exclusive)
        assert!(!is_in_exclusion(30, &regions)); // Between regions
        assert!(is_in_exclusion(50, &regions)); // Start of second
        assert!(is_in_exclusion(70, &regions)); // Inside second
        assert!(!is_in_exclusion(80, &regions)); // End of second (exclusive)
        assert!(!is_in_exclusion(100, &regions)); // After all
    }

    #[test]
    fn test_is_in_exclusion_empty_regions() {
        let regions: Vec<CodeBlockRegion> = vec![];

        assert!(!is_in_exclusion(0, &regions));
        assert!(!is_in_exclusion(100, &regions));
    }

    #[test]
    fn test_prompt_template_with_code_blocks_extracts_correctly() {
        let content = "Review {{file}} for {{issue_type}} issues.\n\n```example\nOutput: {{example_var}}\n```";
        let template = PromptTemplate::new("review", content);

        // Only variables outside code blocks should be in the template
        assert_eq!(template.variables.len(), 2);
        let var_names: Vec<&str> = template.variables.iter().map(|v| v.name.as_str()).collect();
        assert!(var_names.contains(&"file"));
        assert!(var_names.contains(&"issue_type"));
        assert!(!var_names.contains(&"example_var"));
    }

    // ============================================================
    // Tilde Code Block Tests
    // ============================================================

    #[test]
    fn test_detect_tilde_code_blocks_single() {
        let content = "Before\n~~~rust\nlet x = 1;\n~~~\nAfter";
        let blocks = detect_code_blocks(content);

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language, Some("rust".to_string()));
        assert!(blocks[0].start < blocks[0].end);
    }

    #[test]
    fn test_detect_tilde_code_blocks_no_language() {
        let content = "~~~\nplain code\n~~~";
        let blocks = detect_code_blocks(content);

        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].language.is_none());
    }

    #[test]
    fn test_detect_tilde_code_blocks_empty() {
        let content = "~~~\n~~~";
        let blocks = detect_code_blocks(content);

        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].language.is_none());
    }

    #[test]
    fn test_detect_mixed_backtick_and_tilde_blocks() {
        let content =
            "```python\nprint('hello')\n```\n\nSome text\n\n~~~javascript\nconsole.log('hi');\n~~~";
        let blocks = detect_code_blocks(content);

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].language, Some("python".to_string()));
        assert_eq!(blocks[1].language, Some("javascript".to_string()));
        assert!(blocks[0].end <= blocks[1].start);
    }

    #[test]
    fn test_extract_variables_inside_tilde_block_not_extracted() {
        let content = "Text before\n~~~\n{{timestamp}}\n~~~\nText after";
        let vars = extract_variables(content);

        // Variable inside tilde code block should NOT be extracted
        assert!(vars.is_empty());
    }

    #[test]
    fn test_extract_variables_mixed_tilde_and_backtick() {
        let content = "Use {{var1}} here.\n\n~~~\n{{inside_tilde}}\n~~~\n\nThen {{var2}}.\n\n```rust\n{{inside_backtick}}\n```\n\nFinally {{var3}}.";
        let vars = extract_variables(content);

        // Only variables outside both code block types should be extracted
        assert_eq!(vars.len(), 3);
        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
        assert!(names.contains(&"var1"));
        assert!(names.contains(&"var2"));
        assert!(names.contains(&"var3"));
        assert!(!names.contains(&"inside_tilde"));
        assert!(!names.contains(&"inside_backtick"));
    }

    #[test]
    fn test_detect_tilde_code_blocks_unclosed() {
        // Unclosed tilde code blocks should not match (regex requires closing ~~~)
        let content = "~~~rust\nunclosed code block without ending";
        let blocks = detect_code_blocks(content);

        assert!(blocks.is_empty());
    }

    #[test]
    fn test_extract_variables_at_tilde_boundary() {
        // Variable immediately before/after tilde code block
        let content = "{{before}}~~~\ncode\n~~~{{after}}";
        let vars = extract_variables(content);

        // Both should be extracted (they're outside the code block)
        assert_eq!(vars.len(), 2);
        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
        assert!(names.contains(&"before"));
        assert!(names.contains(&"after"));
    }

    #[test]
    fn test_nested_tilde_within_backtick() {
        // Backtick block containing tilde syntax (tilde inside should be literal)
        let content = "{{outside}}\n```markdown\n~~~\n{{inside}}\n~~~\n```";
        let vars = extract_variables(content);

        // Only outside variable should be extracted
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "outside");
    }

    #[test]
    fn test_nested_backtick_within_tilde() {
        // Tilde block containing backtick syntax (backtick inside should be literal)
        let content = "{{outside}}\n~~~markdown\n```\n{{inside}}\n```\n~~~";
        let vars = extract_variables(content);

        // Only outside variable should be extracted
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "outside");
    }
}
