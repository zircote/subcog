# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Docker**: Migrated to scratch base image for minimal container footprint
  - Reduced image size and attack surface
  - Fixed workflow trigger configuration

### Changed

- **Security**: Added security documentation for npm postinstall script ([#74](https://github.com/zircote/subcog/pull/74))
  - Documents binary download verification process
  - Explains SHA256 checksum validation
  - Added `.trivy.yaml` for portable Trivy scanner configuration

### Fixed

- **Plugin**: Updated version to 0.14.0 and corrected npx-based MCP config

## [0.14.0] - 2026-01-21

### Changed

- **Hooks**: Updated all hooks to use npx-based invocation with cache fallback
  - Primary: `npx --prefer-offline @zircote/subcog` (uses cached package, no network)
  - Fallback: `npx -y @zircote/subcog` (downloads if cache miss)
  - Users no longer need `subcog` binary in PATH
  - Works in conjunction with MCP server's `npx -y` which populates the cache

- **Documentation**: Updated all docs to use npx-based MCP server configuration
  - README, QUICKSTART, integration guides now show `npx -y @zircote/subcog serve`
  - Hook documentation updated with npx cache fallback pattern

### Fixed

- **Tests**: Fixed graceful degradation tests that expected no auto-initialized backends
  - `CaptureService::new()` now auto-initializes SQLite from user data dir
  - Tests now use `new_minimal()` which skips auto-initialization

## [0.13.0] - 2026-01-21

### Added

- **Release**: npm package publishing with OIDC Trusted Publishing in release workflow
  - Automated npm publish job triggered after GitHub release
  - Uses `--provenance` flag for supply chain security
  - Version synced from release tag automatically

### Changed

- **MCP Config**: Updated `subcog.mcp.json` to use npm package installation
  - Changed from direct `subcog` binary to `npx -y @zircote/subcog serve`
  - Enables zero-install usage via npm

## [0.12.0] - 2026-01-21

### Added

- **Testing**: Hook-driven automated test framework for MCP tool validation
  - 88 functional tests across 12 categories (initialization, CRUD, search, filters, entities, relationships, graph, prompts, templates, maintenance, privacy, cleanup)
  - State management via `.claude/test-state.json`
  - Test definitions in `tests/functional/tests.yaml`

- **MCP**: URN parsing and support in MCP handlers
  - Enables direct URN-based memory access

### Changed

- **Project Structure**: Moved commands and skills to `.claude` directory
  - Better organization following Claude Code conventions

- **Code Quality**: Code formatting and error handling improvements

## [0.11.0] - 2026-01-20

### Fixed

- **Entity Extraction**: Fixed auto entity extraction not running despite `auto_extract_entities = true` in config
  - `ServiceContainer::for_repo()` and `for_user()` now properly propagate `auto_extract_entities` from loaded `SubcogConfig`
  - Previously, `Config::new()` defaulted to `false`, ignoring user's config file setting
  - Entities are now automatically extracted during memory capture when enabled

- **Entity Extraction Timeout**: Increased default timeout from 30s to 120s for LLM-powered entity extraction
  - Added `entity_extraction_ms` config option under `[timeouts]` section
  - Complex content extraction no longer times out prematurely

- **Graph Service**: Fixed `graph()` method to respect config `data_dir` instead of hardcoding `get_user_data_dir()`
  - Graph database now correctly uses user-configured data directory

### Changed

- **Version**: Bumped to 0.11.0

## [0.10.0] - 2026-01-20

### Added

- **Documentation**: Added `subcog_init` to `prompt_understanding` guidance
  - Documents preferred session initialization approach with example
  - Includes full parameter documentation (`include_recall`, `recall_query`, `recall_limit`)
  - Adds explicit warning about `recall_limit` vs `limit` parameter naming to prevent LLM confusion

### Changed

- **Dependencies**: Updated multiple dependencies
  - parquet: 54.3.1 → 57.2.0
  - arrow: 54.3.1 → 57.2.0
  - chrono: 0.4.42 → 0.4.43
  - thiserror: 2.0.17 → 2.0.18
  - rmcp: 0.12.0 → 0.13.0
  - peter-evans/create-pull-request: 7.0.5 → 8.0.0
  - actions/upload-artifact: 4 → 6

## [0.9.2] - 2026-01-17

### Fixed

- **Documentation**: Removed all legacy git-notes references from user-facing docs (CLAUDE.md, README.md, CHANGELOG.md, CONTRIBUTING.md)

### Changed

- **Build**: Added `.subcog/` to `.gitignore` to prevent accidental commits of local data directories

## [0.9.1] - 2026-01-17

### Fixed

- **Plugin**: Added missing `mcpServers` field in plugin.json to properly register MCP server with Claude Code plugin system

## [0.9.0] - 2026-01-17

### Added

- **Subcog Integrator Skill**: New skill to help users enhance AI prompts with Subcog memory integration
  - Analyzes CLAUDE.md, hooks, skills, and commands for integration gaps
  - Provides recommendations with code snippets for memory protocol sections
  - Interactive workflow via `/subcog:integrate` command
  - Supports analyze, enhance, and create modes
  - Files: `skills/subcog-integrator/SKILL.md`, `commands/integrate.md`

### Documentation

- **ADR Expansion**: Comprehensive expansion of 24 Architecture Decision Records with pedantic detail
  - Added weighted decision drivers, detailed options analysis, implementation code examples
  - Each ADR now includes context, rationale, consequences, and cross-references
  - ADRs covered: 0001-0004, 0005-0009, 0010-0014, 0017-0022, 0025-0033, 0037-0060
- **ADR Migration**: Migrated 39 ADRs to structured-madr format
- **Integration Guides**: Added guides for OpenAI, Gemini, and OpenCode platforms
  - Corrected integration guides with accurate platform info
  - Fixed MCP server command from 'mcp-server' to 'serve'

### Fixed

- **Config**: Fixed `~` home directory expansion in logging file paths
  - Log file paths like `~/.local/share/subcog/logs/subcog.log` now correctly expand to the user's home directory
  - Previously would create a literal `~` folder in the working directory
  - Also applies to `SUBCOG_LOG_FILE` environment variable
  - Updated `example.config.toml` to use `~/.local/share/subcog` as default `data_dir`

- **Documentation**: Corrected multiple integration guide issues
  - Restored hooks config to OpenCode guide
  - Aligned guides with actual `subcog_init` protocol
  - Removed fabricated integration content

### Changed

- **CI/CD**: Added docs deploy workflow with adrscope integration
  - Fixed adrscope theme value to 'auto'
  - Used inline deployment steps instead of composite action

## [0.8.0] - 2026-01-14

### Added

#### MCP Tool Consolidation
- **Consolidated Tools**: Reduced tool count from ~43 to ~22 using action-based patterns
  - `subcog_prompts`: Unified prompt management (actions: `save`, `list`, `get`, `run`, `delete`)
  - `subcog_templates`: Unified context template management (actions: `save`, `list`, `get`, `render`, `delete`)
  - `subcog_graph`: Unified graph operations (operations: `neighbors`, `path`, `stats`, `visualize`)
  - `subcog_groups`: Unified group management (actions: `create`, `list`, `get`, `add_member`, `remove_member`, `update_role`, `delete`) [feature-gated: `group-scope`]
- **Extended Entity Tool**: `subcog_entities` now supports `extract` and `merge` actions
  - `extract`: LLM-powered entity extraction from text (previously `subcog_extract_entities`)
  - `merge`: Deduplicate similar entities (previously `subcog_entity_merge`)
- **Extended Relationship Tool**: `subcog_relationships` now supports `infer` action
  - `infer`: LLM-powered relationship inference (previously `subcog_relationship_infer`)
- **Enhanced Recall Tool**: `subcog_recall` now subsumes `subcog_list`
  - Omit `query` parameter to list all memories with filtering and pagination
  - Added `offset`, `user_id`, `agent_id` parameters for multi-tenant support
  - Different defaults: 10 results for search, 50 for list mode
- **Security**: All consolidated tools use `additionalProperties: false` for parameter validation

### Changed

- **Tool Descriptions**: Updated `prompt_understanding` documentation to reflect consolidated tools
- **API Compatibility**: Legacy tools remain available for backward compatibility

### Deprecated

- `subcog_sync`: SQLite is now authoritative storage; this tool is a no-op
- `subcog_list`: Use `subcog_recall` without `query` parameter instead
- Legacy prompt tools (`prompt_save`, `prompt_list`, `prompt_get`, `prompt_run`, `prompt_delete`): Use `subcog_prompts` instead
- Legacy template tools (`context_template_*`): Use `subcog_templates` instead
- Legacy graph tools (`subcog_graph_query`, `subcog_graph_visualize`): Use `subcog_graph` instead
- Legacy entity/relationship tools (`subcog_extract_entities`, `subcog_entity_merge`, `subcog_relationship_infer`): Use action parameters on `subcog_entities` and `subcog_relationships` instead

#### Group/Shared Memory Graphs (feature-gated: `group-scope`)
- **Group Management**: New group CRUD operations for team collaboration
  - `subcog_group_create`: Create groups with name, description, and automatic admin role
  - `subcog_group_list`: List all groups the user belongs to
  - `subcog_group_get`: Get group details including member list
  - `subcog_group_add_member`: Add members with role (admin/write/read)
  - `subcog_group_remove_member`: Remove members from groups
  - `subcog_group_update_role`: Change member role permissions
  - `subcog_group_delete`: Delete groups (admin only)
- **Role-Based Access Control**: Three-tier permission model
  - `admin`: Full control (manage members, delete group)
  - `write`: Create and edit memories
  - `read`: View-only access
- **SQLite Group Backend**: Dedicated `groups.db` storage with:
  - Groups table with org_id, name, description, timestamps
  - Group members table with email-based identity and roles
  - Proper `ON CONFLICT` handling to preserve `joined_at` timestamps
- **MCP Tool Integration**: Full tool definitions and handlers for all group operations
- **Environment Configuration**: `SUBCOG_USER_ID` and `SUBCOG_ORG_ID` environment variables

## [0.7.0] - 2026-01-13

### Added

#### Release Infrastructure
- **Docker Distribution**: Multi-arch Docker images (amd64/arm64) on ghcr.io
  - Distroless static base image (~15MB compressed)
  - SBOM generation and Trivy vulnerability scanning
  - Automatic build triggered after releases
- **npm Package**: `@zircote/subcog` for Node.js users
  - Binary download with SHA256 checksum verification
  - Automatic platform detection (macOS, Linux, Windows)
  - Fallback to `cargo install` if binary unavailable
  - Run via `npx @zircote/subcog` or install globally
- **Windows Support**: Native Windows x64 builds (.zip format)
- **ARM64 musl**: Static Linux ARM64 binaries for containers
- **Security Workflows**: CodeQL, Trivy, cargo-audit, dependency review
- **Benchmark Workflows**: Criterion benchmarks with regression detection
- **Release Automation**: Version bump, changelog generation, auto-tagging

#### Webhooks/Event Notifications
- New webhook system for real-time notifications when memory events occur
- Configuration via `[[webhooks]]` in `~/.config/subcog/config.toml` with:
  - Multiple webhook endpoints
  - Event type filtering (`captured`, `deleted`, `updated`, `consolidated`, `archived`, `retrieved`, `synced`)
  - Domain scope filtering (`project`, `user`, `org`)
  - Environment variable expansion (`${SECRET_NAME}`)
  - Payload format selection: `default`, `slack` (Block Kit), `discord` (Embeds)
- Authentication options:
  - Bearer token (`Authorization: Bearer <token>`)
  - HMAC-SHA256 signature (`X-Subcog-Signature: sha256=<sig>`)
  - Combined Bearer + HMAC for maximum security
- Exponential backoff retry with configurable delays
- GDPR-compliant SQLite audit logging with export/delete by domain
- New CLI commands:
  - `subcog webhook list` - List configured webhooks
  - `subcog webhook test <name>` - Send test event
  - `subcog webhook history` - View delivery history
  - `subcog webhook stats` - View statistics
  - `subcog webhook export <domain>` - Export audit logs (GDPR Article 20)
  - `subcog webhook delete-logs <domain>` - Delete audit logs (GDPR Article 17)
- New `ServiceContainer::webhook_service()` method for programmatic access
- Prometheus metrics: `webhook_deliveries_total`, `webhook_delivery_duration_ms`, success/failure counters

## [0.6.1] - 2026-01-13

### Fixed

- **User-Scope Memory Capture**: Fixed `subcog_capture` MCP tool to correctly route memories to user-scoped storage when `domain: "user"` is specified. Previously, user-scoped memories were incorrectly stored in project scope (`subcog://project/...` instead of `subcog://user/...`).

### Added

- **Domain Parameter for Capture**: Added `domain` parameter to `subcog_capture` MCP tool schema, allowing explicit storage scope selection:
  - `"project"` (default): Stored with project context
  - `"user"`: Global across all projects
  - `"org"`: Organization-shared storage
- **CLI Domain Flag**: Added `--domain` / `-d` flag to `subcog capture` CLI command with same options as MCP tool

## [0.6.0] - 2026-01-13

### Added

#### Organization-Scoped Storage
- New `org` scope for team collaboration with shared memories across projects
- Org-scoped storage infrastructure with dedicated SQLite databases
- Support for `SUBCOG_ORG_ID` environment variable

#### Memory Consolidation Service
- LLM-powered memory consolidation that groups related memories and creates summaries
- New `subcog consolidate` CLI command with `--namespace`, `--days`, `--dry-run`, `--min-memories`, `--similarity` options
- New `subcog_consolidate` MCP tool for triggering consolidation
- `subcog://summaries` and `subcog://summaries/{id}` MCP resources for browsing summary nodes
- Memory edges table for storing `SummarizedBy` relationships
- Prometheus metrics: `consolidation_operations_total`, timing histograms
- Graceful degradation when LLM unavailable (still detects related memories)
- Support for OpenAI and Ollama LLM providers

#### Knowledge Graph
- Entity-centric memory retrieval via knowledge graph
- Entity extraction from memory content
- Graph-based relationship queries

#### Context Templates
- New Context Templates system for formatting memories in hooks
- `context_template_save`, `context_template_get`, `context_template_list`, `context_template_delete` MCP tools
- `context_template_render` for applying templates to memory sets

#### GC Expiration Module
- Garbage collection for expired memories
- Configurable expiration policies per namespace

#### Session Initialization Enforcement
- Cross-client MCP support with session initialization requirements
- Ensures consistent state across different MCP clients

#### Security & Compliance
- RBAC foundation for role-based access control
- GDPR consent tracking and audit reports
- Request body size limits (1MB) to prevent DoS attacks
- Deserialization size validation before JSON parsing
- XML content escaping in prompt enrichment
- LLM response redaction in parse errors

### Changed

- Extracted generic `Bulkhead<T>` for shared concurrency limiting across storage backends
- Centralized `DEFAULT_DIMENSIONS` constant in `embedding` module
- Made MCP rate limits configurable via `SUBCOG_MCP_RATE_LIMIT_MAX_REQUESTS` and `SUBCOG_MCP_RATE_LIMIT_WINDOW_SECS`
- Reduced deduplication search limit from 10 to 3 for performance
- Added word limit (1000) to pseudo-embedding generation for performance
- Start new trace per MCP request for better observability
- Cache branch lookups during recall for performance

### Fixed

- SessionStart hook now reports correct memory count
- Hook schema compliance for Stop and PreCompact events
- Docker security: explicit USER directives in OTEL Collector and Grafana Dockerfiles
- Database connection retry with exponential backoff
- Preserve LLM HTTP timeouts in fallback provider
- Avoid repo-local storage fallback (architecture fix)
- CI failures in consolidation service and integration tests
- Vector search integration test backend sharing
- Rustdoc warning for unclosed HTML tag in events.rs
- Pre-allocated HashMap in RRF fusion for performance

### Removed

- Removed placeholder functions (`add`, `divide`, `Config`) from lib.rs
- Removed sync tool from MCP and CLI (SQLite is now authoritative)

## [0.2.0] - 2026-01-02

### Added

#### Real Semantic Embeddings (MEM-001)
- Replaced placeholder hash-based embeddings with real semantic embeddings via fastembed-rs
- Uses all-MiniLM-L6-v2 model (384 dimensions)
- Thread-safe singleton for model loading with lazy initialization
- Model loads on first embed() call to preserve cold start time

#### RecallService Vector Search (MEM-002)
- Added embedder and vector backend fields to RecallService
- Implemented real `vector_search()` with query embedding
- Hybrid search now uses both text (BM25) and vector results
- Graceful degradation when embedder/vector unavailable

#### CaptureService Integration (MEM-003)
- CaptureService now generates embeddings during capture
- Writes to all storage layers: SQLite FTS5, usearch HNSW
- Non-blocking index/vector operations (capture succeeds even if they fail)

#### Score Normalization (MEM-005)
- All search results now return normalized scores in 0.0-1.0 range
- `--raw` flag in CLI to display original RRF scores
- MCP tools return both normalized `score` and `raw_score` fields
- Score proportions preserved (relative ordering unchanged)

#### Migration Tooling
- New `subcog migrate embeddings` command
- Options: `--dry-run`, `--force`, `--repo`
- MigrationService with progress tracking
- Scans all memories, generates embeddings for those lacking them

#### Performance Benchmarks
- New benchmark suite in `benches/search.rs`
- Benchmarks for 100, 1,000, and 10,000 memories
- All search modes tested (text, vector, hybrid)
- Results far exceed targets:
  - 100 memories: ~82µs (target <20ms)
  - 1,000 memories: ~413µs (target <50ms)
  - 10,000 memories: ~3.7ms (target <100ms)

### Changed

- ServiceContainer now supports `with_embedder()` and `with_vector()` builders
- RecallService constructor signature updated to accept embedder/vector
- CaptureService constructor signature updated to accept all three backends

### Fixed

- RecallService no longer returns empty results when embedder unavailable
- Hybrid search properly combines text and vector results with RRF fusion
- Score normalization handles edge cases (empty results, zero scores)

### Performance

- Search latency: <5ms at 10,000 memories (target was <100ms)
- Capture latency: ~25ms with embedding generation
- Cold start: ~5ms (target was <10ms)
- Binary size: ~50MB (target was <100MB)

## [0.1.0] - 2025-12-28

### Added

- Initial release of Subcog (Rust rewrite)
- Two-layer storage: SQLite FTS5, usearch HNSW
- MCP server integration
- Claude Code hooks (all 5 hooks)
- 10 memory namespaces
- Multi-domain support (project, user, organization)
- Proactive memory surfacing with search intent detection
- Prompt template management
- Deduplication service

[Unreleased]: https://github.com/zircote/subcog/compare/v0.14.0...HEAD
[0.14.0]: https://github.com/zircote/subcog/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/zircote/subcog/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/zircote/subcog/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/zircote/subcog/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/zircote/subcog/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/zircote/subcog/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/zircote/subcog/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/zircote/subcog/compare/v0.6.1...v0.7.0
[0.6.1]: https://github.com/zircote/subcog/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/zircote/subcog/compare/v0.2.0...v0.6.0
[0.2.0]: https://github.com/zircote/subcog/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/zircote/subcog/releases/tag/v0.1.0
