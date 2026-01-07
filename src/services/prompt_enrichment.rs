//! Prompt enrichment service.
//!
//! Enriches prompt templates with LLM-generated metadata including:
//! - Prompt-level description
//! - Tags for categorization
//! - Variable descriptions, defaults, and required flags
//!
//! # Fallback Behavior
//!
//! The enrichment service degrades gracefully when the LLM is unavailable:
//!
//! | Condition | Behavior | Result Status |
//! |-----------|----------|---------------|
//! | LLM available, success | Full metadata generated | `Full` |
//! | LLM available, timeout | Use extracted variables only | `Fallback` |
//! | LLM available, error | Use extracted variables only | `Fallback` |
//! | No LLM configured | Use extracted variables only | `Fallback` |
//! | User passed `--no-enrich` | Skip enrichment entirely | `Skipped` |
//!
//! ## Fallback Metadata
//!
//! When enrichment fails or is unavailable, minimal metadata is generated:
//!
//! - **description**: Empty string (user can edit later)
//! - **tags**: Empty array
//! - **variables**: Names preserved, descriptions set to "No description"
//!
//! ## User Values Take Precedence
//!
//! User-provided metadata is never overwritten by LLM enrichment:
//!
//! ```text
//! User provides: description="My review prompt", tags=["review"]
//! LLM generates: description="Code review assistant", tags=["code", "review", "quality"]
//!
//! Result: description="My review prompt", tags=["review"]  (user values preserved)
//! ```
//!
//! ## Error Handling
//!
//! | Error Type | Logging | User Impact |
//! |------------|---------|-------------|
//! | Timeout (5s) | WARN | Prompt saved with fallback metadata |
//! | Parse error | WARN | Prompt saved with fallback metadata |
//! | Network error | WARN | Prompt saved with fallback metadata |
//! | No LLM config | DEBUG | Prompt saved with fallback metadata |
//!
//! The service never fails a prompt save due to enrichment errors.

use crate::llm::{LlmProvider, sanitize_llm_response_for_error};
use crate::models::PromptVariable;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::instrument;

/// System prompt for prompt template enrichment.
///
/// Instructs the LLM to analyze a prompt template and generate rich metadata.
pub const PROMPT_ENRICHMENT_SYSTEM_PROMPT: &str = r#"<task>
You are analyzing a prompt template to generate helpful metadata.
Your goal is to understand the prompt's purpose and generate accurate descriptions
for both the prompt itself and its variables.
</task>

<output_format>
Respond with ONLY valid JSON, no markdown formatting.

{
  "description": "One sentence describing what this prompt does",
  "tags": ["tag1", "tag2", "tag3"],
  "variables": [
    {
      "name": "variable_name",
      "description": "What this variable represents",
      "required": true,
      "default": null
    }
  ]
}
</output_format>

<guidelines>
- description: Clear, one-sentence summary of the prompt's purpose
- tags: 2-5 lowercase, hyphenated tags (e.g., "code-review", "documentation")
- variables: For each detected variable:
  - description: What value the user should provide
  - required: true if the prompt makes no sense without it
  - default: Sensible default value, or null if none appropriate
</guidelines>

<rules>
- Only include variables that were detected in the prompt
- Use lowercase hyphenated format for tags
- Keep descriptions concise but informative
- Respond with valid JSON only, no explanation
</rules>"#;

/// Default timeout for LLM enrichment calls.
pub const ENRICHMENT_TIMEOUT: Duration = Duration::from_secs(5);

/// Request for prompt enrichment.
#[derive(Debug, Clone, Serialize)]
pub struct EnrichmentRequest {
    /// The prompt content to analyze.
    pub content: String,
    /// Variable names extracted from the prompt.
    pub variables: Vec<String>,
    /// Existing metadata to preserve (user-provided values).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing: Option<PartialMetadata>,
}

impl EnrichmentRequest {
    /// Creates a new enrichment request.
    #[must_use]
    pub fn new(content: impl Into<String>, variables: Vec<String>) -> Self {
        Self {
            content: content.into(),
            variables,
            existing: None,
        }
    }

    /// Sets existing metadata to preserve.
    #[must_use]
    pub fn with_existing(mut self, existing: PartialMetadata) -> Self {
        self.existing = Some(existing);
        self
    }

    /// Optionally sets existing metadata if provided.
    #[must_use]
    pub fn with_optional_existing(mut self, existing: Option<PartialMetadata>) -> Self {
        self.existing = existing;
        self
    }
}

/// Partial metadata provided by the user.
///
/// Fields that are `Some` will be preserved and not overwritten by LLM enrichment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PartialMetadata {
    /// User-provided description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// User-provided tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// User-provided variable definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<PromptVariable>,
}

impl PartialMetadata {
    /// Creates empty partial metadata.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the tags.
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Sets the variables.
    #[must_use]
    pub fn with_variables(mut self, variables: Vec<PromptVariable>) -> Self {
        self.variables = variables;
        self
    }

    /// Checks if any metadata is provided.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.description.is_none() && self.tags.is_empty() && self.variables.is_empty()
    }

    /// Gets a user-defined variable by name.
    #[must_use]
    pub fn get_variable(&self, name: &str) -> Option<&PromptVariable> {
        self.variables.iter().find(|v| v.name == name)
    }
}

/// Result of prompt enrichment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptEnrichmentResult {
    /// Generated or preserved description.
    pub description: String,
    /// Generated or preserved tags.
    pub tags: Vec<String>,
    /// Enriched variable definitions.
    pub variables: Vec<PromptVariable>,
    /// Status of enrichment.
    #[serde(default)]
    pub status: EnrichmentStatus,
}

/// Status of the enrichment operation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnrichmentStatus {
    /// Full LLM enrichment was applied.
    #[default]
    Full,
    /// Fallback to basic extraction (LLM unavailable or failed).
    Fallback,
    /// Enrichment was skipped by request.
    Skipped,
}

impl PromptEnrichmentResult {
    /// Creates a basic result from variable names (fallback when LLM unavailable).
    ///
    /// Each variable is marked as required with no description or default.
    #[must_use]
    pub fn basic_from_variables(variables: &[String]) -> Self {
        Self {
            description: String::new(),
            tags: Vec::new(),
            variables: variables
                .iter()
                .map(|name| PromptVariable {
                    name: name.clone(),
                    description: None,
                    default: None,
                    required: true,
                })
                .collect(),
            status: EnrichmentStatus::Fallback,
        }
    }

    /// Merges LLM-generated metadata with user-provided partial metadata.
    ///
    /// User-provided values are preserved; LLM fills in the gaps.
    #[must_use]
    pub fn merge_with_user(mut self, user: &PartialMetadata) -> Self {
        // Preserve user description if provided
        if let Some(ref desc) = user.description {
            self.description.clone_from(desc);
        }

        // Preserve user tags if provided
        if !user.tags.is_empty() {
            self.tags.clone_from(&user.tags);
        }

        // Merge variables: user-provided values take precedence
        for var in &mut self.variables {
            let Some(user_var) = user.get_variable(&var.name) else {
                continue;
            };
            // Preserve user-provided description if set
            if let Some(ref desc) = user_var.description {
                var.description = Some(desc.clone());
            }
            // Preserve user-provided default if set
            if let Some(ref default) = user_var.default {
                var.default = Some(default.clone());
            }
            // Preserve user-provided required flag
            var.required = user_var.required;
        }

        self
    }
}

/// LLM response format for enrichment.
#[derive(Debug, Clone, Deserialize)]
struct LlmEnrichmentResponse {
    description: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    variables: Vec<LlmVariableResponse>,
}

/// Variable definition from LLM response.
#[derive(Debug, Clone, Deserialize)]
struct LlmVariableResponse {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default = "default_required")]
    required: bool,
    #[serde(default)]
    default: Option<String>,
}

/// Default for required field.
const fn default_required() -> bool {
    true
}

/// Service for enriching prompt templates with LLM-generated metadata.
pub struct PromptEnrichmentService<P: LlmProvider> {
    /// LLM provider for generating enrichments.
    llm: P,
}

impl<P: LlmProvider> PromptEnrichmentService<P> {
    /// Creates a new prompt enrichment service.
    #[must_use]
    pub const fn new(llm: P) -> Self {
        Self { llm }
    }

    /// Enriches a prompt template with LLM-generated metadata.
    ///
    /// # Arguments
    ///
    /// * `request` - The enrichment request containing prompt content and variables.
    ///
    /// # Returns
    ///
    /// An `EnrichmentResult` with descriptions, tags, and variable definitions.
    ///
    /// # Errors
    ///
    /// Returns an error if the LLM call fails. Use `enrich_with_fallback` for
    /// graceful degradation.
    #[instrument(skip(self), fields(operation = "prompt_enrich", variables_count = request.variables.len()))]
    pub fn enrich(&self, request: &EnrichmentRequest) -> Result<PromptEnrichmentResult> {
        // Build the user message
        let user_message = self.build_user_message(request);

        // Call LLM with system prompt
        let response = self
            .llm
            .complete_with_system(PROMPT_ENRICHMENT_SYSTEM_PROMPT, &user_message)?;

        // Parse JSON response
        let llm_result = self.parse_response(&response, &request.variables)?;

        // Merge with user-provided metadata if any
        let result = if let Some(ref existing) = request.existing {
            llm_result.merge_with_user(existing)
        } else {
            llm_result
        };

        Ok(result)
    }

    /// Enriches with graceful fallback on failure.
    ///
    /// If LLM enrichment fails (network, timeout, parse error), returns basic
    /// metadata with just the variable names.
    #[instrument(skip(self), fields(operation = "prompt_enrich_fallback"))]
    pub fn enrich_with_fallback(&self, request: &EnrichmentRequest) -> PromptEnrichmentResult {
        match self.enrich(request) {
            Ok(result) => result,
            Err(e) => {
                tracing::warn!("Prompt enrichment failed, using fallback: {}", e);
                let mut result = PromptEnrichmentResult::basic_from_variables(&request.variables);

                // Still merge with user-provided metadata
                if let Some(ref existing) = request.existing {
                    result = result.merge_with_user(existing);
                }

                result
            },
        }
    }

    /// Builds the user message for the LLM.
    fn build_user_message(&self, request: &EnrichmentRequest) -> String {
        let variables_str = if request.variables.is_empty() {
            "No variables detected".to_string()
        } else {
            request.variables.join(", ")
        };
        format!(
            "<prompt_content>\n{}\n</prompt_content>\n\n<detected_variables>\n{}\n</detected_variables>",
            request.content, variables_str
        )
    }

    /// Parses the LLM response into an enrichment result.
    fn parse_response(
        &self,
        response: &str,
        expected_variables: &[String],
    ) -> Result<PromptEnrichmentResult> {
        // Extract JSON from response (handle potential markdown wrapping)
        let json_str = crate::llm::extract_json_from_response(response);

        // Parse JSON
        let sanitized = sanitize_llm_response_for_error(response);
        let llm_response: LlmEnrichmentResponse =
            serde_json::from_str(json_str).map_err(|e| Error::OperationFailed {
                operation: "parse_enrichment_response".to_string(),
                cause: format!("Failed to parse LLM response: {e}. Response was: {sanitized}"),
            })?;

        // Convert to result, ensuring all expected variables are present
        let mut variable_map: std::collections::HashMap<String, PromptVariable> = llm_response
            .variables
            .into_iter()
            .map(|v| {
                (
                    v.name.clone(),
                    PromptVariable {
                        name: v.name,
                        description: v.description,
                        default: v.default,
                        required: v.required,
                    },
                )
            })
            .collect();

        // Ensure all expected variables are present
        let variables: Vec<PromptVariable> = expected_variables
            .iter()
            .map(|name| {
                variable_map.remove(name).unwrap_or_else(|| PromptVariable {
                    name: name.clone(),
                    description: None,
                    default: None,
                    required: true,
                })
            })
            .collect();

        Ok(PromptEnrichmentResult {
            description: llm_response.description,
            tags: llm_response.tags,
            variables,
            status: EnrichmentStatus::Full,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock LLM provider for testing.
    struct MockLlmProvider {
        response: String,
        should_fail: bool,
    }

    impl MockLlmProvider {
        fn new(response: impl Into<String>) -> Self {
            Self {
                response: response.into(),
                should_fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                response: String::new(),
                should_fail: true,
            }
        }
    }

    impl LlmProvider for MockLlmProvider {
        fn name(&self) -> &'static str {
            "mock"
        }

        fn complete(&self, _prompt: &str) -> Result<String> {
            if self.should_fail {
                Err(Error::OperationFailed {
                    operation: "mock_complete".to_string(),
                    cause: "Mock LLM failure".to_string(),
                })
            } else {
                Ok(self.response.clone())
            }
        }

        fn complete_with_system(&self, _system: &str, _prompt: &str) -> Result<String> {
            if self.should_fail {
                Err(Error::OperationFailed {
                    operation: "mock_complete".to_string(),
                    cause: "Mock LLM failure".to_string(),
                })
            } else {
                Ok(self.response.clone())
            }
        }

        fn analyze_for_capture(&self, _content: &str) -> Result<crate::llm::CaptureAnalysis> {
            Ok(crate::llm::CaptureAnalysis {
                should_capture: true,
                confidence: 0.8,
                suggested_namespace: Some("decisions".to_string()),
                suggested_tags: vec![],
                reasoning: "Mock analysis".to_string(),
            })
        }
    }

    #[test]
    fn test_enrichment_request_new() {
        let request = EnrichmentRequest::new(
            "Review {{file}} for {{issue_type}}",
            vec!["file".to_string(), "issue_type".to_string()],
        );
        assert_eq!(request.content, "Review {{file}} for {{issue_type}}");
        assert_eq!(request.variables.len(), 2);
        assert!(request.existing.is_none());
    }

    #[test]
    fn test_enrichment_request_with_existing() {
        let existing = PartialMetadata::new().with_description("My description");
        let request =
            EnrichmentRequest::new("Test {{var}}", vec!["var".to_string()]).with_existing(existing);
        assert!(request.existing.is_some());
        assert_eq!(
            request.existing.unwrap().description,
            Some("My description".to_string())
        );
    }

    #[test]
    fn test_partial_metadata_is_empty() {
        let empty = PartialMetadata::new();
        assert!(empty.is_empty());

        let with_desc = PartialMetadata::new().with_description("test");
        assert!(!with_desc.is_empty());

        let with_tags = PartialMetadata::new().with_tags(vec!["tag".to_string()]);
        assert!(!with_tags.is_empty());
    }

    #[test]
    fn test_partial_metadata_get_variable() {
        let vars = vec![
            PromptVariable {
                name: "file".to_string(),
                description: Some("File path".to_string()),
                default: None,
                required: true,
            },
            PromptVariable {
                name: "type".to_string(),
                description: None,
                default: Some("general".to_string()),
                required: false,
            },
        ];
        let partial = PartialMetadata::new().with_variables(vars);

        let file_var = partial.get_variable("file");
        assert!(file_var.is_some());
        assert_eq!(file_var.unwrap().description, Some("File path".to_string()));

        let missing = partial.get_variable("missing");
        assert!(missing.is_none());
    }

    #[test]
    fn test_enrichment_result_basic_from_variables() {
        let result =
            PromptEnrichmentResult::basic_from_variables(&["file".to_string(), "type".to_string()]);

        assert!(result.description.is_empty());
        assert!(result.tags.is_empty());
        assert_eq!(result.variables.len(), 2);
        assert_eq!(result.status, EnrichmentStatus::Fallback);

        assert_eq!(result.variables[0].name, "file");
        assert!(result.variables[0].required);
        assert!(result.variables[0].description.is_none());
    }

    #[test]
    fn test_enrichment_result_merge_with_user() {
        let llm_result = PromptEnrichmentResult {
            description: "LLM description".to_string(),
            tags: vec!["llm-tag".to_string()],
            variables: vec![
                PromptVariable {
                    name: "file".to_string(),
                    description: Some("LLM file desc".to_string()),
                    default: None,
                    required: true,
                },
                PromptVariable {
                    name: "type".to_string(),
                    description: Some("LLM type desc".to_string()),
                    default: Some("llm-default".to_string()),
                    required: true,
                },
            ],
            status: EnrichmentStatus::Full,
        };

        let user = PartialMetadata::new()
            .with_description("User description")
            .with_variables(vec![PromptVariable {
                name: "file".to_string(),
                description: Some("User file desc".to_string()),
                default: Some("user-default".to_string()),
                required: false,
            }]);

        let merged = llm_result.merge_with_user(&user);

        // User description takes precedence
        assert_eq!(merged.description, "User description");
        // Tags not provided by user, so LLM tags remain
        assert_eq!(merged.tags, vec!["llm-tag".to_string()]);

        // file variable: user values take precedence
        assert_eq!(
            merged.variables[0].description,
            Some("User file desc".to_string())
        );
        assert_eq!(
            merged.variables[0].default,
            Some("user-default".to_string())
        );
        assert!(!merged.variables[0].required);

        // type variable: no user override, LLM values remain
        assert_eq!(
            merged.variables[1].description,
            Some("LLM type desc".to_string())
        );
        assert_eq!(merged.variables[1].default, Some("llm-default".to_string()));
    }

    #[test]
    fn test_enrichment_service_successful() {
        let mock_response = r#"{
            "description": "Code review prompt for specific files",
            "tags": ["code-review", "analysis"],
            "variables": [
                {
                    "name": "file",
                    "description": "Path to the file to review",
                    "required": true,
                    "default": null
                },
                {
                    "name": "issue_type",
                    "description": "Category of issues to look for",
                    "required": false,
                    "default": "general"
                }
            ]
        }"#;

        let llm = MockLlmProvider::new(mock_response);
        let service = PromptEnrichmentService::new(llm);

        let request = EnrichmentRequest::new(
            "Review {{file}} for {{issue_type}} issues",
            vec!["file".to_string(), "issue_type".to_string()],
        );

        let result = service.enrich(&request).unwrap();

        assert_eq!(result.description, "Code review prompt for specific files");
        assert_eq!(result.tags, vec!["code-review", "analysis"]);
        assert_eq!(result.variables.len(), 2);
        assert_eq!(result.status, EnrichmentStatus::Full);

        let file_var = result.variables.iter().find(|v| v.name == "file").unwrap();
        assert_eq!(
            file_var.description,
            Some("Path to the file to review".to_string())
        );
        assert!(file_var.required);

        let type_var = result
            .variables
            .iter()
            .find(|v| v.name == "issue_type")
            .unwrap();
        assert_eq!(type_var.default, Some("general".to_string()));
        assert!(!type_var.required);
    }

    #[test]
    fn test_enrichment_service_with_json_in_markdown() {
        let mock_response = r#"```json
{
    "description": "Test prompt",
    "tags": ["test"],
    "variables": []
}
```"#;

        let llm = MockLlmProvider::new(mock_response);
        let service = PromptEnrichmentService::new(llm);

        let request = EnrichmentRequest::new("Test content", vec![]);

        let result = service.enrich(&request).unwrap();
        assert_eq!(result.description, "Test prompt");
    }

    #[test]
    fn test_enrichment_service_fallback_on_error() {
        let llm = MockLlmProvider::failing();
        let service = PromptEnrichmentService::new(llm);

        let request = EnrichmentRequest::new("Review {{file}}", vec!["file".to_string()]);

        let result = service.enrich_with_fallback(&request);

        assert_eq!(result.status, EnrichmentStatus::Fallback);
        assert!(result.description.is_empty());
        assert_eq!(result.variables.len(), 1);
        assert_eq!(result.variables[0].name, "file");
    }

    #[test]
    fn test_enrichment_service_fallback_preserves_user_metadata() {
        let llm = MockLlmProvider::failing();
        let service = PromptEnrichmentService::new(llm);

        let existing = PartialMetadata::new()
            .with_description("User description")
            .with_tags(vec!["user-tag".to_string()]);

        let request = EnrichmentRequest::new("Review {{file}}", vec!["file".to_string()])
            .with_existing(existing);

        let result = service.enrich_with_fallback(&request);

        assert_eq!(result.description, "User description");
        assert_eq!(result.tags, vec!["user-tag".to_string()]);
    }

    #[test]
    fn test_enrichment_service_missing_variable_filled() {
        // LLM response missing one variable
        let mock_response = r#"{
            "description": "Test",
            "tags": [],
            "variables": [
                {"name": "file", "description": "File path", "required": true}
            ]
        }"#;

        let llm = MockLlmProvider::new(mock_response);
        let service = PromptEnrichmentService::new(llm);

        let request = EnrichmentRequest::new(
            "Review {{file}} for {{issue_type}}",
            vec!["file".to_string(), "issue_type".to_string()],
        );

        let result = service.enrich(&request).unwrap();

        // Both variables should be present
        assert_eq!(result.variables.len(), 2);

        // Missing variable should have defaults
        let missing = result
            .variables
            .iter()
            .find(|v| v.name == "issue_type")
            .unwrap();
        assert!(missing.description.is_none());
        assert!(missing.required);
    }

    #[test]
    fn test_enrichment_service_empty_variables() {
        let mock_response = r#"{
            "description": "Static prompt with no variables",
            "tags": ["static"],
            "variables": []
        }"#;

        let llm = MockLlmProvider::new(mock_response);
        let service = PromptEnrichmentService::new(llm);

        let request = EnrichmentRequest::new("Hello, world!", vec![]);

        let result = service.enrich(&request).unwrap();

        assert_eq!(result.description, "Static prompt with no variables");
        assert!(result.variables.is_empty());
    }

    #[test]
    fn test_enrichment_service_invalid_json() {
        let mock_response = "This is not JSON";

        let llm = MockLlmProvider::new(mock_response);
        let service = PromptEnrichmentService::new(llm);

        let request = EnrichmentRequest::new("Test {{var}}", vec!["var".to_string()]);

        let result = service.enrich(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_enrichment_status_serialization() {
        let full = EnrichmentStatus::Full;
        let serialized = serde_json::to_string(&full).unwrap();
        assert_eq!(serialized, r#""full""#);

        let fallback = EnrichmentStatus::Fallback;
        let serialized = serde_json::to_string(&fallback).unwrap();
        assert_eq!(serialized, r#""fallback""#);

        let skipped = EnrichmentStatus::Skipped;
        let serialized = serde_json::to_string(&skipped).unwrap();
        assert_eq!(serialized, r#""skipped""#);
    }

    #[test]
    fn test_system_prompt_contains_required_sections() {
        assert!(PROMPT_ENRICHMENT_SYSTEM_PROMPT.contains("<task>"));
        assert!(PROMPT_ENRICHMENT_SYSTEM_PROMPT.contains("<output_format>"));
        assert!(PROMPT_ENRICHMENT_SYSTEM_PROMPT.contains("<guidelines>"));
        assert!(PROMPT_ENRICHMENT_SYSTEM_PROMPT.contains("<rules>"));
        assert!(PROMPT_ENRICHMENT_SYSTEM_PROMPT.contains("description"));
        assert!(PROMPT_ENRICHMENT_SYSTEM_PROMPT.contains("tags"));
        assert!(PROMPT_ENRICHMENT_SYSTEM_PROMPT.contains("variables"));
    }
}
