//! Search intent detection patterns.
//!
//! Static pattern data for keyword-based search intent detection.
//! Extracted from `search_intent.rs` to reduce file size.
// Allow expect() on static regex patterns - these are guaranteed to compile
#![allow(clippy::expect_used)]

use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

use super::search_intent::SearchIntentType;

/// A search signal pattern with associated intent type.
#[derive(Debug)]
pub struct SearchSignal {
    /// The regex pattern to match.
    pub pattern: Regex,
    /// The intent type this pattern indicates.
    pub intent_type: SearchIntentType,
    /// Human-readable description of the signal.
    #[allow(dead_code)]
    pub description: &'static str,
}

/// Static search signal patterns grouped by intent type.
pub static SEARCH_SIGNALS: LazyLock<Vec<SearchSignal>> = LazyLock::new(|| {
    vec![
        // HowTo patterns
        SearchSignal {
            pattern: Regex::new(r"(?i)\bhow\s+(do|can|should|would)\s+(i|we|you)\b")
                .expect("static regex: how do I"),
            intent_type: SearchIntentType::HowTo,
            description: "how do I/we/you",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\bhow\s+to\b").expect("static regex: how to"),
            intent_type: SearchIntentType::HowTo,
            description: "how to",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\b(implement|create|build|make|add|write)\s+a?\b")
                .expect("static regex: implement/create"),
            intent_type: SearchIntentType::HowTo,
            description: "implement/create/build",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\bsteps?\s+(to|for)\b").expect("static regex: steps to"),
            intent_type: SearchIntentType::HowTo,
            description: "steps to/for",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\bguide\s+(me|us|to)\b").expect("static regex: guide me"),
            intent_type: SearchIntentType::HowTo,
            description: "guide me/us/to",
        },
        // Location patterns
        SearchSignal {
            pattern: Regex::new(r"(?i)\bwhere\s+(is|are|can\s+i\s+find)\b")
                .expect("static regex: where is"),
            intent_type: SearchIntentType::Location,
            description: "where is/are",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\b(find|locate|show\s+me)\s+(the|a)?\b")
                .expect("static regex: find/locate"),
            intent_type: SearchIntentType::Location,
            description: "find/locate/show me",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\b(which|what)\s+file\b").expect("static regex: which file"),
            intent_type: SearchIntentType::Location,
            description: "which/what file",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\blook\s+(for|at|up)\b").expect("static regex: look for"),
            intent_type: SearchIntentType::Location,
            description: "look for/at/up",
        },
        // Explanation patterns
        SearchSignal {
            pattern: Regex::new(r"(?i)\bwhat\s+(is|are|does)\b").expect("static regex: what is"),
            intent_type: SearchIntentType::Explanation,
            description: "what is/are/does",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\bexplain\b").expect("static regex: explain"),
            intent_type: SearchIntentType::Explanation,
            description: "explain",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\b(tell|help)\s+me\s+(about|understand)\b")
                .expect("static regex: tell me about"),
            intent_type: SearchIntentType::Explanation,
            description: "tell me about/understand",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\bwhat('s|\s+is)\s+the\s+(purpose|meaning|role)\b")
                .expect("static regex: what's the purpose"),
            intent_type: SearchIntentType::Explanation,
            description: "what's the purpose/meaning/role",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\bcan\s+you\s+describe\b")
                .expect("static regex: can you describe"),
            intent_type: SearchIntentType::Explanation,
            description: "can you describe",
        },
        // Comparison patterns
        SearchSignal {
            pattern: Regex::new(r"(?i)\bdifference\s+between\b")
                .expect("static regex: difference between"),
            intent_type: SearchIntentType::Comparison,
            description: "difference between",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\b(compare|vs\.?|versus)\b").expect("static regex: compare"),
            intent_type: SearchIntentType::Comparison,
            description: "compare/vs/versus",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\bwhich\s+(is|one|should)\s+(better|best|prefer)\b")
                .expect("static regex: which is better"),
            intent_type: SearchIntentType::Comparison,
            description: "which is better",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\b(pros|cons|advantages|disadvantages)\b")
                .expect("static regex: pros/cons"),
            intent_type: SearchIntentType::Comparison,
            description: "pros/cons/advantages/disadvantages",
        },
        // Troubleshoot patterns
        SearchSignal {
            pattern: Regex::new(r"(?i)\bwhy\s+(is|does|am|are)\b.*\b(error|fail|wrong|issue)\b")
                .expect("static regex: why is error"),
            intent_type: SearchIntentType::Troubleshoot,
            description: "why is/does...error",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\b(error|exception|failure|crash|bug)\b")
                .expect("static regex: error/exception"),
            intent_type: SearchIntentType::Troubleshoot,
            description: "error/exception/failure/crash/bug",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\b(not\s+working|doesn't\s+work|won't\s+work|broken)\b")
                .expect("static regex: not working"),
            intent_type: SearchIntentType::Troubleshoot,
            description: "not working/doesn't work/broken",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\b(fix|solve|resolve|debug)\b")
                .expect("static regex: fix/solve"),
            intent_type: SearchIntentType::Troubleshoot,
            description: "fix/solve/resolve/debug",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\b(issue|problem)\s+with\b")
                .expect("static regex: issue with"),
            intent_type: SearchIntentType::Troubleshoot,
            description: "issue/problem with",
        },
        // General patterns
        SearchSignal {
            pattern: Regex::new(r"(?i)\b(search|find|lookup|query)\b")
                .expect("static regex: search/find"),
            intent_type: SearchIntentType::General,
            description: "search/find/lookup",
        },
        SearchSignal {
            pattern: Regex::new(r"(?i)\bshow\s+(me|us)\b").expect("static regex: show me"),
            intent_type: SearchIntentType::General,
            description: "show me/us",
        },
    ]
});

/// Common stop words to filter from topic extraction.
pub static STOP_WORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "a",
        "an",
        "the",
        "and",
        "or",
        "but",
        "in",
        "on",
        "at",
        "to",
        "for",
        "of",
        "with",
        "by",
        "from",
        "as",
        "is",
        "was",
        "are",
        "were",
        "been",
        "be",
        "have",
        "has",
        "had",
        "do",
        "does",
        "did",
        "will",
        "would",
        "could",
        "should",
        "may",
        "might",
        "must",
        "shall",
        "can",
        "need",
        "i",
        "you",
        "he",
        "she",
        "it",
        "we",
        "they",
        "me",
        "him",
        "her",
        "us",
        "them",
        "my",
        "your",
        "his",
        "its",
        "our",
        "their",
        "this",
        "that",
        "these",
        "those",
        "what",
        "which",
        "who",
        "whom",
        "how",
        "when",
        "where",
        "why",
        "all",
        "each",
        "every",
        "both",
        "few",
        "more",
        "most",
        "other",
        "some",
        "such",
        "no",
        "nor",
        "not",
        "only",
        "own",
        "same",
        "so",
        "than",
        "too",
        "very",
        "just",
        "about",
        "also",
        "now",
        "here",
        "there",
        "up",
        "down",
        "out",
        "if",
        "then",
        "into",
        "through",
        "during",
        "before",
        "after",
        "above",
        "below",
        "between",
        "under",
        "again",
        "further",
        "once",
        "any",
        "something",
        "anything",
        "nothing",
    ]
    .into_iter()
    .collect()
});
