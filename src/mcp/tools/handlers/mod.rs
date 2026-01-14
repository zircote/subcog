//! Tool execution handlers.
//!
//! This module contains the execution logic for all MCP tools,
//! organized into submodules by domain.

mod context_templates;
mod core;
mod graph;
#[cfg(feature = "group-scope")]
mod groups;
mod prompts;

pub use context_templates::{
    execute_context_template_delete, execute_context_template_get, execute_context_template_list,
    execute_context_template_render, execute_context_template_save,
};
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
#[cfg(feature = "group-scope")]
pub use groups::{
    execute_group_add_member, execute_group_create, execute_group_delete, execute_group_get,
    execute_group_list, execute_group_remove_member, execute_group_update_role,
};
pub use prompts::{
    execute_prompt_delete, execute_prompt_get, execute_prompt_list, execute_prompt_run,
    execute_prompt_save,
};
