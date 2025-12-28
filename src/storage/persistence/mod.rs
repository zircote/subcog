//! Persistence backend implementations.

mod filesystem;
mod git_notes;

pub use filesystem::FilesystemBackend;
pub use git_notes::GitNotesBackend;

// PostgreSQL backend available with feature flag
#[cfg(feature = "postgres")]
mod postgresql;
#[cfg(feature = "postgres")]
pub use postgresql::PostgresBackend;
