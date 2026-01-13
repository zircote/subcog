//! Tool execution handlers.
//!
//! This module contains the execution logic for all MCP tools,
//! organized into submodules by domain.

mod core;
mod graph;
mod prompts;

pub use core::{
    execute_capture, execute_consolidate, execute_delete, execute_delete_all, execute_enrich,
    execute_gdpr_export, execute_get, execute_get_summary, execute_history, execute_init,
    execute_list, execute_namespaces, execute_prompt_understanding, execute_recall,
    execute_reindex, execute_restore, execute_status, execute_update,
};
pub use graph::{
    execute_entities, execute_entity_merge, execute_extract_entities, execute_graph_query,
    execute_graph_visualize, execute_relationship_infer, execute_relationships,
};
pub use prompts::{
    execute_prompt_delete, execute_prompt_get, execute_prompt_list, execute_prompt_run,
    execute_prompt_save,
};
