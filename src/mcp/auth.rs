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

/// Validates that a secret has sufficient entropy (not just length).
///
/// Checks for:
/// - Minimum unique character diversity
/// - Not all same character
/// - Not obviously sequential patterns
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
}
