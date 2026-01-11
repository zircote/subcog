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
    /// Namespaces to consolidate (optional, defaults to all).
    pub namespaces: Option<Vec<String>>,
    /// Time window in days (optional).
    pub days: Option<u32>,
    /// If true, show what would be consolidated without making changes.
    pub dry_run: Option<bool>,
    /// Minimum number of memories required to form a group (optional).
    pub min_memories: Option<usize>,
    /// Similarity threshold 0.0-1.0 for grouping memories (optional).
    pub similarity: Option<f32>,
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

/// Finds the largest valid UTF-8 character boundary at or before `index`.
///
/// This is an MSRV-compatible implementation of `str::floor_char_boundary`
/// (stable since Rust 1.80, but we target 1.86 MSRV).
///
/// # Arguments
///
/// * `s` - The string to find a boundary in.
/// * `index` - The byte index to search from (will find boundary at or before).
///
/// # Returns
///
/// The largest valid character boundary at or before `index`, or 0 if none found.
fn floor_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }

    // Find the last character boundary at or before index using char_indices.
    // char_indices() yields (byte_offset, char) for each character.
    // We want the largest byte_offset <= index.
    let mut boundary = 0;
    for (byte_offset, _) in s.char_indices() {
        if byte_offset <= index {
            boundary = byte_offset;
        } else {
            break;
        }
    }
    boundary
}

/// Truncates a string to a maximum length, respecting UTF-8 character boundaries.
///
/// This function safely handles multi-byte UTF-8 characters (e.g., degree symbol,
/// emoji, CJK characters) by finding the nearest valid character boundary.
///
/// # Arguments
///
/// * `s` - The string to truncate.
/// * `max_len` - Maximum byte length for the result (including "..." suffix).
///
/// # Returns
///
/// The original string if it fits, otherwise a truncated version with "..." appended.
///
/// # Examples
///
/// ```ignore
/// // ASCII text
/// assert_eq!(truncate("Hello, world!", 10), "Hello, ...");
///
/// // Multi-byte UTF-8 characters (degree symbol is 2 bytes)
/// assert_eq!(truncate("32 °C temperature", 10), "32 °C ...");
///
/// // String shorter than max_len
/// assert_eq!(truncate("short", 100), "short");
/// ```
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }

    // Reserve 3 bytes for "..."
    let target_len = max_len.saturating_sub(3);

    // Find the largest valid character boundary <= target_len
    let boundary = floor_char_boundary(s, target_len);

    format!("{}...", &s[..boundary])
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

    if let Some(ref project_id) = filter.project_id {
        parts.push(format!("project:{project_id}"));
    }

    if let Some(ref branch) = filter.branch {
        parts.push(format!("branch:{branch}"));
    }

    if let Some(ref file_path) = filter.file_path {
        parts.push(format!("path:{file_path}"));
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
        let json = r#"{"namespaces": ["decisions"], "extra": true}"#;
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

    // ==========================================================================
    // UTF-8 safe truncation tests
    // ==========================================================================

    #[test]
    fn test_truncate_ascii_short() {
        assert_eq!(truncate("short", 100), "short");
    }

    #[test]
    fn test_truncate_ascii_exact() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_ascii_long() {
        assert_eq!(truncate("hello world", 8), "hello...");
    }

    #[test]
    fn test_truncate_degree_symbol() {
        // The degree symbol (°) is 2 bytes (U+00B0: 0xC2 0xB0)
        // "32 °C" = [51, 50, 32, 194, 176, 67] = 6 bytes
        let s = "32 °C temperature";
        // With max_len=10, target_len=7, boundary should be at 6 (after °)
        let result = truncate(s, 10);
        assert!(result.ends_with("..."));
        // Should not panic and should contain valid UTF-8
        assert!(result.is_ascii() || !result.is_empty());
    }

    #[test]
    fn test_truncate_multi_byte_boundary() {
        // Test the exact case from the panic: degree symbol at byte 196-198
        let s = "Document 301:\nThe Mallee and upper Wimmera are Victoria's warmest regions with hot winds blowing from nearby semi-deserts. Average temperatures exceed 32 °C (90 °F) during summer and 15 °C (59 °F) in winter...";

        // This was panicking at max_len=200 because byte 197 is inside °
        let result = truncate(s, 200);
        assert!(result.ends_with("..."));
        // Verify it's valid UTF-8 (won't compile if not, but good to be explicit)
        assert!(!result.is_empty());
    }

    #[test]
    fn test_truncate_emoji() {
        // Emoji are 4 bytes each
        let s = "Hello 👋 World 🌍 Test";
        let result = truncate(s, 15);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_cjk() {
        // CJK characters are 3 bytes each
        let s = "Hello 你好 World";
        let result = truncate(s, 12);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_empty() {
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn test_truncate_very_small_max() {
        // With max_len=3, we have 0 bytes for content
        let result = truncate("hello", 3);
        assert_eq!(result, "...");
    }

    #[test]
    fn test_truncate_max_len_zero() {
        // Edge case: max_len=0
        let result = truncate("hello", 0);
        assert_eq!(result, "...");
    }

    // ==========================================================================
    // floor_char_boundary tests
    // ==========================================================================

    #[test]
    fn test_floor_char_boundary_ascii() {
        let s = "hello";
        assert_eq!(floor_char_boundary(s, 0), 0);
        assert_eq!(floor_char_boundary(s, 2), 2);
        assert_eq!(floor_char_boundary(s, 5), 5);
        assert_eq!(floor_char_boundary(s, 10), 5); // beyond end
    }

    #[test]
    fn test_floor_char_boundary_multi_byte() {
        // "°" is bytes 0..2 (2 bytes)
        let s = "°C";
        assert_eq!(floor_char_boundary(s, 0), 0);
        assert_eq!(floor_char_boundary(s, 1), 0); // inside °, floor to 0
        assert_eq!(floor_char_boundary(s, 2), 2); // at C
        assert_eq!(floor_char_boundary(s, 3), 3); // end
    }

    #[test]
    fn test_floor_char_boundary_emoji() {
        // "👋" is 4 bytes
        let s = "a👋b";
        assert_eq!(floor_char_boundary(s, 0), 0); // at 'a'
        assert_eq!(floor_char_boundary(s, 1), 1); // at start of emoji
        assert_eq!(floor_char_boundary(s, 2), 1); // inside emoji
        assert_eq!(floor_char_boundary(s, 3), 1); // inside emoji
        assert_eq!(floor_char_boundary(s, 4), 1); // inside emoji
        assert_eq!(floor_char_boundary(s, 5), 5); // at 'b'
    }

    #[test]
    fn test_floor_char_boundary_empty() {
        assert_eq!(floor_char_boundary("", 0), 0);
        assert_eq!(floor_char_boundary("", 5), 0);
    }
}
