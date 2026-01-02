//! Prompt message generation logic.
//!
//! Contains the implementation of prompt generation methods.

use serde_json::Value;

use super::templates::{
    BROWSE_DASHBOARD_INSTRUCTIONS, BROWSE_SYSTEM_RESPONSE, CAPTURE_ASSISTANT_SYSTEM,
    CONTEXT_CAPTURE_INSTRUCTIONS, CONTEXT_CAPTURE_RESPONSE, DISCOVER_INSTRUCTIONS,
    DISCOVER_RESPONSE, GENERATE_TUTORIAL_RESPONSE, GENERATE_TUTORIAL_STRUCTURE,
    INTENT_SEARCH_INSTRUCTIONS, INTENT_SEARCH_RESPONSE, LIST_FORMAT_INSTRUCTIONS,
    QUERY_SUGGEST_INSTRUCTIONS, QUERY_SUGGEST_RESPONSE, SEARCH_HELP_SYSTEM,
    TUTORIAL_BEST_PRACTICES, TUTORIAL_CAPTURE, TUTORIAL_NAMESPACES, TUTORIAL_OVERVIEW,
    TUTORIAL_SEARCH, TUTORIAL_WORKFLOWS,
};
use super::types::{PromptContent, PromptMessage};

/// Generator for tutorial prompts.
pub fn generate_tutorial_prompt(arguments: &Value) -> Vec<PromptMessage> {
    let familiarity = arguments
        .get("familiarity")
        .and_then(|v| v.as_str())
        .unwrap_or("beginner");

    let focus = arguments
        .get("focus")
        .and_then(|v| v.as_str())
        .unwrap_or("overview");

    let intro = match familiarity {
        "advanced" => {
            "I see you're experienced with memory systems. Let me show you Subcog's advanced features."
        },
        "intermediate" => {
            "Great, you have some familiarity with memory systems. Let me explain Subcog's key concepts."
        },
        _ => "Welcome to Subcog! I'll guide you through the basics of the memory system.",
    };

    let focus_content = match focus {
        "capture" => TUTORIAL_CAPTURE,
        "recall" | "search" => TUTORIAL_SEARCH,
        "namespaces" => TUTORIAL_NAMESPACES,
        "workflows" => TUTORIAL_WORKFLOWS,
        "best-practices" => TUTORIAL_BEST_PRACTICES,
        _ => TUTORIAL_OVERVIEW,
    };

    vec![
        PromptMessage {
            role: "user".to_string(),
            content: PromptContent::Text {
                text: format!(
                    "I'd like to learn about Subcog. My familiarity level is '{familiarity}' and I want to focus on '{focus}'."
                ),
            },
        },
        PromptMessage {
            role: "assistant".to_string(),
            content: PromptContent::Text {
                text: format!("{intro}\n\n{focus_content}"),
            },
        },
    ]
}

/// Generates the `generate_tutorial` prompt messages.
///
/// Creates a tutorial on any topic using memories as source material.
pub fn generate_generate_tutorial_messages(arguments: &Value) -> Vec<PromptMessage> {
    let topic = arguments
        .get("topic")
        .and_then(|v| v.as_str())
        .unwrap_or("general");

    let level = arguments
        .get("level")
        .and_then(|v| v.as_str())
        .unwrap_or("beginner");

    let format = arguments
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("markdown");

    let level_description = match level {
        "advanced" => "for an experienced developer who knows the fundamentals",
        "intermediate" => "for a developer with some experience",
        _ => "for someone new to this topic",
    };

    let format_instruction = match format {
        "outline" => {
            "Present the tutorial as a structured outline with main sections and sub-points."
        },
        "steps" => "Present the tutorial as numbered step-by-step instructions.",
        _ => "Present the tutorial in full markdown with headings, examples, and explanations.",
    };

    let prompt = format!(
        "Generate a comprehensive tutorial about **{topic}** {level_description}.\n\n\
        **Instructions**:\n\
        1. First, search for relevant memories using `mcp__plugin_subcog_subcog__subcog_recall` with query: \"{topic}\"\n\
        2. Incorporate insights, decisions, and patterns from the memories found\n\
        3. Structure the tutorial with clear sections\n\
        4. Include practical examples where applicable\n\
        5. Reference specific memories that inform the content\n\n\
        **Format**: {format_instruction}\n\n\
        {GENERATE_TUTORIAL_STRUCTURE}"
    );

    vec![
        PromptMessage {
            role: "user".to_string(),
            content: PromptContent::Text { text: prompt },
        },
        PromptMessage {
            role: "assistant".to_string(),
            content: PromptContent::Text {
                text: GENERATE_TUTORIAL_RESPONSE.to_string(),
            },
        },
    ]
}

/// Generates the capture assistant prompt.
pub fn generate_capture_assistant_prompt(arguments: &Value) -> Vec<PromptMessage> {
    let context = arguments
        .get("context")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    vec![
        PromptMessage {
            role: "user".to_string(),
            content: PromptContent::Text {
                text: format!(
                    "Please analyze this context and suggest what memories to capture:\n\n{context}"
                ),
            },
        },
        PromptMessage {
            role: "assistant".to_string(),
            content: PromptContent::Text {
                text: CAPTURE_ASSISTANT_SYSTEM.to_string(),
            },
        },
    ]
}

/// Generates the review prompt.
pub fn generate_review_prompt(arguments: &Value) -> Vec<PromptMessage> {
    let namespace = arguments
        .get("namespace")
        .and_then(|v| v.as_str())
        .unwrap_or("all");

    let action = arguments
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("summarize");

    vec![PromptMessage {
        role: "user".to_string(),
        content: PromptContent::Text {
            text: format!(
                "Please {action} the memories in the '{namespace}' namespace. Help me understand what we have and identify any gaps or improvements."
            ),
        },
    }]
}

/// Generates the decision documentation prompt.
pub fn generate_decision_prompt(arguments: &Value) -> Vec<PromptMessage> {
    let decision = arguments
        .get("decision")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let alternatives = arguments
        .get("alternatives")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut prompt =
        format!("I need to document the following decision:\n\n**Decision**: {decision}\n");

    if !alternatives.is_empty() {
        prompt.push_str(&format!("\n**Alternatives considered**: {alternatives}\n"));
    }

    prompt.push_str(
        "\nPlease help me document this decision in a structured way that captures:\n\
        1. The context and problem being solved\n\
        2. The decision and rationale\n\
        3. Consequences and trade-offs\n\
        4. Suggested tags for searchability",
    );

    vec![PromptMessage {
        role: "user".to_string(),
        content: PromptContent::Text { text: prompt },
    }]
}

/// Generates the search help prompt.
pub fn generate_search_help_prompt(arguments: &Value) -> Vec<PromptMessage> {
    let goal = arguments.get("goal").and_then(|v| v.as_str()).unwrap_or("");

    vec![
        PromptMessage {
            role: "user".to_string(),
            content: PromptContent::Text {
                text: format!(
                    "I'm trying to find memories related to: {goal}\n\nHelp me craft effective search queries."
                ),
            },
        },
        PromptMessage {
            role: "assistant".to_string(),
            content: PromptContent::Text {
                text: SEARCH_HELP_SYSTEM.to_string(),
            },
        },
    ]
}

/// Generates the browse prompt (discovery dashboard).
pub fn generate_browse_prompt(arguments: &Value) -> Vec<PromptMessage> {
    let filter = arguments
        .get("filter")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let view = arguments
        .get("view")
        .and_then(|v| v.as_str())
        .unwrap_or("dashboard");

    let top = arguments
        .get("top")
        .and_then(|v| v.as_str())
        .unwrap_or("10");

    let mut prompt = String::from(
        "Show me a memory browser dashboard.\n\n**IMPORTANT**: Use the `mcp__plugin_subcog_subcog__subcog_recall` tool to fetch memories with server-side filtering:\n",
    );

    if filter.is_empty() {
        prompt.push_str(
            "```json\n{ \"query\": \"*\", \"limit\": 100, \"detail\": \"medium\" }\n```\n\n",
        );
        prompt.push_str("No filters applied - show the full dashboard with:\n");
    } else {
        prompt.push_str(&format!(
            "```json\n{{ \"query\": \"*\", \"filter\": \"{filter}\", \"limit\": 100, \"detail\": \"medium\" }}\n```\n\n"
        ));
    }

    prompt.push_str(&format!(
        "View mode: {view}\nShow top {top} items per facet.\n\n"
    ));

    prompt.push_str(BROWSE_DASHBOARD_INSTRUCTIONS);

    vec![
        PromptMessage {
            role: "user".to_string(),
            content: PromptContent::Text { text: prompt },
        },
        PromptMessage {
            role: "assistant".to_string(),
            content: PromptContent::Text {
                text: BROWSE_SYSTEM_RESPONSE.to_string(),
            },
        },
    ]
}

/// Generates the list prompt (formatted inventory).
pub fn generate_list_prompt(arguments: &Value) -> Vec<PromptMessage> {
    let filter = arguments
        .get("filter")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let format = arguments
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("table");

    let limit = arguments
        .get("limit")
        .and_then(|v| v.as_str())
        .unwrap_or("50");

    let mut prompt = String::from(
        "List memories from Subcog.\n\n**IMPORTANT**: Use the `mcp__plugin_subcog_subcog__subcog_recall` tool to fetch memories with server-side filtering:\n",
    );

    if filter.is_empty() {
        prompt.push_str(&format!(
            "```json\n{{ \"query\": \"*\", \"limit\": {limit}, \"detail\": \"medium\" }}\n```\n\n"
        ));
    } else {
        prompt.push_str(&format!(
            "```json\n{{ \"query\": \"*\", \"filter\": \"{filter}\", \"limit\": {limit}, \"detail\": \"medium\" }}\n```\n\n"
        ));
    }

    prompt.push_str(&format!("Format: {format}\n\n"));

    prompt.push_str(LIST_FORMAT_INSTRUCTIONS);

    vec![PromptMessage {
        role: "user".to_string(),
        content: PromptContent::Text { text: prompt },
    }]
}

// Phase 4: Intent-aware prompt generators

/// Generates the intent search prompt.
pub fn generate_intent_search_prompt(arguments: &Value) -> Vec<PromptMessage> {
    let query = arguments
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let context = arguments
        .get("context")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut prompt = format!("Search for memories related to: **{query}**\n\n");

    if !context.is_empty() {
        prompt.push_str(&format!("Current context: {context}\n\n"));
    }

    prompt.push_str(INTENT_SEARCH_INSTRUCTIONS);

    vec![
        PromptMessage {
            role: "user".to_string(),
            content: PromptContent::Text { text: prompt },
        },
        PromptMessage {
            role: "assistant".to_string(),
            content: PromptContent::Text {
                text: INTENT_SEARCH_RESPONSE.to_string(),
            },
        },
    ]
}

/// Generates the query suggest prompt.
pub fn generate_query_suggest_prompt(arguments: &Value) -> Vec<PromptMessage> {
    let topic = arguments
        .get("topic")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let namespace = arguments
        .get("namespace")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut prompt = String::from("Help me explore my memory collection.\n\n");

    if !topic.is_empty() {
        prompt.push_str(&format!("Topic area: **{topic}**\n"));
    }

    if !namespace.is_empty() {
        prompt.push_str(&format!("Focus namespace: **{namespace}**\n"));
    }

    prompt.push('\n');
    prompt.push_str(QUERY_SUGGEST_INSTRUCTIONS);

    vec![
        PromptMessage {
            role: "user".to_string(),
            content: PromptContent::Text { text: prompt },
        },
        PromptMessage {
            role: "assistant".to_string(),
            content: PromptContent::Text {
                text: QUERY_SUGGEST_RESPONSE.to_string(),
            },
        },
    ]
}

/// Generates the context capture prompt.
pub fn generate_context_capture_prompt(arguments: &Value) -> Vec<PromptMessage> {
    let conversation = arguments
        .get("conversation")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let threshold = arguments
        .get("threshold")
        .and_then(|v| v.as_str())
        .unwrap_or("0.7");

    let prompt = format!(
        "Analyze this conversation/context and suggest memories to capture:\n\n\
        ---\n{conversation}\n---\n\n\
        Confidence threshold: {threshold}\n\n\
        {CONTEXT_CAPTURE_INSTRUCTIONS}"
    );

    vec![
        PromptMessage {
            role: "user".to_string(),
            content: PromptContent::Text { text: prompt },
        },
        PromptMessage {
            role: "assistant".to_string(),
            content: PromptContent::Text {
                text: CONTEXT_CAPTURE_RESPONSE.to_string(),
            },
        },
    ]
}

/// Generates the discover prompt.
pub fn generate_discover_prompt(arguments: &Value) -> Vec<PromptMessage> {
    let start = arguments
        .get("start")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let depth = arguments
        .get("depth")
        .and_then(|v| v.as_str())
        .unwrap_or("2");

    let mut prompt = String::from("Explore related memories and topics.\n\n");

    if start.is_empty() {
        prompt.push_str("No starting point specified - show an overview of available topics.\n");
    } else {
        prompt.push_str(&format!("Starting point: **{start}**\n"));
    }

    prompt.push_str(&format!("Exploration depth: {depth} hops\n\n"));
    prompt.push_str(DISCOVER_INSTRUCTIONS);

    vec![
        PromptMessage {
            role: "user".to_string(),
            content: PromptContent::Text { text: prompt },
        },
        PromptMessage {
            role: "assistant".to_string(),
            content: PromptContent::Text {
                text: DISCOVER_RESPONSE.to_string(),
            },
        },
    ]
}
