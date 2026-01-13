//! Storage backend traits.

pub mod graph;
mod index;
mod persistence;
mod vector;

pub use graph::{GraphBackend, GraphStats};
pub use index::IndexBackend;
pub use persistence::PersistenceBackend;
pub use vector::{VectorBackend, VectorFilter};
