//! Memory capture service.
//!
//! Handles capturing new memories, including validation, redaction, and storage.

use crate::config::Config;
use crate::git::{NotesManager, YamlFrontMatterParser};
use crate::models::{CaptureRequest, CaptureResult, Memory, MemoryId, MemoryStatus};
use crate::security::{ContentRedactor, SecretDetector};
use crate::{Error, Result};
use std::time::{SystemTime, UNIX_EPOCH};

/// Service for capturing memories.
pub struct CaptureService {
    /// Configuration.
    config: Config,
    /// Secret detector.
    secret_detector: SecretDetector,
    /// Content redactor.
    redactor: ContentRedactor,
}

impl CaptureService {
    /// Creates a new capture service.
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            secret_detector: SecretDetector::new(),
            redactor: ContentRedactor::new(),
        }
    }

    /// Captures a memory.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The content is empty
    /// - The content contains unredacted secrets (when blocking is enabled)
    /// - Storage fails
    pub fn capture(&self, request: CaptureRequest) -> Result<CaptureResult> {
        // Validate content
        if request.content.trim().is_empty() {
            return Err(Error::InvalidInput("Content cannot be empty".to_string()));
        }

        // Check for secrets
        let has_secrets = self.secret_detector.contains_secrets(&request.content);
        if has_secrets && self.config.features.block_secrets && !request.skip_security_check {
            return Err(Error::ContentBlocked {
                reason: "Content contains detected secrets".to_string(),
            });
        }

        // Optionally redact secrets
        let (content, was_redacted) =
            if has_secrets && self.config.features.redact_secrets && !request.skip_security_check {
                (self.redactor.redact(&request.content), true)
            } else {
                (request.content.clone(), false)
            };

        // Generate memory ID
        let memory_id = self.generate_id(&request.namespace);

        // Get current timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Create memory
        let memory = Memory {
            id: memory_id.clone(),
            content,
            namespace: request.namespace,
            domain: request.domain,
            status: MemoryStatus::Active,
            created_at: now,
            updated_at: now,
            embedding: None,
            tags: request.tags,
            source: request.source,
        };

        // Store to git notes if configured
        if let Some(ref repo_path) = self.config.repo_path {
            let notes = NotesManager::new(repo_path);

            // Serialize memory with front matter
            let metadata = serde_json::json!({
                "id": memory.id.as_str(),
                "namespace": memory.namespace.as_str(),
                "domain": memory.domain.to_string(),
                "status": memory.status.as_str(),
                "created_at": memory.created_at,
                "updated_at": memory.updated_at,
                "tags": memory.tags
            });

            let note_content = YamlFrontMatterParser::serialize(&metadata, &memory.content)?;
            notes.add_to_head(&note_content)?;
        }

        // Generate URN
        let urn = self.generate_urn(&memory);

        // Collect warnings
        let mut warnings = Vec::new();
        if was_redacted {
            warnings.push("Content was redacted due to detected secrets".to_string());
        }

        Ok(CaptureResult {
            memory_id,
            urn,
            content_modified: was_redacted,
            warnings,
        })
    }

    /// Generates a unique memory ID.
    fn generate_id(&self, namespace: &crate::models::Namespace) -> MemoryId {
        let uuid = uuid::Uuid::new_v4();
        MemoryId::new(format!("{}_{}", namespace.as_str(), uuid))
    }

    /// Generates a URN for the memory.
    fn generate_urn(&self, memory: &Memory) -> String {
        let domain_part = if memory.domain.is_global() {
            "global".to_string()
        } else {
            memory.domain.to_string().replace('/', ":")
        };

        format!(
            "urn:subcog:{}:{}:{}",
            domain_part,
            memory.namespace.as_str(),
            memory.id.as_str()
        )
    }

    /// Validates a capture request without storing.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails.
    pub fn validate(&self, request: &CaptureRequest) -> Result<ValidationResult> {
        let mut issues = Vec::new();
        let mut warnings = Vec::new();

        // Check content length
        if request.content.trim().is_empty() {
            issues.push("Content cannot be empty".to_string());
        } else if request.content.len() > 100_000 {
            warnings.push("Content is very long (>100KB)".to_string());
        }

        // Check for secrets
        let secrets = self.secret_detector.detect_types(&request.content);
        if !secrets.is_empty() {
            if self.config.features.block_secrets {
                issues.push(format!("Content contains secrets: {}", secrets.join(", ")));
            } else {
                warnings.push(format!("Content contains secrets: {}", secrets.join(", ")));
            }
        }

        Ok(ValidationResult {
            is_valid: issues.is_empty(),
            issues,
            warnings,
        })
    }
}

impl Default for CaptureService {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

/// Result of capture validation.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the capture request is valid.
    pub is_valid: bool,
    /// List of blocking issues.
    pub issues: Vec<String>,
    /// List of non-blocking warnings.
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, Namespace};

    fn test_config() -> Config {
        Config::default()
    }

    fn test_request(content: &str) -> CaptureRequest {
        CaptureRequest {
            content: content.to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::default(),
            tags: vec!["test".to_string()],
            source: Some("test.rs".to_string()),
            skip_security_check: false,
        }
    }

    #[test]
    fn test_capture_success() {
        let service = CaptureService::new(test_config());
        let request = test_request("Use PostgreSQL for primary storage");

        let result = service.capture(request);
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(result.memory_id.as_str().starts_with("decisions_"));
        assert!(result.urn.starts_with("urn:subcog:"));
        assert!(!result.content_modified);
    }

    #[test]
    fn test_capture_empty_content() {
        let service = CaptureService::new(test_config());
        let request = test_request("   ");

        let result = service.capture(request);
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::InvalidInput(_))));
    }

    #[test]
    fn test_capture_with_secrets_redacted() {
        let mut config = test_config();
        config.features.redact_secrets = true;
        config.features.block_secrets = false;

        let service = CaptureService::new(config);
        let request = test_request("My API key is AKIAIOSFODNN7EXAMPLE");

        let result = service.capture(request);
        assert!(result.is_ok());
        assert!(result.unwrap().content_modified);
    }

    #[test]
    fn test_capture_with_secrets_blocked() {
        let mut config = test_config();
        config.features.block_secrets = true;

        let service = CaptureService::new(config);
        let request = test_request("My API key is AKIAIOSFODNN7EXAMPLE");

        let result = service.capture(request);
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::ContentBlocked { .. })));
    }

    #[test]
    fn test_validate_valid() {
        let service = CaptureService::new(test_config());
        let request = test_request("Valid content");

        let result = service.validate(&request).unwrap();
        assert!(result.is_valid);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_validate_empty() {
        let service = CaptureService::new(test_config());
        let request = test_request("");

        let result = service.validate(&request).unwrap();
        assert!(!result.is_valid);
        assert!(!result.issues.is_empty());
    }

    #[test]
    fn test_generate_urn() {
        let service = CaptureService::new(test_config());

        let memory = Memory {
            id: MemoryId::new("test_123"),
            content: "Test".to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::for_repository("zircote", "subcog"),
            status: MemoryStatus::Active,
            created_at: 0,
            updated_at: 0,
            embedding: None,
            tags: vec![],
            source: None,
        };

        let urn = service.generate_urn(&memory);
        assert!(urn.contains("subcog"));
        assert!(urn.contains("decisions"));
        assert!(urn.contains("test_123"));
    }
}
