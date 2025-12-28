//! Memory recall (search) service.

use crate::models::{SearchFilter, SearchMode, SearchResult};
use crate::Result;

/// Service for searching and retrieving memories.
pub struct RecallService {
    // TODO: Add storage backends
}

impl RecallService {
    /// Creates a new recall service.
    #[must_use]
    pub const fn new() -> Self {
        Self {}
    }

    /// Searches for memories matching a query.
    ///
    /// # Errors
    ///
    /// Returns an error if the search fails.
    pub fn search(
        &self,
        _query: &str,
        _mode: SearchMode,
        _filter: &SearchFilter,
        _limit: usize,
    ) -> Result<SearchResult> {
        // TODO: Implement search logic
        todo!("RecallService::search not yet implemented")
    }
}

impl Default for RecallService {
    fn default() -> Self {
        Self::new()
    }
}
