//! Tool execution handlers.
//!
//! This module contains the execution logic for all MCP tools,
//! organized into submodules by domain.

mod core;
mod prompts;

pub use core::{
    execute_capture, execute_consolidate, execute_enrich, execute_namespaces, execute_recall,
    execute_reindex, execute_status, execute_sync,
};
pub use prompts::{
    execute_prompt_delete, execute_prompt_get, execute_prompt_list, execute_prompt_run,
    execute_prompt_save,
};
