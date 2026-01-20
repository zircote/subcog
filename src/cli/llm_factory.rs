//! LLM client factory functions for CLI commands.
//!
//! Provides builders for creating LLM clients from configuration.

use std::sync::Arc;

use crate::config::LlmConfig;
use crate::llm::{
    AnthropicClient, LlmHttpConfig, LlmProvider, LlmResilienceConfig, LmStudioClient, OllamaClient,
    OpenAiClient, ResilientLlmProvider,
};

/// Builds HTTP configuration from LLM config with environment overrides.
#[must_use]
pub fn build_http_config(llm_config: &LlmConfig) -> LlmHttpConfig {
    LlmHttpConfig::from_config(llm_config).with_env_overrides()
}

/// Builds resilience configuration from LLM config with environment overrides.
#[must_use]
pub fn build_resilience_config(llm_config: &LlmConfig) -> LlmResilienceConfig {
    LlmResilienceConfig::from_config(llm_config).with_env_overrides()
}

/// Builds an `OpenAI` client from configuration.
#[must_use]
pub fn build_openai_client(llm_config: &LlmConfig) -> OpenAiClient {
    let mut client = OpenAiClient::new();
    if let Some(ref api_key) = llm_config.api_key {
        client = client.with_api_key(api_key);
    }
    if let Some(ref model) = llm_config.model {
        client = client.with_model(model);
    }
    if let Some(ref base_url) = llm_config.base_url {
        client = client.with_endpoint(base_url);
    }
    if let Some(max_tokens) = llm_config.max_tokens {
        client = client.with_max_tokens(max_tokens);
    }
    client.with_http_config(build_http_config(llm_config))
}

/// Builds an Anthropic client from configuration.
#[must_use]
pub fn build_anthropic_client(llm_config: &LlmConfig) -> AnthropicClient {
    let mut client = AnthropicClient::new();
    if let Some(ref api_key) = llm_config.api_key {
        client = client.with_api_key(api_key);
    }
    if let Some(ref model) = llm_config.model {
        client = client.with_model(model);
    }
    if let Some(ref base_url) = llm_config.base_url {
        client = client.with_endpoint(base_url);
    }
    client.with_http_config(build_http_config(llm_config))
}

/// Builds an Ollama client from configuration.
#[must_use]
pub fn build_ollama_client(llm_config: &LlmConfig) -> OllamaClient {
    let mut client = OllamaClient::new();
    if let Some(ref model) = llm_config.model {
        client = client.with_model(model);
    }
    if let Some(ref base_url) = llm_config.base_url {
        client = client.with_endpoint(base_url);
    }
    client.with_http_config(build_http_config(llm_config))
}

/// Builds an LM Studio client from configuration.
#[must_use]
pub fn build_lmstudio_client(llm_config: &LlmConfig) -> LmStudioClient {
    let mut client = LmStudioClient::new();
    if let Some(ref model) = llm_config.model {
        client = client.with_model(model);
    }
    if let Some(ref base_url) = llm_config.base_url {
        client = client.with_endpoint(base_url);
    }
    client.with_http_config(build_http_config(llm_config))
}

/// Builds an LLM provider for hooks from configuration.
///
/// Returns `None` if LLM is disabled for search intent.
#[must_use]
pub fn build_hook_llm_provider(
    config: &crate::config::SubcogConfig,
) -> Option<Arc<dyn LlmProvider>> {
    use crate::config::LlmProvider as Provider;

    if !config.search_intent.use_llm {
        return None;
    }

    let llm_config = &config.llm;
    let provider: Arc<dyn LlmProvider> = match llm_config.provider {
        Provider::OpenAi => {
            let resilience_config = build_resilience_config(llm_config);
            Arc::new(ResilientLlmProvider::new(
                build_openai_client(llm_config),
                resilience_config,
            ))
        },
        Provider::Anthropic => {
            let resilience_config = build_resilience_config(llm_config);
            Arc::new(ResilientLlmProvider::new(
                build_anthropic_client(llm_config),
                resilience_config,
            ))
        },
        Provider::Ollama => {
            let resilience_config = build_resilience_config(llm_config);
            Arc::new(ResilientLlmProvider::new(
                build_ollama_client(llm_config),
                resilience_config,
            ))
        },
        Provider::LmStudio => {
            let resilience_config = build_resilience_config(llm_config);
            Arc::new(ResilientLlmProvider::new(
                build_lmstudio_client(llm_config),
                resilience_config,
            ))
        },
        Provider::None => return None,
    };

    Some(provider)
}

/// Builds an LLM provider for entity extraction with a longer timeout.
///
/// Entity extraction often requires processing complex content that can take
/// longer than the default LLM timeout. This function creates an LLM provider
/// with the entity extraction timeout from config (default: 120s).
///
/// Returns `None` if LLM features are disabled in config.
#[must_use]
pub fn build_llm_provider_for_entity_extraction(
    config: &crate::config::SubcogConfig,
) -> Option<Arc<dyn LlmProvider>> {
    use crate::config::{LlmProvider as Provider, OperationType};

    tracing::debug!(
        llm_features = config.features.llm_features,
        provider = ?config.llm.provider,
        "build_llm_provider_for_entity_extraction called"
    );

    if !config.features.llm_features {
        tracing::debug!("LLM features disabled in config, returning None");
        return None;
    }

    // Create a modified LLM config with the entity extraction timeout
    let entity_timeout_ms = config.timeouts.get(OperationType::EntityExtraction).as_millis() as u64;
    let mut llm_config = config.llm.clone();
    llm_config.timeout_ms = Some(entity_timeout_ms);

    tracing::debug!(
        entity_timeout_ms = entity_timeout_ms,
        "Using entity extraction timeout for LLM"
    );

    let provider: Arc<dyn LlmProvider> = match llm_config.provider {
        Provider::OpenAi => {
            let resilience_config = build_resilience_config(&llm_config);
            Arc::new(ResilientLlmProvider::new(
                build_openai_client(&llm_config),
                resilience_config,
            ))
        },
        Provider::Anthropic => {
            let resilience_config = build_resilience_config(&llm_config);
            Arc::new(ResilientLlmProvider::new(
                build_anthropic_client(&llm_config),
                resilience_config,
            ))
        },
        Provider::Ollama => {
            let resilience_config = build_resilience_config(&llm_config);
            Arc::new(ResilientLlmProvider::new(
                build_ollama_client(&llm_config),
                resilience_config,
            ))
        },
        Provider::LmStudio => {
            let resilience_config = build_resilience_config(&llm_config);
            Arc::new(ResilientLlmProvider::new(
                build_lmstudio_client(&llm_config),
                resilience_config,
            ))
        },
        Provider::None => {
            tracing::debug!("LLM provider is None, returning None");
            return None;
        },
    };

    tracing::debug!(
        provider_type = ?llm_config.provider,
        timeout_ms = entity_timeout_ms,
        "LLM provider for entity extraction built successfully"
    );
    Some(provider)
}

/// Builds an LLM provider from configuration.
///
/// Returns `None` if LLM features are disabled in config.
/// This is a general-purpose LLM provider builder for entity extraction
/// and other LLM-powered features (not tied to search intent).
#[must_use]
pub fn build_llm_provider(config: &crate::config::SubcogConfig) -> Option<Arc<dyn LlmProvider>> {
    use crate::config::LlmProvider as Provider;

    tracing::debug!(
        llm_features = config.features.llm_features,
        provider = ?config.llm.provider,
        "build_llm_provider called"
    );

    if !config.features.llm_features {
        tracing::debug!("LLM features disabled in config, returning None");
        return None;
    }

    let llm_config = &config.llm;
    let provider: Arc<dyn LlmProvider> = match llm_config.provider {
        Provider::OpenAi => {
            let resilience_config = build_resilience_config(llm_config);
            Arc::new(ResilientLlmProvider::new(
                build_openai_client(llm_config),
                resilience_config,
            ))
        },
        Provider::Anthropic => {
            let resilience_config = build_resilience_config(llm_config);
            Arc::new(ResilientLlmProvider::new(
                build_anthropic_client(llm_config),
                resilience_config,
            ))
        },
        Provider::Ollama => {
            let resilience_config = build_resilience_config(llm_config);
            Arc::new(ResilientLlmProvider::new(
                build_ollama_client(llm_config),
                resilience_config,
            ))
        },
        Provider::LmStudio => {
            let resilience_config = build_resilience_config(llm_config);
            Arc::new(ResilientLlmProvider::new(
                build_lmstudio_client(llm_config),
                resilience_config,
            ))
        },
        Provider::None => {
            tracing::debug!("LLM provider is None, returning None");
            return None;
        },
    };

    tracing::debug!(provider_type = ?llm_config.provider, "LLM provider built successfully");
    Some(provider)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LlmConfig, LlmProvider as Provider, SubcogConfig};

    #[test]
    fn test_build_http_config_with_defaults() {
        let llm_config = LlmConfig::default();
        let http_config = build_http_config(&llm_config);

        // Verify we get a valid config (defaults are applied)
        assert!(http_config.connect_timeout_ms > 0);
        assert!(http_config.timeout_ms > 0);
    }

    #[test]
    fn test_build_resilience_config_with_defaults() {
        let llm_config = LlmConfig::default();
        let resilience_config = build_resilience_config(&llm_config);

        // Verify we get valid defaults (max_retries is u32, so always >= 0)
        assert!(resilience_config.breaker_failure_threshold > 0);
    }

    #[test]
    fn test_build_openai_client_with_config() {
        let llm_config = LlmConfig {
            api_key: Some("test-api-key".to_string()),
            model: Some("gpt-4".to_string()),
            base_url: Some("https://custom.openai.com".to_string()),
            ..Default::default()
        };

        let client = build_openai_client(&llm_config);
        assert_eq!(client.name(), "openai");
    }

    #[test]
    fn test_build_anthropic_client_with_config() {
        let llm_config = LlmConfig {
            api_key: Some("sk-ant-test-key".to_string()),
            model: Some("claude-3-opus".to_string()),
            ..Default::default()
        };

        let client = build_anthropic_client(&llm_config);
        assert_eq!(client.name(), "anthropic");
    }

    #[test]
    fn test_build_ollama_client_with_config() {
        let llm_config = LlmConfig {
            model: Some("llama2".to_string()),
            base_url: Some("http://localhost:11434".to_string()),
            ..Default::default()
        };

        let client = build_ollama_client(&llm_config);
        assert_eq!(client.name(), "ollama");
    }

    #[test]
    fn test_build_lmstudio_client_with_config() {
        let llm_config = LlmConfig {
            model: Some("local-model".to_string()),
            base_url: Some("http://localhost:1234".to_string()),
            ..Default::default()
        };

        let client = build_lmstudio_client(&llm_config);
        assert_eq!(client.name(), "lmstudio");
    }

    #[test]
    fn test_build_hook_llm_provider_disabled() {
        let mut config = SubcogConfig::default();
        config.search_intent.use_llm = false;

        let provider = build_hook_llm_provider(&config);
        assert!(provider.is_none());
    }

    #[test]
    fn test_build_hook_llm_provider_openai() {
        let mut config = SubcogConfig::default();
        config.search_intent.use_llm = true;
        config.llm.provider = Provider::OpenAi;
        config.llm.api_key = Some("test-key".to_string());

        let provider = build_hook_llm_provider(&config);
        assert!(provider.is_some());
    }

    #[test]
    fn test_build_hook_llm_provider_anthropic() {
        let mut config = SubcogConfig::default();
        config.search_intent.use_llm = true;
        config.llm.provider = Provider::Anthropic;

        let provider = build_hook_llm_provider(&config);
        assert!(provider.is_some());
    }

    #[test]
    fn test_build_hook_llm_provider_ollama() {
        let mut config = SubcogConfig::default();
        config.search_intent.use_llm = true;
        config.llm.provider = Provider::Ollama;

        let provider = build_hook_llm_provider(&config);
        assert!(provider.is_some());
    }

    #[test]
    fn test_build_hook_llm_provider_lmstudio() {
        let mut config = SubcogConfig::default();
        config.search_intent.use_llm = true;
        config.llm.provider = Provider::LmStudio;

        let provider = build_hook_llm_provider(&config);
        assert!(provider.is_some());
    }
}
