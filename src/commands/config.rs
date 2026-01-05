//! Config command handler.
//!
//! Contains the implementation of the `config` CLI command and
//! display helpers for configuration output.

use subcog::config::{StorageBackendType, SubcogConfig};

/// Config command.
pub fn cmd_config(
    config: SubcogConfig,
    show: bool,
    _set: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if show {
        println!("Current Configuration");
        println!("=====================");
        println!();

        // Show config file sources
        println!("Config Files Loaded:");
        if config.config_sources.is_empty() {
            println!("  (none - using defaults)");
        } else {
            for source in &config.config_sources {
                println!("  - {}", source.display());
            }
        }
        println!();

        println!("Repository Path: {}", config.repo_path.display());
        println!("Data Directory: {}", config.data_dir.display());
        println!("Max Results: {}", config.max_results);
        println!("Default Search Mode: {:?}", config.default_search_mode);
        println!();

        println!("Observability:");
        display_tracing_config(&config);
        display_metrics_config(&config);
        display_logging_config(&config);
        println!();

        println!("Feature Flags:");
        println!("  Secrets Filter: {}", config.features.secrets_filter);
        println!("  PII Filter: {}", config.features.pii_filter);
        println!("  Multi-Domain: {}", config.features.multi_domain);
        println!("  Audit Log: {}", config.features.audit_log);
        println!("  LLM Features: {}", config.features.llm_features);
        println!("  Auto-Capture: {}", config.features.auto_capture);
        println!("  Consolidation: {}", config.features.consolidation);
        println!("  Org Scope Enabled: {}", config.features.org_scope_enabled);
        println!();
        println!("LLM Configuration:");
        println!("  Provider: {:?}", config.llm.provider);
        println!(
            "  Model: {}",
            config.llm.model.as_deref().unwrap_or("(default)")
        );
        println!(
            "  Base URL: {}",
            config.llm.base_url.as_deref().unwrap_or("(default)")
        );

        println!("\nSearch Intent:");
        println!("  Enabled: {}", config.search_intent.enabled);
        println!("  Use LLM: {}", config.search_intent.use_llm);
        println!("  LLM Timeout: {}ms", config.search_intent.llm_timeout_ms);
        println!(
            "  Min Confidence: {:.2}",
            config.search_intent.min_confidence
        );
        println!(
            "  Memory Count: {}-{}",
            config.search_intent.base_count, config.search_intent.max_count
        );
        println!("  Max Tokens: {}", config.search_intent.max_tokens);

        // Prompt customization
        let has_prompt_config = config.prompt.identity_addendum.is_some()
            || config.prompt.additional_guidance.is_some()
            || config.prompt.operation_guidance.capture.is_some()
            || config.prompt.operation_guidance.search.is_some()
            || config.prompt.operation_guidance.enrichment.is_some()
            || config.prompt.operation_guidance.consolidation.is_some();

        if has_prompt_config {
            println!("\nPrompt Customization:");
            if let Some(ref addendum) = config.prompt.identity_addendum {
                let preview: String = addendum.chars().take(50).collect();
                println!("  Identity Addendum: {}...", preview.replace('\n', " "));
            }
            if let Some(ref guidance) = config.prompt.additional_guidance {
                let preview: String = guidance.chars().take(50).collect();
                println!("  Additional Guidance: {}...", preview.replace('\n', " "));
            }
            if config.prompt.operation_guidance.capture.is_some() {
                println!("  Capture Guidance: (configured)");
            }
            if config.prompt.operation_guidance.search.is_some() {
                println!("  Search Guidance: (configured)");
            }
            if config.prompt.operation_guidance.enrichment.is_some() {
                println!("  Enrichment Guidance: (configured)");
            }
            if config.prompt.operation_guidance.consolidation.is_some() {
                println!("  Consolidation Guidance: (configured)");
            }
        }

        // Storage info
        println!("\nStorage:");
        display_storage_config(&config);
    } else {
        println!("Use --show to display configuration");
        println!("Use --set KEY=VALUE to set a value");
    }

    Ok(())
}

/// Helper to display tracing configuration.
fn display_tracing_config(config: &SubcogConfig) {
    let tracing_enabled = config
        .observability
        .tracing
        .as_ref()
        .and_then(|t| t.enabled)
        .unwrap_or(false);
    println!(
        "  Tracing: {}",
        if tracing_enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    let Some(ref tracing) = config.observability.tracing else {
        return;
    };
    if !tracing_enabled {
        return;
    }
    if let Some(ref otlp) = tracing.otlp {
        if let Some(ref endpoint) = otlp.endpoint {
            println!("    OTLP Endpoint: {endpoint}");
        }
        if let Some(ref protocol) = otlp.protocol {
            println!("    Protocol: {protocol}");
        }
    }
    if let Some(ratio) = tracing.sample_ratio {
        println!("    Sample Ratio: {ratio}");
    }
}

/// Helper to display metrics configuration.
fn display_metrics_config(config: &SubcogConfig) {
    let metrics_enabled = config
        .observability
        .metrics
        .as_ref()
        .and_then(|m| m.enabled)
        .unwrap_or(false);
    println!(
        "  Metrics: {}",
        if metrics_enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    let Some(ref metrics) = config.observability.metrics else {
        return;
    };
    if !metrics_enabled {
        return;
    }
    if let Some(port) = metrics.port {
        println!("    Prometheus Port: {port}");
    }
    if let Some(ref push_gw) = metrics.push_gateway {
        if let Some(ref endpoint) = push_gw.endpoint {
            println!("    Push Gateway: {endpoint}");
        }
    }
}

/// Helper to display logging configuration.
fn display_logging_config(config: &SubcogConfig) {
    let Some(ref logging) = config.observability.logging else {
        return;
    };
    println!("  Logging:");
    if let Some(ref format) = logging.format {
        println!("    Format: {format}");
    }
    if let Some(ref level) = logging.level {
        println!("    Level: {level}");
    }
    if let Some(ref filter) = logging.filter {
        println!("    Filter: {filter}");
    }
}

/// Helper to display storage configuration.
fn display_storage_config(config: &SubcogConfig) {
    let storage = &config.storage;

    // Project storage
    print!("  Project: {:?}", storage.project.backend);
    if let Some(ref path) = storage.project.path {
        print!(" (path: {path})");
    }
    if let Some(ref conn) = storage.project.connection_string {
        let display = if conn.len() > 30 {
            format!("{}...", &conn[..30])
        } else {
            conn.clone()
        };
        print!(" (connection: {display})");
    }
    println!();

    // User storage
    print!("  User: {:?}", storage.user.backend);
    match storage.user.backend {
        StorageBackendType::Sqlite => {
            if let Some(ref path) = storage.user.path {
                print!(" (path: {path})");
            } else {
                print!(" (path: ~/.config/subcog/memories.db)");
            }
        },
        StorageBackendType::Filesystem => {
            if let Some(ref path) = storage.user.path {
                print!(" (path: {path})");
            } else {
                print!(" (path: ~/.config/subcog/prompts/)");
            }
        },
        StorageBackendType::PostgreSQL | StorageBackendType::Redis => {
            if let Some(ref conn) = storage.user.connection_string {
                let display = if conn.len() > 30 {
                    format!("{}...", &conn[..30])
                } else {
                    conn.clone()
                };
                print!(" (connection: {display})");
            }
        },
    }
    println!();

    // Org storage
    print!("  Org: {:?}", storage.org.backend);
    if let Some(ref conn) = storage.org.connection_string {
        let display = if conn.len() > 30 {
            format!("{}...", &conn[..30])
        } else {
            conn.clone()
        };
        print!(" (connection: {display})");
    }
    println!(" (not yet implemented)");
}
