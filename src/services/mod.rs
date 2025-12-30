//! Business logic services.
//!
//! Services orchestrate storage backends and provide high-level operations.

// Allow cast_precision_loss for score calculations where exact precision is not critical.
#![allow(clippy::cast_precision_loss)]
// Allow option_if_let_else for clearer code in some contexts.
#![allow(clippy::option_if_let_else)]
// Allow significant_drop_tightening as dropping slightly early provides no benefit.
#![allow(clippy::significant_drop_tightening)]
// Allow unused_self for methods kept for API consistency.
#![allow(clippy::unused_self)]
// Allow trivially_copy_pass_by_ref for namespace references.
#![allow(clippy::trivially_copy_pass_by_ref)]
// Allow unnecessary_wraps for const fn methods returning Result.
#![allow(clippy::unnecessary_wraps)]
// Allow manual_let_else for clearer error handling patterns.
#![allow(clippy::manual_let_else)]
// Allow or_fun_call for entry API with closures.
#![allow(clippy::or_fun_call)]

mod capture;
mod consolidation;
mod context;
mod recall;
mod sync;

pub use capture::CaptureService;
pub use consolidation::ConsolidationService;
pub use context::ContextBuilderService;
pub use recall::RecallService;
pub use sync::SyncService;

use crate::Result;
use crate::storage::index::SqliteBackend;
use directories::BaseDirs;
use once_cell::sync::OnceCell;
use std::path::PathBuf;

/// Global service container instance.
static SERVICES: OnceCell<ServiceContainer> = OnceCell::new();

/// Container for initialized services with configured backends.
pub struct ServiceContainer {
    /// Recall service with index backend.
    recall: RecallService,
    /// Capture service.
    capture: CaptureService,
    /// Sync service.
    sync: SyncService,
}

impl ServiceContainer {
    /// Initializes the service container with backends.
    ///
    /// # Errors
    ///
    /// Returns an error if backends cannot be initialized.
    pub fn init(data_dir: Option<PathBuf>) -> Result<&'static Self> {
        SERVICES.get_or_try_init(|| {
            let data_dir = data_dir.unwrap_or_else(|| {
                BaseDirs::new()
                    .map_or_else(|| PathBuf::from("."), |b| b.data_local_dir().to_path_buf())
                    .join("subcog")
            });

            // Ensure data directory exists
            std::fs::create_dir_all(&data_dir).map_err(|e| crate::Error::OperationFailed {
                operation: "create_data_dir".to_string(),
                cause: e.to_string(),
            })?;

            // Initialize SQLite index
            let db_path = data_dir.join("index.db");
            let index = SqliteBackend::new(&db_path)?;

            Ok(Self {
                recall: RecallService::with_index(index),
                capture: CaptureService::default(),
                sync: SyncService::default(),
            })
        })
    }

    /// Gets the global service container, initializing if needed.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
    pub fn get() -> Result<&'static Self> {
        Self::init(None)
    }

    /// Returns the recall service.
    #[must_use]
    pub const fn recall(&self) -> &RecallService {
        &self.recall
    }

    /// Returns the capture service.
    #[must_use]
    pub const fn capture(&self) -> &CaptureService {
        &self.capture
    }

    /// Returns the sync service.
    #[must_use]
    pub const fn sync(&self) -> &SyncService {
        &self.sync
    }
}
