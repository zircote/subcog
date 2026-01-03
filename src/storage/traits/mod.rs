//! Storage backend traits.

mod index;
mod persistence;
mod vector;

pub use index::IndexBackend;
pub use persistence::PersistenceBackend;
pub use vector::{VectorBackend, VectorFilter};
