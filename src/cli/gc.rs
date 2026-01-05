//! Garbage collection CLI command.

#![allow(clippy::print_stdout)]

use crate::Result;
use crate::services::TombstoneService;
use crate::storage::persistence::FilesystemBackend;
use std::sync::Arc;
use std::time::Duration;

/// Runs garbage collection.
///
/// # Errors
///
/// Returns an error if persistence access or tombstone operations fail.
pub fn execute(dry_run: bool, purge: bool, older_than_days: u64) -> Result<()> {
    // Use project-local persistence
    let persistence = Arc::new(FilesystemBackend::new(".subcog/memories"))
        as Arc<dyn crate::storage::traits::PersistenceBackend>;
    let tombstone_service = TombstoneService::new(persistence.clone());

    if dry_run {
        println!("Dry-run mode: showing what would be deleted\n");

        // List tombstoned memories
        let all_ids = persistence.list_ids()?;
        let mut tombstoned_count = 0;

        for id in all_ids {
            let Some(memory) = persistence.get(&id)? else {
                continue;
            };
            if memory.status != crate::models::MemoryStatus::Tombstoned {
                continue;
            }
            tombstoned_count += 1;
            println!(
                "Would delete: {} ({})",
                id.as_str(),
                memory.namespace.as_str()
            );
        }

        println!("\nTotal tombstoned memories: {tombstoned_count}");
        return Ok(());
    }

    if purge {
        let older_than = Duration::from_secs(older_than_days * 24 * 60 * 60);
        let purged = tombstone_service.purge_tombstoned(older_than)?;

        println!(
            "Purged {purged} tombstoned memories older than {older_than_days} days"
        );
    } else {
        println!("Garbage collection complete");
        println!("Use --purge to permanently delete tombstoned memories");
        println!("Use --dry-run to preview what would be deleted");
    }

    Ok(())
}
