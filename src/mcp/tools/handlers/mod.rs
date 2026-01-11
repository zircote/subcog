//! Tool execution handlers.
//!
//! This module contains the execution logic for all MCP tools,
//! organized into submodules by domain.

mod core;
mod prompts;

pub use core::{
    execute_capture, execute_consolidate, execute_enrich, execute_gdpr_export,
    execute_get_summary, execute_namespaces, execute_prompt_understanding, execute_recall,
    execute_reindex, execute_status,
};
pub use prompts::{
    execute_prompt_delete, execute_prompt_get, execute_prompt_list, execute_prompt_run,
    execute_prompt_save,
};
