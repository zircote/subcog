//! CLI command implementations.

mod capture;
mod config;
mod consolidate;
mod hook;
mod prompt;
mod recall;
mod serve;
mod status;
mod sync;

pub use capture::CaptureCommand;
pub use config::ConfigCommand;
pub use consolidate::ConsolidateCommand;
pub use hook::HookCommand;
pub use prompt::{
    OutputFormat, PromptCommand, cmd_prompt_delete, cmd_prompt_export, cmd_prompt_get,
    cmd_prompt_list, cmd_prompt_run, cmd_prompt_save,
};
pub use recall::RecallCommand;
pub use serve::ServeCommand;
pub use status::StatusCommand;
pub use sync::SyncCommand;
