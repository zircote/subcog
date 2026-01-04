//! Search context builder for adaptive memory injection.
//!
//! Builds memory context based on detected search intent for proactive surfacing.

use crate::Result;
use crate::config::{NamespaceWeightsConfig, SearchIntentConfig};
use crate::hooks::search_intent::{SearchIntent, SearchIntentType};
use crate::models::{Namespace, SearchFilter, SearchMode};
use crate::services::{ContextBuilderService, RecallService};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for adaptive memory context injection.
#[derive(Debug, Clone)]
pub struct AdaptiveContextConfig {
    /// Base number of memories to retrieve.
    pub base_count: usize,
    /// Maximum number of memories to retrieve (high confidence).
    pub max_count: usize,
    /// Maximum tokens for injected memory content.
    pub max_tokens: usize,
    /// Maximum length for content preview.
    pub preview_length: usize,
    /// Minimum confidence threshold for injection.
    pub min_confidence: f32,
    /// Namespace weights configuration.
    pub weights: NamespaceWeightsConfig,
}

/// Tokens per character approximation (consistent with ContextBuilderService).
const TOKENS_PER_CHAR: usize = 4;

impl Default for AdaptiveContextConfig {
    fn default() -> Self {
        Self {
            base_count: 5,
            max_count: 15,
            max_tokens: 4000,
            preview_length: 200,
            min_confidence: 0.5,
            weights: NamespaceWeightsConfig::with_defaults(),
        }
    }
}

impl AdaptiveContextConfig {
    /// Creates a new configuration with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the base memory count.
    #[must_use]
    pub const fn with_base_count(mut self, count: usize) -> Self {
        self.base_count = count;
        self
    }

    /// Sets the maximum memory count.
    #[must_use]
    pub const fn with_max_count(mut self, count: usize) -> Self {
        self.max_count = count;
        self
    }

    /// Sets the maximum token budget.
    #[must_use]
    pub const fn with_max_tokens(mut self, tokens: usize) -> Self {
        self.max_tokens = tokens;
        self
    }

    /// Sets the preview length for memory content.
    #[must_use]
    pub const fn with_preview_length(mut self, length: usize) -> Self {
        self.preview_length = length;
        self
    }

    /// Sets the minimum confidence threshold.
    #[must_use]
    pub const fn with_min_confidence(mut self, confidence: f32) -> Self {
        self.min_confidence = confidence;
        self
    }

    /// Builds context configuration from search intent settings.
    #[must_use]
    pub fn from_search_intent_config(config: &SearchIntentConfig) -> Self {
        Self {
            base_count: config.base_count,
            max_count: config.max_count,
            max_tokens: config.max_tokens,
            preview_length: 200,
            min_confidence: config.min_confidence,
            weights: config.weights.clone(),
        }
    }

    /// Sets custom namespace weights.
    #[must_use]
    pub fn with_weights(mut self, weights: NamespaceWeightsConfig) -> Self {
        self.weights = weights;
        self
    }

    /// Calculates the number of memories to retrieve based on confidence.
    #[must_use]
    pub const fn memories_for_confidence(&self, confidence: f32) -> usize {
        if confidence >= 0.8 {
            self.max_count
        } else if confidence >= 0.5 {
            self.base_count + 5
        } else {
            self.base_count
        }
    }
}

/// Namespace weight multipliers for intent-based search.
#[derive(Debug, Clone, Default)]
pub struct NamespaceWeights {
    weights: HashMap<Namespace, f32>,
}

impl NamespaceWeights {
    /// Creates empty namespace weights (all 1.0).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates namespace weights for a specific intent type using hard-coded defaults.
    #[must_use]
    pub fn for_intent(intent_type: SearchIntentType) -> Self {
        Self::for_intent_with_config(intent_type, &NamespaceWeightsConfig::default())
    }

    /// Creates namespace weights for a specific intent type using config overrides.
    ///
    /// Config weights take precedence over hard-coded defaults. If a namespace
    /// is not specified in config, the hard-coded default is used.
    #[must_use]
    pub fn for_intent_with_config(
        intent_type: SearchIntentType,
        config: &NamespaceWeightsConfig,
    ) -> Self {
        let mut weights = HashMap::new();

        // Get the intent name for config lookup
        let intent_name = match intent_type {
            SearchIntentType::HowTo => "howto",
            SearchIntentType::Troubleshoot => "troubleshoot",
            SearchIntentType::Location => "location",
            SearchIntentType::Explanation => "explanation",
            SearchIntentType::Comparison => "comparison",
            SearchIntentType::General => "general",
        };

        // Apply hard-coded defaults first
        let defaults = Self::get_defaults(intent_type);
        for (ns, weight) in defaults {
            weights.insert(ns, weight);
        }

        // Apply config overrides
        for (ns_str, weight) in config.get_intent_weights(intent_name) {
            if let Ok(ns) = ns_str.parse::<Namespace>() {
                weights.insert(ns, weight);
            }
        }

        Self { weights }
    }

    /// Gets the hard-coded default weights for an intent type.
    fn get_defaults(intent_type: SearchIntentType) -> Vec<(Namespace, f32)> {
        match intent_type {
            SearchIntentType::HowTo => {
                vec![
                    (Namespace::Patterns, 1.5),
                    (Namespace::Learnings, 1.3),
                    (Namespace::Decisions, 1.0),
                ]
            },
            SearchIntentType::Troubleshoot => {
                vec![
                    (Namespace::Blockers, 1.5),
                    (Namespace::Learnings, 1.3),
                    (Namespace::Decisions, 1.0),
                ]
            },
            SearchIntentType::Location | SearchIntentType::Explanation => {
                vec![
                    (Namespace::Decisions, 1.5),
                    (Namespace::Context, 1.3),
                    (Namespace::Patterns, 1.0),
                ]
            },
            SearchIntentType::Comparison => {
                vec![
                    (Namespace::Decisions, 1.5),
                    (Namespace::Patterns, 1.3),
                    (Namespace::Learnings, 1.0),
                ]
            },
            SearchIntentType::General => {
                vec![
                    (Namespace::Decisions, 1.2),
                    (Namespace::Patterns, 1.2),
                    (Namespace::Learnings, 1.0),
                ]
            },
        }
    }

    /// Gets the weight for a namespace (defaults to 1.0).
    #[must_use]
    pub fn get(&self, namespace: &Namespace) -> f32 {
        self.weights.get(namespace).copied().unwrap_or(1.0)
    }

    /// Applies weights to a score based on namespace.
    #[must_use]
    pub fn apply(&self, namespace: &Namespace, score: f32) -> f32 {
        score * self.get(namespace)
    }
}

/// An injected memory in the context response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectedMemory {
    /// Memory ID (URN format).
    pub id: String,
    /// Memory namespace.
    pub namespace: String,
    /// Truncated content preview.
    pub content_preview: String,
    /// Relevance score.
    pub score: f32,
    /// Optional tags.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// Memory context for hook response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryContext {
    /// Whether search intent was detected.
    pub search_intent_detected: bool,
    /// The detected intent type (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent_type: Option<String>,
    /// Extracted topics from the prompt.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub topics: Vec<String>,
    /// Injected memories.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub injected_memories: Vec<InjectedMemory>,
    /// Suggested resource URIs.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggested_resources: Vec<String>,
    /// Optional reminder text for the assistant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reminder: Option<String>,
}

impl MemoryContext {
    /// Creates an empty memory context (no intent detected).
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Creates a memory context from a search intent.
    #[must_use]
    pub fn from_intent(intent: &SearchIntent) -> Self {
        Self {
            search_intent_detected: true,
            intent_type: Some(intent.intent_type.as_str().to_string()),
            topics: intent.topics.clone(),
            injected_memories: Vec::new(),
            suggested_resources: Vec::new(),
            reminder: None,
        }
    }

    /// Adds injected memories to the context.
    #[must_use]
    pub fn with_memories(mut self, memories: Vec<InjectedMemory>) -> Self {
        self.injected_memories = memories;
        self
    }

    /// Adds suggested resources to the context.
    #[must_use]
    pub fn with_resources(mut self, resources: Vec<String>) -> Self {
        self.suggested_resources = resources;
        self
    }

    /// Adds a reminder to the context.
    #[must_use]
    pub fn with_reminder(mut self, reminder: impl Into<String>) -> Self {
        self.reminder = Some(reminder.into());
        self
    }
}

/// Builder for search context with adaptive memory injection.
pub struct SearchContextBuilder<'a> {
    config: AdaptiveContextConfig,
    recall_service: Option<&'a RecallService>,
}

impl Default for SearchContextBuilder<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> SearchContextBuilder<'a> {
    /// Creates a new search context builder with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: AdaptiveContextConfig::default(),
            recall_service: None,
        }
    }

    /// Sets the configuration.
    #[must_use]
    pub fn with_config(mut self, config: AdaptiveContextConfig) -> Self {
        self.config = config;
        self
    }

    /// Sets the recall service for memory retrieval.
    #[must_use]
    pub const fn with_recall_service(mut self, service: &'a RecallService) -> Self {
        self.recall_service = Some(service);
        self
    }

    /// Builds the memory context for a search intent.
    ///
    /// # Errors
    ///
    /// Returns an error if memory retrieval fails.
    pub fn build_context(&self, intent: &SearchIntent) -> Result<MemoryContext> {
        // Check confidence threshold
        if intent.confidence < self.config.min_confidence {
            return Ok(MemoryContext::empty());
        }

        let mut context = MemoryContext::from_intent(intent);

        // Build suggested resources from topics
        let resources = self.build_suggested_resources(intent);
        context = context.with_resources(resources);

        // Add reminder if confidence is high enough
        if intent.confidence >= self.config.min_confidence {
            context = context.with_reminder(build_reminder_text(intent));
        }

        // Retrieve memories if recall service is available
        if let Some(recall) = self.recall_service {
            let memories = self.retrieve_memories(recall, intent)?;
            context = context.with_memories(memories);
        }

        Ok(context)
    }

    /// Retrieves memories based on intent.
    fn retrieve_memories(
        &self,
        recall: &RecallService,
        intent: &SearchIntent,
    ) -> Result<Vec<InjectedMemory>> {
        let limit = self.config.memories_for_confidence(intent.confidence);
        let weights =
            NamespaceWeights::for_intent_with_config(intent.intent_type, &self.config.weights);

        // Build query from topics and keywords
        let query = build_search_query(intent);
        if query.is_empty() {
            return Ok(Vec::new());
        }

        // Search with double limit to allow for reranking
        let filter = SearchFilter::new();
        let result = recall.search(&query, SearchMode::Hybrid, &filter, limit * 2)?;

        // Apply namespace weights and rerank
        let mut weighted_memories: Vec<_> = result
            .memories
            .into_iter()
            .map(|hit| {
                let weighted_score = weights.apply(&hit.memory.namespace, hit.score);
                (hit, weighted_score)
            })
            .collect();

        // Sort by weighted score
        weighted_memories
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top results and convert to InjectedMemory with token budget enforcement.
        let mut injected = Vec::new();
        let mut remaining_tokens = self.config.max_tokens;

        for (hit, score) in weighted_memories.into_iter().take(limit) {
            if remaining_tokens == 0 {
                break;
            }

            let preview = truncate_content(&hit.memory.content, self.config.preview_length);
            let preview_tokens = ContextBuilderService::estimate_tokens(&preview);

            let (content_preview, tokens_used) = if preview_tokens <= remaining_tokens {
                (preview, preview_tokens)
            } else {
                let max_chars = remaining_tokens.saturating_mul(TOKENS_PER_CHAR);
                let truncated = truncate_content(&hit.memory.content, max_chars);
                let truncated_tokens = ContextBuilderService::estimate_tokens(&truncated);
                if truncated_tokens == 0 {
                    break;
                }
                (truncated, truncated_tokens.min(remaining_tokens))
            };

            remaining_tokens = remaining_tokens.saturating_sub(tokens_used);

            injected.push(InjectedMemory {
                id: format!("subcog://memories/{}", hit.memory.id.as_str()),
                namespace: hit.memory.namespace.as_str().to_string(),
                content_preview,
                score,
                tags: hit.memory.tags.clone(),
            });
        }

        Ok(injected)
    }

    /// Builds suggested resource URIs from intent.
    fn build_suggested_resources(&self, intent: &SearchIntent) -> Vec<String> {
        let mut resources = Vec::with_capacity(4);

        // Add topic-based resources
        for topic in intent.topics.iter().take(3) {
            resources.push(format!("subcog://topics/{topic}"));
        }

        // Add topics list
        if !intent.topics.is_empty() {
            resources.push("subcog://topics".to_string());
        }

        resources
    }
}

/// Builds a search query from intent topics and keywords.
fn build_search_query(intent: &SearchIntent) -> String {
    let mut parts = Vec::new();

    // Add topics
    parts.extend(intent.topics.iter().cloned());

    // Add keywords (cleaned up)
    for keyword in &intent.keywords {
        let cleaned = keyword
            .trim()
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != ' ', "");
        if !cleaned.is_empty() && !parts.contains(&cleaned) {
            parts.push(cleaned);
        }
    }

    parts.join(" ")
}

/// Builds reminder text for the assistant.
fn build_reminder_text(intent: &SearchIntent) -> String {
    let intent_desc = match intent.intent_type {
        SearchIntentType::HowTo => "implementation guidance",
        SearchIntentType::Location => "code location",
        SearchIntentType::Explanation => "explanation",
        SearchIntentType::Comparison => "comparison",
        SearchIntentType::Troubleshoot => "troubleshooting help",
        SearchIntentType::General => "information",
    };

    format!(
        "User appears to be seeking {intent_desc}. \
         Consider using `subcog_recall` to retrieve relevant memories \
         or access the suggested resources for more context."
    )
}

/// Truncates content for preview.
fn truncate_content(content: &str, max_len: usize) -> String {
    if content.len() <= max_len {
        content.to_string()
    } else {
        let truncated = &content[..max_len.saturating_sub(3)];
        // Try to break at a word boundary
        truncated.rfind(' ').map_or_else(
            || format!("{truncated}..."),
            |last_space| format!("{}...", &truncated[..last_space]),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::search_intent::DetectionSource;
    use crate::models::{Domain, Memory, MemoryId, MemoryStatus, Namespace};
    use crate::services::RecallService;
    use crate::storage::index::SqliteBackend;
    use crate::storage::traits::IndexBackend;

    fn create_test_intent() -> SearchIntent {
        SearchIntent {
            intent_type: SearchIntentType::HowTo,
            confidence: 0.8,
            keywords: vec!["how to".to_string(), "implement".to_string()],
            topics: vec!["authentication".to_string(), "oauth".to_string()],
            source: DetectionSource::Keyword,
        }
    }

    // AdaptiveContextConfig tests

    #[test]
    fn test_config_defaults() {
        let config = AdaptiveContextConfig::default();
        assert_eq!(config.base_count, 5);
        assert_eq!(config.max_count, 15);
        assert_eq!(config.max_tokens, 4000);
        assert!((config.min_confidence - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_config_builder() {
        let config = AdaptiveContextConfig::new()
            .with_base_count(10)
            .with_max_count(20)
            .with_max_tokens(8000)
            .with_min_confidence(0.6);

        assert_eq!(config.base_count, 10);
        assert_eq!(config.max_count, 20);
        assert_eq!(config.max_tokens, 8000);
        assert!((config.min_confidence - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn test_memories_for_confidence() {
        let config = AdaptiveContextConfig::default();

        // High confidence -> max_count
        assert_eq!(config.memories_for_confidence(0.9), 15);
        assert_eq!(config.memories_for_confidence(0.8), 15);

        // Medium confidence -> base_count + 5
        assert_eq!(config.memories_for_confidence(0.7), 10);
        assert_eq!(config.memories_for_confidence(0.5), 10);

        // Low confidence -> base_count
        assert_eq!(config.memories_for_confidence(0.4), 5);
        assert_eq!(config.memories_for_confidence(0.1), 5);
    }

    // NamespaceWeights tests

    #[test]
    fn test_weights_for_howto() {
        let weights = NamespaceWeights::for_intent(SearchIntentType::HowTo);

        assert!((weights.get(&Namespace::Patterns) - 1.5).abs() < f32::EPSILON);
        assert!((weights.get(&Namespace::Learnings) - 1.3).abs() < f32::EPSILON);
        assert!((weights.get(&Namespace::Decisions) - 1.0).abs() < f32::EPSILON);
        // Unknown namespace defaults to 1.0
        assert!((weights.get(&Namespace::Apis) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_weights_for_troubleshoot() {
        let weights = NamespaceWeights::for_intent(SearchIntentType::Troubleshoot);

        assert!((weights.get(&Namespace::Blockers) - 1.5).abs() < f32::EPSILON);
        assert!((weights.get(&Namespace::Learnings) - 1.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_weights_apply() {
        let weights = NamespaceWeights::for_intent(SearchIntentType::HowTo);

        let score = 0.5;
        let weighted = weights.apply(&Namespace::Patterns, score);
        assert!((weighted - 0.75).abs() < f32::EPSILON); // 0.5 * 1.5
    }

    // MemoryContext tests

    #[test]
    fn test_memory_context_empty() {
        let ctx = MemoryContext::empty();
        assert!(!ctx.search_intent_detected);
        assert!(ctx.intent_type.is_none());
        assert!(ctx.topics.is_empty());
        assert!(ctx.injected_memories.is_empty());
    }

    #[test]
    fn test_memory_context_from_intent() {
        let intent = create_test_intent();
        let ctx = MemoryContext::from_intent(&intent);

        assert!(ctx.search_intent_detected);
        assert_eq!(ctx.intent_type, Some("howto".to_string()));
        assert_eq!(ctx.topics, vec!["authentication", "oauth"]);
    }

    #[test]
    fn test_memory_context_builder() {
        let intent = create_test_intent();
        let ctx = MemoryContext::from_intent(&intent)
            .with_memories(vec![InjectedMemory {
                id: "subcog://memories/test".to_string(),
                namespace: "patterns".to_string(),
                content_preview: "Test content".to_string(),
                score: 0.9,
                tags: vec![],
            }])
            .with_resources(vec!["subcog://topics/auth".to_string()])
            .with_reminder("Test reminder");

        assert_eq!(ctx.injected_memories.len(), 1);
        assert_eq!(ctx.suggested_resources.len(), 1);
        assert_eq!(ctx.reminder, Some("Test reminder".to_string()));
    }

    // SearchContextBuilder tests

    #[test]
    fn test_build_context_low_confidence() {
        let builder = SearchContextBuilder::new();
        let intent = SearchIntent {
            confidence: 0.3,
            ..create_test_intent()
        };

        let ctx = builder.build_context(&intent).unwrap();
        assert!(!ctx.search_intent_detected);
    }

    #[test]
    fn test_build_context_high_confidence() {
        let builder = SearchContextBuilder::new();
        let intent = create_test_intent();

        let ctx = builder.build_context(&intent).unwrap();
        assert!(ctx.search_intent_detected);
        assert!(ctx.reminder.is_some());
        assert!(!ctx.suggested_resources.is_empty());
    }

    #[test]
    fn test_build_suggested_resources() {
        let builder = SearchContextBuilder::new();
        let intent = create_test_intent();

        let resources = builder.build_suggested_resources(&intent);
        assert!(resources.contains(&"subcog://topics/authentication".to_string()));
        assert!(resources.contains(&"subcog://topics/oauth".to_string()));
        assert!(resources.contains(&"subcog://topics".to_string()));
    }

    // Helper function tests

    #[test]
    fn test_build_search_query() {
        let intent = create_test_intent();
        let query = build_search_query(&intent);

        assert!(query.contains("authentication"));
        assert!(query.contains("oauth"));
        assert!(query.contains("implement"));
    }

    #[test]
    fn test_truncate_content_short() {
        let content = "Short content";
        let truncated = truncate_content(content, 50);
        assert_eq!(truncated, content);
    }

    #[test]
    fn test_truncate_content_long() {
        let content =
            "This is a much longer content that needs to be truncated for display purposes";
        let truncated = truncate_content(content, 30);
        assert!(truncated.ends_with("..."));
        assert!(truncated.len() <= 30);
    }

    #[test]
    fn test_build_reminder_text() {
        let intent = create_test_intent();
        let reminder = build_reminder_text(&intent);
        assert!(reminder.contains("implementation guidance"));
        assert!(reminder.contains("subcog_recall"));
    }

    // InjectedMemory tests

    #[test]
    fn test_injected_memory_serialization() {
        let memory = InjectedMemory {
            id: "subcog://memories/test-123".to_string(),
            namespace: "decisions".to_string(),
            content_preview: "Use PostgreSQL for storage".to_string(),
            score: 0.85,
            tags: vec!["database".to_string()],
        };

        let json = serde_json::to_string(&memory).unwrap();
        assert!(json.contains("test-123"));
        assert!(json.contains("decisions"));
        assert!(json.contains("PostgreSQL"));
    }

    fn create_test_memory(id: &str, content: &str, now: u64) -> Memory {
        Memory {
            id: MemoryId::new(id),
            content: content.to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: now,
            updated_at: now,
            tombstoned_at: None,
            embedding: None,
            tags: vec![],
            source: None,
        }
    }

    #[test]
    fn test_injected_memories_respect_token_budget() {
        let index = SqliteBackend::in_memory().unwrap();
        let now = 1_700_000_000;
        let content = "authentication ".repeat(40);

        let memory1 = create_test_memory("mem1", &content, now);
        let memory2 = create_test_memory("mem2", &content, now);
        index.index(&memory1).unwrap();
        index.index(&memory2).unwrap();

        let recall = RecallService::with_index(index);
        let config = AdaptiveContextConfig::new()
            .with_base_count(2)
            .with_max_count(2)
            .with_max_tokens(20)
            .with_min_confidence(0.0);
        let builder = SearchContextBuilder::new()
            .with_config(config)
            .with_recall_service(&recall);

        let intent = SearchIntent {
            intent_type: SearchIntentType::General,
            confidence: 0.9,
            keywords: vec!["authentication".to_string()],
            topics: vec!["authentication".to_string()],
            source: DetectionSource::Keyword,
        };

        let ctx = builder.build_context(&intent).unwrap();
        let total_tokens: usize = ctx
            .injected_memories
            .iter()
            .map(|memory| ContextBuilderService::estimate_tokens(&memory.content_preview))
            .sum();

        assert!(total_tokens <= 20);
    }
}
