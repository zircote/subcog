//! Memory capture service.

use crate::models::{CaptureRequest, CaptureResult};
use crate::Result;

/// Service for capturing new memories.
pub struct CaptureService {
    // TODO: Add storage backends
}

impl CaptureService {
    /// Creates a new capture service.
    #[must_use]
    pub const fn new() -> Self {
        Self {}
    }

    /// Captures a new memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the capture fails.
    pub fn capture(&mut self, _request: CaptureRequest) -> Result<CaptureResult> {
        // TODO: Implement capture logic
        todo!("CaptureService::capture not yet implemented")
    }
}

impl Default for CaptureService {
    fn default() -> Self {
        Self::new()
    }
}
