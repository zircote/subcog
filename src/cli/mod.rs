//! CLI command implementations.

mod capture;
mod config;
mod consolidate;
mod hook;
mod llm_factory;
mod namespaces;
mod prompt;
mod recall;
mod serve;
mod status;
mod sync;

pub use capture::CaptureCommand;
pub use config::ConfigCommand;
pub use consolidate::ConsolidateCommand;
pub use hook::HookCommand;
pub use llm_factory::{
    build_anthropic_client, build_hook_llm_provider, build_http_config, build_lmstudio_client,
    build_ollama_client, build_openai_client, build_resilience_config,
};
pub use namespaces::{NamespaceInfo, NamespacesOutputFormat, cmd_namespaces, get_all_namespaces};
pub use prompt::{
    OutputFormat, PromptCommand, cmd_prompt_delete, cmd_prompt_export, cmd_prompt_get,
    cmd_prompt_import, cmd_prompt_list, cmd_prompt_run, cmd_prompt_save, cmd_prompt_share,
};
pub use recall::RecallCommand;
pub use serve::ServeCommand;
pub use status::StatusCommand;
pub use sync::SyncCommand;
