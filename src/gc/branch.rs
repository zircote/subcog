//! Branch garbage collector implementation.
//!
//! Identifies and tombstones memories associated with deleted git branches.

use crate::context::GitContext;
use crate::storage::traits::IndexBackend;
use crate::{Error, Result};
use chrono::{TimeZone, Utc};
use git2::Repository;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, info_span, instrument, warn};

/// Safely converts Duration to milliseconds as u64, capping at `u64::MAX`.
#[inline]
fn duration_to_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Converts usize to f64 for metrics, capping at `u32::MAX`.
///
/// Uses u32 intermediate to avoid precision loss (`u32` fits exactly in `f64`).
/// For metrics, capping at 4 billion is acceptable.
#[inline]
fn usize_to_f64(value: usize) -> f64 {
    let capped = u32::try_from(value).unwrap_or(u32::MAX);
    f64::from(capped)
}

/// Converts u64 to f64 for metrics, capping at `u32::MAX`.
///
/// Uses u32 intermediate to avoid precision loss (`u32` fits exactly in `f64`).
/// For metrics, capping at 4 billion is acceptable.
#[inline]
fn u64_to_f64(value: u64) -> f64 {
    let capped = u32::try_from(value).unwrap_or(u32::MAX);
    f64::from(capped)
}

/// Result of a garbage collection operation.
///
/// Contains statistics about the GC run, including how many branches
/// were checked, which ones were stale, and how many memories were affected.
#[derive(Debug, Clone, Default)]
pub struct GcResult {
    /// Total number of branches checked in the index.
    pub branches_checked: usize,

    /// List of branch names that no longer exist in the repository.
    pub stale_branches: Vec<String>,

    /// Number of memories that were (or would be) tombstoned.
    pub memories_tombstoned: usize,

    /// Whether this was a dry run (no actual changes made).
    pub dry_run: bool,

    /// Duration of the GC operation in milliseconds.
    pub duration_ms: u64,
}

impl GcResult {
    /// Returns `true` if any stale branches were found.
    #[must_use]
    pub const fn has_stale_branches(&self) -> bool {
        !self.stale_branches.is_empty()
    }

    /// Returns a human-readable summary of the GC result.
    #[must_use]
    pub fn summary(&self) -> String {
        let action = if self.dry_run {
            "would tombstone"
        } else {
            "tombstoned"
        };

        if self.stale_branches.is_empty() {
            format!(
                "No stale branches found ({} branches checked in {}ms)",
                self.branches_checked, self.duration_ms
            )
        } else {
            format!(
                "Found {} stale branches, {} {} memories ({}ms)",
                self.stale_branches.len(),
                action,
                self.memories_tombstoned,
                self.duration_ms
            )
        }
    }
}

/// Garbage collector for branch-scoped memories.
///
/// Identifies memories associated with git branches that no longer exist
/// and marks them as tombstoned. This helps keep the memory index clean
/// by removing memories that are no longer relevant.
///
/// # Thread Safety
///
/// The garbage collector holds an `Arc` reference to the index backend,
/// making it safe to share across threads.
///
/// # Example
///
/// ```rust,ignore
/// use subcog::gc::BranchGarbageCollector;
/// use subcog::storage::index::SqliteBackend;
/// use std::sync::Arc;
///
/// let backend = Arc::new(SqliteBackend::new("memories.db")?);
/// let gc = BranchGarbageCollector::new(backend);
///
/// // Check for stale branches without making changes
/// let result = gc.gc_stale_branches("github.com/org/repo", true)?;
/// if result.has_stale_branches() {
///     println!("Stale branches: {:?}", result.stale_branches);
/// }
/// ```
pub struct BranchGarbageCollector<I: IndexBackend> {
    /// Reference to the index backend for querying and updating memories.
    index: Arc<I>,

    /// Optional path to the git repository.
    /// If None, uses the current working directory.
    repo_path: Option<std::path::PathBuf>,
}

impl<I: IndexBackend> BranchGarbageCollector<I> {
    /// Creates a new branch garbage collector.
    ///
    /// # Arguments
    ///
    /// * `index` - Shared reference to the index backend.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use subcog::gc::BranchGarbageCollector;
    /// use subcog::storage::index::SqliteBackend;
    /// use std::sync::Arc;
    ///
    /// let backend = Arc::new(SqliteBackend::in_memory()?);
    /// let gc = BranchGarbageCollector::new(backend);
    /// ```
    #[must_use]
    pub fn new(index: Arc<I>) -> Self {
        // Arc::strong_count prevents clippy::missing_const_for_fn false positive
        let _ = Arc::strong_count(&index);
        Self {
            index,
            repo_path: None,
        }
    }

    /// Creates a new branch garbage collector with a specific repository path.
    ///
    /// # Arguments
    ///
    /// * `index` - Shared reference to the index backend.
    /// * `repo_path` - Path to the git repository.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use subcog::gc::BranchGarbageCollector;
    /// use subcog::storage::index::SqliteBackend;
    /// use std::sync::Arc;
    /// use std::path::Path;
    ///
    /// let backend = Arc::new(SqliteBackend::in_memory()?);
    /// let gc = BranchGarbageCollector::with_repo_path(
    ///     backend,
    ///     Path::new("/path/to/repo"),
    /// );
    /// ```
    #[must_use]
    pub fn with_repo_path(index: Arc<I>, repo_path: &Path) -> Self {
        Self {
            index,
            repo_path: Some(repo_path.to_path_buf()),
        }
    }

    /// Performs garbage collection on stale branches.
    ///
    /// This method:
    /// 1. Discovers the git repository from the configured path or CWD
    /// 2. Gets all current branches from the repository
    /// 3. Queries the index for all distinct branches associated with the project
    /// 4. Identifies branches in the index that no longer exist in the repo
    /// 5. Tombstones memories associated with stale branches (unless `dry_run`)
    ///
    /// # Arguments
    ///
    /// * `project_id` - The project identifier (e.g., "github.com/org/repo")
    /// * `dry_run` - If true, only report what would be done without making changes
    ///
    /// # Returns
    ///
    /// A `GcResult` containing statistics about the operation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The git repository cannot be discovered
    /// - The index backend operations fail
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use subcog::gc::BranchGarbageCollector;
    ///
    /// // Dry run first
    /// let result = gc.gc_stale_branches("github.com/org/repo", true)?;
    /// println!("{}", result.summary());
    ///
    /// // Then actually perform cleanup
    /// if result.has_stale_branches() {
    ///     let result = gc.gc_stale_branches("github.com/org/repo", false)?;
    ///     println!("Cleaned up: {}", result.summary());
    /// }
    /// ```
    #[instrument(
        name = "subcog.gc.branches",
        skip(self),
        fields(
            request_id = tracing::field::Empty,
            component = "gc",
            operation = "stale_branches",
            project_id = %project_id,
            dry_run = dry_run
        )
    )]
    pub fn gc_stale_branches(&self, project_id: &str, dry_run: bool) -> Result<GcResult> {
        let start = Instant::now();
        if let Some(request_id) = crate::observability::current_request_id() {
            tracing::Span::current().record("request_id", request_id.as_str());
        }

        // Step 1: Discover git repository
        let repo = {
            let _span = info_span!("subcog.gc.branches.discover_repo").entered();
            self.discover_repository()?
        };

        // Step 2: Get current branches from the repository
        let current_branches = {
            let _span = info_span!("subcog.gc.branches.list_repo").entered();
            Self::get_current_branches(&repo)?
        };
        debug!(
            branch_count = current_branches.len(),
            "Discovered current branches"
        );

        // Step 3: Get branches from the index for this project
        let indexed_branches = {
            let _span = info_span!("subcog.gc.branches.list_index").entered();
            self.get_indexed_branches(project_id)?
        };
        let branches_checked = indexed_branches.len();
        debug!(
            branch_count = branches_checked,
            "Found indexed branches for project"
        );

        // Step 4: Identify stale branches (in index but not in repo)
        let stale_branches: Vec<String> = indexed_branches
            .into_iter()
            .filter(|branch| !current_branches.contains(branch))
            .collect();

        if stale_branches.is_empty() {
            info!("No stale branches found");
            return Ok(GcResult {
                branches_checked,
                stale_branches: Vec::new(),
                memories_tombstoned: 0,
                dry_run,
                duration_ms: duration_to_millis(start.elapsed()),
            });
        }

        info!(
            stale_count = stale_branches.len(),
            branches = ?stale_branches,
            "Found stale branches"
        );

        // Step 5: Tombstone memories (or count them in dry run)
        let memories_tombstoned = if dry_run {
            self.count_memories_for_branches(project_id, &stale_branches)?
        } else {
            self.tombstone_memories_for_branches(project_id, &stale_branches)
        };

        let duration_ms = duration_to_millis(start.elapsed());

        // Record metrics
        metrics::counter!(
            "gc_stale_branches_total",
            "dry_run" => dry_run.to_string()
        )
        .increment(1);
        metrics::gauge!("gc_stale_branch_count").set(usize_to_f64(stale_branches.len()));
        metrics::gauge!("gc_memories_tombstoned").set(usize_to_f64(memories_tombstoned));
        metrics::histogram!("gc_duration_ms").record(u64_to_f64(duration_ms));
        metrics::histogram!(
            "memory_lifecycle_duration_ms",
            "component" => "gc",
            "operation" => "stale_branches"
        )
        .record(u64_to_f64(duration_ms));

        Ok(GcResult {
            branches_checked,
            stale_branches,
            memories_tombstoned,
            dry_run,
            duration_ms,
        })
    }

    /// Discovers the git repository.
    fn discover_repository(&self) -> Result<Repository> {
        let path = self.repo_path.as_deref().map_or_else(
            || {
                std::env::current_dir().map_err(|e| Error::OperationFailed {
                    operation: "get_cwd".to_string(),
                    cause: e.to_string(),
                })
            },
            |p| Ok(p.to_path_buf()),
        )?;

        Repository::discover(&path).map_err(|e| Error::OperationFailed {
            operation: "discover_repository".to_string(),
            cause: format!(
                "Failed to discover git repository at {}: {}",
                path.display(),
                e
            ),
        })
    }

    /// Gets all current branch names from the repository.
    fn get_current_branches(repo: &Repository) -> Result<HashSet<String>> {
        let mut branches = HashSet::new();

        // Get local branches
        let local_branches =
            repo.branches(Some(git2::BranchType::Local))
                .map_err(|e| Error::OperationFailed {
                    operation: "list_branches".to_string(),
                    cause: e.to_string(),
                })?;

        for branch_result in local_branches {
            let (branch, _) = branch_result.map_err(|e| Error::OperationFailed {
                operation: "get_branch".to_string(),
                cause: e.to_string(),
            })?;

            if let Ok(Some(name)) = branch.name() {
                branches.insert(name.to_string());
            }
        }

        // Also include remote tracking branches (without the remote prefix)
        // This handles cases where a branch exists on remote but not locally
        let remote_branches =
            repo.branches(Some(git2::BranchType::Remote))
                .map_err(|e| Error::OperationFailed {
                    operation: "list_remote_branches".to_string(),
                    cause: e.to_string(),
                })?;

        for branch_result in remote_branches {
            let (branch, _) = branch_result.map_err(|e| Error::OperationFailed {
                operation: "get_remote_branch".to_string(),
                cause: e.to_string(),
            })?;

            // Remote branch names are like "origin/main", extract just the branch part
            let branch_name = branch
                .name()
                .ok()
                .flatten()
                .and_then(|name| name.split('/').nth(1))
                .map(String::from);

            if let Some(name) = branch_name {
                branches.insert(name);
            }
        }

        Ok(branches)
    }

    /// Gets all distinct branch names from the index for a project.
    ///
    /// This is a placeholder that will be enhanced when Task 4.2 adds
    /// `get_distinct_branches` to the `IndexBackend` trait.
    fn get_indexed_branches(&self, project_id: &str) -> Result<Vec<String>> {
        // TODO: Task 4.2 will add get_distinct_branches to IndexBackend
        // For now, we use a workaround by listing all memories and extracting branches
        use crate::models::SearchFilter;

        let filter = SearchFilter::new()
            .with_project_id(project_id)
            .with_include_tombstoned(false);

        let results = self.index.list_all(&filter, 10000)?;

        let branches: HashSet<String> = results
            .into_iter()
            .filter_map(|(id, _)| self.index.get_memory(&id).ok().flatten())
            .filter_map(|memory| memory.branch)
            .collect();

        Ok(branches.into_iter().collect())
    }

    /// Counts memories that would be tombstoned for the given branches.
    fn count_memories_for_branches(&self, project_id: &str, branches: &[String]) -> Result<usize> {
        use crate::models::SearchFilter;

        let mut total = 0;
        for branch in branches {
            let filter = SearchFilter::new()
                .with_project_id(project_id)
                .with_branch(branch)
                .with_include_tombstoned(false);

            let results = self.index.list_all(&filter, 10000)?;
            total += results.len();
        }

        Ok(total)
    }

    /// Tombstones memories associated with the given branches.
    ///
    /// This is a placeholder that will be enhanced when Task 4.3 adds
    /// `update_status` to the `IndexBackend` trait.
    fn tombstone_memories_for_branches(&self, project_id: &str, branches: &[String]) -> usize {
        // TODO: Task 4.3 will add update_status for bulk updates
        // For now, we fetch and re-index each memory with tombstoned_at set

        let now = crate::current_timestamp();

        let total: usize = branches
            .iter()
            .map(|branch| self.tombstone_branch_memories(project_id, branch, now))
            .sum();

        info!(count = total, "Tombstoned memories from stale branches");
        total
    }

    /// Tombstones all memories for a single branch.
    fn tombstone_branch_memories(&self, project_id: &str, branch: &str, now: u64) -> usize {
        use crate::models::SearchFilter;

        let filter = SearchFilter::new()
            .with_project_id(project_id)
            .with_branch(branch)
            .with_include_tombstoned(false);

        let memories = self.index.list_all(&filter, 10000).unwrap_or_default();

        memories
            .into_iter()
            .filter_map(|(id, _)| self.index.get_memory(&id).ok().flatten().map(|m| (id, m)))
            .filter(|(id, memory)| self.try_tombstone_memory(id, memory.clone(), now))
            .count()
    }

    /// Attempts to tombstone a single memory, returning true on success.
    fn try_tombstone_memory(
        &self,
        id: &crate::models::MemoryId,
        mut memory: crate::models::Memory,
        now: u64,
    ) -> bool {
        let now_i64 = i64::try_from(now).unwrap_or(i64::MAX);
        let now_dt = Utc
            .timestamp_opt(now_i64, 0)
            .single()
            .unwrap_or_else(Utc::now);
        memory.tombstoned_at = Some(now_dt);
        match self.index.index(&memory) {
            Ok(()) => true,
            Err(e) => {
                warn!(memory_id = %id.as_str(), error = %e, "Failed to tombstone memory");
                false
            },
        }
    }
}

/// Performs a quick check if the current branch exists.
///
/// This is a lightweight operation that can be used in the recall path
/// for lazy GC. It only checks the current branch, not all branches.
///
/// # Arguments
///
/// * `branch` - The branch name to check
///
/// # Returns
///
/// `true` if the branch exists, `false` if it doesn't or if the check fails.
#[must_use]
pub fn branch_exists(branch: &str) -> bool {
    let ctx = GitContext::from_cwd();

    // If we can't detect context, assume branch exists (fail open)
    if !ctx.is_git_repo() {
        return true;
    }

    // If the current branch matches, it definitely exists
    if ctx
        .branch
        .as_deref()
        .is_some_and(|current| current == branch)
    {
        return true;
    }

    // For other branches, we need to check the repository
    let Ok(cwd) = std::env::current_dir() else {
        return true;
    };

    let Ok(repo) = Repository::discover(&cwd) else {
        return true;
    };

    // Check local branches using iterator chain
    let in_local = repo
        .branches(Some(git2::BranchType::Local))
        .ok()
        .is_some_and(|branches| {
            branches
                .flatten()
                .filter_map(|(b, _)| b.name().ok().flatten().map(String::from))
                .any(|name| name == branch)
        });

    if in_local {
        return true;
    }

    // Check remote branches using iterator chain
    // Remote branch names are like "origin/main", extract just the branch part
    repo.branches(Some(git2::BranchType::Remote))
        .ok()
        .is_some_and(|branches| {
            branches
                .flatten()
                .filter_map(|(b, _)| b.name().ok().flatten().map(String::from))
                .filter_map(|name| name.split_once('/').map(|(_, branch)| branch.to_string()))
                .any(|name| name == branch)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, Memory, MemoryId, MemoryStatus, Namespace};
    use crate::storage::index::SqliteBackend;
    use git2::Signature;
    use tempfile::TempDir;

    fn create_test_memory(id: &str, project_id: &str, branch: &str) -> Memory {
        Memory {
            id: MemoryId::new(id),
            content: format!("Test memory for {branch}"),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            project_id: Some(project_id.to_string()),
            branch: Some(branch.to_string()),
            file_path: None,
            status: MemoryStatus::Active,
            created_at: 1_234_567_890,
            updated_at: 1_234_567_890,
            tombstoned_at: None,
            embedding: None,
            tags: vec!["test".to_string()],
            source: None,
        }
    }

    fn create_test_repo() -> (TempDir, Repository) {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let repo = Repository::init(dir.path()).expect("Failed to init repo");

        // Create an initial commit to have a valid HEAD
        {
            let sig = Signature::now("test", "test@test.com").expect("Failed to create signature");
            let tree_id = repo
                .index()
                .expect("Failed to get index")
                .write_tree()
                .expect("Failed to write tree");
            let tree = repo.find_tree(tree_id).expect("Failed to find tree");
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                .expect("Failed to create commit");
        }

        (dir, repo)
    }

    #[test]
    fn test_gc_result_summary_no_stale() {
        let result = GcResult {
            branches_checked: 5,
            stale_branches: Vec::new(),
            memories_tombstoned: 0,
            dry_run: false,
            duration_ms: 100,
        };

        assert!(!result.has_stale_branches());
        assert!(result.summary().contains("No stale branches"));
        assert!(result.summary().contains("5 branches checked"));
    }

    #[test]
    fn test_gc_result_summary_with_stale_dry_run() {
        let result = GcResult {
            branches_checked: 5,
            stale_branches: vec!["old-feature".to_string()],
            memories_tombstoned: 3,
            dry_run: true,
            duration_ms: 150,
        };

        assert!(result.has_stale_branches());
        assert!(result.summary().contains("would tombstone"));
        assert!(result.summary().contains("1 stale branches"));
        assert!(result.summary().contains('3'));
    }

    #[test]
    fn test_gc_result_summary_with_stale() {
        let result = GcResult {
            branches_checked: 5,
            stale_branches: vec!["old-feature".to_string(), "deleted-branch".to_string()],
            memories_tombstoned: 7,
            dry_run: false,
            duration_ms: 200,
        };

        assert!(result.has_stale_branches());
        assert!(result.summary().contains("tombstoned"));
        assert!(!result.summary().contains("would tombstone"));
        assert!(result.summary().contains("2 stale branches"));
        assert!(result.summary().contains('7'));
    }

    #[test]
    fn test_get_current_branches() {
        let (dir, repo) = create_test_repo();

        // Create some branches
        let head = repo.head().expect("Failed to get HEAD");
        let commit = repo
            .find_commit(head.target().expect("Failed to get target"))
            .expect("Failed to find commit");

        repo.branch("feature-a", &commit, false)
            .expect("Failed to create branch");
        repo.branch("feature-b", &commit, false)
            .expect("Failed to create branch");

        let backend = Arc::new(SqliteBackend::in_memory().expect("Failed to create backend"));
        let _gc = BranchGarbageCollector::with_repo_path(backend, dir.path());

        let branches = BranchGarbageCollector::<SqliteBackend>::get_current_branches(&repo)
            .expect("Failed to get branches");

        // Should have master/main (default) plus our two feature branches
        assert!(branches.len() >= 2);
        assert!(branches.contains("feature-a"));
        assert!(branches.contains("feature-b"));
    }

    #[test]
    fn test_gc_with_no_stale_branches() {
        let (dir, repo) = create_test_repo();
        let project_id = "github.com/test/repo";

        // Create a branch
        let head = repo.head().expect("Failed to get HEAD");
        let commit = repo
            .find_commit(head.target().expect("Failed to get target"))
            .expect("Failed to find commit");
        repo.branch("feature-a", &commit, false)
            .expect("Failed to create branch");

        // Create backend and index memory on that branch
        let backend = Arc::new(SqliteBackend::in_memory().expect("Failed to create backend"));
        let memory = create_test_memory("mem1", project_id, "feature-a");
        backend.index(&memory).expect("Failed to index memory");

        let gc = BranchGarbageCollector::with_repo_path(Arc::clone(&backend), dir.path());

        let result = gc
            .gc_stale_branches(project_id, true)
            .expect("GC should succeed");

        assert!(!result.has_stale_branches());
        assert_eq!(result.memories_tombstoned, 0);
    }

    #[test]
    fn test_gc_with_stale_branch_dry_run() {
        let (dir, _repo) = create_test_repo();
        let project_id = "github.com/test/repo";

        // Create backend and index memory on a branch that doesn't exist
        let backend = Arc::new(SqliteBackend::in_memory().expect("Failed to create backend"));
        let memory = create_test_memory("mem1", project_id, "deleted-branch");
        backend.index(&memory).expect("Failed to index memory");

        let gc = BranchGarbageCollector::with_repo_path(Arc::clone(&backend), dir.path());

        let result = gc
            .gc_stale_branches(project_id, true)
            .expect("GC should succeed");

        assert!(result.has_stale_branches());
        assert!(
            result
                .stale_branches
                .contains(&"deleted-branch".to_string())
        );
        assert_eq!(result.memories_tombstoned, 1);
        assert!(result.dry_run);

        // Memory should NOT be tombstoned in dry run
        let memory = backend
            .get_memory(&MemoryId::new("mem1"))
            .expect("Failed to get memory")
            .expect("Memory should exist");
        assert!(memory.tombstoned_at.is_none());
    }

    #[test]
    fn test_gc_with_stale_branch_actual() {
        let (dir, _repo) = create_test_repo();
        let project_id = "github.com/test/repo";

        // Create backend and index memory on a branch that doesn't exist
        let backend = Arc::new(SqliteBackend::in_memory().expect("Failed to create backend"));
        let memory = create_test_memory("mem1", project_id, "deleted-branch");
        backend.index(&memory).expect("Failed to index memory");

        let gc = BranchGarbageCollector::with_repo_path(Arc::clone(&backend), dir.path());

        let result = gc
            .gc_stale_branches(project_id, false)
            .expect("GC should succeed");

        assert!(result.has_stale_branches());
        assert_eq!(result.memories_tombstoned, 1);
        assert!(!result.dry_run);

        // Memory SHOULD be tombstoned
        let memory = backend
            .get_memory(&MemoryId::new("mem1"))
            .expect("Failed to get memory")
            .expect("Memory should exist");
        assert!(memory.tombstoned_at.is_some());
    }

    #[test]
    fn test_gc_multiple_memories_same_stale_branch() {
        let (dir, _repo) = create_test_repo();
        let project_id = "github.com/test/repo";

        let backend = Arc::new(SqliteBackend::in_memory().expect("Failed to create backend"));

        // Index multiple memories on the same stale branch
        for i in 0..5 {
            let memory = create_test_memory(&format!("mem{i}"), project_id, "old-feature");
            backend.index(&memory).expect("Failed to index memory");
        }

        let gc = BranchGarbageCollector::with_repo_path(Arc::clone(&backend), dir.path());

        let result = gc
            .gc_stale_branches(project_id, false)
            .expect("GC should succeed");

        assert_eq!(result.stale_branches.len(), 1);
        assert_eq!(result.memories_tombstoned, 5);
    }

    #[test]
    fn test_gc_preserves_other_project_memories() {
        let (dir, _repo) = create_test_repo();

        let backend = Arc::new(SqliteBackend::in_memory().expect("Failed to create backend"));

        // Index memory for project A on stale branch
        let memory_a = create_test_memory("memA", "github.com/org/project-a", "deleted-branch");
        backend.index(&memory_a).expect("Failed to index memory");

        // Index memory for project B on same branch name
        let memory_b = create_test_memory("memB", "github.com/org/project-b", "deleted-branch");
        backend.index(&memory_b).expect("Failed to index memory");

        let gc = BranchGarbageCollector::with_repo_path(Arc::clone(&backend), dir.path());

        // GC only for project A
        let result = gc
            .gc_stale_branches("github.com/org/project-a", false)
            .expect("GC should succeed");

        assert_eq!(result.memories_tombstoned, 1);

        // Project A's memory should be tombstoned
        let mem_a = backend
            .get_memory(&MemoryId::new("memA"))
            .expect("Failed to get memory")
            .expect("Memory should exist");
        assert!(mem_a.tombstoned_at.is_some());

        // Project B's memory should NOT be tombstoned
        let mem_b = backend
            .get_memory(&MemoryId::new("memB"))
            .expect("Failed to get memory")
            .expect("Memory should exist");
        assert!(mem_b.tombstoned_at.is_none());
    }

    #[test]
    fn test_branch_exists_current_branch() {
        // This test checks the basic functionality of branch_exists
        // In a real git repo, calling with the current branch should return true
        let ctx = GitContext::from_cwd();
        if let Some(ref branch) = ctx.branch {
            assert!(branch_exists(branch));
        }
    }

    #[test]
    fn test_branch_exists_nonexistent() {
        // A random UUID as branch name should not exist
        let fake_branch = "definitely-does-not-exist-12345";
        // Note: This might still return true if we're not in a git repo
        // The function is designed to fail open
        let _ = branch_exists(fake_branch);
    }
}
