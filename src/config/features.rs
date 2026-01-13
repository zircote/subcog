//! Feature flags for optional functionality.

use super::ConfigFileFeatures;

/// Feature flags for controlling optional subcog features.
#[derive(Debug, Clone, Default)]
#[allow(clippy::struct_excessive_bools)]
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
    /// Enable org-scope storage (PostgreSQL shared storage).
    pub org_scope_enabled: bool,
    /// Enable automatic entity extraction during memory capture.
    ///
    /// When enabled, entities (people, organizations, technologies, concepts)
    /// are automatically extracted from captured memories and stored in the
    /// knowledge graph for graph-augmented retrieval.
    pub auto_extract_entities: bool,
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
            org_scope_enabled: false,
            auto_extract_entities: false,
        }
    }

    /// Creates feature flags with core features enabled.
    #[must_use]
    pub const fn core() -> Self {
        Self {
            secrets_filter: true,
            pii_filter: true,
            multi_domain: false,
            audit_log: false,
            llm_features: false,
            auto_capture: false,
            consolidation: false,
            org_scope_enabled: false,
            auto_extract_entities: true,
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
            org_scope_enabled: true,
            auto_extract_entities: true,
        }
    }

    /// Creates feature flags from config file settings.
    ///
    /// ARCH-HIGH-002: Delegated from `SubcogConfig::apply_config_file`.
    #[must_use]
    pub fn from_config_file(file: &ConfigFileFeatures) -> Self {
        let mut flags = Self::default();
        if let Some(v) = file.secrets_filter {
            flags.secrets_filter = v;
        }
        if let Some(v) = file.pii_filter {
            flags.pii_filter = v;
        }
        if let Some(v) = file.multi_domain {
            flags.multi_domain = v;
        }
        if let Some(v) = file.audit_log {
            flags.audit_log = v;
        }
        if let Some(v) = file.llm_features {
            flags.llm_features = v;
        }
        if let Some(v) = file.auto_capture {
            flags.auto_capture = v;
        }
        if let Some(v) = file.consolidation {
            flags.consolidation = v;
        }
        if let Some(v) = file.org_scope_enabled {
            flags.org_scope_enabled = v;
        }
        if let Some(v) = file.auto_extract_entities {
            flags.auto_extract_entities = v;
        }
        flags
    }

    /// Merges another set of flags into this one.
    ///
    /// Only overrides fields that are set in the source.
    pub const fn merge_from(&mut self, file: &ConfigFileFeatures) {
        if let Some(v) = file.secrets_filter {
            self.secrets_filter = v;
        }
        if let Some(v) = file.pii_filter {
            self.pii_filter = v;
        }
        if let Some(v) = file.multi_domain {
            self.multi_domain = v;
        }
        if let Some(v) = file.audit_log {
            self.audit_log = v;
        }
        if let Some(v) = file.llm_features {
            self.llm_features = v;
        }
        if let Some(v) = file.auto_capture {
            self.auto_capture = v;
        }
        if let Some(v) = file.consolidation {
            self.consolidation = v;
        }
        if let Some(v) = file.org_scope_enabled {
            self.org_scope_enabled = v;
        }
        if let Some(v) = file.auto_extract_entities {
            self.auto_extract_entities = v;
        }
    }
}
