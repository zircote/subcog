//! Context detection for git repositories.
//!
//! This module provides automatic detection of project context from git repositories,
//! including project identification, branch names, and file paths.
//!
//! # Overview
//!
//! The context detector uses git2 to discover repository information from the
//! current working directory or a specified path. It handles edge cases gracefully:
//!
//! - Non-git directories (returns `GitContext` with all fields `None`)
//! - Detached HEAD state (branch is `None`)
//! - Git worktrees (correctly identifies the main repository)
//! - Credentials in remote URLs (automatically sanitized)
//!
//! # Example
//!
//! ```rust,ignore
//! use subcog::context::GitContext;
//!
//! // Detect from current working directory
//! let ctx = GitContext::from_cwd();
//! if let Some(project) = ctx.project_id {
//!     println!("Project: {project}");
//! }
//!
//! // Detect from specific path
//! let ctx = GitContext::from_path("/path/to/repo");
//! println!("Branch: {:?}", ctx.branch);
//! ```
//!
//! # Security
//!
//! Remote URLs are automatically sanitized to remove credentials:
//!
//! | Input | Output |
//! |-------|--------|
//! | `https://user:pass@github.com/org/repo` | `github.com/org/repo` |
//! | `git@github.com:org/repo.git` | `github.com/org/repo` |
//! | `https://github.com/org/repo.git` | `github.com/org/repo` |

mod detector;

pub use detector::GitContext;
