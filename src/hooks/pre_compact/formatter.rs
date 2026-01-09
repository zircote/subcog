//! Response formatting for pre-compact hook.
//!
//! This module handles building human-readable and structured JSON responses
//! for the pre-compact hook output.

use serde::{Deserialize, Serialize};

/// A memory that was auto-captured.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedMemory {
    /// Memory ID.
    pub memory_id: String,
    /// Namespace.
    pub namespace: String,
    /// Confidence score.
    pub confidence: f32,
}

/// A candidate that was skipped due to duplication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedDuplicate {
    /// The reason it was skipped.
    pub reason: String,
    /// URN of the existing memory it matched.
    pub matched_urn: String,
    /// Similarity score (for semantic matches).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub similarity_score: Option<f32>,
    /// Namespace of the candidate.
    pub namespace: String,
}

/// Formats hook responses for Claude Code.
pub struct ResponseFormatter;

impl ResponseFormatter {
    /// Builds the human-readable context message for the hook response.
    #[must_use]
    pub fn build_context_message(
        captured: &[CapturedMemory],
        skipped: &[SkippedDuplicate],
    ) -> Option<String> {
        if captured.is_empty() && skipped.is_empty() {
            return None;
        }

        let mut lines = vec!["**Subcog Pre-Compact Auto-Capture**\n".to_string()];

        if !captured.is_empty() {
            lines.push(format!(
                "Captured {} memories before context compaction:\n",
                captured.len()
            ));
            for c in captured {
                lines.push(format!(
                    "- `{}`: {} (confidence: {:.0}%)",
                    c.namespace,
                    c.memory_id,
                    c.confidence * 100.0
                ));
            }
        }

        if !skipped.is_empty() {
            if !captured.is_empty() {
                lines.push(String::new()); // blank line
            }
            lines.push(format!("Skipped {} duplicates:\n", skipped.len()));
            for s in skipped {
                let score_str = s
                    .similarity_score
                    .map_or(String::new(), |sc| format!(" ({:.0}% similar)", sc * 100.0));
                lines.push(format!(
                    "- `{}`: {} ({}{})",
                    s.namespace, s.matched_urn, s.reason, score_str
                ));
            }
        }

        Some(lines.join("\n"))
    }

    /// Builds the Claude Code hook response JSON.
    ///
    /// Note: `PreCompact` hooks don't support `hookSpecificOutput` per Claude Code
    /// hook specification. The context message is logged for debugging but not
    /// returned in the response. Returns empty JSON `{}`.
    #[must_use]
    pub fn build_hook_response(
        captured: &[CapturedMemory],
        skipped: &[SkippedDuplicate],
    ) -> serde_json::Value {
        // Build metadata for logging/debugging purposes
        let metadata = serde_json::json!({
            "captured": !captured.is_empty(),
            "captures": captured.iter().map(|c| serde_json::json!({
                "memory_id": c.memory_id,
                "namespace": c.namespace,
                "confidence": c.confidence
            })).collect::<Vec<_>>(),
            "skipped_duplicates": skipped.len(),
            "duplicates": skipped.iter().map(|s| serde_json::json!({
                "reason": s.reason,
                "matched_urn": s.matched_urn,
                "namespace": s.namespace,
                "similarity_score": s.similarity_score
            })).collect::<Vec<_>>()
        });

        // Log the context for debugging (PreCompact hooks cannot inject context)
        if let Some(ctx) = Self::build_context_message(captured, skipped) {
            tracing::info!(
                captures = captured.len(),
                skipped = skipped.len(),
                "PreCompact auto-capture completed"
            );
            tracing::debug!(context = %ctx, metadata = ?metadata, "PreCompact context (not returned)");
        }

        // PreCompact hooks don't support hookSpecificOutput - return empty
        serde_json::json!({})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_context_message_empty() {
        let captured: Vec<CapturedMemory> = vec![];
        let skipped: Vec<SkippedDuplicate> = vec![];

        let result = ResponseFormatter::build_context_message(&captured, &skipped);
        assert!(result.is_none());
    }

    #[test]
    fn test_build_context_message_with_captures() {
        let captured = vec![CapturedMemory {
            memory_id: "mem-123".to_string(),
            namespace: "decisions".to_string(),
            confidence: 0.85,
        }];
        let skipped: Vec<SkippedDuplicate> = vec![];

        let result = ResponseFormatter::build_context_message(&captured, &skipped);
        assert!(result.is_some());
        let msg = result.unwrap();
        assert!(msg.contains("Captured 1 memories"));
        assert!(msg.contains("decisions"));
        assert!(msg.contains("mem-123"));
        assert!(msg.contains("85%"));
    }

    #[test]
    fn test_build_context_message_with_skipped() {
        let captured: Vec<CapturedMemory> = vec![];
        let skipped = vec![SkippedDuplicate {
            reason: "exact_match".to_string(),
            matched_urn: "subcog://project/decisions/abc123".to_string(),
            similarity_score: None,
            namespace: "decisions".to_string(),
        }];

        let result = ResponseFormatter::build_context_message(&captured, &skipped);
        assert!(result.is_some());
        let msg = result.unwrap();
        assert!(msg.contains("Skipped 1 duplicates"));
        assert!(msg.contains("exact_match"));
        assert!(msg.contains("subcog://project/decisions/abc123"));
    }

    #[test]
    fn test_build_context_message_with_similarity_score() {
        let captured: Vec<CapturedMemory> = vec![];
        let skipped = vec![SkippedDuplicate {
            reason: "semantic_similar".to_string(),
            matched_urn: "subcog://project/patterns/def456".to_string(),
            similarity_score: Some(0.92),
            namespace: "patterns".to_string(),
        }];

        let result = ResponseFormatter::build_context_message(&captured, &skipped);
        assert!(result.is_some());
        let msg = result.unwrap();
        assert!(msg.contains("92% similar"));
    }

    #[test]
    fn test_build_hook_response_empty() {
        let captured: Vec<CapturedMemory> = vec![];
        let skipped: Vec<SkippedDuplicate> = vec![];

        let response = ResponseFormatter::build_hook_response(&captured, &skipped);
        // PreCompact hooks don't support hookSpecificOutput - always empty
        assert!(response.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_build_hook_response_with_data() {
        let captured = vec![CapturedMemory {
            memory_id: "mem-789".to_string(),
            namespace: "learnings".to_string(),
            confidence: 0.9,
        }];
        let skipped: Vec<SkippedDuplicate> = vec![];

        let response = ResponseFormatter::build_hook_response(&captured, &skipped);
        // PreCompact hooks don't support hookSpecificOutput per Claude Code spec
        // Context is logged but not returned
        assert!(response.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_build_hook_response_returns_empty_json() {
        let captured = vec![CapturedMemory {
            memory_id: "mem-abc".to_string(),
            namespace: "blockers".to_string(),
            confidence: 0.75,
        }];
        let skipped: Vec<SkippedDuplicate> = vec![];

        let response = ResponseFormatter::build_hook_response(&captured, &skipped);
        // PreCompact hooks return empty JSON - context is logged only
        assert!(response.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_build_hook_response_mixed() {
        let captured = vec![CapturedMemory {
            memory_id: "new-mem".to_string(),
            namespace: "context".to_string(),
            confidence: 0.88,
        }];
        let skipped = vec![SkippedDuplicate {
            reason: "recent_capture".to_string(),
            matched_urn: "subcog://project/context/old-mem".to_string(),
            similarity_score: None,
            namespace: "context".to_string(),
        }];

        let response = ResponseFormatter::build_hook_response(&captured, &skipped);
        // PreCompact hooks return empty JSON - context is logged only
        assert!(response.as_object().unwrap().is_empty());

        // Verify context message generation still works (for logging)
        let context = ResponseFormatter::build_context_message(&captured, &skipped);
        assert!(context.is_some());
        let ctx = context.unwrap();
        assert!(ctx.contains("Captured 1 memories"));
        assert!(ctx.contains("Skipped 1 duplicates"));
        assert!(ctx.contains("new-mem"));
        assert!(ctx.contains("old-mem"));
    }
}
