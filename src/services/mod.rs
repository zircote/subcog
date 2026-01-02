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
pub mod deduplication;
mod enrichment;
mod prompt;
mod prompt_enrichment;
mod prompt_parser;
mod query_parser;
mod recall;
mod sync;
mod topic_index;

pub use capture::CaptureService;
pub use consolidation::ConsolidationService;
pub use context::{ContextBuilderService, MemoryStatistics};
pub use deduplication::{
    DeduplicationConfig, DeduplicationService, Deduplicator, DuplicateCheckResult, DuplicateReason,
};
pub use enrichment::{EnrichmentResult, EnrichmentService, EnrichmentStats};
pub use prompt::{PromptFilter, PromptService, SaveOptions, SaveResult};
pub use prompt_enrichment::{
    ENRICHMENT_TIMEOUT, EnrichmentRequest, EnrichmentStatus, PROMPT_ENRICHMENT_SYSTEM_PROMPT,
    PartialMetadata, PromptEnrichmentResult, PromptEnrichmentService,
};
pub use prompt_parser::{PromptFormat, PromptParser};
pub use query_parser::parse_filter_query;
pub use recall::RecallService;
pub use sync::SyncService;
pub use topic_index::{TopicIndexService, TopicInfo};

use crate::git::{NotesManager, YamlFrontMatterParser};
use crate::models::{Domain, Memory, MemoryId, MemoryStatus, Namespace};
use crate::storage::index::{
    DomainIndexConfig, DomainIndexManager, DomainScope, OrgIndexConfig, SqliteBackend,
    find_repo_root,
};
use crate::storage::traits::IndexBackend;
use crate::{Error, Result};
use std::path::PathBuf;
use std::sync::Mutex;

/// Container for initialized services with configured backends.
///
/// Unlike the previous singleton design, this can be instantiated per-context
/// with domain-scoped indices.
pub struct ServiceContainer {
    /// Capture service.
    capture: CaptureService,
    /// Sync service.
    sync: SyncService,
    /// Domain index manager for multi-domain indices.
    index_manager: Mutex<DomainIndexManager>,
    /// Repository path (if known).
    repo_path: Option<PathBuf>,
}

impl ServiceContainer {
    /// Creates a new service container for a repository.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to or within a git repository
    /// * `org_config` - Optional organization index configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the repository cannot be found or backends fail to initialize.
    pub fn for_repo(
        repo_path: impl Into<PathBuf>,
        org_config: Option<OrgIndexConfig>,
    ) -> Result<Self> {
        let repo_path = repo_path.into();

        // Find repository root
        let repo_root = find_repo_root(&repo_path)?;

        let config = DomainIndexConfig {
            repo_path: Some(repo_root.clone()),
            org_config,
        };

        let index_manager = DomainIndexManager::new(config)?;

        // Create CaptureService with repo_path so it stores to git notes
        let capture_config = crate::config::Config::new().with_repo_path(&repo_root);

        Ok(Self {
            capture: CaptureService::new(capture_config),
            sync: SyncService::default(),
            index_manager: Mutex::new(index_manager),
            repo_path: Some(repo_root),
        })
    }

    /// Creates a service container from the current directory.
    ///
    /// # Errors
    ///
    /// Returns an error if not in a git repository.
    pub fn from_current_dir() -> Result<Self> {
        let cwd = std::env::current_dir().map_err(|e| Error::OperationFailed {
            operation: "get_current_dir".to_string(),
            cause: e.to_string(),
        })?;

        Self::for_repo(cwd, None)
    }

    /// Creates a recall service for a specific domain scope.
    ///
    /// # Errors
    ///
    /// Returns an error if the index cannot be initialized.
    pub fn recall_for_scope(&self, scope: DomainScope) -> Result<RecallService> {
        let manager = self
            .index_manager
            .lock()
            .map_err(|e| Error::OperationFailed {
                operation: "lock_index_manager".to_string(),
                cause: e.to_string(),
            })?;

        let index_path = manager.get_index_path(scope)?;

        // Ensure parent directory exists
        if let Some(parent) = index_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::OperationFailed {
                operation: "create_index_dir".to_string(),
                cause: e.to_string(),
            })?;
        }

        let index = SqliteBackend::new(&index_path)?;
        Ok(RecallService::with_index(index))
    }

    /// Creates a recall service for the project scope.
    ///
    /// # Errors
    ///
    /// Returns an error if the index cannot be initialized.
    pub fn recall(&self) -> Result<RecallService> {
        self.recall_for_scope(DomainScope::Project)
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

    /// Returns the repository path.
    #[must_use]
    pub const fn repo_path(&self) -> Option<&PathBuf> {
        self.repo_path.as_ref()
    }

    /// Creates a deduplication service without embedding support.
    ///
    /// This variant supports:
    /// - Exact match (SHA256 hash comparison)
    /// - Recent capture (LRU cache with TTL)
    ///
    /// For full semantic similarity support, create a `DeduplicationService`
    /// directly with an embedder and vector backend.
    ///
    /// # Errors
    ///
    /// Returns an error if the recall service cannot be initialized.
    pub fn deduplication(
        &self,
    ) -> Result<
        deduplication::DeduplicationService<
            crate::embedding::FastEmbedEmbedder,
            crate::storage::vector::UsearchBackend,
        >,
    > {
        let recall = std::sync::Arc::new(self.recall()?);
        let config = deduplication::DeduplicationConfig::from_env();
        Ok(deduplication::DeduplicationService::without_embeddings(
            recall, config,
        ))
    }

    /// Creates a deduplication service with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Custom deduplication configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the recall service cannot be initialized.
    pub fn deduplication_with_config(
        &self,
        config: deduplication::DeduplicationConfig,
    ) -> Result<
        deduplication::DeduplicationService<
            crate::embedding::FastEmbedEmbedder,
            crate::storage::vector::UsearchBackend,
        >,
    > {
        let recall = std::sync::Arc::new(self.recall()?);
        Ok(deduplication::DeduplicationService::without_embeddings(
            recall, config,
        ))
    }

    /// Gets the index path for a domain scope.
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be determined.
    pub fn get_index_path(&self, scope: DomainScope) -> Result<PathBuf> {
        let manager = self
            .index_manager
            .lock()
            .map_err(|e| Error::OperationFailed {
                operation: "lock_index_manager".to_string(),
                cause: e.to_string(),
            })?;
        manager.get_index_path(scope)
    }

    /// Reindexes memories from git notes into the index for a specific scope.
    ///
    /// # Arguments
    ///
    /// * `scope` - The domain scope to reindex
    ///
    /// # Returns
    ///
    /// The number of memories indexed.
    ///
    /// # Errors
    ///
    /// Returns an error if notes cannot be read or indexing fails.
    pub fn reindex_scope(&self, scope: DomainScope) -> Result<usize> {
        let repo_path = self
            .repo_path
            .as_ref()
            .ok_or_else(|| Error::InvalidInput("Repository path not configured".to_string()))?;

        let notes = NotesManager::new(repo_path);

        // Get all notes
        let all_notes = notes.list()?;

        if all_notes.is_empty() {
            return Ok(0);
        }

        // Parse notes into memories
        let mut memories = Vec::with_capacity(all_notes.len());
        for (note_id, content) in &all_notes {
            match parse_note_to_memory(note_id, content) {
                Ok(memory) => memories.push(memory),
                Err(e) => {
                    tracing::warn!("Failed to parse note {note_id}: {e}");
                },
            }
        }

        if memories.is_empty() {
            return Ok(0);
        }

        // Get index path and create backend
        let index_path = self.get_index_path(scope)?;

        // Ensure parent directory exists
        if let Some(parent) = index_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::OperationFailed {
                operation: "create_index_dir".to_string(),
                cause: e.to_string(),
            })?;
        }

        let mut index = SqliteBackend::new(&index_path)?;

        // Clear and reindex
        index.clear()?;
        let count = memories.len();
        index.reindex(&memories)?;

        Ok(count)
    }

    /// Reindexes memories for the project scope (default).
    ///
    /// # Errors
    ///
    /// Returns an error if notes cannot be read or indexing fails.
    pub fn reindex(&self) -> Result<usize> {
        self.reindex_scope(DomainScope::Project)
    }

    /// Reindexes all domain scopes.
    ///
    /// # Returns
    ///
    /// A map of scope to count of indexed memories.
    ///
    /// # Errors
    ///
    /// Returns an error if any scope fails to reindex.
    pub fn reindex_all(&self) -> Result<std::collections::HashMap<DomainScope, usize>> {
        let mut results = std::collections::HashMap::new();

        for scope in [DomainScope::Project, DomainScope::User, DomainScope::Org] {
            match self.reindex_scope(scope) {
                Ok(count) => {
                    results.insert(scope, count);
                },
                Err(e) => {
                    tracing::warn!("Failed to reindex scope {:?}: {e}", scope);
                },
            }
        }

        Ok(results)
    }
}

// Legacy compatibility: Keep a global instance for backward compatibility
use once_cell::sync::OnceCell;
static LEGACY_SERVICES: OnceCell<LegacyServiceContainer> = OnceCell::new();

/// Legacy service container for backward compatibility.
///
/// Deprecated: Use `ServiceContainer::for_repo()` instead.
pub struct LegacyServiceContainer {
    recall: RecallService,
    capture: CaptureService,
    sync: SyncService,
    index: Mutex<SqliteBackend>,
    data_dir: PathBuf,
}

impl LegacyServiceContainer {
    /// Initializes the legacy service container.
    ///
    /// # Errors
    ///
    /// Returns an error if backends cannot be initialized.
    #[deprecated(note = "Use ServiceContainer::for_repo() instead")]
    pub fn init(data_dir: Option<PathBuf>) -> Result<&'static Self> {
        LEGACY_SERVICES.get_or_try_init(|| {
            let data_dir = data_dir.unwrap_or_else(|| {
                directories::BaseDirs::new()
                    .map_or_else(|| PathBuf::from("."), |b| b.data_local_dir().to_path_buf())
                    .join("subcog")
            });

            std::fs::create_dir_all(&data_dir).map_err(|e| Error::OperationFailed {
                operation: "create_data_dir".to_string(),
                cause: e.to_string(),
            })?;

            let db_path = data_dir.join("index.db");
            let index = SqliteBackend::new(&db_path)?;
            let recall_index = SqliteBackend::new(&db_path)?;

            Ok(Self {
                recall: RecallService::with_index(recall_index),
                capture: CaptureService::default(),
                sync: SyncService::default(),
                index: Mutex::new(index),
                data_dir,
            })
        })
    }

    /// Gets the legacy service container.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
    #[deprecated(note = "Use ServiceContainer::for_repo() instead")]
    #[allow(deprecated)]
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

    /// Returns the data directory path.
    #[must_use]
    pub const fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    /// Reindexes memories from git notes.
    ///
    /// # Errors
    ///
    /// Returns an error if notes cannot be read or indexing fails.
    pub fn reindex(&self, repo_path: &std::path::Path) -> Result<usize> {
        let notes = NotesManager::new(repo_path);
        let all_notes = notes.list()?;

        if all_notes.is_empty() {
            return Ok(0);
        }

        let mut memories = Vec::with_capacity(all_notes.len());
        for (note_id, content) in &all_notes {
            match parse_note_to_memory(note_id, content) {
                Ok(memory) => memories.push(memory),
                Err(e) => {
                    tracing::warn!("Failed to parse note {note_id}: {e}");
                },
            }
        }

        if memories.is_empty() {
            return Ok(0);
        }

        let mut index = self.index.lock().map_err(|e| Error::OperationFailed {
            operation: "lock_index".to_string(),
            cause: e.to_string(),
        })?;

        index.clear()?;
        let count = memories.len();
        index.reindex(&memories)?;

        Ok(count)
    }
}

/// Parses a git note into a Memory object.
///
/// # Arguments
///
/// * `note_id` - The git commit OID the note is attached to (used as fallback ID)
/// * `content` - The note content with optional YAML front matter
///
/// # Errors
///
/// Returns an error if the note cannot be parsed.
fn parse_note_to_memory(note_id: &str, content: &str) -> Result<Memory> {
    let (metadata, body) = YamlFrontMatterParser::parse(content)?;

    // Extract fields from metadata, using defaults where necessary
    let id = metadata
        .get("id")
        .and_then(|v| v.as_str())
        .map_or_else(|| MemoryId::new(note_id), MemoryId::new);

    let namespace = metadata
        .get("namespace")
        .and_then(|v| v.as_str())
        .and_then(Namespace::parse)
        .unwrap_or_default();

    let domain = metadata
        .get("domain")
        .and_then(|v| v.as_str())
        .map_or_else(Domain::new, parse_domain_string);

    let status = metadata
        .get("status")
        .and_then(|v| v.as_str())
        .map_or(MemoryStatus::Active, parse_status_string);

    let created_at = metadata
        .get("created_at")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);

    let updated_at = metadata
        .get("updated_at")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(created_at);

    let tags = metadata
        .get("tags")
        .and_then(|v| v.as_array())
        .map_or_else(Vec::new, |arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

    let source = metadata
        .get("source")
        .and_then(|v| v.as_str())
        .map(String::from);

    Ok(Memory {
        id,
        content: body,
        namespace,
        domain,
        status,
        created_at,
        updated_at,
        embedding: None,
        tags,
        source,
    })
}

/// Parses a domain string (e.g., "org/repo") into a Domain.
fn parse_domain_string(s: &str) -> Domain {
    if s == "global" || s.is_empty() {
        return Domain::new();
    }

    let parts: Vec<&str> = s.split('/').collect();
    match parts.len() {
        1 => Domain {
            organization: Some(parts[0].to_string()),
            project: None,
            repository: None,
        },
        2 => Domain {
            organization: Some(parts[0].to_string()),
            project: None,
            repository: Some(parts[1].to_string()),
        },
        3 => Domain {
            organization: Some(parts[0].to_string()),
            project: Some(parts[1].to_string()),
            repository: Some(parts[2].to_string()),
        },
        _ => Domain::new(),
    }
}

/// Parses a status string into a `MemoryStatus`.
fn parse_status_string(s: &str) -> MemoryStatus {
    match s.to_lowercase().as_str() {
        "archived" => MemoryStatus::Archived,
        "superseded" => MemoryStatus::Superseded,
        "pending" => MemoryStatus::Pending,
        "deleted" => MemoryStatus::Deleted,
        // Default to Active for "active" and any unknown status
        _ => MemoryStatus::Active,
    }
}
