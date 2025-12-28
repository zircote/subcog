//! Feature flags for optional functionality.

/// Feature flags for controlling optional subcog features.
#[derive(Debug, Clone, Default)]
pub struct FeatureFlags {
    /// Enable secret detection and filtering.
    pub secrets_filter: bool,
    /// Enable PII detection and filtering.
    pub pii_filter: bool,
    /// Enable multi-domain support.
    pub multi_domain: bool,
    /// Enable audit logging.
    pub audit_log: bool,
    /// Enable LLM-powered features.
    pub llm_features: bool,
    /// Enable auto-capture during hooks.
    pub auto_capture: bool,
    /// Enable memory consolidation.
    pub consolidation: bool,
}

impl FeatureFlags {
    /// Creates feature flags with all features disabled.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            secrets_filter: false,
            pii_filter: false,
            multi_domain: false,
            audit_log: false,
            llm_features: false,
            auto_capture: false,
            consolidation: false,
        }
    }

    /// Creates feature flags with core features enabled.
    #[must_use]
    pub const fn core() -> Self {
        Self {
            secrets_filter: true,
            pii_filter: false,
            multi_domain: false,
            audit_log: false,
            llm_features: false,
            auto_capture: false,
            consolidation: false,
        }
    }

    /// Creates feature flags with all features enabled.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            secrets_filter: true,
            pii_filter: true,
            multi_domain: true,
            audit_log: true,
            llm_features: true,
            auto_capture: true,
            consolidation: true,
        }
    }
}
