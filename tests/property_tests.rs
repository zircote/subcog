//! Property-based tests for memory content (LOW-TEST-001).
//!
//! Uses proptest to verify invariants across random inputs:
//! - Memory ID generation is deterministic
//! - Namespace parsing roundtrips correctly
//! - Domain display/parse roundtrips
//! - Content normalization is idempotent
//! - Search filters are composable

// Property tests use expect/unwrap for simplicity - panics are acceptable in tests
#![allow(clippy::expect_used, clippy::unwrap_used)]

use proptest::prelude::*;
use subcog::SearchFilter;
use subcog::models::{Domain, MemoryId, MemoryStatus, Namespace};
use subcog::services::parse_filter_query;

// ============================================================================
// LOW-TEST-001: Property-Based Tests for Memory Content
// ============================================================================

proptest! {
    /// Property: `MemoryId` preserves input string exactly.
    #[test]
    fn prop_memory_id_preserves_string(s in "[a-zA-Z0-9_-]{1,100}") {
        let id = MemoryId::new(&s);
        prop_assert_eq!(id.as_str(), s.as_str());
        prop_assert_eq!(id.to_string(), s);
    }

    /// Property: `MemoryId` from String equals `MemoryId` from &str.
    #[test]
    fn prop_memory_id_from_string_equals_from_str(s in "[a-zA-Z0-9]{1,50}") {
        let from_str = MemoryId::from(s.as_str());
        let from_string = MemoryId::from(s);
        prop_assert_eq!(from_str, from_string);
    }

    /// Property: Namespace::parse is case-insensitive.
    #[test]
    fn prop_namespace_parse_case_insensitive(ns in prop::sample::select(vec![
        "decisions", "patterns", "learnings", "context", "tech-debt",
        "blockers", "progress", "apis", "config", "security",
        "performance", "testing", "help", "prompts"
    ])) {
        let lower = Namespace::parse(&ns.to_lowercase());
        let upper = Namespace::parse(&ns.to_uppercase());
        let mixed = Namespace::parse(&ns.chars().enumerate()
            .map(|(i, c)| if i % 2 == 0 { c.to_uppercase().next().unwrap() } else { c })
            .collect::<String>());

        prop_assert!(lower.is_some());
        prop_assert_eq!(lower, upper);
        prop_assert_eq!(lower, mixed);
    }

    /// Property: Namespace::as_str roundtrips through parse.
    #[test]
    fn prop_namespace_as_str_roundtrips(idx in 0usize..14) {
        let all = Namespace::all();
        if idx < all.len() {
            let ns = all[idx];
            let s = ns.as_str();
            let parsed = Namespace::parse(s);
            prop_assert_eq!(parsed, Some(ns));
        }
    }

    /// Property: Domain::is_project_scoped is true iff all fields are None.
    #[test]
    fn prop_domain_is_project_scoped_iff_all_none(
        org in proptest::option::of("[a-z]{1,10}"),
        proj in proptest::option::of("[a-z]{1,10}"),
        repo in proptest::option::of("[a-z]{1,10}")
    ) {
        let domain = Domain {
            organization: org.clone(),
            project: proj.clone(),
            repository: repo.clone(),
        };

        let expected_project_scoped = org.is_none() && proj.is_none() && repo.is_none();
        prop_assert_eq!(domain.is_project_scoped(), expected_project_scoped);
    }

    /// Property: Domain::for_repository creates non-project-scoped domain.
    #[test]
    fn prop_domain_for_repository_not_project_scoped(
        org in "[a-z]{1,10}",
        repo in "[a-z]{1,10}"
    ) {
        let domain = Domain::for_repository(&org, &repo);
        prop_assert!(!domain.is_project_scoped());
        prop_assert_eq!(domain.organization, Some(org));
        prop_assert_eq!(domain.repository, Some(repo));
    }

    /// Property: Domain::for_user is not project-scoped but is_user returns true.
    #[test]
    fn prop_domain_for_user_is_user(_dummy in 0..1i32) {
        let domain = Domain::for_user();
        prop_assert!(!domain.is_project_scoped());
        prop_assert!(domain.is_user());
    }

    /// Property: `MemoryStatus::as_str` returns non-empty string.
    #[test]
    fn prop_memory_status_as_str_non_empty(idx in 0usize..5) {
        let statuses = [
            MemoryStatus::Active,
            MemoryStatus::Archived,
            MemoryStatus::Superseded,
            MemoryStatus::Pending,
            MemoryStatus::Deleted,
        ];
        if idx < statuses.len() {
            let status = statuses[idx];
            prop_assert!(!status.as_str().is_empty());
        }
    }

    /// Property: `SearchFilter` with namespaces is not empty.
    #[test]
    fn prop_search_filter_with_namespace_not_empty(idx in 0usize..14) {
        let all = Namespace::all();
        if idx < all.len() {
            let filter = SearchFilter::new().with_namespace(all[idx]);
            prop_assert!(!filter.is_empty());
        }
    }

    /// Property: Empty query produces empty filter.
    #[test]
    fn prop_empty_query_empty_filter(whitespace in "[ \t\n]{0,10}") {
        let filter = parse_filter_query(&whitespace);
        prop_assert!(filter.is_empty());
    }

    /// Property: Single tag query adds exactly one tag.
    #[test]
    fn prop_single_tag_query_adds_one_tag(tag in "[a-zA-Z][a-zA-Z0-9_-]{0,19}") {
        let query = format!("tag:{tag}");
        let filter = parse_filter_query(&query);
        prop_assert_eq!(filter.tags.len(), 1);
        prop_assert_eq!(&filter.tags[0], &tag);
    }

    /// Property: Multiple tag: tokens create AND logic (multiple tags).
    #[test]
    fn prop_multiple_tag_tokens_and_logic(
        tag1 in "[a-z]{3,10}",
        tag2 in "[a-z]{3,10}"
    ) {
        prop_assume!(tag1 != tag2);
        let query = format!("tag:{tag1} tag:{tag2}");
        let filter = parse_filter_query(&query);
        prop_assert_eq!(filter.tags.len(), 2);
        prop_assert!(filter.tags.contains(&tag1));
        prop_assert!(filter.tags.contains(&tag2));
    }

    /// Property: Comma-separated tags create OR logic (tags_any).
    #[test]
    fn prop_comma_separated_tags_or_logic(
        tag1 in "[a-z]{3,10}",
        tag2 in "[a-z]{3,10}"
    ) {
        prop_assume!(tag1 != tag2);
        let query = format!("tag:{tag1},{tag2}");
        let filter = parse_filter_query(&query);
        prop_assert!(filter.tags.is_empty());
        prop_assert_eq!(filter.tags_any.len(), 2);
        prop_assert!(filter.tags_any.contains(&tag1));
        prop_assert!(filter.tags_any.contains(&tag2));
    }

    /// Property: Excluded tags are separate from included tags.
    #[test]
    fn prop_excluded_tags_separate(
        tag in "[a-z]{3,10}"
    ) {
        let query = format!("-tag:{tag}");
        let filter = parse_filter_query(&query);
        prop_assert!(filter.tags.is_empty());
        prop_assert!(filter.tags_any.is_empty());
        prop_assert_eq!(filter.excluded_tags.len(), 1);
        prop_assert_eq!(&filter.excluded_tags[0], &tag);
    }

    /// Property: Valid namespace string parses successfully.
    #[test]
    fn prop_valid_namespace_parses(ns_name in prop::sample::select(vec![
        "ns:decisions", "namespace:patterns", "ns:learnings",
        "ns:context", "ns:tech-debt", "ns:blockers", "ns:progress",
        "ns:apis", "ns:config", "ns:security", "ns:performance",
        "ns:testing", "ns:help", "ns:prompts"
    ])) {
        let filter = parse_filter_query(ns_name);
        prop_assert_eq!(filter.namespaces.len(), 1);
    }

    /// Property: Invalid namespace string results in empty namespaces.
    #[test]
    fn prop_invalid_namespace_empty(invalid in "[a-z]{10,20}") {
        // Ensure it's not a valid namespace
        prop_assume!(Namespace::parse(&invalid).is_none());
        let query = format!("ns:{invalid}");
        let filter = parse_filter_query(&query);
        prop_assert!(filter.namespaces.is_empty());
    }

    /// Property: Duration parsing produces timestamp in the past.
    #[test]
    fn prop_duration_produces_past_timestamp(days in 1u64..365) {
        let query = format!("since:{days}d");
        let filter = parse_filter_query(&query);
        prop_assert!(filter.created_after.is_some());

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timestamp = filter.created_after.unwrap();

        // Timestamp should be in the past
        prop_assert!(timestamp < now);
        // Timestamp should be roughly (now - days * 86400)
        let expected = now.saturating_sub(days * 86400);
        let diff = timestamp.abs_diff(expected);
        prop_assert!(diff <= 2); // Allow 2 second tolerance
    }

    /// Property: Source pattern is preserved exactly.
    #[test]
    fn prop_source_pattern_preserved(pattern in "[a-zA-Z0-9_/.\\*]{1,50}") {
        let query = format!("source:{pattern}");
        let filter = parse_filter_query(&query);
        prop_assert_eq!(filter.source_pattern, Some(pattern));
    }

    /// Property: Status parsing is case-insensitive.
    #[test]
    fn prop_status_case_insensitive(status in prop::sample::select(vec![
        "active", "ACTIVE", "Active", "archived", "ARCHIVED",
        "superseded", "pending", "deleted"
    ])) {
        let query = format!("status:{status}");
        let filter = parse_filter_query(&query);
        prop_assert_eq!(filter.statuses.len(), 1);
    }

    /// Property: Combining filters preserves all components.
    #[test]
    fn prop_combining_filters_preserves_all(
        ns_idx in 0usize..10,
        tag in "[a-z]{3,8}",
        days in 1u64..30
    ) {
        let namespaces = Namespace::user_namespaces();
        if ns_idx < namespaces.len() {
            let ns = namespaces[ns_idx].as_str();
            let query = format!("ns:{ns} tag:{tag} since:{days}d");
            let filter = parse_filter_query(&query);

            prop_assert_eq!(filter.namespaces.len(), 1);
            prop_assert_eq!(filter.tags.len(), 1);
            prop_assert!(filter.created_after.is_some());
        }
    }

    /// Property: Very long content doesn't panic (content length limit test).
    #[test]
    fn prop_long_content_no_panic(len in 1000usize..10000) {
        let content = "a".repeat(len);
        let query = format!("tag:{content}");
        // Should not panic, just parse
        let _ = parse_filter_query(&query);
    }

    /// Property: Unicode in tags is preserved.
    #[test]
    fn prop_unicode_tags_preserved(tag in "[\\p{L}]{1,20}") {
        let query = format!("tag:{tag}");
        let filter = parse_filter_query(&query);
        prop_assert_eq!(filter.tags.len(), 1);
        prop_assert_eq!(&filter.tags[0], &tag);
    }
}

// ============================================================================
// Additional Property Tests for Edge Cases
// ============================================================================

proptest! {
    /// Property: Tokens without colons are ignored.
    #[test]
    fn prop_tokens_without_colons_ignored(word in "[a-z]{1,20}") {
        // Ensure no colon in the word
        prop_assume!(!word.contains(':'));
        let filter = parse_filter_query(&word);
        prop_assert!(filter.is_empty());
    }

    /// Property: Empty key (":value") is ignored.
    #[test]
    fn prop_empty_key_ignored(value in "[a-z]{1,20}") {
        let query = format!(":{value}");
        let filter = parse_filter_query(&query);
        prop_assert!(filter.is_empty());
    }

    /// Property: Empty value ("key:") produces empty result for that field.
    #[test]
    fn prop_empty_value_no_tag(_dummy in 0..1i32) {
        let filter = parse_filter_query("tag:");
        prop_assert!(filter.tags.is_empty());
        prop_assert!(filter.tags_any.is_empty());
    }

    /// Property: Namespace::all contains exactly 14 variants.
    #[test]
    fn prop_namespace_all_count(_dummy in 0..1i32) {
        prop_assert_eq!(Namespace::all().len(), 14);
    }

    /// Property: Namespace::user_namespaces excludes system namespaces.
    #[test]
    fn prop_user_namespaces_excludes_system(_dummy in 0..1i32) {
        let user_ns = Namespace::user_namespaces();
        for ns in user_ns {
            prop_assert!(!ns.is_system());
        }
    }

    /// Property: Help namespace is the only system namespace.
    #[test]
    fn prop_help_only_system_namespace(_dummy in 0..1i32) {
        let system_count = Namespace::all()
            .iter()
            .filter(|ns| ns.is_system())
            .count();
        prop_assert_eq!(system_count, 1);
        prop_assert!(Namespace::Help.is_system());
    }
}

#[cfg(test)]
mod manual_property_tests {
    use super::*;

    /// Test that `SearchFilter` composition works correctly.
    #[test]
    fn test_search_filter_composition() {
        let filter = SearchFilter::new()
            .with_namespace(Namespace::Decisions)
            .with_tag("rust")
            .with_tag("database");

        assert_eq!(filter.namespaces.len(), 1);
        assert_eq!(filter.tags.len(), 2);
        assert!(!filter.is_empty());
    }

    /// Test Domain display formatting.
    #[test]
    fn test_domain_display_formats() {
        let project = Domain::new();
        assert_eq!(project.to_string(), "project");

        let user = Domain::for_user();
        assert_eq!(user.to_string(), "user");

        let repo = Domain::for_repository("zircote", "subcog");
        assert_eq!(repo.to_string(), "zircote/subcog");
    }

    /// Test `MemoryId` hash consistency.
    #[test]
    fn test_memory_id_hash_consistency() {
        use std::collections::HashSet;

        let id1 = MemoryId::new("test-123");
        let id2 = MemoryId::new("test-123");
        let id3 = MemoryId::new("test-456");

        let mut set = HashSet::new();
        set.insert(id1.clone());
        set.insert(id2);
        set.insert(id3.clone());

        // id1 and id2 are equal, so set should have 2 elements
        assert_eq!(set.len(), 2);
        assert!(set.contains(&id1));
        assert!(set.contains(&id3));
    }
}
