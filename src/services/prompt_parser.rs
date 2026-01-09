//! Prompt file parsing service.
//!
//! Supports parsing prompt templates from multiple formats:
//! - Markdown with YAML front matter (`.md`)
//! - YAML files (`.yaml`, `.yml`)
//! - JSON files (`.json`)
//! - Plain text (`.txt` or no extension)
//!
//! # Format Examples
//!
//! ## Markdown
//! ```text
//! ---
//! name: code-review
//! description: Review code for issues
//! tags: [code, review]
//! ---
//! Please review the following code:
//! {{code}}
//! ```
//!
//! ## YAML
//! ```yaml
//! name: code-review
//! description: Review code for issues
//! content: |
//!   Please review the following code:
//!   {{code}}
//! tags:
//!   - code
//!   - review
//! ```
//!
//! ## JSON
//! ```json
//! {
//!   "name": "code-review",
//!   "description": "Review code for issues",
//!   "content": "Please review {{code}}",
//!   "tags": ["code", "review"]
//! }
//! ```

use std::io::{self, Read};
use std::path::Path;

use crate::git::YamlFrontMatterParser;
use crate::models::{PromptTemplate, PromptVariable, extract_variables};
use crate::{Error, Result};

/// Supported prompt file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PromptFormat {
    /// Markdown with optional YAML front matter.
    #[default]
    Markdown,
    /// YAML format.
    Yaml,
    /// JSON format.
    Json,
    /// Plain text (template content only).
    PlainText,
}

impl PromptFormat {
    /// Detects format from file extension.
    ///
    /// # Arguments
    ///
    /// * `path` - File path to detect format from
    ///
    /// # Returns
    ///
    /// The detected format based on extension, defaulting to `Markdown`.
    #[must_use]
    pub fn from_extension(path: &Path) -> Self {
        path.extension()
            .and_then(std::ffi::OsStr::to_str)
            .map_or(Self::Markdown, Self::from_extension_str)
    }

    /// Detects format from extension string.
    #[must_use]
    pub fn from_extension_str(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "yaml" | "yml" => Self::Yaml,
            "json" => Self::Json,
            "txt" => Self::PlainText,
            // Markdown is the default for .md, .markdown, and any unknown extension
            _ => Self::Markdown,
        }
    }

    /// Returns the file extension for this format.
    #[must_use]
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Yaml => "yaml",
            Self::Json => "json",
            Self::PlainText => "txt",
        }
    }

    /// Returns the MIME type for this format.
    #[must_use]
    pub const fn mime_type(&self) -> &'static str {
        match self {
            Self::Markdown => "text/markdown",
            Self::Yaml => "application/x-yaml",
            Self::Json => "application/json",
            Self::PlainText => "text/plain",
        }
    }
}

/// Parser for prompt template files.
pub struct PromptParser;

impl PromptParser {
    /// Parses a prompt template from a file.
    ///
    /// The format is auto-detected from the file extension.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the prompt file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use subcog::services::PromptParser;
    ///
    /// let template = PromptParser::from_file("prompts/review.md")?;
    /// println!("Loaded: {}", template.name);
    /// # Ok::<(), subcog::Error>(())
    /// ```
    pub fn from_file(path: impl AsRef<Path>) -> Result<PromptTemplate> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| Error::OperationFailed {
            operation: "read_prompt_file".to_string(),
            cause: e.to_string(),
        })?;

        let format = PromptFormat::from_extension(path);
        let mut template = Self::parse(&content, format)?;

        // If no name was specified, derive from filename
        if template.name.is_empty() {
            template.name = path
                .file_stem()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or("unnamed")
                .to_string();
        }

        Ok(template)
    }

    /// Parses a prompt template from stdin.
    ///
    /// # Arguments
    ///
    /// * `format` - The format to parse as
    /// * `name` - Name for the template (required since stdin has no filename)
    ///
    /// # Errors
    ///
    /// Returns an error if stdin cannot be read or parsed.
    pub fn from_stdin(format: PromptFormat, name: impl Into<String>) -> Result<PromptTemplate> {
        let mut content = String::new();
        io::stdin()
            .read_to_string(&mut content)
            .map_err(|e| Error::OperationFailed {
                operation: "read_stdin".to_string(),
                cause: e.to_string(),
            })?;

        let mut template = Self::parse(&content, format)?;
        if template.name.is_empty() {
            template.name = name.into();
        }
        Ok(template)
    }

    /// Parses a prompt template from string content.
    ///
    /// # Arguments
    ///
    /// * `content` - The raw content to parse
    /// * `format` - The format to parse as
    ///
    /// # Errors
    ///
    /// Returns an error if parsing fails.
    pub fn parse(content: &str, format: PromptFormat) -> Result<PromptTemplate> {
        match format {
            PromptFormat::Markdown => Self::parse_markdown(content),
            PromptFormat::Yaml => Self::parse_yaml(content),
            PromptFormat::Json => Self::parse_json(content),
            PromptFormat::PlainText => Self::parse_plain_text(content),
        }
    }

    /// Parses markdown with optional YAML front matter.
    fn parse_markdown(content: &str) -> Result<PromptTemplate> {
        let (metadata, body) = YamlFrontMatterParser::parse(content)?;

        let name = metadata
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string();

        let description = metadata
            .get("description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string();

        let tags = metadata
            .get("tags")
            .and_then(serde_json::Value::as_array)
            .map_or_else(Vec::new, |arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            });

        let author = metadata
            .get("author")
            .and_then(serde_json::Value::as_str)
            .map(String::from);

        // Parse explicit variable definitions from front matter
        let explicit_variables = metadata
            .get("variables")
            .and_then(serde_json::Value::as_array)
            .map_or_else(Vec::new, |arr| {
                arr.iter().filter_map(parse_variable_def).collect()
            });

        // Extract variables from content and merge with explicit definitions
        let extracted = extract_variables(&body);
        let variables = merge_variables(explicit_variables, extracted);

        Ok(PromptTemplate {
            name,
            description,
            content: body,
            variables,
            tags,
            author,
            usage_count: 0,
            created_at: 0,
            updated_at: 0,
        })
    }

    /// Parses YAML format.
    fn parse_yaml(content: &str) -> Result<PromptTemplate> {
        let value: serde_json::Value = serde_yaml_ng::from_str(content)
            .map_err(|e| Error::InvalidInput(format!("Invalid YAML: {e}")))?;

        Self::parse_structured(&value)
    }

    /// Parses JSON format.
    fn parse_json(content: &str) -> Result<PromptTemplate> {
        let value: serde_json::Value = serde_json::from_str(content)
            .map_err(|e| Error::InvalidInput(format!("Invalid JSON: {e}")))?;

        Self::parse_structured(&value)
    }

    /// Parses a structured value (JSON or YAML converted to JSON).
    fn parse_structured(value: &serde_json::Value) -> Result<PromptTemplate> {
        let name = value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string();

        let description = value
            .get("description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string();

        let content = value
            .get("content")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| Error::InvalidInput("Missing 'content' field".to_string()))?
            .to_string();

        let tags = value
            .get("tags")
            .and_then(serde_json::Value::as_array)
            .map_or_else(Vec::new, |arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            });

        let author = value
            .get("author")
            .and_then(serde_json::Value::as_str)
            .map(String::from);

        // Parse explicit variable definitions
        let explicit_variables = value
            .get("variables")
            .and_then(serde_json::Value::as_array)
            .map_or_else(Vec::new, |arr| {
                arr.iter().filter_map(parse_variable_def).collect()
            });

        // Extract variables from content and merge with explicit definitions
        let extracted = extract_variables(&content);
        let variables = merge_variables(explicit_variables, extracted);

        Ok(PromptTemplate {
            name,
            description,
            content,
            variables,
            tags,
            author,
            usage_count: 0,
            created_at: 0,
            updated_at: 0,
        })
    }

    /// Parses plain text (just content, no metadata).
    fn parse_plain_text(content: &str) -> Result<PromptTemplate> {
        let extracted = extract_variables(content);
        let variables = extracted
            .into_iter()
            .map(|v| PromptVariable::new(v.name))
            .collect();

        Ok(PromptTemplate {
            name: String::new(),
            description: String::new(),
            content: content.to_string(),
            variables,
            tags: Vec::new(),
            author: None,
            usage_count: 0,
            created_at: 0,
            updated_at: 0,
        })
    }

    /// Serializes a prompt template to the specified format.
    ///
    /// # Arguments
    ///
    /// * `template` - The template to serialize
    /// * `format` - The output format
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn serialize(template: &PromptTemplate, format: PromptFormat) -> Result<String> {
        match format {
            PromptFormat::Markdown => Self::serialize_markdown(template),
            PromptFormat::Yaml => Self::serialize_yaml(template),
            PromptFormat::Json => Self::serialize_json(template),
            PromptFormat::PlainText => Ok(template.content.clone()),
        }
    }

    /// Serializes to markdown with YAML front matter.
    fn serialize_markdown(template: &PromptTemplate) -> Result<String> {
        use serde_json::json;

        let mut metadata = json!({
            "name": template.name,
        });

        if !template.description.is_empty() {
            metadata["description"] = json!(template.description);
        }

        if !template.tags.is_empty() {
            metadata["tags"] = json!(template.tags);
        }

        if let Some(author) = &template.author {
            metadata["author"] = json!(author);
        }

        // Add variable definitions if any have non-default settings
        let has_custom_vars = template
            .variables
            .iter()
            .any(|v| v.description.is_some() || v.default.is_some() || !v.required);

        if has_custom_vars {
            let vars: Vec<_> = template
                .variables
                .iter()
                .map(serialize_variable_to_json)
                .collect();
            metadata["variables"] = json!(vars);
        }

        YamlFrontMatterParser::serialize(&metadata, &template.content)
    }

    /// Serializes to YAML.
    fn serialize_yaml(template: &PromptTemplate) -> Result<String> {
        serde_yaml_ng::to_string(template).map_err(|e| Error::OperationFailed {
            operation: "serialize_yaml".to_string(),
            cause: e.to_string(),
        })
    }

    /// Serializes to JSON.
    fn serialize_json(template: &PromptTemplate) -> Result<String> {
        serde_json::to_string_pretty(template).map_err(|e| Error::OperationFailed {
            operation: "serialize_json".to_string(),
            cause: e.to_string(),
        })
    }
}

/// Serializes a `PromptVariable` to a JSON value for markdown front matter.
fn serialize_variable_to_json(v: &PromptVariable) -> serde_json::Value {
    use serde_json::json;

    let mut var = json!({"name": v.name});
    if let Some(desc) = &v.description {
        var["description"] = json!(desc);
    }
    if let Some(default) = &v.default {
        var["default"] = json!(default);
    }
    if !v.required {
        var["required"] = json!(false);
    }
    var
}

/// Parses a variable definition from a JSON value.
fn parse_variable_def(value: &serde_json::Value) -> Option<PromptVariable> {
    // Support both object format and simple string format
    if let Some(name) = value.as_str() {
        return Some(PromptVariable::new(name));
    }

    let name = value.get("name")?.as_str()?;
    let description = value
        .get("description")
        .and_then(serde_json::Value::as_str)
        .map(String::from);
    let default = value
        .get("default")
        .and_then(serde_json::Value::as_str)
        .map(String::from);
    let required = value
        .get("required")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);

    Some(PromptVariable {
        name: name.to_string(),
        description,
        default,
        required,
    })
}

/// Merges explicit variable definitions with extracted variables.
///
/// Explicit definitions take precedence. Variables extracted from content
/// that are not in explicit definitions are added with default settings.
fn merge_variables(
    explicit: Vec<PromptVariable>,
    extracted: Vec<crate::models::ExtractedVariable>,
) -> Vec<PromptVariable> {
    use std::collections::HashSet;

    // Collect names first, then transfer ownership
    let explicit_names: HashSet<String> = explicit.iter().map(|v| v.name.clone()).collect();

    let mut result = explicit;

    // Add any extracted variables that weren't explicitly defined
    for ext in extracted {
        if !explicit_names.contains(&ext.name) {
            result.push(PromptVariable::new(ext.name));
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_from_extension() {
        assert_eq!(
            PromptFormat::from_extension(Path::new("test.md")),
            PromptFormat::Markdown
        );
        assert_eq!(
            PromptFormat::from_extension(Path::new("test.yaml")),
            PromptFormat::Yaml
        );
        assert_eq!(
            PromptFormat::from_extension(Path::new("test.yml")),
            PromptFormat::Yaml
        );
        assert_eq!(
            PromptFormat::from_extension(Path::new("test.json")),
            PromptFormat::Json
        );
        assert_eq!(
            PromptFormat::from_extension(Path::new("test.txt")),
            PromptFormat::PlainText
        );
        assert_eq!(
            PromptFormat::from_extension(Path::new("test")),
            PromptFormat::Markdown
        );
    }

    #[test]
    fn test_format_extension() {
        assert_eq!(PromptFormat::Markdown.extension(), "md");
        assert_eq!(PromptFormat::Yaml.extension(), "yaml");
        assert_eq!(PromptFormat::Json.extension(), "json");
        assert_eq!(PromptFormat::PlainText.extension(), "txt");
    }

    #[test]
    fn test_parse_markdown_with_front_matter() {
        let content = r"---
name: code-review
description: Review code for issues
tags:
  - code
  - review
---
Please review this {{language}} code:
{{code}}
";

        let template = PromptParser::parse(content, PromptFormat::Markdown).unwrap();

        assert_eq!(template.name, "code-review");
        assert_eq!(template.description, "Review code for issues");
        assert_eq!(template.tags, vec!["code", "review"]);
        assert!(template.content.contains("{{language}}"));
        assert!(template.content.contains("{{code}}"));
        assert_eq!(template.variables.len(), 2);
    }

    #[test]
    fn test_parse_markdown_without_front_matter() {
        let content = "Hello {{name}}, welcome to {{place}}!";

        let template = PromptParser::parse(content, PromptFormat::Markdown).unwrap();

        assert!(template.name.is_empty());
        assert_eq!(template.content, content);
        assert_eq!(template.variables.len(), 2);
        assert_eq!(template.variables[0].name, "name");
        assert_eq!(template.variables[1].name, "place");
    }

    #[test]
    fn test_parse_yaml() {
        let content = r#"
name: greeting
description: A friendly greeting
content: "Hello {{name}}!"
tags:
  - greeting
  - friendly
"#;

        let template = PromptParser::parse(content, PromptFormat::Yaml).unwrap();

        assert_eq!(template.name, "greeting");
        assert_eq!(template.description, "A friendly greeting");
        assert_eq!(template.content, "Hello {{name}}!");
        assert_eq!(template.tags, vec!["greeting", "friendly"]);
        assert_eq!(template.variables.len(), 1);
    }

    #[test]
    fn test_parse_yaml_with_variables() {
        let content = r#"
name: email
content: "Dear {{recipient}}, {{body}} Regards, {{sender}}"
variables:
  - name: recipient
    description: Email recipient
    required: true
  - name: sender
    default: "Support Team"
    required: false
"#;

        let template = PromptParser::parse(content, PromptFormat::Yaml).unwrap();

        assert_eq!(template.variables.len(), 3);

        let recipient = template.variables.iter().find(|v| v.name == "recipient");
        assert!(recipient.is_some());
        assert_eq!(
            recipient.unwrap().description,
            Some("Email recipient".to_string())
        );

        let sender = template.variables.iter().find(|v| v.name == "sender");
        assert!(sender.is_some());
        assert!(!sender.unwrap().required);
        assert_eq!(sender.unwrap().default, Some("Support Team".to_string()));

        // body should be auto-extracted
        let body = template.variables.iter().find(|v| v.name == "body");
        assert!(body.is_some());
    }

    #[test]
    fn test_parse_json() {
        let content = r#"{
            "name": "json-prompt",
            "description": "A JSON-defined prompt",
            "content": "Process {{input}} and return {{output}}",
            "tags": ["json", "test"]
        }"#;

        let template = PromptParser::parse(content, PromptFormat::Json).unwrap();

        assert_eq!(template.name, "json-prompt");
        assert_eq!(template.tags, vec!["json", "test"]);
        assert_eq!(template.variables.len(), 2);
    }

    #[test]
    fn test_parse_plain_text() {
        let content = "Simple {{variable}} template.";

        let template = PromptParser::parse(content, PromptFormat::PlainText).unwrap();

        assert!(template.name.is_empty());
        assert_eq!(template.content, content);
        assert_eq!(template.variables.len(), 1);
        assert_eq!(template.variables[0].name, "variable");
    }

    #[test]
    fn test_parse_json_missing_content() {
        let content = r#"{"name": "incomplete"}"#;

        let result = PromptParser::parse(content, PromptFormat::Json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("content"));
    }

    #[test]
    fn test_serialize_markdown() {
        let template = PromptTemplate::new("test-prompt", "Hello {{name}}!")
            .with_description("A test prompt")
            .with_tags(vec!["test".to_string()]);

        let serialized = PromptParser::serialize(&template, PromptFormat::Markdown).unwrap();

        assert!(serialized.contains("---"));
        assert!(serialized.contains("name: test-prompt"));
        assert!(serialized.contains("description: A test prompt"));
        assert!(serialized.contains("Hello {{name}}!"));
    }

    #[test]
    fn test_serialize_yaml() {
        let template =
            PromptTemplate::new("yaml-test", "Content {{var}}").with_description("YAML test");

        let serialized = PromptParser::serialize(&template, PromptFormat::Yaml).unwrap();

        assert!(serialized.contains("name: yaml-test"));
        assert!(serialized.contains("content:"));
    }

    #[test]
    fn test_serialize_json() {
        let template = PromptTemplate::new("json-test", "Content {{var}}");

        let serialized = PromptParser::serialize(&template, PromptFormat::Json).unwrap();

        assert!(serialized.contains("\"name\": \"json-test\""));
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["name"], "json-test");
    }

    #[test]
    fn test_serialize_plain_text() {
        let template = PromptTemplate::new("plain", "Just {{content}}");

        let serialized = PromptParser::serialize(&template, PromptFormat::PlainText).unwrap();

        assert_eq!(serialized, "Just {{content}}");
    }

    #[test]
    fn test_roundtrip_markdown() {
        let original = PromptTemplate::new("roundtrip", "Test {{var}}")
            .with_description("Roundtrip test")
            .with_tags(vec!["test".to_string()]);

        let serialized = PromptParser::serialize(&original, PromptFormat::Markdown).unwrap();
        let parsed = PromptParser::parse(&serialized, PromptFormat::Markdown).unwrap();

        assert_eq!(parsed.name, original.name);
        assert_eq!(parsed.description, original.description);
        assert_eq!(parsed.content, original.content);
        assert_eq!(parsed.tags, original.tags);
    }

    #[test]
    fn test_merge_variables() {
        let explicit = vec![
            PromptVariable::new("name").with_description("User name"),
            PromptVariable::optional("status", "active"),
        ];

        let extracted = vec![
            crate::models::ExtractedVariable {
                name: "name".to_string(),
                position: 0,
            },
            crate::models::ExtractedVariable {
                name: "extra".to_string(),
                position: 10,
            },
        ];

        let merged = merge_variables(explicit, extracted);

        assert_eq!(merged.len(), 3);
        // Explicit definitions preserved
        assert!(
            merged
                .iter()
                .any(|v| v.name == "name" && v.description == Some("User name".to_string()))
        );
        assert!(merged.iter().any(|v| v.name == "status" && !v.required));
        // Extra variable added with defaults
        assert!(merged.iter().any(|v| v.name == "extra" && v.required));
    }

    #[test]
    fn test_parse_variable_def_string() {
        let value = serde_json::json!("simple_var");
        let var = parse_variable_def(&value).unwrap();
        assert_eq!(var.name, "simple_var");
        assert!(var.required);
    }

    #[test]
    fn test_parse_variable_def_object() {
        let value = serde_json::json!({
            "name": "complex_var",
            "description": "A complex variable",
            "default": "default_value",
            "required": false
        });

        let var = parse_variable_def(&value).unwrap();
        assert_eq!(var.name, "complex_var");
        assert_eq!(var.description, Some("A complex variable".to_string()));
        assert_eq!(var.default, Some("default_value".to_string()));
        assert!(!var.required);
    }
}
