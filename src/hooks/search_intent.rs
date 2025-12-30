//! Search intent detection for proactive memory surfacing.
//!
//! Detects user intent to search for information and extracts topics for memory injection.
//!
//! This module supports three detection modes:
//! - **Keyword**: Fast pattern-based detection using regex signals
//! - **LLM**: High-accuracy classification using language models (with timeout)
//! - **Hybrid**: Combined keyword + LLM detection with merged results
// Allow expect() on static regex patterns - these are guaranteed to compile
#![allow(clippy::expect_used)]

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::Duration;

use crate::Result;
use crate::config::SearchIntentConfig;
use crate::llm::LlmProvider as LlmProviderTrait;

/// Types of search intent detected from user prompts.
///
/// Each intent type corresponds to a specific information-seeking pattern
/// and has associated namespace weights for memory retrieval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SearchIntentType {
    /// "How do I...", "How to..." - seeking implementation guidance.
    HowTo,
    /// "Where is...", "Find..." - seeking file or code location.
    Location,
    /// "What is...", "What does..." - seeking explanation.
    Explanation,
    /// "Difference between...", "X vs Y" - seeking comparison.
    Comparison,
    /// "Why is...error", "...not working" - seeking troubleshooting help.
    Troubleshoot,
    /// Generic search or unclassified intent.
    #[default]
    General,
}

impl SearchIntentType {
    /// Returns all intent type variants.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::HowTo,
            Self::Location,
            Self::Explanation,
            Self::Comparison,
            Self::Troubleshoot,
            Self::General,
        ]
    }

    /// Returns the intent type as a string slice.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::HowTo => "howto",
            Self::Location => "location",
            Self::Explanation => "explanation",
            Self::Comparison => "comparison",
            Self::Troubleshoot => "troubleshoot",
            Self::General => "general",
        }
    }

    /// Returns a human-readable description of the intent type.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::HowTo => "Seeking implementation guidance or how-to instructions",
            Self::Location => "Seeking file, function, or code location",
            Self::Explanation => "Seeking explanation or definition",
            Self::Comparison => "Seeking comparison between options",
            Self::Troubleshoot => "Seeking help with error or issue",
            Self::General => "Generic search or information seeking",
        }
    }

    /// Parses an intent type from a string.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "howto" | "how_to" | "how-to" => Some(Self::HowTo),
            "location" | "locate" | "find" => Some(Self::Location),
            "explanation" | "explain" | "what" => Some(Self::Explanation),
            "comparison" | "compare" | "difference" => Some(Self::Comparison),
            "troubleshoot" | "troubleshooting" | "debug" | "error" => Some(Self::Troubleshoot),
            "general" | "search" => Some(Self::General),
            _ => None,
        }
    }
}

impl std::fmt::Display for SearchIntentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Source of search intent detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DetectionSource {
    /// Intent detected via keyword pattern matching.
    #[default]
    Keyword,
    /// Intent detected via LLM classification.
    Llm,
    /// Intent detected via both keyword and LLM (merged results).
    Hybrid,
}

impl DetectionSource {
    /// Returns the detection source as a string slice.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Keyword => "keyword",
            Self::Llm => "llm",
            Self::Hybrid => "hybrid",
        }
    }
}

impl std::fmt::Display for DetectionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Detected search intent from a user prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchIntent {
    /// The type of search intent detected.
    pub intent_type: SearchIntentType,
    /// Confidence score (0.0 to 1.0).
    pub confidence: f32,
    /// Keywords that triggered detection.
    pub keywords: Vec<String>,
    /// Extracted topics from the prompt.
    pub topics: Vec<String>,
    /// Source of the detection (keyword, llm, or hybrid).
    pub source: DetectionSource,
}

impl Default for SearchIntent {
    fn default() -> Self {
        Self {
            intent_type: SearchIntentType::General,
            confidence: 0.0,
            keywords: Vec::new(),
            topics: Vec::new(),
            source: DetectionSource::Keyword,
        }
    }
}

impl SearchIntent {
    /// Creates a new search intent.
    #[must_use]
    pub fn new(intent_type: SearchIntentType, confidence: f32) -> Self {
        Self {
            intent_type,
            confidence,
            ..Default::default()
        }
    }

    /// Sets the keywords that triggered detection.
    #[must_use]
    pub fn with_keywords(mut self, keywords: Vec<String>) -> Self {
        self.keywords = keywords;
        self
    }

    /// Sets the extracted topics.
    #[must_use]
    pub fn with_topics(mut self, topics: Vec<String>) -> Self {
        self.topics = topics;
        self
    }

    /// Sets the detection source.
    #[must_use]
    pub const fn with_source(mut self, source: DetectionSource) -> Self {
        self.source = source;
        self
    }
}

/// A search signal pattern with associated intent type.
#[derive(Debug)]
struct SearchSignal {
    /// The regex pattern to match.
    pattern: Regex,
    /// The intent type this pattern indicates.
    intent_type: SearchIntentType,
    /// Human-readable description of the signal.
    #[allow(dead_code)]
    description: &'static str,
}

/// Static search signal patterns grouped by intent type.
static SEARCH_SIGNALS: LazyLock<Vec<SearchSignal>> = LazyLock::new(|| {
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
static STOP_WORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
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

/// Detects search intent from a user prompt.
///
/// Returns `None` if no search intent is detected.
///
/// # Arguments
///
/// * `prompt` - The user prompt to analyze.
///
/// # Returns
///
/// A `SearchIntent` if search signals are detected, `None` otherwise.
#[must_use]
pub fn detect_search_intent(prompt: &str) -> Option<SearchIntent> {
    if prompt.is_empty() {
        return None;
    }

    let prompt_lower = prompt.to_lowercase();
    let mut matched_signals: Vec<(&SearchSignal, String)> = Vec::new();

    // Check each signal pattern
    for signal in SEARCH_SIGNALS.iter() {
        if let Some(matched) = signal.pattern.find(&prompt_lower) {
            matched_signals.push((signal, matched.as_str().to_string()));
        }
    }

    if matched_signals.is_empty() {
        return None;
    }

    // Determine primary intent type by counting matches
    let intent_type = determine_primary_intent(&matched_signals);

    // Extract keywords that triggered detection
    let keywords: Vec<String> = matched_signals.iter().map(|(_, m)| m.clone()).collect();

    // Calculate confidence
    let confidence = calculate_confidence(&matched_signals, prompt);

    // Extract topics from the prompt
    let topics = extract_topics(prompt);

    Some(SearchIntent {
        intent_type,
        confidence,
        keywords,
        topics,
        source: DetectionSource::Keyword,
    })
}

/// Determines the primary intent type from matched signals.
fn determine_primary_intent(matched_signals: &[(&SearchSignal, String)]) -> SearchIntentType {
    use std::collections::HashMap;

    let mut intent_counts: HashMap<SearchIntentType, usize> = HashMap::new();

    for (signal, _) in matched_signals {
        *intent_counts.entry(signal.intent_type).or_insert(0) += 1;
    }

    // Prioritize more specific intents over General
    let priority_order = [
        SearchIntentType::HowTo,
        SearchIntentType::Troubleshoot,
        SearchIntentType::Location,
        SearchIntentType::Explanation,
        SearchIntentType::Comparison,
        SearchIntentType::General,
    ];

    for intent in priority_order {
        if intent_counts.contains_key(&intent) {
            return intent;
        }
    }

    SearchIntentType::General
}

/// Calculates confidence score based on matched signals and prompt characteristics.
#[allow(clippy::cast_precision_loss)]
fn calculate_confidence(matched_signals: &[(&SearchSignal, String)], prompt: &str) -> f32 {
    let base_confidence: f32 = 0.5;

    // Bonus for multiple matches (max +0.15)
    let match_bonus = 0.15_f32.min(matched_signals.len() as f32 * 0.05);

    // Bonus for longer prompts (more context)
    let length_factor = if prompt.len() > 50 { 0.1 } else { 0.0 };

    // Bonus for multiple sentences (more structured query)
    let sentence_count = prompt
        .chars()
        .filter(|&c| c == '.' || c == '?' || c == '!')
        .count();
    let sentence_factor = if sentence_count > 1 { 0.1 } else { 0.0 };

    // Bonus for question marks (explicit question)
    let question_factor = if prompt.contains('?') { 0.1 } else { 0.0 };

    (base_confidence + match_bonus + length_factor + sentence_factor + question_factor).min(0.95)
}

/// Extracts topics from a prompt.
///
/// Topics are significant words that might map to memory tags or namespaces.
fn extract_topics(prompt: &str) -> Vec<String> {
    let mut topics = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // Simple word tokenization and filtering
    let words: Vec<&str> = prompt
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == ':')
        .filter(|w| !w.is_empty())
        .collect();

    for word in words {
        // Clean up the word
        let cleaned = word
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
            .to_lowercase();

        // Filter criteria
        if cleaned.len() < 3 {
            continue;
        }
        if STOP_WORDS.contains(cleaned.as_str()) {
            continue;
        }
        if seen.contains(&cleaned) {
            continue;
        }
        // Skip pure numbers
        if cleaned.chars().all(char::is_numeric) {
            continue;
        }

        seen.insert(cleaned.clone());
        topics.push(cleaned);
    }

    // Limit to 5 topics
    topics.truncate(5);
    topics
}

// ============================================================================
// Phase 5: LLM Intent Classification
// ============================================================================

/// Prompt template for LLM intent classification.
const LLM_INTENT_PROMPT: &str = "Classify the search intent of the following user prompt.

USER PROMPT:
<<PROMPT>>

Respond with a JSON object containing:
- \"intent_type\": one of \"howto\", \"location\", \"explanation\", \"comparison\", \"troubleshoot\", \"general\"
- \"confidence\": a float from 0.0 to 1.0
- \"topics\": array of up to 5 relevant topic strings
- \"reasoning\": brief explanation of classification

Response (JSON only):";

/// LLM classification result structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmIntentResponse {
    intent_type: String,
    confidence: f32,
    #[serde(default)]
    topics: Vec<String>,
    #[serde(default)]
    reasoning: String,
}

/// Classifies search intent using an LLM provider.
///
/// # Arguments
///
/// * `provider` - The LLM provider to use for classification.
/// * `prompt` - The user prompt to classify.
///
/// # Returns
///
/// A `SearchIntent` with LLM classification results.
///
/// # Errors
///
/// Returns an error if the LLM call fails or response parsing fails.
pub fn classify_intent_with_llm<P: LlmProviderTrait + ?Sized>(
    provider: &P,
    prompt: &str,
) -> Result<SearchIntent> {
    let classification_prompt = LLM_INTENT_PROMPT.replace("<<PROMPT>>", prompt);
    let response = provider.complete(&classification_prompt)?;
    parse_llm_response(&response)
}

/// Parses LLM response into a `SearchIntent`.
fn parse_llm_response(response: &str) -> Result<SearchIntent> {
    // Try to extract JSON from response (handle markdown code blocks)
    let json_str = extract_json_from_response(response);

    let parsed: LlmIntentResponse =
        serde_json::from_str(json_str).map_err(|e| crate::Error::OperationFailed {
            operation: "parse_llm_intent_response".to_string(),
            cause: format!("Invalid JSON: {e}"),
        })?;

    let intent_type =
        SearchIntentType::parse(&parsed.intent_type).unwrap_or(SearchIntentType::General);

    Ok(SearchIntent {
        intent_type,
        confidence: parsed.confidence.clamp(0.0, 1.0),
        keywords: Vec::new(), // LLM doesn't provide keywords
        topics: parsed.topics,
        source: DetectionSource::Llm,
    })
}

/// Extracts JSON from LLM response, handling markdown code blocks.
fn extract_json_from_response(response: &str) -> &str {
    let trimmed = response.trim();

    // Handle ```json ... ``` blocks
    if let Some(start) = trimmed.find("```json") {
        let json_start = start + 7;
        if let Some(end) = trimmed[json_start..].find("```") {
            return trimmed[json_start..json_start + end].trim();
        }
    }

    // Handle ``` ... ``` blocks (without json marker)
    if let Some(start) = trimmed.find("```") {
        let content_start = start + 3;
        // Skip language identifier if present (e.g., "json\n")
        let after_marker = &trimmed[content_start..];
        let json_start = after_marker
            .find('{')
            .map_or(content_start, |pos| content_start + pos);
        if let Some(end) = trimmed[json_start..].find("```") {
            return trimmed[json_start..json_start + end].trim();
        }
    }

    // Handle raw JSON (find first { to last })
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            return &trimmed[start..=end];
        }
    }

    trimmed
}

/// Detects search intent with LLM classification and timeout.
///
/// Uses LLM classification with a configurable timeout. Falls back to keyword
/// detection if LLM times out or fails.
///
/// # Arguments
///
/// * `provider` - Optional LLM provider. If None, uses keyword-only detection.
/// * `prompt` - The user prompt to analyze.
/// * `config` - Configuration for intent detection.
///
/// # Returns
///
/// A `SearchIntent` from either LLM or keyword detection.
///
/// # Panics
///
/// This function does not panic. The internal `expect` is guarded by a prior
/// `is_none()` check and will never be reached when provider is None.
#[must_use]
pub fn detect_search_intent_with_timeout<P: LlmProviderTrait + ?Sized>(
    provider: Option<&P>,
    prompt: &str,
    config: &SearchIntentConfig,
) -> SearchIntent {
    // If LLM is disabled or no provider, use keyword detection
    if !config.use_llm || provider.is_none() {
        return detect_search_intent(prompt).unwrap_or_default();
    }

    let provider = provider.expect("provider checked above");
    let timeout = Duration::from_millis(config.llm_timeout_ms);

    // Try LLM classification with timeout
    let prompt_owned = prompt.to_string();
    let llm_result = std::thread::scope(|s| {
        let handle = s.spawn(|| classify_intent_with_llm(provider, &prompt_owned));

        // Wait with timeout using a simple polling approach
        let start = std::time::Instant::now();
        loop {
            if handle.is_finished() {
                return handle.join().ok().and_then(std::result::Result::ok);
            }
            if start.elapsed() >= timeout {
                return None; // Timeout
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    // Return LLM result if successful, otherwise fall back to keyword
    llm_result.unwrap_or_else(|| detect_search_intent(prompt).unwrap_or_default())
}

/// Detects search intent using hybrid keyword + LLM detection.
///
/// Runs keyword detection immediately and LLM detection in parallel.
/// Merges results with LLM taking precedence for intent type if confidence is high.
///
/// # Arguments
///
/// * `provider` - Optional LLM provider.
/// * `prompt` - The user prompt to analyze.
/// * `config` - Configuration for intent detection.
///
/// # Returns
///
/// A merged `SearchIntent` from both detection methods.
///
/// # Panics
///
/// This function does not panic. The internal `expect` is guarded by a prior
/// `is_none()` check and will never be reached when provider is None.
#[must_use]
pub fn detect_search_intent_hybrid<P: LlmProviderTrait + ?Sized>(
    provider: Option<&P>,
    prompt: &str,
    config: &SearchIntentConfig,
) -> SearchIntent {
    // Always run keyword detection (fast)
    let keyword_result = detect_search_intent(prompt);

    // If LLM is disabled or no provider, return keyword result
    if !config.use_llm || provider.is_none() {
        return keyword_result.unwrap_or_default();
    }

    let provider = provider.expect("provider checked above");
    let timeout = Duration::from_millis(config.llm_timeout_ms);

    // Try LLM classification with timeout
    let prompt_owned = prompt.to_string();
    let llm_result = std::thread::scope(|s| {
        let handle = s.spawn(|| classify_intent_with_llm(provider, &prompt_owned));

        let start = std::time::Instant::now();
        loop {
            if handle.is_finished() {
                return handle.join().ok().and_then(std::result::Result::ok);
            }
            if start.elapsed() >= timeout {
                return None;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    // Merge results
    merge_intent_results(keyword_result, llm_result, config)
}

/// Merges keyword and LLM intent results.
fn merge_intent_results(
    keyword: Option<SearchIntent>,
    llm: Option<SearchIntent>,
    config: &SearchIntentConfig,
) -> SearchIntent {
    match (keyword, llm) {
        // Both available: prefer LLM if high confidence
        (Some(kw), Some(llm_intent)) => {
            if llm_intent.confidence >= config.min_confidence {
                SearchIntent {
                    intent_type: llm_intent.intent_type,
                    // Average confidence weighted toward LLM
                    confidence: llm_intent
                        .confidence
                        .mul_add(0.7, kw.confidence * 0.3)
                        .min(0.95),
                    // Combine keywords from keyword detection
                    keywords: kw.keywords,
                    // Prefer LLM topics if available, otherwise keyword topics
                    topics: if llm_intent.topics.is_empty() {
                        kw.topics
                    } else {
                        llm_intent.topics
                    },
                    source: DetectionSource::Hybrid,
                }
            } else {
                // LLM confidence too low, use keyword result
                SearchIntent {
                    source: DetectionSource::Hybrid,
                    ..kw
                }
            }
        },
        // Only keyword available
        (Some(kw), None) => kw,
        // Only LLM available (unusual but possible)
        (None, Some(llm_intent)) => SearchIntent {
            source: DetectionSource::Llm,
            ..llm_intent
        },
        // Neither available
        (None, None) => SearchIntent::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Task 1.9: Unit Tests for Intent Type Detection

    #[test]
    fn test_intent_type_howto() {
        let intent = detect_search_intent("How do I implement authentication?");
        assert!(intent.is_some());
        let intent = intent.unwrap();
        assert_eq!(intent.intent_type, SearchIntentType::HowTo);
        assert!(intent.confidence >= 0.5);
    }

    #[test]
    fn test_intent_type_howto_variations() {
        let prompts = [
            "how can I create a new module?",
            "how should we structure the code?",
            "how to configure the database?",
            "steps to deploy the application",
            "guide me through the setup",
        ];

        for prompt in prompts {
            let intent = detect_search_intent(prompt);
            assert!(intent.is_some(), "Failed for: {prompt}");
            assert_eq!(
                intent.unwrap().intent_type,
                SearchIntentType::HowTo,
                "Failed for: {prompt}"
            );
        }
    }

    #[test]
    fn test_intent_type_location() {
        let intent = detect_search_intent("Where is the database configuration?");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().intent_type, SearchIntentType::Location);
    }

    #[test]
    fn test_intent_type_location_variations() {
        let prompts = [
            "where can I find the config file?",
            "find the authentication module",
            "locate the user model",
            "which file handles routing?",
            "look for the database schema",
        ];

        for prompt in prompts {
            let intent = detect_search_intent(prompt);
            assert!(intent.is_some(), "Failed for: {prompt}");
            assert_eq!(
                intent.unwrap().intent_type,
                SearchIntentType::Location,
                "Failed for: {prompt}"
            );
        }
    }

    #[test]
    fn test_intent_type_explanation() {
        let intent = detect_search_intent("What is the purpose of the ServiceContainer?");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().intent_type, SearchIntentType::Explanation);
    }

    #[test]
    fn test_intent_type_explanation_variations() {
        let prompts = [
            "what does this function do?",
            "explain the architecture",
            "tell me about the search system",
            "help me understand the flow",
            "what's the role of the middleware?",
        ];

        for prompt in prompts {
            let intent = detect_search_intent(prompt);
            assert!(intent.is_some(), "Failed for: {prompt}");
            assert_eq!(
                intent.unwrap().intent_type,
                SearchIntentType::Explanation,
                "Failed for: {prompt}"
            );
        }
    }

    #[test]
    fn test_intent_type_comparison() {
        let intent = detect_search_intent("What's the difference between git notes and SQLite?");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().intent_type, SearchIntentType::Comparison);
    }

    #[test]
    fn test_intent_type_comparison_variations() {
        let prompts = [
            "compare PostgreSQL vs SQLite",
            "which is better: tokio or async-std?",
            "pros and cons of microservices",
            "advantages of using Rust",
        ];

        for prompt in prompts {
            let intent = detect_search_intent(prompt);
            assert!(intent.is_some(), "Failed for: {prompt}");
            assert_eq!(
                intent.unwrap().intent_type,
                SearchIntentType::Comparison,
                "Failed for: {prompt}"
            );
        }
    }

    #[test]
    fn test_intent_type_troubleshoot() {
        let intent =
            detect_search_intent("Why is the authentication failing with this error message?");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().intent_type, SearchIntentType::Troubleshoot);
    }

    #[test]
    fn test_intent_type_troubleshoot_variations() {
        let prompts = [
            "getting an error when compiling",
            "the tests are not working",
            "fix the bug in the parser",
            "solve the connection issue",
            "debug the authentication problem",
            "this crash keeps happening every time",
        ];

        for prompt in prompts {
            let intent = detect_search_intent(prompt);
            assert!(intent.is_some(), "Failed for: {prompt}");
            assert_eq!(
                intent.unwrap().intent_type,
                SearchIntentType::Troubleshoot,
                "Failed for: {prompt}"
            );
        }
    }

    #[test]
    fn test_intent_type_general() {
        let intent = detect_search_intent("search for memory implementations");
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().intent_type, SearchIntentType::General);
    }

    #[test]
    fn test_no_intent_for_generic_text() {
        // Plain statements without search indicators
        let intent = detect_search_intent("Hello, I want to work on this project.");
        // May or may not detect intent depending on patterns
        if let Some(i) = intent {
            // If detected, should be low confidence
            assert!(i.confidence < 0.7);
        }
    }

    #[test]
    fn test_empty_prompt() {
        let intent = detect_search_intent("");
        assert!(intent.is_none());
    }

    // Task 1.10: Unit Tests for Confidence Calculation

    #[test]
    fn test_confidence_single_match() {
        let intent = detect_search_intent("how to");
        assert!(intent.is_some());
        let confidence = intent.unwrap().confidence;
        // Single short match should have base confidence around 0.5
        assert!(confidence >= 0.5);
        assert!(confidence < 0.7);
    }

    #[test]
    fn test_confidence_multiple_matches() {
        // This prompt should match multiple patterns
        let intent = detect_search_intent(
            "How do I implement authentication? Guide me through the steps to create a secure login.",
        );
        assert!(intent.is_some());
        let confidence = intent.unwrap().confidence;
        // Multiple matches + long prompt + punctuation
        assert!(confidence >= 0.7);
    }

    #[test]
    fn test_confidence_long_prompt_bonus() {
        let short = detect_search_intent("how to do it");
        let long =
            detect_search_intent("how to do it with a much longer prompt that provides context");

        assert!(short.is_some());
        assert!(long.is_some());

        let short_conf = short.unwrap().confidence;
        let long_conf = long.unwrap().confidence;
        assert!(long_conf >= short_conf);
    }

    #[test]
    fn test_confidence_question_mark_bonus() {
        let without = detect_search_intent("how to implement this");
        let with = detect_search_intent("how to implement this?");

        assert!(without.is_some());
        assert!(with.is_some());

        let without_conf = without.unwrap().confidence;
        let with_conf = with.unwrap().confidence;
        assert!(with_conf >= without_conf);
    }

    #[test]
    fn test_confidence_max_cap() {
        // Even with many signals, confidence should cap at 0.95
        let intent = detect_search_intent(
            "How do I implement and fix the authentication error? \
             What is the issue? Where is the config? Compare options. \
             Guide me through the steps to solve this problem.",
        );
        assert!(intent.is_some());
        assert!(intent.unwrap().confidence <= 0.95);
    }

    // Task 1.11: Unit Tests for Topic Extraction

    #[test]
    fn test_topic_extraction_basic() {
        let intent = detect_search_intent("how do I implement authentication?");
        assert!(intent.is_some());
        let topics = intent.unwrap().topics;
        assert!(!topics.is_empty());
        assert!(
            topics
                .iter()
                .any(|t| t.contains("implement") || t.contains("authentication"))
        );
    }

    #[test]
    fn test_topic_extraction_database_config() {
        let intent = detect_search_intent("where is the database config?");
        assert!(intent.is_some());
        let topics = intent.unwrap().topics;
        assert!(topics.contains(&"database".to_string()) || topics.contains(&"config".to_string()));
    }

    #[test]
    fn test_topic_extraction_filters_stop_words() {
        let intent = detect_search_intent("what is the purpose of the system?");
        assert!(intent.is_some());
        let topics = intent.unwrap().topics;
        // Should not contain common stop words
        assert!(!topics.contains(&"the".to_string()));
        assert!(!topics.contains(&"of".to_string()));
        assert!(!topics.contains(&"is".to_string()));
    }

    #[test]
    fn test_topic_extraction_max_limit() {
        let intent = detect_search_intent(
            "search for authentication, authorization, database, config, \
             models, services, handlers, middleware, routing, security",
        );
        assert!(intent.is_some());
        let topics = intent.unwrap().topics;
        // Should limit to 5 topics
        assert!(topics.len() <= 5);
    }

    #[test]
    fn test_topic_extraction_empty() {
        // Very short prompt might not yield topics
        let intent = detect_search_intent("how to?");
        if let Some(i) = intent {
            // Topics might be empty for very short prompts
            assert!(i.topics.len() <= 5);
        }
    }

    // SearchIntentType tests

    #[test]
    fn test_intent_type_as_str() {
        assert_eq!(SearchIntentType::HowTo.as_str(), "howto");
        assert_eq!(SearchIntentType::Location.as_str(), "location");
        assert_eq!(SearchIntentType::Explanation.as_str(), "explanation");
        assert_eq!(SearchIntentType::Comparison.as_str(), "comparison");
        assert_eq!(SearchIntentType::Troubleshoot.as_str(), "troubleshoot");
        assert_eq!(SearchIntentType::General.as_str(), "general");
    }

    #[test]
    fn test_intent_type_parse() {
        assert_eq!(
            SearchIntentType::parse("howto"),
            Some(SearchIntentType::HowTo)
        );
        assert_eq!(
            SearchIntentType::parse("how-to"),
            Some(SearchIntentType::HowTo)
        );
        assert_eq!(
            SearchIntentType::parse("LOCATION"),
            Some(SearchIntentType::Location)
        );
        assert_eq!(SearchIntentType::parse("unknown"), None);
    }

    #[test]
    fn test_intent_type_all() {
        let all = SearchIntentType::all();
        assert_eq!(all.len(), 6);
        assert!(all.contains(&SearchIntentType::HowTo));
        assert!(all.contains(&SearchIntentType::General));
    }

    #[test]
    fn test_detection_source_as_str() {
        assert_eq!(DetectionSource::Keyword.as_str(), "keyword");
        assert_eq!(DetectionSource::Llm.as_str(), "llm");
        assert_eq!(DetectionSource::Hybrid.as_str(), "hybrid");
    }

    #[test]
    fn test_search_intent_builder() {
        let intent = SearchIntent::new(SearchIntentType::HowTo, 0.8)
            .with_keywords(vec!["how to".to_string()])
            .with_topics(vec!["authentication".to_string()])
            .with_source(DetectionSource::Hybrid);

        assert_eq!(intent.intent_type, SearchIntentType::HowTo);
        assert!((intent.confidence - 0.8).abs() < f32::EPSILON);
        assert_eq!(intent.keywords, vec!["how to".to_string()]);
        assert_eq!(intent.topics, vec!["authentication".to_string()]);
        assert_eq!(intent.source, DetectionSource::Hybrid);
    }

    #[test]
    fn test_search_intent_default() {
        let intent = SearchIntent::default();
        assert_eq!(intent.intent_type, SearchIntentType::General);
        assert!(intent.confidence.abs() < f32::EPSILON);
        assert!(intent.keywords.is_empty());
        assert!(intent.topics.is_empty());
        assert_eq!(intent.source, DetectionSource::Keyword);
    }

    // ========================================================================
    // Phase 5: LLM Intent Classification Tests
    // ========================================================================

    // Task 5.9: Tests for LLM Response Parsing

    #[test]
    fn test_parse_llm_response_valid_json() {
        let response = r#"{"intent_type": "howto", "confidence": 0.85, "topics": ["authentication", "login"], "reasoning": "User is asking how to implement"}"#;
        let intent = parse_llm_response(response).unwrap();
        assert_eq!(intent.intent_type, SearchIntentType::HowTo);
        assert!((intent.confidence - 0.85).abs() < f32::EPSILON);
        assert_eq!(intent.topics, vec!["authentication", "login"]);
        assert_eq!(intent.source, DetectionSource::Llm);
    }

    #[test]
    fn test_parse_llm_response_markdown_code_block() {
        let response = r#"```json
{"intent_type": "location", "confidence": 0.9, "topics": ["config", "database"]}
```"#;
        let intent = parse_llm_response(response).unwrap();
        assert_eq!(intent.intent_type, SearchIntentType::Location);
        assert!((intent.confidence - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_llm_response_code_block_without_json() {
        let response = r#"```
{"intent_type": "explanation", "confidence": 0.75, "topics": []}
```"#;
        let intent = parse_llm_response(response).unwrap();
        assert_eq!(intent.intent_type, SearchIntentType::Explanation);
    }

    #[test]
    fn test_parse_llm_response_with_prefix_text() {
        let response = r#"Here is the classification:
{"intent_type": "troubleshoot", "confidence": 0.8, "topics": ["error"], "reasoning": "User has an issue"}"#;
        let intent = parse_llm_response(response).unwrap();
        assert_eq!(intent.intent_type, SearchIntentType::Troubleshoot);
    }

    #[test]
    fn test_parse_llm_response_missing_optional_fields() {
        let response = r#"{"intent_type": "general", "confidence": 0.5}"#;
        let intent = parse_llm_response(response).unwrap();
        assert_eq!(intent.intent_type, SearchIntentType::General);
        assert!(intent.topics.is_empty());
    }

    #[test]
    fn test_parse_llm_response_clamps_confidence() {
        let high = r#"{"intent_type": "howto", "confidence": 1.5, "topics": []}"#;
        let intent = parse_llm_response(high).unwrap();
        assert!(intent.confidence <= 1.0);

        let low = r#"{"intent_type": "howto", "confidence": -0.5, "topics": []}"#;
        let intent = parse_llm_response(low).unwrap();
        assert!(intent.confidence >= 0.0);
    }

    #[test]
    fn test_parse_llm_response_unknown_intent_type() {
        let response = r#"{"intent_type": "unknown_type", "confidence": 0.7, "topics": []}"#;
        let intent = parse_llm_response(response).unwrap();
        // Unknown types default to General
        assert_eq!(intent.intent_type, SearchIntentType::General);
    }

    #[test]
    fn test_parse_llm_response_invalid_json() {
        let response = "not valid json";
        let result = parse_llm_response(response);
        assert!(result.is_err());
    }

    // Task 5.10: Tests for JSON Extraction

    #[test]
    fn test_extract_json_from_response_raw() {
        let response = r#"{"key": "value"}"#;
        let json = extract_json_from_response(response);
        assert_eq!(json, r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_from_response_with_prefix() {
        let response = r#"Here is the result: {"key": "value"} hope this helps"#;
        let json = extract_json_from_response(response);
        assert_eq!(json, r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_from_response_markdown_json() {
        let response = "```json\n{\"key\": \"value\"}\n```";
        let json = extract_json_from_response(response);
        assert!(json.contains("\"key\""));
    }

    #[test]
    fn test_extract_json_from_response_markdown_plain() {
        let response = "```\n{\"key\": \"value\"}\n```";
        let json = extract_json_from_response(response);
        assert!(json.contains("\"key\""));
    }

    // Task 5.11: Tests for Timeout and Fallback

    // Mock LLM provider for testing
    struct MockLlmProvider {
        response: String,
        delay_ms: u64,
    }

    impl MockLlmProvider {
        fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
                delay_ms: 0,
            }
        }

        fn with_delay(response: &str, delay_ms: u64) -> Self {
            Self {
                response: response.to_string(),
                delay_ms,
            }
        }
    }

    impl LlmProviderTrait for MockLlmProvider {
        fn name(&self) -> &'static str {
            "mock"
        }

        fn complete(&self, _prompt: &str) -> crate::Result<String> {
            if self.delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(self.delay_ms));
            }
            Ok(self.response.clone())
        }

        fn analyze_for_capture(
            &self,
            _content: &str,
        ) -> crate::Result<crate::llm::CaptureAnalysis> {
            Ok(crate::llm::CaptureAnalysis {
                should_capture: false,
                confidence: 0.0,
                suggested_namespace: None,
                suggested_tags: Vec::new(),
                reasoning: String::new(),
            })
        }
    }

    #[test]
    fn test_classify_intent_with_llm_success() {
        let provider = MockLlmProvider::new(
            r#"{"intent_type": "howto", "confidence": 0.9, "topics": ["auth"]}"#,
        );
        let intent = classify_intent_with_llm(&provider, "How do I implement auth?").unwrap();
        assert_eq!(intent.intent_type, SearchIntentType::HowTo);
        assert_eq!(intent.source, DetectionSource::Llm);
    }

    #[test]
    fn test_detect_search_intent_with_timeout_llm_disabled() {
        let config = SearchIntentConfig::default().with_use_llm(false);
        let provider = MockLlmProvider::new(r#"{"intent_type": "howto", "confidence": 0.9}"#);

        let intent =
            detect_search_intent_with_timeout(Some(&provider), "how to implement?", &config);
        // Should use keyword detection, not LLM
        assert_eq!(intent.source, DetectionSource::Keyword);
    }

    #[test]
    fn test_detect_search_intent_with_timeout_no_provider() {
        let config = SearchIntentConfig::default();
        let intent = detect_search_intent_with_timeout::<MockLlmProvider>(
            None,
            "how to implement?",
            &config,
        );
        // Should use keyword detection
        assert_eq!(intent.source, DetectionSource::Keyword);
        assert_eq!(intent.intent_type, SearchIntentType::HowTo);
    }

    #[test]
    fn test_detect_search_intent_with_timeout_llm_success() {
        let config = SearchIntentConfig::default().with_llm_timeout_ms(1000);
        let provider = MockLlmProvider::new(
            r#"{"intent_type": "location", "confidence": 0.85, "topics": ["config"]}"#,
        );

        let intent =
            detect_search_intent_with_timeout(Some(&provider), "where is the config?", &config);
        assert_eq!(intent.intent_type, SearchIntentType::Location);
        assert_eq!(intent.source, DetectionSource::Llm);
    }

    #[test]
    fn test_detect_search_intent_with_timeout_llm_timeout() {
        // Very short timeout with slow provider
        let config = SearchIntentConfig::default().with_llm_timeout_ms(10);
        let provider = MockLlmProvider::with_delay(
            r#"{"intent_type": "location", "confidence": 0.85}"#,
            500, // Provider takes 500ms
        );

        let intent =
            detect_search_intent_with_timeout(Some(&provider), "where is the config?", &config);
        // Should fall back to keyword detection due to timeout
        assert_eq!(intent.source, DetectionSource::Keyword);
    }

    // Task 5.12: Tests for Hybrid Detection

    #[test]
    fn test_detect_search_intent_hybrid_both_available() {
        let config = SearchIntentConfig::default()
            .with_llm_timeout_ms(1000)
            .with_min_confidence(0.5);
        let provider = MockLlmProvider::new(
            r#"{"intent_type": "troubleshoot", "confidence": 0.9, "topics": ["database", "connection"]}"#,
        );

        // Keyword would detect "error" as Troubleshoot
        let intent =
            detect_search_intent_hybrid(Some(&provider), "error connecting to database", &config);
        assert_eq!(intent.intent_type, SearchIntentType::Troubleshoot);
        assert_eq!(intent.source, DetectionSource::Hybrid);
        // Should use LLM topics since confidence is high
        assert!(intent.topics.contains(&"database".to_string()));
    }

    #[test]
    fn test_detect_search_intent_hybrid_llm_low_confidence() {
        let config = SearchIntentConfig::default()
            .with_llm_timeout_ms(1000)
            .with_min_confidence(0.8); // High threshold
        let provider = MockLlmProvider::new(
            r#"{"intent_type": "comparison", "confidence": 0.5, "topics": ["option1"]}"#,
        );

        // Keyword detects "how to" as HowTo
        let intent =
            detect_search_intent_hybrid(Some(&provider), "how to implement this?", &config);
        // Should use keyword intent type since LLM confidence is below threshold
        assert_eq!(intent.intent_type, SearchIntentType::HowTo);
        assert_eq!(intent.source, DetectionSource::Hybrid);
    }

    #[test]
    fn test_detect_search_intent_hybrid_only_keyword() {
        let config = SearchIntentConfig::default().with_use_llm(false);
        let provider = MockLlmProvider::new(r#"{"intent_type": "howto", "confidence": 0.9}"#);

        let intent = detect_search_intent_hybrid(Some(&provider), "how to implement?", &config);
        // LLM disabled, use keyword only
        assert_eq!(intent.source, DetectionSource::Keyword);
    }

    #[test]
    fn test_merge_intent_results_both_available_high_confidence() {
        let config = SearchIntentConfig::default().with_min_confidence(0.5);

        let keyword = Some(SearchIntent {
            intent_type: SearchIntentType::HowTo,
            confidence: 0.7,
            keywords: vec!["how to".to_string()],
            topics: vec!["auth".to_string()],
            source: DetectionSource::Keyword,
        });

        let llm = Some(SearchIntent {
            intent_type: SearchIntentType::Location,
            confidence: 0.9,
            keywords: Vec::new(),
            topics: vec!["config".to_string(), "settings".to_string()],
            source: DetectionSource::Llm,
        });

        let merged = merge_intent_results(keyword, llm, &config);
        // LLM intent type preferred due to high confidence
        assert_eq!(merged.intent_type, SearchIntentType::Location);
        // Keywords from keyword detection
        assert_eq!(merged.keywords, vec!["how to".to_string()]);
        // Topics from LLM
        assert!(merged.topics.contains(&"config".to_string()));
        assert_eq!(merged.source, DetectionSource::Hybrid);
    }

    #[test]
    fn test_merge_intent_results_only_keyword() {
        let config = SearchIntentConfig::default();

        let keyword = Some(SearchIntent::new(SearchIntentType::HowTo, 0.7));
        let merged = merge_intent_results(keyword, None, &config);
        assert_eq!(merged.intent_type, SearchIntentType::HowTo);
        assert_eq!(merged.source, DetectionSource::Keyword);
    }

    #[test]
    fn test_merge_intent_results_only_llm() {
        let config = SearchIntentConfig::default();

        let llm = Some(SearchIntent {
            intent_type: SearchIntentType::Explanation,
            confidence: 0.8,
            keywords: Vec::new(),
            topics: vec!["concept".to_string()],
            source: DetectionSource::Llm,
        });

        let merged = merge_intent_results(None, llm, &config);
        assert_eq!(merged.intent_type, SearchIntentType::Explanation);
        assert_eq!(merged.source, DetectionSource::Llm);
    }

    #[test]
    fn test_merge_intent_results_neither_available() {
        let config = SearchIntentConfig::default();
        let merged = merge_intent_results(None, None, &config);
        assert_eq!(merged.intent_type, SearchIntentType::General);
        assert!(merged.confidence.abs() < f32::EPSILON);
    }

    #[test]
    fn test_search_intent_config_from_env() {
        // Test that from_env works (without setting env vars, should return defaults)
        let config = SearchIntentConfig::from_env();
        assert!(config.enabled);
        assert!(config.use_llm);
        assert_eq!(config.llm_timeout_ms, 200);
        assert!((config.min_confidence - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_search_intent_config_builder() {
        let config = SearchIntentConfig::new()
            .with_use_llm(false)
            .with_llm_timeout_ms(500)
            .with_min_confidence(0.7);

        assert!(!config.use_llm);
        assert_eq!(config.llm_timeout_ms, 500);
        assert!((config.min_confidence - 0.7).abs() < f32::EPSILON);
    }
}
