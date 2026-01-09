//! CLI command for listing memory namespaces.

use crate::models::Namespace;
use serde::Serialize;
use std::io::{self, Write};
use std::str::FromStr;

/// Information about a namespace.
#[derive(Debug, Clone, Serialize)]
pub struct NamespaceInfo {
    /// Namespace identifier.
    pub namespace: String,
    /// Description of the namespace.
    pub description: String,
    /// Signal words that trigger this namespace.
    pub signal_words: Vec<String>,
}

impl NamespaceInfo {
    /// Creates a new namespace info.
    fn new(namespace: Namespace, description: &str, signal_words: &[&str]) -> Self {
        Self {
            namespace: namespace.to_string(),
            description: description.to_string(),
            signal_words: signal_words.iter().map(|s| (*s).to_string()).collect(),
        }
    }
}

/// Returns all namespace information.
#[must_use]
pub fn get_all_namespaces() -> Vec<NamespaceInfo> {
    vec![
        NamespaceInfo::new(
            Namespace::Decisions,
            "Architectural and design decisions",
            &["decided", "chose", "going with"],
        ),
        NamespaceInfo::new(
            Namespace::Patterns,
            "Discovered patterns and conventions",
            &["always", "never", "convention"],
        ),
        NamespaceInfo::new(
            Namespace::Learnings,
            "Lessons learned from debugging",
            &["TIL", "learned", "discovered"],
        ),
        NamespaceInfo::new(
            Namespace::Context,
            "Important background information",
            &["because", "constraint", "requirement"],
        ),
        NamespaceInfo::new(
            Namespace::TechDebt,
            "Technical debt tracking",
            &["TODO", "FIXME", "temporary", "hack"],
        ),
        NamespaceInfo::new(
            Namespace::Blockers,
            "Blockers and impediments",
            &["blocked", "waiting", "depends on"],
        ),
        NamespaceInfo::new(
            Namespace::Progress,
            "Work progress and milestones",
            &["completed", "milestone", "shipped"],
        ),
        NamespaceInfo::new(
            Namespace::Apis,
            "API documentation and contracts",
            &["endpoint", "request", "response"],
        ),
        NamespaceInfo::new(
            Namespace::Config,
            "Configuration details",
            &["environment", "setting", "variable"],
        ),
        NamespaceInfo::new(
            Namespace::Security,
            "Security findings and notes",
            &["vulnerability", "CVE", "auth"],
        ),
        NamespaceInfo::new(
            Namespace::Testing,
            "Test strategies and edge cases",
            &["test", "edge case", "coverage"],
        ),
    ]
}

/// Output format for namespaces command.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NamespacesOutputFormat {
    /// Table format (default).
    #[default]
    Table,
    /// JSON format.
    Json,
    /// YAML format.
    Yaml,
}

impl FromStr for NamespacesOutputFormat {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "json" => Self::Json,
            "yaml" => Self::Yaml,
            _ => Self::Table,
        })
    }
}

/// Writes namespaces as a table to the given writer.
///
/// # Errors
///
/// Returns an error if writing fails.
pub fn write_table<W: Write>(
    writer: &mut W,
    namespaces: &[NamespaceInfo],
    verbose: bool,
) -> io::Result<()> {
    if verbose {
        writeln!(
            writer,
            "{:<14}{:<38}SIGNAL WORDS",
            "NAMESPACE", "DESCRIPTION"
        )?;
        for ns in namespaces {
            writeln!(
                writer,
                "{:<14}{:<38}{}",
                ns.namespace,
                ns.description,
                ns.signal_words.join(", ")
            )?;
        }
    } else {
        writeln!(writer, "{:<14}DESCRIPTION", "NAMESPACE")?;
        for ns in namespaces {
            writeln!(writer, "{:<14}{}", ns.namespace, ns.description)?;
        }
    }
    Ok(())
}

/// Writes namespaces as JSON to the given writer.
///
/// # Errors
///
/// Returns an error if serialization or writing fails.
pub fn write_json<W: Write>(
    writer: &mut W,
    namespaces: &[NamespaceInfo],
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(namespaces)?;
    writeln!(writer, "{json}")?;
    Ok(())
}

/// Writes namespaces as YAML to the given writer.
///
/// # Errors
///
/// Returns an error if serialization or writing fails.
pub fn write_yaml<W: Write>(
    writer: &mut W,
    namespaces: &[NamespaceInfo],
) -> Result<(), Box<dyn std::error::Error>> {
    let yaml = serde_yaml_ng::to_string(namespaces)?;
    write!(writer, "{yaml}")?;
    Ok(())
}

/// Executes the namespaces command.
///
/// # Errors
///
/// Returns an error if serialization or output fails.
pub fn cmd_namespaces(
    format: NamespacesOutputFormat,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let namespaces = get_all_namespaces();
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    match format {
        NamespacesOutputFormat::Table => {
            write_table(&mut handle, &namespaces, verbose)?;
            Ok(())
        },
        NamespacesOutputFormat::Json => write_json(&mut handle, &namespaces),
        NamespacesOutputFormat::Yaml => write_yaml(&mut handle, &namespaces),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_all_namespaces() {
        let namespaces = get_all_namespaces();
        assert_eq!(namespaces.len(), 11);

        // Verify first namespace
        assert_eq!(namespaces[0].namespace, "decisions");
        assert_eq!(
            namespaces[0].description,
            "Architectural and design decisions"
        );
        assert!(!namespaces[0].signal_words.is_empty());
    }

    #[test]
    fn test_output_format_from_str() {
        assert_eq!(
            NamespacesOutputFormat::from_str("json").unwrap(),
            NamespacesOutputFormat::Json
        );
        assert_eq!(
            NamespacesOutputFormat::from_str("JSON").unwrap(),
            NamespacesOutputFormat::Json
        );
        assert_eq!(
            NamespacesOutputFormat::from_str("yaml").unwrap(),
            NamespacesOutputFormat::Yaml
        );
        assert_eq!(
            NamespacesOutputFormat::from_str("table").unwrap(),
            NamespacesOutputFormat::Table
        );
        assert_eq!(
            NamespacesOutputFormat::from_str("invalid").unwrap(),
            NamespacesOutputFormat::Table
        );
    }

    #[test]
    fn test_all_namespaces_have_signal_words() {
        let namespaces = get_all_namespaces();
        for ns in namespaces {
            assert!(
                !ns.signal_words.is_empty(),
                "Namespace {} should have signal words",
                ns.namespace
            );
        }
    }

    #[test]
    fn test_namespace_descriptions_not_empty() {
        let namespaces = get_all_namespaces();
        for ns in namespaces {
            assert!(
                !ns.description.is_empty(),
                "Namespace {} should have a description",
                ns.namespace
            );
        }
    }

    #[test]
    fn test_write_table_simple() {
        let namespaces = get_all_namespaces();
        let mut buffer = Vec::new();
        write_table(&mut buffer, &namespaces, false).unwrap();
        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("NAMESPACE"));
        assert!(output.contains("DESCRIPTION"));
        assert!(output.contains("decisions"));
    }

    #[test]
    fn test_write_table_verbose() {
        let namespaces = get_all_namespaces();
        let mut buffer = Vec::new();
        write_table(&mut buffer, &namespaces, true).unwrap();
        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("SIGNAL WORDS"));
        assert!(output.contains("decided, chose, going with"));
    }

    #[test]
    fn test_write_json() {
        let namespaces = get_all_namespaces();
        let mut buffer = Vec::new();
        write_json(&mut buffer, &namespaces).unwrap();
        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("\"namespace\""));
        assert!(output.contains("\"decisions\""));
    }

    #[test]
    fn test_write_yaml() {
        let namespaces = get_all_namespaces();
        let mut buffer = Vec::new();
        write_yaml(&mut buffer, &namespaces).unwrap();
        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("namespace:"));
        assert!(output.contains("decisions"));
    }
}
