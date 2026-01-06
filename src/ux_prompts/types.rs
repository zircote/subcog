//! UX helper prompt type definitions.
//!
//! Contains the core data structures for CLI UX helper prompts.

use serde::{Deserialize, Serialize};

/// Definition of a UX helper prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptDefinition {
    /// Prompt name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Prompt arguments.
    pub arguments: Vec<PromptArgument>,
}

/// Argument for a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    /// Argument name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Whether the argument is required.
    pub required: bool,
}

/// A message in a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    /// Role: user, assistant, or system.
    pub role: String,
    /// Message content.
    pub content: PromptContent,
}

/// Content of a prompt message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PromptContent {
    /// Text content.
    Text {
        /// The text content.
        text: String,
    },
    /// Image content.
    Image {
        /// Image data (base64 or URL).
        data: String,
        /// MIME type.
        mime_type: String,
    },
    /// Resource reference.
    Resource {
        /// Resource URI.
        uri: String,
    },
}

/// Converts a user `PromptTemplate` to a `PromptDefinition`.
pub fn user_prompt_to_definition(template: &crate::models::PromptTemplate) -> PromptDefinition {
    let description = if template.description.is_empty() {
        None
    } else {
        Some(template.description.clone())
    };

    PromptDefinition {
        name: format!("user/{}", template.name),
        description,
        arguments: template
            .variables
            .iter()
            .map(|v| PromptArgument {
                name: v.name.clone(),
                description: v.description.clone(),
                required: v.required,
            })
            .collect(),
    }
}
