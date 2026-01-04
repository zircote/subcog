//! Git operations.
//!
//! Git context detection for repository, branch, and path information.

mod parser;
mod remote;

pub use parser::YamlFrontMatterParser;
pub use remote::RemoteManager;
