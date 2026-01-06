//! Prompt tool execution handlers.
//!
//! Contains handlers for prompt management operations:
//! save, list, get, run, delete.

use std::collections::HashMap;
use std::path::Path;

use crate::mcp::tool_types::{
    PromptDeleteArgs, PromptGetArgs, PromptListArgs, PromptRunArgs, PromptSaveArgs,
    domain_scope_to_display, find_missing_required_variables, format_variable_info,
    parse_domain_scope,
};
use crate::mcp::{PromptContent, PromptDefinition, PromptMessage, PromptRegistry};
use crate::models::{PromptTemplate, substitute_variables};
use crate::services::{
    PromptFilter, PromptParser, PromptService, ServiceContainer, prompt_service_for_repo,
};
use crate::{Error, Result};
use serde_json::Value;

use super::super::{ToolContent, ToolResult};

/// Creates a properly configured `PromptService` with storage settings from config.
///
/// Delegates to the canonical factory function in the services module to avoid
/// layer violations (MCP layer should not directly construct services).
fn create_prompt_service(repo_path: &Path) -> PromptService {
    prompt_service_for_repo(repo_path)
}

/// Formats a field value for display, returning "(none)" if empty.
fn format_field_or_none(value: &str) -> String {
    if value.is_empty() {
        "(none)".to_string()
    } else {
        value.to_string()
    }
}

/// Formats a list of items for display, returning "(none)" if empty.
fn format_list_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "(none)".to_string()
    } else {
        items.join(", ")
    }
}

fn builtin_prompt_template(definition: &PromptDefinition) -> PromptTemplate {
    let description = definition
        .description
        .clone()
        .unwrap_or_else(|| "Built-in MCP prompt".to_string());
    let variables = definition
        .arguments
        .iter()
        .map(|arg| crate::models::PromptVariable {
            name: arg.name.clone(),
            description: arg.description.clone(),
            default: None,
            required: arg.required,
        })
        .collect();

    PromptTemplate {
        name: definition.name.clone(),
        description,
        content: "Built-in MCP prompt (generated at runtime). Use prompt_run to render."
            .to_string(),
        variables,
        tags: vec!["built-in".to_string()],
        ..Default::default()
    }
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

fn builtin_matches_filter(definition: &PromptDefinition, filter: &PromptFilter) -> bool {
    if !filter.tags.is_empty() && !filter.tags.iter().all(|t| t == "built-in") {
        return false;
    }

    if let Some(pattern) = filter.name_pattern.as_deref()
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

fn builtin_prompt_definition(name: &str) -> Option<PromptDefinition> {
    let registry = PromptRegistry::default();
    registry.get_prompt(name).cloned()
}

fn execute_builtin_prompt_run(
    name: &str,
    values: HashMap<String, String>,
    definition: PromptDefinition,
) -> Result<ToolResult> {
    let missing: Vec<&str> = definition
        .arguments
        .iter()
        .filter(|arg| arg.required && !values.contains_key(&arg.name))
        .map(|arg| arg.name.as_str())
        .collect();

    if !missing.is_empty() {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "Missing required variables: {}\n\n\
                     Use the 'variables' parameter to provide values:\n\
                     ```json\n{{\n  \"variables\": {{\n{}\n  }}\n}}\n```",
                    missing.join(", "),
                    missing
                        .iter()
                        .map(|n| format!("    \"{n}\": \"<value>\""))
                        .collect::<Vec<_>>()
                        .join(",\n")
                ),
            }],
            is_error: true,
        });
    }

    let mut args_map = serde_json::Map::new();
    for (key, value) in values {
        args_map.insert(key, Value::String(value));
    }

    let messages = PromptRegistry::default()
        .get_prompt_messages(name, &Value::Object(args_map))
        .ok_or_else(|| Error::InvalidInput("Prompt generation failed".to_string()))?;
    let rendered = format_prompt_messages(&messages);

    Ok(ToolResult {
        content: vec![ToolContent::Text { text: rendered }],
        is_error: false,
    })
}

/// Executes the prompt.save tool.
pub fn execute_prompt_save(arguments: Value) -> Result<ToolResult> {
    use crate::services::{EnrichmentStatus, PartialMetadata, SaveOptions};

    let args: PromptSaveArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    // Parse domain scope
    let domain = parse_domain_scope(args.domain.as_deref());

    // Get content either directly or from file
    let (content, mut base_template) = if let Some(content) = args.content {
        (content.clone(), PromptTemplate::new(&args.name, &content))
    } else if let Some(file_path) = args.file_path {
        let template = PromptParser::from_file(&file_path)?;
        (template.content.clone(), template)
    } else {
        return Err(Error::InvalidInput(
            "Either 'content' or 'file_path' must be provided".to_string(),
        ));
    };

    // Build partial metadata from user-provided values
    let mut existing = PartialMetadata::new();
    if let Some(desc) = args.description {
        existing = existing.with_description(desc);
    }
    if let Some(tags) = args.tags {
        existing = existing.with_tags(tags);
    }
    if let Some(vars) = args.variables {
        use crate::models::PromptVariable;
        let variables: Vec<PromptVariable> = vars
            .into_iter()
            .map(|v| PromptVariable {
                name: v.name,
                description: v.description,
                default: v.default,
                required: v.required.unwrap_or(true),
            })
            .collect();
        existing = existing.with_variables(variables);
    } else if !base_template.variables.is_empty() {
        existing = existing.with_variables(std::mem::take(&mut base_template.variables));
    }

    // Configure save options
    let options = SaveOptions::new().with_skip_enrichment(args.skip_enrichment);

    // Get repo path and create service (works in both project and user scope)
    let services = ServiceContainer::from_current_dir_or_user()?;
    let mut prompt_service = if let Some(repo_path) = services.repo_path() {
        create_prompt_service(repo_path)
    } else {
        // User-scope: create prompt service with user data directory
        let user_dir = crate::storage::get_user_data_dir()?;
        create_prompt_service(&user_dir)
    };

    // Use save_with_enrichment (no LLM provider for now - fallback mode)
    let result = prompt_service.save_with_enrichment::<crate::llm::OllamaClient>(
        &args.name,
        &content,
        domain,
        &options,
        None, // No LLM provider - uses fallback
        if existing.is_empty() {
            None
        } else {
            Some(existing)
        },
    )?;

    // Format enrichment status
    let enrichment_str = match result.enrichment_status {
        EnrichmentStatus::Full => "LLM-enhanced",
        EnrichmentStatus::Fallback => "Basic (LLM unavailable)",
        EnrichmentStatus::Skipped => "Skipped",
    };

    let var_names: Vec<String> = result
        .template
        .variables
        .iter()
        .map(|v| v.name.clone())
        .collect();
    Ok(ToolResult {
        content: vec![ToolContent::Text {
            text: format!(
                "Prompt saved successfully!\n\n\
                 Name: {}\n\
                 ID: {}\n\
                 Domain: {}\n\
                 Enrichment: {}\n\
                 Description: {}\n\
                 Tags: {}\n\
                 Variables: {}",
                result.template.name,
                result.id,
                domain_scope_to_display(domain),
                enrichment_str,
                format_field_or_none(&result.template.description),
                format_list_or_none(&result.template.tags),
                format_list_or_none(&var_names),
            ),
        }],
        is_error: false,
    })
}

/// Executes the prompt.list tool.
pub fn execute_prompt_list(arguments: Value) -> Result<ToolResult> {
    let args: PromptListArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    let limit = args.limit.unwrap_or(20).min(100);

    // Build filter
    let mut filter = PromptFilter::new();
    if let Some(domain) = args.domain {
        filter = filter.with_domain(parse_domain_scope(Some(&domain)));
    }
    if let Some(tags) = args.tags {
        filter = filter.with_tags(tags);
    }
    if let Some(pattern) = args.name_pattern {
        filter = filter.with_name_pattern(pattern);
    }

    // Get prompts (works in both project and user scope)
    let services = ServiceContainer::from_current_dir_or_user()?;
    let mut prompt_service = if let Some(repo_path) = services.repo_path() {
        create_prompt_service(repo_path)
    } else {
        let user_dir = crate::storage::get_user_data_dir()?;
        create_prompt_service(&user_dir)
    };
    let mut user_filter = filter.clone();
    user_filter.limit = None;
    let mut prompts = prompt_service.list(&user_filter)?;

    let registry = PromptRegistry::default();
    let mut builtin_prompts: Vec<PromptTemplate> = registry
        .list_prompts()
        .into_iter()
        .filter(|definition| builtin_matches_filter(definition, &filter))
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

    if prompts.is_empty() {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: "No prompts found matching the filter.".to_string(),
            }],
            is_error: false,
        });
    }

    let mut output = format!("Found {} prompt(s):\n\n", prompts.len());
    for (i, prompt) in prompts.iter().enumerate() {
        let tags_display = if prompt.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", prompt.tags.join(", "))
        };

        let vars_count = prompt.variables.len();
        let usage_info = if prompt.usage_count > 0 {
            format!(" (used {} times)", prompt.usage_count)
        } else {
            String::new()
        };

        output.push_str(&format!(
            "{}. **{}**{}{}\n   {}\n   Variables: {}\n\n",
            i + 1,
            prompt.name,
            tags_display,
            usage_info,
            if prompt.description.is_empty() {
                "(no description)"
            } else {
                &prompt.description
            },
            if vars_count == 0 {
                "none".to_string()
            } else {
                format!(
                    "{} ({})",
                    vars_count,
                    prompt
                        .variables
                        .iter()
                        .map(|v| v.name.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        ));
    }

    Ok(ToolResult {
        content: vec![ToolContent::Text { text: output }],
        is_error: false,
    })
}

/// Executes the prompt.get tool.
pub fn execute_prompt_get(arguments: Value) -> Result<ToolResult> {
    let args: PromptGetArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    let domain = args.domain.map(|d| parse_domain_scope(Some(&d)));

    // Works in both project and user scope
    let services = ServiceContainer::from_current_dir_or_user()?;
    let mut prompt_service = if let Some(repo_path) = services.repo_path() {
        create_prompt_service(repo_path)
    } else {
        let user_dir = crate::storage::get_user_data_dir()?;
        create_prompt_service(&user_dir)
    };
    let prompt = prompt_service.get(&args.name, domain)?;

    match prompt {
        Some(p) => {
            let vars_info: Vec<String> = p.variables.iter().map(format_variable_info).collect();

            Ok(ToolResult {
                content: vec![ToolContent::Text {
                    text: format!(
                        "**{}**\n\n\
                         {}\n\n\
                         **Variables:**\n{}\n\n\
                         **Content:**\n```\n{}\n```\n\n\
                         Tags: {}\n\
                         Usage count: {}",
                        p.name,
                        if p.description.is_empty() {
                            "(no description)".to_string()
                        } else {
                            p.description.clone()
                        },
                        if vars_info.is_empty() {
                            "none".to_string()
                        } else {
                            vars_info.join("\n")
                        },
                        p.content,
                        if p.tags.is_empty() {
                            "none".to_string()
                        } else {
                            p.tags.join(", ")
                        },
                        p.usage_count
                    ),
                }],
                is_error: false,
            })
        },
        None => {
            if let Some(definition) = builtin_prompt_definition(&args.name) {
                let template = builtin_prompt_template(&definition);
                let vars_info: Vec<String> = template
                    .variables
                    .iter()
                    .map(format_variable_info)
                    .collect();

                Ok(ToolResult {
                    content: vec![ToolContent::Text {
                        text: format!(
                            "**{}**\n\n\
                             {}\n\n\
                             **Variables:**\n{}\n\n\
                             **Content:**\n```\n{}\n```\n\n\
                             Tags: {}\n\
                             Usage count: {}",
                            template.name,
                            if template.description.is_empty() {
                                "(no description)".to_string()
                            } else {
                                template.description.clone()
                            },
                            if vars_info.is_empty() {
                                "none".to_string()
                            } else {
                                vars_info.join("\n")
                            },
                            template.content,
                            template.tags.join(", "),
                            template.usage_count
                        ),
                    }],
                    is_error: false,
                })
            } else {
                Ok(ToolResult {
                    content: vec![ToolContent::Text {
                        text: format!("Prompt '{}' not found.", args.name),
                    }],
                    is_error: true,
                })
            }
        },
    }
}

/// Executes the prompt.run tool.
pub fn execute_prompt_run(arguments: Value) -> Result<ToolResult> {
    let args: PromptRunArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    let domain = args.domain.map(|d| parse_domain_scope(Some(&d)));

    // Works in both project and user scope
    let services = ServiceContainer::from_current_dir_or_user()?;
    let mut prompt_service = if let Some(repo_path) = services.repo_path() {
        create_prompt_service(repo_path)
    } else {
        let user_dir = crate::storage::get_user_data_dir()?;
        create_prompt_service(&user_dir)
    };
    let prompt = prompt_service.get(&args.name, domain)?;

    match prompt {
        Some(p) => {
            // Convert variables to HashMap
            let values: HashMap<String, String> = args.variables.unwrap_or_default();

            // Check for missing required variables
            let missing: Vec<&str> = find_missing_required_variables(&p.variables, &values);

            if !missing.is_empty() {
                return Ok(ToolResult {
                    content: vec![ToolContent::Text {
                        text: format!(
                            "Missing required variables: {}\n\n\
                             Use the 'variables' parameter to provide values:\n\
                             ```json\n{{\n  \"variables\": {{\n{}\n  }}\n}}\n```",
                            missing.join(", "),
                            missing
                                .iter()
                                .map(|n| format!("    \"{n}\": \"<value>\""))
                                .collect::<Vec<_>>()
                                .join(",\n")
                        ),
                    }],
                    is_error: true,
                });
            }

            // Substitute variables
            let result = substitute_variables(&p.content, &values, &p.variables)?;

            // Increment usage count (best effort)
            if let Some(scope) = domain {
                let _ = prompt_service.increment_usage(&args.name, scope);
            }

            Ok(ToolResult {
                content: vec![ToolContent::Text {
                    text: format!(
                        "**Prompt: {}**\n\n{}\n\n---\n_Variables substituted: {}_",
                        p.name,
                        result,
                        if values.is_empty() {
                            "none (defaults used)".to_string()
                        } else {
                            values.keys().cloned().collect::<Vec<_>>().join(", ")
                        }
                    ),
                }],
                is_error: false,
            })
        },
        None => {
            if let Some(definition) = builtin_prompt_definition(&args.name) {
                let values = args.variables.unwrap_or_default();
                execute_builtin_prompt_run(&args.name, values, definition)
            } else {
                Ok(ToolResult {
                    content: vec![ToolContent::Text {
                        text: format!("Prompt '{}' not found.", args.name),
                    }],
                    is_error: true,
                })
            }
        },
    }
}

/// Executes the prompt.delete tool.
pub fn execute_prompt_delete(arguments: Value) -> Result<ToolResult> {
    let args: PromptDeleteArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    let domain = parse_domain_scope(Some(&args.domain));

    if builtin_prompt_definition(&args.name).is_some() {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!("Prompt '{}' is built-in and cannot be deleted.", args.name),
            }],
            is_error: true,
        });
    }

    // Works in both project and user scope
    let services = ServiceContainer::from_current_dir_or_user()?;
    let mut prompt_service = if let Some(repo_path) = services.repo_path() {
        create_prompt_service(repo_path)
    } else {
        let user_dir = crate::storage::get_user_data_dir()?;
        create_prompt_service(&user_dir)
    };
    let deleted = prompt_service.delete(&args.name, domain)?;

    if deleted {
        Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "Prompt '{}' deleted from {} scope.",
                    args.name,
                    domain_scope_to_display(domain)
                ),
            }],
            is_error: false,
        })
    } else {
        Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "Prompt '{}' not found in {} scope.",
                    args.name,
                    domain_scope_to_display(domain)
                ),
            }],
            is_error: true,
        })
    }
}
