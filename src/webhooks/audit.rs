//! GDPR-compliant webhook delivery audit logging.
//!
//! This module provides audit logging for webhook deliveries with full
//! GDPR compliance, including:
//! - Recording all delivery attempts with metadata
//! - Export of logs by domain (GDPR Article 20 - Data Portability)
//! - Deletion of logs by domain (GDPR Article 17 - Right to Erasure)
//!
//! # Schema
//!
//! ```sql
//! CREATE TABLE webhook_deliveries (
//!     id TEXT PRIMARY KEY,
//!     webhook_name TEXT NOT NULL,
//!     event_type TEXT NOT NULL,
//!     event_id TEXT NOT NULL,
//!     domain TEXT NOT NULL,
//!     url TEXT NOT NULL,
//!     status TEXT NOT NULL,
//!     status_code INTEGER,
//!     attempts INTEGER NOT NULL,
//!     duration_ms INTEGER NOT NULL,
//!     error TEXT,
//!     timestamp INTEGER NOT NULL
//! );
//! ```

use crate::{Error, Result};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

/// Delivery status for audit logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryStatus {
    /// Delivery succeeded.
    Success,
    /// Delivery failed after all retries.
    Failed,
    /// Delivery timed out.
    Timeout,
}

impl DeliveryStatus {
    /// Returns the string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Timeout => "timeout",
        }
    }
}

impl std::fmt::Display for DeliveryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for DeliveryStatus {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "success" => Ok(Self::Success),
            "failed" => Ok(Self::Failed),
            "timeout" => Ok(Self::Timeout),
            _ => Err(Error::InvalidInput(format!("Invalid delivery status: {s}"))),
        }
    }
}

/// A webhook delivery audit record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryRecord {
    /// Unique record ID.
    pub id: String,
    /// Webhook name.
    pub webhook_name: String,
    /// Event type that triggered the delivery.
    pub event_type: String,
    /// Original event ID.
    pub event_id: String,
    /// Domain scope (project/user/org).
    pub domain: String,
    /// Target URL.
    pub url: String,
    /// Delivery status.
    pub status: DeliveryStatus,
    /// HTTP status code (if available).
    pub status_code: Option<i32>,
    /// Number of delivery attempts.
    pub attempts: i32,
    /// Total duration in milliseconds.
    pub duration_ms: i64,
    /// Error message (if failed).
    pub error: Option<String>,
    /// Unix timestamp of the delivery.
    pub timestamp: i64,
}

impl DeliveryRecord {
    /// Creates a new delivery record.
    #[must_use]
    pub fn new(
        webhook_name: &str,
        event_type: &str,
        event_id: &str,
        domain: &str,
        url: &str,
        result: &super::delivery::DeliveryResult,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            webhook_name: webhook_name.to_string(),
            event_type: event_type.to_string(),
            event_id: event_id.to_string(),
            domain: domain.to_string(),
            url: url.to_string(),
            status: if result.success {
                DeliveryStatus::Success
            } else {
                DeliveryStatus::Failed
            },
            status_code: result.status_code.map(i32::from),
            attempts: i32::try_from(result.attempts).unwrap_or(0),
            duration_ms: i64::try_from(result.duration_ms).unwrap_or(0),
            error: result.error.clone(),
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

/// Trait for webhook audit storage backends.
pub trait WebhookAuditBackend: Send + Sync {
    /// Stores a delivery record.
    ///
    /// # Errors
    ///
    /// Returns an error if the record cannot be stored.
    fn store(&self, record: &DeliveryRecord) -> Result<()>;

    /// Gets delivery records for a webhook.
    ///
    /// # Arguments
    ///
    /// * `webhook_name` - Name of the webhook
    /// * `limit` - Maximum number of records to return
    ///
    /// # Errors
    ///
    /// Returns an error if the records cannot be retrieved.
    fn get_history(&self, webhook_name: &str, limit: usize) -> Result<Vec<DeliveryRecord>>;

    /// Exports all records for a domain (GDPR Article 20).
    ///
    /// # Arguments
    ///
    /// * `domain` - Domain scope to export
    ///
    /// # Errors
    ///
    /// Returns an error if the records cannot be exported.
    fn export_domain_logs(&self, domain: &str) -> Result<Vec<DeliveryRecord>>;

    /// Deletes all records for a domain (GDPR Article 17).
    ///
    /// # Arguments
    ///
    /// * `domain` - Domain scope to delete
    ///
    /// # Returns
    ///
    /// The number of records deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if the records cannot be deleted.
    fn delete_domain_logs(&self, domain: &str) -> Result<usize>;

    /// Counts records by status for a webhook.
    ///
    /// # Errors
    ///
    /// Returns an error if the count cannot be retrieved.
    fn count_by_status(&self, webhook_name: &str) -> Result<WebhookStats>;
}

/// Statistics for a webhook.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebhookStats {
    /// Total number of deliveries.
    pub total: usize,
    /// Number of successful deliveries.
    pub success: usize,
    /// Number of failed deliveries.
    pub failed: usize,
    /// Average duration in milliseconds.
    pub avg_duration_ms: f64,
}

/// `SQLite`-backed webhook audit logger.
pub struct WebhookAuditLogger {
    /// `SQLite` connection.
    conn: Mutex<Connection>,
}

// Mutex guards are held for the duration of database operations, which is correct behavior
#[allow(clippy::significant_drop_tightening)]
impl WebhookAuditLogger {
    /// Creates a new audit logger with the given database path.
    ///
    /// # Arguments
    ///
    /// * `db_path` - Path to the `SQLite` database file
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or initialized.
    pub fn new(db_path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::OperationFailed {
                operation: "create_audit_dir".to_string(),
                cause: e.to_string(),
            })?;
        }

        let conn = Connection::open(db_path).map_err(|e| Error::OperationFailed {
            operation: "open_audit_db".to_string(),
            cause: e.to_string(),
        })?;

        // Enable WAL mode for better concurrency
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
            .map_err(|e| Error::OperationFailed {
                operation: "configure_audit_db".to_string(),
                cause: e.to_string(),
            })?;

        let logger = Self {
            conn: Mutex::new(conn),
        };

        logger.create_schema()?;

        Ok(logger)
    }

    /// Creates an in-memory audit logger for testing.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be initialized.
    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| Error::OperationFailed {
            operation: "open_memory_db".to_string(),
            cause: e.to_string(),
        })?;

        let logger = Self {
            conn: Mutex::new(conn),
        };

        logger.create_schema()?;

        Ok(logger)
    }

    /// Creates the database schema.
    fn create_schema(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_audit_db".to_string(),
            cause: e.to_string(),
        })?;

        conn.execute_batch(
            r"
            CREATE TABLE IF NOT EXISTS webhook_deliveries (
                id TEXT PRIMARY KEY,
                webhook_name TEXT NOT NULL,
                event_type TEXT NOT NULL,
                event_id TEXT NOT NULL,
                domain TEXT NOT NULL,
                url TEXT NOT NULL,
                status TEXT NOT NULL,
                status_code INTEGER,
                attempts INTEGER NOT NULL,
                duration_ms INTEGER NOT NULL,
                error TEXT,
                timestamp INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_webhook_name
                ON webhook_deliveries(webhook_name);
            CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_domain
                ON webhook_deliveries(domain);
            CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_timestamp
                ON webhook_deliveries(timestamp);
            CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_event_id
                ON webhook_deliveries(event_id);
            ",
        )
        .map_err(|e| Error::OperationFailed {
            operation: "create_audit_schema".to_string(),
            cause: e.to_string(),
        })?;

        Ok(())
    }

    /// Records a delivery result.
    ///
    /// # Arguments
    ///
    /// * `webhook_name` - Name of the webhook
    /// * `event_type` - Type of event
    /// * `event_id` - ID of the event
    /// * `domain` - Domain scope
    /// * `url` - Target URL
    /// * `result` - Delivery result
    ///
    /// # Errors
    ///
    /// Returns an error if the record cannot be stored.
    pub fn log_delivery(
        &self,
        webhook_name: &str,
        event_type: &str,
        event_id: &str,
        domain: &str,
        url: &str,
        result: &super::delivery::DeliveryResult,
    ) -> Result<()> {
        let record = DeliveryRecord::new(webhook_name, event_type, event_id, domain, url, result);
        self.store(&record)
    }
}

// Mutex guards are held for the duration of database operations, which is correct behavior
#[allow(clippy::significant_drop_tightening)]
impl WebhookAuditBackend for WebhookAuditLogger {
    fn store(&self, record: &DeliveryRecord) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_audit_db".to_string(),
            cause: e.to_string(),
        })?;

        conn.execute(
            r"
            INSERT INTO webhook_deliveries
                (id, webhook_name, event_type, event_id, domain, url, status,
                 status_code, attempts, duration_ms, error, timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ",
            params![
                record.id,
                record.webhook_name,
                record.event_type,
                record.event_id,
                record.domain,
                record.url,
                record.status.as_str(),
                record.status_code,
                record.attempts,
                record.duration_ms,
                record.error,
                record.timestamp,
            ],
        )
        .map_err(|e| Error::OperationFailed {
            operation: "store_delivery_record".to_string(),
            cause: e.to_string(),
        })?;

        Ok(())
    }

    fn get_history(&self, webhook_name: &str, limit: usize) -> Result<Vec<DeliveryRecord>> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_audit_db".to_string(),
            cause: e.to_string(),
        })?;

        let mut stmt = conn
            .prepare(
                r"
                SELECT id, webhook_name, event_type, event_id, domain, url, status,
                       status_code, attempts, duration_ms, error, timestamp
                FROM webhook_deliveries
                WHERE webhook_name = ?1
                ORDER BY timestamp DESC
                LIMIT ?2
                ",
            )
            .map_err(|e| Error::OperationFailed {
                operation: "prepare_history_query".to_string(),
                cause: e.to_string(),
            })?;

        let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);
        let records = stmt
            .query_map(params![webhook_name, limit_i64], |row| {
                Ok(DeliveryRecord {
                    id: row.get(0)?,
                    webhook_name: row.get(1)?,
                    event_type: row.get(2)?,
                    event_id: row.get(3)?,
                    domain: row.get(4)?,
                    url: row.get(5)?,
                    status: row
                        .get::<_, String>(6)?
                        .parse()
                        .unwrap_or(DeliveryStatus::Failed),
                    status_code: row.get(7)?,
                    attempts: row.get(8)?,
                    duration_ms: row.get(9)?,
                    error: row.get(10)?,
                    timestamp: row.get(11)?,
                })
            })
            .map_err(|e| Error::OperationFailed {
                operation: "query_history".to_string(),
                cause: e.to_string(),
            })?
            .filter_map(std::result::Result::ok)
            .collect();

        Ok(records)
    }

    fn export_domain_logs(&self, domain: &str) -> Result<Vec<DeliveryRecord>> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_audit_db".to_string(),
            cause: e.to_string(),
        })?;

        let mut stmt = conn
            .prepare(
                r"
                SELECT id, webhook_name, event_type, event_id, domain, url, status,
                       status_code, attempts, duration_ms, error, timestamp
                FROM webhook_deliveries
                WHERE domain = ?1
                ORDER BY timestamp DESC
                ",
            )
            .map_err(|e| Error::OperationFailed {
                operation: "prepare_export_query".to_string(),
                cause: e.to_string(),
            })?;

        let records = stmt
            .query_map(params![domain], |row| {
                Ok(DeliveryRecord {
                    id: row.get(0)?,
                    webhook_name: row.get(1)?,
                    event_type: row.get(2)?,
                    event_id: row.get(3)?,
                    domain: row.get(4)?,
                    url: row.get(5)?,
                    status: row
                        .get::<_, String>(6)?
                        .parse()
                        .unwrap_or(DeliveryStatus::Failed),
                    status_code: row.get(7)?,
                    attempts: row.get(8)?,
                    duration_ms: row.get(9)?,
                    error: row.get(10)?,
                    timestamp: row.get(11)?,
                })
            })
            .map_err(|e| Error::OperationFailed {
                operation: "export_logs".to_string(),
                cause: e.to_string(),
            })?
            .filter_map(std::result::Result::ok)
            .collect();

        Ok(records)
    }

    fn delete_domain_logs(&self, domain: &str) -> Result<usize> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_audit_db".to_string(),
            cause: e.to_string(),
        })?;

        let count = conn
            .execute(
                "DELETE FROM webhook_deliveries WHERE domain = ?1",
                params![domain],
            )
            .map_err(|e| Error::OperationFailed {
                operation: "delete_domain_logs".to_string(),
                cause: e.to_string(),
            })?;

        Ok(count)
    }

    fn count_by_status(&self, webhook_name: &str) -> Result<WebhookStats> {
        let conn = self.conn.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_audit_db".to_string(),
            cause: e.to_string(),
        })?;

        let mut stmt = conn
            .prepare(
                r"
                SELECT
                    COUNT(*) as total,
                    SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END) as success,
                    SUM(CASE WHEN status != 'success' THEN 1 ELSE 0 END) as failed,
                    AVG(duration_ms) as avg_duration
                FROM webhook_deliveries
                WHERE webhook_name = ?1
                ",
            )
            .map_err(|e| Error::OperationFailed {
                operation: "prepare_stats_query".to_string(),
                cause: e.to_string(),
            })?;

        let stats = stmt
            .query_row(params![webhook_name], |row| {
                Ok(WebhookStats {
                    total: usize::try_from(row.get::<_, i64>(0).unwrap_or(0)).unwrap_or(0),
                    success: usize::try_from(row.get::<_, i64>(1).unwrap_or(0)).unwrap_or(0),
                    failed: usize::try_from(row.get::<_, i64>(2).unwrap_or(0)).unwrap_or(0),
                    avg_duration_ms: row.get::<_, f64>(3).unwrap_or(0.0),
                })
            })
            .map_err(|e| Error::OperationFailed {
                operation: "query_stats".to_string(),
                cause: e.to_string(),
            })?;

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webhooks::delivery::DeliveryResult;

    #[test]
    fn test_audit_logger_creation() {
        let logger = WebhookAuditLogger::in_memory().expect("create logger");
        let stats = logger.count_by_status("test").expect("get stats");
        assert_eq!(stats.total, 0);
    }

    #[test]
    fn test_store_and_retrieve() {
        let logger = WebhookAuditLogger::in_memory().expect("create logger");

        let result = DeliveryResult::success(200, 1, 100);
        logger
            .log_delivery(
                "test-webhook",
                "captured",
                "event-123",
                "project",
                "https://example.com",
                &result,
            )
            .expect("log delivery");

        let history = logger.get_history("test-webhook", 10).expect("get history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].webhook_name, "test-webhook");
        assert_eq!(history[0].status, DeliveryStatus::Success);
    }

    #[test]
    fn test_export_domain_logs() {
        let logger = WebhookAuditLogger::in_memory().expect("create logger");

        // Add records for different domains
        let result = DeliveryResult::success(200, 1, 100);
        logger
            .log_delivery(
                "webhook-1",
                "captured",
                "e1",
                "project",
                "https://a.com",
                &result,
            )
            .expect("log 1");
        logger
            .log_delivery(
                "webhook-2",
                "deleted",
                "e2",
                "user",
                "https://b.com",
                &result,
            )
            .expect("log 2");
        logger
            .log_delivery(
                "webhook-3",
                "updated",
                "e3",
                "project",
                "https://c.com",
                &result,
            )
            .expect("log 3");

        let project_logs = logger.export_domain_logs("project").expect("export");
        assert_eq!(project_logs.len(), 2);

        let user_logs = logger.export_domain_logs("user").expect("export");
        assert_eq!(user_logs.len(), 1);
    }

    #[test]
    fn test_delete_domain_logs() {
        let logger = WebhookAuditLogger::in_memory().expect("create logger");

        let result = DeliveryResult::success(200, 1, 100);
        logger
            .log_delivery(
                "webhook-1",
                "captured",
                "e1",
                "project",
                "https://a.com",
                &result,
            )
            .expect("log 1");
        logger
            .log_delivery(
                "webhook-2",
                "captured",
                "e2",
                "project",
                "https://b.com",
                &result,
            )
            .expect("log 2");
        logger
            .log_delivery(
                "webhook-3",
                "captured",
                "e3",
                "user",
                "https://c.com",
                &result,
            )
            .expect("log 3");

        let deleted = logger.delete_domain_logs("project").expect("delete");
        assert_eq!(deleted, 2);

        let project_logs = logger.export_domain_logs("project").expect("export");
        assert_eq!(project_logs.len(), 0);

        // User logs should still exist
        let user_logs = logger.export_domain_logs("user").expect("export");
        assert_eq!(user_logs.len(), 1);
    }

    #[test]
    fn test_count_by_status() {
        let logger = WebhookAuditLogger::in_memory().expect("create logger");

        let success = DeliveryResult::success(200, 1, 100);
        let failure = DeliveryResult::failure("error".to_string(), 3, 5000);

        logger
            .log_delivery(
                "webhook",
                "captured",
                "e1",
                "project",
                "https://a.com",
                &success,
            )
            .expect("log 1");
        logger
            .log_delivery(
                "webhook",
                "captured",
                "e2",
                "project",
                "https://a.com",
                &success,
            )
            .expect("log 2");
        logger
            .log_delivery(
                "webhook",
                "captured",
                "e3",
                "project",
                "https://a.com",
                &failure,
            )
            .expect("log 3");

        let stats = logger.count_by_status("webhook").expect("stats");
        assert_eq!(stats.total, 3);
        assert_eq!(stats.success, 2);
        assert_eq!(stats.failed, 1);
    }
}
