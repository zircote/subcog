//! Context template tool execution handlers.
//!
//! Contains handlers for context template management operations:
//! save, list, get, render, delete.

use std::collections::HashMap;

use crate::mcp::tool_types::{
    ContextTemplateDeleteArgs, ContextTemplateGetArgs, ContextTemplateListArgs,
    ContextTemplateRenderArgs, ContextTemplateSaveArgs, domain_scope_to_display,
    parse_domain_scope,
};
use crate::models::{ContextTemplate, OutputFormat, SearchFilter, SearchMode, TemplateVariable};
use crate::services::{
    ContextTemplateFilter, ContextTemplateService, MemoryStatistics, ServiceContainer,
};
use crate::{Error, Result};
use serde_json::Value;

use super::super::{ToolContent, ToolResult};

/// Parses output format from string.
fn parse_output_format(s: &str) -> Option<OutputFormat> {
    match s.to_lowercase().as_str() {
        "markdown" | "md" => Some(OutputFormat::Markdown),
        "json" => Some(OutputFormat::Json),
        "xml" => Some(OutputFormat::Xml),
        _ => None,
    }
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

/// Executes the `context_template_save` tool.
pub fn execute_context_template_save(arguments: Value) -> Result<ToolResult> {
    let args: ContextTemplateSaveArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    // Parse domain scope
    let domain = parse_domain_scope(args.domain.as_deref());

    // Parse output format
    let output_format = args
        .output_format
        .as_deref()
        .and_then(parse_output_format)
        .unwrap_or(OutputFormat::Markdown);

    // Build template
    let mut template = ContextTemplate::new(&args.name, &args.content);
    template.output_format = output_format;

    if let Some(desc) = args.description {
        template = template.with_description(desc);
    }

    if let Some(tags) = args.tags {
        template = template.with_tags(tags);
    }

    // Convert variable args to TemplateVariable
    if let Some(vars) = args.variables {
        let variables: Vec<TemplateVariable> = vars
            .into_iter()
            .map(|v| {
                let mut tv = TemplateVariable::new(&v.name);
                if let Some(desc) = v.description {
                    tv = tv.with_description(desc);
                }
                if let Some(default) = v.default {
                    tv = tv.with_default(default);
                }
                // Set required - with_default sets required to false
                if v.required.unwrap_or(true) && tv.default.is_none() {
                    tv.required = true;
                }
                tv
            })
            .collect();
        template = template.with_variables(variables);
    }

    // Create service and save
    let mut service = ContextTemplateService::new();
    let (name, version) = service.save(&template, domain)?;

    let var_names: Vec<String> = template.variables.iter().map(|v| v.name.clone()).collect();

    Ok(ToolResult {
        content: vec![ToolContent::Text {
            text: format!(
                "Context template saved successfully!\n\n\
                 Name: {}\n\
                 Version: {}\n\
                 Domain: {}\n\
                 Format: {}\n\
                 Description: {}\n\
                 Tags: {}\n\
                 Variables: {}",
                name,
                version,
                domain_scope_to_display(domain),
                template.output_format,
                format_field_or_none(&template.description),
                format_list_or_none(&template.tags),
                format_list_or_none(&var_names),
            ),
        }],
        is_error: false,
    })
}

/// Executes the `context_template_list` tool.
pub fn execute_context_template_list(arguments: Value) -> Result<ToolResult> {
    let args: ContextTemplateListArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    // Parse domain scope
    let domain = args.domain.as_deref().map(|s| parse_domain_scope(Some(s)));

    // Build filter
    let mut filter = ContextTemplateFilter::new();
    if let Some(d) = domain {
        filter = filter.with_domain(d);
    }
    if let Some(tags) = args.tags {
        filter = filter.with_tags(tags);
    }
    if let Some(pattern) = args.name_pattern {
        filter = filter.with_name_pattern(pattern);
    }
    if let Some(limit) = args.limit {
        filter = filter.with_limit(limit.min(100));
    } else {
        filter = filter.with_limit(20);
    }

    // Create service and list
    let mut service = ContextTemplateService::new();
    let templates = service.list(&filter)?;

    if templates.is_empty() {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: "No context templates found matching the criteria.".to_string(),
            }],
            is_error: false,
        });
    }

    // Format results
    let mut lines = vec![format!("Found {} context template(s):\n", templates.len())];

    for template in templates {
        lines.push(format!(
            "- **{}** (v{})\n  {}\n  Tags: {}\n  Format: {}",
            template.name,
            template.version,
            if template.description.is_empty() {
                "(no description)"
            } else {
                &template.description
            },
            format_list_or_none(&template.tags),
            template.output_format,
        ));
    }

    Ok(ToolResult {
        content: vec![ToolContent::Text {
            text: lines.join("\n"),
        }],
        is_error: false,
    })
}

/// Executes the `context_template_get` tool.
pub fn execute_context_template_get(arguments: Value) -> Result<ToolResult> {
    let args: ContextTemplateGetArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    // Parse domain scope
    let domain = args.domain.as_deref().map(|s| parse_domain_scope(Some(s)));

    // Create service and get
    let mut service = ContextTemplateService::new();
    let template = service.get(&args.name, args.version, domain)?;

    match template {
        Some(t) => {
            let var_info: Vec<String> = t
                .variables
                .iter()
                .map(|v| {
                    let req = if v.required { "required" } else { "optional" };
                    let default = v
                        .default
                        .as_ref()
                        .map(|d| format!(", default: \"{d}\""))
                        .unwrap_or_default();
                    format!(
                        "  - **{}** ({}{}){}",
                        v.name,
                        req,
                        default,
                        v.description
                            .as_ref()
                            .map(|d| format!(": {d}"))
                            .unwrap_or_default()
                    )
                })
                .collect();

            let variables_section = if var_info.is_empty() {
                "(none)".to_string()
            } else {
                format!("\n{}", var_info.join("\n"))
            };

            Ok(ToolResult {
                content: vec![ToolContent::Text {
                    text: format!(
                        "# Context Template: {}\n\n\
                         **Version**: {}\n\
                         **Format**: {}\n\
                         **Description**: {}\n\
                         **Tags**: {}\n\
                         **Created**: {}\n\
                         **Updated**: {}\n\n\
                         ## Variables\n{}\n\n\
                         ## Content\n\n```\n{}\n```",
                        t.name,
                        t.version,
                        t.output_format,
                        format_field_or_none(&t.description),
                        format_list_or_none(&t.tags),
                        t.created_at,
                        t.updated_at,
                        variables_section,
                        t.content,
                    ),
                }],
                is_error: false,
            })
        },
        None => Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "Context template '{}' not found{}.",
                    args.name,
                    args.version
                        .map(|v| format!(" (version {v})"))
                        .unwrap_or_default()
                ),
            }],
            is_error: true,
        }),
    }
}

/// Executes the `context_template_render` tool.
pub fn execute_context_template_render(arguments: Value) -> Result<ToolResult> {
    let args: ContextTemplateRenderArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    // Parse output format override
    let format_override = args.format.as_deref().and_then(parse_output_format);

    // Get memories via search if query provided
    let (memories, statistics) = if let Some(query) = &args.query {
        // Get service container for recall
        let services = ServiceContainer::from_current_dir_or_user()?;
        let recall_service = services.recall()?;

        // Build search filter
        let limit = args.limit.unwrap_or(10) as usize;
        let mut filter = SearchFilter::new();

        // Add namespace filter if provided
        if let Some(ns_list) = &args.namespaces {
            filter.namespaces = ns_list
                .iter()
                .map(|s| crate::mcp::tool_types::parse_namespace(s))
                .collect();
        }

        // Execute search
        let results = recall_service.search(query, SearchMode::Hybrid, &filter, limit)?;

        // Extract memories
        let mems: Vec<crate::models::Memory> =
            results.memories.into_iter().map(|r| r.memory).collect();

        // Build statistics
        let mut namespace_counts: HashMap<String, usize> = HashMap::new();
        for m in &mems {
            *namespace_counts
                .entry(m.namespace.as_str().to_string())
                .or_insert(0) += 1;
        }

        let stats = MemoryStatistics {
            total_count: mems.len(),
            namespace_counts,
            top_tags: vec![],
            recent_topics: vec![],
        };

        (mems, stats)
    } else {
        // No query - use empty memories
        (
            vec![],
            MemoryStatistics {
                total_count: 0,
                namespace_counts: HashMap::new(),
                top_tags: vec![],
                recent_topics: vec![],
            },
        )
    };

    // Get custom variables
    let custom_vars = args.variables.unwrap_or_default();

    // Create service and render
    let mut service = ContextTemplateService::new();
    let result = service.render_with_memories(
        &args.name,
        args.version,
        &memories,
        &statistics,
        &custom_vars,
        format_override,
    )?;

    Ok(ToolResult {
        content: vec![ToolContent::Text {
            text: format!(
                "# Rendered: {} (v{}, {})\n\n{}",
                result.template_name, result.template_version, result.format, result.output
            ),
        }],
        is_error: false,
    })
}

/// Executes the `context_template_delete` tool.
pub fn execute_context_template_delete(arguments: Value) -> Result<ToolResult> {
    let args: ContextTemplateDeleteArgs =
        serde_json::from_value(arguments).map_err(|e| Error::InvalidInput(e.to_string()))?;

    // Parse domain scope
    let domain = parse_domain_scope(Some(&args.domain));

    // Create service and delete
    let mut service = ContextTemplateService::new();
    let deleted = service.delete(&args.name, args.version, domain)?;

    if deleted {
        let version_info = args.version.map_or_else(
            || " (all versions)".to_string(),
            |v| format!(" (version {v})"),
        );

        Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "Context template '{}'{} deleted from {} scope.",
                    args.name,
                    version_info,
                    domain_scope_to_display(domain)
                ),
            }],
            is_error: false,
        })
    } else {
        Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "Context template '{}' not found in {} scope.",
                    args.name,
                    domain_scope_to_display(domain)
                ),
            }],
            is_error: true,
        })
    }
}
