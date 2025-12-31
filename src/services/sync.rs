//! Memory synchronization service.
//!
//! Handles syncing memories with git remotes.

use crate::Result;
use crate::config::Config;
use crate::git::RemoteManager;
use crate::models::MemoryEvent;
use crate::security::record_event;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::instrument;

/// Service for synchronizing memories with remote storage.
pub struct SyncService {
    /// Configuration.
    config: Config,
}

impl SyncService {
    /// Creates a new sync service.
    #[must_use]
    pub const fn new(config: Config) -> Self {
        Self { config }
    }

    /// Fetches memories from remote.
    ///
    /// # Errors
    ///
    /// Returns an error if the fetch fails.
    #[instrument(skip(self), fields(operation = "sync.fetch"))]
    pub fn fetch(&self) -> Result<SyncStats> {
        let start = Instant::now();
        let result =
            (|| {
                let repo_path = self.config.repo_path.as_ref().ok_or_else(|| {
                    crate::Error::OperationFailed {
                        operation: "fetch".to_string(),
                        cause: "No repository path configured".to_string(),
                    }
                })?;

                let remote = RemoteManager::new(repo_path);

                // Get default remote
                let remote_name =
                    remote
                        .default_remote()?
                        .ok_or_else(|| crate::Error::OperationFailed {
                            operation: "fetch".to_string(),
                            cause: "No remote configured".to_string(),
                        })?;

                // Fetch from remote
                let pulled = remote.fetch(&remote_name)?;

                let stats = SyncStats {
                    pushed: 0,
                    pulled,
                    conflicts: 0,
                };
                record_event(MemoryEvent::Synced {
                    pushed: stats.pushed,
                    pulled: stats.pulled,
                    conflicts: stats.conflicts,
                    timestamp: current_timestamp(),
                });
                Ok(stats)
            })();

        let status = if result.is_ok() { "success" } else { "error" };
        metrics::counter!(
            "memory_sync_total",
            "direction" => "fetch",
            "domain" => "project",
            "status" => status
        )
        .increment(1);
        metrics::histogram!("memory_sync_duration_ms", "direction" => "fetch")
            .record(start.elapsed().as_secs_f64() * 1000.0);

        result
    }

    /// Pushes memories to remote.
    ///
    /// # Errors
    ///
    /// Returns an error if the push fails.
    #[instrument(skip(self), fields(operation = "sync.push"))]
    pub fn push(&self) -> Result<SyncStats> {
        let start = Instant::now();
        let result =
            (|| {
                let repo_path = self.config.repo_path.as_ref().ok_or_else(|| {
                    crate::Error::OperationFailed {
                        operation: "push".to_string(),
                        cause: "No repository path configured".to_string(),
                    }
                })?;

                let remote = RemoteManager::new(repo_path);

                // Get default remote
                let remote_name =
                    remote
                        .default_remote()?
                        .ok_or_else(|| crate::Error::OperationFailed {
                            operation: "push".to_string(),
                            cause: "No remote configured".to_string(),
                        })?;

                // Push to remote
                let pushed = remote.push(&remote_name)?;

                let stats = SyncStats {
                    pushed,
                    pulled: 0,
                    conflicts: 0,
                };
                record_event(MemoryEvent::Synced {
                    pushed: stats.pushed,
                    pulled: stats.pulled,
                    conflicts: stats.conflicts,
                    timestamp: current_timestamp(),
                });
                Ok(stats)
            })();

        let status = if result.is_ok() { "success" } else { "error" };
        metrics::counter!(
            "memory_sync_total",
            "direction" => "push",
            "domain" => "project",
            "status" => status
        )
        .increment(1);
        metrics::histogram!("memory_sync_duration_ms", "direction" => "push")
            .record(start.elapsed().as_secs_f64() * 1000.0);

        result
    }

    /// Performs a full sync (fetch + push).
    ///
    /// # Errors
    ///
    /// Returns an error if the sync fails.
    #[instrument(skip(self), fields(operation = "sync.full"))]
    pub fn sync(&self) -> Result<SyncStats> {
        let start = Instant::now();
        let result = (|| {
            // Fetch first
            let fetch_stats = self.fetch()?;

            // Then push
            let push_stats = self.push()?;

            Ok(SyncStats {
                pushed: push_stats.pushed,
                pulled: fetch_stats.pulled,
                conflicts: 0, // TODO: Implement conflict detection
            })
        })();

        let status = if result.is_ok() { "success" } else { "error" };
        metrics::counter!(
            "memory_sync_total",
            "direction" => "full",
            "domain" => "project",
            "status" => status
        )
        .increment(1);
        metrics::histogram!("memory_sync_duration_ms", "direction" => "full")
            .record(start.elapsed().as_secs_f64() * 1000.0);

        result
    }

    /// Checks if sync is available (remote exists and is reachable).
    ///
    /// # Errors
    ///
    /// Returns an error if the check fails.
    pub fn is_available(&self) -> Result<bool> {
        let repo_path = match &self.config.repo_path {
            Some(p) => p,
            None => return Ok(false),
        };

        let remote = RemoteManager::new(repo_path);
        Ok(remote.default_remote()?.is_some())
    }

    /// Returns the configured remote name.
    ///
    /// # Errors
    ///
    /// Returns an error if no repo is configured.
    pub fn remote_name(&self) -> Result<Option<String>> {
        let repo_path = match &self.config.repo_path {
            Some(p) => p,
            None => return Ok(None),
        };

        let remote = RemoteManager::new(repo_path);
        remote.default_remote()
    }

    /// Returns the remote URL.
    ///
    /// # Errors
    ///
    /// Returns an error if no repo is configured.
    pub fn remote_url(&self) -> Result<Option<String>> {
        let repo_path = match &self.config.repo_path {
            Some(p) => p,
            None => return Ok(None),
        };

        let remote = RemoteManager::new(repo_path);
        let remote_name = match remote.default_remote()? {
            Some(name) => name,
            None => return Ok(None),
        };

        remote.get_remote_url(&remote_name)
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Default for SyncService {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

/// Statistics from a sync operation.
#[derive(Debug, Clone, Default)]
pub struct SyncStats {
    /// Number of memories pushed.
    pub pushed: usize,
    /// Number of memories pulled.
    pub pulled: usize,
    /// Number of conflicts encountered.
    pub conflicts: usize,
}

impl SyncStats {
    /// Returns true if the sync was a no-op.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.pushed == 0 && self.pulled == 0 && self.conflicts == 0
    }

    /// Returns a human-readable summary.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.is_empty() {
            "Already up to date".to_string()
        } else {
            format!(
                "Pushed: {}, Pulled: {}, Conflicts: {}",
                self.pushed, self.pulled, self.conflicts
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_stats_empty() {
        let stats = SyncStats::default();
        assert!(stats.is_empty());
        assert_eq!(stats.summary(), "Already up to date");
    }

    #[test]
    fn test_sync_stats_summary() {
        let stats = SyncStats {
            pushed: 5,
            pulled: 3,
            conflicts: 1,
        };
        assert!(!stats.is_empty());
        assert!(stats.summary().contains("Pushed: 5"));
        assert!(stats.summary().contains("Pulled: 3"));
        assert!(stats.summary().contains("Conflicts: 1"));
    }

    #[test]
    fn test_sync_service_no_repo() {
        let service = SyncService::default();

        let result = service.fetch();
        assert!(result.is_err());

        let result = service.push();
        assert!(result.is_err());

        let result = service.sync();
        assert!(result.is_err());
    }

    #[test]
    fn test_sync_service_availability() {
        let service = SyncService::default();
        assert!(!service.is_available().unwrap());
    }
}
