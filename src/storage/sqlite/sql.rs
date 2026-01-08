//! SQL helper functions for `SQLite` backends.
//!
//! This module provides utilities for SQL query construction, including:
//! - LIKE wildcard escaping for security
//! - Glob pattern conversion to SQL LIKE patterns
//! - Filter clause building with numbered parameters
//!
//! These utilities ensure consistent, secure SQL query generation across
//! both the index and persistence backends.

use crate::models::SearchFilter;

/// Escapes SQL LIKE wildcards in a string to make them literal.
///
/// SQL LIKE uses `%` (match any characters) and `_` (match single character)
/// as wildcards. When searching for literal `%` or `_` characters, they must
/// be escaped with a backslash. The backslash itself also needs escaping.
///
/// # Security
///
/// This function is critical for preventing SQL injection when user input
/// is used in LIKE clauses (see SEC-M4 in security audit).
///
/// # Arguments
///
/// * `s` - The string to escape
///
/// # Returns
///
/// A new string with all LIKE wildcards escaped
///
/// # Examples
///
/// ```
/// use subcog::storage::sqlite::escape_like_wildcards;
///
/// assert_eq!(escape_like_wildcards("100%"), "100\\%");
/// assert_eq!(escape_like_wildcards("user_name"), "user\\_name");
/// assert_eq!(escape_like_wildcards("path\\file"), "path\\\\file");
/// ```
#[must_use]
pub fn escape_like_wildcards(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' | '_' | '\\' => {
                result.push('\\');
                result.push(c);
            },
            _ => result.push(c),
        }
    }
    result
}

/// Converts a glob-style pattern to a SQL LIKE pattern.
///
/// Glob patterns use `*` (match any characters) and `?` (match single character).
/// This function converts them to SQL LIKE equivalents (`%` and `_`), while
/// escaping any literal SQL LIKE wildcards that appear in the pattern.
///
/// # Security
///
/// This function properly escapes literal `%`, `_`, and `\` characters before
/// converting glob wildcards, preventing SQL injection (see HIGH-SEC-005).
///
/// # Arguments
///
/// * `pattern` - The glob-style pattern to convert
///
/// # Returns
///
/// A SQL LIKE pattern with glob wildcards converted and literals escaped
///
/// # Examples
///
/// ```
/// use subcog::storage::sqlite::glob_to_like_pattern;
///
/// // Glob wildcards are converted
/// assert_eq!(glob_to_like_pattern("src/*.rs"), "src/%.rs");
/// assert_eq!(glob_to_like_pattern("test?.txt"), "test_.txt");
///
/// // Literal % is escaped
/// assert_eq!(glob_to_like_pattern("100%"), "100\\%");
///
/// // Combined: literal % escaped, glob * converted
/// assert_eq!(glob_to_like_pattern("foo%*bar"), "foo\\%%bar");
/// ```
#[must_use]
pub fn glob_to_like_pattern(pattern: &str) -> String {
    let mut result = String::with_capacity(pattern.len() * 2);
    for c in pattern.chars() {
        match c {
            // Escape existing SQL LIKE wildcards (they're meant to be literal)
            '%' | '_' | '\\' => {
                result.push('\\');
                result.push(c);
            },
            // Convert glob wildcards to SQL LIKE wildcards
            '*' => result.push('%'),
            '?' => result.push('_'),
            // All other characters pass through unchanged
            _ => result.push(c),
        }
    }
    result
}

/// Builds a WHERE clause from a search filter with numbered parameters.
///
/// This function constructs a SQL WHERE clause from a `SearchFilter`, using
/// numbered parameters (`?1`, `?2`, etc.) for safe SQL query construction.
/// It handles all filter fields including namespaces, statuses, tags, dates,
/// and metadata fields.
///
/// # Arguments
///
/// * `filter` - The search filter to convert
/// * `start_param` - The starting parameter number (e.g., 1 or 2)
///
/// # Returns
///
/// A tuple containing:
/// - The WHERE clause string (prefixed with " AND " if non-empty)
/// - Vector of parameter values (as strings)
/// - The next available parameter index
///
/// # Examples
///
/// ```ignore
/// use subcog::storage::sqlite::build_filter_clause_numbered;
/// use subcog::models::{SearchFilter, Namespace};
///
/// let filter = SearchFilter::new()
///     .with_namespace(Namespace::Decisions)
///     .with_tag("security");
///
/// let (clause, params, next_idx) = build_filter_clause_numbered(&filter, 1);
/// // clause = " AND m.namespace IN (?1) AND (',' || m.tags || ',') LIKE ?2 ESCAPE '\\'"
/// // params = ["decisions", "%,security,%"]
/// // next_idx = 3
/// ```
#[must_use]
pub fn build_filter_clause_numbered(
    filter: &SearchFilter,
    start_param: usize,
) -> (String, Vec<String>, usize) {
    let mut conditions = Vec::new();
    let mut params = Vec::new();
    let mut param_idx = start_param;

    if !filter.namespaces.is_empty() {
        let placeholders: Vec<String> = filter
            .namespaces
            .iter()
            .map(|_| {
                let p = format!("?{param_idx}");
                param_idx += 1;
                p
            })
            .collect();
        conditions.push(format!("m.namespace IN ({})", placeholders.join(",")));
        for ns in &filter.namespaces {
            params.push(ns.as_str().to_string());
        }
    }

    if !filter.statuses.is_empty() {
        let placeholders: Vec<String> = filter
            .statuses
            .iter()
            .map(|_| {
                let p = format!("?{param_idx}");
                param_idx += 1;
                p
            })
            .collect();
        conditions.push(format!("m.status IN ({})", placeholders.join(",")));
        for s in &filter.statuses {
            params.push(s.as_str().to_string());
        }
    }

    // Tag filtering (AND logic - must have ALL tags)
    // Use ',tag,' pattern with wrapped column to match whole tags only
    // Escape LIKE wildcards in tags to prevent SQL injection (SEC-M4)
    for tag in &filter.tags {
        conditions.push(format!(
            "(',' || m.tags || ',') LIKE ?{param_idx} ESCAPE '\\'"
        ));
        param_idx += 1;
        params.push(format!("%,{},%", escape_like_wildcards(tag)));
    }

    // Tag filtering (OR logic - must have ANY tag)
    if !filter.tags_any.is_empty() {
        let or_conditions: Vec<String> = filter
            .tags_any
            .iter()
            .map(|tag| {
                let cond = format!("(',' || m.tags || ',') LIKE ?{param_idx} ESCAPE '\\'");
                param_idx += 1;
                params.push(format!("%,{},%", escape_like_wildcards(tag)));
                cond
            })
            .collect();
        conditions.push(format!("({})", or_conditions.join(" OR ")));
    }

    // Excluded tags (NOT LIKE) - match whole tags only
    // Escape LIKE wildcards (SEC-M4)
    for tag in &filter.excluded_tags {
        conditions.push(format!(
            "(',' || m.tags || ',') NOT LIKE ?{param_idx} ESCAPE '\\'"
        ));
        param_idx += 1;
        params.push(format!("%,{},%", escape_like_wildcards(tag)));
    }

    // Source pattern (glob-style converted to SQL LIKE)
    // HIGH-SEC-005: Use glob_to_like_pattern to escape SQL wildcards before conversion
    if let Some(ref pattern) = filter.source_pattern {
        conditions.push(format!("m.source LIKE ?{param_idx} ESCAPE '\\'"));
        param_idx += 1;
        params.push(glob_to_like_pattern(pattern));
    }

    if let Some(ref project_id) = filter.project_id {
        conditions.push(format!("m.project_id = ?{param_idx}"));
        param_idx += 1;
        params.push(project_id.clone());
    }

    if let Some(ref branch) = filter.branch {
        conditions.push(format!("m.branch = ?{param_idx}"));
        param_idx += 1;
        params.push(branch.clone());
    }

    if let Some(ref file_path) = filter.file_path {
        conditions.push(format!("m.file_path = ?{param_idx}"));
        param_idx += 1;
        params.push(file_path.clone());
    }

    if let Some(after) = filter.created_after {
        conditions.push(format!("m.created_at >= ?{param_idx}"));
        param_idx += 1;
        params.push(after.to_string());
    }

    if let Some(before) = filter.created_before {
        conditions.push(format!("m.created_at <= ?{param_idx}"));
        param_idx += 1;
        params.push(before.to_string());
    }

    // Exclude tombstoned memories by default (ADR-0053)
    if !filter.include_tombstoned {
        conditions.push("m.status != 'tombstoned'".to_string());
    }

    let clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" AND {}", conditions.join(" AND "))
    };

    (clause, params, param_idx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MemoryStatus, Namespace};
    use chrono::Utc;

    #[test]
    fn test_escape_like_wildcards() {
        // No special characters
        assert_eq!(escape_like_wildcards("normal"), "normal");
        assert_eq!(escape_like_wildcards("test-tag"), "test-tag");

        // Percent sign (LIKE wildcard for "any characters")
        assert_eq!(escape_like_wildcards("100%"), "100\\%");
        assert_eq!(escape_like_wildcards("%prefix"), "\\%prefix");

        // Underscore (LIKE wildcard for "single character")
        assert_eq!(escape_like_wildcards("user_name"), "user\\_name");
        assert_eq!(escape_like_wildcards("_private"), "\\_private");

        // Backslash (the escape character itself)
        assert_eq!(escape_like_wildcards("path\\file"), "path\\\\file");

        // Multiple special characters
        assert_eq!(escape_like_wildcards("100%_test\\"), "100\\%\\_test\\\\");

        // Empty string
        assert_eq!(escape_like_wildcards(""), "");
    }

    #[test]
    fn test_glob_to_like_pattern() {
        // Glob wildcards are converted
        assert_eq!(glob_to_like_pattern("*"), "%");
        assert_eq!(glob_to_like_pattern("?"), "_");
        assert_eq!(glob_to_like_pattern("src/*.rs"), "src/%.rs");
        assert_eq!(glob_to_like_pattern("test?.txt"), "test_.txt");

        // Literal SQL LIKE wildcards are escaped
        assert_eq!(glob_to_like_pattern("100%"), "100\\%");
        assert_eq!(glob_to_like_pattern("user_name"), "user\\_name");

        // Combined: literal % escaped, glob * converted
        assert_eq!(glob_to_like_pattern("foo%*bar"), "foo\\%%bar");
        assert_eq!(glob_to_like_pattern("*_test%?"), "%\\_test\\%_");

        // Backslash is escaped
        assert_eq!(glob_to_like_pattern("path\\file*"), "path\\\\file%");

        // Complex pattern (** becomes %%, each * is a separate wildcard)
        assert_eq!(
            glob_to_like_pattern("src/**/test_*.rs"),
            "src/%%/test\\_%.rs"
        );

        // Empty string
        assert_eq!(glob_to_like_pattern(""), "");

        // No special characters
        assert_eq!(glob_to_like_pattern("normal"), "normal");
    }

    #[test]
    fn test_build_filter_clause_numbered_empty() {
        let filter = SearchFilter::new();
        let (clause, params, next_idx) = build_filter_clause_numbered(&filter, 1);

        // Empty filter should produce tombstone exclusion only
        assert_eq!(clause, " AND m.status != 'tombstoned'");
        assert!(params.is_empty());
        assert_eq!(next_idx, 1);
    }

    #[test]
    fn test_build_filter_clause_numbered_namespace() {
        let filter = SearchFilter::new().with_namespace(Namespace::Decisions);
        let (clause, params, next_idx) = build_filter_clause_numbered(&filter, 1);

        assert!(clause.contains("m.namespace IN (?1)"));
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], "decisions");
        assert_eq!(next_idx, 2);
    }

    #[test]
    fn test_build_filter_clause_numbered_multiple_namespaces() {
        let filter = SearchFilter::new()
            .with_namespace(Namespace::Decisions)
            .with_namespace(Namespace::Patterns);
        let (clause, params, next_idx) = build_filter_clause_numbered(&filter, 1);

        assert!(clause.contains("m.namespace IN (?1,?2)"));
        assert_eq!(params.len(), 2);
        assert_eq!(params[0], "decisions");
        assert_eq!(params[1], "patterns");
        assert_eq!(next_idx, 3);
    }

    #[test]
    fn test_build_filter_clause_numbered_status() {
        let filter = SearchFilter::new().with_status(MemoryStatus::Active);
        let (clause, params, next_idx) = build_filter_clause_numbered(&filter, 1);

        assert!(clause.contains("m.status IN (?1)"));
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], "active");
        assert_eq!(next_idx, 2);
    }

    #[test]
    fn test_build_filter_clause_numbered_tags_and() {
        let filter = SearchFilter::new().with_tag("security").with_tag("urgent");
        let (clause, params, next_idx) = build_filter_clause_numbered(&filter, 1);

        assert!(clause.contains("(',' || m.tags || ',') LIKE ?1 ESCAPE '\\'"));
        assert!(clause.contains("(',' || m.tags || ',') LIKE ?2 ESCAPE '\\'"));
        assert_eq!(params.len(), 2);
        assert_eq!(params[0], "%,security,%");
        assert_eq!(params[1], "%,urgent,%");
        assert_eq!(next_idx, 3);
    }

    #[test]
    fn test_build_filter_clause_numbered_tags_any() {
        let filter = SearchFilter::new()
            .with_tag_any("bug")
            .with_tag_any("feature");
        let (clause, params, next_idx) = build_filter_clause_numbered(&filter, 1);

        assert!(clause.contains("((',' || m.tags || ',') LIKE ?1 ESCAPE '\\'"));
        assert!(clause.contains("(',' || m.tags || ',') LIKE ?2 ESCAPE '\\'"));
        assert_eq!(params.len(), 2);
        assert_eq!(params[0], "%,bug,%");
        assert_eq!(params[1], "%,feature,%");
        assert_eq!(next_idx, 3);
    }

    #[test]
    fn test_build_filter_clause_numbered_excluded_tags() {
        let filter = SearchFilter::new().with_excluded_tag("draft");
        let (clause, params, next_idx) = build_filter_clause_numbered(&filter, 1);

        assert!(clause.contains("(',' || m.tags || ',') NOT LIKE ?1 ESCAPE '\\'"));
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], "%,draft,%");
        assert_eq!(next_idx, 2);
    }

    #[test]
    fn test_build_filter_clause_numbered_tag_escaping() {
        // Test that tags with SQL wildcards are properly escaped
        let filter = SearchFilter::new().with_tag("100%_complete");
        let (clause, params, next_idx) = build_filter_clause_numbered(&filter, 1);

        assert!(clause.contains("(',' || m.tags || ',') LIKE ?1 ESCAPE '\\'"));
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], "%,100\\%\\_complete,%");
        assert_eq!(next_idx, 2);
    }

    #[test]
    fn test_build_filter_clause_numbered_source_pattern() {
        let filter = SearchFilter::new().with_source_pattern("src/*.rs");
        let (clause, params, next_idx) = build_filter_clause_numbered(&filter, 1);

        assert!(clause.contains("m.source LIKE ?1 ESCAPE '\\'"));
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], "src/%.rs"); // Glob * converted to SQL %
        assert_eq!(next_idx, 2);
    }

    #[test]
    fn test_build_filter_clause_numbered_project_branch_file() {
        let filter = SearchFilter::new()
            .with_project_id("subcog")
            .with_branch("main")
            .with_file_path("src/lib.rs");
        let (clause, params, next_idx) = build_filter_clause_numbered(&filter, 1);

        assert!(clause.contains("m.project_id = ?1"));
        assert!(clause.contains("m.branch = ?2"));
        assert!(clause.contains("m.file_path = ?3"));
        assert_eq!(params.len(), 3);
        assert_eq!(params[0], "subcog");
        assert_eq!(params[1], "main");
        assert_eq!(params[2], "src/lib.rs");
        assert_eq!(next_idx, 4);
    }

    #[test]
    #[allow(clippy::cast_sign_loss)] // Test uses current timestamp which is always positive
    fn test_build_filter_clause_numbered_dates() {
        let now = Utc::now();
        let after = now.timestamp() as u64;
        let before = (now.timestamp() + 3600) as u64; // 1 hour later
        let filter = SearchFilter::new()
            .with_created_after(after)
            .with_created_before(before);
        let (clause, params, next_idx) = build_filter_clause_numbered(&filter, 1);

        assert!(clause.contains("m.created_at >= ?1"));
        assert!(clause.contains("m.created_at <= ?2"));
        assert_eq!(params.len(), 2);
        assert_eq!(params[0], after.to_string());
        assert_eq!(params[1], before.to_string());
        assert_eq!(next_idx, 3);
    }

    #[test]
    fn test_build_filter_clause_numbered_include_tombstoned() {
        let filter = SearchFilter::new().with_include_tombstoned(true);
        let (clause, params, next_idx) = build_filter_clause_numbered(&filter, 1);

        // Should not include tombstone exclusion
        assert!(!clause.contains("m.status != 'tombstoned'"));
        assert!(params.is_empty());
        assert_eq!(next_idx, 1);
    }

    #[test]
    fn test_build_filter_clause_numbered_start_param() {
        // Test that start_param is respected
        let filter = SearchFilter::new()
            .with_namespace(Namespace::Decisions)
            .with_tag("test");
        let (clause, params, next_idx) = build_filter_clause_numbered(&filter, 5);

        assert!(clause.contains("?5")); // Namespace starts at 5
        assert!(clause.contains("?6")); // Tag uses 6
        assert_eq!(params.len(), 2);
        assert_eq!(next_idx, 7);
    }

    #[test]
    fn test_build_filter_clause_numbered_complex() {
        // Test a complex filter with multiple conditions
        let filter = SearchFilter::new()
            .with_namespace(Namespace::Decisions)
            .with_namespace(Namespace::Patterns)
            .with_status(MemoryStatus::Active)
            .with_tag("security")
            .with_tag_any("review")
            .with_excluded_tag("draft")
            .with_source_pattern("src/*.rs")
            .with_project_id("subcog")
            .with_branch("main");

        let (clause, params, next_idx) = build_filter_clause_numbered(&filter, 1);

        // Should have conditions for all filter criteria
        assert!(clause.contains("m.namespace IN (?1,?2)"));
        assert!(clause.contains("m.status IN (?3)"));
        assert!(clause.contains("(',' || m.tags || ',') LIKE ?4"));
        assert!(clause.contains("(',' || m.tags || ',') LIKE ?5"));
        assert!(clause.contains("NOT LIKE ?6"));
        assert!(clause.contains("m.source LIKE ?7"));
        assert!(clause.contains("m.project_id = ?8"));
        assert!(clause.contains("m.branch = ?9"));

        assert_eq!(params.len(), 9);
        assert_eq!(next_idx, 10);
    }
}
