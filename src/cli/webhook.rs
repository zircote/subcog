//! Webhook CLI command.
//!
//! Provides commands for managing webhook notifications:
//! - List configured webhooks
//! - Test webhook delivery
//! - View delivery history
//! - Export/delete audit logs (GDPR compliance)

// CLI commands are allowed to use println! for output
#![allow(clippy::print_stdout)]

use crate::Result;
use crate::storage::index::DomainScope;
use crate::webhooks::{
    DeliveryStatus, WebhookAuditBackend, WebhookAuditLogger, WebhookConfig, WebhookService,
};
use std::path::Path;

/// Webhook command handler.
pub struct WebhookCommand;

impl WebhookCommand {
    /// Creates a new webhook command.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for WebhookCommand {
    fn default() -> Self {
        Self::new()
    }
}

/// Lists all configured webhooks.
///
/// # Arguments
///
/// * `format` - Output format: "table", "json", or "yaml"
///
/// # Errors
///
/// Returns an error if the configuration cannot be loaded.
pub fn cmd_webhook_list(format: &str) -> Result<()> {
    let config = WebhookConfig::load_default();

    if config.webhooks.is_empty() {
        println!("No webhooks configured.");
        println!();
        println!("To add webhooks, add [[webhooks]] to ~/.config/subcog/config.toml");
        println!("See documentation for configuration format.");
        return Ok(());
    }

    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&config.webhooks).map_err(|e| {
                crate::Error::OperationFailed {
                    operation: "serialize_webhooks".to_string(),
                    cause: e.to_string(),
                }
            })?;
            println!("{json}");
        },
        "yaml" => {
            let yaml = serde_yaml_ng::to_string(&config.webhooks).map_err(|e| {
                crate::Error::OperationFailed {
                    operation: "serialize_webhooks".to_string(),
                    cause: e.to_string(),
                }
            })?;
            println!("{yaml}");
        },
        _ => {
            // Table format
            println!("Configured Webhooks:");
            println!("{}", "-".repeat(80));
            println!(
                "{:<20} {:<10} {:<15} {:<30}",
                "NAME", "ENABLED", "AUTH", "EVENTS"
            );
            println!("{}", "-".repeat(80));

            for webhook in &config.webhooks {
                let auth_type = match &webhook.auth {
                    crate::webhooks::WebhookAuth::Bearer { .. } => "Bearer",
                    crate::webhooks::WebhookAuth::Hmac { .. } => "HMAC",
                    crate::webhooks::WebhookAuth::Both { .. } => "Bearer+HMAC",
                    crate::webhooks::WebhookAuth::None => "None",
                };

                let events = if webhook.events.is_empty() {
                    "*".to_string()
                } else {
                    webhook.events.join(", ")
                };

                let events_display = if events.len() > 28 {
                    format!("{}...", &events[..25])
                } else {
                    events
                };

                println!(
                    "{:<20} {:<10} {:<15} {:<30}",
                    truncate(&webhook.name, 18),
                    if webhook.enabled { "Yes" } else { "No" },
                    auth_type,
                    events_display
                );
            }

            println!("{}", "-".repeat(80));
            println!("Total: {} webhook(s)", config.webhooks.len());
        },
    }

    Ok(())
}

/// Tests a webhook by sending a test event.
///
/// # Arguments
///
/// * `name` - Name of the webhook to test
/// * `data_dir` - Data directory for the audit database
///
/// # Errors
///
/// Returns an error if the webhook is not found or delivery fails.
pub fn cmd_webhook_test(name: &str, data_dir: &Path) -> Result<()> {
    let service = WebhookService::from_config_file(DomainScope::Project, data_dir)?
        .ok_or_else(|| crate::Error::InvalidInput("No webhooks configured".to_string()))?;

    println!("Testing webhook '{name}'...");

    let result = service.test_webhook(name)?;

    if result.success {
        println!("✓ Webhook test successful!");
        println!("  Status code: {}", result.status_code.unwrap_or(0));
        println!("  Attempts: {}", result.attempts);
        println!("  Duration: {}ms", result.duration_ms);
    } else {
        println!("✗ Webhook test failed!");
        println!("  Attempts: {}", result.attempts);
        println!("  Duration: {}ms", result.duration_ms);
        if let Some(error) = &result.error {
            println!("  Error: {error}");
        }
    }

    Ok(())
}

/// Shows delivery history for a webhook.
///
/// # Arguments
///
/// * `name` - Webhook name (optional, shows all if not specified)
/// * `limit` - Maximum number of records to show
/// * `data_dir` - Data directory for the audit database
/// * `format` - Output format: "table" or "json"
///
/// # Errors
///
/// Returns an error if the audit database cannot be accessed.
pub fn cmd_webhook_history(
    name: Option<&str>,
    limit: usize,
    data_dir: &Path,
    format: &str,
) -> Result<()> {
    let audit_path = data_dir.join("webhook_audit.db");

    if !audit_path.exists() {
        println!("No webhook delivery history found.");
        return Ok(());
    }

    let logger = WebhookAuditLogger::new(&audit_path)?;

    let records = if let Some(webhook_name) = name {
        logger.get_history(webhook_name, limit)?
    } else {
        // Get history for all webhooks (export all)
        logger.export_domain_logs("*")?
    };

    if records.is_empty() {
        println!("No delivery history found.");
        return Ok(());
    }

    if format == "json" {
        let json =
            serde_json::to_string_pretty(&records).map_err(|e| crate::Error::OperationFailed {
                operation: "serialize_history".to_string(),
                cause: e.to_string(),
            })?;
        println!("{json}");
    } else {
        println!("Webhook Delivery History:");
        println!("{}", "-".repeat(100));
        println!(
            "{:<20} {:<12} {:<10} {:<8} {:<10} {:<35}",
            "WEBHOOK", "EVENT", "STATUS", "CODE", "ATTEMPTS", "TIMESTAMP"
        );
        println!("{}", "-".repeat(100));

        for record in records.iter().take(limit) {
            let status = match record.status {
                DeliveryStatus::Success => "✓ OK",
                DeliveryStatus::Failed => "✗ FAIL",
                DeliveryStatus::Timeout => "◷ TOUT",
            };

            let code: String = record
                .status_code
                .map_or_else(|| "-".to_string(), |c| c.to_string());

            let timestamp = chrono::DateTime::from_timestamp(record.timestamp, 0).map_or_else(
                || "Unknown".to_string(),
                |dt| dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            );

            println!(
                "{:<20} {:<12} {:<10} {:<8} {:<10} {:<35}",
                truncate(&record.webhook_name, 18),
                truncate(&record.event_type, 10),
                status,
                code,
                record.attempts,
                timestamp
            );
        }

        println!("{}", "-".repeat(100));
        println!(
            "Showing {} of {} record(s)",
            records.len().min(limit),
            records.len()
        );
    }

    Ok(())
}

/// Shows webhook statistics.
///
/// # Arguments
///
/// * `name` - Webhook name (optional)
/// * `data_dir` - Data directory for the audit database
///
/// # Errors
///
/// Returns an error if the audit database cannot be accessed.
pub fn cmd_webhook_stats(name: Option<&str>, data_dir: &Path) -> Result<()> {
    let config = WebhookConfig::load_default();
    let audit_path = data_dir.join("webhook_audit.db");

    if !audit_path.exists() {
        println!("No webhook statistics available (no deliveries recorded).");
        return Ok(());
    }

    let logger = WebhookAuditLogger::new(&audit_path)?;

    let webhooks_to_show: Vec<_> = if let Some(webhook_name) = name {
        config
            .webhooks
            .iter()
            .filter(|w| w.name == webhook_name)
            .collect()
    } else {
        config.webhooks.iter().collect()
    };

    if webhooks_to_show.is_empty() {
        if let Some(n) = name {
            println!("Webhook '{n}' not found.");
        } else {
            println!("No webhooks configured.");
        }
        return Ok(());
    }

    println!("Webhook Statistics:");
    println!("{}", "-".repeat(80));
    println!(
        "{:<20} {:<10} {:<10} {:<10} {:<12} {:<12}",
        "WEBHOOK", "TOTAL", "SUCCESS", "FAILED", "AVG MS", "SUCCESS %"
    );
    println!("{}", "-".repeat(80));

    for webhook in webhooks_to_show {
        let stats = logger.count_by_status(&webhook.name)?;
        #[allow(clippy::cast_precision_loss)]
        let success_pct = if stats.total > 0 {
            (stats.success as f64 / stats.total as f64) * 100.0
        } else {
            0.0
        };

        println!(
            "{:<20} {:<10} {:<10} {:<10} {:<12.1} {:<12.1}%",
            truncate(&webhook.name, 18),
            stats.total,
            stats.success,
            stats.failed,
            stats.avg_duration_ms,
            success_pct
        );
    }

    println!("{}", "-".repeat(80));

    Ok(())
}

/// Exports webhook audit logs for a domain (GDPR compliance).
///
/// # Arguments
///
/// * `domain` - Domain to export logs for
/// * `output` - Output file path (optional, prints to stdout if not specified)
/// * `data_dir` - Data directory for the audit database
///
/// # Errors
///
/// Returns an error if the audit database cannot be accessed.
pub fn cmd_webhook_export(domain: &str, output: Option<&Path>, data_dir: &Path) -> Result<()> {
    let audit_path = data_dir.join("webhook_audit.db");

    if !audit_path.exists() {
        println!("No webhook audit logs found.");
        return Ok(());
    }

    let logger = WebhookAuditLogger::new(&audit_path)?;
    let records = logger.export_domain_logs(domain)?;

    if records.is_empty() {
        println!("No audit logs found for domain '{domain}'.");
        return Ok(());
    }

    let json =
        serde_json::to_string_pretty(&records).map_err(|e| crate::Error::OperationFailed {
            operation: "export_audit_logs".to_string(),
            cause: e.to_string(),
        })?;

    if let Some(path) = output {
        std::fs::write(path, &json).map_err(|e| crate::Error::OperationFailed {
            operation: "write_export".to_string(),
            cause: e.to_string(),
        })?;
        println!("Exported {} records to {}", records.len(), path.display());
    } else {
        println!("{json}");
    }

    Ok(())
}

/// Deletes webhook audit logs for a domain (GDPR Right to Erasure).
///
/// # Arguments
///
/// * `domain` - Domain to delete logs for
/// * `force` - Skip confirmation prompt
/// * `data_dir` - Data directory for the audit database
///
/// # Errors
///
/// Returns an error if the audit database cannot be accessed.
pub fn cmd_webhook_delete_logs(domain: &str, force: bool, data_dir: &Path) -> Result<()> {
    let audit_path = data_dir.join("webhook_audit.db");

    if !audit_path.exists() {
        println!("No webhook audit logs found.");
        return Ok(());
    }

    let logger = WebhookAuditLogger::new(&audit_path)?;

    // Count records first
    let records = logger.export_domain_logs(domain)?;
    if records.is_empty() {
        println!("No audit logs found for domain '{domain}'.");
        return Ok(());
    }

    if !force {
        println!(
            "This will permanently delete {} audit log record(s) for domain '{domain}'.",
            records.len()
        );
        println!("This action cannot be undone.");
        println!();
        println!("To proceed, run with --force flag.");
        return Ok(());
    }

    let deleted = logger.delete_domain_logs(domain)?;
    println!("Deleted {deleted} audit log record(s) for domain '{domain}'.");

    Ok(())
}

/// Truncates a string to a maximum length.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_command_new() {
        let _cmd = WebhookCommand::new();
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn test_webhook_command_default() {
        let _cmd = WebhookCommand::default();
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
        assert_eq!(truncate("hi", 2), "hi");
    }
}
