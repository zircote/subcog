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

fn resolve_prompt_content(
    args: &PromptSaveArgs,
    existing_template: Option<&PromptTemplate>,
) -> Result<(String, PromptTemplate)> {
    if let Some(content) = args.content.as_ref() {
        return Ok((content.clone(), PromptTemplate::new(&args.name, content)));
    }

    if let Some(file_path) = args.file_path.as_ref() {
        let template = PromptParser::from_file(file_path)?;
        return Ok((template.content.clone(), template));
    }

    if args.merge {
        let Some(template) = existing_template else {
            return Err(Error::InvalidInput(
                "Prompt not found for merge update; provide 'content' or 'file_path' to create it."
                    .to_string(),
            ));
        };
        return Ok((
            template.content.clone(),
            PromptTemplate::new(&args.name, &template.content),
        ));
    }

    Err(Error::InvalidInput(
        "Either 'content' or 'file_path' must be provided".to_string(),
    ))
}

fn prompt_variables_from_args(
    vars: &[crate::mcp::tool_types::PromptVariableArg],
) -> Vec<crate::models::PromptVariable> {
    vars.iter()
        .map(|v| crate::models::PromptVariable {
            name: v.name.clone(),
            description: v.description.clone(),
            default: v.default.clone(),
            required: v.required.unwrap_or(true),
        })
        .collect()
}

fn build_partial_metadata(
    args: &PromptSaveArgs,
    base_template: &mut PromptTemplate,
    existing_template: Option<&PromptTemplate>,
) -> crate::services::PartialMetadata {
    let mut existing = crate::services::PartialMetadata::new();
    if args.merge
        && let Some(template) = existing_template
    {
        if !template.description.is_empty() {
            existing = existing.with_description(template.description.clone());
        }
        if !template.tags.is_empty() {
            existing = existing.with_tags(template.tags.clone());
        }
        if !template.variables.is_empty() {
            existing = existing.with_variables(template.variables.clone());
        }
    }

    if let Some(desc) = args.description.as_ref() {
        existing = existing.with_description(desc.clone());
    }
    if let Some(tags) = args.tags.as_ref() {
        existing = existing.with_tags(tags.clone());
    }
    if let Some(vars) = args.variables.as_ref() {
        existing = existing.with_variables(prompt_variables_from_args(vars));
    } else if !base_template.variables.is_empty() && (!args.merge || args.file_path.is_some()) {
        existing = existing.with_variables(std::mem::take(&mut base_template.variables));
    }

    existing
}

/// Executes the prompt.save tool.
pub fn execute_prompt_save(arguments: Value) -> Result<ToolResult> {
    use crate::services::{EnrichmentStatus, SaveOptions};

    let args: PromptSaveArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    // Parse domain scope
    let domain = parse_domain_scope(args.domain.as_deref());

    // Get repo path and create service (works in both project and user scope)
    let services = ServiceContainer::from_current_dir_or_user()?;
    let mut prompt_service = if let Some(repo_path) = services.repo_path() {
        create_prompt_service(repo_path)
    } else {
        // User-scope: create prompt service with user data directory
        let user_dir = crate::storage::get_user_data_dir()?;
        create_prompt_service(&user_dir)
    };

    let existing_template = if args.merge {
        prompt_service.get(&args.name, Some(domain))?
    } else {
        None
    };

    let (content, mut base_template) = resolve_prompt_content(&args, existing_template.as_ref())?;

    // Build partial metadata from existing prompt (optional) + user-provided values
    let existing = build_partial_metadata(&args, &mut base_template, existing_template.as_ref());

    // Configure save options
    let options = SaveOptions::new().with_skip_enrichment(args.skip_enrichment);

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
    let mut user_filter = filter;
    user_filter.limit = Some(limit);
    let prompts = prompt_service.list(&user_filter)?;

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
        None => Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!("Prompt '{}' not found.", args.name),
            }],
            is_error: true,
        }),
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
        None => Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!("Prompt '{}' not found.", args.name),
            }],
            is_error: true,
        }),
    }
}

/// Executes the prompt.delete tool.
pub fn execute_prompt_delete(arguments: Value) -> Result<ToolResult> {
    let args: PromptDeleteArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    let domain = parse_domain_scope(Some(&args.domain));

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
