//! URN (Uniform Resource Name) parsing and handling.
//!
//! Subcog uses URNs to identify and filter memories. The scheme is:
//!
//! ```text
//! subcog://{domain}/{namespace}/{memory_id}
//! ```
//!
//! Where:
//! - `domain`: `project`, `user`, `org`, or `_` (wildcard)
//! - `namespace`: `decisions`, `learnings`, `patterns`, etc., or `_` (wildcard)
//! - `memory_id`: The specific memory ID (optional for filters)
//!
//! # Examples
//!
//! ```
//! use subcog::models::Urn;
//!
//! // Specific memory lookup
//! let urn = Urn::parse("subcog://project/patterns/abc123").unwrap();
//! assert_eq!(urn.memory_id(), Some("abc123"));
//!
//! // Filter by namespace (any domain)
//! let urn = Urn::parse("subcog://_/learnings").unwrap();
//! assert!(urn.domain().is_wildcard());
//! assert_eq!(urn.namespace_str(), Some("learnings"));
//!
//! // Filter by domain (any namespace)
//! let urn = Urn::parse("subcog://project/_").unwrap();
//! assert_eq!(urn.domain_str(), Some("project"));
//! assert!(urn.namespace().is_wildcard());
//! ```

use crate::models::{Domain, Namespace};
use crate::{Error, Result};
use std::fmt;
use std::str::FromStr;

/// A parsed Subcog URN.
///
/// URNs can represent either a specific memory or a filter pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Urn {
    /// Domain component (project, user, org, or wildcard).
    domain: UrnComponent,
    /// Namespace component (decisions, learnings, etc., or wildcard).
    namespace: UrnComponent,
    /// Optional memory ID for specific lookups.
    memory_id: Option<String>,
    /// Original URN string for display.
    original: String,
}

/// A component of a URN that can be a specific value or a wildcard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrnComponent {
    /// A specific value.
    Value(String),
    /// Wildcard (`_`) - matches any value.
    Wildcard,
}

impl UrnComponent {
    /// Returns `true` if this is a wildcard.
    #[must_use]
    pub const fn is_wildcard(&self) -> bool {
        matches!(self, Self::Wildcard)
    }

    /// Returns the value if this is not a wildcard.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Value(s) => Some(s),
            Self::Wildcard => None,
        }
    }
}

impl Urn {
    /// Parses a URN string.
    ///
    /// # Errors
    ///
    /// Returns an error if the string is not a valid Subcog URN.
    ///
    /// # Examples
    ///
    /// ```
    /// use subcog::models::Urn;
    ///
    /// // Full URN with memory ID
    /// let urn = Urn::parse("subcog://project/patterns/abc123")?;
    ///
    /// // Namespace filter
    /// let urn = Urn::parse("subcog://_/learnings")?;
    ///
    /// // Domain filter
    /// let urn = Urn::parse("subcog://user/_")?;
    /// # Ok::<(), subcog::Error>(())
    /// ```
    pub fn parse(s: &str) -> Result<Self> {
        let original = s.to_string();

        // Must start with subcog://
        let path = s
            .strip_prefix("subcog://")
            .ok_or_else(|| Error::InvalidInput(format!("URN must start with 'subcog://': {s}")))?;

        // Split into components
        let parts: Vec<&str> = path.split('/').collect();

        if parts.is_empty() || parts.len() > 3 {
            return Err(Error::InvalidInput(format!(
                "URN must have 1-3 path components (domain/namespace/id): {s}"
            )));
        }

        // Parse domain (first component)
        let domain = Self::parse_component(parts[0]);

        // Parse namespace (second component, if present)
        let namespace = if parts.len() > 1 {
            Self::parse_component(parts[1])
        } else {
            UrnComponent::Wildcard
        };

        // Parse memory_id (third component, if present)
        let memory_id = if parts.len() > 2 && !parts[2].is_empty() && parts[2] != "_" {
            Some(parts[2].to_string())
        } else {
            None
        };

        Ok(Self {
            domain,
            namespace,
            memory_id,
            original,
        })
    }

    /// Parses a component, treating `_` or empty as wildcard.
    fn parse_component(s: &str) -> UrnComponent {
        if s.is_empty() || s == "_" {
            UrnComponent::Wildcard
        } else {
            UrnComponent::Value(s.to_string())
        }
    }

    /// Tries to parse a string as a URN, returning `None` if it's not a URN.
    ///
    /// This is useful for checking if a `memory_id` argument is a URN or a raw ID.
    #[must_use]
    pub fn try_parse(s: &str) -> Option<Self> {
        if s.starts_with("subcog://") {
            Self::parse(s).ok()
        } else {
            None
        }
    }

    /// Extracts just the memory ID from a string that might be a URN.
    ///
    /// If the string is a URN, returns the `memory_id` component (last path segment).
    /// If the string is not a URN, returns it as-is (it's already a raw ID).
    #[must_use]
    pub fn extract_memory_id(s: &str) -> &str {
        if !s.starts_with("subcog://") {
            return s;
        }
        // Extract the last path segment as the memory ID
        let Some(last_slash) = s.rfind('/') else {
            return s;
        };
        let id = &s[last_slash + 1..];
        if id.is_empty() || id == "_" {
            return s;
        }
        id
    }

    /// Extracts just the memory ID from a string, returning owned String.
    ///
    /// If the string is a URN with a `memory_id`, returns that ID.
    /// If the string is a URN without a `memory_id` (filter), returns `None`.
    /// If the string is not a URN, returns it as-is (it's already a raw ID).
    #[must_use]
    pub fn extract_memory_id_owned(s: &str) -> Option<String> {
        if s.starts_with("subcog://") {
            // Parse as URN
            Self::parse(s).ok().and_then(|urn| urn.memory_id)
        } else {
            // Raw ID
            Some(s.to_string())
        }
    }

    /// Returns `true` if this URN represents a specific memory (has a `memory_id`).
    #[must_use]
    pub const fn is_specific(&self) -> bool {
        self.memory_id.is_some()
    }

    /// Returns `true` if this URN is a filter pattern (no `memory_id`, or has wildcards).
    #[must_use]
    pub const fn is_filter(&self) -> bool {
        self.memory_id.is_none() || self.domain.is_wildcard() || self.namespace.is_wildcard()
    }

    /// Returns the domain component.
    #[must_use]
    pub const fn domain(&self) -> &UrnComponent {
        &self.domain
    }

    /// Returns the domain as a string, if not a wildcard.
    #[must_use]
    pub fn domain_str(&self) -> Option<&str> {
        self.domain.as_str()
    }

    /// Returns the namespace component.
    #[must_use]
    pub const fn namespace(&self) -> &UrnComponent {
        &self.namespace
    }

    /// Returns the namespace as a string, if not a wildcard.
    #[must_use]
    pub fn namespace_str(&self) -> Option<&str> {
        self.namespace.as_str()
    }

    /// Returns the memory ID, if this URN specifies one.
    #[must_use]
    pub fn memory_id(&self) -> Option<&str> {
        self.memory_id.as_deref()
    }

    /// Converts the domain component to a `Domain`, if not a wildcard.
    ///
    /// # Errors
    ///
    /// Returns an error if the domain string is invalid.
    pub fn to_domain(&self) -> Result<Option<Domain>> {
        match &self.domain {
            UrnComponent::Wildcard => Ok(None),
            UrnComponent::Value(s) => {
                // Map domain strings to Domain enum
                match s.as_str() {
                    "project" => Ok(Some(Domain::default_for_context())),
                    "user" => Ok(Some(Domain::for_user())),
                    "org" => Ok(Some(Domain::for_org())),
                    _ => Err(Error::InvalidInput(format!("Unknown domain: {s}"))),
                }
            },
        }
    }

    /// Converts the namespace component to a `Namespace`, if not a wildcard.
    #[must_use]
    pub fn to_namespace(&self) -> Option<Namespace> {
        match &self.namespace {
            UrnComponent::Wildcard => None,
            UrnComponent::Value(s) => Namespace::from_str(s).ok(),
        }
    }

    /// Returns the original URN string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.original
    }
}

impl fmt::Display for Urn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.original)
    }
}

impl FromStr for Urn {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_urn() {
        let urn = Urn::parse("subcog://project/patterns/abc123").unwrap();
        assert_eq!(urn.domain_str(), Some("project"));
        assert_eq!(urn.namespace_str(), Some("patterns"));
        assert_eq!(urn.memory_id(), Some("abc123"));
        assert!(urn.is_specific());
        assert!(!urn.domain().is_wildcard());
        assert!(!urn.namespace().is_wildcard());
    }

    #[test]
    fn test_parse_namespace_filter() {
        let urn = Urn::parse("subcog://_/learnings").unwrap();
        assert!(urn.domain().is_wildcard());
        assert_eq!(urn.namespace_str(), Some("learnings"));
        assert!(urn.memory_id().is_none());
        assert!(urn.is_filter());
    }

    #[test]
    fn test_parse_domain_filter() {
        let urn = Urn::parse("subcog://project/_").unwrap();
        assert_eq!(urn.domain_str(), Some("project"));
        assert!(urn.namespace().is_wildcard());
        assert!(urn.memory_id().is_none());
        assert!(urn.is_filter());
    }

    #[test]
    fn test_parse_user_decisions() {
        let urn = Urn::parse("subcog://user/decisions/_").unwrap();
        assert_eq!(urn.domain_str(), Some("user"));
        assert_eq!(urn.namespace_str(), Some("decisions"));
        assert!(urn.memory_id().is_none());
        assert!(urn.is_filter());
    }

    #[test]
    fn test_parse_all_wildcard() {
        let urn = Urn::parse("subcog://_/_").unwrap();
        assert!(urn.domain().is_wildcard());
        assert!(urn.namespace().is_wildcard());
        assert!(urn.memory_id().is_none());
        assert!(urn.is_filter());
    }

    #[test]
    fn test_extract_memory_id_from_urn() {
        assert_eq!(
            Urn::extract_memory_id("subcog://project/patterns/abc123"),
            "abc123"
        );
    }

    #[test]
    fn test_extract_memory_id_raw() {
        assert_eq!(Urn::extract_memory_id("abc123"), "abc123");
    }

    #[test]
    fn test_extract_memory_id_owned_from_urn() {
        assert_eq!(
            Urn::extract_memory_id_owned("subcog://project/patterns/abc123"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn test_extract_memory_id_owned_filter() {
        assert_eq!(Urn::extract_memory_id_owned("subcog://_/learnings"), None);
    }

    #[test]
    fn test_extract_memory_id_owned_raw() {
        assert_eq!(
            Urn::extract_memory_id_owned("abc123"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn test_try_parse_urn() {
        assert!(Urn::try_parse("subcog://project/patterns/abc").is_some());
        assert!(Urn::try_parse("abc123").is_none());
        assert!(Urn::try_parse("not-a-urn").is_none());
    }

    #[test]
    fn test_invalid_urn_no_prefix() {
        assert!(Urn::parse("project/patterns/abc").is_err());
    }

    #[test]
    fn test_to_namespace() {
        let urn = Urn::parse("subcog://project/decisions/abc").unwrap();
        assert_eq!(urn.to_namespace(), Some(Namespace::Decisions));

        let urn = Urn::parse("subcog://project/_/abc").unwrap();
        assert!(urn.to_namespace().is_none());
    }

    #[test]
    fn test_display() {
        let urn = Urn::parse("subcog://project/patterns/abc123").unwrap();
        assert_eq!(urn.to_string(), "subcog://project/patterns/abc123");
    }
}
