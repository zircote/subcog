//! Argument types and helper functions for MCP tools.
//!
//! Extracted from `tools.rs` to reduce file size.
//!
//! # Security
//!
//! All argument types use `#[serde(deny_unknown_fields)]` to prevent
//! parameter pollution attacks where attackers inject unexpected fields
//! that could bypass validation or trigger unintended behavior.

use crate::models::{DetailLevel, MemoryStatus, Namespace, SearchFilter, SearchMode};
use crate::storage::index::DomainScope;
use serde::Deserialize;
use std::collections::HashMap;

/// Arguments for the capture tool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureArgs {
    /// The memory content to capture.
    pub content: String,
    /// Memory category (e.g., "decisions", "patterns", "learnings").
    pub namespace: String,
    /// Optional tags for categorization and filtering.
    pub tags: Option<Vec<String>>,
    /// Optional source reference (file path, URL, etc.).
    pub source: Option<String>,
}

/// Arguments for the recall tool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecallArgs {
    /// Search query text.
    pub query: String,
    /// GitHub-style filter query (e.g., "ns:decisions tag:rust -tag:test since:7d").
    pub filter: Option<String>,
    /// Filter by namespace (deprecated: use `filter` instead).
    pub namespace: Option<String>,
    /// Search mode: "hybrid" (default), "vector", or "text".
    pub mode: Option<String>,
    /// Detail level: "light", "medium" (default), or "everything".
    pub detail: Option<String>,
    /// Maximum number of results to return (default: 10).
    pub limit: Option<usize>,
}

/// Arguments for the consolidate tool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsolidateArgs {
    /// Namespace to consolidate (required).
    pub namespace: String,
    /// Optional query to filter memories for consolidation.
    pub query: Option<String>,
    /// Consolidation strategy: "merge", "summarize", or "dedupe".
    pub strategy: Option<String>,
    /// If true, show what would be consolidated without making changes.
    pub dry_run: Option<bool>,
}

/// Arguments for the enrich tool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrichArgs {
    /// ID of the memory to enrich.
    pub memory_id: String,
    /// Generate or improve tags (default: true).
    pub enrich_tags: Option<bool>,
    /// Restructure content for clarity (default: true).
    pub enrich_structure: Option<bool>,
    /// Add inferred context and rationale (default: false).
    pub add_context: Option<bool>,
}

/// Arguments for the sync tool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncArgs {
    /// Sync direction: "push", "fetch", or "full" (default: "full").
    pub direction: Option<String>,
}

/// Arguments for the reindex tool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReindexArgs {
    /// Path to git repository (default: current directory).
    pub repo_path: Option<String>,
}

// ============================================================================
// Prompt Tool Arguments
// ============================================================================

/// Arguments for the prompt.save tool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptSaveArgs {
    /// Unique prompt name (kebab-case, e.g., "code-review").
    pub name: String,
    /// Prompt content with `{{variable}}` placeholders.
    pub content: Option<String>,
    /// Path to file containing prompt (alternative to content).
    pub file_path: Option<String>,
    /// Human-readable description of the prompt.
    pub description: Option<String>,
    /// Tags for categorization and search.
    pub tags: Option<Vec<String>>,
    /// Storage scope: "project" (default), "user", or "org".
    pub domain: Option<String>,
    /// Explicit variable definitions with metadata.
    pub variables: Option<Vec<PromptVariableArg>>,
    /// Skip LLM-powered metadata enrichment.
    #[serde(default)]
    pub skip_enrichment: bool,
}

/// Variable definition argument for prompt.save.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptVariableArg {
    /// Variable name (without braces).
    pub name: String,
    /// Human-readable description for elicitation.
    pub description: Option<String>,
    /// Default value if not provided.
    pub default: Option<String>,
    /// Whether variable is required (default: true).
    pub required: Option<bool>,
}

/// Arguments for the prompt.list tool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptListArgs {
    /// Filter by domain scope: "project", "user", or "org".
    pub domain: Option<String>,
    /// Filter by tags (AND logic - must have all).
    pub tags: Option<Vec<String>>,
    /// Filter by name pattern (glob-style, e.g., "code-*").
    pub name_pattern: Option<String>,
    /// Maximum number of results (default: 20, max: 100).
    pub limit: Option<usize>,
}

/// Arguments for the prompt.get tool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptGetArgs {
    /// Prompt name to retrieve.
    pub name: String,
    /// Domain to search (if not specified, searches Project -> User -> Org).
    pub domain: Option<String>,
}

/// Arguments for the prompt.run tool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptRunArgs {
    /// Prompt name to execute.
    pub name: String,
    /// Variable values to substitute (key: value pairs).
    pub variables: Option<HashMap<String, String>>,
    /// Domain to search for the prompt.
    pub domain: Option<String>,
}

/// Arguments for the prompt.delete tool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptDeleteArgs {
    /// Prompt name to delete.
    pub name: String,
    /// Domain scope to delete from (required for safety).
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

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // MED-SEC-001: Tests for deny_unknown_fields protection
    // ==========================================================================

    #[test]
    fn test_capture_args_rejects_unknown_fields() {
        let json = r#"{"content": "test", "namespace": "decisions", "unknown_field": "bad"}"#;
        let result: Result<CaptureArgs, _> = serde_json::from_str(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown field"));
    }

    #[test]
    fn test_capture_args_accepts_valid_fields() {
        let json = r#"{"content": "test", "namespace": "decisions", "tags": ["a", "b"]}"#;
        let result: Result<CaptureArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_recall_args_rejects_unknown_fields() {
        let json = r#"{"query": "test", "malicious_param": "attack"}"#;
        let result: Result<RecallArgs, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_consolidate_args_rejects_unknown_fields() {
        let json = r#"{"namespace": "decisions", "extra": true}"#;
        let result: Result<ConsolidateArgs, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_enrich_args_rejects_unknown_fields() {
        let json = r#"{"memory_id": "123", "inject": "payload"}"#;
        let result: Result<EnrichArgs, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_sync_args_rejects_unknown_fields() {
        let json = r#"{"direction": "push", "force": true}"#;
        let result: Result<SyncArgs, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_reindex_args_rejects_unknown_fields() {
        let json = r#"{"repo_path": "/path", "delete_all": true}"#;
        let result: Result<ReindexArgs, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_save_args_rejects_unknown_fields() {
        let json = r#"{"name": "test", "admin_override": true}"#;
        let result: Result<PromptSaveArgs, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_variable_arg_rejects_unknown_fields() {
        let json = r#"{"name": "var", "execute_code": "rm -rf /"}"#;
        let result: Result<PromptVariableArg, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_list_args_rejects_unknown_fields() {
        let json = r#"{"domain": "user", "bypass_auth": true}"#;
        let result: Result<PromptListArgs, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_get_args_rejects_unknown_fields() {
        let json = r#"{"name": "test", "include_secrets": true}"#;
        let result: Result<PromptGetArgs, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_run_args_rejects_unknown_fields() {
        let json = r#"{"name": "test", "shell_escape": true}"#;
        let result: Result<PromptRunArgs, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_delete_args_rejects_unknown_fields() {
        let json = r#"{"name": "test", "domain": "user", "recursive": true}"#;
        let result: Result<PromptDeleteArgs, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
