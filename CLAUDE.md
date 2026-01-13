# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Subcog is a persistent memory system for AI coding assistants, written in Rust. It captures decisions, learnings, and context from coding sessions and surfaces them when relevant. This is a Rust rewrite of the [git-notes-memory](https://github.com/zircote/git-notes-memory) Python system.

### Key Features

- **Single-binary distribution** (<100MB, <10ms cold start)
- **Three-layer storage architecture** (Persistence, Index, Vector)
- **Pluggable backends** (`SQLite`+usearch, PostgreSQL+pgvector)
- **MCP server integration** for AI agent interoperability
- **Claude Code hooks** for seamless IDE integration
- **Semantic search** with hybrid vector + BM25 ranking (RRF fusion)

## Project Structure

```
src/
├── lib.rs                    # Library entry point
├── main.rs                   # CLI entry point
│
├── models/                   # Data structures
│   ├── mod.rs
│   ├── memory.rs            # Memory, MemoryId, MemoryResult
│   ├── capture.rs           # CaptureResult, CaptureRequest
│   ├── search.rs            # SearchResult, SearchFilter, SearchMode
│   ├── consolidation.rs     # MemoryTier, EdgeType, RetentionScore
│   ├── domain.rs            # Domain, Namespace (11 variants), MemoryStatus
│   ├── events.rs            # MemoryEvent variants
│   ├── prompt.rs            # PromptTemplate, PromptVariable, validation
│   └── graph.rs             # Entity, Relationship, EntityType, GraphStats
│
├── storage/                  # Three-layer storage abstraction
│   ├── mod.rs               # CompositeStorage, layer trait re-exports
│   ├── traits/
│   │   ├── persistence.rs   # PersistenceBackend trait
│   │   ├── index.rs         # IndexBackend trait
│   │   └── vector.rs        # VectorBackend trait
│   ├── persistence/
│   │   ├── sqlite.rs        # SQLite implementation (primary)
│   │   ├── postgresql.rs    # PostgreSQL implementation
│   │   └── filesystem.rs    # Fallback filesystem storage
│   ├── index/
│   │   ├── sqlite.rs        # SQLite + FTS5 implementation
│   │   ├── postgresql.rs    # PostgreSQL full-text
│   │   └── redis.rs         # RediSearch implementation
│   ├── vector/
│   │   ├── usearch.rs       # usearch HNSW implementation
│   │   ├── pgvector.rs      # pgvector implementation
│   │   └── redis.rs         # Redis vector search
│   └── graph/               # Knowledge graph storage
│       ├── mod.rs           # GraphBackend trait
│       └── sqlite.rs        # SQLite graph implementation
│
├── services/                 # Business logic
│   ├── mod.rs               # ServiceContainer
│   ├── capture.rs           # CaptureService
│   ├── recall.rs            # RecallService (search)
│   ├── sync.rs              # SyncService
│   ├── consolidation.rs     # ConsolidationService
│   ├── context.rs           # ContextBuilderService
│   ├── topic_index.rs       # TopicIndexService (topic → memory map)
│   ├── prompt.rs            # PromptService (CRUD for prompts)
│   ├── prompt_parser.rs     # Multi-format parsing (MD, YAML, JSON)
│   ├── prompt_enrichment.rs # LLM-assisted metadata enrichment
│   ├── graph.rs             # GraphService (entity storage, traversal)
│   ├── entity_extraction.rs # EntityExtractorService (LLM + fallback)
│   └── deduplication/       # Deduplication service
│       ├── mod.rs           # Module exports, Deduplicator trait
│       ├── types.rs         # DuplicateCheckResult, DuplicateReason
│       ├── config.rs        # DeduplicationConfig with env loading
│       ├── hasher.rs        # ContentHasher (SHA256 + normalization)
│       ├── exact_match.rs   # ExactMatchChecker (hash tag lookup)
│       ├── semantic.rs      # SemanticSimilarityChecker (embeddings)
│       ├── recent.rs        # RecentCaptureChecker (LRU + TTL)
│       └── service.rs       # DeduplicationService orchestrator
│
├── git/                      # Git operations
│   ├── remote.rs            # Git context detection (branch, remote, repo root)
│   └── parser.rs            # YAML front matter parsing
│
├── embedding/                # Embedding generation
│   ├── mod.rs               # Embedder trait
│   ├── fastembed.rs         # FastEmbed implementation
│   └── fallback.rs          # Fallback to BM25-only
│
├── llm/                      # LLM client abstraction
│   ├── mod.rs               # LLMProvider trait
│   ├── anthropic.rs         # Anthropic Claude implementation
│   ├── openai.rs            # OpenAI implementation
│   ├── ollama.rs            # Ollama (local) implementation
│   └── lmstudio.rs          # LM Studio implementation
│
├── hooks/                    # Claude Code hooks
│   ├── mod.rs               # HookHandler trait
│   ├── session_start.rs     # Context injection
│   ├── user_prompt.rs       # Signal detection + search intent
│   ├── search_intent.rs     # Search intent detection (6 types)
│   ├── search_context.rs    # Adaptive context building
│   ├── post_tool_use.rs     # Related memory surfacing
│   ├── pre_compact.rs       # Auto-capture before compaction
│   └── stop.rs              # Session analysis, sync
│
├── mcp/                      # MCP server
│   ├── server.rs            # MCP server setup (rmcp)
│   ├── tools.rs             # Tool implementations
│   ├── resources.rs         # Resource handlers (URN scheme)
│   └── prompts.rs           # Pre-defined prompts
│
├── security/                 # Security features
│   ├── secrets.rs           # Secret detection patterns
│   ├── pii.rs               # PII detection
│   ├── redactor.rs          # Content redaction/masking
│   └── audit.rs             # SOC2/GDPR audit logging
│
├── config/                   # Configuration
│   ├── mod.rs               # Config struct, loading
│   └── features.rs          # FeatureFlags
│
├── cli/                      # CLI commands
│   ├── capture.rs           # capture subcommand
│   ├── recall.rs            # recall subcommand
│   ├── status.rs            # status subcommand
│   ├── sync.rs              # sync subcommand
│   ├── consolidate.rs       # consolidate subcommand
│   ├── config.rs            # config subcommand
│   ├── serve.rs             # serve subcommand (MCP)
│   ├── hook.rs              # hook subcommand
│   └── prompt.rs            # prompt subcommand (save, list, get, run, delete, export)
│
├── commands/                 # Command implementations
│   ├── mod.rs               # Command re-exports
│   ├── core.rs              # Core commands (capture, recall, status)
│   ├── graph.rs             # Graph commands (entities, relationships, stats, get)
│   ├── prompt.rs            # Prompt management
│   └── hook.rs              # Hook event handlers
│
└── observability/            # Telemetry
    ├── metrics.rs           # Prometheus metrics
    ├── tracing.rs           # Distributed tracing
    ├── logging.rs           # Structured logging
    └── otlp.rs              # OTLP export

tests/
└── integration_test.rs      # Integration tests

benches/
└── search_intent.rs         # Performance benchmarks

docs/spec/active/2025-12-28-subcog-rust-rewrite/
├── README.md                # Spec overview
├── REQUIREMENTS.md          # Product requirements
├── ARCHITECTURE.md          # Technical architecture
├── IMPLEMENTATION_PLAN.md   # Phased implementation
├── DECISIONS.md             # Architecture decision records
└── PROGRESS.md              # Implementation progress
```

## Build Commands

This project uses [Cargo](https://doc.rust-lang.org/cargo/) as the build system.

```bash
# Build the project
cargo build

# Build with optimizations
cargo build --release

# Run tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run benchmarks
cargo bench

# Run linting
cargo clippy --all-targets --all-features

# Format code
cargo fmt

# Check formatting
cargo fmt -- --check

# Generate documentation
cargo doc --open

# Check supply chain security
cargo deny check

# Run with MIRI (undefined behavior detection)
cargo +nightly miri test

# Run all checks (lint + format + test + doc + deny)
cargo fmt -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test && cargo doc --no-deps && cargo deny check
```

## CLI Usage

```bash
# Capture a memory
subcog capture --namespace decisions "Use PostgreSQL for primary storage"

# Search memories
subcog recall "database storage decision"

# Check status
subcog status

# Sync with git remote
subcog sync

# Consolidate related memories
subcog consolidate --namespace decisions --days 30 --dry-run

# Run as MCP server
subcog serve

# Hook commands (called by Claude Code)
subcog hook session-start
subcog hook user-prompt-submit
subcog hook post-tool-use
subcog hook pre-compact
subcog hook stop

# Prompt management commands
subcog prompt save my-prompt --content "Review {{file}} for {{issue_type}}"
subcog prompt save code-review --file prompts/code-review.md --domain user
subcog prompt list --domain project
subcog prompt get my-prompt
subcog prompt run code-review --var file=src/main.rs --var issue_type=security
subcog prompt delete my-prompt --domain project
subcog prompt export my-prompt --format yaml --output my-prompt.yaml

# Knowledge graph commands
subcog graph entities                    # List all entities
subcog graph entities --query "Rust"     # Search entities by name
subcog graph entities --type technology  # Filter by type
subcog graph relationships Alice         # Show relationships for entity
subcog graph relationships Alice --depth 2  # Traverse 2 levels deep
subcog graph stats                       # Show graph statistics
subcog graph get Alice                   # Get entity details
```

### Prompt Templates

Subcog supports user-defined prompt templates with variable substitution. Templates can be stored at project, user, or org scope and shared across sessions.

**Variable Syntax**: `{{variable_name}}`
- Valid names: alphanumeric and underscores only
- Reserved prefixes: `subcog_`, `system_`, `__`

**Example Template** (YAML format):
```yaml
name: code-review
description: Comprehensive code review
content: |
  Review {{file}} for:
  - {{issue_type}} issues
  - Best practices
  - Edge cases
variables:
  - name: file
    description: File path to review
    required: true
  - name: issue_type
    description: Type of issues to focus on
    default: general
tags: [review, quality]
```

**MCP Tools**:
| Tool | Description |
|------|-------------|
| `prompt_save` | Save a new prompt template (with optional LLM enrichment) |
| `prompt_list` | List prompts with optional filtering |
| `prompt_get` | Get a specific prompt by name |
| `prompt_run` | Execute a prompt with variable substitution |
| `prompt_delete` | Delete a prompt |

### Context-Aware Variable Extraction

Variable extraction is context-aware and skips `{{variable}}` patterns inside fenced code blocks:

```markdown
This prompt uses {{active_variable}} which will be extracted.

```python
# This is a code example showing syntax
template = "Hello {{code_example_variable}}"
```

The {{another_active_variable}} after the code block is also extracted.
```

In the above example, only `active_variable` and `another_active_variable` are extracted as template variables. The `code_example_variable` inside the fenced code block is treated as literal documentation.

**Supported Code Block Syntaxes**:
- Triple backticks: ` ```language ... ``` `
- Triple tildes: `~~~ language ... ~~~`
- Nested code blocks (backticks within tildes)

### LLM-Assisted Metadata Enrichment

When saving prompts, Subcog can automatically generate or enhance metadata using an LLM:

**CLI Usage**:
```bash
# Default: LLM enrichment enabled (if provider configured)
subcog prompt save my-prompt --content "Review {{file}} for issues"

# Skip enrichment
subcog prompt save my-prompt --content "..." --no-enrich

# Preview enrichment without saving
subcog prompt save my-prompt --content "..." --dry-run
```

**MCP Tool**:
```json
{
  "name": "subcog_prompt_save",
  "arguments": {
    "name": "my-prompt",
    "content": "Review {{file}} for issues",
    "skip_enrichment": false
  }
}
```

**Enrichment Behavior**:
| Status | Description |
|--------|-------------|
| `Full` | LLM successfully generated/enhanced metadata |
| `Fallback` | LLM unavailable; used extracted variables only |
| `Skipped` | Enrichment explicitly disabled via `--no-enrich` |

**What Gets Enriched**:
- **Description**: Generated if missing, based on prompt content
- **Tags**: Inferred from content (e.g., "security", "review", "debugging")
- **Variables**: Descriptions and defaults for extracted variables

**User Values Preserved**: Explicitly provided metadata (description, tags, variables) is preserved and merged with LLM suggestions.

## Proactive Memory Surfacing

The proactive memory surfacing system automatically detects search intent in user prompts and injects relevant memories into the context. This enables the AI assistant to leverage prior decisions, patterns, and learnings without explicit recall commands.

### Search Intent Detection

When a user prompt is processed, the system detects one of six intent types:

| Intent Type | Trigger Patterns | Example |
|-------------|-----------------|---------|
| **HowTo** | "how do I...", "how to...", "implement...", "create..." | "How do I implement authentication?" |
| **Location** | "where is...", "find...", "locate..." | "Where is the database config?" |
| **Explanation** | "what is...", "explain...", "describe..." | "What is the ServiceContainer?" |
| **Comparison** | "difference between...", "vs", "compare..." | "PostgreSQL vs SQLite?" |
| **Troubleshoot** | "error...", "fix...", "not working...", "debug..." | "Why is this test failing?" |
| **General** | "search...", "show me..." | "Search for recent decisions" |

Detection uses a hybrid approach:
1. **Keyword detection**: Fast pattern matching (<10ms)
2. **LLM classification**: Enhanced accuracy with 200ms timeout
3. **Hybrid mode**: Combines both for best results

### Adaptive Memory Injection

Based on detected intent confidence:

| Confidence | Memory Count | Behavior |
|------------|--------------|----------|
| ≥ 0.8 (high) | 15 memories | Full context injection |
| ≥ 0.5 (medium) | 10 memories | Standard injection |
| < 0.5 (low) | 5 memories | Minimal injection |

Namespace weights are applied based on intent type:
- **HowTo**: Prioritizes `patterns`, `learnings`
- **Troubleshoot**: Prioritizes `blockers`, `learnings`
- **Explanation/Location**: Prioritizes `decisions`, `context`
- **Comparison**: Prioritizes `decisions`, `patterns`

### MCP Resources

New MCP resources for topic-based access:

```
subcog://topics              # List all topics with memory counts
subcog://topics/{topic}      # Get memories for a specific topic
subcog://namespaces          # List all namespaces
subcog://namespaces/{ns}     # Get memories in a namespace
```

### MCP Prompts

New prompts for context-aware operations:

| Prompt | Description |
|--------|-------------|
| `search_with_context` | Search with intent-aware weighting |
| `research_topic` | Deep-dive into a topic with related memories |
| `capture_decision` | Capture with guided namespace selection |

### Configuration

Environment variables for search intent:

```bash
SUBCOG_SEARCH_INTENT_ENABLED=true       # Enable/disable detection
SUBCOG_SEARCH_INTENT_USE_LLM=true       # Enable LLM classification
SUBCOG_SEARCH_INTENT_LLM_TIMEOUT_MS=200 # LLM timeout in milliseconds
SUBCOG_SEARCH_INTENT_MIN_CONFIDENCE=0.5 # Minimum confidence threshold
```

Namespace weights (config file only):

```toml
[search_intent]
base_count = 5      # Memories for low-confidence matches
max_count = 15      # Memories for high-confidence matches
max_tokens = 4000   # Token budget for context

# Per-intent namespace weight multipliers (default: 1.0)
[search_intent.weights.troubleshoot]
blockers = 2.0      # Boost blockers for debugging queries
learnings = 1.5
tech-debt = 1.2

[search_intent.weights.howto]
patterns = 2.0      # Boost patterns for how-to queries
learnings = 1.5
```

### Graceful Degradation

The system degrades gracefully when components are unavailable:

- **LLM unavailable**: Falls back to keyword-only detection
- **Embeddings down**: Falls back to text search (BM25)
- **Index down**: Skips memory injection, continues processing
- **Low confidence**: Reduces memory count, may skip injection

## Deduplication Service

The deduplication service prevents duplicate memory captures in the pre-compact hook using a three-tier detection system with short-circuit evaluation.

### Detection Tiers

| Tier | Method | Performance | Use Case |
|------|--------|-------------|----------|
| **1. Exact Match** | SHA256 hash lookup via tag | <5ms | Identical content |
| **2. Semantic Similarity** | Embedding cosine similarity | <50ms | Paraphrased content |
| **3. Recent Capture** | LRU cache with TTL | <1ms | Same-session duplicates |

### Short-Circuit Evaluation

Checks are performed in order; the first match returns immediately:

```
ExactMatch → SemanticSimilarity → RecentCapture → Not Duplicate
    ↓               ↓                   ↓
  (match)        (match)            (match)
    ↓               ↓                   ↓
  SKIP            SKIP               SKIP
```

### Content Normalization

Before hashing or embedding, content is normalized:
- Trim leading/trailing whitespace
- Convert to lowercase
- Collapse multiple whitespace to single space

```rust
// These produce the same hash:
"Use PostgreSQL for storage"
"  use   postgresql   for   storage  "
"USE POSTGRESQL FOR STORAGE"
```

### Per-Namespace Thresholds

Semantic similarity thresholds can be configured per namespace:

| Namespace | Default Threshold | Rationale |
|-----------|-------------------|-----------|
| `decisions` | 92% | High precision for architectural choices |
| `patterns` | 90% | Standard threshold |
| `learnings` | 88% | Allow more variation in insights |
| `blockers` | 90% | Standard threshold |
| Default | 90% | Applied to all other namespaces |

### Usage

The `PreCompactHandler` integrates with the deduplication service:

```rust
use subcog::services::{DeduplicationService, ServiceContainer};

// Create service container with deduplication
let container = ServiceContainer::new(config)?;
let dedup = container.deduplication()?;

// Create handler with deduplication
let handler = PreCompactHandler::new(capture_service, recall_service)
    .with_deduplication(dedup);
```

### Configuration

| Environment Variable | Description | Default |
|---------------------|-------------|---------|
| `SUBCOG_DEDUP_ENABLED` | Enable deduplication | `true` |
| `SUBCOG_DEDUP_DEFAULT_THRESHOLD` | Default similarity threshold | `0.90` |
| `SUBCOG_DEDUP_DECISIONS_THRESHOLD` | Decisions namespace threshold | `0.92` |
| `SUBCOG_DEDUP_PATTERNS_THRESHOLD` | Patterns namespace threshold | `0.90` |
| `SUBCOG_DEDUP_LEARNINGS_THRESHOLD` | Learnings namespace threshold | `0.88` |
| `SUBCOG_DEDUP_RECENT_TTL_SECONDS` | Recent capture TTL | `300` |
| `SUBCOG_DEDUP_RECENT_CACHE_SIZE` | Recent cache size | `1000` |
| `SUBCOG_DEDUP_MIN_SEMANTIC_LENGTH` | Min length for semantic check | `50` |

### Hook Response Format

When duplicates are skipped, they appear in the hook response:

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreCompact",
    "additionalContext": "**Subcog Pre-Compact Auto-Capture**\n\nCaptured 1 memory...\n\nSkipped 2 duplicates:\n- `decisions`: subcog://global/decisions/abc123 (exact_match)\n- `learnings`: subcog://global/learnings/def456 (semantic_similar, 95% similar)"
  }
}
```

### Graceful Degradation

The service degrades gracefully when components fail:

- **Embedding service down**: Skips semantic check, uses exact + recent only
- **RecallService error**: Skips exact match check, logs warning
- **Any checker fails**: Logs warning, continues to next tier
- **All checks pass/fail**: Capture proceeds

### Metrics

| Metric | Description |
|--------|-------------|
| `deduplication_duplicates_found_total` | Total duplicates detected |
| `deduplication_not_duplicates_total` | Total unique captures |
| `deduplication_check_duration_ms` | Check latency histogram |
| `deduplication_recent_cache_size` | Current cache size |
| `hook_deduplication_skipped_total` | Skipped by hook (labels: namespace, reason) |

## Memory Consolidation Service

The consolidation service intelligently summarizes related memories using LLM-powered analysis while preserving original memories and creating bidirectional edge relationships. This prevents memory accumulation without losing historical context.

### How It Works

Consolidation creates **summary nodes** that aggregate related memories while preserving originals:

1. **Semantic Clustering**: Groups memories by namespace using configurable similarity threshold (default: 0.7)
2. **LLM Summarization**: Generates coherent summaries preserving key details from each memory
3. **Summary Node Creation**: Creates new memories marked with `is_summary=true` and `source_memory_ids` linking to originals
4. **Edge Relationships**: Stores bidirectional edges (`SummarizedBy` / `SourceOf`) for traversal
5. **Graceful Degradation**: Creates `RelatedTo` edges when LLM unavailable

### CLI Usage

```bash
# Basic consolidation (uses config file settings)
subcog consolidate

# Filter by specific namespaces
subcog consolidate --namespace decisions --namespace patterns

# Set time window to last 7 days
subcog consolidate --days 7

# Preview what would be consolidated (dry-run mode)
subcog consolidate --dry-run

# Override similarity threshold (0.0-1.0)
subcog consolidate --similarity 0.85

# Set minimum memories per group
subcog consolidate --min-memories 5

# Combine multiple options
subcog consolidate --namespace learnings --days 14 --similarity 0.9 --dry-run
```

**Dry-Run Output Example**:
```
Finding related memory groups...

Found 3 group(s) across 2 namespace(s)
  Total memories to consolidate: 12

  Decisions: 2 group(s), 8 memories
  Patterns: 1 group(s), 4 memories

Would create 3 summary node(s)
Would consolidate 12 memory/memories

Run without --dry-run to apply changes
```

**Normal Run Output Example**:
```
Finding related memory groups...

Found 3 group(s) across 2 namespace(s)
  Total memories to consolidate: 12

Creating summaries...

Consolidation completed:
  Processed 12 memories
  ✓ Created 3 summary node(s)
  ✓ Linked 12 source memories via edges
```

### Configuration

#### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_CONSOLIDATION_ENABLED` | Enable consolidation service | `false` |
| `SUBCOG_CONSOLIDATION_TIME_WINDOW_DAYS` | Time window in days for memories | `30` |
| `SUBCOG_CONSOLIDATION_MIN_MEMORIES` | Minimum memories to form a group | `3` |
| `SUBCOG_CONSOLIDATION_SIMILARITY_THRESHOLD` | Similarity threshold (0.0-1.0) | `0.7` |
| `SUBCOG_CONSOLIDATION_NAMESPACE_FILTER` | Comma-separated namespaces | None (all) |

#### Config File

```toml
[consolidation]
enabled = true
time_window_days = 30
min_memories_to_consolidate = 3
similarity_threshold = 0.7
namespace_filter = ["decisions", "patterns", "learnings"]
```

### MCP Tools

#### `subcog_consolidate`

Triggers consolidation with optional filters. Returns consolidation statistics.

**Arguments**:
- `namespaces` (optional, array): Filter to specific namespaces (e.g., `["decisions", "patterns"]`)
- `days` (optional, number): Time window in days for memories to consolidate
- `min_memories` (optional, number): Minimum memories per group (≥2)
- `similarity` (optional, number): Similarity threshold 0.0-1.0
- `dry_run` (optional, boolean): Preview mode without making changes

**Example**:
```json
{
  "name": "subcog_consolidate",
  "arguments": {
    "namespaces": ["decisions"],
    "days": 7,
    "similarity": 0.85,
    "dry_run": true
  }
}
```

**Dry-Run Response**:
```markdown
**Memory Consolidation (Dry Run)**

Found 2 group(s) across 1 namespace(s):
- Decisions: 2 group(s), 8 memories

Would create 2 summary node(s)
Would consolidate 8 memory/memories

Run without dry_run to apply changes
```

**Normal Run Response**:
```markdown
**Memory Consolidation Completed**

Processed 8 memories
✓ Created 2 summary node(s)
✓ Linked 8 source memories via edges
```

#### `subcog_get_summary`

Retrieves a summary node and its linked source memories via edge relationships.

**Arguments**:
- `memory_id` (required, string): The ID of the summary memory to retrieve

**Example**:
```json
{
  "name": "subcog_get_summary",
  "arguments": {
    "memory_id": "summary_abc123"
  }
}
```

**Response**:
```markdown
**Summary: summary_abc123**

Namespace: decisions
Tags: redis, session-storage, architecture
Consolidated: 2026-01-11T15:00:00Z

Content:
The team decided to use Redis for session storage with AOF persistence...

**Source Memories (3):**

1. decisions | [redis, storage] | Use Redis for session storage because... (150 chars)
   subcog://user/project1/main/decisions/mem_001

2. decisions | [redis, persistence] | Configure Redis with AOF persistence... (150 chars)
   subcog://user/project1/main/decisions/mem_002

3. decisions | [redis, eviction] | Set Redis eviction policy to allkeys-lru... (150 chars)
   subcog://user/project1/main/decisions/mem_003
```

### MCP Resources

#### `subcog://summaries`

Lists all summary nodes across all namespaces.

**Response Format**:
```json
{
  "uri": "subcog://summaries",
  "mimeType": "application/json",
  "contents": [
    {
      "id": "summary_abc123",
      "namespace": "decisions",
      "tags": ["redis", "session-storage"],
      "content_preview": "The team decided to use Redis...",
      "source_count": 3,
      "consolidation_timestamp": 1704988800,
      "uri": "subcog://summaries/summary_abc123"
    }
  ]
}
```

#### `subcog://summaries/{id}`

Retrieves a specific summary node with full source memory details.

**Example**: `subcog://summaries/summary_abc123`

**Response Format**:
```json
{
  "uri": "subcog://summaries/summary_abc123",
  "mimeType": "application/json",
  "summary": {
    "id": "summary_abc123",
    "namespace": "decisions",
    "domain": "user",
    "content": "Full summary content...",
    "tags": ["redis", "session-storage"],
    "consolidation_timestamp": 1704988800,
    "source_count": 3,
    "source_memories": [
      {
        "id": "mem_001",
        "namespace": "decisions",
        "tags": ["redis"],
        "content_preview": "Use Redis for session storage...",
        "uri": "subcog://user/project1/main/decisions/mem_001"
      }
    ]
  }
}
```

### Edge Relationships

Consolidation creates bidirectional edge relationships stored in the `memory_edges` table:

| Edge Type | Direction | Description |
|-----------|-----------|-------------|
| `SummarizedBy` | Original → Summary | Links source memory to its summary node |
| `SourceOf` | Summary → Original | Links summary node to its source memories |
| `RelatedTo` | Memory ↔ Memory | Links semantically similar memories (LLM fallback) |

**Query edges via index backend**:
```rust
use subcog::models::EdgeType;

// Get all summaries containing this memory
let edges = index.query_edges(memory_id, EdgeType::SummarizedBy)?;

// Get all source memories of a summary
let sources = index.query_edges(summary_id, EdgeType::SourceOf)?;
```

### Graceful Degradation

The consolidation service degrades gracefully when components are unavailable:

| Component | Behavior |
|-----------|----------|
| **LLM unavailable** | Creates `RelatedTo` edges between similar memories without summarization |
| **Index backend unavailable** | Skips edge storage, continues with summary creation |
| **Embeddings unavailable** | Cannot group by similarity, returns empty groups |
| **LLM fails mid-run** | Logs warning, skips failed group, continues with remaining groups |

**Circuit Breaker Pattern**: When using `ResilientLlmProvider`:
- **Automatic retries**: 3 attempts with exponential backoff (100ms, 200ms, 400ms)
- **Circuit breaker**: Opens after 5 consecutive failures
- **Transient error detection**: Retries on timeouts, 5xx errors, rate limiting
- **Error budget tracking**: Monitors SLO violations

### Metrics

| Metric | Description |
|--------|-------------|
| `consolidation_operations_total` | Total consolidation operations (labels: status) |
| `consolidation_summaries_created` | Total summary nodes created |
| `consolidation_edges_created` | Total edges created (labels: edge_type) |
| `consolidation_duration_ms` | Operation duration histogram |
| `consolidation_llm_failures` | LLM failures by namespace (labels: namespace) |

## Knowledge Graph (Graph-Augmented Retrieval)

The knowledge graph enables entity-centric memory retrieval by extracting named entities (people, organizations, technologies, concepts) from captured memories and storing their relationships.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      CaptureService                             │
│                           │                                     │
│                           ▼                                     │
│              ┌────────────────────────┐                         │
│              │   EntityExtractor      │ ◄── LLM or Fallback    │
│              │   (extract entities)   │                         │
│              └────────────────────────┘                         │
│                           │                                     │
│                           ▼                                     │
│              ┌────────────────────────┐                         │
│              │     GraphService       │                         │
│              │  (store in SQLite)     │                         │
│              └────────────────────────┘                         │
└─────────────────────────────────────────────────────────────────┘
```

### Entity Types

| Type | Description | Examples |
|------|-------------|----------|
| `Person` | Individual people | Alice, Bob, John Smith |
| `Organization` | Companies, teams, groups | Anthropic, Rust Foundation |
| `Technology` | Tools, languages, frameworks | Rust, PostgreSQL, React |
| `Concept` | Abstract ideas, patterns | Microservices, CQRS |
| `File` | Source files, documents | src/main.rs, README.md |

### Relationship Types

| Type | Description |
|------|-------------|
| `WorksAt` | Person → Organization |
| `Uses` | Entity → Technology |
| `Implements` | Entity → Concept |
| `RelatesTo` | Generic relationship |
| `DependsOn` | Dependency relationship |
| `Contains` | Containment relationship |

### Configuration

Enable auto-extraction during capture:

```bash
# Environment variable
export SUBCOG_AUTO_EXTRACT_ENTITIES=true

# Or in subcog.toml
[features]
auto_extract_entities = true
```

### CLI Commands

```bash
# List entities
subcog graph entities
subcog graph entities --query "database" --type technology --limit 10

# Show relationships
subcog graph relationships "PostgreSQL"
subcog graph relationships "Alice" --depth 2 --format json

# View statistics
subcog graph stats

# Get entity details
subcog graph get "PostgreSQL" --format json
```

### Data Model

**Entity** (`src/models/graph.rs`):
- `id: EntityId` - Unique identifier (UUID-based)
- `entity_type: EntityType` - Person, Organization, Technology, Concept, File
- `name: String` - Display name
- `aliases: Vec<String>` - Alternative names
- `domain: Domain` - Scope (user/org/project)
- `confidence: f32` - Extraction confidence (0.0-1.0)
- `properties: HashMap<String, Value>` - Custom metadata

**Relationship**:
- `from_entity: EntityId` - Source entity
- `to_entity: EntityId` - Target entity
- `relationship_type: RelationshipType` - Type of relationship
- `confidence: f32` - Relationship confidence

**Mention**:
- `entity_id: EntityId` - Referenced entity
- `memory_id: MemoryId` - Memory containing the mention
- `context: Option<String>` - Surrounding text

### Storage Backend

The graph uses `SQLite` with the following tables:

| Table | Purpose |
|-------|---------|
| `entities` | Entity records with properties |
| `relationships` | Entity-to-entity relationships |
| `mentions` | Entity mentions in memories |

Database location: `{data_dir}/graph.db`

### Graceful Degradation

| Component | Fallback Behavior |
|-----------|------------------|
| LLM unavailable | Uses regex-based entity extraction |
| Graph storage fails | Capture continues without graph storage |
| Entity extraction fails | Warning logged, capture succeeds |

### Metrics

| Metric | Description |
|--------|-------------|
| `entity_extraction_total` | Total extractions (labels: status, fallback) |
| `graph_entities_stored` | Entities stored in graph |
| `graph_relationships_stored` | Relationships stored |
| `graph_query_duration_ms` | Query latency histogram |

## Code Style Requirements

This project uses **clippy** with pedantic and nursery lints, and **rustfmt** for formatting.

### Key Rules

- **Line length**: 100 characters
- **Edition**: 2024
- **MSRV**: 1.85
- **Unsafe code**: Forbidden unless explicitly justified
- **Panics**: Not allowed in library code (`unwrap`, `expect`, `panic!`)

### Error Handling

Always use `Result` types for fallible operations. Never panic in library code:

```rust
// Good - Returns Result
pub fn parse(input: &str) -> Result<Value, ParseError> {
    if input.is_empty() {
        return Err(ParseError::EmptyInput);
    }
    // parsing logic
    Ok(value)
}

// Bad - Panics
pub fn parse(input: &str) -> Value {
    input.parse().unwrap() // Never do this in library code
}
```

Use `thiserror` for custom error types:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Capture failed: {0}")]
    Capture(#[source] CaptureError),

    #[error("Memory not found: {0}")]
    NotFound(String),

    #[error("Security: content blocked")]
    ContentBlocked,

    #[error("Feature not enabled: {0}")]
    FeatureNotEnabled(String),
}
```

### Documentation

All public items must have documentation with examples:

```rust
/// Captures a memory to persistent storage.
///
/// # Arguments
///
/// * `request` - The capture request containing content and metadata.
///
/// # Returns
///
/// A [`CaptureResult`] with the memory ID and URN.
///
/// # Errors
///
/// Returns [`MemoryError::ContentBlocked`] if secrets are detected.
///
/// # Examples
///
/// ```rust
/// use subcog::{CaptureService, CaptureRequest, Namespace};
///
/// let service = CaptureService::new(config)?;
/// let result = service.capture(CaptureRequest {
///     namespace: Namespace::Decisions,
///     content: "Use PostgreSQL".to_string(),
///     ..Default::default()
/// })?;
/// assert!(!result.memory_id.is_empty());
/// # Ok::<(), subcog::MemoryError>(())
/// ```
pub async fn capture(&self, request: CaptureRequest) -> Result<CaptureResult, MemoryError> {
    // implementation
}
```

### Ownership and Borrowing

Prefer borrowing over ownership:

```rust
// Good - borrows
pub fn process(data: &[u8]) -> Vec<u8> { ... }

// Avoid - takes ownership unnecessarily
pub fn process(data: Vec<u8>) -> Vec<u8> { ... }
```

Use `Cow` for flexible string handling:

```rust
use std::borrow::Cow;

pub fn normalize(s: &str) -> Cow<'_, str> {
    if s.contains(' ') {
        Cow::Owned(s.replace(' ', "_"))
    } else {
        Cow::Borrowed(s)
    }
}
```

### Builder Pattern

Use builder pattern for complex configuration:

```rust
#[derive(Debug, Clone, Default)]
pub struct Config {
    timeout: Duration,
    retries: u32,
}

impl Config {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            retries: 3,
        }
    }

    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    #[must_use]
    pub const fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }
}
```

## Testing Conventions

- **Unit tests**: Inside `src/*.rs` with `#[cfg(test)]` modules
- **Integration tests**: `tests/` directory
- **Doc tests**: Examples in documentation
- **Property tests**: Use `proptest` for property-based testing

### Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_case() {
        let result = function_under_test(valid_input);
        assert_eq!(result, expected_output);
    }

    #[test]
    fn test_error_case() {
        let result = function_under_test(invalid_input);
        assert!(matches!(result, Err(MemoryError::NotFound(_))));
    }
}
```

### Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn property_holds(input in any::<i64>()) {
        prop_assert!(predicate(input));
    }
}
```

## Linting Configuration

Clippy is configured to deny:
- `unwrap_used`, `expect_used`, `panic` - Use Result instead
- `todo`, `unimplemented` - Complete implementation
- `dbg_macro`, `print_stdout`, `print_stderr` - Use proper logging

## Supply Chain Security

This project uses `cargo-deny` to audit dependencies:
- **Advisories**: Deny crates with known vulnerabilities
- **Licenses**: Only allow permissive licenses (MIT, Apache-2.0, BSD)
- **Bans**: Block specific problematic crates
- **Sources**: Only allow crates.io

### Dependency Audit Schedule

| Frequency | Task | Command |
|-----------|------|---------|
| **Every PR** | CI runs cargo-deny | `cargo deny check` |
| **Weekly** | Dependabot updates | Automated via GitHub |
| **Quarterly** | Full audit review | See checklist below |

**Quarterly Audit Checklist** (January, April, July, October):
1. `cargo update --dry-run` - Review available updates
2. `cargo outdated` - Check for major version bumps
3. `cargo deny check advisories` - Review any new advisories
4. Review `deny.toml` ignored advisories - Remove resolved, document ongoing
5. Check pre-release dependencies: `cargo tree | grep -E '(rc|alpha|beta)'`
6. Review transitive dependency tree: `cargo tree --duplicates`
7. Update MSRV if Rust stable has new features we need

**Pre-release Dependencies** (monitored, not blocked):
- `ort v2.0.0-rc.9` - Transitive via fastembed. Tracking stable v2.0.0 release

## Architecture Guidelines

### Three-Layer Storage

1. **Persistence Layer** (Authoritative): `SQLite` (primary), PostgreSQL, Filesystem
2. **Index Layer** (Searchable): SQLite + FTS5, PostgreSQL full-text, RediSearch
3. **Vector Layer** (Embeddings): usearch HNSW, pgvector, Redis vector

### Feature Tiers

| Tier | Features | Requirements |
|------|----------|--------------|
| **Core** | Capture, search, `SQLite`, CLI | None |
| **Enhanced** | Secrets filtering, multi-domain, audit | Configuration |
| **LLM-Powered** | Auto-capture, consolidation, temporal | LLM provider |

### Design Principles

1. **Zero-cost abstractions**: Prefer compile-time over runtime overhead
2. **Explicit over implicit**: No hidden allocations or side effects
3. **Error propagation**: Use `?` operator, avoid `.unwrap()`
4. **Const by default**: Use `const fn` where possible
5. **Minimal dependencies**: Only add what's truly needed
6. **Graceful degradation**: Features fail open with fallbacks

## Performance Targets

| Metric | Target |
|--------|--------|
| Cold start | <10ms |
| Capture latency | <30ms |
| Search latency | <50ms |
| Binary size | <100MB |
| Memory (idle) | <50MB |

## CI/CD

The CI pipeline includes:
1. **Format check**: `cargo fmt -- --check`
2. **Lint**: `cargo clippy --all-targets --all-features`
3. **Test**: `cargo test --all-features` (ubuntu, macos, windows)
4. **Documentation**: `cargo doc --no-deps`
5. **Supply chain**: `cargo deny check`
6. **MSRV check**: Rust 1.85
7. **Coverage**: Generate code coverage reports

## LSP Integration

This project is configured with rust-analyzer LSP for enhanced code intelligence.

### Available LSP Operations

Use the LSP tool for semantic code navigation:

| Operation | Use Case |
|-----------|----------|
| `goToDefinition` | Jump to where a symbol is defined |
| `findReferences` | Find all usages of a symbol |
| `hover` | Get type info and documentation |
| `documentSymbol` | List all symbols in a file |
| `workspaceSymbol` | Search symbols across the project |
| `goToImplementation` | Find trait implementations |

### LSP Workflow

When working with Rust code:

1. **Before modifying**: Use `findReferences` to understand impact
2. **For refactoring**: Use `goToDefinition` to trace dependencies
3. **For traits**: Use `goToImplementation` to find all implementors
4. **For exploration**: Use `documentSymbol` to understand file structure

### Hooks Configured

The following hooks run automatically on file save:
- `rustfmt` - Auto-formats code to project standards
- `cargo check` - Fast compilation checking
- `cargo clippy` - Lint warnings and suggestions

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` | CLI argument parsing |
| `rmcp` | MCP server implementation |
| `fastembed` | Embedding generation (all-MiniLM-L6-v2) |
| `usearch` | HNSW vector similarity search |
| `rusqlite` | SQLite + FTS5 for indexing |
| `git2` | Git operations |
| `serde` / `serde_json` / `serde_yml` | Serialization |
| `tokio` | Async runtime |
| `tracing` | Observability |
| `thiserror` / `anyhow` | Error handling |

## Specification Documents

### Active Specifications

**Issue #45: Storage Config Fix** - `docs/spec/active/2026-01-03-issue-45-storage-config/`:
- Active remediation for GitNotes removal and SQLite consolidation
- always run `make ci` before committing or declaring success ensuring all gates pass

### Completed Specifications

- **[MCP Server JSON-RPC Notification Compliance](docs/spec/completed/2026-01-04-issue-46-mcp-notification-fix/)** (2026-01-04)
  - **GitHub Issue**: [#46](https://github.com/zircote/subcog/issues/46)
  - **PR**: [#47](https://github.com/zircote/subcog/pull/47)
  - **Completed**: 2026-01-04
  - **Outcome**: Success - All 7 tasks delivered (100% scope completion)
  - **Effort**: ~1.5 hours (planned 2-4 hours, 38-63% under budget)
  - **Features**:
    - Added `is_notification()` const fn to `JsonRpcRequest` for notification detection
    - Updated stdio transport to skip responses for notifications (empty string + skip writeln)
    - Updated HTTP transport to return 204 No Content for notifications
    - Fixed `format_error()` to always include `id` field (null for parse errors)
    - 12 new unit tests for JSON-RPC 2.0 compliance
  - **Quality**: 1019+ tests passing, `make ci` clean
  - **Key learnings**: Serde deserializes `"id": null` as `None` (not `Some(Value::Null)`), JSON-RPC 2.0 spec treatment of `"id": null` is ambiguous (treated as notification for safety), clippy enforces `const fn` for simple methods
  - **Key docs**: REQUIREMENTS.md, ARCHITECTURE.md, IMPLEMENTATION_PLAN.md, DECISIONS.md, PROGRESS.md, RETROSPECTIVE.md

- **[Storage Architecture Simplification](docs/spec/completed/2026-01-03-storage-simplification/)** (2026-01-03)
  - **Completed**: 2026-01-03
  - **Outcome**: Success - All 32 tasks + 176 code review fixes delivered
  - **Effort**: ~10 hours (planned 24-40 hours, 60-75% under budget)
  - **Features**:
    - Removed git-notes storage layer (fixes critical CaptureService HEAD overwrite bug)
    - User-level SQLite/PostgreSQL storage with project/branch/path faceting
    - Context detection from git remote, branch, and cwd
    - Branch garbage collection with lazy/explicit modes
    - Tombstone pattern for soft deletes
    - 7 CRITICAL security fixes (CRIT-001 to CRIT-007)
    - 30 HIGH priority fixes (security, performance, testing, database)
    - 77 MEDIUM priority fixes (quality, architecture, compliance)
  - **Quality**: 896+ tests passing, all clippy lints resolved, make ci passes
  - **PR**: https://github.com/zircote/subcog/pull/44
  - **Satisfaction**: Very satisfied
  - **Key learnings**: Bottom-up development prevents rework, code review timing matters (176 findings post-implementation), LRU caches require careful const handling, rustdoc link resolution is strict
  - **Key docs**: REQUIREMENTS.md, ARCHITECTURE.md, IMPLEMENTATION_PLAN.md, DECISIONS.md, PROGRESS.md, RETROSPECTIVE.md

- **[Pre-Compact Deduplication](docs/spec/completed/2026-01-01-pre-compact-deduplication/)** (2026-01-02)
  - **Completed**: 2026-01-02
  - **Outcome**: Success - All 7 phases delivered (25/26 tasks, 1 deferred)
  - **Effort**: 8 hours (planned 20-32 hours, 67% under budget)
  - **Features**:
    - Three-tier deduplication: exact match (SHA256 hash tag lookup), semantic similarity (configurable per-namespace thresholds), recent capture (5-minute LRU cache with TTL)
    - DeduplicationService with short-circuit evaluation (exact → semantic → recent)
    - Graceful degradation when embeddings/recall unavailable
    - 64+ deduplication tests + 10 property-based tests (619 total tests)
    - Comprehensive observability (5 metrics, tracing, debug logging)
  - **Key docs**: REQUIREMENTS.md, ARCHITECTURE.md, IMPLEMENTATION_PLAN.md, DECISIONS.md, PROGRESS.md, RETROSPECTIVE.md

- **[Prompt Variable Context-Aware Extraction](docs/spec/completed/2026-01-01-prompt-variable-context-awareness/)** (2026-01-02)
  - **GitHub Issue**: [#29](https://github.com/zircote/subcog/issues/29)
  - **Completed**: 2026-01-02
  - **Outcome**: Success - All 4 phases delivered (20/20 tasks)
  - **Effort**: 8 hours (planned 16-24 hours, ~50% under budget)
  - **Features**:
    - Code block detection (triple backticks and tildes) with regex pattern
    - Context-aware variable extraction skipping code blocks (fixes Issue #29)
    - LLM-assisted metadata enrichment (description, tags, variable definitions)
    - Graceful fallback when LLM unavailable (EnrichmentStatus::Fallback)
    - CLI `--no-enrich` and `--dry-run` flags
    - MCP `skip_enrichment` parameter
    - 685 tests passing, clippy clean
  - **Key docs**: REQUIREMENTS.md, ARCHITECTURE.md, IMPLEMENTATION_PLAN.md, DECISIONS.md, PROGRESS.md, RETROSPECTIVE.md

- **[User Prompt Management](docs/spec/completed/2025-12-30-prompt-management/)** (2025-12-30)
  - **Issues**: [#6](https://github.com/zircote/subcog/issues/6), [#8](https://github.com/zircote/subcog/issues/8), [#9](https://github.com/zircote/subcog/issues/9), [#10](https://github.com/zircote/subcog/issues/10), [#11](https://github.com/zircote/subcog/issues/11), [#12](https://github.com/zircote/subcog/issues/12), [#13](https://github.com/zircote/subcog/issues/13), [#14](https://github.com/zircote/subcog/issues/14)
  - **PR**: [#26](https://github.com/zircote/subcog/pull/26)
  - **Outcome**: Success - All 7 phases delivered (55/56 tasks), 460 tests passing
  - **Features**:
    - Reusable prompt templates with `{{variable}}` substitution
    - Multi-format support (YAML, JSON, Markdown, plain text)
    - Domain-scoped storage (user, org, project, repo)
    - 6 storage backends (Filesystem, SQLite, Git Notes, PostgreSQL, Redis, stub)
    - PostgreSQL auto-migrations for all storage layers
    - 5 MCP tools (prompt_save, prompt_list, prompt_get, prompt_run, prompt_delete)
    - 8 CLI commands (list, get, save, delete, run, export, import, share)
    - Post-tool-use hook for validation
    - Usage tracking and analytics
  - **Key Learnings**:
    - Migration systems pay off early - zero manual schema management
    - PROGRESS.md effective for tracking multi-phase projects
    - Test coverage (460 tests for ~3,000 LOC) provided confidence
    - Git notes storage surprisingly effective for <10k items
  - **Key docs**: REQUIREMENTS.md, ARCHITECTURE.md, IMPLEMENTATION_PLAN.md, PROGRESS.md, RETROSPECTIVE.md

- **[Proactive Memory Surfacing](docs/spec/completed/2025-12-30-issue-15-memory-surfacing/)** (2025-12-30)
  - **Issues**: [#15](https://github.com/zircote/subcog/issues/15), [#24](https://github.com/zircote/subcog/issues/24)
  - **PR**: [#23](https://github.com/zircote/subcog/pull/23)
  - **Outcome**: Success - All 77 tasks delivered, 388 tests passing
  - **Features**:
    - Search intent detection (6 types: HowTo, Location, Explanation, Comparison, Troubleshoot, General)
    - Hybrid detection (keyword <10ms + optional LLM <200ms)
    - Namespace weighting for intent-specific prioritization
    - 3 new MCP resources (search, topics, topics/{topic})
    - 6 new MCP prompts (intent_search, query_suggest, discover, generate_decision, generate_tutorial, context_capture)
    - Hook response format fixes (all 5 hooks now compliant with Claude Code spec)
  - **Key Learnings**:
    - Rust `map_or_else` preferred over `if let Some(...)` per clippy::option_if_let_else
    - Claude Code hook format: `{hookSpecificOutput: {hookEventName, additionalContext}}`
    - HashSet deduplication requires `mut` AND `.insert()` call
    - Graceful degradation testing critical for production readiness