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
    };

    Some(provider)
}
