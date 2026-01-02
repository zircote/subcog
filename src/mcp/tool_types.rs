//! Argument types and helper functions for MCP tools.
//!
//! Extracted from `tools.rs` to reduce file size.

use crate::models::{DetailLevel, MemoryStatus, Namespace, SearchFilter, SearchMode};
use crate::storage::index::DomainScope;
use serde::Deserialize;
use std::collections::HashMap;

/// Arguments for the capture tool.
#[derive(Debug, Deserialize)]
pub struct CaptureArgs {
    pub content: String,
    pub namespace: String,
    pub tags: Option<Vec<String>>,
    pub source: Option<String>,
}

/// Arguments for the recall tool.
#[derive(Debug, Deserialize)]
pub struct RecallArgs {
    pub query: String,
    pub filter: Option<String>,
    pub namespace: Option<String>,
    pub mode: Option<String>,
    pub detail: Option<String>,
    pub limit: Option<usize>,
}

/// Arguments for the consolidate tool.
#[derive(Debug, Deserialize)]
pub struct ConsolidateArgs {
    pub namespace: String,
    pub query: Option<String>,
    pub strategy: Option<String>,
    pub dry_run: Option<bool>,
}

/// Arguments for the enrich tool.
#[derive(Debug, Deserialize)]
pub struct EnrichArgs {
    pub memory_id: String,
    pub enrich_tags: Option<bool>,
    pub enrich_structure: Option<bool>,
    pub add_context: Option<bool>,
}

/// Arguments for the sync tool.
#[derive(Debug, Deserialize)]
pub struct SyncArgs {
    pub direction: Option<String>,
}

/// Arguments for the reindex tool.
#[derive(Debug, Deserialize)]
pub struct ReindexArgs {
    pub repo_path: Option<String>,
}

// ============================================================================
// Prompt Tool Arguments
// ============================================================================

/// Arguments for the prompt.save tool.
#[derive(Debug, Deserialize)]
pub struct PromptSaveArgs {
    pub name: String,
    pub content: Option<String>,
    pub file_path: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub domain: Option<String>,
    pub variables: Option<Vec<PromptVariableArg>>,
}

/// Variable definition argument for prompt.save.
#[derive(Debug, Deserialize)]
pub struct PromptVariableArg {
    pub name: String,
    pub description: Option<String>,
    pub default: Option<String>,
    pub required: Option<bool>,
}

/// Arguments for the prompt.list tool.
#[derive(Debug, Deserialize)]
pub struct PromptListArgs {
    pub domain: Option<String>,
    pub tags: Option<Vec<String>>,
    pub name_pattern: Option<String>,
    pub limit: Option<usize>,
}

/// Arguments for the prompt.get tool.
#[derive(Debug, Deserialize)]
pub struct PromptGetArgs {
    pub name: String,
    pub domain: Option<String>,
}

/// Arguments for the prompt.run tool.
#[derive(Debug, Deserialize)]
pub struct PromptRunArgs {
    pub name: String,
    pub variables: Option<HashMap<String, String>>,
    pub domain: Option<String>,
}

/// Arguments for the prompt.delete tool.
#[derive(Debug, Deserialize)]
pub struct PromptDeleteArgs {
    pub name: String,
    pub domain: String,
}

/// Parses a namespace string to Namespace enum.
pub fn parse_namespace(s: &str) -> Namespace {
    match s.to_lowercase().as_str() {
        "decisions" => Namespace::Decisions,
        "patterns" => Namespace::Patterns,
        "learnings" => Namespace::Learnings,
        "context" => Namespace::Context,
        "tech-debt" | "techdebt" => Namespace::TechDebt,
        "apis" => Namespace::Apis,
        "config" => Namespace::Config,
        "security" => Namespace::Security,
        "performance" => Namespace::Performance,
        "testing" => Namespace::Testing,
        _ => Namespace::Decisions,
    }
}

/// Parses a search mode string to `SearchMode` enum.
pub fn parse_search_mode(s: &str) -> SearchMode {
    match s.to_lowercase().as_str() {
        "vector" => SearchMode::Vector,
        "text" => SearchMode::Text,
        _ => SearchMode::Hybrid,
    }
}

/// Parses a domain scope string to `DomainScope` enum.
pub fn parse_domain_scope(s: Option<&str>) -> DomainScope {
    match s.map(str::to_lowercase).as_deref() {
        Some("user") => DomainScope::User,
        Some("org") => DomainScope::Org,
        _ => DomainScope::Project,
    }
}

/// Converts a `DomainScope` to a display string.
pub const fn domain_scope_to_display(scope: DomainScope) -> &'static str {
    match scope {
        DomainScope::Project => "project",
        DomainScope::User => "user",
        DomainScope::Org => "org",
    }
}

/// Formats a `PromptVariable` for display.
pub fn format_variable_info(v: &crate::models::PromptVariable) -> String {
    let mut info = format!("- **{{{{{}}}}}**", v.name);
    if let Some(ref desc) = v.description {
        info.push_str(&format!(": {desc}"));
    }
    if let Some(ref default) = v.default {
        info.push_str(&format!(" (default: `{default}`)"));
    }
    if !v.required {
        info.push_str(" [optional]");
    }
    info
}

/// Finds missing required variables.
pub fn find_missing_required_variables<'a>(
    variables: &'a [crate::models::PromptVariable],
    values: &HashMap<String, String>,
) -> Vec<&'a str> {
    variables
        .iter()
        .filter(|v| v.required && v.default.is_none() && !values.contains_key(&v.name))
        .map(|v| v.name.as_str())
        .collect()
}

/// Truncates a string to a maximum length.
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Formats content based on detail level.
pub fn format_content_for_detail(content: &str, detail: DetailLevel) -> String {
    if content.is_empty() {
        return String::new();
    }
    match detail {
        DetailLevel::Light => String::new(),
        DetailLevel::Medium => format!("\n   {}", truncate(content, 200)),
        DetailLevel::Everything => format!("\n   {content}"),
    }
}

/// Builds a human-readable description of the active filters.
pub fn build_filter_description(filter: &SearchFilter) -> String {
    let mut parts = Vec::new();

    if !filter.namespaces.is_empty() {
        let ns_list: Vec<_> = filter.namespaces.iter().map(Namespace::as_str).collect();
        parts.push(format!("ns:{}", ns_list.join(",")));
    }

    if !filter.tags.is_empty() {
        for tag in &filter.tags {
            parts.push(format!("tag:{tag}"));
        }
    }

    if !filter.tags_any.is_empty() {
        parts.push(format!("tag:{}", filter.tags_any.join(",")));
    }

    if !filter.excluded_tags.is_empty() {
        for tag in &filter.excluded_tags {
            parts.push(format!("-tag:{tag}"));
        }
    }

    if let Some(ref pattern) = filter.source_pattern {
        parts.push(format!("source:{pattern}"));
    }

    if !filter.statuses.is_empty() {
        let status_list: Vec<_> = filter.statuses.iter().map(MemoryStatus::as_str).collect();
        parts.push(format!("status:{}", status_list.join(",")));
    }

    if filter.created_after.is_some() {
        parts.push("since:active".to_string());
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!(", filter: {}", parts.join(" "))
    }
}
