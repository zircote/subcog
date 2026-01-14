//! Delete CLI command for removing memories.
//!
//! Provides both soft delete (tombstone) and hard delete capabilities.
//!
//! # Usage
//!
//! ```bash
//! # Soft delete (default) - can be restored via gc command
//! subcog delete abc123
//! subcog delete id1 id2 id3
//!
//! # Hard delete - permanent, irreversible
//! subcog delete --hard abc123
//!
//! # Skip confirmation
//! subcog delete --force abc123
//!
//! # Preview what would be deleted
//! subcog delete --dry-run abc123
//! ```

// Allow print_stdout/stderr in CLI module (consistent with main.rs)
#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]
// Allow pass-by-value for command functions (consistent with main.rs)
#![allow(clippy::needless_pass_by_value)]

use crate::Result;
use crate::models::{EventMeta, Memory, MemoryEvent, MemoryId, MemoryStatus};
use crate::observability::current_request_id;
use crate::security::record_event;
use crate::services::ServiceContainer;
use crate::storage::index::SqliteBackend;
use crate::storage::traits::IndexBackend;
use chrono::TimeZone;
use std::io::{self, Write};

/// Result of a delete operation.
#[derive(Debug, Default)]
pub struct DeleteResult {
    /// Number of memories successfully deleted.
    pub deleted: usize,
    /// Number of memories not found.
    pub not_found: usize,
    /// IDs that were deleted.
    pub deleted_ids: Vec<String>,
    /// IDs that were not found.
    pub not_found_ids: Vec<String>,
}

/// Executes the delete command.
///
/// # Arguments
///
/// * `ids` - Memory IDs to delete
/// * `hard` - If true, permanently delete; otherwise tombstone (soft delete)
/// * `force` - If true, skip confirmation prompt
/// * `dry_run` - If true, show what would be deleted without making changes
///
/// # Errors
///
/// Returns an error if storage access fails.
pub fn execute(ids: Vec<String>, hard: bool, force: bool, dry_run: bool) -> Result<()> {
    if ids.is_empty() {
        println!("No memory IDs provided. Usage: subcog delete <ID>...");
        return Ok(());
    }

    // Get service container with proper SQLite backend access
    let container = ServiceContainer::from_current_dir_or_user()?;
    let index = container.index()?;

    // Validate IDs and collect existing memories
    let mut valid_ids: Vec<(MemoryId, String, Memory)> = Vec::new();
    let mut not_found_ids = Vec::new();

    for id_str in &ids {
        let id = MemoryId::new(id_str);
        match index.get_memory(&id)? {
            Some(memory) => {
                valid_ids.push((id, memory.namespace.as_str().to_string(), memory));
            },
            None => {
                not_found_ids.push(id_str.clone());
            },
        }
    }

    // Report not found IDs
    if !not_found_ids.is_empty() {
        println!("Not found ({}):", not_found_ids.len());
        for id in &not_found_ids {
            println!("  - {id}");
        }
        println!();
    }

    if valid_ids.is_empty() {
        println!("No valid memories to delete.");
        return Ok(());
    }

    // Dry-run mode
    if dry_run {
        let action = if hard { "hard delete" } else { "tombstone" };
        println!(
            "Dry-run mode: would {action} {} memories:\n",
            valid_ids.len()
        );
        for (id, namespace, _) in &valid_ids {
            println!("  - {} ({})", id.as_str(), namespace);
        }
        return Ok(());
    }

    // Confirmation prompt (unless --force)
    if !force {
        let action = if hard {
            "PERMANENTLY DELETE"
        } else {
            "tombstone (soft delete)"
        };
        println!("About to {action} {} memories:\n", valid_ids.len());
        for (id, namespace, _) in &valid_ids {
            println!("  - {} ({})", id.as_str(), namespace);
        }
        println!();

        if hard {
            println!("WARNING: Hard delete is IRREVERSIBLE!");
        } else {
            println!("Note: Tombstoned memories can be restored or purged later with `subcog gc`.");
        }

        print!("\nProceed? [y/N] ");
        io::stdout()
            .flush()
            .map_err(|e| crate::Error::OperationFailed {
                operation: "flush_stdout".to_string(),
                cause: e.to_string(),
            })?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| crate::Error::OperationFailed {
                operation: "read_stdin".to_string(),
                cause: e.to_string(),
            })?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Execute deletion
    let result = if hard {
        hard_delete(&index, &valid_ids)
    } else {
        soft_delete(&index, &valid_ids)
    };

    // Report results
    let action = if hard { "Deleted" } else { "Tombstoned" };
    println!("\n{action} {} memories.", result.deleted);

    if !result.deleted_ids.is_empty() && result.deleted <= 10 {
        for id in &result.deleted_ids {
            println!("  ✓ {id}");
        }
    }

    if !hard {
        println!("\nTo permanently delete, run: subcog gc --purge");
        println!("To restore, tombstoned memories remain searchable with --include-tombstoned");
    }

    Ok(())
}

/// Performs soft delete (tombstone) on the given memories.
fn soft_delete(index: &SqliteBackend, ids: &[(MemoryId, String, Memory)]) -> DeleteResult {
    let mut result = DeleteResult::default();
    let now = crate::current_timestamp();
    let now_i64 = i64::try_from(now).unwrap_or(i64::MAX);
    let now_dt = chrono::Utc
        .timestamp_opt(now_i64, 0)
        .single()
        .unwrap_or_else(chrono::Utc::now);

    for (id, _namespace, memory) in ids {
        // Update memory status to tombstoned
        let mut updated_memory = memory.clone();
        updated_memory.status = MemoryStatus::Tombstoned;
        updated_memory.tombstoned_at = Some(now_dt);
        updated_memory.updated_at = now;

        // Re-index with updated status
        match index.index(&updated_memory) {
            Ok(()) => {
                result.deleted += 1;
                result.deleted_ids.push(id.as_str().to_string());

                record_event(MemoryEvent::Updated {
                    meta: EventMeta::with_timestamp("cli.delete", current_request_id(), now),
                    memory_id: id.clone(),
                    modified_fields: vec!["status".to_string(), "tombstoned_at".to_string()],
                });

                tracing::info!(
                    memory_id = %id.as_str(),
                    tombstoned_at = now,
                    "Tombstoned memory via CLI"
                );
            },
            Err(e) => {
                eprintln!("Failed to tombstone {}: {e}", id.as_str());
                result.not_found += 1;
                result.not_found_ids.push(id.as_str().to_string());
            },
        }
    }

    metrics::counter!("cli_delete_tombstoned_total").increment(result.deleted as u64);
    result
}

/// Performs hard delete (permanent) on the given memories.
fn hard_delete(index: &SqliteBackend, ids: &[(MemoryId, String, Memory)]) -> DeleteResult {
    let mut result = DeleteResult::default();

    for (id, _namespace, _memory) in ids {
        match index.remove(id) {
            Ok(true) => {
                result.deleted += 1;
                result.deleted_ids.push(id.as_str().to_string());

                record_event(MemoryEvent::Deleted {
                    meta: EventMeta::new("cli.delete", current_request_id()),
                    memory_id: id.clone(),
                    reason: "cli.delete --hard".to_string(),
                });

                tracing::info!(
                    memory_id = %id.as_str(),
                    "Hard deleted memory via CLI"
                );
            },
            Ok(false) => {
                result.not_found += 1;
                result.not_found_ids.push(id.as_str().to_string());
            },
            Err(e) => {
                eprintln!("Failed to delete {}: {e}", id.as_str());
                result.not_found += 1;
                result.not_found_ids.push(id.as_str().to_string());
            },
        }
    }

    metrics::counter!("cli_delete_hard_total").increment(result.deleted as u64);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, Namespace};
    use tempfile::TempDir;

    fn create_test_memory(id: &str) -> Memory {
        Memory {
            id: MemoryId::new(id),
            content: format!("Test content for {id}"),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: 1_000_000,
            updated_at: 1_000_000,
            tombstoned_at: None,
            expires_at: None,
            embedding: None,
            tags: vec![],
            #[cfg(feature = "group-scope")]
            group_id: None,
            source: None,
            is_summary: false,
            source_memory_ids: None,
            consolidation_timestamp: None,
        }
    }

    #[test]
    fn test_soft_delete_single() {
        let dir = TempDir::new().unwrap();
        let backend = SqliteBackend::new(dir.path().join("test.db")).unwrap();

        // Store a memory
        let memory = create_test_memory("test-soft-1");
        backend.index(&memory).unwrap();

        // Soft delete
        let ids = vec![(memory.id.clone(), "decisions".to_string(), memory.clone())];
        let result = soft_delete(&backend, &ids);

        assert_eq!(result.deleted, 1);
        assert_eq!(result.not_found, 0);

        // Verify it's tombstoned, not deleted
        let retrieved = backend.get_memory(&memory.id).unwrap().unwrap();
        assert_eq!(retrieved.status, MemoryStatus::Tombstoned);
    }

    #[test]
    fn test_hard_delete_single() {
        let dir = TempDir::new().unwrap();
        let backend = SqliteBackend::new(dir.path().join("test.db")).unwrap();

        // Store a memory
        let memory = create_test_memory("test-hard-1");
        backend.index(&memory).unwrap();

        // Hard delete
        let ids = vec![(memory.id.clone(), "decisions".to_string(), memory.clone())];
        let result = hard_delete(&backend, &ids);

        assert_eq!(result.deleted, 1);
        assert_eq!(result.not_found, 0);

        // Verify it's gone
        let retrieved = backend.get_memory(&memory.id).unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_delete_not_found() {
        let dir = TempDir::new().unwrap();
        let backend = SqliteBackend::new(dir.path().join("test.db")).unwrap();

        // Create a memory object but don't index it
        let memory = create_test_memory("nonexistent");

        // Try to delete non-existent memory
        let ids = vec![(
            MemoryId::new("nonexistent"),
            "decisions".to_string(),
            memory,
        )];
        let result = hard_delete(&backend, &ids);

        assert_eq!(result.deleted, 0);
        assert_eq!(result.not_found, 1);
    }

    #[test]
    fn test_delete_multiple() {
        let dir = TempDir::new().unwrap();
        let backend = SqliteBackend::new(dir.path().join("test.db")).unwrap();

        // Store multiple memories
        let m1 = create_test_memory("test-multi-1");
        let m2 = create_test_memory("test-multi-2");
        let m3 = create_test_memory("test-multi-3");
        backend.index(&m1).unwrap();
        backend.index(&m2).unwrap();
        backend.index(&m3).unwrap();

        // Hard delete all
        let ids = vec![
            (m1.id.clone(), "decisions".to_string(), m1),
            (m2.id.clone(), "decisions".to_string(), m2),
            (m3.id.clone(), "decisions".to_string(), m3),
        ];
        let result = hard_delete(&backend, &ids);

        assert_eq!(result.deleted, 3);
        assert_eq!(result.not_found, 0);
    }
}
