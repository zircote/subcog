//! Prompt command handler.
//!
//! Contains the implementation of the `prompt` CLI command for
//! managing prompt templates.

use subcog::cli::{
    cmd_prompt_delete, cmd_prompt_export, cmd_prompt_get, cmd_prompt_import, cmd_prompt_list,
    cmd_prompt_run, cmd_prompt_save, cmd_prompt_share,
};

use super::PromptAction;

/// Prompt command.
pub fn cmd_prompt(action: PromptAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        PromptAction::Save {
            name,
            content,
            description,
            tags,
            domain,
            from_file,
            from_stdin,
            no_enrich,
            dry_run,
        } => cmd_prompt_save(
            name,
            content,
            description,
            tags,
            domain,
            from_file,
            from_stdin,
            no_enrich,
            dry_run,
        ),

        PromptAction::List {
            domain,
            tags,
            name,
            format,
            limit,
        } => cmd_prompt_list(domain, tags, name, format, limit),

        PromptAction::Get {
            name,
            domain,
            format,
        } => cmd_prompt_get(name, domain, format),

        PromptAction::Run {
            name,
            variables,
            domain,
            interactive,
        } => cmd_prompt_run(name, variables, domain, interactive),

        PromptAction::Delete {
            name,
            domain,
            force,
        } => cmd_prompt_delete(name, domain, force),

        PromptAction::Export {
            name,
            output,
            format,
            domain,
        } => cmd_prompt_export(name, output, format, domain),

        PromptAction::Import {
            source,
            domain,
            name,
            no_validate,
        } => cmd_prompt_import(source, domain, name, no_validate),

        PromptAction::Share {
            name,
            output,
            format,
            domain,
            include_stats,
        } => cmd_prompt_share(name, output, format, domain, include_stats),
    }
}
