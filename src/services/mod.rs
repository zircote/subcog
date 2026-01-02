//! Business logic services.
//!
//! Services orchestrate storage backends and provide high-level operations.
//!
//! # Clippy Lints
//!
//! The following lints are allowed at module level due to their pervasive nature
//! in service code. Each has a documented rationale:
//!
//! | Lint | Rationale |
//! |------|-----------|
//! | `cast_precision_loss` | Metrics/score calculations don't require exact precision |
//! | `unused_self` | Methods retained for API consistency or future extension |
//! | `option_if_let_else` | If-let chains often clearer than nested `map_or_else` |
//! | `manual_let_else` | Match patterns with logging clearer than `let...else` |
//! | `unnecessary_wraps` | Result types for API consistency across trait impls |
//! | `or_fun_call` | Entry API with closures for lazy initialization |
//! | `significant_drop_tightening` | Drop timing not critical for correctness |

// Metrics and scoring calculations don't require exact precision
#![allow(clippy::cast_precision_loss)]
// Methods kept for API consistency or future self usage
#![allow(clippy::unused_self)]
// If-let chains often clearer than nested map_or_else
#![allow(clippy::option_if_let_else)]
// Match patterns with logging are clearer than let-else
#![allow(clippy::manual_let_else)]
// Result types maintained for API consistency across trait implementations
#![allow(clippy::unnecessary_wraps)]
// Entry API with closures for lazy initialization
#![allow(clippy::or_fun_call)]
// Drop timing not critical for correctness in service code
#![allow(clippy::significant_drop_tightening)]

mod capture;
mod consolidation;
mod context;
pub mod deduplication;
mod enrichment;
pub mod migration;
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

use crate::config::SubcogConfig;
use crate::embedding::{Embedder, FastEmbedEmbedder};
use crate::git::{NotesManager, YamlFrontMatterParser};
use crate::models::{Domain, Memory, MemoryId, MemoryStatus, Namespace};
use crate::storage::index::{
    DomainIndexConfig, DomainIndexManager, DomainScope, OrgIndexConfig, SqliteBackend,
    find_repo_root, get_user_data_dir,
};
use crate::storage::traits::{IndexBackend, VectorBackend};
use crate::storage::vector::UsearchBackend;
use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// ============================================================================
// Service Factory Functions
// ============================================================================

/// Creates a [`PromptService`] for the given repository path.
///
/// This is the canonical way to create a `PromptService` from MCP or CLI layers.
/// Configuration is loaded from the default location and merged with repo settings.
///
/// # Arguments
///
/// * `repo_path` - Path to or within a git repository
///
/// # Returns
///
/// A fully configured `PromptService` with storage backends initialized.
///
/// # Example
///
/// ```rust,ignore
/// use subcog::services::prompt_service_for_repo;
///
/// let service = prompt_service_for_repo("/path/to/repo")?;
/// let prompts = service.list(PromptFilter::new())?;
/// ```
#[must_use]
pub fn prompt_service_for_repo(repo_path: impl AsRef<Path>) -> PromptService {
    let repo_path = repo_path.as_ref();
    let config = SubcogConfig::load_default().with_repo_path(repo_path);
    PromptService::with_subcog_config(config).with_repo_path(repo_path)
}

/// Creates a [`PromptService`] for the current working directory.
///
/// This is a convenience function for CLI commands that operate on the current directory.
///
/// # Errors
///
/// Returns an error if the current working directory cannot be determined.
///
/// # Example
///
/// ```rust,ignore
/// use subcog::services::prompt_service_for_cwd;
///
/// let service = prompt_service_for_cwd()?;
/// let prompts = service.list(PromptFilter::new())?;
/// ```
pub fn prompt_service_for_cwd() -> Result<PromptService> {
    let cwd = std::env::current_dir().map_err(|e| Error::OperationFailed {
        operation: "get_current_dir".to_string(),
        cause: e.to_string(),
    })?;
    Ok(prompt_service_for_repo(&cwd))
}

// ============================================================================
// Service Container
// ============================================================================

/// Container for initialized services with configured backends.
///
/// Unlike the previous singleton design, this can be instantiated per-context
/// with domain-scoped indices.
///
/// # `DomainIndexManager` Complexity
///
/// The `index_manager` field uses [`DomainIndexManager`] to provide multi-domain
/// index support with lazy initialization. Key complexity points:
///
/// ## Architecture
///
/// ```text
/// ServiceContainer
///   └── Mutex<DomainIndexManager>
///         ├── Project index (.subcog/index.db)   // per-repo
///         ├── User index (~/.subcog/index.db)    // per-user
///         └── Org index (configured path)         // optional
/// ```
///
/// ## Lazy Initialization
///
/// Indices are created on-demand via `index_for_scope()`:
/// 1. Lock the `Mutex<DomainIndexManager>`
/// 2. Check if index exists for requested `DomainScope`
/// 3. If missing, create `SQLite` database at scope-specific path
/// 4. Return reference to the index
///
/// ## Thread Safety
///
/// - `Mutex` guards the manager, not individual indices
/// - Each index has its own internal locking via `SqliteBackend`
/// - Callers should minimize lock hold time
///
/// ## Path Resolution
///
/// | Scope | Path |
/// |-------|------|
/// | Project | `{repo}/.subcog/index.db` |
/// | User | `~/.subcog/index.db` |
/// | Org | Configured via `OrgIndexConfig` |
///
/// ## Error Handling
///
/// - Missing repo returns `Error::OperationFailed`
/// - `SQLite` initialization errors propagate as `Error::OperationFailed`
/// - Index creation is idempotent (safe to call multiple times)
pub struct ServiceContainer {
    /// Capture service.
    capture: CaptureService,
    /// Sync service.
    sync: SyncService,
    /// Domain index manager for multi-domain indices.
    ///
    /// See struct-level documentation for complexity notes.
    index_manager: Mutex<DomainIndexManager>,
    /// Repository path (if known).
    repo_path: Option<PathBuf>,
    /// Shared embedder for both capture and recall.
    embedder: Option<Arc<dyn Embedder>>,
    /// Shared vector backend for both capture and recall.
    vector: Option<Arc<dyn VectorBackend + Send + Sync>>,
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

        // Create storage backends for CaptureService
        let subcog_dir = repo_root.join(".subcog");
        let index_path = subcog_dir.join("index.db");
        let vector_path = subcog_dir.join("vectors.idx");

        // Ensure .subcog directory exists
        if let Err(e) = std::fs::create_dir_all(&subcog_dir) {
            tracing::warn!("Failed to create .subcog directory: {e}");
        }

        // Create embedder (singleton, always available)
        let embedder: Arc<dyn Embedder> = Arc::new(FastEmbedEmbedder::new());

        // Create index backend (SQLite FTS5)
        let index: Arc<dyn IndexBackend + Send + Sync> = match SqliteBackend::new(&index_path) {
            Ok(backend) => Arc::new(backend),
            Err(e) => {
                tracing::warn!("Failed to create index backend: {e}");
                // Continue without index backend - CaptureService handles gracefully
                return Ok(Self {
                    capture: CaptureService::new(capture_config)
                        .with_embedder(Arc::clone(&embedder)),
                    sync: SyncService::default(),
                    index_manager: Mutex::new(index_manager),
                    repo_path: Some(repo_root),
                    embedder: Some(embedder),
                    vector: None,
                });
            },
        };

        // Create vector backend (usearch HNSW)
        // Note: Fallback UsearchBackend::new() returns Self, native returns Result<Self>
        #[cfg(feature = "usearch-hnsw")]
        let vector_result =
            UsearchBackend::new(&vector_path, FastEmbedEmbedder::DEFAULT_DIMENSIONS);
        #[cfg(not(feature = "usearch-hnsw"))]
        let vector_result: Result<UsearchBackend> = Ok(UsearchBackend::new(
            &vector_path,
            FastEmbedEmbedder::DEFAULT_DIMENSIONS,
        ));

        let vector: Arc<dyn VectorBackend + Send + Sync> = match vector_result {
            Ok(backend) => Arc::new(backend),
            Err(e) => {
                tracing::warn!("Failed to create vector backend: {e}");
                // Continue without vector backend - CaptureService handles gracefully
                return Ok(Self {
                    capture: CaptureService::new(capture_config)
                        .with_embedder(Arc::clone(&embedder))
                        .with_index(Arc::clone(&index)),
                    sync: SyncService::default(),
                    index_manager: Mutex::new(index_manager),
                    repo_path: Some(repo_root),
                    embedder: Some(embedder),
                    vector: None,
                });
            },
        };

        // Create CaptureService with all backends
        let capture = CaptureService::with_backends(
            capture_config,
            Arc::clone(&embedder),
            Arc::clone(&index),
            Arc::clone(&vector),
        );

        Ok(Self {
            capture,
            sync: SyncService::default(),
            index_manager: Mutex::new(index_manager),
            repo_path: Some(repo_root),
            embedder: Some(embedder),
            vector: Some(vector),
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

    /// Creates a service container for user-scoped storage.
    ///
    /// Used when operating outside a git repository. Stores memories in the
    /// user's local data directory using `SQLite` persistence (no git notes).
    ///
    /// # Storage Paths
    ///
    /// | Platform | Path |
    /// |----------|------|
    /// | macOS | `~/Library/Application Support/subcog/` |
    /// | Linux | `~/.local/share/subcog/` |
    /// | Windows | `C:\Users\<User>\AppData\Local\subcog\` |
    ///
    /// # Errors
    ///
    /// Returns an error if the user data directory cannot be created or
    /// storage backends fail to initialize.
    pub fn for_user() -> Result<Self> {
        let user_data_dir = get_user_data_dir()?;

        // Ensure user data directory exists
        std::fs::create_dir_all(&user_data_dir).map_err(|e| Error::OperationFailed {
            operation: "create_user_data_dir".to_string(),
            cause: format!(
                "Cannot create {}: {}. Please create manually with: mkdir -p {}",
                user_data_dir.display(),
                e,
                user_data_dir.display()
            ),
        })?;

        // Create storage paths
        let index_path = user_data_dir.join("index.db");
        let vector_path = user_data_dir.join("vectors.idx");

        // Create domain index config for user-only mode (no repo)
        let config = DomainIndexConfig {
            repo_path: None,
            org_config: None,
        };
        let index_manager = DomainIndexManager::new(config)?;

        // Create CaptureService WITHOUT repo_path (no git notes)
        let capture_config = crate::config::Config::new();

        // Create embedder
        let embedder: Arc<dyn Embedder> = Arc::new(FastEmbedEmbedder::new());

        // Create index backend (SQLite FTS5)
        let index: Arc<dyn IndexBackend + Send + Sync> = match SqliteBackend::new(&index_path) {
            Ok(backend) => Arc::new(backend),
            Err(e) => {
                tracing::warn!("Failed to create index backend for user scope: {e}");
                return Ok(Self {
                    capture: CaptureService::new(capture_config)
                        .with_embedder(Arc::clone(&embedder)),
                    sync: SyncService::no_op(),
                    index_manager: Mutex::new(index_manager),
                    repo_path: None,
                    embedder: Some(embedder),
                    vector: None,
                });
            },
        };

        // Create vector backend (usearch HNSW)
        #[cfg(feature = "usearch-hnsw")]
        let vector_result =
            UsearchBackend::new(&vector_path, FastEmbedEmbedder::DEFAULT_DIMENSIONS);
        #[cfg(not(feature = "usearch-hnsw"))]
        let vector_result: Result<UsearchBackend> = Ok(UsearchBackend::new(
            &vector_path,
            FastEmbedEmbedder::DEFAULT_DIMENSIONS,
        ));

        let vector: Arc<dyn VectorBackend + Send + Sync> = match vector_result {
            Ok(backend) => Arc::new(backend),
            Err(e) => {
                tracing::warn!("Failed to create vector backend for user scope: {e}");
                return Ok(Self {
                    capture: CaptureService::new(capture_config)
                        .with_embedder(Arc::clone(&embedder))
                        .with_index(Arc::clone(&index)),
                    sync: SyncService::no_op(),
                    index_manager: Mutex::new(index_manager),
                    repo_path: None,
                    embedder: Some(embedder),
                    vector: None,
                });
            },
        };

        // Create CaptureService with all backends (but no repo_path = no git notes)
        let capture = CaptureService::with_backends(
            capture_config,
            Arc::clone(&embedder),
            Arc::clone(&index),
            Arc::clone(&vector),
        );

        tracing::info!(
            user_data_dir = %user_data_dir.display(),
            "Created user-scoped service container"
        );

        Ok(Self {
            capture,
            sync: SyncService::no_op(),
            index_manager: Mutex::new(index_manager),
            repo_path: None,
            embedder: Some(embedder),
            vector: Some(vector),
        })
    }

    /// Creates a service container from the current directory, falling back to user scope.
    ///
    /// This is the recommended factory method for CLI and MCP entry points:
    /// - If in a git repository → uses project scope (git notes + local index)
    /// - If NOT in a git repository → uses user scope (SQLite-only)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Works in any directory
    /// let container = ServiceContainer::from_current_dir_or_user()?;
    ///
    /// // In git repo: subcog://global/{namespace}/{id}
    /// // Outside git: subcog://user/{namespace}/{id}
    /// let result = container.capture().capture(request)?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error only if both project and user scope fail to initialize.
    pub fn from_current_dir_or_user() -> Result<Self> {
        // Try project scope first
        match Self::from_current_dir() {
            Ok(container) => {
                tracing::debug!("Using project-scoped service container");
                Ok(container)
            },
            Err(e) => {
                tracing::debug!(
                    error = %e,
                    "Not in git repository, falling back to user scope"
                );
                Self::for_user()
            },
        }
    }

    /// Returns whether this container is using user scope (no git repository).
    #[must_use]
    pub const fn is_user_scope(&self) -> bool {
        self.repo_path.is_none()
    }

    /// Creates a recall service for a specific domain scope.
    ///
    /// The recall service is configured with:
    /// - Index backend (`SQLite` FTS5) for text search
    /// - Embedder for generating query embeddings (if available)
    /// - Vector backend for similarity search (if available)
    ///
    /// # Errors
    ///
    /// Returns an error if the index cannot be initialized.
    pub fn recall_for_scope(&self, scope: DomainScope) -> Result<RecallService> {
        // Get index path while holding the lock, then release before I/O
        let index_path = {
            let manager = self
                .index_manager
                .lock()
                .map_err(|e| Error::OperationFailed {
                    operation: "lock_index_manager".to_string(),
                    cause: e.to_string(),
                })?;
            manager.get_index_path(scope)?
        }; // Lock released here

        // Ensure parent directory exists (I/O outside mutex)
        if let Some(parent) = index_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::OperationFailed {
                operation: "create_index_dir".to_string(),
                cause: e.to_string(),
            })?;
        }

        let index = SqliteBackend::new(&index_path)?;

        // Start with index-only service
        let mut service = RecallService::with_index(index);

        // Add embedder and vector backends if available
        if let Some(ref embedder) = self.embedder {
            service = service.with_embedder(Arc::clone(embedder));
        }
        if let Some(ref vector) = self.vector {
            service = service.with_vector(Arc::clone(vector));
        }

        Ok(service)
    }

    /// Creates a recall service for the appropriate scope.
    ///
    /// Uses user scope for user-scoped containers, project scope otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the index cannot be initialized.
    pub fn recall(&self) -> Result<RecallService> {
        let scope = if self.is_user_scope() {
            DomainScope::User
        } else {
            DomainScope::Project
        };
        self.recall_for_scope(scope)
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

    /// Returns a reference to the embedder if available.
    #[must_use]
    pub fn embedder(&self) -> Option<Arc<dyn Embedder>> {
        self.embedder.clone()
    }

    /// Returns a reference to the vector backend if available.
    #[must_use]
    pub fn vector(&self) -> Option<Arc<dyn VectorBackend + Send + Sync>> {
        self.vector.clone()
    }

    /// Creates an index backend for the project scope.
    ///
    /// # Errors
    ///
    /// Returns an error if the index cannot be initialized.
    pub fn index(&self) -> Result<SqliteBackend> {
        let index_path = self.get_index_path(DomainScope::Project)?;

        // Ensure parent directory exists
        if let Some(parent) = index_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::OperationFailed {
                operation: "create_index_dir".to_string(),
                cause: e.to_string(),
            })?;
        }

        SqliteBackend::new(&index_path)
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

        let index = SqliteBackend::new(&index_path)?;

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
