//! Import and export command handlers.

use std::path::PathBuf;
use std::sync::Arc;

use subcog::config::{Config, SubcogConfig};
use subcog::io::formats::Format;
use subcog::io::services::export::{ExportOptions, ExportService};
use subcog::io::services::import::{ImportOptions, ImportService};
use subcog::models::{Domain, Namespace};
use subcog::services::CaptureService;
use subcog::storage::index::SqliteBackend;
use subcog::{Error, Result};

/// Executes the import command.
#[allow(clippy::too_many_arguments)]
pub fn cmd_import(
    config: &Config,
    file: PathBuf,
    format: Option<String>,
    namespace: Option<String>,
    domain: Option<String>,
    skip_duplicates: bool,
    dry_run: bool,
) -> Result<()> {
    // Determine format from argument or file extension
    let format = match format {
        Some(f) => f.parse::<Format>()?,
        None => Format::from_path(&file)?,
    };

    if !format.supports_import() {
        return Err(Error::InvalidInput(format!(
            "Format '{}' does not support import",
            format
        )));
    }

    // Parse namespace
    let default_namespace = namespace
        .as_deref()
        .and_then(Namespace::parse)
        .unwrap_or(Namespace::Decisions);

    // Parse domain
    let default_domain = domain.as_deref().map(parse_domain).unwrap_or_default();

    let options = ImportOptions {
        format,
        default_namespace,
        default_domain,
        skip_duplicates,
        skip_invalid: true,
        dry_run,
    };

    // Create capture service
    let capture_service = Arc::new(CaptureService::new(config.clone()));
    let import_service = ImportService::new(capture_service);

    // Progress callback for line-based updates
    let progress_callback = Box::new(|progress: &subcog::io::services::import::ImportProgress| {
        if let Some(total) = progress.total_estimate {
            print!(
                "\rProcessing: {}/{} ({:.1}%) - Imported: {}, Skipped: {}, Invalid: {}",
                progress.processed,
                total,
                progress.percent_complete().unwrap_or(0.0),
                progress.imported,
                progress.skipped_duplicates,
                progress.skipped_invalid,
            );
        } else {
            print!(
                "\rProcessing: {} - Imported: {}, Skipped: {}, Invalid: {}",
                progress.processed,
                progress.imported,
                progress.skipped_duplicates,
                progress.skipped_invalid,
            );
        }
        // Flush to ensure output appears immediately
        use std::io::Write;
        let _ = std::io::stdout().flush();
    });

    let result = import_service.import_from_file(&file, options, Some(progress_callback))?;

    // Clear progress line and print final summary
    println!();
    println!();

    if dry_run {
        println!("Dry run completed (no changes made):");
    } else {
        println!("Import completed:");
    }

    println!("  Imported:         {}", result.imported);
    println!("  Skipped (dupe):   {}", result.skipped_duplicates);
    println!("  Skipped (invalid):{}", result.skipped_invalid);
    println!("  Total processed:  {}", result.total_processed);

    if !result.warnings.is_empty() {
        println!();
        println!("Warnings ({}):", result.warnings.len());
        for warning in result.warnings.iter().take(10) {
            println!("  - {warning}");
        }
        if result.warnings.len() > 10 {
            println!("  ... and {} more", result.warnings.len() - 10);
        }
    }

    if !result.errors.is_empty() {
        println!();
        println!("Errors ({}):", result.errors.len());
        for error in result.errors.iter().take(10) {
            println!("  - {error}");
        }
        if result.errors.len() > 10 {
            println!("  ... and {} more", result.errors.len() - 10);
        }
    }

    Ok(())
}

/// Executes the export command.
#[allow(clippy::too_many_arguments)]
pub fn cmd_export(
    config: &SubcogConfig,
    output: PathBuf,
    format: Option<String>,
    filter: Option<String>,
    limit: Option<usize>,
    domain: Option<String>,
) -> Result<()> {
    // Determine format from argument or file extension
    let format = match format {
        Some(f) => f.parse::<Format>()?,
        None => Format::from_path(&output)?,
    };

    if !format.supports_export() {
        return Err(Error::InvalidInput(format!(
            "Format '{}' does not support export",
            format
        )));
    }

    // Build filter query string with domain if specified
    let mut filter_parts = Vec::new();
    if let Some(f) = filter {
        filter_parts.push(f);
    }
    if let Some(d) = domain {
        // Domain is not a standard filter, but we can document this limitation
        // For now, we'd need to handle this at the service level
        filter_parts.push(format!("domain:{d}"));
    }
    let filter_query = if filter_parts.is_empty() {
        None
    } else {
        Some(filter_parts.join(" "))
    };

    let mut options = ExportOptions::default().with_format(format);
    if let Some(f) = filter_query {
        options = options.with_filter(f);
    }
    if let Some(l) = limit {
        options = options.with_limit(l);
    }

    // Create index backend for querying
    let sqlite_path = config.data_dir.join("index.sqlite");
    let index = Arc::new(SqliteBackend::new(&sqlite_path)?);
    let export_service = ExportService::new(index);

    // Progress callback
    let progress_callback = Box::new(|exported: usize, total: Option<usize>| {
        if let Some(t) = total {
            print!("\rExporting: {}/{}", exported, t);
        } else {
            print!("\rExporting: {}", exported);
        }
        use std::io::Write;
        let _ = std::io::stdout().flush();
    });

    let result = export_service.export_to_file(&output, options, Some(progress_callback))?;

    // Clear progress line and print final summary
    println!();
    println!();

    println!("Export completed:");
    println!("  Exported:     {}", result.exported);
    println!("  Total matched:{}", result.total_matched);
    println!("  Format:       {}", result.format);
    if let Some(path) = result.output_path {
        println!("  Output:       {path}");
    }

    Ok(())
}

/// Parses a domain string into a Domain.
fn parse_domain(s: &str) -> Domain {
    match s.to_lowercase().as_str() {
        "user" => Domain::for_user(),
        "org" => Domain::for_org(),
        _ => Domain::new(), // Default to project-scoped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_domain() {
        // User scope tests
        assert!(parse_domain("user").is_user());
        assert!(parse_domain("USER").is_user());

        // Project scope tests (default for unknown values)
        assert!(parse_domain("project").is_project_scoped());
        assert!(parse_domain("unknown").is_project_scoped());

        // Org scope - returns Domain with organization set
        let org_domain = parse_domain("org");
        assert!(!org_domain.is_user());
        assert!(!org_domain.is_project_scoped());
    }
}
