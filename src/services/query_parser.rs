//! Filter query parser for memory search.
//!
//! Parses GitHub-style filter syntax like:
//! - `ns:decisions` - Filter by namespace
//! - `tag:rust` - Filter by tag (AND with other tags)
//! - `tag:rust,python` - Filter by tags (OR logic)
//! - `-tag:test` - Exclude memories with tag
//! - `since:7d` - Filter by time
//! - `source:src/*` - Filter by source pattern
//! - `status:active` - Filter by status

use crate::models::{MemoryStatus, Namespace, SearchFilter};

/// Parses a filter query string into a `SearchFilter`.
///
/// # Arguments
///
/// * `query` - The filter query string (e.g., "ns:decisions tag:rust")
///
/// # Returns
///
/// A `SearchFilter` populated with the parsed criteria.
///
/// # Examples
///
/// ```
/// use subcog::services::parse_filter_query;
///
/// let filter = parse_filter_query("ns:decisions tag:rust -tag:test");
/// assert_eq!(filter.namespaces.len(), 1);
/// assert_eq!(filter.tags.len(), 1);
/// assert_eq!(filter.excluded_tags.len(), 1);
/// ```
#[must_use]
pub fn parse_filter_query(query: &str) -> SearchFilter {
    let mut filter = SearchFilter::new();

    // Split on whitespace to get individual tokens
    for token in query.split_whitespace() {
        parse_token(token, &mut filter);
    }

    filter
}

/// Parses a single filter token and updates the filter.
fn parse_token(token: &str, filter: &mut SearchFilter) {
    // Check for exclusion prefix
    if let Some(rest) = token.strip_prefix('-') {
        parse_excluded_token(rest, filter);
        return;
    }

    // Parse key:value tokens
    let Some((key, value)) = token.split_once(':') else {
        return;
    };

    match key.to_lowercase().as_str() {
        "ns" | "namespace" => {
            if let Some(ns) = Namespace::parse(value) {
                filter.namespaces.push(ns);
            }
        },
        "tag" | "tags" => parse_tag_value(value, filter),
        "since" => {
            if let Some(timestamp) = parse_duration_to_timestamp(value) {
                filter.created_after = Some(timestamp);
            }
        },
        "source" | "src" => {
            filter.source_pattern = Some(value.to_string());
        },
        "status" => {
            if let Some(status) = parse_status(value) {
                filter.statuses.push(status);
            }
        },
        _ => {
            // Unknown key, ignore
        },
    }
}

/// Parses an excluded token (prefixed with -).
fn parse_excluded_token(rest: &str, filter: &mut SearchFilter) {
    let Some(tag_value) = rest.strip_prefix("tag:") else {
        return;
    };
    // Excluded tags support comma-separated values
    for tag in tag_value.split(',') {
        let tag = tag.trim();
        if !tag.is_empty() {
            filter.excluded_tags.push(tag.to_string());
        }
    }
}

/// Parses tag values and adds them to the appropriate filter field.
fn parse_tag_value(value: &str, filter: &mut SearchFilter) {
    // Comma-separated values use OR logic (tags_any)
    // Space-separated (multiple tag: tokens) use AND logic (tags)
    let tags: Vec<&str> = value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    if tags.len() > 1 {
        // Multiple values in one token = OR logic
        filter.tags_any.extend(tags.iter().map(|&t| t.to_string()));
    } else if let Some(&tag) = tags.first() {
        // Single value = AND logic
        filter.tags.push(tag.to_string());
    }
}

/// Parses a duration string (e.g., "7d", "30d") into a Unix timestamp.
///
/// Returns the timestamp representing "now minus duration".
fn parse_duration_to_timestamp(duration: &str) -> Option<u64> {
    let duration = duration.trim().to_lowercase();

    // Parse number and unit
    let (num_str, unit) = if duration.ends_with('d') {
        (duration.trim_end_matches('d'), "d")
    } else if duration.ends_with('h') {
        (duration.trim_end_matches('h'), "h")
    } else if duration.ends_with('w') {
        (duration.trim_end_matches('w'), "w")
    } else {
        // Default to days if no unit
        (duration.as_str(), "d")
    };

    let num: u64 = num_str.parse().ok()?;

    let seconds = match unit {
        "h" => num * 3600,
        "w" => num * 604_800,
        // Default to days for "d" and unknown units
        _ => num * 86400,
    };

    // Get current time and subtract duration
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    Some(now.saturating_sub(seconds))
}

/// Parses a status string into a `MemoryStatus`.
fn parse_status(s: &str) -> Option<MemoryStatus> {
    match s.to_lowercase().as_str() {
        "active" => Some(MemoryStatus::Active),
        "archived" => Some(MemoryStatus::Archived),
        "superseded" => Some(MemoryStatus::Superseded),
        "pending" => Some(MemoryStatus::Pending),
        "deleted" => Some(MemoryStatus::Deleted),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_query() {
        let filter = parse_filter_query("");
        assert!(filter.is_empty());
    }

    #[test]
    fn test_parse_namespace() {
        let filter = parse_filter_query("ns:decisions");
        assert_eq!(filter.namespaces.len(), 1);
        assert_eq!(filter.namespaces[0], Namespace::Decisions);
    }

    #[test]
    fn test_parse_namespace_full_name() {
        let filter = parse_filter_query("namespace:patterns");
        assert_eq!(filter.namespaces.len(), 1);
        assert_eq!(filter.namespaces[0], Namespace::Patterns);
    }

    #[test]
    fn test_parse_single_tag() {
        let filter = parse_filter_query("tag:rust");
        assert_eq!(filter.tags.len(), 1);
        assert_eq!(filter.tags[0], "rust");
        assert!(filter.tags_any.is_empty());
    }

    #[test]
    fn test_parse_multiple_tags_and_logic() {
        let filter = parse_filter_query("tag:rust tag:error");
        assert_eq!(filter.tags.len(), 2);
        assert!(filter.tags.contains(&"rust".to_string()));
        assert!(filter.tags.contains(&"error".to_string()));
    }

    #[test]
    fn test_parse_tags_or_logic() {
        let filter = parse_filter_query("tag:rust,python,go");
        assert!(filter.tags.is_empty());
        assert_eq!(filter.tags_any.len(), 3);
        assert!(filter.tags_any.contains(&"rust".to_string()));
        assert!(filter.tags_any.contains(&"python".to_string()));
        assert!(filter.tags_any.contains(&"go".to_string()));
    }

    #[test]
    fn test_parse_excluded_tags() {
        let filter = parse_filter_query("-tag:test");
        assert_eq!(filter.excluded_tags.len(), 1);
        assert_eq!(filter.excluded_tags[0], "test");
    }

    #[test]
    fn test_parse_excluded_tags_multiple() {
        let filter = parse_filter_query("-tag:test,deprecated");
        assert_eq!(filter.excluded_tags.len(), 2);
        assert!(filter.excluded_tags.contains(&"test".to_string()));
        assert!(filter.excluded_tags.contains(&"deprecated".to_string()));
    }

    #[test]
    fn test_parse_status() {
        let filter = parse_filter_query("status:active");
        assert_eq!(filter.statuses.len(), 1);
        assert_eq!(filter.statuses[0], MemoryStatus::Active);
    }

    #[test]
    fn test_parse_source_pattern() {
        let filter = parse_filter_query("source:src/*");
        assert_eq!(filter.source_pattern, Some("src/*".to_string()));
    }

    #[test]
    fn test_parse_since() {
        let filter = parse_filter_query("since:7d");
        assert!(filter.created_after.is_some());

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let seven_days_ago = now - (7 * 86400);

        // Allow 1 second tolerance
        let diff = filter.created_after.unwrap().abs_diff(seven_days_ago);
        assert!(diff <= 1);
    }

    #[test]
    fn test_parse_complex_query() {
        let filter = parse_filter_query("ns:decisions tag:rust tag:database -tag:test since:30d");
        assert_eq!(filter.namespaces.len(), 1);
        assert_eq!(filter.namespaces[0], Namespace::Decisions);
        assert_eq!(filter.tags.len(), 2);
        assert!(filter.tags.contains(&"rust".to_string()));
        assert!(filter.tags.contains(&"database".to_string()));
        assert_eq!(filter.excluded_tags.len(), 1);
        assert_eq!(filter.excluded_tags[0], "test");
        assert!(filter.created_after.is_some());
    }

    #[test]
    fn test_parse_unknown_filter_ignored() {
        let filter = parse_filter_query("unknown:value tag:rust");
        assert_eq!(filter.tags.len(), 1);
        assert_eq!(filter.tags[0], "rust");
    }

    #[test]
    fn test_parse_case_insensitive() {
        let filter = parse_filter_query("NS:DECISIONS TAG:Rust STATUS:Active");
        assert_eq!(filter.namespaces.len(), 1);
        assert_eq!(filter.tags.len(), 1);
        assert_eq!(filter.tags[0], "Rust"); // Tag value preserves case
        assert_eq!(filter.statuses.len(), 1);
    }
}
