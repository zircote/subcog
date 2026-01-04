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

pub mod auth;
mod backend_factory;
mod capture;
mod consolidation;
mod context;
pub mod deduplication;
mod enrichment;
pub mod migration;
mod path_manager;
mod prompt;
mod prompt_enrichment;
mod prompt_parser;
mod query_parser;
mod recall;
mod sync;
mod topic_index;

pub use auth::{AuthContext, AuthContextBuilder, Permission};
pub use backend_factory::{BackendFactory, BackendSet};
pub use capture::CaptureService;
pub use consolidation::ConsolidationService;
pub use context::{ContextBuilderService, MemoryStatistics};
pub use deduplication::{
    DeduplicationConfig, DeduplicationService, Deduplicator, DuplicateCheckResult, DuplicateReason,
};
pub use enrichment::{EnrichmentResult, EnrichmentService, EnrichmentStats};
pub use path_manager::{INDEX_DB_NAME, PathManager, SUBCOG_DIR_NAME, VECTOR_INDEX_NAME};
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
use crate::embedding::Embedder;
use crate::models::{Memory, MemoryId};
use crate::storage::index::{
    DomainIndexConfig, DomainIndexManager, DomainScope, OrgIndexConfig, SqliteBackend,
    find_repo_root, get_user_data_dir,
};
use crate::storage::traits::{IndexBackend, VectorBackend};
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

        // Create CaptureService with repo_path for project-scoped storage
        let capture_config = crate::config::Config::new().with_repo_path(&repo_root);

        // Create storage paths using PathManager
        let paths = PathManager::for_repo(&repo_root);

        // Ensure .subcog directory exists
        if let Err(e) = paths.ensure_subcog_dir() {
            tracing::warn!("Failed to create .subcog directory: {e}");
        }

        // Create backends using factory (centralizes initialization logic)
        let backends = BackendFactory::create_all(&paths.index_path(), &paths.vector_path());

        // Build CaptureService based on available backends
        let capture = Self::build_capture_service(capture_config, &backends);

        Ok(Self {
            capture,
            sync: SyncService::default(),
            index_manager: Mutex::new(index_manager),
            repo_path: Some(repo_root),
            embedder: backends.embedder,
            vector: backends.vector,
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
    /// user's local data directory using `SQLite` persistence.
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

        // Create storage paths using PathManager
        let paths = PathManager::for_user(&user_data_dir);

        // Create domain index config for user-only mode (no repo)
        let config = DomainIndexConfig {
            repo_path: None,
            org_config: None,
        };
        let index_manager = DomainIndexManager::new(config)?;

        // Create CaptureService WITHOUT repo_path (user scope)
        let capture_config = crate::config::Config::new();

        // Create backends using factory (centralizes initialization logic)
        let backends = BackendFactory::create_all(&paths.index_path(), &paths.vector_path());

        // Build CaptureService based on available backends
        let capture = Self::build_capture_service(capture_config, &backends);

        tracing::info!(
            user_data_dir = %user_data_dir.display(),
            "Created user-scoped service container"
        );

        Ok(Self {
            capture,
            sync: SyncService::no_op(),
            index_manager: Mutex::new(index_manager),
            repo_path: None,
            embedder: backends.embedder,
            vector: backends.vector,
        })
    }

    /// Creates a service container from the current directory, falling back to user scope.
    ///
    /// This is the recommended factory method for CLI and MCP entry points:
    /// - If in a git repository → uses project scope (`SQLite` + local index)
    /// - If NOT in a git repository → uses user scope (SQLite-only)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Works in any directory
    /// let container = ServiceContainer::from_current_dir_or_user()?;
    ///
    /// // In git repo: subcog://project/{namespace}/{id}
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
        // Use high-level API that handles path resolution and directory creation
        let index = {
            let manager = self
                .index_manager
                .lock()
                .map_err(|e| Error::OperationFailed {
                    operation: "lock_index_manager".to_string(),
                    cause: e.to_string(),
                })?;
            manager.create_backend(scope)?
        }; // Lock released here

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
        let manager = self
            .index_manager
            .lock()
            .map_err(|e| Error::OperationFailed {
                operation: "lock_index_manager".to_string(),
                cause: e.to_string(),
            })?;
        manager.create_backend(DomainScope::Project)
    }

    /// Builds a `CaptureService` from available backends.
    ///
    /// Applies graceful degradation: uses whatever backends are available.
    fn build_capture_service(
        config: crate::config::Config,
        backends: &BackendSet,
    ) -> CaptureService {
        let mut service = CaptureService::new(config);

        // Add embedder if available
        if let Some(ref embedder) = backends.embedder {
            service = service.with_embedder(Arc::clone(embedder));
        }

        // Add index backend if available
        if let Some(ref index) = backends.index {
            service = service.with_index(Arc::clone(index));
        }

        // Add vector backend if available
        if let Some(ref vector) = backends.vector {
            service = service.with_vector(Arc::clone(vector));
        }

        service
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

    /// Rebuilds the FTS index from `SQLite` data for a specific scope.
    ///
    /// Since `SQLite` is the authoritative storage, this function reads all memories
    /// from the `SQLite` database and rebuilds the FTS5 full-text search index.
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
    /// Returns an error if reading or indexing fails.
    pub fn reindex_scope(&self, scope: DomainScope) -> Result<usize> {
        use crate::models::SearchFilter;

        // Create index backend using high-level API
        let index = {
            let manager = self
                .index_manager
                .lock()
                .map_err(|e| Error::OperationFailed {
                    operation: "lock_index_manager".to_string(),
                    cause: e.to_string(),
                })?;
            manager.create_backend(scope)?
        };

        // Get all memory IDs from SQLite
        let filter = SearchFilter::default();
        let all_ids = index.list_all(&filter, usize::MAX)?;

        if all_ids.is_empty() {
            return Ok(0);
        }

        // Get full memories
        let ids: Vec<MemoryId> = all_ids.into_iter().map(|(id, _)| id).collect();
        let memories: Vec<Memory> = index
            .get_memories_batch(&ids)?
            .into_iter()
            .flatten()
            .collect();

        if memories.is_empty() {
            return Ok(0);
        }

        // Clear FTS and rebuild
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
