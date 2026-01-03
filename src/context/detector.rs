//! Git context detection implementation.

use git2::Repository;
use std::path::Path;

/// Git repository context information.
///
/// Provides project identification, branch name, and optional file context
/// derived from a git repository. All fields are optional to handle
/// non-git directories and edge cases gracefully.
///
/// # Examples
///
/// ```rust,ignore
/// use subcog::context::GitContext;
///
/// let ctx = GitContext::from_cwd();
/// match (&ctx.project_id, &ctx.branch) {
///     (Some(project), Some(branch)) => {
///         println!("Working on {project} @ {branch}");
///     }
///     (Some(project), None) => {
///         println!("Working on {project} (detached HEAD)");
///     }
///     (None, _) => {
///         println!("Not in a git repository");
///     }
/// }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitContext {
    /// Project identifier derived from git remote URL or repository directory name.
    ///
    /// Format: `org/repo` from remote URL, or just the directory name if no remote.
    /// Credentials are stripped from URLs for security.
    pub project_id: Option<String>,

    /// Current branch name.
    ///
    /// `None` if in detached HEAD state or if HEAD is unborn (empty repository).
    pub branch: Option<String>,

    /// Optional file path context.
    ///
    /// Can be set to provide file-specific context for operations.
    pub file_path: Option<String>,
}

impl GitContext {
    /// Detects git context from the current working directory.
    ///
    /// Uses `git2::Repository::discover()` to find the repository root,
    /// traversing parent directories if necessary.
    ///
    /// # Returns
    ///
    /// A `GitContext` with detected values. If not in a git repository,
    /// all fields will be `None`.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use subcog::context::GitContext;
    ///
    /// let ctx = GitContext::from_cwd();
    /// if ctx.project_id.is_some() {
    ///     println!("In a git repository");
    /// }
    /// ```
    #[must_use]
    pub fn from_cwd() -> Self {
        std::env::current_dir().map_or_else(|_| Self::default(), |cwd| Self::from_path(&cwd))
    }

    /// Detects git context from a specific path.
    ///
    /// Uses `git2::Repository::discover()` to find the repository containing
    /// the given path, traversing parent directories if necessary.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to detect context from. Can be any path within a repository.
    ///
    /// # Returns
    ///
    /// A `GitContext` with detected values. If the path is not in a git repository,
    /// all fields will be `None`.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use subcog::context::GitContext;
    /// use std::path::Path;
    ///
    /// let ctx = GitContext::from_path(Path::new("/path/to/repo/subdir"));
    /// println!("Project: {:?}", ctx.project_id);
    /// ```
    #[must_use]
    pub fn from_path(path: &Path) -> Self {
        let Ok(repo) = Repository::discover(path) else {
            return Self::default();
        };

        Self {
            project_id: detect_project_id(&repo),
            branch: detect_branch(&repo),
            file_path: None,
        }
    }

    /// Creates a new `GitContext` with the specified file path.
    ///
    /// This is useful for adding file-specific context to an existing detection.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The file path to associate with this context.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use subcog::context::GitContext;
    ///
    /// let ctx = GitContext::from_cwd()
    ///     .with_file_path("src/main.rs");
    /// ```
    #[must_use]
    pub fn with_file_path(mut self, file_path: impl Into<String>) -> Self {
        self.file_path = Some(file_path.into());
        self
    }

    /// Returns `true` if this context represents a git repository.
    ///
    /// A context is considered to be in a git repository if it has a project ID.
    #[must_use]
    pub const fn is_git_repo(&self) -> bool {
        self.project_id.is_some()
    }

    /// Returns `true` if the repository is in detached HEAD state.
    ///
    /// Returns `false` if not in a git repository.
    #[must_use]
    pub const fn is_detached(&self) -> bool {
        self.project_id.is_some() && self.branch.is_none()
    }
}

/// Detects the project ID from a repository.
///
/// Priority:
/// 1. Remote "origin" URL (sanitized)
/// 2. First available remote URL (sanitized)
/// 3. Repository directory name
fn detect_project_id(repo: &Repository) -> Option<String> {
    // Try to get origin remote first
    if let Ok(origin) = repo.find_remote("origin") {
        if let Some(url) = origin.url() {
            if let Some(project_id) = sanitize_git_url(url) {
                return Some(project_id);
            }
        }
    }

    // Try any other remote using iterator chain
    let from_remote = repo.remotes().ok().and_then(|remotes| {
        remotes
            .iter()
            .flatten()
            .filter_map(|name| repo.find_remote(name).ok())
            .find_map(|remote| remote.url().and_then(sanitize_git_url))
    });

    if from_remote.is_some() {
        return from_remote;
    }

    // Fall back to repository directory name
    repo.workdir()
        .or_else(|| repo.path().parent())
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(String::from)
}

/// Detects the current branch name.
///
/// Returns `None` if:
/// - HEAD is detached (pointing directly to a commit)
/// - HEAD is unborn (empty repository with no commits)
fn detect_branch(repo: &Repository) -> Option<String> {
    let head = repo.head().ok()?;

    // Check if HEAD is a branch (not detached)
    if !head.is_branch() {
        return None;
    }

    // Get the short name (e.g., "main" instead of "refs/heads/main")
    head.shorthand().map(String::from)
}

/// Sanitizes a git remote URL by removing credentials and normalizing format.
///
/// # Security
///
/// This function strips any embedded credentials from URLs:
/// - `https://user:password@host/path` -> `host/path`
/// - `git@host:org/repo.git` -> `host/org/repo`
///
/// # Supported Formats
///
/// | Format | Example | Result |
/// |--------|---------|--------|
/// | HTTPS | `https://github.com/org/repo.git` | `github.com/org/repo` |
/// | HTTPS with creds | `https://user:pass@github.com/org/repo` | `github.com/org/repo` |
/// | SSH | `git@github.com:org/repo.git` | `github.com/org/repo` |
/// | Git protocol | `git://github.com/org/repo.git` | `github.com/org/repo` |
fn sanitize_git_url(url: &str) -> Option<String> {
    let url = url.trim();

    if url.is_empty() {
        return None;
    }

    // Handle SSH format: git@host:org/repo.git
    if let Some(ssh_part) = url.strip_prefix("git@") {
        return sanitize_ssh_url(ssh_part);
    }

    // Handle URL format: https://host/path or git://host/path
    sanitize_http_url(url)
}

/// Sanitizes an SSH-format URL (after stripping "git@" prefix).
///
/// Input: `github.com:org/repo.git`
/// Output: `github.com/org/repo`
fn sanitize_ssh_url(url: &str) -> Option<String> {
    // Split on ':' to separate host from path
    let (host, path) = url.split_once(':')?;

    if host.is_empty() || path.is_empty() {
        return None;
    }

    // Remove .git suffix and construct result
    let path = path.strip_suffix(".git").unwrap_or(path);

    Some(format!("{host}/{path}"))
}

/// Sanitizes an HTTP/HTTPS/Git protocol URL.
///
/// Handles:
/// - `https://github.com/org/repo.git`
/// - `https://user:pass@github.com/org/repo.git`
/// - `git://github.com/org/repo.git`
fn sanitize_http_url(url: &str) -> Option<String> {
    // Strip protocol prefix
    let without_protocol = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("git://"))?;

    // Strip credentials if present (user:pass@host -> host)
    let without_creds = without_protocol
        .find('@')
        .map_or(without_protocol, |at_pos| &without_protocol[at_pos + 1..]);

    if without_creds.is_empty() {
        return None;
    }

    // Remove .git suffix and trailing slashes
    let result = without_creds
        .strip_suffix(".git")
        .unwrap_or(without_creds)
        .trim_end_matches('/');

    if result.is_empty() {
        None
    } else {
        Some(result.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;
    use tempfile::TempDir;

    // ============================================================================
    // URL Sanitization Tests
    // ============================================================================

    #[test]
    fn test_sanitize_https_url() {
        assert_eq!(
            sanitize_git_url("https://github.com/org/repo.git"),
            Some("github.com/org/repo".to_string())
        );
    }

    #[test]
    fn test_sanitize_https_url_without_git_suffix() {
        assert_eq!(
            sanitize_git_url("https://github.com/org/repo"),
            Some("github.com/org/repo".to_string())
        );
    }

    #[test]
    fn test_sanitize_https_url_with_credentials() {
        assert_eq!(
            sanitize_git_url("https://user:password@github.com/org/repo.git"),
            Some("github.com/org/repo".to_string())
        );
    }

    #[test]
    fn test_sanitize_https_url_with_user_only() {
        assert_eq!(
            sanitize_git_url("https://user@github.com/org/repo.git"),
            Some("github.com/org/repo".to_string())
        );
    }

    #[test]
    fn test_sanitize_ssh_url() {
        assert_eq!(
            sanitize_git_url("git@github.com:org/repo.git"),
            Some("github.com/org/repo".to_string())
        );
    }

    #[test]
    fn test_sanitize_ssh_url_without_git_suffix() {
        assert_eq!(
            sanitize_git_url("git@github.com:org/repo"),
            Some("github.com/org/repo".to_string())
        );
    }

    #[test]
    fn test_sanitize_git_protocol_url() {
        assert_eq!(
            sanitize_git_url("git://github.com/org/repo.git"),
            Some("github.com/org/repo".to_string())
        );
    }

    #[test]
    fn test_sanitize_http_url() {
        assert_eq!(
            sanitize_git_url("http://github.com/org/repo.git"),
            Some("github.com/org/repo".to_string())
        );
    }

    #[test]
    fn test_sanitize_url_with_trailing_slash() {
        assert_eq!(
            sanitize_git_url("https://github.com/org/repo/"),
            Some("github.com/org/repo".to_string())
        );
    }

    #[test]
    fn test_sanitize_empty_url() {
        assert_eq!(sanitize_git_url(""), None);
    }

    #[test]
    fn test_sanitize_whitespace_url() {
        assert_eq!(sanitize_git_url("   "), None);
    }

    #[test]
    fn test_sanitize_invalid_url() {
        // No protocol, treated as invalid
        assert_eq!(sanitize_git_url("just-a-string"), None);
    }

    #[test]
    fn test_sanitize_url_with_complex_credentials() {
        // Password with special characters
        assert_eq!(
            sanitize_git_url("https://user:p%40ssw0rd!@github.com/org/repo.git"),
            Some("github.com/org/repo".to_string())
        );
    }

    #[test]
    fn test_sanitize_gitlab_url() {
        assert_eq!(
            sanitize_git_url("https://gitlab.com/group/subgroup/repo.git"),
            Some("gitlab.com/group/subgroup/repo".to_string())
        );
    }

    #[test]
    fn test_sanitize_bitbucket_url() {
        assert_eq!(
            sanitize_git_url("git@bitbucket.org:team/repo.git"),
            Some("bitbucket.org/team/repo".to_string())
        );
    }

    #[test]
    fn test_sanitize_self_hosted_url() {
        assert_eq!(
            sanitize_git_url("https://git.company.com/team/project.git"),
            Some("git.company.com/team/project".to_string())
        );
    }

    // ============================================================================
    // GitContext Construction Tests
    // ============================================================================

    #[test]
    fn test_git_context_default() {
        let ctx = GitContext::default();
        assert!(ctx.project_id.is_none());
        assert!(ctx.branch.is_none());
        assert!(ctx.file_path.is_none());
    }

    #[test]
    fn test_git_context_with_file_path() {
        let ctx = GitContext::default().with_file_path("src/main.rs");
        assert_eq!(ctx.file_path, Some("src/main.rs".to_string()));
    }

    #[test]
    fn test_git_context_is_git_repo() {
        let ctx = GitContext {
            project_id: Some("org/repo".to_string()),
            branch: Some("main".to_string()),
            file_path: None,
        };
        assert!(ctx.is_git_repo());

        let non_repo = GitContext::default();
        assert!(!non_repo.is_git_repo());
    }

    #[test]
    fn test_git_context_is_detached() {
        let detached = GitContext {
            project_id: Some("org/repo".to_string()),
            branch: None,
            file_path: None,
        };
        assert!(detached.is_detached());

        let attached = GitContext {
            project_id: Some("org/repo".to_string()),
            branch: Some("main".to_string()),
            file_path: None,
        };
        assert!(!attached.is_detached());

        let non_repo = GitContext::default();
        assert!(!non_repo.is_detached());
    }

    // ============================================================================
    // Repository Detection Tests
    // ============================================================================

    fn create_test_repo() -> (TempDir, Repository) {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        // Create an initial commit
        {
            let sig = Signature::now("test", "test@test.com").unwrap();
            let tree_id = repo.index().unwrap().write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                .unwrap();
        }

        (dir, repo)
    }

    #[test]
    fn test_from_path_non_git_directory() {
        let dir = TempDir::new().unwrap();
        let ctx = GitContext::from_path(dir.path());

        assert!(ctx.project_id.is_none());
        assert!(ctx.branch.is_none());
        assert!(ctx.file_path.is_none());
    }

    #[test]
    fn test_from_path_git_repo_no_remote() {
        let (dir, _repo) = create_test_repo();
        let ctx = GitContext::from_path(dir.path());

        // Should fall back to directory name
        assert!(ctx.project_id.is_some());
        assert!(ctx.branch.is_some());
    }

    #[test]
    fn test_from_path_git_repo_with_remote() {
        let (dir, repo) = create_test_repo();

        // Add a remote
        repo.remote("origin", "https://github.com/testorg/testrepo.git")
            .unwrap();

        let ctx = GitContext::from_path(dir.path());

        assert_eq!(
            ctx.project_id,
            Some("github.com/testorg/testrepo".to_string())
        );
        assert!(ctx.branch.is_some());
    }

    #[test]
    fn test_from_path_subdirectory() {
        let (dir, repo) = create_test_repo();

        repo.remote("origin", "https://github.com/org/repo.git")
            .unwrap();

        // Create a subdirectory
        let subdir = dir.path().join("src").join("lib");
        std::fs::create_dir_all(&subdir).unwrap();

        let ctx = GitContext::from_path(&subdir);

        // Should still detect the repository
        assert_eq!(ctx.project_id, Some("github.com/org/repo".to_string()));
    }

    #[test]
    fn test_from_path_detached_head() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        // Create initial commit
        let sig = Signature::now("test", "test@test.com").unwrap();
        let tree_id = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let commit_oid = repo
            .commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        // Detach HEAD
        repo.set_head_detached(commit_oid).unwrap();

        let ctx = GitContext::from_path(dir.path());

        assert!(ctx.project_id.is_some()); // Should still have project ID
        assert!(ctx.branch.is_none()); // Branch should be None
        assert!(ctx.is_detached());
    }

    #[test]
    fn test_from_path_empty_repo() {
        let dir = TempDir::new().unwrap();
        let _repo = Repository::init(dir.path()).unwrap();

        // Empty repo - no commits yet
        let ctx = GitContext::from_path(dir.path());

        // Should still detect as a repo via directory name
        assert!(ctx.project_id.is_some());
        // Branch may be None since HEAD is unborn
        // This is expected behavior
    }

    #[test]
    fn test_from_path_remote_with_credentials() {
        let (dir, repo) = create_test_repo();

        // Add a remote with embedded credentials
        repo.remote("origin", "https://user:secretpass@github.com/org/repo.git")
            .unwrap();

        let ctx = GitContext::from_path(dir.path());

        // Credentials should be stripped
        assert_eq!(ctx.project_id, Some("github.com/org/repo".to_string()));
        // Verify no credentials in project_id
        assert!(!ctx.project_id.as_ref().unwrap().contains("user"));
        assert!(!ctx.project_id.as_ref().unwrap().contains("secret"));
    }

    #[test]
    fn test_from_path_ssh_remote() {
        let (dir, repo) = create_test_repo();

        repo.remote("origin", "git@github.com:org/repo.git")
            .unwrap();

        let ctx = GitContext::from_path(dir.path());

        assert_eq!(ctx.project_id, Some("github.com/org/repo".to_string()));
    }

    #[test]
    fn test_from_path_multiple_remotes() {
        let (dir, repo) = create_test_repo();

        // Add origin and another remote
        repo.remote("upstream", "https://github.com/upstream/repo.git")
            .unwrap();
        repo.remote("origin", "https://github.com/fork/repo.git")
            .unwrap();

        let ctx = GitContext::from_path(dir.path());

        // Should prefer origin
        assert_eq!(ctx.project_id, Some("github.com/fork/repo".to_string()));
    }

    #[test]
    fn test_from_path_non_origin_remote() {
        let (dir, repo) = create_test_repo();

        // Only add a non-origin remote
        repo.remote("upstream", "https://github.com/upstream/repo.git")
            .unwrap();

        let ctx = GitContext::from_path(dir.path());

        // Should fall back to any available remote
        assert_eq!(ctx.project_id, Some("github.com/upstream/repo".to_string()));
    }

    #[test]
    fn test_from_path_feature_branch() {
        let (dir, repo) = create_test_repo();

        // Create and checkout a feature branch
        let head = repo.head().unwrap().target().unwrap();
        let commit = repo.find_commit(head).unwrap();
        repo.branch("feature/my-feature", &commit, false).unwrap();
        repo.set_head("refs/heads/feature/my-feature").unwrap();

        let ctx = GitContext::from_path(dir.path());

        assert_eq!(ctx.branch, Some("feature/my-feature".to_string()));
    }

    #[test]
    fn test_from_path_worktree() {
        let (dir, repo) = create_test_repo();

        // Create a branch for the worktree
        let head = repo.head().unwrap().target().unwrap();
        let commit = repo.find_commit(head).unwrap();
        repo.branch("worktree-branch", &commit, false).unwrap();

        // Create a worktree
        let worktree_path = dir.path().parent().unwrap().join("test-worktree");
        repo.worktree(
            "test-worktree",
            &worktree_path,
            Some(
                git2::WorktreeAddOptions::new().reference(Some(
                    &repo
                        .find_branch("worktree-branch", git2::BranchType::Local)
                        .unwrap()
                        .into_reference(),
                )),
            ),
        )
        .unwrap();

        // Detect from worktree path
        let ctx = GitContext::from_path(&worktree_path);

        // Should detect the same project
        assert!(ctx.project_id.is_some());
        assert_eq!(ctx.branch, Some("worktree-branch".to_string()));

        // Cleanup worktree
        std::fs::remove_dir_all(&worktree_path).ok();
    }

    // ============================================================================
    // Edge Cases
    // ============================================================================

    #[test]
    fn test_from_path_bare_repo() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init_bare(dir.path()).unwrap();

        repo.remote("origin", "https://github.com/org/repo.git")
            .unwrap();

        let ctx = GitContext::from_path(dir.path());

        // Should still detect project ID from remote
        assert_eq!(ctx.project_id, Some("github.com/org/repo".to_string()));
    }

    #[test]
    fn test_sanitize_ssh_url_no_path() {
        // Malformed SSH URL with no path
        assert_eq!(sanitize_ssh_url("github.com:"), None);
    }

    #[test]
    fn test_sanitize_ssh_url_no_host() {
        // Malformed SSH URL with no host
        assert_eq!(sanitize_ssh_url(":org/repo"), None);
    }
}
