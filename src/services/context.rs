//! Context builder service.
//!
//! Builds context for Claude Code hooks, selecting the most relevant memories.

use crate::Result;
use crate::models::{Memory, Namespace, SearchFilter, SearchMode};
use crate::services::RecallService;
use std::borrow::Cow;
use std::collections::HashMap;

// Context building limits - tunable parameters for memory selection
/// Maximum memories to fetch for decisions (high priority).
const CONTEXT_DECISIONS_LIMIT: usize = 5;
/// Maximum memories to fetch for patterns.
const CONTEXT_PATTERNS_LIMIT: usize = 3;
/// Maximum memories to fetch for project context.
const CONTEXT_PROJECT_LIMIT: usize = 3;
/// Maximum memories to fetch for tech debt.
const CONTEXT_TECH_DEBT_LIMIT: usize = 2;
/// Default search result limit.
const SEARCH_RESULT_LIMIT: usize = 10;
/// Maximum recent memories to fetch for statistics.
const RECENT_MEMORIES_LIMIT: usize = 100;
/// Maximum top tags to return.
const TOP_TAGS_LIMIT: usize = 10;
/// Maximum topics to track.
const MAX_TOPICS: usize = 10;
/// Tokens per character approximation (for context truncation).
const TOKENS_PER_CHAR: usize = 4;
/// Maximum length for memory content preview in formatted output.
const MEMORY_CONTENT_PREVIEW_LENGTH: usize = 200;
/// Maximum words to extract for topic summary.
const TOPIC_WORDS_LIMIT: usize = 5;
/// Maximum length for topic display.
const MAX_TOPIC_DISPLAY_LENGTH: usize = 50;

/// Statistics about memories in the system.
#[derive(Debug, Clone, Default)]
pub struct MemoryStatistics {
    /// Total memory count.
    pub total_count: usize,
    /// Count per namespace.
    pub namespace_counts: HashMap<String, usize>,
    /// Most common tags (top 10).
    pub top_tags: Vec<(String, usize)>,
    /// Recent topics extracted from memories.
    pub recent_topics: Vec<String>,
}

/// Service for building context for AI assistants.
pub struct ContextBuilderService {
    /// Recall service for searching memories.
    recall: Option<RecallService>,
}

impl ContextBuilderService {
    /// Creates a new context builder service.
    #[must_use]
    pub const fn new() -> Self {
        Self { recall: None }
    }

    /// Creates a context builder with a recall service.
    #[must_use]
    pub const fn with_recall(recall: RecallService) -> Self {
        Self {
            recall: Some(recall),
        }
    }

    /// Builds context for the current session.
    ///
    /// # Errors
    ///
    /// Returns an error if context building fails.
    pub fn build_context(&self, max_tokens: usize) -> Result<String> {
        // Estimate tokens per character (rough approximation)
        let max_chars = max_tokens * TOKENS_PER_CHAR;

        let mut context_parts = Vec::new();

        // Add recent decisions (high priority)
        if let Some(decisions) =
            self.get_relevant_memories(Namespace::Decisions, CONTEXT_DECISIONS_LIMIT)?
            && !decisions.is_empty()
        {
            context_parts.push(format_section("Recent Decisions", &decisions));
        }

        // Add active patterns
        if let Some(patterns) =
            self.get_relevant_memories(Namespace::Patterns, CONTEXT_PATTERNS_LIMIT)?
            && !patterns.is_empty()
        {
            context_parts.push(format_section("Active Patterns", &patterns));
        }

        // Add relevant context
        if let Some(ctx) = self.get_relevant_memories(Namespace::Context, CONTEXT_PROJECT_LIMIT)?
            && !ctx.is_empty()
        {
            context_parts.push(format_section("Project Context", &ctx));
        }

        // Add known tech debt
        if let Some(debt) =
            self.get_relevant_memories(Namespace::TechDebt, CONTEXT_TECH_DEBT_LIMIT)?
            && !debt.is_empty()
        {
            context_parts.push(format_section("Known Tech Debt", &debt));
        }

        // Combine and truncate to fit token budget
        let full_context = context_parts.join("\n\n");

        if full_context.len() > max_chars {
            Ok(truncate_context(&full_context, max_chars))
        } else {
            Ok(full_context)
        }
    }

    /// Builds context for a specific query.
    ///
    /// # Errors
    ///
    /// Returns an error if context building fails.
    pub fn build_query_context(&self, query: &str, max_tokens: usize) -> Result<String> {
        let max_chars = max_tokens * TOKENS_PER_CHAR;

        let recall = self
            .recall
            .as_ref()
            .ok_or_else(|| crate::Error::OperationFailed {
                operation: "build_query_context".to_string(),
                cause: "No recall service configured".to_string(),
            })?;

        // Search for relevant memories
        let result = recall.search(
            query,
            SearchMode::Hybrid,
            &SearchFilter::new(),
            SEARCH_RESULT_LIMIT,
        )?;

        if result.memories.is_empty() {
            return Ok(String::new());
        }

        let mut context_parts = Vec::new();
        context_parts.push("# Relevant Memories".to_string());

        for hit in &result.memories {
            let memory = &hit.memory;
            context_parts.push(format!(
                "## {} ({})\n{}\n_Score: {:.2}_",
                memory.namespace,
                memory.id.as_str(),
                memory.content,
                hit.score
            ));
        }

        let full_context = context_parts.join("\n\n");

        if full_context.len() > max_chars {
            Ok(truncate_context(&full_context, max_chars))
        } else {
            Ok(full_context)
        }
    }

    /// Gets relevant memories for a namespace.
    ///
    /// Returns `None` if recall service is not configured, otherwise returns
    /// an empty vector (placeholder for full storage integration).
    #[allow(clippy::unnecessary_wraps)] // Returns Result for API consistency with other methods
    const fn get_relevant_memories(
        &self,
        _namespace: Namespace,
        _limit: usize,
    ) -> Result<Option<Vec<Memory>>> {
        // Without a recall service, return None
        if self.recall.is_none() {
            return Ok(None);
        }

        // For now, return empty since we'd need full storage integration
        Ok(Some(Vec::new()))
    }

    /// Estimates the token count for a string.
    #[must_use]
    pub const fn estimate_tokens(text: &str) -> usize {
        // Rough estimation: uses TOKENS_PER_CHAR for English text
        text.len() / TOKENS_PER_CHAR
    }

    /// Gets memory statistics for session context.
    ///
    /// Uses [`RecallService::list_all_with_content`] to fetch memories with full content
    /// for topic extraction. This is intentionally more expensive than the lightweight
    /// [`RecallService::list_all`] used by MCP tools.
    ///
    /// # Errors
    ///
    /// Returns an error if statistics gathering fails.
    pub fn get_statistics(&self) -> Result<MemoryStatistics> {
        let Some(recall) = &self.recall else {
            return Ok(MemoryStatistics::default());
        };

        // Fetch all memories with content for topic extraction
        let result = recall.list_all_with_content(&SearchFilter::new(), RECENT_MEMORIES_LIMIT)?;

        let mut namespace_counts: HashMap<String, usize> = HashMap::new();
        let mut tag_counts: HashMap<String, usize> = HashMap::new();
        let mut topics: Vec<String> = Vec::new();

        for hit in &result.memories {
            let memory = &hit.memory;

            // Count namespaces
            *namespace_counts
                .entry(memory.namespace.as_str().to_string())
                .or_insert(0) += 1;

            // Count tags
            for tag in &memory.tags {
                *tag_counts.entry(tag.clone()).or_insert(0) += 1;
            }

            // Extract topics (first few words of content)
            if let Some(topic) = extract_topic(&memory.content) {
                add_topic_if_unique(&mut topics, topic);
            }
        }

        // Sort tags by count
        let mut top_tags: Vec<(String, usize)> = tag_counts.into_iter().collect();
        top_tags.sort_by(|a, b| b.1.cmp(&a.1));
        top_tags.truncate(TOP_TAGS_LIMIT);

        Ok(MemoryStatistics {
            total_count: result.memories.len(),
            namespace_counts,
            top_tags,
            recent_topics: topics,
        })
    }
}

impl Default for ContextBuilderService {
    fn default() -> Self {
        Self::new()
    }
}

/// Formats a section with a title and memories.
fn format_section(title: &str, memories: &[Memory]) -> String {
    let mut parts = vec![format!("## {title}")];

    for memory in memories {
        parts.push(format!(
            "- **{}** ({}): {}",
            memory.namespace,
            memory.id.as_str(),
            truncate_content(&memory.content, MEMORY_CONTENT_PREVIEW_LENGTH)
        ));
    }

    parts.join("\n")
}

/// Truncates content to a maximum length.
///
/// # Performance
///
/// Returns `Cow::Borrowed` when no truncation is needed (zero allocation).
/// Only allocates when truncation is required.
fn truncate_content(content: &str, max_len: usize) -> Cow<'_, str> {
    if content.len() <= max_len {
        Cow::Borrowed(content)
    } else {
        Cow::Owned(format!("{}...", &content[..max_len - 3]))
    }
}

/// Adds a topic to the list if it's unique and list has space.
fn add_topic_if_unique(topics: &mut Vec<String>, topic: String) {
    if !topics.contains(&topic) && topics.len() < MAX_TOPICS {
        topics.push(topic);
    }
}

/// Extracts a topic summary from memory content.
fn extract_topic(content: &str) -> Option<String> {
    // Get first meaningful words (skip common prefixes)
    let words: Vec<&str> = content
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .take(TOPIC_WORDS_LIMIT)
        .collect();

    if words.is_empty() {
        return None;
    }

    let topic = words.join(" ");
    if topic.len() > MAX_TOPIC_DISPLAY_LENGTH {
        Some(format!("{}...", &topic[..MAX_TOPIC_DISPLAY_LENGTH - 3]))
    } else {
        Some(topic)
    }
}

/// Truncates context to fit within a character limit.
#[allow(clippy::option_if_let_else)] // if-let chain is clearer than nested map_or_else
fn truncate_context(context: &str, max_chars: usize) -> String {
    if context.len() <= max_chars {
        return context.to_string();
    }

    // Try to truncate at a section boundary
    let truncated = &context[..max_chars];
    if let Some(last_section) = truncated.rfind("\n##") {
        format!(
            "{}\n\n_[Context truncated due to token limit]_",
            &context[..last_section]
        )
    } else if let Some(last_newline) = truncated.rfind('\n') {
        format!(
            "{}\n\n_[Context truncated due to token limit]_",
            &context[..last_newline]
        )
    } else {
        format!("{}...", &context[..max_chars - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_builder_creation() {
        let service = ContextBuilderService::default();
        let result = service.build_context(1000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_estimate_tokens() {
        let text = "This is a test string with about 40 characters.";
        let tokens = ContextBuilderService::estimate_tokens(text);
        assert!(tokens > 0);
        assert!(tokens < text.len());
    }

    #[test]
    fn test_truncate_content() {
        let short = "Short text";
        assert_eq!(truncate_content(short, 100), short);

        let long =
            "This is a longer text that should be truncated because it exceeds the maximum length";
        let truncated = truncate_content(long, 30);
        assert!(truncated.ends_with("..."));
        assert!(truncated.len() <= 30);
    }

    #[test]
    fn test_truncate_context() {
        let context =
            "## Section 1\nContent 1\n\n## Section 2\nContent 2\n\n## Section 3\nContent 3";

        // Should fit without truncation
        let result = truncate_context(context, 1000);
        assert_eq!(result, context);

        // Should truncate at section boundary
        let result = truncate_context(context, 40);
        assert!(result.contains("truncated"));
    }

    #[test]
    fn test_format_section() {
        use crate::models::{Domain, MemoryId, MemoryStatus};

        let memories = vec![Memory {
            id: MemoryId::new("test_id"),
            content: "Test content".to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: 0,
            updated_at: 0,
            tombstoned_at: None,
            expires_at: None,
            embedding: None,
            tags: vec![],
            #[cfg(feature = "group-scope")]
            group_id: None,
            source: None,
            is_summary: false,
            source_memory_ids: None,
            consolidation_timestamp: None,
        }];

        let section = format_section("Test Section", &memories);
        assert!(section.contains("## Test Section"));
        assert!(section.contains("Test content"));
    }

    #[test]
    fn test_build_query_context_no_recall() {
        let service = ContextBuilderService::default();
        let result = service.build_query_context("test", 1000);
        assert!(result.is_err());
    }
}
