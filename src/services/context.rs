//! Context builder service.
//!
//! Builds context for Claude Code hooks, selecting the most relevant memories.

use crate::Result;
use crate::models::{Memory, Namespace, SearchFilter, SearchMode};
use crate::services::RecallService;

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
        let max_chars = max_tokens * 4;

        let mut context_parts = Vec::new();

        // Add recent decisions (high priority)
        if let Some(decisions) = self.get_relevant_memories(Namespace::Decisions, 5)? {
            if !decisions.is_empty() {
                context_parts.push(format_section("Recent Decisions", &decisions));
            }
        }

        // Add active patterns
        if let Some(patterns) = self.get_relevant_memories(Namespace::Patterns, 3)? {
            if !patterns.is_empty() {
                context_parts.push(format_section("Active Patterns", &patterns));
            }
        }

        // Add relevant context
        if let Some(ctx) = self.get_relevant_memories(Namespace::Context, 3)? {
            if !ctx.is_empty() {
                context_parts.push(format_section("Project Context", &ctx));
            }
        }

        // Add known tech debt
        if let Some(debt) = self.get_relevant_memories(Namespace::TechDebt, 2)? {
            if !debt.is_empty() {
                context_parts.push(format_section("Known Tech Debt", &debt));
            }
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
        let max_chars = max_tokens * 4;

        let recall = self
            .recall
            .as_ref()
            .ok_or_else(|| crate::Error::OperationFailed {
                operation: "build_query_context".to_string(),
                cause: "No recall service configured".to_string(),
            })?;

        // Search for relevant memories
        let result = recall.search(query, SearchMode::Hybrid, &SearchFilter::new(), 10)?;

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
        // Rough estimation: ~4 characters per token for English text
        text.len() / 4
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
            truncate_content(&memory.content, 200)
        ));
    }

    parts.join("\n")
}

/// Truncates content to a maximum length.
fn truncate_content(content: &str, max_len: usize) -> String {
    if content.len() <= max_len {
        content.to_string()
    } else {
        format!("{}...", &content[..max_len - 3])
    }
}

/// Truncates context to fit within a character limit.
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
            status: MemoryStatus::Active,
            created_at: 0,
            updated_at: 0,
            embedding: None,
            tags: vec![],
            source: None,
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
