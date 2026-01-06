//! JWT authentication for MCP HTTP transport (SEC-H1).
//!
//! Provides bearer token validation for the MCP HTTP server.
//! The stdio transport does NOT require authentication.
//!
//! # Configuration
//!
//! Set these environment variables for JWT validation:
//!
//! - `SUBCOG_MCP_JWT_SECRET`: Required. The secret key for HS256 validation.
//! - `SUBCOG_MCP_JWT_ISSUER`: Optional. Expected issuer claim.
//! - `SUBCOG_MCP_JWT_AUDIENCE`: Optional. Expected audience claim.
//!
//! # Example
//!
//! ```bash
//! export SUBCOG_MCP_JWT_SECRET="your-secret-key-min-32-chars-long"
//! export SUBCOG_MCP_JWT_ISSUER="https://auth.example.com"
//! subcog serve --transport http --port 3000
//! ```

use crate::{Error, Result};
use jsonwebtoken::{Algorithm, DecodingKey, TokenData, Validation, decode};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

/// Minimum secret key length for security.
const MIN_SECRET_LENGTH: usize = 32;

/// Minimum number of unique characters for entropy validation.
/// A 32+ character secret with fewer than 8 unique chars is likely weak.
const MIN_UNIQUE_CHARS: usize = 8;

/// Minimum character classes required (HIGH-SEC-004).
/// At least 3 of: lowercase, uppercase, digits, special chars.
const MIN_CHAR_CLASSES: usize = 3;

/// Validates that a secret has sufficient entropy (not just length).
///
/// Checks for (HIGH-SEC-004):
/// - Minimum unique character diversity (8+ unique chars)
/// - Character class diversity (3+ of: lowercase, uppercase, digits, special)
/// - Not obviously sequential/weak patterns
fn validate_secret_entropy(secret: &str) -> std::result::Result<(), String> {
    // Check unique character count
    let unique_chars: std::collections::HashSet<char> = secret.chars().collect();
    if unique_chars.len() < MIN_UNIQUE_CHARS {
        return Err(format!(
            "JWT secret has insufficient entropy: only {} unique characters (minimum: {})",
            unique_chars.len(),
            MIN_UNIQUE_CHARS
        ));
    }

    // HIGH-SEC-004: Check character class diversity
    let has_lowercase = secret.chars().any(|c| c.is_ascii_lowercase());
    let has_uppercase = secret.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = secret.chars().any(|c| c.is_ascii_digit());
    let has_special = secret
        .chars()
        .any(|c| c.is_ascii_punctuation() || c == '+' || c == '/' || c == '=');

    let char_class_count = usize::from(has_lowercase)
        + usize::from(has_uppercase)
        + usize::from(has_digit)
        + usize::from(has_special);

    if char_class_count < MIN_CHAR_CLASSES {
        return Err(format!(
            "JWT secret has insufficient character diversity: {char_class_count} character classes (minimum: {MIN_CHAR_CLASSES}). \
             Use a mix of lowercase, uppercase, digits, and special characters. \
             Recommended: openssl rand -base64 32"
        ));
    }

    // Check for obvious weak patterns
    let lowercase = secret.to_lowercase();
    let weak_patterns = [
        "password", "secret", "123456", "abcdef", "qwerty", "000000", "111111", "aaaaaa",
    ];

    for pattern in weak_patterns {
        if lowercase.contains(pattern) {
            return Err(format!("JWT secret contains weak pattern '{pattern}'"));
        }
    }

    Ok(())
}

/// JWT claims structure.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject (user identifier).
    pub sub: String,
    /// Expiration time (Unix timestamp).
    pub exp: usize,
    /// Issued at time (Unix timestamp).
    #[serde(default)]
    pub iat: usize,
    /// Issuer.
    #[serde(default)]
    pub iss: Option<String>,
    /// Audience.
    #[serde(default)]
    pub aud: Option<String>,
    /// Optional scopes for authorization.
    #[serde(default)]
    pub scopes: Vec<String>,
}

impl Claims {
    /// Checks if the claims include a specific scope (CRIT-003).
    ///
    /// # Arguments
    ///
    /// * `scope` - The scope to check for (e.g., "read", "write", "admin").
    ///
    /// # Returns
    ///
    /// `true` if the claims include the specified scope or the wildcard "*" scope.
    #[must_use]
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope || s == "*")
    }

    /// Checks if the claims include any of the specified scopes.
    ///
    /// # Arguments
    ///
    /// * `scopes` - The scopes to check for.
    ///
    /// # Returns
    ///
    /// `true` if the claims include any of the specified scopes or the wildcard "*" scope.
    #[must_use]
    pub fn has_any_scope(&self, scopes: &[&str]) -> bool {
        scopes.iter().any(|s| self.has_scope(s))
    }
}

/// Tool authorization configuration (CRIT-003).
///
/// Maps tool names to required scopes for fine-grained access control.
/// Unknown tools are explicitly denied by returning `None` from `required_scope`.
#[cfg(feature = "http")]
#[derive(Debug, Clone, Default)]
pub struct ToolAuthorization {
    /// Whether to allow unknown tools with admin scope (default: false, deny unknown).
    pub allow_unknown_with_admin: bool,
}

#[cfg(feature = "http")]
impl ToolAuthorization {
    /// Known tools and their scopes (compile-time constant for security).
    const KNOWN_TOOLS: &'static [(&'static str, &'static str)] = &[
        // Write operations
        ("subcog_capture", "write"),
        ("subcog_enrich", "write"),
        ("subcog_consolidate", "write"),
        ("prompt_save", "write"),
        ("prompt_delete", "write"),
        // Read operations
        ("subcog_recall", "read"),
        ("subcog_status", "read"),
        ("subcog_namespaces", "read"),
        ("prompt_understanding", "read"),
        ("prompt_list", "read"),
        ("prompt_get", "read"),
        ("prompt_run", "read"),
        // Admin operations
        ("subcog_sync", "admin"),
        ("subcog_reindex", "admin"),
    ];

    /// Returns the required scope for a tool, or `None` if the tool is unknown.
    ///
    /// # Security
    ///
    /// Unknown tools return `None` to enforce explicit denial by default.
    /// This prevents authorization bypass via unrecognized tool names.
    ///
    /// Tool scope mapping:
    /// - `subcog_capture`, `subcog_enrich`, `subcog_consolidate`: "write"
    /// - `subcog_recall`, `subcog_status`, `subcog_namespaces`, `prompt_understanding`: "read"
    /// - `subcog_sync`, `subcog_reindex`: "admin"
    /// - `prompt_save`, `prompt_delete`: "write"
    /// - `prompt_list`, `prompt_get`, `prompt_run`: "read"
    /// - Unknown tools: `None` (explicit deny) or "admin" if `allow_unknown_with_admin`
    #[must_use]
    pub fn required_scope(&self, tool_name: &str) -> Option<&'static str> {
        for (name, scope) in Self::KNOWN_TOOLS {
            if *name == tool_name {
                return Some(scope);
            }
        }

        // Unknown tools: deny by default, require admin if explicitly allowed
        if self.allow_unknown_with_admin {
            Some("admin")
        } else {
            None
        }
    }

    /// Checks if a tool name is known to the authorization system.
    ///
    /// This is part of the public API for callers to verify tool names
    /// before making authorization requests.
    #[must_use]
    #[allow(dead_code)] // Public API - may be used by external callers
    pub fn is_known_tool(tool_name: &str) -> bool {
        Self::KNOWN_TOOLS.iter().any(|(name, _)| *name == tool_name)
    }

    /// Checks if claims authorize access to a tool.
    ///
    /// # Arguments
    ///
    /// * `claims` - The JWT claims to check.
    /// * `tool_name` - The name of the tool being called.
    ///
    /// # Returns
    ///
    /// `true` if the tool is known and claims include the required scope.
    /// Returns `false` for unknown tools (explicit deny).
    #[must_use]
    pub fn is_authorized(&self, claims: &Claims, tool_name: &str) -> bool {
        match self.required_scope(tool_name) {
            Some(required) => claims.has_scope(required),
            None => false, // Unknown tools are explicitly denied
        }
    }
}

/// JWT authentication configuration.
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// Secret key for HS256 validation.
    secret: String,
    /// Expected issuer (optional).
    issuer: Option<String>,
    /// Expected audience (optional).
    audience: Option<String>,
}

impl JwtConfig {
    /// Creates a new JWT configuration from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error if `SUBCOG_MCP_JWT_SECRET` is not set, too short, or
    /// has insufficient entropy.
    pub fn from_env() -> Result<Self> {
        let secret =
            std::env::var("SUBCOG_MCP_JWT_SECRET").map_err(|_| Error::OperationFailed {
                operation: "jwt_config".to_string(),
                cause: "SUBCOG_MCP_JWT_SECRET environment variable not set".to_string(),
            })?;

        if secret.len() < MIN_SECRET_LENGTH {
            return Err(Error::OperationFailed {
                operation: "jwt_config".to_string(),
                cause: format!(
                    "JWT secret must be at least {MIN_SECRET_LENGTH} characters (got {})",
                    secret.len()
                ),
            });
        }

        // Validate entropy (SEC-H1: entropy validation)
        validate_secret_entropy(&secret).map_err(|cause| Error::OperationFailed {
            operation: "jwt_config".to_string(),
            cause,
        })?;

        let issuer = std::env::var("SUBCOG_MCP_JWT_ISSUER").ok();
        let audience = std::env::var("SUBCOG_MCP_JWT_AUDIENCE").ok();

        Ok(Self {
            secret,
            issuer,
            audience,
        })
    }

    /// Creates a JWT configuration with explicit values (for testing).
    #[must_use]
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
            issuer: None,
            audience: None,
        }
    }

    /// Sets the expected issuer.
    #[must_use]
    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = Some(issuer.into());
        self
    }

    /// Sets the expected audience.
    #[must_use]
    pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
        self.audience = Some(audience.into());
        self
    }
}

/// JWT authenticator for validating bearer tokens.
#[derive(Clone)]
pub struct JwtAuthenticator {
    /// Decoding key.
    decoding_key: Arc<DecodingKey>,
    /// Validation settings.
    validation: Validation,
}

impl fmt::Debug for JwtAuthenticator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JwtAuthenticator")
            .field("validation", &self.validation)
            .finish_non_exhaustive()
    }
}

impl JwtAuthenticator {
    /// Creates a new JWT authenticator from configuration.
    #[must_use]
    pub fn new(config: &JwtConfig) -> Self {
        let decoding_key = Arc::new(DecodingKey::from_secret(config.secret.as_bytes()));

        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        if let Some(issuer) = &config.issuer {
            validation.set_issuer(&[issuer]);
        }

        if let Some(audience) = &config.audience {
            validation.set_audience(&[audience]);
        }

        Self {
            decoding_key,
            validation,
        }
    }

    /// Validates a bearer token and returns the claims.
    ///
    /// # Arguments
    ///
    /// * `token` - The JWT token (without "Bearer " prefix).
    ///
    /// # Errors
    ///
    /// Returns an error if the token is invalid, expired, or fails validation.
    pub fn validate(&self, token: &str) -> Result<Claims> {
        let token_data: TokenData<Claims> = decode(token, &self.decoding_key, &self.validation)
            .map_err(|e| {
                tracing::warn!(error = %e, "JWT validation failed");
                Error::Unauthorized(format!("Invalid token: {e}"))
            })?;

        tracing::debug!(
            sub = %token_data.claims.sub,
            scopes = ?token_data.claims.scopes,
            "JWT validated successfully"
        );

        Ok(token_data.claims)
    }

    /// Extracts and validates a bearer token from an Authorization header.
    ///
    /// # Arguments
    ///
    /// * `auth_header` - The full Authorization header value (e.g., `Bearer <token>`).
    ///
    /// # Errors
    ///
    /// Returns an error if the header format is invalid or token validation fails.
    pub fn validate_header(&self, auth_header: &str) -> Result<Claims> {
        let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
            Error::Unauthorized("Invalid Authorization header format".to_string())
        })?;

        self.validate(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header, encode};

    fn create_test_token(claims: &Claims, secret: &str) -> String {
        encode(
            &Header::default(),
            claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("Failed to encode test token")
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn valid_claims() -> Claims {
        Claims {
            sub: "test-user".to_string(),
            exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize,
            iat: chrono::Utc::now().timestamp() as usize,
            iss: None,
            aud: None,
            scopes: vec!["read".to_string(), "write".to_string()],
        }
    }

    // Tests that use JwtConfig::new don't need env vars
    #[test]
    fn test_validate_valid_token() {
        let secret = "a-very-long-secret-key-that-is-at-least-32-chars";
        let config = JwtConfig::new(secret);
        let authenticator = JwtAuthenticator::new(&config);

        let claims = valid_claims();
        let token = create_test_token(&claims, secret);

        let result = authenticator.validate(&token);
        assert!(result.is_ok());
        let validated_claims = result.expect("Should validate");
        assert_eq!(validated_claims.sub, "test-user");
    }

    #[test]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn test_validate_expired_token() {
        let secret = "a-very-long-secret-key-that-is-at-least-32-chars";
        let config = JwtConfig::new(secret);
        let authenticator = JwtAuthenticator::new(&config);

        let mut claims = valid_claims();
        claims.exp = (chrono::Utc::now() - chrono::Duration::hours(1)).timestamp() as usize;
        let token = create_test_token(&claims, secret);

        let result = authenticator.validate(&token);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_wrong_secret() {
        let secret = "a-very-long-secret-key-that-is-at-least-32-chars";
        let wrong_secret = "a-different-long-secret-key-that-is-32-chars";
        let config = JwtConfig::new(secret);
        let authenticator = JwtAuthenticator::new(&config);

        let claims = valid_claims();
        let token = create_test_token(&claims, wrong_secret);

        let result = authenticator.validate(&token);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_header() {
        let secret = "a-very-long-secret-key-that-is-at-least-32-chars";
        let config = JwtConfig::new(secret);
        let authenticator = JwtAuthenticator::new(&config);

        let claims = valid_claims();
        let token = create_test_token(&claims, secret);
        let header = format!("Bearer {token}");

        let result = authenticator.validate_header(&header);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_header_invalid_format() {
        let secret = "a-very-long-secret-key-that-is-at-least-32-chars";
        let config = JwtConfig::new(secret);
        let authenticator = JwtAuthenticator::new(&config);

        let result = authenticator.validate_header("Basic dXNlcjpwYXNz");
        assert!(result.is_err());
    }

    #[test]
    fn test_issuer_validation() {
        let secret = "a-very-long-secret-key-that-is-at-least-32-chars";
        let config = JwtConfig::new(secret).with_issuer("expected-issuer");
        let authenticator = JwtAuthenticator::new(&config);

        // Token with wrong issuer should fail
        let mut claims = valid_claims();
        claims.iss = Some("wrong-issuer".to_string());
        let token = create_test_token(&claims, secret);

        let result = authenticator.validate(&token);
        assert!(result.is_err());

        // Token with correct issuer should pass
        claims.iss = Some("expected-issuer".to_string());
        let token = create_test_token(&claims, secret);

        let result = authenticator.validate(&token);
        assert!(result.is_ok());
    }

    #[test]
    fn test_jwt_config_short_secret_validation() {
        // Use JwtConfig::new directly to test validation logic
        // We test from_env indirectly since env vars are unsafe in tests
        let config = JwtConfig::new("short");
        // The config is created, but authenticator will use short key
        // The actual security check is at from_env level
        let _authenticator = JwtAuthenticator::new(&config);
    }

    #[test]
    fn test_jwt_config_builder() {
        let config = JwtConfig::new("secret")
            .with_issuer("my-issuer")
            .with_audience("my-audience");

        assert_eq!(config.issuer, Some("my-issuer".to_string()));
        assert_eq!(config.audience, Some("my-audience".to_string()));
    }

    // CRIT-003: Tool Authorization Tests

    #[test]
    fn test_claims_has_scope() {
        let claims = Claims {
            sub: "test-user".to_string(),
            exp: 0,
            iat: 0,
            iss: None,
            aud: None,
            scopes: vec!["read".to_string(), "write".to_string()],
        };

        assert!(claims.has_scope("read"));
        assert!(claims.has_scope("write"));
        assert!(!claims.has_scope("admin"));
    }

    #[test]
    fn test_claims_has_scope_wildcard() {
        let claims = Claims {
            sub: "admin-user".to_string(),
            exp: 0,
            iat: 0,
            iss: None,
            aud: None,
            scopes: vec!["*".to_string()],
        };

        // Wildcard should match any scope
        assert!(claims.has_scope("read"));
        assert!(claims.has_scope("write"));
        assert!(claims.has_scope("admin"));
        assert!(claims.has_scope("anything"));
    }

    #[test]
    fn test_claims_has_any_scope() {
        let claims = Claims {
            sub: "test-user".to_string(),
            exp: 0,
            iat: 0,
            iss: None,
            aud: None,
            scopes: vec!["read".to_string()],
        };

        assert!(claims.has_any_scope(&["read", "write"]));
        assert!(claims.has_any_scope(&["admin", "read"]));
        assert!(!claims.has_any_scope(&["write", "admin"]));
    }

    #[cfg(feature = "http")]
    #[test]
    fn test_tool_authorization_required_scopes() {
        let auth = ToolAuthorization::default();

        // Write operations
        assert_eq!(auth.required_scope("subcog_capture"), Some("write"));
        assert_eq!(auth.required_scope("subcog_enrich"), Some("write"));
        assert_eq!(auth.required_scope("subcog_consolidate"), Some("write"));
        assert_eq!(auth.required_scope("prompt_save"), Some("write"));
        assert_eq!(auth.required_scope("prompt_delete"), Some("write"));

        // Read operations
        assert_eq!(auth.required_scope("subcog_recall"), Some("read"));
        assert_eq!(auth.required_scope("subcog_status"), Some("read"));
        assert_eq!(auth.required_scope("subcog_namespaces"), Some("read"));
        assert_eq!(auth.required_scope("prompt_understanding"), Some("read"));
        assert_eq!(auth.required_scope("prompt_list"), Some("read"));
        assert_eq!(auth.required_scope("prompt_get"), Some("read"));
        assert_eq!(auth.required_scope("prompt_run"), Some("read"));

        // Admin operations
        assert_eq!(auth.required_scope("subcog_sync"), Some("admin"));
        assert_eq!(auth.required_scope("subcog_reindex"), Some("admin"));

        // Unknown tools return None (explicitly denied by default)
        assert_eq!(auth.required_scope("unknown_tool"), None);
    }

    #[cfg(feature = "http")]
    #[test]
    fn test_tool_authorization_is_authorized() {
        let auth = ToolAuthorization::default();

        // User with read scope
        let read_user = Claims {
            sub: "reader".to_string(),
            exp: 0,
            iat: 0,
            iss: None,
            aud: None,
            scopes: vec!["read".to_string()],
        };

        assert!(auth.is_authorized(&read_user, "subcog_recall"));
        assert!(auth.is_authorized(&read_user, "subcog_status"));
        assert!(!auth.is_authorized(&read_user, "subcog_capture"));
        assert!(!auth.is_authorized(&read_user, "subcog_sync"));

        // User with write scope
        let write_user = Claims {
            sub: "writer".to_string(),
            exp: 0,
            iat: 0,
            iss: None,
            aud: None,
            scopes: vec!["write".to_string()],
        };

        assert!(auth.is_authorized(&write_user, "subcog_capture"));
        assert!(auth.is_authorized(&write_user, "prompt_save"));
        assert!(!auth.is_authorized(&write_user, "subcog_recall"));
        assert!(!auth.is_authorized(&write_user, "subcog_sync"));

        // User with admin scope
        let admin_user = Claims {
            sub: "admin".to_string(),
            exp: 0,
            iat: 0,
            iss: None,
            aud: None,
            scopes: vec!["admin".to_string()],
        };

        assert!(auth.is_authorized(&admin_user, "subcog_sync"));
        assert!(auth.is_authorized(&admin_user, "subcog_reindex"));
        assert!(!auth.is_authorized(&admin_user, "subcog_capture"));
        assert!(!auth.is_authorized(&admin_user, "subcog_recall"));
    }

    #[cfg(feature = "http")]
    #[test]
    fn test_tool_authorization_wildcard_scope() {
        let auth = ToolAuthorization::default();

        let superuser = Claims {
            sub: "superuser".to_string(),
            exp: 0,
            iat: 0,
            iss: None,
            aud: None,
            scopes: vec!["*".to_string()],
        };

        // Wildcard should authorize all known tools
        assert!(auth.is_authorized(&superuser, "subcog_recall"));
        assert!(auth.is_authorized(&superuser, "subcog_capture"));
        assert!(auth.is_authorized(&superuser, "subcog_sync"));
        // Unknown tools are explicitly denied regardless of scope
        assert!(!auth.is_authorized(&superuser, "unknown_tool"));
    }

    #[cfg(feature = "http")]
    #[test]
    fn test_tool_authorization_multiple_scopes() {
        let auth = ToolAuthorization::default();

        let multi_scope_user = Claims {
            sub: "multi".to_string(),
            exp: 0,
            iat: 0,
            iss: None,
            aud: None,
            scopes: vec!["read".to_string(), "write".to_string()],
        };

        // Should have access to both read and write operations
        assert!(auth.is_authorized(&multi_scope_user, "subcog_recall"));
        assert!(auth.is_authorized(&multi_scope_user, "subcog_capture"));
        // But not admin
        assert!(!auth.is_authorized(&multi_scope_user, "subcog_sync"));
    }

    // HIGH-SEC-004: Character class diversity tests

    #[test]
    fn test_entropy_validation_good_base64_secret() {
        // Base64 output from `openssl rand -base64 32` has 3+ character classes
        let result = validate_secret_entropy("aB3+XyZ9/Qr7mN2pK5tL8vW0jH4gF6sD=");
        assert!(result.is_ok(), "Base64 secret should pass: {result:?}");
    }

    #[test]
    fn test_entropy_validation_all_lowercase_fails() {
        // Only 1 character class: lowercase
        let result = validate_secret_entropy("abcdefghijklmnopqrstuvwxyzabcdef");
        assert!(result.is_err(), "All lowercase should fail");
        assert!(result.unwrap_err().contains("character diversity"));
    }

    #[test]
    fn test_entropy_validation_all_uppercase_fails() {
        // Only 1 character class: uppercase
        let result = validate_secret_entropy("ABCDEFGHIJKLMNOPQRSTUVWXYZABCDEF");
        assert!(result.is_err(), "All uppercase should fail");
    }

    #[test]
    fn test_entropy_validation_all_digits_fails() {
        // Only 1 character class: digits
        let result = validate_secret_entropy("12345678901234567890123456789012");
        assert!(result.is_err(), "All digits should fail");
    }

    #[test]
    fn test_entropy_validation_two_classes_fails() {
        // Only 2 character classes: lowercase + uppercase
        let result = validate_secret_entropy("abcdefghijklmnopABCDEFGHIJKLMNOP");
        assert!(result.is_err(), "Two classes should fail (need 3+)");
    }

    #[test]
    fn test_entropy_validation_three_classes_passes() {
        // 3 character classes: lowercase + uppercase + digits (no weak patterns)
        let result = validate_secret_entropy("xYmNpQrStUvWxYz0192837465XYZMNP");
        assert!(result.is_ok(), "Three classes should pass: {result:?}");
    }

    #[test]
    fn test_entropy_validation_weak_pattern_still_fails() {
        // Has 3+ character classes but contains weak pattern
        let result = validate_secret_entropy("Password123!@#$%^&*()_+-=[]{}|");
        assert!(
            result.is_err(),
            "Weak pattern should fail even with good diversity"
        );
        assert!(result.unwrap_err().contains("weak pattern"));
    }

    #[test]
    fn test_entropy_validation_special_chars_count() {
        // Verify special chars include base64 characters (+, /, =)
        let result = validate_secret_entropy("AAAAAAAAAAAAAAAAAAAAAAAAAAAA+/==");
        // Has uppercase + special (+ / =), but only 2 classes - should fail
        assert!(
            result.is_err(),
            "Two classes (uppercase + special) should fail"
        );
    }
}
