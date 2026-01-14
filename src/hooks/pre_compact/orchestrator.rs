//! Capture orchestration for pre-compact hook.
//!
//! This module coordinates the capture of candidates with deduplication,
//! handling the interaction between capture service and deduplication service.

use super::analyzer::CaptureCandidate;
use super::formatter::{CapturedMemory, SkippedDuplicate};
use crate::models::{CaptureRequest, Domain, MemoryId};
use crate::services::CaptureService;
use crate::services::deduplication::{ContentHasher, Deduplicator, DuplicateReason};
use std::sync::Arc;

/// Orchestrates capture operations with deduplication support.
pub struct CaptureOrchestrator {
    /// Capture service instance.
    capture: Option<CaptureService>,
    /// Deduplication service instance (trait object for flexibility).
    dedup: Option<Arc<dyn Deduplicator>>,
}

impl CaptureOrchestrator {
    /// Creates a new orchestrator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            capture: None,
            dedup: None,
        }
    }

    /// Sets the capture service.
    #[must_use]
    pub fn with_capture(mut self, capture: CaptureService) -> Self {
        self.capture = Some(capture);
        self
    }

    /// Sets the deduplication service.
    #[must_use]
    pub fn with_deduplication(mut self, dedup: Arc<dyn Deduplicator>) -> Self {
        self.dedup = Some(dedup);
        self
    }

    /// Returns whether deduplication is configured.
    ///
    /// This method is primarily used in tests to verify builder configuration.
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn has_deduplication(&self) -> bool {
        self.dedup.is_some()
    }

    /// Performs the actual capture of candidates.
    ///
    /// If a deduplication service is configured, checks each candidate for
    /// duplicates before capture. Returns both captured memories and skipped duplicates.
    ///
    /// **Note**: If no capture service is configured, this method returns empty results
    /// and logs a debug message. Configure a capture service using [`with_capture`].
    pub fn capture_candidates(
        &self,
        candidates: Vec<CaptureCandidate>,
    ) -> (Vec<CapturedMemory>, Vec<SkippedDuplicate>) {
        let Some(capture) = &self.capture else {
            if !candidates.is_empty() {
                tracing::debug!(
                    candidate_count = candidates.len(),
                    "CaptureOrchestrator: No capture service configured, skipping {} candidates",
                    candidates.len()
                );
            }
            return (Vec::new(), Vec::new());
        };

        let mut captured = Vec::new();
        let mut skipped = Vec::new();

        for candidate in candidates {
            if candidate.confidence < 0.6 {
                continue;
            }

            // Check for duplicates
            if let Some(skip_info) = self.check_for_duplicate(&candidate) {
                skipped.push(skip_info);
                continue;
            }

            // Capture the candidate
            let request = CaptureRequest {
                content: candidate.content.clone(),
                namespace: candidate.namespace,
                domain: Domain::default(),
                tags: vec!["auto-captured".to_string(), "pre-compact".to_string()],
                source: Some("PreCompactHandler".to_string()),
                skip_security_check: false,
                ttl_seconds: None,
                scope: None, // Use default scope
                #[cfg(feature = "group-scope")]
                group_id: None,
            };

            if let Ok(result) = capture.capture(request.clone()) {
                self.record_capture_for_dedup(&request.content, &result.memory_id);

                captured.push(CapturedMemory {
                    memory_id: result.memory_id.to_string(),
                    namespace: candidate.namespace.as_str().to_string(),
                    confidence: candidate.confidence,
                });
            }
            // Errors are silently ignored, continue with other candidates
        }

        (captured, skipped)
    }

    /// Checks if a candidate is a duplicate and returns skip info if so.
    ///
    /// Returns `Some(SkippedDuplicate)` if the candidate should be skipped,
    /// `None` if it should be captured.
    fn check_for_duplicate(&self, candidate: &CaptureCandidate) -> Option<SkippedDuplicate> {
        let dedup = self.dedup.as_ref()?;

        match dedup.check_duplicate(&candidate.content, candidate.namespace) {
            Ok(result) if result.is_duplicate => {
                let reason_str = reason_to_str(result.reason);
                let matched_urn = result.matched_urn.unwrap_or_default();

                tracing::debug!(
                    namespace = %candidate.namespace.as_str(),
                    matched_urn = %matched_urn,
                    reason = reason_str,
                    "Skipping duplicate candidate"
                );

                metrics::counter!(
                    "hook_deduplication_skipped_total",
                    "hook_type" => "PreCompact",
                    "namespace" => candidate.namespace.as_str().to_string(),
                    "reason" => reason_str.to_string()
                )
                .increment(1);

                Some(SkippedDuplicate {
                    reason: reason_str.to_string(),
                    matched_urn,
                    similarity_score: result.similarity_score,
                    namespace: candidate.namespace.as_str().to_string(),
                })
            },
            Ok(_) => None, // Not a duplicate
            Err(e) => {
                // Graceful degradation: log error and proceed with capture
                tracing::warn!(
                    error = %e,
                    namespace = %candidate.namespace.as_str(),
                    "Deduplication check failed, proceeding with capture"
                );
                None
            },
        }
    }

    /// Records a successful capture in the deduplication service.
    fn record_capture_for_dedup(&self, content: &str, memory_id: &MemoryId) {
        if let Some(dedup) = &self.dedup {
            let hash = ContentHasher::hash(content);
            dedup.record_capture(&hash, memory_id);
        }
    }
}

impl Default for CaptureOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

/// Converts a `DuplicateReason` to a string.
#[must_use]
pub fn reason_to_str(reason: Option<DuplicateReason>) -> &'static str {
    reason.map_or("unknown", |r| match r {
        DuplicateReason::ExactMatch => "exact_match",
        DuplicateReason::SemanticSimilar => "semantic_similar",
        DuplicateReason::RecentCapture => "recent_capture",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Namespace;
    use crate::services::deduplication::DuplicateCheckResult;

    #[test]
    fn test_orchestrator_creation() {
        let orchestrator = CaptureOrchestrator::new();
        assert!(!orchestrator.has_deduplication());
    }

    #[test]
    fn test_with_deduplication() {
        struct MockDedup;
        impl Deduplicator for MockDedup {
            fn check_duplicate(
                &self,
                _content: &str,
                _namespace: Namespace,
            ) -> crate::Result<DuplicateCheckResult> {
                Ok(DuplicateCheckResult::not_duplicate(0))
            }
            fn record_capture(&self, _hash: &str, _memory_id: &MemoryId) {}
        }

        let orchestrator = CaptureOrchestrator::new().with_deduplication(Arc::new(MockDedup));
        assert!(orchestrator.has_deduplication());
    }

    #[test]
    fn test_check_for_duplicate_skips() {
        struct MockDedupAlwaysDup;
        impl Deduplicator for MockDedupAlwaysDup {
            fn check_duplicate(
                &self,
                _content: &str,
                _namespace: Namespace,
            ) -> crate::Result<DuplicateCheckResult> {
                Ok(DuplicateCheckResult::exact_match(
                    MemoryId::new("123"),
                    "subcog://test/decisions/123".to_string(),
                    0,
                ))
            }
            fn record_capture(&self, _hash: &str, _memory_id: &MemoryId) {}
        }

        let orchestrator =
            CaptureOrchestrator::new().with_deduplication(Arc::new(MockDedupAlwaysDup));

        let candidate = CaptureCandidate {
            content: "Test content".to_string(),
            namespace: Namespace::Decisions,
            confidence: 0.8,
        };

        let result = orchestrator.check_for_duplicate(&candidate);
        assert!(result.is_some());
        let skip = result.unwrap();
        assert_eq!(skip.reason, "exact_match");
        assert_eq!(skip.matched_urn, "subcog://test/decisions/123");
    }

    #[test]
    fn test_check_for_duplicate_passes() {
        struct MockDedupNoDup;
        impl Deduplicator for MockDedupNoDup {
            fn check_duplicate(
                &self,
                _content: &str,
                _namespace: Namespace,
            ) -> crate::Result<DuplicateCheckResult> {
                Ok(DuplicateCheckResult::not_duplicate(0))
            }
            fn record_capture(&self, _hash: &str, _memory_id: &MemoryId) {}
        }

        let orchestrator = CaptureOrchestrator::new().with_deduplication(Arc::new(MockDedupNoDup));

        let candidate = CaptureCandidate {
            content: "Test content".to_string(),
            namespace: Namespace::Decisions,
            confidence: 0.8,
        };

        let result = orchestrator.check_for_duplicate(&candidate);
        assert!(result.is_none());
    }

    #[test]
    fn test_check_for_duplicate_graceful_degradation() {
        struct MockDedupError;
        impl Deduplicator for MockDedupError {
            fn check_duplicate(
                &self,
                _content: &str,
                _namespace: Namespace,
            ) -> crate::Result<DuplicateCheckResult> {
                Err(crate::Error::OperationFailed {
                    operation: "test".to_string(),
                    cause: "simulated error".to_string(),
                })
            }
            fn record_capture(&self, _hash: &str, _memory_id: &MemoryId) {}
        }

        let orchestrator = CaptureOrchestrator::new().with_deduplication(Arc::new(MockDedupError));

        let candidate = CaptureCandidate {
            content: "Test content".to_string(),
            namespace: Namespace::Decisions,
            confidence: 0.8,
        };

        // Error should result in None (proceed with capture)
        let result = orchestrator.check_for_duplicate(&candidate);
        assert!(result.is_none());
    }

    #[test]
    fn test_reason_to_str() {
        assert_eq!(
            reason_to_str(Some(DuplicateReason::ExactMatch)),
            "exact_match"
        );
        assert_eq!(
            reason_to_str(Some(DuplicateReason::SemanticSimilar)),
            "semantic_similar"
        );
        assert_eq!(
            reason_to_str(Some(DuplicateReason::RecentCapture)),
            "recent_capture"
        );
        assert_eq!(reason_to_str(None), "unknown");
    }
}
