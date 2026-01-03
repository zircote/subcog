//! Persistence backend implementations.

mod filesystem;
mod git_notes;
mod postgresql;

pub use filesystem::FilesystemBackend;
pub use git_notes::GitNotesBackend;
pub use postgresql::PostgresBackend;
