# Research Plan: Feature Documentation vs Implementation Audit

## Research Type
**CODEBASE** - Comprehensive audit of documented features vs actual implementation

## Research Objective
Produce an exhaustive inventory of all documented features in Subcog, verify their implementation status, and identify deficiencies for remediation.

## Success Criteria
1. Every documented feature is catalogued with doc reference
2. Every feature is verified against source code
3. Implementation status classified as: **IMPLEMENTED**, **STUB**, **PARTIAL**, **MISSING**
4. Stubs are NOT counted as implementations
5. Deficiencies list produced for future work

## Methodology

### Phase 1: Documentation Inventory
Read all docs/ files and extract:
- Feature name
- Feature description/requirements
- Expected behavior
- Source file reference (if mentioned)

### Phase 2: Implementation Verification
For each documented feature:
- Locate implementing code in src/
- Use LSP for symbol navigation
- Read implementation fully
- Verify logic is genuine (not stub/placeholder)

### Phase 3: Classification
- **IMPLEMENTED**: Genuine, working code matching spec
- **STUB**: Function exists but returns placeholder/todo/unimplemented
- **PARTIAL**: Some functionality implemented, gaps remain
- **MISSING**: No corresponding implementation found

### Phase 4: Deficiency Report
- List all STUB, PARTIAL, MISSING items
- Reference doc requirements
- Suggest remediation priority

## Documentation Categories to Analyze

### Core Documentation
- [ ] docs/README.md - Project overview
- [ ] docs/QUICKSTART.md - Getting started
- [ ] docs/BENCHMARKS.md - Performance targets
- [ ] docs/QUERY_SYNTAX.md - Query language
- [ ] docs/URN-GUIDE.md - URN scheme

### CLI Documentation (docs/cli/)
- [ ] README.md - CLI overview
- [ ] capture.md - Capture command
- [ ] recall.md - Recall command
- [ ] status.md - Status command
- [ ] sync.md - Sync command
- [ ] consolidate.md - Consolidate command
- [ ] namespaces.md - Namespace management
- [ ] config.md - Configuration command
- [ ] serve.md - MCP server command
- [ ] hook.md - Hook command
- [ ] prompt.md - Prompt command

### Hooks Documentation (docs/hooks/)
- [ ] README.md - Hooks overview
- [ ] session-start.md - SessionStart hook
- [ ] user-prompt-submit.md - UserPromptSubmit hook
- [ ] post-tool-use.md - PostToolUse hook
- [ ] pre-compact.md - PreCompact hook
- [ ] stop.md - Stop hook
- [ ] search-intent.md - Search intent detection

### MCP Documentation (docs/mcp/)
- [ ] README.md - MCP overview
- [ ] tools.md - MCP tools
- [ ] resources.md - MCP resources
- [ ] protocol.md - Protocol details
- [ ] prompts.md - MCP prompts

### Storage Documentation (docs/storage/)
- [ ] README.md - Storage overview
- [ ] persistence.md - Persistence layer
- [ ] index.md - Index layer
- [ ] vector.md - Vector layer
- [ ] domains.md - Domain scoping

### Architecture Documentation (docs/architecture/)
- [ ] README.md - Architecture overview
- [ ] overview.md - System overview
- [ ] services.md - Services layer
- [ ] models.md - Data models
- [ ] search.md - Search architecture
- [ ] security.md - Security design

### Configuration Documentation (docs/configuration/)
- [ ] README.md - Configuration overview
- [ ] config-file.md - Config file format
- [ ] environment.md - Environment variables
- [ ] locations.md - Config locations
- [ ] features.md - Feature flags

### Prompts Documentation (docs/prompts/)
- [ ] README.md - Prompts overview
- [ ] overview.md - Prompts system
- [ ] formats.md - Prompt formats
- [ ] storage.md - Prompt storage
- [ ] variables.md - Variable substitution
- [ ] SYSTEM_PROMPTS.md - System prompts
- [ ] mcp.md - MCP prompt tools

### Specifications (docs/spec/)
- [ ] Active spec: 2025-12-28-subcog-rust-rewrite
- [ ] Completed: 2025-12-30-issue-15-memory-surfacing
- [ ] Completed: 2025-12-30-prompt-management

## Source Code to Verify

### CLI (src/cli/)
- capture.rs, recall.rs, status.rs, sync.rs
- consolidate.rs, config.rs, serve.rs
- hook.rs, prompt.rs, mod.rs

### Hooks (src/hooks/)
- session_start.rs, user_prompt.rs
- post_tool_use.rs, pre_compact.rs, stop.rs
- search_intent.rs, search_context.rs, mod.rs

### MCP (src/mcp/)
- server.rs, tools.rs, resources.rs
- prompts.rs, mod.rs

### Services (src/services/)
- capture.rs, recall.rs, sync.rs
- consolidation.rs, context.rs
- topic_index.rs, prompt.rs
- prompt_parser.rs, enrichment.rs
- query_parser.rs, mod.rs

### Storage (src/storage/)
- persistence/: git_notes.rs, postgresql.rs, filesystem.rs
- index/: sqlite.rs, postgresql.rs, redis.rs, domain.rs
- vector/: usearch.rs, pgvector.rs, redis.rs
- prompt/: filesystem.rs, sqlite.rs, git_notes.rs, postgresql.rs, redis.rs

### LLM (src/llm/)
- anthropic.rs, openai.rs, ollama.rs, lmstudio.rs
- system_prompt.rs, resilience.rs, mod.rs

### Security (src/security/)
- secrets.rs, pii.rs, redactor.rs, audit.rs, mod.rs

### Embedding (src/embedding/)
- fastembed.rs, fallback.rs, mod.rs

### Observability (src/observability/)
- metrics.rs, tracing.rs, logging.rs, otlp.rs, mod.rs

### Models (src/models/)
- memory.rs, capture.rs, search.rs
- consolidation.rs, events.rs
- domain.rs, prompt.rs, mod.rs

### Git (src/git/)
- notes.rs, parser.rs, remote.rs, mod.rs

### Config (src/config/)
- mod.rs, features.rs

## Deliverable
`docs/FEATURES_REPORT.md` containing:
1. Complete feature inventory with doc references
2. Implementation status for each feature
3. Deficiency list for remediation
