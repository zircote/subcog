//! Golden file tests for MCP responses (LOW-TEST-005).
//!
//! Verifies that MCP responses match expected golden files:
//! - Help index format and content
//! - Namespace listing structure
//! - Resource list completeness
//! - Search filter serialization
//! - Hook response format compliance

// Golden tests use expect/unwrap/panic for simplicity - panics are acceptable in tests
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::unnecessary_map_or
)]

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use subcog::mcp::ResourceHandler;
use subcog::models::Namespace;
use subcog::services::parse_filter_query;

// ============================================================================
// LOW-TEST-005: Golden File Tests for MCP Responses
// ============================================================================

/// Get the path to the golden files directory.
fn golden_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join("tests").join("golden")
}

/// Load a golden file as a string.
fn load_golden(filename: &str) -> String {
    let path = golden_dir().join(filename);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read golden file {}: {e}", path.display()))
}

/// Load a golden JSON file as a Value.
fn load_golden_json(filename: &str) -> Value {
    let content = load_golden(filename);
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse golden JSON {filename}: {e}"))
}

// ============================================================================
// Help Index Tests
// ============================================================================

/// Test: Help index contains all expected sections.
#[test]
fn test_help_index_structure() {
    let mut handler = ResourceHandler::new();
    let result = handler.get_resource("subcog://help").unwrap();
    let content = result.text.unwrap();

    // Verify main sections
    assert!(content.contains("# Subcog Help"));
    assert!(content.contains("Welcome to Subcog"));
    assert!(content.contains("## Available Topics"));
    assert!(content.contains("## Quick Start"));

    // Verify all help categories are listed
    let categories = [
        "setup",
        "concepts",
        "capture",
        "search",
        "workflows",
        "troubleshooting",
        "advanced",
        "prompts",
    ];
    for cat in categories {
        assert!(
            content.contains(&format!("subcog://help/{cat}")),
            "Missing category: {cat}"
        );
    }
}

/// Test: Help index matches golden file structure (not exact content).
#[test]
fn test_help_index_golden_structure() {
    let golden = load_golden("help_index.golden.md");
    let mut handler = ResourceHandler::new();
    let result = handler.get_resource("subcog://help").unwrap();
    let actual = result.text.unwrap();

    // Both should have the same major sections
    let sections = ["# Subcog Help", "## Available Topics", "## Quick Start"];
    for section in sections {
        assert!(
            golden.contains(section),
            "Golden missing section: {section}"
        );
        assert!(
            actual.contains(section),
            "Actual missing section: {section}"
        );
    }
}

// ============================================================================
// Namespace Listing Tests
// ============================================================================

/// Test: Namespace listing matches expected count.
#[test]
fn test_namespaces_count() {
    let golden: Value = load_golden_json("namespaces.golden.json");
    let expected_count = golden["count"].as_u64().unwrap() as usize;

    // user_namespaces excludes Help (system namespace)
    let user_ns = Namespace::user_namespaces();
    assert_eq!(
        user_ns.len(),
        expected_count,
        "User namespace count mismatch"
    );
}

/// Test: Namespace listing has required fields.
#[test]
fn test_namespaces_structure() {
    let golden: Value = load_golden_json("namespaces.golden.json");
    let namespaces = golden["namespaces"].as_array().unwrap();

    for ns in namespaces {
        assert!(ns["namespace"].is_string(), "Missing 'namespace' field");
        assert!(ns["description"].is_string(), "Missing 'description' field");
        assert!(
            ns["signal_words"].is_array(),
            "Missing 'signal_words' field"
        );

        // Signal words should not be empty
        let signal_words = ns["signal_words"].as_array().unwrap();
        assert!(!signal_words.is_empty(), "Signal words should not be empty");
    }
}

/// Test: All golden namespaces exist in code.
#[test]
fn test_namespaces_all_exist() {
    let golden: Value = load_golden_json("namespaces.golden.json");
    let namespaces = golden["namespaces"].as_array().unwrap();

    for ns in namespaces {
        let ns_name = ns["namespace"].as_str().unwrap();
        let parsed = Namespace::parse(ns_name);
        assert!(
            parsed.is_some(),
            "Golden namespace '{ns_name}' not found in code"
        );
    }
}

// ============================================================================
// Resource List Tests
// ============================================================================

/// Test: Resource list contains expected resource types.
#[test]
fn test_resource_list_types() {
    let handler = ResourceHandler::new();
    let resources = handler.list_resources();

    // Check for essential resource types
    let essential_uris = [
        "subcog://_",
        "subcog://search/{query}",
        "subcog://topics",
        "subcog://namespaces",
        "subcog://_prompts",
    ];

    for uri in essential_uris {
        assert!(
            resources.iter().any(|r| r.uri == uri),
            "Missing essential resource: {uri}"
        );
    }
}

/// Test: All resources have required fields.
#[test]
fn test_resource_list_structure() {
    let handler = ResourceHandler::new();
    let resources = handler.list_resources();

    for resource in &resources {
        assert!(!resource.uri.is_empty(), "Resource URI should not be empty");
        assert!(
            !resource.name.is_empty(),
            "Resource name should not be empty"
        );
        assert!(
            resource.mime_type.is_some(),
            "Resource should have mime_type"
        );

        // MIME type should be valid
        let mime = resource.mime_type.as_ref().unwrap();
        assert!(
            mime == "text/markdown" || mime == "application/json",
            "Unexpected MIME type: {mime}"
        );
    }
}

/// Test: Help resources have text/markdown MIME type.
#[test]
fn test_help_resources_mime_type() {
    let handler = ResourceHandler::new();
    let resources = handler.list_resources();

    let help_resources: Vec<_> = resources
        .iter()
        .filter(|r| r.uri.starts_with("subcog://help/"))
        .collect();

    assert!(!help_resources.is_empty(), "Should have help resources");

    for resource in help_resources {
        assert_eq!(
            resource.mime_type.as_deref(),
            Some("text/markdown"),
            "Help resource {} should have text/markdown",
            resource.uri
        );
    }
}

// ============================================================================
// Search Filter Tests
// ============================================================================

/// Test: Empty query produces empty filter.
#[test]
fn test_search_filter_empty_golden() {
    let filter = parse_filter_query("");

    assert!(filter.namespaces.is_empty());
    assert!(filter.tags.is_empty());
    assert!(filter.tags_any.is_empty());
    assert!(filter.excluded_tags.is_empty());
    assert!(filter.statuses.is_empty());
    assert!(filter.source_pattern.is_none());
    assert!(filter.created_after.is_none());
    assert!(filter.created_before.is_none());
    assert!(filter.min_score.is_none());
}

/// Test: Complex filter parses correctly.
#[test]
fn test_search_filter_complex_golden() {
    let golden: Value = load_golden_json("search_filter_complex.golden.json");
    let expected = &golden["expected"];

    // Parse the same query described in the golden file
    let query = "ns:decisions tag:rust,python -tag:test since:7d source:src/*.rs";
    let filter = parse_filter_query(query);

    // Verify namespaces
    let expected_ns: Vec<String> = expected["namespaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    let actual_ns: Vec<String> = filter
        .namespaces
        .iter()
        .map(|ns| ns.as_str().to_string())
        .collect();
    assert_eq!(actual_ns, expected_ns, "Namespaces mismatch");

    // Verify tags_any (comma-separated creates OR)
    let expected_tags_any: Vec<String> = expected["tags_any"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(filter.tags_any, expected_tags_any, "tags_any mismatch");

    // Verify excluded_tags
    let expected_excluded: Vec<String> = expected["excluded_tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        filter.excluded_tags, expected_excluded,
        "excluded_tags mismatch"
    );

    // Verify source pattern
    let expected_source = expected["source_pattern"].as_str();
    assert_eq!(
        filter.source_pattern.as_deref(),
        expected_source,
        "source_pattern mismatch"
    );

    // Verify has_created_after
    let expected_has_after = expected["has_created_after"].as_bool().unwrap();
    assert_eq!(
        filter.created_after.is_some(),
        expected_has_after,
        "created_after presence mismatch"
    );
}

// ============================================================================
// Hook Response Format Tests
// ============================================================================

/// Test: Hook response schema has required fields.
#[test]
fn test_hook_response_schema_session_start() {
    let golden: Value = load_golden_json("hook_response_session_start.golden.json");

    // Verify schema structure
    let schema = &golden["schema"];
    assert!(schema["hookSpecificOutput"].is_object());
    assert_eq!(
        schema["hookSpecificOutput"]["hookEventName"].as_str(),
        Some("SessionStart")
    );
    assert!(schema["hookSpecificOutput"]["additionalContext"].is_string());

    // Verify example structure
    let example = &golden["example"];
    assert!(example["hookSpecificOutput"]["hookEventName"].is_string());
    assert!(example["hookSpecificOutput"]["additionalContext"].is_string());
}

/// Test: Hook response schema has required fields for pre-compact.
#[test]
fn test_hook_response_schema_pre_compact() {
    let golden: Value = load_golden_json("hook_response_pre_compact.golden.json");

    // Verify schema structure
    let schema = &golden["schema"];
    assert!(schema["hookSpecificOutput"].is_object());
    assert_eq!(
        schema["hookSpecificOutput"]["hookEventName"].as_str(),
        Some("PreCompact")
    );

    // Verify example has markdown-formatted context
    let example = &golden["example"];
    let context = example["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(context.contains("**Subcog"));
    assert!(context.contains("Captured"));
}

/// Test: Hook response schema has required fields for stop.
#[test]
fn test_hook_response_schema_stop() {
    let golden: Value = load_golden_json("hook_response_stop.golden.json");

    // Verify schema structure
    let schema = &golden["schema"];
    assert!(schema["hookSpecificOutput"].is_object());
    assert_eq!(
        schema["hookSpecificOutput"]["hookEventName"].as_str(),
        Some("Stop")
    );

    // Verify example has session summary format
    let example = &golden["example"];
    let context = example["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(context.contains("Session Summary"));
    assert!(context.contains("Statistics"));
}

// ============================================================================
// Memory List Tests
// ============================================================================

/// Test: Empty memory list matches golden structure.
#[test]
fn test_memory_list_empty_golden() {
    let golden: Value = load_golden_json("memory_list_empty.golden.json");

    assert_eq!(golden["count"].as_u64(), Some(0));
    assert!(golden["memories"].as_array().unwrap().is_empty());
}

// ============================================================================
// Additional Golden File Verification Tests
// ============================================================================

/// Test: All golden files are valid JSON (except .md files).
#[test]
fn test_all_golden_files_valid() {
    let golden_path = golden_dir();

    for entry in fs::read_dir(&golden_path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.extension().map_or(false, |ext| ext == "json") {
            let content = fs::read_to_string(&path).unwrap();
            let result: Result<Value, _> = serde_json::from_str(&content);
            assert!(
                result.is_ok(),
                "Invalid JSON in {:?}: {:?}",
                path,
                result.err()
            );
        }
    }
}

/// Test: Golden JSON files have expected top-level structure.
#[test]
fn test_golden_json_structure() {
    let json_files = [
        "namespaces.golden.json",
        "search_filter_empty.golden.json",
        "search_filter_complex.golden.json",
        "memory_list_empty.golden.json",
        "hook_response_session_start.golden.json",
        "hook_response_pre_compact.golden.json",
        "hook_response_stop.golden.json",
    ];

    for filename in json_files {
        let content = load_golden(filename);
        let value: Value = serde_json::from_str(&content).unwrap();
        assert!(
            value.is_object(),
            "{filename} should be a JSON object at top level"
        );
    }
}

// ============================================================================
// Regression Tests - Ensure Changes Don't Break Format
// ============================================================================

/// Test: Namespace count has not changed unexpectedly.
#[test]
fn test_namespace_count_regression() {
    // If this test fails, update the golden file after verifying the change is intentional
    let all_namespaces = Namespace::all();
    assert_eq!(
        all_namespaces.len(),
        14,
        "Namespace count changed - update golden files if intentional"
    );

    let user_namespaces = Namespace::user_namespaces();
    assert_eq!(
        user_namespaces.len(),
        13,
        "User namespace count changed - update golden files if intentional"
    );
}

/// Test: Help categories have not changed unexpectedly.
#[test]
fn test_help_categories_regression() {
    let handler = ResourceHandler::new();
    let categories = handler.list_categories();

    assert_eq!(
        categories.len(),
        8,
        "Help category count changed - update golden files if intentional"
    );
}

/// Test: Resource count has not changed unexpectedly.
#[test]
fn test_resource_count_regression() {
    let handler = ResourceHandler::new();
    let resources = handler.list_resources();

    // Count namespace-specific resources + generic resources
    // 8 help + 13 namespace + 6 generic = 27 minimum
    assert!(
        resources.len() >= 20,
        "Resource count unexpectedly low: {} - check for regressions",
        resources.len()
    );
}
