//! Template renderer implementation.
//!
//! Provides the core rendering engine for context templates with:
//! - Variable substitution (reuses sanitization from prompt module)
//! - Iteration support (`{{#each collection}}...{{/each}}`)
//! - Output format conversion (Markdown, JSON, XML)

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::LazyLock;

use crate::models::{
    ContextTemplate, OutputFormat, PromptVariable, TemplateVariable, extract_variables,
    substitute_variables,
};
use crate::{Error, Result};

/// Maximum number of items in an iteration (prevents denial-of-service).
const MAX_ITERATION_ITEMS: usize = 1000;

/// Maximum nesting depth for iterations (currently only 1 level supported).
const MAX_ITERATION_DEPTH: usize = 1;

/// Regex pattern for iteration blocks: `{{#each collection}}...{{/each}}`
static EACH_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{#each\s+(\w+)\}\}([\s\S]*?)\{\{/each\}\}").unwrap_or_else(|_| unreachable!())
});

/// Regex pattern for item variable references: `{{item.field}}` where item matches collection singular
static ITEM_VAR_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{(\w+)\.(\w+)\}\}").unwrap_or_else(|_| unreachable!()));

/// A value that can be rendered in a template.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RenderValue {
    /// A simple string value.
    String(String),
    /// A list of items for iteration.
    List(Vec<HashMap<String, String>>),
    /// A nested object (for statistics, etc.).
    Object(HashMap<String, String>),
}

impl RenderValue {
    /// Creates a string value.
    #[must_use]
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    /// Creates a list value.
    #[must_use]
    pub const fn list(items: Vec<HashMap<String, String>>) -> Self {
        Self::List(items)
    }

    /// Creates an object value.
    #[must_use]
    pub const fn object(fields: HashMap<String, String>) -> Self {
        Self::Object(fields)
    }

    /// Returns the value as a string, or None if not a string.
    #[must_use]
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the value as a list, or None if not a list.
    #[must_use]
    pub fn as_list(&self) -> Option<&[HashMap<String, String>]> {
        match self {
            Self::List(l) => Some(l),
            _ => None,
        }
    }

    /// Converts the value to a string representation.
    #[must_use]
    pub fn to_string_repr(&self) -> String {
        match self {
            Self::String(s) => s.clone(),
            Self::List(l) => serde_json::to_string(l).unwrap_or_else(|_| "[]".to_string()),
            Self::Object(o) => serde_json::to_string(o).unwrap_or_else(|_| "{}".to_string()),
        }
    }
}

impl From<String> for RenderValue {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for RenderValue {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl From<Vec<HashMap<String, String>>> for RenderValue {
    fn from(l: Vec<HashMap<String, String>>) -> Self {
        Self::List(l)
    }
}

/// Context for rendering a template.
#[derive(Debug, Clone, Default)]
pub struct RenderContext {
    /// Variable values keyed by name.
    values: HashMap<String, RenderValue>,
}

impl RenderContext {
    /// Creates a new empty render context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a string value to the context.
    pub fn add_string(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.values
            .insert(name.into(), RenderValue::String(value.into()));
    }

    /// Adds a list value to the context for iteration.
    pub fn add_list(&mut self, name: impl Into<String>, items: Vec<HashMap<String, String>>) {
        self.values.insert(name.into(), RenderValue::List(items));
    }

    /// Adds an object value to the context.
    pub fn add_object(&mut self, name: impl Into<String>, fields: HashMap<String, String>) {
        self.values.insert(name.into(), RenderValue::Object(fields));
    }

    /// Adds a render value to the context.
    pub fn add_value(&mut self, name: impl Into<String>, value: RenderValue) {
        self.values.insert(name.into(), value);
    }

    /// Sets a value in the context (alias for `add_value`).
    pub fn set(&mut self, name: impl Into<String>, value: RenderValue) {
        self.add_value(name, value);
    }

    /// Gets a value from the context.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&RenderValue> {
        self.values.get(name)
    }

    /// Gets a string value from the context.
    #[must_use]
    pub fn get_string(&self, name: &str) -> Option<&str> {
        self.values.get(name).and_then(RenderValue::as_string)
    }

    /// Gets a list value from the context.
    #[must_use]
    pub fn get_list(&self, name: &str) -> Option<&[HashMap<String, String>]> {
        self.values.get(name).and_then(RenderValue::as_list)
    }

    /// Returns all values as a flat string map for variable substitution.
    #[must_use]
    pub fn to_string_map(&self) -> HashMap<String, String> {
        self.values
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string_repr()))
            .collect()
    }

    /// Checks if the context contains a value.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.values.contains_key(name)
    }

    /// Returns the number of values in the context.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns true if the context is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Template rendering engine.
#[derive(Debug, Clone, Default)]
pub struct TemplateRenderer {
    _private: (), // Prevent external construction, allow future fields
}

impl TemplateRenderer {
    /// Creates a new template renderer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Renders a template with the given context and output format.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Required variables are missing
    /// - Iteration collection is not found or not a list
    /// - Format conversion fails
    #[allow(clippy::similar_names)]
    pub fn render(
        &self,
        template: &ContextTemplate,
        ctx: &RenderContext,
        format: OutputFormat,
    ) -> Result<String> {
        // 1. Process iteration blocks first
        let processed = self.process_iterations(&template.content, ctx)?;

        // 2. Build variable map for substitution
        let values = ctx.to_string_map();

        // 3. Re-extract variables from processed content (after iteration removal)
        //    This ensures we only validate variables that remain after iteration
        let remaining_vars = extract_variables(&processed);
        let prompt_vars: Vec<PromptVariable> = remaining_vars
            .into_iter()
            .map(|ev| {
                // Try to find the variable in template's definitions for metadata
                template
                    .variables
                    .iter()
                    .find(|v| v.name == ev.name)
                    .cloned()
                    .unwrap_or_else(|| TemplateVariable::new(&ev.name))
            })
            .map(Into::into)
            .collect();

        // 4. Substitute variables (reuses sanitization from prompt module)
        let rendered = substitute_variables(&processed, &values, &prompt_vars)?;

        // 5. Convert to output format
        self.format_output(&rendered, format)
    }

    /// Processes iteration blocks in the template content.
    #[allow(clippy::similar_names)]
    fn process_iterations(&self, input: &str, ctx: &RenderContext) -> Result<String> {
        let mut result = input.to_string();
        let mut depth = 0;

        // Process all {{#each}}...{{/each}} blocks
        while let Some(captures) = EACH_PATTERN.captures(&result) {
            depth += 1;
            if depth > MAX_ITERATION_DEPTH {
                return Err(Error::InvalidInput(format!(
                    "Maximum iteration depth ({MAX_ITERATION_DEPTH}) exceeded"
                )));
            }

            let full_match = captures.get(0).map_or("", |m| m.as_str());
            let collection_name = captures.get(1).map_or("", |m| m.as_str());
            let block_body = captures.get(2).map_or("", |m| m.as_str());

            // Get the collection from context
            let items = ctx.get_list(collection_name).ok_or_else(|| {
                Error::InvalidInput(format!(
                    "Iteration collection '{collection_name}' not found or not a list"
                ))
            })?;

            // Check iteration limit
            if items.len() > MAX_ITERATION_ITEMS {
                return Err(Error::InvalidInput(format!(
                    "Iteration collection '{collection_name}' has {} items, max is {MAX_ITERATION_ITEMS}",
                    items.len()
                )));
            }

            // Determine the item variable prefix (singular form of collection)
            let item_prefix = get_item_prefix(collection_name);

            // Render each item
            let rendered_items: Vec<String> = items
                .iter()
                .map(|item| self.render_iteration_item(block_body, &item_prefix, item))
                .collect();

            // Replace the {{#each}}...{{/each}} block with rendered items
            result = result.replace(full_match, &rendered_items.join(""));
        }

        Ok(result)
    }

    /// Renders a single iteration item.
    #[allow(clippy::unused_self)]
    fn render_iteration_item(
        &self,
        block: &str,
        item_prefix: &str,
        item: &HashMap<String, String>,
    ) -> String {
        let mut rendered = block.to_string();

        // Replace {{item_prefix.field}} with item values
        for (field, value) in item {
            let pattern = format!("{{{{{item_prefix}.{field}}}}}");
            rendered = rendered.replace(&pattern, value);
        }

        // Also handle {{this.field}} pattern (common Handlebars/Mustache syntax)
        for (field, value) in item {
            let pattern = format!("{{{{this.{field}}}}}");
            rendered = rendered.replace(&pattern, value);
        }

        // Also handle the generic {{prefix.field}} pattern via regex
        rendered = ITEM_VAR_PATTERN
            .replace_all(&rendered, |caps: &regex::Captures| {
                let prefix = caps.get(1).map_or("", |m| m.as_str());
                let field = caps.get(2).map_or("", |m| m.as_str());

                // Replace if prefix matches our item prefix or is "this"
                if prefix == item_prefix || prefix == "this" {
                    item.get(field).cloned().unwrap_or_default()
                } else {
                    // Leave other patterns unchanged
                    caps.get(0).map_or("", |m| m.as_str()).to_string()
                }
            })
            .to_string();

        rendered
    }

    /// Converts rendered markdown to the target output format.
    fn format_output(&self, markdown: &str, format: OutputFormat) -> Result<String> {
        match format {
            OutputFormat::Markdown => Ok(markdown.to_string()),
            OutputFormat::Json => self.markdown_to_json(markdown),
            OutputFormat::Xml => self.markdown_to_xml(markdown),
        }
    }

    /// Converts markdown to JSON format.
    ///
    /// Creates a structured JSON object with sections and content.
    #[allow(clippy::unused_self)]
    fn markdown_to_json(&self, markdown: &str) -> Result<String> {
        let mut sections: Vec<serde_json::Value> = Vec::new();
        let mut current_section: Option<(String, Vec<String>)> = None;

        for line in markdown.lines() {
            if let Some(title) = line.strip_prefix("# ") {
                flush_section(&mut sections, &mut current_section, 1);
                current_section = Some((title.to_string(), Vec::new()));
            } else if let Some(title) = line.strip_prefix("## ") {
                flush_section(&mut sections, &mut current_section, 2);
                current_section = Some((title.to_string(), Vec::new()));
            } else if let Some(title) = line.strip_prefix("### ") {
                flush_section(&mut sections, &mut current_section, 2);
                current_section = Some((title.to_string(), Vec::new()));
            } else if let Some((_, ref mut lines)) = current_section {
                lines.push(line.to_string());
            } else if !line.trim().is_empty() {
                // Content before any section
                sections.push(serde_json::json!({
                    "level": 0,
                    "title": "",
                    "content": line
                }));
            }
        }

        // Flush final section
        flush_section(&mut sections, &mut current_section, 2);

        serde_json::to_string_pretty(&serde_json::json!({
            "sections": sections,
            "raw": markdown
        }))
        .map_err(|e| Error::OperationFailed {
            operation: "json_conversion".to_string(),
            cause: e.to_string(),
        })
    }

    /// Converts markdown to XML format.
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    fn markdown_to_xml(&self, markdown: &str) -> Result<String> {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<context>\n");
        let mut state = XmlState::default();

        for line in markdown.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            process_xml_line(trimmed, &mut xml, &mut state);
        }

        // Close any open sections
        while state.level > 0 {
            let _ = writeln!(xml, "{}</section>", "  ".repeat(state.level));
            state.level -= 1;
        }

        xml.push_str("</context>");
        Ok(xml)
    }
}

/// State for XML conversion.
#[derive(Default)]
struct XmlState {
    in_section: bool,
    level: usize,
}

/// Processes a single line for XML conversion.
fn process_xml_line(trimmed: &str, xml: &mut String, state: &mut XmlState) {
    if let Some(title) = trimmed.strip_prefix("# ") {
        close_section_if_needed(xml, state);
        state.level = 1;
        state.in_section = true;
        let escaped = escape_xml(title);
        let _ = writeln!(xml, "  <section level=\"1\" title=\"{escaped}\">");
    } else if let Some(title) = trimmed.strip_prefix("## ") {
        close_section_at_level(xml, state, 2);
        state.level = 2;
        state.in_section = true;
        let escaped = escape_xml(title);
        let _ = writeln!(xml, "    <section level=\"2\" title=\"{escaped}\">");
    } else if let Some(title) = trimmed.strip_prefix("### ") {
        close_section_at_level(xml, state, 3);
        state.level = 3;
        state.in_section = true;
        let escaped = escape_xml(title);
        let _ = writeln!(xml, "      <section level=\"3\" title=\"{escaped}\">");
    } else if let Some(item_text) = trimmed.strip_prefix("- ") {
        let escaped = escape_xml(item_text);
        let indent = "  ".repeat(state.level + 1);
        let _ = writeln!(xml, "{indent}<item>{escaped}</item>");
    } else {
        let escaped = escape_xml(trimmed);
        let indent = "  ".repeat(state.level + 1);
        let _ = writeln!(xml, "{indent}<text>{escaped}</text>");
    }
}

/// Closes the current section if one is open.
fn close_section_if_needed(xml: &mut String, state: &XmlState) {
    if state.in_section {
        let _ = writeln!(xml, "{}</section>", "  ".repeat(state.level));
    }
}

/// Closes section if at or above the given level.
fn close_section_at_level(xml: &mut String, state: &XmlState, target: usize) {
    if state.in_section && state.level >= target {
        let _ = writeln!(xml, "{}</section>", "  ".repeat(state.level));
    }
}

/// Flushes a section to the sections list.
fn flush_section(
    sections: &mut Vec<serde_json::Value>,
    current: &mut Option<(String, Vec<String>)>,
    level: usize,
) {
    if let Some((title, lines)) = current.take() {
        sections.push(serde_json::json!({
            "level": level,
            "title": title,
            "content": lines.join("\n").trim()
        }));
    }
}

/// Gets the item prefix for iteration (singular form of collection name).
fn get_item_prefix(collection_name: &str) -> String {
    // Handle common plurals
    if collection_name == "memories" {
        return "memory".to_string();
    }
    if let Some(stripped) = collection_name.strip_suffix("ies") {
        // e.g., "entries" -> "entry"
        return format!("{stripped}y");
    }
    if let Some(stripped) = collection_name.strip_suffix('s') {
        // e.g., "items" -> "item"
        return stripped.to_string();
    }
    // Default: use collection name as-is
    collection_name.to_string()
}

/// Escapes special XML characters.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ContextTemplate;

    #[test]
    fn test_render_value_string() {
        let value = RenderValue::string("hello");
        assert_eq!(value.as_string(), Some("hello"));
        assert!(value.as_list().is_none());
        assert_eq!(value.to_string_repr(), "hello");
    }

    #[test]
    fn test_render_value_list() {
        let mut item = HashMap::new();
        item.insert("name".to_string(), "test".to_string());
        let value = RenderValue::list(vec![item]);

        assert!(value.as_string().is_none());
        assert!(value.as_list().is_some());
        assert_eq!(value.as_list().unwrap().len(), 1);
    }

    #[test]
    fn test_render_context_add_and_get() {
        let mut ctx = RenderContext::new();
        ctx.add_string("name", "Alice");
        ctx.add_string("count", "42");

        assert_eq!(ctx.get_string("name"), Some("Alice"));
        assert_eq!(ctx.get_string("count"), Some("42"));
        assert!(ctx.get_string("missing").is_none());
    }

    #[test]
    fn test_render_context_add_list() {
        let mut ctx = RenderContext::new();
        let mut item1 = HashMap::new();
        item1.insert("id".to_string(), "1".to_string());
        let mut item2 = HashMap::new();
        item2.insert("id".to_string(), "2".to_string());

        ctx.add_list("items", vec![item1, item2]);

        let list = ctx.get_list("items").unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_render_context_to_string_map() {
        let mut ctx = RenderContext::new();
        ctx.add_string("name", "test");
        ctx.add_string("value", "123");

        let map = ctx.to_string_map();
        assert_eq!(map.get("name"), Some(&"test".to_string()));
        assert_eq!(map.get("value"), Some(&"123".to_string()));
    }

    #[test]
    fn test_simple_variable_substitution() {
        let renderer = TemplateRenderer::new();
        let template = ContextTemplate::new("test", "Hello {{name}}!");

        let mut ctx = RenderContext::new();
        ctx.add_string("name", "World");

        let result = renderer
            .render(&template, &ctx, OutputFormat::Markdown)
            .unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_iteration_with_memories() {
        let renderer = TemplateRenderer::new();
        let template = ContextTemplate::new(
            "test",
            "Memories:\n{{#each memories}}- {{memory.content}}\n{{/each}}",
        );

        let mut ctx = RenderContext::new();
        let mut mem1 = HashMap::new();
        mem1.insert("content".to_string(), "First memory".to_string());
        let mut mem2 = HashMap::new();
        mem2.insert("content".to_string(), "Second memory".to_string());
        ctx.add_list("memories", vec![mem1, mem2]);

        let result = renderer
            .render(&template, &ctx, OutputFormat::Markdown)
            .unwrap();
        assert!(result.contains("- First memory"));
        assert!(result.contains("- Second memory"));
    }

    #[test]
    fn test_iteration_with_multiple_fields() {
        let renderer = TemplateRenderer::new();
        let template = ContextTemplate::new(
            "test",
            "{{#each memories}}**{{memory.namespace}}**: {{memory.content}} ({{memory.score}})\n{{/each}}",
        );

        let mut ctx = RenderContext::new();
        let mut mem = HashMap::new();
        mem.insert("namespace".to_string(), "decisions".to_string());
        mem.insert("content".to_string(), "Use Rust".to_string());
        mem.insert("score".to_string(), "0.95".to_string());
        ctx.add_list("memories", vec![mem]);

        let result = renderer
            .render(&template, &ctx, OutputFormat::Markdown)
            .unwrap();
        assert!(result.contains("**decisions**"));
        assert!(result.contains("Use Rust"));
        assert!(result.contains("0.95"));
    }

    #[test]
    fn test_iteration_empty_collection() {
        let renderer = TemplateRenderer::new();
        let template = ContextTemplate::new(
            "test",
            "Start\n{{#each memories}}{{memory.content}}\n{{/each}}End",
        );

        let mut ctx = RenderContext::new();
        ctx.add_list("memories", vec![]);

        let result = renderer
            .render(&template, &ctx, OutputFormat::Markdown)
            .unwrap();
        assert_eq!(result, "Start\nEnd");
    }

    #[test]
    fn test_iteration_missing_collection() {
        let renderer = TemplateRenderer::new();
        let template = ContextTemplate::new("test", "{{#each missing}}{{item.value}}{{/each}}");

        let ctx = RenderContext::new();
        let result = renderer.render(&template, &ctx, OutputFormat::Markdown);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[test]
    fn test_mixed_iteration_and_variables() {
        let renderer = TemplateRenderer::new();
        let template = ContextTemplate::new(
            "test",
            "# {{title}}\n\n{{#each items}}- {{item.name}}\n{{/each}}\n\nTotal: {{total}}",
        );

        let mut ctx = RenderContext::new();
        ctx.add_string("title", "My List");
        ctx.add_string("total", "2");

        let mut item1 = HashMap::new();
        item1.insert("name".to_string(), "Item A".to_string());
        let mut item2 = HashMap::new();
        item2.insert("name".to_string(), "Item B".to_string());
        ctx.add_list("items", vec![item1, item2]);

        let result = renderer
            .render(&template, &ctx, OutputFormat::Markdown)
            .unwrap();
        assert!(result.contains("# My List"));
        assert!(result.contains("- Item A"));
        assert!(result.contains("- Item B"));
        assert!(result.contains("Total: 2"));
    }

    #[test]
    fn test_get_item_prefix() {
        assert_eq!(get_item_prefix("memories"), "memory");
        assert_eq!(get_item_prefix("items"), "item");
        assert_eq!(get_item_prefix("entries"), "entry");
        assert_eq!(get_item_prefix("tags"), "tag");
        assert_eq!(get_item_prefix("data"), "data"); // No change for non-standard
    }

    #[test]
    fn test_format_json() {
        let renderer = TemplateRenderer::new();
        let template = ContextTemplate::new("test", "# Title\n\nContent here");

        let ctx = RenderContext::new();
        let result = renderer
            .render(&template, &ctx, OutputFormat::Json)
            .unwrap();

        // Should be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("sections").is_some());
        assert!(parsed.get("raw").is_some());
    }

    #[test]
    fn test_format_xml() {
        let renderer = TemplateRenderer::new();
        let template = ContextTemplate::new("test", "# Title\n\nContent here\n- Item 1");

        let ctx = RenderContext::new();
        let result = renderer.render(&template, &ctx, OutputFormat::Xml).unwrap();

        assert!(result.starts_with("<?xml"));
        assert!(result.contains("<context>"));
        assert!(result.contains("</context>"));
        assert!(result.contains("<section"));
        assert!(result.contains("<item>"));
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("a < b"), "a &lt; b");
        assert_eq!(escape_xml("a > b"), "a &gt; b");
        assert_eq!(escape_xml("a & b"), "a &amp; b");
        assert_eq!(escape_xml("a \"b\" c"), "a &quot;b&quot; c");
    }

    #[test]
    fn test_render_value_from_string() {
        let value: RenderValue = "test".into();
        assert_eq!(value.as_string(), Some("test"));

        let owned: RenderValue = String::from("owned").into();
        assert_eq!(owned.as_string(), Some("owned"));
    }

    #[test]
    fn test_iteration_with_this_syntax() {
        let renderer = TemplateRenderer::new();
        let template = ContextTemplate::new(
            "test",
            "## Memories\n{{#each memories}}- **{{this.namespace}}**: {{this.content}}\n{{/each}}",
        );

        let mut ctx = RenderContext::new();
        let mut mem1 = HashMap::new();
        mem1.insert("namespace".to_string(), "decisions".to_string());
        mem1.insert("content".to_string(), "Use Rust".to_string());
        let mut mem2 = HashMap::new();
        mem2.insert("namespace".to_string(), "learnings".to_string());
        mem2.insert("content".to_string(), "SQLite is fast".to_string());
        ctx.add_list("memories", vec![mem1, mem2]);

        let result = renderer
            .render(&template, &ctx, OutputFormat::Markdown)
            .unwrap();
        assert!(result.contains("**decisions**"));
        assert!(result.contains("Use Rust"));
        assert!(result.contains("**learnings**"));
        assert!(result.contains("SQLite is fast"));
    }

    #[test]
    fn test_render_context_length() {
        let mut ctx = RenderContext::new();
        assert!(ctx.is_empty());
        assert_eq!(ctx.len(), 0);

        ctx.add_string("a", "1");
        ctx.add_string("b", "2");

        assert!(!ctx.is_empty());
        assert_eq!(ctx.len(), 2);
        assert!(ctx.contains("a"));
        assert!(!ctx.contains("c"));
    }
}
