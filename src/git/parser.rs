//! YAML front matter parsing.
//!
//! Parses and serializes YAML front matter in memory content.
//! Front matter format:
//! ```text
//! ---
//! namespace: decisions
//! domain: org/repo
//! tags: [rust, architecture]
//! ---
//! The actual memory content here.
//! ```

use crate::{Error, Result};

/// Parser for YAML front matter in memory content.
pub struct YamlFrontMatterParser;

impl YamlFrontMatterParser {
    /// The front matter delimiter.
    const DELIMITER: &'static str = "---";

    /// Parses YAML front matter from content.
    ///
    /// Returns the parsed metadata and remaining content.
    ///
    /// # Errors
    ///
    /// Returns an error if the YAML is malformed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use subcog::git::YamlFrontMatterParser;
    ///
    /// let content = "---\nnamespace: decisions\n---\nActual content";
    /// let (metadata, body) = YamlFrontMatterParser::parse(content).unwrap();
    /// assert_eq!(metadata["namespace"], "decisions");
    /// assert_eq!(body, "Actual content");
    /// ```
    pub fn parse(content: &str) -> Result<(serde_json::Value, String)> {
        let content = content.trim_start();

        // Check if content starts with front matter delimiter
        if !content.starts_with(Self::DELIMITER) {
            // No front matter, return empty metadata and original content
            return Ok((
                serde_json::Value::Object(serde_json::Map::new()),
                content.to_string(),
            ));
        }

        // Find the end of front matter
        let after_first = &content[Self::DELIMITER.len()..];
        let after_first = after_first.trim_start_matches(['\r', '\n']);

        if let Some(end_pos) = after_first.find(Self::DELIMITER) {
            let yaml_content = &after_first[..end_pos].trim();
            let body_start = end_pos + Self::DELIMITER.len();
            let body = after_first[body_start..].trim_start_matches(['\r', '\n']);

            // Parse YAML to serde_json::Value
            let metadata: serde_json::Value = serde_yaml::from_str(yaml_content)
                .map_err(|e| Error::InvalidInput(format!("Invalid YAML front matter: {e}")))?;

            Ok((metadata, body.to_string()))
        } else {
            // No closing delimiter found
            Err(Error::InvalidInput(
                "Front matter missing closing delimiter".to_string(),
            ))
        }
    }

    /// Serializes metadata to YAML front matter format.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use subcog::git::YamlFrontMatterParser;
    /// use serde_json::json;
    ///
    /// let metadata = json!({"namespace": "decisions"});
    /// let result = YamlFrontMatterParser::serialize(&metadata, "Content here").unwrap();
    /// assert!(result.contains("---"));
    /// assert!(result.contains("namespace: decisions"));
    /// assert!(result.contains("Content here"));
    /// ```
    pub fn serialize(metadata: &serde_json::Value, content: &str) -> Result<String> {
        // If metadata is empty, just return content
        if metadata.is_null()
            || (metadata.is_object() && metadata.as_object().is_some_and(serde_json::Map::is_empty))
        {
            return Ok(content.to_string());
        }

        let yaml = serde_yaml::to_string(metadata).map_err(|e| Error::OperationFailed {
            operation: "serialize_yaml".to_string(),
            cause: e.to_string(),
        })?;

        Ok(format!(
            "{}\n{}{}\n{}",
            Self::DELIMITER,
            yaml,
            Self::DELIMITER,
            content
        ))
    }

    /// Extracts just the body content without parsing metadata.
    #[must_use]
    pub fn extract_body(content: &str) -> &str {
        let content = content.trim_start();

        if !content.starts_with(Self::DELIMITER) {
            return content;
        }

        let after_first = &content[Self::DELIMITER.len()..];
        let after_first = after_first.trim_start_matches(['\r', '\n']);

        after_first
            .find(Self::DELIMITER)
            .map_or(content, |end_pos| {
                let body_start = end_pos + Self::DELIMITER.len();
                after_first[body_start..].trim_start_matches(['\r', '\n'])
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_with_front_matter() {
        let content = "---\nnamespace: decisions\ntags:\n  - rust\n  - arch\n---\nThe content.";
        let (metadata, body) = YamlFrontMatterParser::parse(content).unwrap();

        assert_eq!(metadata["namespace"], "decisions");
        assert_eq!(metadata["tags"][0], "rust");
        assert_eq!(metadata["tags"][1], "arch");
        assert_eq!(body, "The content.");
    }

    #[test]
    fn test_parse_without_front_matter() {
        let content = "Just plain content";
        let (metadata, body) = YamlFrontMatterParser::parse(content).unwrap();

        assert!(metadata.is_object());
        assert!(metadata.as_object().unwrap().is_empty());
        assert_eq!(body, "Just plain content");
    }

    #[test]
    fn test_parse_missing_closing_delimiter() {
        let content = "---\nnamespace: test\nNo closing delimiter";
        let result = YamlFrontMatterParser::parse(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_serialize() {
        let metadata = json!({
            "namespace": "learnings",
            "domain": "zircote/subcog"
        });
        let content = "Learning about Rust";
        let result = YamlFrontMatterParser::serialize(&metadata, content).unwrap();

        assert!(result.starts_with("---"));
        assert!(result.contains("namespace: learnings"));
        assert!(result.contains("domain: zircote/subcog"));
        assert!(result.ends_with("Learning about Rust"));
    }

    #[test]
    fn test_serialize_empty_metadata() {
        let metadata = json!({});
        let content = "Just content";
        let result = YamlFrontMatterParser::serialize(&metadata, content).unwrap();
        assert_eq!(result, "Just content");
    }

    #[test]
    fn test_extract_body() {
        let content = "---\nfoo: bar\n---\nThe body";
        assert_eq!(YamlFrontMatterParser::extract_body(content), "The body");

        let plain = "No front matter";
        assert_eq!(
            YamlFrontMatterParser::extract_body(plain),
            "No front matter"
        );
    }

    #[test]
    fn test_roundtrip() {
        let original_meta = json!({
            "namespace": "decisions",
            "tags": ["a", "b"]
        });
        let original_body = "Decision content";

        let serialized = YamlFrontMatterParser::serialize(&original_meta, original_body).unwrap();
        let (parsed_meta, parsed_body) = YamlFrontMatterParser::parse(&serialized).unwrap();

        assert_eq!(parsed_meta["namespace"], original_meta["namespace"]);
        assert_eq!(parsed_body, original_body);
    }
}
