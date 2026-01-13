//! Webhook CLI command handler.
//!
//! This module provides the command handler for webhook management.

use subcog::cli::webhook::{
    cmd_webhook_delete_logs, cmd_webhook_export, cmd_webhook_history, cmd_webhook_list,
    cmd_webhook_stats, cmd_webhook_test,
};
use subcog::storage::get_user_data_dir;

pub use super::WebhookAction;

/// Handles webhook subcommands.
///
/// # Errors
///
/// Returns an error if the subcommand fails.
pub fn cmd_webhook(action: WebhookAction) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = get_user_data_dir()?;

    match action {
        WebhookAction::List { format } => {
            cmd_webhook_list(&format)?;
        },
        WebhookAction::Test { name } => {
            cmd_webhook_test(&name, &data_dir)?;
        },
        WebhookAction::History {
            name,
            limit,
            format,
        } => {
            cmd_webhook_history(name.as_deref(), limit, &data_dir, &format)?;
        },
        WebhookAction::Stats { name } => {
            cmd_webhook_stats(name.as_deref(), &data_dir)?;
        },
        WebhookAction::Export { domain, output } => {
            cmd_webhook_export(&domain, output.as_deref(), &data_dir)?;
        },
        WebhookAction::DeleteLogs { domain, force } => {
            cmd_webhook_delete_logs(&domain, force, &data_dir)?;
        },
    }

    Ok(())
}
