//! Business logic services.
//!
//! Services orchestrate storage backends and provide high-level operations.
//!
//! # Examples
//!
//! Create a service container and capture a memory:
//!
//! ```rust,ignore
//! use subcog::services::ServiceContainer;
//! use subcog::models::{CaptureRequest, Namespace, Domain};
//!
//! let container = ServiceContainer::from_current_dir_or_user()?;
//!
//! let request = CaptureRequest {
//!     content: "Use PostgreSQL for production storage".to_string(),
//!     namespace: Namespace::Decisions,
//!     domain: Domain::default(),
//!     tags: vec!["database".to_string(), "architecture".to_string()],
//!     source: Some("ARCHITECTURE.md".to_string()),
//!     skip_security_check: false,
//! };
//!
//! let result = container.capture().capture(request)?;
//! println!("Captured: {}", result.urn);
//! # Ok::<(), subcog::Error>(())
//! ```
//!
//! Search for memories with the recall service:
//!
//! ```rust,ignore
//! use subcog::services::ServiceContainer;
//! use subcog::models::{SearchFilter, SearchMode};
//!
//! let container = ServiceContainer::from_current_dir_or_user()?;
//! let recall = container.recall()?;
//!
//! let filter = SearchFilter::new().with_namespace(subcog::models::Namespace::Decisions);
//! let results = recall.search("database storage", SearchMode::Hybrid, &filter, 10)?;
//!
//! for hit in &results.memories {
//!     println!("{}: {:.2}", hit.memory.id.as_str(), hit.score);
//! }
//! # Ok::<(), subcog::Error>(())
//! ```
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
mod context_template;
mod data_subject;
pub mod deduplication;
mod enrichment;
mod entity_extraction;
mod graph;
mod graph_rag;
pub mod migration;
mod path_manager;
mod prompt;
mod prompt_enrichment;
mod prompt_parser;
mod query_parser;
mod recall;
mod sync;
mod tombstone;
mod topic_index;

#[cfg(feature = "group-scope")]
pub mod group;

pub use auth::{AuthContext, AuthContextBuilder, Permission};
pub use backend_factory::{BackendFactory, BackendSet};
pub use capture::{CaptureService, EntityExtractionCallback, EntityExtractionStats};
pub use consolidation::{ConsolidationService, ConsolidationStats};
pub use context::{ContextBuilderService, MemoryStatistics};
pub use context_template::{
    ContextTemplateFilter, ContextTemplateService, RenderResult, ValidationIssue, ValidationResult,
    ValidationSeverity,
};
pub use data_subject::{
    ConsentPurpose, ConsentRecord, ConsentStatus, DataSubjectService, DeletionResult,
    ExportMetadata, ExportedMemory, UserDataExport,
};
pub use deduplication::{
    DeduplicationConfig, DeduplicationService, Deduplicator, DuplicateCheckResult, DuplicateReason,
};
pub use enrichment::{EnrichmentResult, EnrichmentService, EnrichmentStats};
pub use entity_extraction::{
    EntityExtractorService, ExtractedEntity, ExtractedRelationship, ExtractionResult,
    InferenceResult, InferredRelationship,
};
pub use graph::GraphService;
pub use graph_rag::{
    ExpansionConfig, GraphRAGConfig, GraphRAGService, GraphSearchHit, GraphSearchResults,
    SearchProvenance,
};
pub use path_manager::{
    GRAPH_DB_NAME, INDEX_DB_NAME, PathManager, SUBCOG_DIR_NAME, VECTOR_INDEX_NAME,
};
pub use prompt::{PromptFilter, PromptService, SaveOptions, SaveResult};
pub use prompt_enrichment::{
    ENRICHMENT_TIMEOUT, EnrichmentRequest, EnrichmentStatus, PROMPT_ENRICHMENT_SYSTEM_PROMPT,
    PartialMetadata, PromptEnrichmentResult, PromptEnrichmentService,
};
pub use prompt_parser::{PromptFormat, PromptParser};
pub use query_parser::parse_filter_query;
pub use recall::RecallService;
pub use sync::SyncService;
pub use tombstone::TombstoneService;
pub use topic_index::{TopicIndexService, TopicInfo};

// Group service (feature-gated)
#[cfg(feature = "group-scope")]
pub use group::GroupService;

use crate::cli::build_llm_provider_for_entity_extraction;
use crate::config::SubcogConfig;
use crate::context::GitContext;
use crate::embedding::Embedder;
use crate::models::{Memory, MemoryId, SearchFilter};
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
///         ├── Project index (<user-data>/index.db) // faceted by project/branch/path
///         ├── User index (<user-data>/index.db)    // user-wide
///         └── Org index (configured path)          // optional
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
/// | Project | `<user-data>/index.db` |
/// | User | `<user-data>/index.db` |
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
    /// User data directory (from config, used for graph and other user-scoped data).
    user_data_dir: PathBuf,
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

        if org_config.is_some() {
            let config = SubcogConfig::load_default().with_repo_path(&repo_root);
            if !(config.features.org_scope_enabled || cfg!(feature = "org-scope")) {
                tracing::warn!(
                    "Org-scope config provided but org-scope is disabled. \
                     Set SUBCOG_ORG_SCOPE_ENABLED=true or build with --features org-scope."
                );
                return Err(Error::FeatureNotEnabled("org-scope".to_string()));
            }
        }

        let config = DomainIndexConfig {
            repo_path: Some(repo_root.clone()),
            org_config,
        };

        let index_manager = DomainIndexManager::new(config)?;

        // Load config to get user's configured data_dir (respects config.toml)
        let subcog_config = SubcogConfig::load_default();

        // Create CaptureService with repo_path for project-scoped storage
        // Propagate auto_extract_entities from loaded config
        let mut capture_config = crate::config::Config::new().with_repo_path(&repo_root);
        capture_config.features.auto_extract_entities =
            subcog_config.features.auto_extract_entities;
        let user_data_dir = subcog_config.data_dir.clone();

        std::fs::create_dir_all(&user_data_dir).map_err(|e| Error::OperationFailed {
            operation: "create_user_data_dir".to_string(),
            cause: format!(
                "Cannot create {}: {}. Please create manually with: mkdir -p {}",
                user_data_dir.display(),
                e,
                user_data_dir.display()
            ),
        })?;

        // Create storage paths using user-level data directory (project facets)
        let paths = PathManager::for_user(&user_data_dir);

        // Create backends using factory (centralizes initialization logic)
        let backends = BackendFactory::create_all(&paths.index_path(), &paths.vector_path());

        // Build LLM provider for entity extraction with longer timeout (120s default)
        let llm_provider = build_llm_provider_for_entity_extraction(&subcog_config);

        // Create entity extraction callback if auto-extraction is enabled
        let entity_extraction =
            Self::create_entity_extraction_callback(&capture_config, &paths, llm_provider);

        // Build CaptureService based on available backends
        let capture = Self::build_capture_service(capture_config, &backends, entity_extraction);

        Ok(Self {
            capture,
            sync: SyncService::default(),
            index_manager: Mutex::new(index_manager),
            repo_path: Some(repo_root),
            user_data_dir,
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
        // Load config to get user's configured data_dir (respects config.toml)
        let subcog_config = SubcogConfig::load_default();
        let user_data_dir = subcog_config.data_dir.clone();

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
        // Propagate auto_extract_entities from loaded config
        let mut capture_config = crate::config::Config::new();
        capture_config.features.auto_extract_entities =
            subcog_config.features.auto_extract_entities;

        // Create backends using factory (centralizes initialization logic)
        let backends = BackendFactory::create_all(&paths.index_path(), &paths.vector_path());

        // Build LLM provider for entity extraction with longer timeout (120s default)
        let llm_provider = build_llm_provider_for_entity_extraction(&subcog_config);

        // Create entity extraction callback if auto-extraction is enabled
        let entity_extraction =
            Self::create_entity_extraction_callback(&capture_config, &paths, llm_provider);

        // Build CaptureService based on available backends
        let capture = Self::build_capture_service(capture_config, &backends, entity_extraction);

        tracing::info!(
            user_data_dir = %user_data_dir.display(),
            "Created user-scoped service container"
        );

        Ok(Self {
            capture,
            sync: SyncService::no_op(),
            index_manager: Mutex::new(index_manager),
            repo_path: None,
            user_data_dir,
            embedder: backends.embedder,
            vector: backends.vector,
        })
    }

    /// Creates a service container from the current directory, falling back to user scope.
    ///
    /// This is the recommended factory method for CLI and MCP entry points:
    /// - If in a git repository → uses project scope (user-level index + project facets)
    /// - If NOT in a git repository → uses user scope (user-level index)
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

        if matches!(scope, DomainScope::Project)
            && let Some(filter) = self.project_scope_filter()
        {
            service = service.with_scope_filter(filter);
        }

        Ok(service)
    }

    fn project_scope_filter(&self) -> Option<SearchFilter> {
        let repo_path = self.repo_path.as_ref()?;
        let git_context = GitContext::from_path(repo_path);
        git_context
            .project_id
            .map(|project_id| SearchFilter::new().with_project_id(project_id))
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
    ///
    /// # Arguments
    ///
    /// * `config` - The capture configuration
    /// * `backends` - Available storage backends
    /// * `entity_extraction` - Optional callback for entity extraction (Graph RAG)
    fn build_capture_service(
        config: crate::config::Config,
        backends: &BackendSet,
        entity_extraction: Option<capture::EntityExtractionCallback>,
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

        // Add entity extraction callback if provided
        if let Some(callback) = entity_extraction {
            service = service.with_entity_extraction(callback);
        }

        service
    }

    /// Creates an entity extraction callback if auto-extraction is enabled.
    ///
    /// The callback:
    /// 1. Extracts entities from the memory content using [`EntityExtractorService`]
    /// 2. Stores entities and relationships in the [`GraphService`]
    /// 3. Records mentions linking memories to entities
    ///
    /// # Arguments
    ///
    /// * `config` - The capture configuration (checked for `auto_extract_entities` flag)
    /// * `paths` - Path manager for locating the graph database
    /// * `llm` - Optional LLM provider for intelligent extraction
    ///
    /// # Returns
    ///
    /// `Some(callback)` if auto-extraction is enabled and graph backend initializes,
    /// `None` otherwise (graceful degradation).
    #[allow(clippy::excessive_nesting)] // Callback closures require nested scopes
    fn create_entity_extraction_callback(
        config: &crate::config::Config,
        paths: &PathManager,
        llm: Option<Arc<dyn crate::llm::LlmProvider>>,
    ) -> Option<capture::EntityExtractionCallback> {
        // Check if auto-extraction is enabled
        if !config.features.auto_extract_entities {
            return None;
        }

        // Create graph backend (gracefully degrade if it fails)
        let graph_path = paths.graph_path();
        let graph_backend = match crate::storage::graph::SqliteGraphBackend::new(&graph_path) {
            Ok(backend) => backend,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Failed to create graph backend for entity extraction, disabling"
                );
                return None;
            },
        };

        // Create services wrapped in Arc for sharing
        let graph_service = Arc::new(GraphService::new(graph_backend));
        let domain = crate::models::Domain::new(); // Default domain for extraction

        let entity_extractor = if let Some(llm) = llm {
            Arc::new(EntityExtractorService::with_shared_llm(llm, domain))
        } else {
            Arc::new(EntityExtractorService::without_llm(domain))
        };

        // Create the callback that captures the services
        let callback: capture::EntityExtractionCallback = Arc::new(move |content, memory_id| {
            use crate::models::graph::{Entity, EntityType, Relationship, RelationshipType};
            use std::collections::HashMap;

            let mut stats = capture::EntityExtractionStats::default();

            // Extract entities from content
            let extraction = entity_extractor.extract(content)?;
            stats.used_fallback = extraction.used_fallback;

            // Map entity names to IDs for relationship resolution
            let mut name_to_id: HashMap<String, crate::models::graph::EntityId> = HashMap::new();

            // Store entities in graph
            for extracted in &extraction.entities {
                // Parse entity type, defaulting to Concept if unknown
                let entity_type =
                    EntityType::parse(&extracted.entity_type).unwrap_or(EntityType::Concept);

                // Create the Entity from ExtractedEntity
                let entity =
                    Entity::new(entity_type, &extracted.name, crate::models::Domain::new())
                        .with_confidence(extracted.confidence)
                        .with_aliases(extracted.aliases.iter().cloned());

                // Store entity with deduplication (returns actual ID, existing or new)
                match graph_service.store_entity_deduped(&entity) {
                    Ok(actual_id) => {
                        stats.entities_stored += 1;

                        // Track name to ID mapping for relationship resolution
                        // Use the actual ID returned (may be existing entity's ID)
                        name_to_id.insert(extracted.name.clone(), actual_id.clone());
                        for alias in &extracted.aliases {
                            name_to_id.insert(alias.clone(), actual_id.clone());
                        }

                        // Record mention linking memory to entity
                        if let Err(e) = graph_service.record_mention(&actual_id, memory_id) {
                            tracing::debug!(
                                memory_id = %memory_id,
                                entity_id = %actual_id.as_ref(),
                                error = %e,
                                "Failed to record entity mention"
                            );
                        }
                    },
                    Err(e) => {
                        tracing::debug!(
                            entity_name = %extracted.name,
                            error = %e,
                            "Failed to store entity"
                        );
                    },
                }
            }

            // Store relationships in graph
            for extracted_rel in &extraction.relationships {
                // Look up entity IDs by name - skip if either entity not found
                let (Some(from), Some(to)) = (
                    name_to_id.get(&extracted_rel.from),
                    name_to_id.get(&extracted_rel.to),
                ) else {
                    tracing::debug!(
                        from = %extracted_rel.from,
                        to = %extracted_rel.to,
                        "Skipping relationship: one or both entities not found"
                    );
                    continue;
                };

                // Parse relationship type, defaulting to RelatesTo if unknown
                let rel_type = RelationshipType::parse(&extracted_rel.relationship_type)
                    .unwrap_or(RelationshipType::RelatesTo);

                let relationship = Relationship::new(from.clone(), to.clone(), rel_type)
                    .with_confidence(extracted_rel.confidence);

                if let Err(e) = graph_service.store_relationship(&relationship) {
                    tracing::debug!(
                        from = %extracted_rel.from,
                        to = %extracted_rel.to,
                        error = %e,
                        "Failed to store relationship"
                    );
                } else {
                    stats.relationships_stored += 1;
                }
            }

            Ok(stats)
        });

        Some(callback)
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

    /// Creates a data subject service for GDPR operations.
    ///
    /// Provides:
    /// - `export_user_data()` - Export all user data (GDPR Article 20)
    /// - `delete_user_data()` - Delete all user data (GDPR Article 17)
    ///
    /// # Errors
    ///
    /// Returns an error if the index backend cannot be initialized.
    pub fn data_subject(&self) -> Result<DataSubjectService> {
        let index = self.index()?;
        let mut service = DataSubjectService::new(index);
        if let Some(ref vector) = self.vector {
            service = service.with_vector(Arc::clone(vector));
        }
        Ok(service)
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

    /// Creates a graph service for knowledge graph operations.
    ///
    /// The graph service stores entities and relationships in a dedicated
    /// `SQLite` database (`graph.db`) in the user data directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the graph backend cannot be initialized.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let container = ServiceContainer::from_current_dir_or_user()?;
    /// let graph = container.graph()?;
    ///
    /// let entity = graph.store_entity(Entity::new(EntityType::Technology, "Rust", domain))?;
    /// ```
    pub fn graph(&self) -> Result<GraphService<crate::storage::graph::SqliteGraphBackend>> {
        use crate::storage::graph::SqliteGraphBackend;

        // Use the configured user_data_dir (respects config.toml data_dir setting)
        let paths = PathManager::for_user(&self.user_data_dir);
        let graph_path = paths.graph_path();

        let backend = SqliteGraphBackend::new(&graph_path).map_err(|e| Error::OperationFailed {
            operation: "create_graph_backend".to_string(),
            cause: e.to_string(),
        })?;

        Ok(GraphService::new(backend))
    }

    /// Creates an entity extractor service for extracting entities from text.
    ///
    /// The extractor uses pattern-based fallback when no LLM is provided.
    /// For LLM-powered extraction, use [`Self::entity_extractor_with_llm`].
    ///
    /// # Returns
    ///
    /// An [`EntityExtractorService`] configured for the appropriate domain.
    #[must_use]
    pub fn entity_extractor(&self) -> EntityExtractorService {
        let domain = self.current_domain();
        EntityExtractorService::without_llm(domain)
    }

    /// Creates an entity extractor service with LLM support.
    ///
    /// The extractor uses the provided LLM for intelligent entity extraction.
    /// Falls back to pattern-based extraction if LLM calls fail.
    ///
    /// # Arguments
    ///
    /// * `llm` - The LLM provider to use for extraction.
    ///
    /// # Returns
    ///
    /// An [`EntityExtractorService`] configured with LLM support.
    pub fn entity_extractor_with_llm(
        &self,
        llm: Arc<dyn crate::llm::LlmProvider>,
    ) -> EntityExtractorService {
        let domain = self.current_domain();
        EntityExtractorService::with_shared_llm(llm, domain)
    }

    /// Returns the current domain based on scope.
    ///
    /// - If in a git repository: returns project-scoped domain (`Domain::new()`)
    /// - If NOT in a git repository: returns user-scoped domain (`Domain::for_user()`)
    fn current_domain(&self) -> crate::models::Domain {
        if self.repo_path.is_some() {
            // Project scope: uses user-level storage with project facets
            crate::models::Domain::new()
        } else {
            // User scope: uses user-level storage without project facets
            crate::models::Domain::for_user()
        }
    }

    /// Creates a webhook service for event notifications.
    ///
    /// The webhook service subscribes to memory events and delivers them to
    /// configured webhook endpoints. Configuration is loaded from
    /// `~/.config/subcog/webhooks.yaml`.
    ///
    /// Returns `Ok(None)` if no webhooks are configured.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid or the audit database
    /// cannot be created.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let container = ServiceContainer::from_current_dir_or_user()?;
    /// if let Some(webhook_service) = container.webhook_service()? {
    ///     // Start webhook dispatcher as background task
    ///     let _handle = webhook_service.start();
    /// }
    /// ```
    pub fn webhook_service(&self) -> Result<Option<crate::webhooks::WebhookService>> {
        let scope = if self.is_user_scope() {
            crate::storage::index::DomainScope::User
        } else {
            crate::storage::index::DomainScope::Project
        };

        let user_data_dir = get_user_data_dir()?;
        crate::webhooks::WebhookService::from_config_file(scope, &user_data_dir)
    }
}
