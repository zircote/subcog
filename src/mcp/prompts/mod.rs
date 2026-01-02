//! MCP pre-defined prompts.
//!
//! Provides prompt templates for the Model Context Protocol.
//!
//! # Module Structure
//!
//! - [`types`]: Core data structures (`PromptDefinition`, `PromptMessage`, etc.)
//! - [`templates`]: Static content strings for prompts
//! - [`generators`]: Prompt message generation logic

mod generators;
mod templates;
mod types;

pub use types::{PromptArgument, PromptContent, PromptDefinition, PromptMessage};

use serde_json::Value;
use std::collections::HashMap;
use types::user_prompt_to_definition;

/// Registry of pre-defined prompts.
pub struct PromptRegistry {
    /// Available prompts.
    prompts: HashMap<String, PromptDefinition>,
}

impl PromptRegistry {
    /// Creates a new prompt registry.
    #[must_use]
    pub fn new() -> Self {
        let mut prompts = HashMap::new();

        // Register all prompts
        for prompt in Self::all_prompts() {
            prompts.insert(prompt.name.clone(), prompt);
        }

        Self { prompts }
    }

    /// Returns all prompt definitions.
    fn all_prompts() -> Vec<PromptDefinition> {
        vec![
            Self::tutorial_prompt(),
            Self::generate_tutorial_definition(),
            Self::capture_assistant_prompt(),
            Self::review_prompt(),
            Self::document_decision_prompt(),
            Self::search_help_prompt(),
            Self::browse_prompt(),
            Self::list_prompt(),
            // Phase 4: Intent-aware prompts
            Self::intent_search_prompt(),
            Self::query_suggest_prompt(),
            Self::context_capture_prompt(),
            Self::discover_prompt(),
        ]
    }

    fn tutorial_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_tutorial".to_string(),
            description: Some("Interactive tutorial for learning Subcog memory system".to_string()),
            arguments: vec![
                PromptArgument {
                    name: "familiarity".to_string(),
                    description: Some("Your familiarity level with memory systems".to_string()),
                    required: false,
                },
                PromptArgument {
                    name: "focus".to_string(),
                    description: Some("Topic to focus on".to_string()),
                    required: false,
                },
            ],
        }
    }

    fn generate_tutorial_definition() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_generate_tutorial".to_string(),
            description: Some(
                "Generate a tutorial on any topic using memories as source material".to_string(),
            ),
            arguments: vec![
                PromptArgument {
                    name: "topic".to_string(),
                    description: Some("Topic to create tutorial for".to_string()),
                    required: true,
                },
                PromptArgument {
                    name: "level".to_string(),
                    description: Some(
                        "Tutorial level: beginner, intermediate, advanced".to_string(),
                    ),
                    required: false,
                },
                PromptArgument {
                    name: "format".to_string(),
                    description: Some("Output format: markdown, outline, steps".to_string()),
                    required: false,
                },
            ],
        }
    }

    fn capture_assistant_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_capture_assistant".to_string(),
            description: Some("Help decide what to capture and which namespace to use".to_string()),
            arguments: vec![PromptArgument {
                name: "context".to_string(),
                description: Some(
                    "The current context or conversation to analyze for memories".to_string(),
                ),
                required: true,
            }],
        }
    }

    fn review_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_review".to_string(),
            description: Some("Review and analyze existing memories for a project".to_string()),
            arguments: vec![
                PromptArgument {
                    name: "namespace".to_string(),
                    description: Some("Optional namespace to focus on".to_string()),
                    required: false,
                },
                PromptArgument {
                    name: "action".to_string(),
                    description: Some(
                        "Action: summarize, consolidate, archive, or cleanup".to_string(),
                    ),
                    required: false,
                },
            ],
        }
    }

    fn document_decision_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_document_decision".to_string(),
            description: Some(
                "Help document an architectural or design decision properly".to_string(),
            ),
            arguments: vec![
                PromptArgument {
                    name: "decision".to_string(),
                    description: Some("Brief description of the decision".to_string()),
                    required: true,
                },
                PromptArgument {
                    name: "alternatives".to_string(),
                    description: Some("Alternatives that were considered".to_string()),
                    required: false,
                },
            ],
        }
    }

    fn search_help_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_search_help".to_string(),
            description: Some("Help craft effective memory search queries".to_string()),
            arguments: vec![PromptArgument {
                name: "goal".to_string(),
                description: Some("What you're trying to find or accomplish".to_string()),
                required: true,
            }],
        }
    }

    fn browse_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_browse".to_string(),
            description: Some(
                "Interactive memory browser with faceted discovery and filtering".to_string(),
            ),
            arguments: vec![
                PromptArgument {
                    name: "filter".to_string(),
                    description: Some(
                        "Filter expression: ns:X, tag:X, tag:X,Y (OR), -tag:X (exclude), since:Nd, source:X, status:X".to_string(),
                    ),
                    required: false,
                },
                PromptArgument {
                    name: "view".to_string(),
                    description: Some(
                        "View mode: dashboard (default), tags, namespaces, memories".to_string(),
                    ),
                    required: false,
                },
                PromptArgument {
                    name: "top".to_string(),
                    description: Some("Number of items per facet (default: 10)".to_string()),
                    required: false,
                },
            ],
        }
    }

    fn list_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_list".to_string(),
            description: Some(
                "List memories in formatted table with namespace/tag summary".to_string(),
            ),
            arguments: vec![
                PromptArgument {
                    name: "filter".to_string(),
                    description: Some(
                        "Filter expression: ns:X, tag:X, since:Nd (same syntax as subcog_browse)"
                            .to_string(),
                    ),
                    required: false,
                },
                PromptArgument {
                    name: "format".to_string(),
                    description: Some(
                        "Output format: table (default), compact, detailed".to_string(),
                    ),
                    required: false,
                },
                PromptArgument {
                    name: "limit".to_string(),
                    description: Some("Maximum memories to list (default: 50)".to_string()),
                    required: false,
                },
            ],
        }
    }

    // Phase 4: Intent-aware prompts

    fn intent_search_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_intent_search".to_string(),
            description: Some(
                "Search memories with automatic intent detection and query refinement".to_string(),
            ),
            arguments: vec![
                PromptArgument {
                    name: "query".to_string(),
                    description: Some("Natural language query to search for".to_string()),
                    required: true,
                },
                PromptArgument {
                    name: "context".to_string(),
                    description: Some(
                        "Current working context (file, task) for relevance boosting".to_string(),
                    ),
                    required: false,
                },
            ],
        }
    }

    fn query_suggest_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_query_suggest".to_string(),
            description: Some(
                "Get query suggestions based on memory topics and current context".to_string(),
            ),
            arguments: vec![
                PromptArgument {
                    name: "topic".to_string(),
                    description: Some("Topic area to explore".to_string()),
                    required: false,
                },
                PromptArgument {
                    name: "namespace".to_string(),
                    description: Some("Namespace to focus suggestions on".to_string()),
                    required: false,
                },
            ],
        }
    }

    fn context_capture_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_context_capture".to_string(),
            description: Some(
                "Analyze conversation context and suggest memories to capture".to_string(),
            ),
            arguments: vec![
                PromptArgument {
                    name: "conversation".to_string(),
                    description: Some("Recent conversation or code changes to analyze".to_string()),
                    required: true,
                },
                PromptArgument {
                    name: "threshold".to_string(),
                    description: Some(
                        "Confidence threshold for suggestions (default: 0.7)".to_string(),
                    ),
                    required: false,
                },
            ],
        }
    }

    fn discover_prompt() -> PromptDefinition {
        PromptDefinition {
            name: "subcog_discover".to_string(),
            description: Some(
                "Discover related memories and topics through exploratory navigation".to_string(),
            ),
            arguments: vec![
                PromptArgument {
                    name: "start".to_string(),
                    description: Some("Starting point: memory ID, topic, or keyword".to_string()),
                    required: false,
                },
                PromptArgument {
                    name: "depth".to_string(),
                    description: Some("How many hops to explore (default: 2)".to_string()),
                    required: false,
                },
            ],
        }
    }

    /// Returns all prompt definitions (built-in only).
    #[must_use]
    pub fn list_prompts(&self) -> Vec<&PromptDefinition> {
        self.prompts.values().collect()
    }

    /// Returns all prompt definitions including user-defined prompts.
    ///
    /// User prompts are fetched from the `PromptService` and combined with built-in prompts.
    /// Built-in prompts take precedence if there are name conflicts.
    #[must_use]
    pub fn list_all_prompts(
        &self,
        prompt_service: &mut crate::services::PromptService,
    ) -> Vec<PromptDefinition> {
        use crate::services::PromptFilter;

        let mut result: Vec<PromptDefinition> = self.prompts.values().cloned().collect();

        // Add user prompts from all domains
        let user_prompts = prompt_service
            .list(&PromptFilter::default())
            .unwrap_or_default();
        for template in user_prompts {
            let definition = user_prompt_to_definition(&template);
            // Skip if we already have a built-in prompt with this name
            if !self.prompts.contains_key(&definition.name) {
                result.push(definition);
            }
        }

        result
    }

    /// Gets a prompt definition by name, including user prompts.
    ///
    /// User prompts are prefixed with "user/" (e.g., "user/code-review").
    #[must_use]
    pub fn get_prompt_with_user(
        &self,
        name: &str,
        prompt_service: &mut crate::services::PromptService,
    ) -> Option<PromptDefinition> {
        // Check built-in prompts first
        if let Some(builtin) = self.prompts.get(name) {
            return Some(builtin.clone());
        }

        // Check user prompts (with or without "user/" prefix)
        let user_name = name.strip_prefix("user/").unwrap_or(name);
        prompt_service
            .get(user_name, None)
            .ok()
            .flatten()
            .map(|t| user_prompt_to_definition(&t))
    }

    /// Gets a prompt definition by name.
    #[must_use]
    pub fn get_prompt(&self, name: &str) -> Option<&PromptDefinition> {
        self.prompts.get(name)
    }

    /// Generates prompt messages for a given prompt and arguments.
    #[must_use]
    pub fn get_prompt_messages(&self, name: &str, arguments: &Value) -> Option<Vec<PromptMessage>> {
        match name {
            "subcog_tutorial" => Some(generators::generate_tutorial_prompt(arguments)),
            "subcog_generate_tutorial" => {
                Some(generators::generate_generate_tutorial_messages(arguments))
            },
            "subcog_capture_assistant" => {
                Some(generators::generate_capture_assistant_prompt(arguments))
            },
            "subcog_review" => Some(generators::generate_review_prompt(arguments)),
            "subcog_document_decision" => Some(generators::generate_decision_prompt(arguments)),
            "subcog_search_help" => Some(generators::generate_search_help_prompt(arguments)),
            "subcog_browse" => Some(generators::generate_browse_prompt(arguments)),
            "subcog_list" => Some(generators::generate_list_prompt(arguments)),
            // Phase 4: Intent-aware prompts
            "subcog_intent_search" => Some(generators::generate_intent_search_prompt(arguments)),
            "subcog_query_suggest" => Some(generators::generate_query_suggest_prompt(arguments)),
            "subcog_context_capture" => {
                Some(generators::generate_context_capture_prompt(arguments))
            },
            "subcog_discover" => Some(generators::generate_discover_prompt(arguments)),
            _ => None,
        }
    }
}

impl Default for PromptRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_registry_creation() {
        let registry = PromptRegistry::new();
        let prompts = registry.list_prompts();

        assert!(!prompts.is_empty());
        assert!(registry.get_prompt("subcog_tutorial").is_some());
        assert!(registry.get_prompt("subcog_capture_assistant").is_some());
    }

    #[test]
    fn test_prompt_definitions() {
        let registry = PromptRegistry::new();

        let tutorial = registry.get_prompt("subcog_tutorial").unwrap();
        assert_eq!(tutorial.name, "subcog_tutorial");
        assert!(tutorial.description.is_some());
        assert!(!tutorial.arguments.is_empty());
    }

    #[test]
    fn test_generate_tutorial_prompt() {
        let registry = PromptRegistry::new();

        let args = serde_json::json!({
            "familiarity": "beginner",
            "focus": "capture"
        });

        let messages = registry
            .get_prompt_messages("subcog_tutorial", &args)
            .unwrap();

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");

        if let PromptContent::Text { text } = &messages[1].content {
            assert!(text.contains("Capturing Memories"));
        }
    }

    #[test]
    fn test_generate_decision_prompt() {
        let registry = PromptRegistry::new();

        let args = serde_json::json!({
            "decision": "Use PostgreSQL",
            "alternatives": "MySQL, SQLite"
        });

        let messages = registry
            .get_prompt_messages("subcog_document_decision", &args)
            .unwrap();

        assert_eq!(messages.len(), 1);
        if let PromptContent::Text { text } = &messages[0].content {
            assert!(text.contains("PostgreSQL"));
            assert!(text.contains("MySQL"));
        }
    }

    #[test]
    fn test_unknown_prompt() {
        let registry = PromptRegistry::new();

        let result = registry.get_prompt_messages("unknown_prompt", &serde_json::json!({}));
        assert!(result.is_none());
    }

    #[test]
    fn test_familiarity_levels() {
        let registry = PromptRegistry::new();

        for level in ["beginner", "intermediate", "advanced"] {
            let args = serde_json::json!({ "familiarity": level });
            let messages = registry
                .get_prompt_messages("subcog_tutorial", &args)
                .unwrap();

            if let PromptContent::Text { text } = &messages[1].content {
                // Each level should have different intro text
                assert!(!text.is_empty());
            }
        }
    }

    // Phase 4: Intent-aware prompt tests

    #[test]
    fn test_intent_search_prompt() {
        let registry = PromptRegistry::new();

        let prompt = registry.get_prompt("subcog_intent_search").unwrap();
        assert_eq!(prompt.name, "subcog_intent_search");
        assert!(prompt.description.is_some());

        let args = serde_json::json!({
            "query": "authentication handling",
            "context": "working on login flow"
        });

        let messages = registry
            .get_prompt_messages("subcog_intent_search", &args)
            .unwrap();

        assert_eq!(messages.len(), 2);
        if let PromptContent::Text { text } = &messages[0].content {
            assert!(text.contains("authentication handling"));
            assert!(text.contains("login flow"));
        }
    }

    #[test]
    fn test_query_suggest_prompt() {
        let registry = PromptRegistry::new();

        let prompt = registry.get_prompt("subcog_query_suggest").unwrap();
        assert_eq!(prompt.name, "subcog_query_suggest");

        let args = serde_json::json!({
            "topic": "error handling",
            "namespace": "patterns"
        });

        let messages = registry
            .get_prompt_messages("subcog_query_suggest", &args)
            .unwrap();

        assert_eq!(messages.len(), 2);
        if let PromptContent::Text { text } = &messages[0].content {
            assert!(text.contains("error handling"));
            assert!(text.contains("patterns"));
        }
    }

    #[test]
    fn test_context_capture_prompt() {
        let registry = PromptRegistry::new();

        let prompt = registry.get_prompt("subcog_context_capture").unwrap();
        assert_eq!(prompt.name, "subcog_context_capture");

        let args = serde_json::json!({
            "conversation": "We decided to use PostgreSQL because it has better JSON support.",
            "threshold": "0.8"
        });

        let messages = registry
            .get_prompt_messages("subcog_context_capture", &args)
            .unwrap();

        assert_eq!(messages.len(), 2);
        if let PromptContent::Text { text } = &messages[0].content {
            assert!(text.contains("PostgreSQL"));
            assert!(text.contains("0.8"));
        }
    }

    #[test]
    fn test_discover_prompt() {
        let registry = PromptRegistry::new();

        let prompt = registry.get_prompt("subcog_discover").unwrap();
        assert_eq!(prompt.name, "subcog_discover");

        let args = serde_json::json!({
            "start": "authentication",
            "depth": "3"
        });

        let messages = registry
            .get_prompt_messages("subcog_discover", &args)
            .unwrap();

        assert_eq!(messages.len(), 2);
        if let PromptContent::Text { text } = &messages[0].content {
            assert!(text.contains("authentication"));
            assert!(text.contains('3'));
        }
    }

    #[test]
    fn test_discover_prompt_no_start() {
        let registry = PromptRegistry::new();

        let args = serde_json::json!({});

        let messages = registry
            .get_prompt_messages("subcog_discover", &args)
            .unwrap();

        if let PromptContent::Text { text } = &messages[0].content {
            assert!(text.contains("overview"));
        }
    }

    #[test]
    fn test_all_phase4_prompts_registered() {
        let registry = PromptRegistry::new();

        // Verify all Phase 4 prompts are registered
        assert!(registry.get_prompt("subcog_intent_search").is_some());
        assert!(registry.get_prompt("subcog_query_suggest").is_some());
        assert!(registry.get_prompt("subcog_context_capture").is_some());
        assert!(registry.get_prompt("subcog_discover").is_some());
    }

    #[test]
    fn test_generate_tutorial_prompt_definition() {
        let registry = PromptRegistry::new();

        let prompt = registry.get_prompt("subcog_generate_tutorial").unwrap();
        assert_eq!(prompt.name, "subcog_generate_tutorial");
        assert!(prompt.description.is_some());
        assert!(
            prompt
                .description
                .as_ref()
                .unwrap()
                .contains("Generate a tutorial")
        );

        // Verify arguments
        assert_eq!(prompt.arguments.len(), 3);

        let topic_arg = prompt.arguments.iter().find(|a| a.name == "topic").unwrap();
        assert!(topic_arg.required);

        let level_arg = prompt.arguments.iter().find(|a| a.name == "level").unwrap();
        assert!(!level_arg.required);

        let format_arg = prompt
            .arguments
            .iter()
            .find(|a| a.name == "format")
            .unwrap();
        assert!(!format_arg.required);
    }

    #[test]
    fn test_generate_tutorial_prompt_messages() {
        let registry = PromptRegistry::new();

        let args = serde_json::json!({
            "topic": "error handling",
            "level": "intermediate",
            "format": "markdown"
        });

        let messages = registry
            .get_prompt_messages("subcog_generate_tutorial", &args)
            .unwrap();

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");

        if let PromptContent::Text { text } = &messages[0].content {
            assert!(text.contains("error handling"));
            assert!(text.contains("developer with some experience"));
            assert!(text.contains("subcog_recall"));
        }
    }

    #[test]
    fn test_generate_tutorial_prompt_levels() {
        let registry = PromptRegistry::new();

        // Test beginner level
        let beginner_args = serde_json::json!({ "topic": "testing", "level": "beginner" });
        let beginner_messages = registry
            .get_prompt_messages("subcog_generate_tutorial", &beginner_args)
            .unwrap();
        if let PromptContent::Text { text } = &beginner_messages[0].content {
            assert!(text.contains("new to this topic"));
        }

        // Test advanced level
        let advanced_args = serde_json::json!({ "topic": "testing", "level": "advanced" });
        let advanced_messages = registry
            .get_prompt_messages("subcog_generate_tutorial", &advanced_args)
            .unwrap();
        if let PromptContent::Text { text } = &advanced_messages[0].content {
            assert!(text.contains("experienced developer"));
        }
    }

    #[test]
    fn test_generate_tutorial_prompt_formats() {
        let registry = PromptRegistry::new();

        // Test outline format
        let outline_args = serde_json::json!({ "topic": "api design", "format": "outline" });
        let outline_messages = registry
            .get_prompt_messages("subcog_generate_tutorial", &outline_args)
            .unwrap();
        if let PromptContent::Text { text } = &outline_messages[0].content {
            assert!(text.contains("structured outline"));
        }

        // Test steps format
        let steps_args = serde_json::json!({ "topic": "api design", "format": "steps" });
        let steps_messages = registry
            .get_prompt_messages("subcog_generate_tutorial", &steps_args)
            .unwrap();
        if let PromptContent::Text { text } = &steps_messages[0].content {
            assert!(text.contains("step-by-step"));
        }
    }

    #[test]
    fn test_generate_tutorial_prompt_defaults() {
        let registry = PromptRegistry::new();

        // Only required topic, others use defaults
        let args = serde_json::json!({ "topic": "rust patterns" });
        let messages = registry
            .get_prompt_messages("subcog_generate_tutorial", &args)
            .unwrap();

        if let PromptContent::Text { text } = &messages[0].content {
            // Default level is beginner
            assert!(text.contains("new to this topic"));
            // Default format is markdown
            assert!(text.contains("full markdown"));
        }
    }
}
