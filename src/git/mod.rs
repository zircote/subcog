//! Git operations.
//!
//! Low-level git operations for notes management, remote sync, and YAML parsing.

mod notes;
mod parser;
mod remote;

pub use notes::NotesManager;
pub use parser::YamlFrontMatterParser;
pub use remote::RemoteManager;
