//! Prompt CLI command.
//!
//! Provides subcommands for managing user-defined prompt templates.

// CLI commands are allowed to use println! for output
#![allow(clippy::print_stdout)]
// CLI commands take owned strings from clap parsing
#![allow(clippy::needless_pass_by_value)]
// The if-let-else pattern is clearer for nested conditionals
#![allow(clippy::option_if_let_else)]

use crate::mcp::{PromptContent, PromptDefinition, PromptMessage, PromptRegistry};
use crate::models::{PromptTemplate, PromptVariable, substitute_variables};
use crate::services::{
    EnrichmentStatus, PartialMetadata, PromptFilter, PromptFormat, PromptParser, PromptService,
    SaveOptions, prompt_service_for_cwd,
};
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

/// Arguments for the `prompt save` command.
///
/// Encapsulates all parameters to avoid function with too many arguments.
#[derive(Debug, Clone, Default)]
pub struct SavePromptArgs {
    /// Prompt name (kebab-case).
    pub name: String,
    /// Optional inline content.
    pub content: Option<String>,
    /// Optional description.
    pub description: Option<String>,
    /// Optional comma-separated tags.
    pub tags: Option<String>,
    /// Optional domain scope.
    pub domain: Option<String>,
    /// Optional file path to load from.
    pub from_file: Option<PathBuf>,
    /// Whether to read from stdin.
    pub from_stdin: bool,
    /// Skip LLM-powered enrichment.
    pub no_enrich: bool,
    /// Show enriched template without saving.
    pub dry_run: bool,
}

impl SavePromptArgs {
    /// Creates new save arguments with a name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Sets the content.
    #[must_use]
    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Sets the description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the tags.
    #[must_use]
    pub fn with_tags(mut self, tags: impl Into<String>) -> Self {
        self.tags = Some(tags.into());
        self
    }

    /// Sets the domain scope.
    #[must_use]
    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Sets the file path to load from.
    #[must_use]
    pub fn with_file(mut self, path: PathBuf) -> Self {
        self.from_file = Some(path);
        self
    }

    /// Sets whether to read from stdin.
    #[must_use]
    pub const fn with_stdin(mut self, from_stdin: bool) -> Self {
        self.from_stdin = from_stdin;
        self
    }

    /// Sets whether to skip enrichment.
    #[must_use]
    pub const fn with_no_enrich(mut self, no_enrich: bool) -> Self {
        self.no_enrich = no_enrich;
        self
    }

    /// Sets whether this is a dry run.
    #[must_use]
    pub const fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
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
/// Delegates to the canonical factory function in the services module to avoid
/// layer violations (CLI layer should not directly construct services).
fn create_prompt_service() -> Result<PromptService, Box<dyn std::error::Error>> {
    prompt_service_for_cwd().map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

/// Executes the `prompt save` subcommand.
///
/// # Arguments
///
/// * `args` - Save command arguments.
///
/// # Errors
///
/// Returns an error if saving fails.
pub fn cmd_prompt_save_with_args(args: SavePromptArgs) -> Result<(), Box<dyn std::error::Error>> {
    let mut service = create_prompt_service()?;
    let scope = parse_domain_scope(args.domain.as_deref());

    // Build template from input source to get content
    let base_template = build_template_from_input(
        args.name.clone(),
        args.content,
        args.from_file,
        args.from_stdin,
    )?;

    // Build partial metadata from user-provided values
    let existing = build_partial_metadata(args.description, args.tags, &base_template);

    // Configure save options
    let options = SaveOptions::new()
        .with_skip_enrichment(args.no_enrich)
        .with_dry_run(args.dry_run);

    // Use save_with_enrichment (no LLM provider for now - fallback mode)
    // Future: Add LLM provider integration when API keys are available
    let result = service.save_with_enrichment::<crate::llm::OllamaClient>(
        &args.name,
        &base_template.content,
        scope,
        &options,
        None, // No LLM provider - uses fallback
        if existing.is_empty() {
            None
        } else {
            Some(existing)
        },
    )?;

    // Display output
    if args.dry_run {
        println!("Dry run - template would be saved as:");
    } else {
        println!("Prompt saved:");
    }
    println!("  Name: {}", result.template.name);
    if !args.dry_run {
        println!("  ID: {}", result.id);
    }
    println!("  Domain: {}", domain_scope_to_display(scope));

    // Show enrichment status
    match result.enrichment_status {
        EnrichmentStatus::Full => println!("  Enrichment: LLM-enhanced"),
        EnrichmentStatus::Fallback => println!("  Enrichment: fallback (LLM unavailable)"),
        EnrichmentStatus::Skipped => println!("  Enrichment: skipped"),
    }

    if !result.template.description.is_empty() {
        println!("  Description: {}", result.template.description);
    }
    if !result.template.tags.is_empty() {
        println!("  Tags: {}", result.template.tags.join(", "));
    }
    if !result.template.variables.is_empty() {
        println!("  Variables:");
        for var in &result.template.variables {
            let required = if var.required { "*" } else { "" };
            let default = var
                .default
                .as_ref()
                .map_or(String::new(), |d| format!(" = \"{d}\""));
            println!("    - {}{required}{default}", var.name);
        }
    }

    Ok(())
}

/// Executes the `prompt save` subcommand (legacy interface).
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
/// * `no_enrich` - Skip LLM-powered enrichment.
/// * `dry_run` - Show enriched template without saving.
///
/// # Errors
///
/// Returns an error if saving fails.
#[allow(clippy::too_many_arguments)]
pub fn cmd_prompt_save(
    name: String,
    content: Option<String>,
    description: Option<String>,
    tags: Option<String>,
    domain: Option<String>,
    from_file: Option<PathBuf>,
    from_stdin: bool,
    no_enrich: bool,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let args = SavePromptArgs {
        name,
        content,
        description,
        tags,
        domain,
        from_file,
        from_stdin,
        no_enrich,
        dry_run,
    };
    cmd_prompt_save_with_args(args)
}

/// Builds partial metadata from user-provided values.
fn build_partial_metadata(
    description: Option<String>,
    tags: Option<String>,
    base_template: &PromptTemplate,
) -> PartialMetadata {
    let mut meta = PartialMetadata::new();

    // User-provided description
    if let Some(desc) = description {
        meta = meta.with_description(desc);
    }

    // User-provided tags
    if let Some(tag_str) = tags {
        let tag_list: Vec<String> = tag_str.split(',').map(|s| s.trim().to_string()).collect();
        meta = meta.with_tags(tag_list);
    }

    // Variables from base template (if loaded from file with existing metadata)
    if !base_template.variables.is_empty() {
        meta = meta.with_variables(base_template.variables.clone());
    }

    meta
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

fn builtin_prompt_template(definition: &PromptDefinition) -> PromptTemplate {
    let description = definition
        .description
        .clone()
        .unwrap_or_else(|| "Built-in MCP prompt".to_string());
    let variables = definition
        .arguments
        .iter()
        .map(|arg| PromptVariable {
            name: arg.name.clone(),
            description: arg.description.clone(),
            default: None,
            required: arg.required,
        })
        .collect();

    PromptTemplate {
        name: definition.name.clone(),
        description,
        content: "Built-in MCP prompt (generated at runtime). Use prompt run to render."
            .to_string(),
        variables,
        tags: vec!["built-in".to_string()],
        ..Default::default()
    }
}

fn builtin_prompt_definition(name: &str) -> Option<PromptDefinition> {
    PromptRegistry::default().get_prompt(name).cloned()
}

fn matches_glob(pattern: &str, text: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == text;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.is_empty() {
        return true;
    }

    if !parts[0].is_empty() && !text.starts_with(parts[0]) {
        return false;
    }

    let last = parts.last().unwrap_or(&"");
    if !last.is_empty() && !text.ends_with(last) {
        return false;
    }

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

fn builtin_matches_filter(
    definition: &PromptDefinition,
    tags: &[String],
    name_pattern: &Option<String>,
) -> bool {
    if !tags.is_empty() && !tags.iter().all(|t| t == "built-in") {
        return false;
    }

    if let Some(pattern) = name_pattern.as_deref()
        && !matches_glob(pattern, &definition.name)
    {
        return false;
    }

    true
}

fn format_prompt_messages(messages: &[PromptMessage]) -> String {
    let mut output = String::new();
    for message in messages {
        let rendered = match &message.content {
            PromptContent::Text { text } => text.clone(),
            PromptContent::Resource { uri } => format!("[resource: {uri}]"),
            PromptContent::Image { data, mime_type } => {
                format!("[image: {mime_type}, {} bytes]", data.len())
            },
        };
        output.push_str(&format!("{}:\n{}\n\n", message.role, rendered));
    }
    output.trim_end().to_string()
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
    let limit = limit.unwrap_or(20);

    // Build filter
    let mut filter = PromptFilter::default();
    let tag_list = tags.map(|tag_str| {
        tag_str
            .split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    });
    if let Some(ref tag_list) = tag_list {
        filter = filter.with_tags(tag_list.clone());
    }
    if let Some(ref pattern) = name_pattern {
        filter = filter.with_name_pattern(pattern);
    }

    let mut user_filter = filter.clone();
    user_filter.limit = None;
    let mut prompts = service.list(&user_filter)?;

    let registry = PromptRegistry::default();
    let mut builtin_prompts: Vec<PromptTemplate> = registry
        .list_prompts()
        .into_iter()
        .filter(|definition| {
            builtin_matches_filter(
                definition,
                tag_list.as_deref().unwrap_or(&[]),
                &name_pattern,
            )
        })
        .filter(|definition| prompts.iter().all(|p| p.name != definition.name))
        .map(builtin_prompt_template)
        .collect();

    prompts.append(&mut builtin_prompts);
    prompts.sort_by(|a, b| {
        b.usage_count
            .cmp(&a.usage_count)
            .then_with(|| a.name.cmp(&b.name))
    });
    prompts.truncate(limit);
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

    let template = match prompt {
        Some(template) => template,
        None => {
            let Some(definition) = builtin_prompt_definition(&name) else {
                return Err(format!("Prompt not found: {name}").into());
            };
            builtin_prompt_template(&definition)
        },
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
    let builtin_definition = prompt
        .is_none()
        .then(|| builtin_prompt_definition(&name))
        .flatten();
    let template = match (prompt, builtin_definition.clone()) {
        (Some(template), _) => template,
        (None, Some(definition)) => builtin_prompt_template(&definition),
        (None, None) => return Err(format!("Prompt not found: {name}").into()),
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

    if let Some(definition) = builtin_definition {
        let mut args_map = serde_json::Map::new();
        for (key, value) in &values {
            args_map.insert(key.clone(), serde_json::Value::String(value.clone()));
        }
        let messages = PromptRegistry::default()
            .get_prompt_messages(&name, &serde_json::Value::Object(args_map))
            .ok_or_else(|| "Prompt generation failed".to_string())?;
        let rendered = format_prompt_messages(&messages);
        println!("{rendered}");
    } else {
        // Substitute variables
        let result = substitute_variables(&template.content, &values, &template.variables)?;

        // Increment usage count
        let actual_scope = scope.unwrap_or(DomainScope::Project);
        let _ = service.increment_usage(&name, actual_scope);

        // Output the result
        println!("{result}");
    }

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
    if builtin_prompt_definition(&name).is_some() {
        return Err(format!("Prompt '{name}' is built-in and cannot be deleted.").into());
    }

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

    let template = match prompt {
        Some(template) => template,
        None => {
            if builtin_prompt_definition(&name).is_some() {
                return Err(format!(
                    "Prompt '{name}' is built-in and cannot be exported. Use `prompt run` to render it."
                )
                .into());
            }
            return Err(format!("Prompt not found: {name}").into());
        },
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
    if let Some(ext) = output.and_then(|path| path.extension().and_then(|e| e.to_str())) {
        return match ext.to_lowercase().as_str() {
            "yaml" | "yml" => OutputFormat::Yaml,
            "json" => OutputFormat::Json,
            // Default to Markdown for .md, .markdown, and unknown extensions
            _ => OutputFormat::Markdown,
        };
    }

    OutputFormat::Markdown
}

/// Executes the `prompt import` subcommand.
///
/// # Arguments
///
/// * `source` - Source file path or URL.
/// * `domain` - Target domain scope.
/// * `name` - Optional name override.
/// * `no_validate` - Skip validation.
///
/// # Errors
///
/// Returns an error if import fails.
pub fn cmd_prompt_import(
    source: String,
    domain: String,
    name: Option<String>,
    no_validate: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut service = create_prompt_service()?;

    // Load template from source
    let mut template = load_template_from_source(&source)?;

    // Override name if provided
    if let Some(override_name) = name {
        template.name = override_name;
    }

    // Validate unless skipped
    if !no_validate {
        validate_template(&template)?;
    }

    let scope = parse_domain_scope(Some(&domain));
    let id = service.save(&template, scope)?;

    println!("Prompt imported:");
    println!("  Name: {}", template.name);
    println!("  ID: {id}");
    println!("  Domain: {}", domain_scope_to_display(scope));
    println!("  Source: {source}");
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

/// Infers the prompt format from a file path or URL extension.
fn infer_format_from_path(source: &str) -> PromptFormat {
    let path = std::path::Path::new(source);
    path.extension()
        .and_then(|ext| ext.to_str())
        .map_or(PromptFormat::Markdown, |ext| {
            match ext.to_lowercase().as_str() {
                "json" => PromptFormat::Json,
                "yaml" | "yml" => PromptFormat::Yaml,
                _ => PromptFormat::Markdown,
            }
        })
}

/// Loads a template from a file path or URL.
fn load_template_from_source(source: &str) -> Result<PromptTemplate, Box<dyn std::error::Error>> {
    if source.starts_with("http://") || source.starts_with("https://") {
        // URL source - fetch and parse
        let response = reqwest::blocking::get(source)?;
        if !response.status().is_success() {
            return Err(format!("Failed to fetch URL: HTTP {}", response.status()).into());
        }

        let content = response.text()?;

        // Determine format from URL extension
        let format = infer_format_from_path(source);

        PromptParser::parse(&content, format).map_err(|e| e.to_string().into())
    } else {
        // File source
        let path = PathBuf::from(source);
        PromptParser::from_file(&path).map_err(|e| e.to_string().into())
    }
}

/// Validates a template for required fields and variable syntax.
fn validate_template(template: &PromptTemplate) -> Result<(), Box<dyn std::error::Error>> {
    // Check name is not empty
    if template.name.trim().is_empty() {
        return Err("Template name cannot be empty".into());
    }

    // Check content is not empty
    if template.content.trim().is_empty() {
        return Err("Template content cannot be empty".into());
    }

    // Validate variable names
    for var in &template.variables {
        if var.name.trim().is_empty() {
            return Err("Variable name cannot be empty".into());
        }
        if var.name.starts_with("subcog_")
            || var.name.starts_with("system_")
            || var.name.starts_with("__")
        {
            return Err(format!(
                "Variable name '{}' uses reserved prefix (subcog_, system_, __)",
                var.name
            )
            .into());
        }
    }

    Ok(())
}

/// Executes the `prompt share` subcommand.
///
/// # Arguments
///
/// * `name` - Prompt name to share.
/// * `output` - Optional output file path.
/// * `format` - Export format.
/// * `domain` - Optional domain scope to search.
/// * `include_stats` - Include usage statistics.
///
/// # Errors
///
/// Returns an error if sharing fails.
pub fn cmd_prompt_share(
    name: String,
    output: Option<PathBuf>,
    format: String,
    domain: Option<String>,
    include_stats: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut service = create_prompt_service()?;

    let scope = domain.as_deref().map(|d| parse_domain_scope(Some(d)));
    let prompt = service.get(&name, scope)?;

    let template = match prompt {
        Some(template) => template,
        None => {
            if builtin_prompt_definition(&name).is_some() {
                return Err(format!(
                    "Prompt '{name}' is built-in and cannot be shared. Use `prompt run` to render it."
                )
                .into());
            }
            return Err(format!("Prompt not found: {name}").into());
        },
    };

    // Build shareable content with metadata
    let share_content = build_share_content(&template, include_stats, &format)?;

    // Write to file or stdout
    if let Some(path) = output {
        std::fs::write(&path, &share_content)?;
        println!("Shared to: {}", path.display());
        println!("  Name: {}", template.name);
        println!("  Format: {format}");
        if include_stats {
            println!("  Usage count: {}", template.usage_count);
        }
    } else {
        println!("{share_content}");
    }

    Ok(())
}

/// Formats a Unix timestamp as a full datetime string.
fn format_timestamp(ts: u64) -> String {
    chrono::DateTime::from_timestamp(i64::try_from(ts).unwrap_or(0), 0).map_or_else(
        || "unknown".to_string(),
        |dt| dt.format("%Y-%m-%d %H:%M:%S").to_string(),
    )
}

/// Formats a Unix timestamp as a short date string.
fn format_timestamp_short(ts: u64) -> String {
    chrono::DateTime::from_timestamp(i64::try_from(ts).unwrap_or(0), 0).map_or_else(
        || "unknown".to_string(),
        |dt| dt.format("%Y-%m-%d").to_string(),
    )
}

/// Builds shareable content with full metadata.
fn build_share_content(
    template: &PromptTemplate,
    include_stats: bool,
    format: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let output_format = OutputFormat::parse(format);

    match output_format {
        OutputFormat::Yaml => {
            // Include stats as metadata comments
            let yaml = PromptParser::serialize(template, PromptFormat::Yaml)?;
            if include_stats {
                let created = format_timestamp(template.created_at);
                let updated = format_timestamp(template.updated_at);
                let stats_header = format!(
                    "# Subcog Prompt Share\n# Usage count: {}\n# Created: {}\n# Last used: {}\n\n",
                    template.usage_count, created, updated,
                );
                Ok(format!("{stats_header}{yaml}"))
            } else {
                Ok(yaml)
            }
        },
        OutputFormat::Json => {
            if include_stats {
                // Include stats in JSON output
                let mut json_value: serde_json::Value = serde_json::to_value(template)?;
                if let Some(obj) = json_value.as_object_mut() {
                    obj.insert(
                        "_share_metadata".to_string(),
                        serde_json::json!({
                            "exported_at": chrono::Utc::now().to_rfc3339(),
                            "usage_count": template.usage_count,
                        }),
                    );
                }
                Ok(serde_json::to_string_pretty(&json_value)?)
            } else {
                Ok(serde_json::to_string_pretty(template)?)
            }
        },
        OutputFormat::Markdown | OutputFormat::Table | OutputFormat::Template => {
            let md = PromptParser::serialize(template, PromptFormat::Markdown)?;
            if include_stats {
                let created = format_timestamp_short(template.created_at);
                let updated = format_timestamp_short(template.updated_at);
                let stats_footer = format!(
                    "\n---\n\n*Usage count: {} | Created: {} | Last used: {}*",
                    template.usage_count, created, updated,
                );
                Ok(format!("{md}{stats_footer}"))
            } else {
                Ok(md)
            }
        },
    }
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

    #[test]
    fn test_infer_format_from_path_json() {
        assert!(matches!(
            infer_format_from_path("prompt.json"),
            PromptFormat::Json
        ));
        assert!(matches!(
            infer_format_from_path("/path/to/prompt.JSON"),
            PromptFormat::Json
        ));
    }

    #[test]
    fn test_infer_format_from_path_yaml() {
        assert!(matches!(
            infer_format_from_path("prompt.yaml"),
            PromptFormat::Yaml
        ));
        assert!(matches!(
            infer_format_from_path("prompt.yml"),
            PromptFormat::Yaml
        ));
        assert!(matches!(
            infer_format_from_path("https://example.com/prompt.YAML"),
            PromptFormat::Yaml
        ));
    }

    #[test]
    fn test_infer_format_from_path_markdown() {
        assert!(matches!(
            infer_format_from_path("prompt.md"),
            PromptFormat::Markdown
        ));
        assert!(matches!(
            infer_format_from_path("prompt.txt"),
            PromptFormat::Markdown
        ));
        assert!(matches!(
            infer_format_from_path("prompt"),
            PromptFormat::Markdown
        ));
    }

    #[test]
    fn test_validate_template_valid() {
        let template = PromptTemplate {
            name: "test-prompt".to_string(),
            content: "Test content".to_string(),
            ..Default::default()
        };
        assert!(validate_template(&template).is_ok());
    }

    #[test]
    fn test_validate_template_empty_name() {
        let template = PromptTemplate {
            name: String::new(),
            content: "Test content".to_string(),
            ..Default::default()
        };
        assert!(validate_template(&template).is_err());
    }

    #[test]
    fn test_validate_template_empty_content() {
        let template = PromptTemplate {
            name: "test".to_string(),
            content: "   ".to_string(),
            ..Default::default()
        };
        assert!(validate_template(&template).is_err());
    }

    #[test]
    fn test_validate_template_reserved_variable_prefix() {
        let template = PromptTemplate {
            name: "test".to_string(),
            content: "Test {{subcog_internal}}".to_string(),
            variables: vec![PromptVariable {
                name: "subcog_internal".to_string(),
                description: None,
                default: None,
                required: false,
            }],
            ..Default::default()
        };
        assert!(validate_template(&template).is_err());
    }

    #[test]
    fn test_format_timestamp() {
        // Test with epoch timestamp
        let result = format_timestamp(0);
        assert!(result.contains("1970-01-01"));

        // Test with a known timestamp (2024-01-01 00:00:00 UTC)
        let result = format_timestamp(1_704_067_200);
        assert!(result.contains("2024-01-01"));
    }

    #[test]
    fn test_format_timestamp_short() {
        let result = format_timestamp_short(0);
        assert_eq!(result, "1970-01-01");

        let result = format_timestamp_short(1_704_067_200);
        assert_eq!(result, "2024-01-01");
    }
}
