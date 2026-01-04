//! Garbage collection command handler.
//!
//! Provides CLI interface for cleaning up stale branch memories.

use std::sync::Arc;

use subcog::context::GitContext;
use subcog::gc::{BranchGarbageCollector, GcResult};
use subcog::services::ServiceContainer;

/// GC command implementation.
///
/// Garbage collects memories associated with deleted git branches.
///
/// # Arguments
///
/// * `branch` - Optional specific branch to check. If not provided, checks all branches.
/// * `dry_run` - If true, shows what would be deleted without making changes.
/// * `purge` - If true, permanently deletes instead of tombstoning (future feature).
/// * `older_than` - For purge mode, only delete memories older than this duration.
///
/// # Examples
///
/// ```bash
/// # Check all stale branches in current project
/// subcog gc
///
/// # Dry run to see what would be cleaned up
/// subcog gc --dry-run
///
/// # Check a specific branch
/// subcog gc --branch=feature/old-feature
/// ```
pub fn cmd_gc(
    branch: Option<String>,
    dry_run: bool,
    _purge: bool,
    _older_than: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Detect git context
    let ctx = GitContext::from_cwd();

    let Some(project_id) = ctx.project_id else {
        eprintln!("Error: Not in a git repository");
        eprintln!("The gc command must be run from within a git repository.");
        return Err("Not in a git repository".into());
    };

    println!("Subcog Garbage Collection");
    println!("=========================");
    println!();
    println!("Project: {project_id}");
    if let Some(ref b) = branch {
        println!("Branch filter: {b}");
    }
    println!("Mode: {}", if dry_run { "dry-run" } else { "execute" });
    println!();

    // Create service container to get index backend
    let container = ServiceContainer::from_current_dir()?;

    // Get the index backend
    let index = container.index()?;
    let index_arc = Arc::new(index);

    // Create garbage collector
    let gc = BranchGarbageCollector::new(index_arc);

    // If a specific branch is provided, we need to handle it differently
    // For now, we'll run GC and filter results
    let result = gc.gc_stale_branches(&project_id, dry_run)?;

    // Display results
    display_gc_result(&result, branch.as_deref());

    Ok(())
}

/// Displays the GC result to the user.
fn display_gc_result(result: &GcResult, branch_filter: Option<&str>) {
    if result.stale_branches.is_empty() {
        println!("No stale branches found.");
        println!();
        println!(
            "Checked {} branches in {}ms",
            result.branches_checked, result.duration_ms
        );
        return;
    }

    // Filter branches if a specific one was requested
    let branches_to_show: Vec<&String> = if let Some(filter) = branch_filter {
        result
            .stale_branches
            .iter()
            .filter(|b| b.contains(filter))
            .collect()
    } else {
        result.stale_branches.iter().collect()
    };

    if branches_to_show.is_empty() {
        println!("No stale branches matching filter found.");
        println!();
        println!(
            "Checked {} branches in {}ms",
            result.branches_checked, result.duration_ms
        );
        return;
    }

    let action = if result.dry_run {
        "Would tombstone"
    } else {
        "Tombstoned"
    };

    println!("Stale branches found:");
    for branch in &branches_to_show {
        println!("  - {branch}");
    }
    println!();

    println!(
        "{action} {} memories from {} stale branches",
        result.memories_tombstoned,
        branches_to_show.len()
    );
    println!();
    println!(
        "Completed in {}ms (checked {} branches total)",
        result.duration_ms, result.branches_checked
    );

    if result.dry_run {
        println!();
        println!("This was a dry run. Run without --dry-run to apply changes.");
    }
}

/// Creates a GC service for programmatic use.
///
/// This function is used by the MCP tool handler to perform GC operations.
///
/// # Arguments
///
/// * `project_id` - The project identifier (e.g., "github.com/org/repo")
/// * `dry_run` - If true, only report what would be done
///
/// # Returns
///
/// A `GcResult` containing statistics about the operation.
///
/// # Errors
///
/// Returns an error if the GC operation fails.
#[allow(dead_code)]
pub fn run_gc(project_id: &str, dry_run: bool) -> Result<GcResult, Box<dyn std::error::Error>> {
    let container = ServiceContainer::from_current_dir()?;
    let index = container.index()?;
    let index_arc = Arc::new(index);

    let gc = BranchGarbageCollector::new(index_arc);
    let result = gc.gc_stale_branches(project_id, dry_run)?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_gc_result_no_stale() {
        let result = GcResult {
            branches_checked: 5,
            stale_branches: Vec::new(),
            memories_tombstoned: 0,
            dry_run: false,
            duration_ms: 100,
        };

        // Just verify it doesn't panic
        display_gc_result(&result, None);
    }

    #[test]
    fn test_display_gc_result_with_stale() {
        let result = GcResult {
            branches_checked: 5,
            stale_branches: vec!["old-feature".to_string(), "deleted-branch".to_string()],
            memories_tombstoned: 10,
            dry_run: true,
            duration_ms: 150,
        };

        // Just verify it doesn't panic
        display_gc_result(&result, None);
    }

    #[test]
    fn test_display_gc_result_with_filter() {
        let result = GcResult {
            branches_checked: 5,
            stale_branches: vec!["feature/old".to_string(), "bugfix/deleted".to_string()],
            memories_tombstoned: 5,
            dry_run: false,
            duration_ms: 100,
        };

        // Filter should only show matching branches
        display_gc_result(&result, Some("feature"));
    }
}
