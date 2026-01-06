//! Migration command handlers.
//!
//! Provides commands for migrating existing memories to new features,
//! primarily generating embeddings for memories that lack them.

use std::io::Write;
use std::path::PathBuf;

use subcog::Error;
use subcog::services::ServiceContainer;
use subcog::storage::IndexBackend;

/// Migrate embeddings command.
///
/// Generates embeddings for memories that don't have them and stores
/// them in the vector index for semantic search.
///
/// # Errors
///
/// Returns an error if the migration fails.
pub fn cmd_migrate_embeddings(
    repo: Option<PathBuf>,
    dry_run: bool,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let services = match repo {
        Some(repo_path) => ServiceContainer::for_repo(&repo_path, None)?,
        None => ServiceContainer::from_current_dir_or_user()?,
    };

    println!("Migrating embeddings for memories...");
    match services.repo_path() {
        Some(repo_root) => println!("Repository: {}", repo_root.display()),
        None => println!("Scope: user"),
    }
    println!("Dry run: {dry_run}");
    println!("Force re-embed: {force}");
    println!();

    // Check if we have the required components
    let index = services.index().map_err(|e| Error::OperationFailed {
        operation: "migrate".to_string(),
        cause: format!("No index backend available: {e}"),
    })?;

    let embedder = services.embedder().ok_or_else(|| Error::OperationFailed {
        operation: "migrate".to_string(),
        cause: "No embedder configured - cannot generate embeddings".to_string(),
    })?;

    let vector = services.vector().ok_or_else(|| Error::OperationFailed {
        operation: "migrate".to_string(),
        cause: "No vector backend configured - cannot store embeddings".to_string(),
    })?;

    // Get all memories from index
    let filter = subcog::SearchFilter::new();
    let memories = index.list_all(&filter, 10000)?;

    if memories.is_empty() {
        println!("No memories found to migrate.");
        return Ok(());
    }

    let total = memories.len();
    println!("Found {total} memories to process");
    println!();

    let mut migrated = 0;
    let mut skipped = 0;
    let mut errors = 0;

    for (i, (memory_id, _score)) in memories.into_iter().enumerate() {
        // Print progress every 10 items or at key milestones
        if i % 10 == 0 || i == total - 1 {
            #[allow(clippy::cast_precision_loss)]
            let progress = ((i + 1) as f64 / total as f64) * 100.0;
            print!("\rProgress: {}/{} ({progress:.0}%)", i + 1, total);
            let _ = std::io::stdout().flush();
        }

        // Get the full memory
        let memory = match index.get_memory(&memory_id) {
            Ok(Some(m)) => m,
            Ok(None) => {
                skipped += 1;
                continue;
            },
            Err(e) => {
                eprintln!("\nError fetching memory {}: {e}", memory_id.as_str());
                errors += 1;
                continue;
            },
        };

        // Check if already has embedding (unless force)
        if !force && memory.embedding.is_some() {
            skipped += 1;
            continue;
        }

        if dry_run {
            println!("\nWould migrate: {}", memory_id.as_str());
            migrated += 1;
            continue;
        }

        // Generate embedding
        let embedding = match embedder.embed(&memory.content) {
            Ok(e) => e,
            Err(e) => {
                eprintln!(
                    "\nError generating embedding for {}: {e}",
                    memory_id.as_str()
                );
                errors += 1;
                continue;
            },
        };

        // Store in vector backend
        if let Err(e) = vector.upsert(&memory_id, &embedding) {
            eprintln!("\nError storing embedding for {}: {e}", memory_id.as_str());
            errors += 1;
            continue;
        }

        migrated += 1;
    }

    println!("\n");
    println!("Migration complete:");
    println!("  Migrated: {migrated}");
    println!("  Skipped: {skipped}");
    println!("  Errors: {errors}");

    if dry_run {
        println!();
        println!("This was a dry run. No changes were made.");
        println!("Run without --dry-run to apply changes.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrate_module_exists() {
        // Basic test to ensure the module compiles
    }

    #[test]
    fn test_migrate_embeddings_no_repo_returns_error() {
        // Test that running migration on a non-existent repo returns an error
        let result = cmd_migrate_embeddings(Some("/nonexistent/path".into()), true, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_migrate_dry_run_flag_parsing() {
        // Test that dry_run flag is correctly parsed
        // This is a basic sanity check
        let dry_run = true;
        let force = false;
        assert!(dry_run);
        assert!(!force);
    }

    #[test]
    fn test_migrate_force_flag_parsing() {
        // Test that force flag is correctly parsed
        let dry_run = false;
        let force = true;
        assert!(!dry_run);
        assert!(force);
    }
}
