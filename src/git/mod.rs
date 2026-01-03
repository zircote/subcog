//! Git operations.
//!
//! Low-level git operations for remote sync and YAML parsing.

mod parser;
mod remote;

pub use parser::YamlFrontMatterParser;
pub use remote::RemoteManager;
