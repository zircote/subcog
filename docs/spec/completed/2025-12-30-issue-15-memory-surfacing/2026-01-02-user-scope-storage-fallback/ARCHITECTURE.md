# Architecture: ServiceContainer User-Scope Storage Fallback

## Document Control

| Field | Value |
|-------|-------|
| Version | 1.0 |
| Status | Draft |
| Last Updated | 2026-01-02 |
| Author | Claude (Architect) |

## Overview

This document describes the architectural changes required to support automatic fallback to user-scoped storage when subcog operates outside a git repository.

## Current Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        ServiceContainer                          │
├─────────────────────────────────────────────────────────────────┤
│  from_current_dir()                                              │
│       │                                                          │
│       ▼                                                          │
│  find_repo_root() ──── ERROR if no .git ────► Operation Fails   │
│       │                                                          │
│       ▼ (success)                                                │
│  for_repo(repo_path)                                             │
│       │                                                          │
│       ├──► CaptureService (git_notes persistence)                │
│       ├──► RecallService (project index)                         │
│       └──► SyncService (git remote)                              │
└─────────────────────────────────────────────────────────────────┘
```

**Problem**: `from_current_dir()` always requires a git repository.

## Proposed Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        ServiceContainer                          │
├─────────────────────────────────────────────────────────────────┤
│  from_current_dir_or_user()                                      │
│       │                                                          │
│       ▼                                                          │
│  is_in_git_repo()?                                               │
│       │                                                          │
│       ├── YES ──► for_repo(repo_path)                            │
│       │               │                                          │
│       │               ├──► CaptureService (git_notes)            │
│       │               ├──► RecallService (project index)         │
│       │               └──► SyncService (git remote)              │
│       │                                                          │
│       └── NO ───► for_user()                 [NEW]               │
│                       │                                          │
│                       ├──► CaptureService (sqlite-only)          │
│                       ├──► RecallService (user index)            │
│                       └──► SyncService (no-op)                   │
└─────────────────────────────────────────────────────────────────┘
```

## Component Changes

### 1. ServiceContainer Extensions

#### New Method: `for_user()`

Creates a ServiceContainer for user-scoped storage without git dependency.

```rust
impl ServiceContainer {
    /// Creates a service container for user-scoped storage.
    ///
    /// Used when operating outside a git repository. Stores memories
    /// in the user's local data directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the user data directory cannot be created.
    pub fn for_user() -> Result<Self> {
        let user_data_dir = get_user_data_dir()?;

        // Create storage paths
        let index_path = user_data_dir.join("index.db");
        let vector_path = user_data_dir.join("vectors.idx");

        // Ensure directory exists
        std::fs::create_dir_all(&user_data_dir)?;

        // Create embedder (singleton, always available)
        let embedder: Arc<dyn Embedder> = Arc::new(FastEmbedEmbedder::new());

        // Create index backend
        let index: Arc<dyn IndexBackend + Send + Sync> =
            Arc::new(SqliteBackend::new(&index_path)?);

        // Create vector backend
        let vector: Arc<dyn VectorBackend + Send + Sync> =
            Arc::new(UsearchBackend::new(&vector_path, 384)?);

        // Create CaptureService WITHOUT git_notes (sqlite-only persistence)
        let capture_config = Config::new(); // No repo_path
        let capture = CaptureService::with_backends_no_git(
            capture_config,
            Arc::clone(&embedder),
            Arc::clone(&index),
            Arc::clone(&vector),
        );

        // Create index manager for user scope
        let index_config = DomainIndexConfig {
            repo_path: None,  // No repo
            org_config: None,
        };
        let index_manager = DomainIndexManager::new_for_user(index_config)?;

        Ok(Self {
            capture,
            sync: SyncService::no_op(), // No git remote
            index_manager: Mutex::new(index_manager),
            repo_path: None,
            embedder: Some(embedder),
            vector: Some(vector),
        })
    }
}
```

#### New Method: `from_current_dir_or_user()`

Factory method that automatically selects the appropriate scope.

```rust
impl ServiceContainer {
    /// Creates a service container from current directory or falls back to user scope.
    ///
    /// - If in a git repository: uses project scope (git notes + local index)
    /// - If NOT in a git repository: uses user scope (sqlite-only)
    ///
    /// # Errors
    ///
    /// Returns an error only if both project and user scope fail to initialize.
    pub fn from_current_dir_or_user() -> Result<Self> {
        // Try project scope first
        if let Ok(container) = Self::from_current_dir() {
            return Ok(container);
        }

        // Fall back to user scope
        Self::for_user()
    }
}
```

### 2. CaptureService Modifications

#### New Constructor: `with_backends_no_git()`

Creates CaptureService that persists to SQLite only (no git notes).

```rust
impl CaptureService {
    /// Creates a CaptureService with storage backends but NO git notes.
    ///
    /// Used for user-scoped storage where git is not available.
    /// Memories are persisted directly to SQLite index.
    pub fn with_backends_no_git(
        config: Config,
        embedder: Arc<dyn Embedder>,
        index: Arc<dyn IndexBackend + Send + Sync>,
        vector: Arc<dyn VectorBackend + Send + Sync>,
    ) -> Self {
        Self {
            config,
            embedder: Some(embedder),
            index: Some(index),
            vector: Some(vector),
            use_git_notes: false,  // New field
        }
    }
}
```

#### Modified `capture()` Method

Conditionally skip git notes persistence.

```rust
impl CaptureService {
    pub fn capture(&self, request: CaptureRequest) -> Result<CaptureResult> {
        // ... existing validation and memory creation ...

        // Store to git notes (if enabled)
        if self.use_git_notes {
            if let Some(repo_path) = &self.config.repo_path {
                let notes = NotesManager::new(repo_path);
                notes.add(&memory)?;
            }
        }

        // Store to index (always, for searchability)
        if let Some(ref index) = self.index {
            if let Err(e) = index.index(&memory) {
                tracing::warn!("Failed to index memory: {e}");
            }
        }

        // Store embedding to vector store (always)
        if let (Some(ref vector), Some(ref embedding)) = (&self.vector, &memory.embedding) {
            if let Err(e) = vector.upsert(&memory.id.to_string(), embedding) {
                tracing::warn!("Failed to store vector: {e}");
            }
        }

        Ok(CaptureResult { ... })
    }
}
```

### 3. DomainIndexManager Extensions

#### New Constructor: `new_for_user()`

Creates index manager configured for user scope only.

```rust
impl DomainIndexManager {
    /// Creates a domain index manager for user scope only.
    ///
    /// Used when no git repository is available.
    pub fn new_for_user(config: DomainIndexConfig) -> Result<Self> {
        let user_data_dir = get_user_data_dir()?;

        Ok(Self {
            indices: HashMap::new(),
            config,
            user_data_dir,
            default_scope: DomainScope::User,  // New field
        })
    }
}
```

### 4. SyncService Extensions

#### New Constructor: `no_op()`

Creates a no-op sync service for user scope.

```rust
impl SyncService {
    /// Creates a sync service that does nothing.
    ///
    /// Used for user-scoped storage where there's no git remote.
    pub fn no_op() -> Self {
        Self {
            config: Config::new(),
            enabled: false,  // New field
        }
    }

    pub fn sync(&self) -> Result<SyncStats> {
        if !self.enabled {
            return Ok(SyncStats::empty());
        }
        // ... existing sync logic ...
    }
}
```

## Storage Layout

### Project Scope (Git Repository)

```
<repo>/
├── .git/
│   └── notes/
│       └── subcog/
│           └── memories    # Git notes storage
└── .subcog/
    ├── index.db            # SQLite FTS5 index
    └── vectors.idx         # usearch HNSW index
```

### User Scope (No Git Repository)

```
~/.local/share/subcog/      # Linux
~/Library/Application Support/subcog/  # macOS
C:\Users\<User>\AppData\Local\subcog\  # Windows
├── memories.db             # SQLite persistence (replaces git notes)
├── index.db                # SQLite FTS5 index
└── vectors.idx             # usearch HNSW index
```

## Data Flow

### Capture Flow (User Scope)

```
Request ──► CaptureService
               │
               ├──► Generate embedding (FastEmbed)
               │
               ├──► Store to SQLite (memories.db) [NEW - replaces git notes]
               │
               ├──► Index in FTS5 (index.db)
               │
               └──► Upsert vector (vectors.idx)
               │
               ▼
           CaptureResult
               │
               └──► URN: subcog://user/{namespace}/{id}
```

### Recall Flow (User Scope)

```
Query ──► RecallService
              │
              ├──► Text search (index.db via FTS5)
              │
              ├──► Vector search (vectors.idx via usearch)
              │
              └──► RRF fusion + normalization
              │
              ▼
          SearchResult
              │
              └──► URN: subcog://user/{namespace}/{id}
```

## Interface Changes

### ServiceContainer

```rust
// Existing (unchanged)
pub fn for_repo(repo_path: impl Into<PathBuf>, org_config: Option<OrgIndexConfig>) -> Result<Self>;
pub fn from_current_dir() -> Result<Self>;

// New
pub fn for_user() -> Result<Self>;
pub fn from_current_dir_or_user() -> Result<Self>;
```

### CaptureService

```rust
// Existing (unchanged)
pub fn new(config: Config) -> Self;
pub fn with_backends(...) -> Self;

// New
pub fn with_backends_no_git(...) -> Self;
```

### SyncService

```rust
// Existing (unchanged)
pub fn new(config: Config) -> Self;

// New
pub fn no_op() -> Self;
```

## Migration Considerations

### No Migration Needed

- User-scope storage is independent of project-scope
- Existing git notes memories remain unchanged
- No data conversion required

### Future: Cross-Scope Search

Out of scope for this implementation, but architecture supports:
- Query both user and project indices
- Merge results with scope indicator
- Filter by scope preference

## Security Considerations

### User Data Directory Permissions

- Created with user-only permissions (0700 on Unix)
- SQLite database inherits directory permissions
- No sensitive data in memory content (user responsibility)

### No Remote Sync for User Scope

- User memories never leave the local machine
- SyncService disabled for user scope
- Future: Optional cloud backup (separate feature)

## Testing Strategy

### Unit Tests

1. `ServiceContainer::for_user()` creates valid container
2. `ServiceContainer::from_current_dir_or_user()` falls back correctly
3. `CaptureService::with_backends_no_git()` persists without git
4. User-scope capture creates valid URN

### Integration Tests

1. Capture outside git repo succeeds
2. Recall outside git repo returns results
3. MCP tools work outside git repo
4. CLI commands work outside git repo

### Property Tests

1. User-scope memories always have `domain: user`
2. URN always starts with `subcog://user/` for user-scope

## Performance Considerations

### User Scope Performance

| Operation | Target | Notes |
|-----------|--------|-------|
| Capture | < 50ms | No git notes overhead |
| Recall | < 100ms | SQLite + usearch only |
| Cold start | < 100ms | No git repo scanning |

### Comparison with Project Scope

| Operation | Project Scope | User Scope |
|-----------|---------------|------------|
| Capture | ~80ms | ~50ms (no git) |
| Recall | ~80ms | ~80ms (similar) |
| Sync | ~500ms | N/A |

## Rollback Plan

If issues arise:
1. User-scope code is additive (no changes to existing paths)
2. `from_current_dir()` remains unchanged
3. Simply don't call `from_current_dir_or_user()` to revert
4. User-scope memories remain in SQLite (not lost)
