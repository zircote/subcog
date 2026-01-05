//! MCP method dispatch using command pattern.
//!
//! This module implements a command pattern for MCP method dispatch,
//! replacing string matching with type-safe enum variants.
//!
//! # Architecture
//!
//! ```text
//! McpMethod (enum)
//!   ├── Initialize
//!   ├── ListTools
//!   ├── CallTool
//!   ├── ListResources
//!   ├── ReadResource
//!   ├── ListPrompts
//!   ├── GetPrompt
//!   ├── Ping
//!   └── Unknown(String)
//! ```
//!
//! # Open/Closed Principle
//!
//! To add a new method:
//! 1. Add a variant to [`McpMethod`]
//! 2. Update [`McpMethod::from_str`] parsing
//! 3. Add handler in [`McpServer::dispatch_method`]
//!
//! The dispatch logic is centralized and type-safe.

use std::fmt;

/// MCP method identifier.
///
/// Represents all supported MCP protocol methods with type-safe variants.
/// Unknown methods are captured for proper error reporting.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum McpMethod {
    /// Initialize the MCP session.
    Initialize,
    /// List available tools.
    ListTools,
    /// Call a specific tool.
    CallTool,
    /// List available resources.
    ListResources,
    /// Read a specific resource.
    ReadResource,
    /// List available prompts.
    ListPrompts,
    /// Get a specific prompt.
    GetPrompt,
    /// Ping the server (health check).
    Ping,
    /// Unknown method (for error handling).
    Unknown(String),
}

impl McpMethod {
    /// Returns the MCP protocol method name.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        match self {
            Self::Initialize => "initialize",
            Self::ListTools => "tools/list",
            Self::CallTool => "tools/call",
            Self::ListResources => "resources/list",
            Self::ReadResource => "resources/read",
            Self::ListPrompts => "prompts/list",
            Self::GetPrompt => "prompts/get",
            Self::Ping => "ping",
            Self::Unknown(s) => s.as_str(),
        }
    }

    /// Returns true if this is a known method.
    #[must_use]
    #[allow(dead_code)] // Useful for introspection and testing
    pub const fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown(_))
    }

    /// Returns all known methods.
    #[must_use]
    #[allow(dead_code)] // Useful for introspection and testing
    pub const fn known_methods() -> &'static [Self] {
        &[
            Self::Initialize,
            Self::ListTools,
            Self::CallTool,
            Self::ListResources,
            Self::ReadResource,
            Self::ListPrompts,
            Self::GetPrompt,
            Self::Ping,
        ]
    }
}

impl From<&str> for McpMethod {
    fn from(s: &str) -> Self {
        match s {
            "initialize" => Self::Initialize,
            "tools/list" => Self::ListTools,
            "tools/call" => Self::CallTool,
            "resources/list" => Self::ListResources,
            "resources/read" => Self::ReadResource,
            "prompts/list" => Self::ListPrompts,
            "prompts/get" => Self::GetPrompt,
            "ping" => Self::Ping,
            unknown => Self::Unknown(unknown.to_string()),
        }
    }
}

impl fmt::Display for McpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_method_from_str() {
        assert_eq!(McpMethod::from("initialize"), McpMethod::Initialize);
        assert_eq!(McpMethod::from("tools/list"), McpMethod::ListTools);
        assert_eq!(McpMethod::from("tools/call"), McpMethod::CallTool);
        assert_eq!(McpMethod::from("resources/list"), McpMethod::ListResources);
        assert_eq!(McpMethod::from("resources/read"), McpMethod::ReadResource);
        assert_eq!(McpMethod::from("prompts/list"), McpMethod::ListPrompts);
        assert_eq!(McpMethod::from("prompts/get"), McpMethod::GetPrompt);
        assert_eq!(McpMethod::from("ping"), McpMethod::Ping);
    }

    #[test]
    fn test_unknown_method() {
        let method = McpMethod::from("unknown/method");
        assert!(!method.is_known());
        assert_eq!(method.as_str(), "unknown/method");
    }

    #[test]
    fn test_method_as_str_roundtrip() {
        for method in McpMethod::known_methods() {
            let s = method.as_str();
            let parsed = McpMethod::from(s);
            assert_eq!(&parsed, method, "Roundtrip failed for {method}");
        }
    }

    #[test]
    fn test_method_display() {
        assert_eq!(format!("{}", McpMethod::Initialize), "initialize");
        assert_eq!(format!("{}", McpMethod::ListTools), "tools/list");
        assert_eq!(format!("{}", McpMethod::Unknown("foo".to_string())), "foo");
    }

    #[test]
    fn test_known_methods_count() {
        // Ensure we have all 8 known methods
        assert_eq!(McpMethod::known_methods().len(), 8);
    }

    #[test]
    fn test_all_known_methods_are_known() {
        for method in McpMethod::known_methods() {
            assert!(method.is_known(), "{method} should be known");
        }
    }
}
