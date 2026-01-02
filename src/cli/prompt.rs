//! Prompt CLI command.
//!
//! Provides subcommands for managing user-defined prompt templates.

// CLI commands are allowed to use println! for output
#![allow(clippy::print_stdout)]
// CLI commands take owned strings from clap parsing
#![allow(clippy::needless_pass_by_value)]
// The if-let-else pattern is clearer for nested conditionals
#![allow(clippy::option_if_let_else)]

use crate::config::SubcogConfig;
use crate::models::{PromptTemplate, PromptVariable, substitute_variables};
use crate::services::{PromptFilter, PromptFormat, PromptParser, PromptService};
use crate::storage::index::DomainScope;
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::PathBuf;

/// Prompt command handler.
pub struct PromptCommand;

impl PromptCommand {
    /// Creates a new prompt command.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PromptCommand {
    fn default() -> Self {
        Self::new()
    }
}

/// Output format for prompt commands.
#[derive(Debug, Clone, Copy, Default)]
pub enum OutputFormat {
    /// Table format (default for list).
    #[default]
    Table,
    /// JSON format.
    Json,
    /// Template format (for get).
    Template,
    /// Markdown format (for export).
    Markdown,
    /// YAML format (for export).
    Yaml,
}

impl OutputFormat {
    /// Parses output format from string.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => Self::Json,
            "template" => Self::Template,
            "markdown" | "md" => Self::Markdown,
            "yaml" | "yml" => Self::Yaml,
            _ => Self::Table,
        }
    }
}

/// Parses domain scope from string.
fn parse_domain_scope(s: Option<&str>) -> DomainScope {
    match s.map(str::to_lowercase).as_deref() {
        Some("user") => DomainScope::User,
        Some("org") => DomainScope::Org,
        _ => DomainScope::Project,
    }
}

/// Converts domain scope to display string.
const fn domain_scope_to_display(scope: DomainScope) -> &'static str {
    match scope {
        DomainScope::Project => "project",
        DomainScope::User => "user",
        DomainScope::Org => "org",
    }
}

/// Creates a [`PromptService`] with full config loaded.
///
/// Uses `SubcogConfig` to respect storage settings from config file.
fn create_prompt_service() -> Result<PromptService, Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir()?;
    let config = SubcogConfig::load_default().with_repo_path(&cwd);
    Ok(PromptService::with_subcog_config(config).with_repo_path(cwd))
}

/// Executes the `prompt save` subcommand.
///
/// # Arguments
///
/// * `name` - Prompt name (kebab-case).
/// * `content` - Optional inline content.
/// * `description` - Optional description.
/// * `tags` - Optional comma-separated tags.
/// * `domain` - Optional domain scope.
/// * `from_file` - Optional file path to load from.
/// * `from_stdin` - Whether to read from stdin.
///
/// # Errors
///
/// Returns an error if saving fails.
pub fn cmd_prompt_save(
    name: String,
    content: Option<String>,
    description: Option<String>,
    tags: Option<String>,
    domain: Option<String>,
    from_file: Option<PathBuf>,
    from_stdin: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut service = create_prompt_service()?;

    // Build template from input source
    let mut template = build_template_from_input(name, content, from_file, from_stdin)?;

    // Apply overrides
    if let Some(desc) = description {
        template.description = desc;
    }
    if let Some(tag_str) = tags {
        template.tags = tag_str.split(',').map(|s| s.trim().to_string()).collect();
    }

    let scope = parse_domain_scope(domain.as_deref());

    let id = service.save(&template, scope)?;

    println!("Prompt saved:");
    println!("  Name: {}", template.name);
    println!("  ID: {id}");
    println!("  Domain: {}", domain_scope_to_display(scope));
    if !template.variables.is_empty() {
        println!(
            "  Variables: {}",
            format_variables_summary(&template.variables)
        );
    }
    if !template.tags.is_empty() {
        println!("  Tags: {}", template.tags.join(", "));
    }

    Ok(())
}

/// Builds a template from the various input sources.
fn build_template_from_input(
    name: String,
    content: Option<String>,
    from_file: Option<PathBuf>,
    from_stdin: bool,
) -> Result<PromptTemplate, Box<dyn std::error::Error>> {
    if let Some(path) = from_file {
        // Parse from file, then override name with CLI argument
        let mut template: PromptTemplate =
            PromptParser::from_file(&path).map_err(|e| e.to_string())?;
        template.name = name; // CLI --name always takes precedence
        Ok(template)
    } else if from_stdin {
        // Parse from stdin, then override name with CLI argument
        let mut template =
            PromptParser::from_stdin(PromptFormat::Markdown, &name).map_err(|e| e.to_string())?;
        template.name = name; // CLI --name always takes precedence
        Ok(template)
    } else if let Some(content_str) = content {
        // Build from inline content
        Ok(PromptTemplate::new(name, content_str))
    } else {
        Err("Either content, --from-file, or --from-stdin is required".into())
    }
}

/// Formats a summary of variables for display.
fn format_variables_summary(variables: &[PromptVariable]) -> String {
    variables
        .iter()
        .map(|v| {
            if v.required {
                format!("{{{{{}}}}}", v.name)
            } else {
                format!("{{{{{}}}}}?", v.name)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Executes the `prompt list` subcommand.
///
/// # Arguments
///
/// * `domain` - Optional domain scope filter.
/// * `tags` - Optional comma-separated tags filter.
/// * `name_pattern` - Optional name pattern (glob).
/// * `format` - Output format.
/// * `limit` - Maximum number of results.
///
/// # Errors
///
/// Returns an error if listing fails.
pub fn cmd_prompt_list(
    domain: Option<String>,
    tags: Option<String>,
    name_pattern: Option<String>,
    format: Option<String>,
    limit: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut service = create_prompt_service()?;

    // Build filter
    let mut filter = PromptFilter::default();
    if let Some(tag_str) = tags {
        filter = filter.with_tags(tag_str.split(',').map(|s| s.trim().to_string()).collect());
    }
    if let Some(pattern) = name_pattern {
        filter = filter.with_name_pattern(&pattern);
    }
    if let Some(n) = limit {
        filter = filter.with_limit(n);
    }

    let prompts = service.list(&filter)?;
    let output_format = format
        .as_deref()
        .map_or(OutputFormat::Table, OutputFormat::parse);

    match output_format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&prompts)?;
            println!("{json}");
        },
        _ => {
            print_prompts_table(&prompts, domain.as_deref());
        },
    }

    Ok(())
}

/// Prints prompts in table format.
fn print_prompts_table(prompts: &[PromptTemplate], _domain_filter: Option<&str>) {
    if prompts.is_empty() {
        println!("No prompts found.");
        return;
    }

    println!("{:<20} {:<40} {:<6} TAGS", "NAME", "DESCRIPTION", "USAGE");
    println!("{}", "-".repeat(80));

    for prompt in prompts {
        let desc = if prompt.description.len() > 38 {
            format!("{}...", &prompt.description[..35])
        } else {
            prompt.description.clone()
        };
        let tags = if prompt.tags.is_empty() {
            String::new()
        } else {
            prompt.tags.join(", ")
        };
        println!(
            "{:<20} {:<40} {:<6} {}",
            prompt.name, desc, prompt.usage_count, tags
        );
    }

    println!();
    println!("Total: {} prompts", prompts.len());
}

/// Executes the `prompt get` subcommand.
///
/// # Arguments
///
/// * `name` - Prompt name to retrieve.
/// * `domain` - Optional domain scope.
/// * `format` - Output format.
///
/// # Errors
///
/// Returns an error if the prompt is not found.
pub fn cmd_prompt_get(
    name: String,
    domain: Option<String>,
    format: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut service = create_prompt_service()?;

    let scope = domain.as_deref().map(|d| parse_domain_scope(Some(d)));
    let prompt = service.get(&name, scope)?;

    let Some(template) = prompt else {
        return Err(format!("Prompt not found: {name}").into());
    };

    let output_format = format
        .as_deref()
        .map_or(OutputFormat::Template, OutputFormat::parse);

    match output_format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&template)?;
            println!("{json}");
        },
        OutputFormat::Template | OutputFormat::Table => {
            print_template_details(&template);
        },
        OutputFormat::Markdown => {
            let md = PromptParser::serialize(&template, PromptFormat::Markdown)?;
            println!("{md}");
        },
        OutputFormat::Yaml => {
            let yaml = PromptParser::serialize(&template, PromptFormat::Yaml)?;
            println!("{yaml}");
        },
    }

    Ok(())
}

/// Prints template details in human-readable format.
fn print_template_details(template: &PromptTemplate) {
    println!("Name: {}", template.name);
    if !template.description.is_empty() {
        println!("Description: {}", template.description);
    }
    if !template.tags.is_empty() {
        println!("Tags: {}", template.tags.join(", "));
    }
    if let Some(author) = &template.author {
        println!("Author: {author}");
    }
    println!("Usage Count: {}", template.usage_count);
    println!();

    if !template.variables.is_empty() {
        println!("Variables:");
        for var in &template.variables {
            let required = if var.required {
                "(required)"
            } else {
                "(optional)"
            };
            let default = var
                .default
                .as_ref()
                .map_or(String::new(), |d| format!(" [default: {d}]"));
            let desc = var
                .description
                .as_ref()
                .map_or(String::new(), |d| format!(" - {d}"));
            println!("  {{{{{}}}}}{} {}{}", var.name, required, default, desc);
        }
        println!();
    }

    println!("Content:");
    println!("--------");
    println!("{}", template.content);
}

/// Executes the `prompt run` subcommand.
///
/// # Arguments
///
/// * `name` - Prompt name to run.
/// * `variables` - Variable values as KEY=VALUE pairs.
/// * `domain` - Optional domain scope.
/// * `interactive` - Whether to prompt for missing variables.
///
/// # Errors
///
/// Returns an error if the prompt is not found or variables are missing.
pub fn cmd_prompt_run(
    name: String,
    variables: Vec<String>,
    domain: Option<String>,
    interactive: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut service = create_prompt_service()?;

    let scope = domain.as_deref().map(|d| parse_domain_scope(Some(d)));
    let prompt = service.get(&name, scope)?;

    let Some(template) = prompt else {
        return Err(format!("Prompt not found: {name}").into());
    };

    // Parse provided variables
    let mut values: HashMap<String, String> = HashMap::new();
    for var_str in &variables {
        if let Some((key, value)) = var_str.split_once('=') {
            values.insert(key.to_string(), value.to_string());
        }
    }

    // Find missing required variables
    let missing = find_missing_variables(&template.variables, &values);

    // If interactive, prompt for missing values
    if !missing.is_empty() {
        if interactive {
            prompt_for_variables(&missing, &template.variables, &mut values)?;
        } else {
            let missing_names: Vec<_> = missing.iter().map(|s| format!("{{{{{s}}}}}")).collect();
            return Err(format!(
                "Missing required variables: {}. Use --interactive or provide with --var KEY=VALUE",
                missing_names.join(", ")
            )
            .into());
        }
    }

    // Substitute variables
    let result = substitute_variables(&template.content, &values, &template.variables)?;

    // Increment usage count
    let actual_scope = scope.unwrap_or(DomainScope::Project);
    let _ = service.increment_usage(&name, actual_scope);

    // Output the result
    println!("{result}");

    Ok(())
}

/// Finds missing required variables.
fn find_missing_variables<'a>(
    variables: &'a [PromptVariable],
    values: &HashMap<String, String>,
) -> Vec<&'a str> {
    variables
        .iter()
        .filter(|v| v.required && v.default.is_none() && !values.contains_key(&v.name))
        .map(|v| v.name.as_str())
        .collect()
}

/// Prompts interactively for variable values.
fn prompt_for_variables(
    missing: &[&str],
    variables: &[PromptVariable],
    values: &mut HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for var_name in missing {
        // Find variable definition for description
        let var_def = variables.iter().find(|v| v.name == *var_name);

        let prompt_text = match var_def.and_then(|v| v.description.as_ref()) {
            Some(desc) => format!("{var_name} ({desc}): "),
            None => format!("{var_name}: "),
        };

        write!(stdout, "{prompt_text}")?;
        stdout.flush()?;

        let mut input = String::new();
        stdin.read_line(&mut input)?;
        let trimmed = input.trim();

        // Use default if available and input is empty
        let value = if trimmed.is_empty() {
            var_def.and_then(|v| v.default.clone()).unwrap_or_default()
        } else {
            trimmed.to_string()
        };

        values.insert((*var_name).to_string(), value);
    }

    Ok(())
}

/// Executes the `prompt delete` subcommand.
///
/// # Arguments
///
/// * `name` - Prompt name to delete.
/// * `domain` - Domain scope (required).
/// * `force` - Skip confirmation.
///
/// # Errors
///
/// Returns an error if deletion fails.
pub fn cmd_prompt_delete(
    name: String,
    domain: String,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut service = create_prompt_service()?;

    let scope = parse_domain_scope(Some(&domain));

    // Confirm deletion unless --force
    if !force {
        print!("Delete prompt '{name}' from {domain}? [y/N]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let deleted = service.delete(&name, scope)?;

    if deleted {
        println!("Prompt '{name}' deleted from {domain}.");
    } else {
        println!("Prompt '{name}' not found in {domain}.");
    }

    Ok(())
}

/// Executes the `prompt export` subcommand.
///
/// # Arguments
///
/// * `name` - Prompt name to export.
/// * `output` - Optional output file path.
/// * `format` - Export format.
/// * `domain` - Optional domain scope.
///
/// # Errors
///
/// Returns an error if export fails.
pub fn cmd_prompt_export(
    name: String,
    output: Option<PathBuf>,
    format: Option<String>,
    domain: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut service = create_prompt_service()?;

    let scope = domain.as_deref().map(|d| parse_domain_scope(Some(d)));
    let prompt = service.get(&name, scope)?;

    let Some(template) = prompt else {
        return Err(format!("Prompt not found: {name}").into());
    };

    // Determine format from output path or explicit format
    let export_format = determine_export_format(format.as_deref(), output.as_ref());

    let content = match export_format {
        OutputFormat::Yaml => PromptParser::serialize(&template, PromptFormat::Yaml)?,
        OutputFormat::Json => serde_json::to_string_pretty(&template)?,
        // Markdown is the default for Table, Template, and explicit Markdown
        OutputFormat::Markdown | OutputFormat::Table | OutputFormat::Template => {
            PromptParser::serialize(&template, PromptFormat::Markdown)?
        },
    };

    // Write to file or stdout
    if let Some(path) = output {
        std::fs::write(&path, &content)?;
        println!("Exported to: {}", path.display());
    } else {
        println!("{content}");
    }

    Ok(())
}

/// Determines export format from explicit format or file extension.
fn determine_export_format(format: Option<&str>, output: Option<&PathBuf>) -> OutputFormat {
    // Explicit format takes precedence
    if let Some(fmt) = format {
        return OutputFormat::parse(fmt);
    }

    // Infer from output file extension
    if let Some(path) = output {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            return match ext.to_lowercase().as_str() {
                "yaml" | "yml" => OutputFormat::Yaml,
                "json" => OutputFormat::Json,
                // Default to Markdown for .md, .markdown, and unknown extensions
                _ => OutputFormat::Markdown,
            };
        }
    }

    OutputFormat::Markdown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_domain_scope() {
        assert!(matches!(parse_domain_scope(None), DomainScope::Project));
        assert!(matches!(
            parse_domain_scope(Some("project")),
            DomainScope::Project
        ));
        assert!(matches!(
            parse_domain_scope(Some("user")),
            DomainScope::User
        ));
        assert!(matches!(
            parse_domain_scope(Some("User")),
            DomainScope::User
        ));
        assert!(matches!(parse_domain_scope(Some("org")), DomainScope::Org));
        assert!(matches!(parse_domain_scope(Some("ORG")), DomainScope::Org));
        assert!(matches!(
            parse_domain_scope(Some("invalid")),
            DomainScope::Project
        ));
    }

    #[test]
    fn test_output_format_from_str() {
        assert!(matches!(OutputFormat::parse("json"), OutputFormat::Json));
        assert!(matches!(OutputFormat::parse("JSON"), OutputFormat::Json));
        assert!(matches!(
            OutputFormat::parse("template"),
            OutputFormat::Template
        ));
        assert!(matches!(
            OutputFormat::parse("markdown"),
            OutputFormat::Markdown
        ));
        assert!(matches!(OutputFormat::parse("md"), OutputFormat::Markdown));
        assert!(matches!(OutputFormat::parse("yaml"), OutputFormat::Yaml));
        assert!(matches!(OutputFormat::parse("yml"), OutputFormat::Yaml));
        assert!(matches!(
            OutputFormat::parse("invalid"),
            OutputFormat::Table
        ));
    }

    #[test]
    fn test_format_variables_summary() {
        let vars = vec![
            PromptVariable {
                name: "required_var".to_string(),
                description: None,
                default: None,
                required: true,
            },
            PromptVariable {
                name: "optional_var".to_string(),
                description: None,
                default: Some("default".to_string()),
                required: false,
            },
        ];

        let summary = format_variables_summary(&vars);
        assert!(summary.contains("{{required_var}}"));
        assert!(summary.contains("{{optional_var}}?"));
    }

    #[test]
    fn test_find_missing_variables() {
        let vars = vec![
            PromptVariable {
                name: "required".to_string(),
                description: None,
                default: None,
                required: true,
            },
            PromptVariable {
                name: "with_default".to_string(),
                description: None,
                default: Some("default".to_string()),
                required: true,
            },
            PromptVariable {
                name: "optional".to_string(),
                description: None,
                default: None,
                required: false,
            },
        ];

        let mut values = HashMap::new();
        let missing = find_missing_variables(&vars, &values);
        assert_eq!(missing, vec!["required"]);

        values.insert("required".to_string(), "value".to_string());
        let missing = find_missing_variables(&vars, &values);
        assert!(missing.is_empty());
    }

    #[test]
    fn test_determine_export_format() {
        // Explicit format takes precedence
        assert!(matches!(
            determine_export_format(Some("json"), None),
            OutputFormat::Json
        ));

        // Infer from file extension
        assert!(matches!(
            determine_export_format(None, Some(&PathBuf::from("test.yaml"))),
            OutputFormat::Yaml
        ));
        assert!(matches!(
            determine_export_format(None, Some(&PathBuf::from("test.json"))),
            OutputFormat::Json
        ));
        assert!(matches!(
            determine_export_format(None, Some(&PathBuf::from("test.md"))),
            OutputFormat::Markdown
        ));

        // Default to markdown
        assert!(matches!(
            determine_export_format(None, None),
            OutputFormat::Markdown
        ));
    }

    #[test]
    fn test_domain_scope_to_display() {
        assert_eq!(domain_scope_to_display(DomainScope::Project), "project");
        assert_eq!(domain_scope_to_display(DomainScope::User), "user");
        assert_eq!(domain_scope_to_display(DomainScope::Org), "org");
    }

    #[test]
    fn test_format_variables_summary_empty() {
        let vars: Vec<PromptVariable> = vec![];
        let summary = format_variables_summary(&vars);
        assert!(summary.is_empty());
    }

    #[test]
    fn test_determine_export_format_explicit_overrides_extension() {
        // Explicit format should override file extension
        assert!(matches!(
            determine_export_format(Some("yaml"), Some(&PathBuf::from("test.json"))),
            OutputFormat::Yaml
        ));
    }

    #[test]
    fn test_output_format_default() {
        let default_format = OutputFormat::default();
        assert!(matches!(default_format, OutputFormat::Table));
    }
}
