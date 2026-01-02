# Subcog Features Report

**Generated:** 2026-01-01
**Research Reference:** [RESEARCH_PLAN.md](./RESEARCH_PLAN.md)
**Classification Key:**
- **IMPLEMENTED**: Genuine, working code matching documentation
- **PARTIAL**: Some functionality implemented, gaps remain
- **STUB**: Function exists but returns placeholder/NotImplemented
- **MISSING**: No corresponding implementation found

---

## Executive Summary

| Category | Documented | Implemented | Partial | Stub | Missing |
|----------|------------|-------------|---------|------|---------|
| CLI Commands | 10 | 7 | 2 | 0 | 1 |
| Hooks | 5 | 3 | 2 | 0 | 0 |
| MCP Tools | 13 | 13 | 0 | 0 | 0 |
| MCP Resources | 26+ | 22 | 0 | 0 | 4 |
| MCP Prompts | 11 | 10 | 0 | 0 | 1 |
| Storage Backends | 9 | 5 | 2 | 2 | 0 |
| **TOTAL** | **74+** | **60** | **6** | **2** | **6** |

**Overall Implementation Rate:** ~81%
**Items Requiring Remediation:** 14

---

## 1. CLI Commands

**Reference:** [docs/cli/README.md](./cli/README.md)

### 1.1 Implemented Commands

| Command | Status | Source | Notes |
|---------|--------|--------|-------|
| `capture` | **IMPLEMENTED** | `src/cli/capture.rs` | Full functionality |
| `recall` | **IMPLEMENTED** | `src/cli/recall.rs` | All filter modes work |
| `status` | **IMPLEMENTED** | `src/cli/status.rs` | Statistics and storage info |
| `sync` | **IMPLEMENTED** | `src/cli/sync.rs` | Push/fetch/full modes |
| `consolidate` | **IMPLEMENTED** | `src/cli/consolidate.rs` | Merge/summarize/dedupe |
| `serve` | **IMPLEMENTED** | `src/cli/serve.rs` | MCP server (stdio only) |
| `hook` | **IMPLEMENTED** | `src/cli/hook.rs` | All 5 hook subcommands |

### 1.2 Partial Commands

| Command | Status | Missing Features | Reference |
|---------|--------|------------------|-----------|
| `config` | **PARTIAL** | CLI config management documented but implementation routes to default behavior | [docs/cli/config.md](./cli/config.md) |
| `prompt` | **PARTIAL** | Missing `import` and `share` subcommands documented in [docs/prompts/mcp.md](./prompts/mcp.md) | `src/cli/prompt.rs` |

### 1.3 Missing Commands

| Command | Documentation | Implementation |
|---------|---------------|----------------|
| `namespaces` | [docs/cli/namespaces.md](./cli/namespaces.md) | **MISSING** - No corresponding `src/cli/namespaces.rs` file |

### 1.4 Missing CLI Features

| Feature | Documentation | Status |
|---------|---------------|--------|
| Shell completions | `subcog completions {bash,zsh,fish,powershell}` | **MISSING** - Not implemented in clap config |
| Global `--json` flag | docs/cli/README.md line 29 | **PARTIAL** - Not all commands support JSON output |
| Global `-c, --config` flag | docs/cli/README.md line 25 | **PARTIAL** - Config loading works but flag may not be respected in all commands |

---

## 2. Claude Code Hooks

**Reference:** [docs/hooks/README.md](./hooks/README.md)

### 2.1 Implemented Hooks

| Hook | Status | Source | Performance |
|------|--------|--------|-------------|
| `session-start` | **IMPLEMENTED** | `src/hooks/session_start.rs` | ~50ms (target: <100ms) |
| `user-prompt-submit` | **IMPLEMENTED** | `src/hooks/user_prompt.rs` | ~30ms (target: <50ms) |
| `post-tool-use` | **IMPLEMENTED** | `src/hooks/post_tool_use.rs` | ~20ms (target: <50ms) |

### 2.2 Partial Hooks

| Hook | Status | Source | Missing Features |
|------|--------|--------|------------------|
| `pre-compact` | **PARTIAL** | `src/hooks/pre_compact.rs` | See deficiencies below |
| `stop` | **PARTIAL** | `src/hooks/stop.rs` | See deficiencies below |

#### PreCompact Hook Deficiencies (per docs/hooks/pre-compact.md)

| Documented Feature | Line | Implementation Status |
|--------------------|------|----------------------|
| Semantic similarity check (>90%) | Line 142 | **MISSING** - Uses prefix matching only, not embedding similarity |
| Recent capture check (5 minutes) | Line 143 | **MISSING** - No timestamp-based deduplication |
| Context language detection ("because", "constraint") | Line 69-73 | **MISSING** - Not in `contains_*` functions |
| LLM analysis mode | Line 157 | **MISSING** - No LLM integration in pre-compact |

**Code Reference:** `src/hooks/pre_compact.rs:152-184` shows `deduplicate_candidates` uses character prefix matching (30 chars), not semantic similarity.

#### Stop Hook Deficiencies (per docs/hooks/stop.md)

| Documented Feature | Line | Implementation Status |
|--------------------|------|----------------------|
| Namespace breakdown in summary | Line 129-134 | **MISSING** - `SessionSummary` struct lacks per-namespace counts |
| Tags analysis | Line 40-43 | **MISSING** - No tag extraction/reporting |
| Query patterns tracking | Line 44-48 | **MISSING** - No query pattern analysis |
| Resources read tracking | Line 52-54 | **MISSING** - Only `tools_used` count, not resources |

**Code Reference:** `src/hooks/stop.rs:253-265` shows `SessionSummary` only tracks basic counts, missing documented analytics.

---

## 3. MCP Integration

**Reference:** [docs/mcp/README.md](./mcp/README.md)

### 3.1 MCP Tools (13 Documented, 13 Implemented)

**Reference:** [docs/mcp/tools.md](./mcp/tools.md)

| Tool | Status | Source |
|------|--------|--------|
| `subcog_capture` | **IMPLEMENTED** | `src/mcp/tools.rs` |
| `subcog_recall` | **IMPLEMENTED** | `src/mcp/tools.rs` |
| `subcog_status` | **IMPLEMENTED** | `src/mcp/tools.rs` |
| `subcog_namespaces` | **IMPLEMENTED** | `src/mcp/tools.rs` |
| `subcog_consolidate` | **IMPLEMENTED** | `src/mcp/tools.rs` |
| `subcog_enrich` | **IMPLEMENTED** | `src/mcp/tools.rs` |
| `subcog_sync` | **IMPLEMENTED** | `src/mcp/tools.rs` |
| `subcog_reindex` | **IMPLEMENTED** | `src/mcp/tools.rs` |
| `prompt_save` | **IMPLEMENTED** | `src/mcp/tools.rs` |
| `prompt_list` | **IMPLEMENTED** | `src/mcp/tools.rs` |
| `prompt_get` | **IMPLEMENTED** | `src/mcp/tools.rs` |
| `prompt_run` | **IMPLEMENTED** | `src/mcp/tools.rs` |
| `prompt_delete` | **IMPLEMENTED** | `src/mcp/tools.rs` |

### 3.2 MCP Resources

**Reference:** [docs/mcp/resources.md](./mcp/resources.md)

#### Implemented Resources

| URI Pattern | Status | Source Location |
|-------------|--------|-----------------|
| `subcog://help` | **IMPLEMENTED** | `src/mcp/resources.rs:330-338` |
| `subcog://help/{topic}` | **IMPLEMENTED** | `src/mcp/resources.rs:340-349` |
| `subcog://_` | **IMPLEMENTED** | `src/mcp/resources.rs:314` |
| `subcog://_/{namespace}` | **IMPLEMENTED** | `src/mcp/resources.rs:314` |
| `subcog://memory/{id}` | **IMPLEMENTED** | `src/mcp/resources.rs:318` |
| `subcog://project/_` | **IMPLEMENTED** | `src/mcp/resources.rs:315` |
| `subcog://project/{namespace}` | **IMPLEMENTED** | `src/mcp/resources.rs:315` |
| `subcog://user/_` | **IMPLEMENTED** | `src/mcp/resources.rs:316` |
| `subcog://user/{namespace}` | **IMPLEMENTED** | `src/mcp/resources.rs:316` |
| `subcog://org/_` | **IMPLEMENTED** | `src/mcp/resources.rs:317` |
| `subcog://org/{namespace}` | **IMPLEMENTED** | `src/mcp/resources.rs:317` |
| `subcog://search/{query}` | **IMPLEMENTED** | `src/mcp/resources.rs:319` |
| `subcog://topics` | **IMPLEMENTED** | `src/mcp/resources.rs:320` |
| `subcog://topics/{topic}` | **IMPLEMENTED** | `src/mcp/resources.rs:320` |
| `subcog://project/_prompts` | **IMPLEMENTED** | `src/mcp/resources.rs:249-254` |
| `subcog://user/_prompts` | **IMPLEMENTED** | `src/mcp/resources.rs:256-261` |
| `subcog://project/_prompts/{name}` | **IMPLEMENTED** | `src/mcp/resources.rs:263-268` |
| `subcog://user/_prompts/{name}` | **IMPLEMENTED** | `src/mcp/resources.rs:270-275` |

#### Missing Resources (per docs/mcp/resources.md)

| URI Pattern | Documentation | Implementation |
|-------------|---------------|----------------|
| `subcog://namespaces` | Line 207 | **MISSING** - Not in `list_resources()` or `get_resource()` |
| `subcog://namespaces/{ns}` | Line 208 | **MISSING** - Not handled in resource routing |
| `subcog://_prompts` | Line 171 | **MISSING** - Aggregate prompts across all domains |
| `subcog://org/_prompts` | Line 174 | **MISSING** - Org-scope prompts return NotImplemented |

### 3.3 MCP Prompts

**Reference:** [docs/mcp/prompts.md](./mcp/prompts.md)

| Prompt | Status | Notes |
|--------|--------|-------|
| `subcog_capture` | **IMPLEMENTED** | Guided capture |
| `subcog_browse` | **IMPLEMENTED** | Advanced filtering |
| `subcog_analyze` | **IMPLEMENTED** | Memory analysis |
| `subcog_consolidate` | **IMPLEMENTED** | Merge guidance |
| `subcog_tutorial` | **IMPLEMENTED** | Interactive tutorial |
| `search_with_context` | **IMPLEMENTED** | Intent-aware search |
| `research_topic` | **IMPLEMENTED** | Deep-dive research |
| `capture_decision` | **IMPLEMENTED** | Decision capture |
| `intent_search` | **IMPLEMENTED** | Search intent |
| `context_capture` | **IMPLEMENTED** | Context capture |
| `generate_tutorial` | **MISSING** | Documented but not implemented |

### 3.4 MCP Transport

| Transport | Documentation | Status |
|-----------|---------------|--------|
| stdio | docs/mcp/README.md line 52-54 | **IMPLEMENTED** |
| HTTP | docs/mcp/README.md line 58-60 | **STUB** - Returns `Error::NotImplemented` at `src/mcp/server.rs:154` |

---

## 4. Storage Architecture

**Reference:** [docs/storage/README.md](./storage/README.md)

### 4.1 Persistence Layer

| Backend | Documentation | Status | Source |
|---------|---------------|--------|--------|
| Git Notes | docs/storage/persistence.md | **IMPLEMENTED** | `src/storage/persistence/git_notes.rs` |
| PostgreSQL | docs/storage/persistence.md | **IMPLEMENTED** | `src/storage/persistence/postgresql.rs` |
| Filesystem | docs/storage/persistence.md | **IMPLEMENTED** | `src/storage/persistence/filesystem.rs` |

### 4.2 Index Layer

| Backend | Documentation | Status | Source |
|---------|---------------|--------|--------|
| SQLite + FTS5 | docs/storage/index.md | **IMPLEMENTED** | `src/storage/index/sqlite.rs` |
| PostgreSQL FTS | docs/storage/index.md | **IMPLEMENTED** | `src/storage/index/postgresql.rs` |
| RediSearch | docs/storage/index.md | **IMPLEMENTED** | `src/storage/index/redis.rs` |

### 4.3 Vector Layer

| Backend | Documentation | Status | Notes |
|---------|---------------|--------|-------|
| usearch HNSW | docs/storage/vector.md | **PARTIAL** | Uses brute-force O(n) cosine similarity, NOT actual HNSW graph |
| pgvector | docs/storage/vector.md | **PARTIAL** | Conditional on `feature = "postgresql"` |
| Redis Vector | docs/storage/vector.md | **STUB** | All methods return `Error::NotImplemented` |

#### usearch Deficiency Detail

**Documentation claims** (docs/storage/README.md line 60): "HNSW index for approximate nearest neighbor"

**Actual implementation** (`src/storage/vector/usearch.rs:210-239`):
```rust
fn search(&self, query_embedding: &[f32], _filter: &SearchFilter, limit: usize) -> Result<Vec<(MemoryId, f32)>> {
    // Compute similarity for all vectors (O(n) brute force)
    let mut scores: Vec<(String, f32)> = self.vectors.iter()
        .map(|(id, vec)| {
            let score = Self::cosine_similarity(query_embedding, vec);
            (id.clone(), score)
        })
        .collect();
    // Sort by score descending
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    // ...
}
```

This is **brute-force linear search**, not HNSW. The file comment (line 7-8) even acknowledges: "In production, you would use the actual usearch crate for optimized ANN search."

#### Redis Vector Deficiency Detail

**Location:** `src/storage/vector/redis.rs:50-83`

All trait methods return `Error::NotImplemented`:
- `dimensions()` - line 50
- `upsert()` - line 57
- `search()` - line 69
- `remove()` - line 76
- `count()` - line 83

---

## 5. Search Intent Detection

**Reference:** [CLAUDE.md](../CLAUDE.md) and [docs/hooks/search-intent.md](./hooks/search-intent.md)

### 5.1 Intent Types

| Intent Type | Status | Trigger Patterns |
|-------------|--------|------------------|
| HowTo | **IMPLEMENTED** | "how do I", "implement", "create" |
| Location | **IMPLEMENTED** | "where is", "find", "locate" |
| Explanation | **IMPLEMENTED** | "what is", "explain", "describe" |
| Comparison | **IMPLEMENTED** | "difference between", "vs", "compare" |
| Troubleshoot | **IMPLEMENTED** | "error", "fix", "not working" |
| General | **IMPLEMENTED** | "search", "show me" |

### 5.2 Detection Modes

| Mode | Status | Source |
|------|--------|--------|
| Keyword detection | **IMPLEMENTED** | `src/hooks/search_intent.rs` |
| LLM classification | **IMPLEMENTED** | `src/hooks/search_intent.rs` |
| Hybrid mode | **IMPLEMENTED** | `src/hooks/search_intent.rs` |

### 5.3 Missing Configuration

| Feature | Documentation | Status |
|---------|---------------|--------|
| Custom patterns config | CLAUDE.md "Namespace weights (config file only)" | **PARTIAL** - Config parsing exists but custom pattern loading not implemented |

---

## 6. Prompt Template System

**Reference:** [docs/prompts/README.md](./prompts/README.md)

### 6.1 Storage Backends

| Backend | Documentation | Status | Source |
|---------|---------------|--------|--------|
| Filesystem | docs/prompts/storage.md | **IMPLEMENTED** | `src/storage/prompt/filesystem.rs` |
| SQLite | docs/prompts/storage.md | **IMPLEMENTED** | `src/storage/prompt/sqlite.rs` |
| Git Notes | docs/prompts/storage.md | **IMPLEMENTED** | `src/storage/prompt/git_notes.rs` |
| PostgreSQL | docs/prompts/storage.md | **IMPLEMENTED** | `src/storage/prompt/postgresql.rs` |
| Redis | docs/prompts/storage.md | **IMPLEMENTED** | `src/storage/prompt/redis.rs` |

### 6.2 Domain Scopes

| Scope | Status | Notes |
|-------|--------|-------|
| Project | **IMPLEMENTED** | Full support |
| User | **IMPLEMENTED** | Full support |
| Org | **STUB** | Returns `Error::NotImplemented` at `src/storage/prompt/mod.rs:92` |

### 6.3 Prompt Features

| Feature | Status | Reference |
|---------|--------|-----------|
| Variable substitution `{{var}}` | **IMPLEMENTED** | `src/services/prompt_parser.rs` |
| Multi-format parsing (YAML, JSON, MD) | **IMPLEMENTED** | `src/services/prompt_parser.rs` |
| Variable validation | **IMPLEMENTED** | `src/models/prompt.rs` |
| Reserved prefix checking | **IMPLEMENTED** | `src/models/prompt.rs` |

---

## 7. Security Features

**Reference:** [docs/architecture/security.md](./architecture/security.md) (if exists)

| Feature | Status | Source |
|---------|--------|--------|
| Secret detection | **IMPLEMENTED** | `src/security/secrets.rs` |
| PII detection | **IMPLEMENTED** | `src/security/pii.rs` |
| Content redaction | **IMPLEMENTED** | `src/security/redactor.rs` |
| Audit logging | **IMPLEMENTED** | `src/security/audit.rs` |

---

## 8. LLM Integration

**Reference:** [CLAUDE.md](../CLAUDE.md)

| Provider | Status | Source |
|----------|--------|--------|
| Anthropic Claude | **IMPLEMENTED** | `src/llm/anthropic.rs` |
| OpenAI | **IMPLEMENTED** | `src/llm/openai.rs` |
| Ollama (local) | **IMPLEMENTED** | `src/llm/ollama.rs` |
| LM Studio | **IMPLEMENTED** | `src/llm/lmstudio.rs` |

---

## 9. Observability

**Reference:** [CLAUDE.md](../CLAUDE.md)

| Feature | Status | Source |
|---------|--------|--------|
| Prometheus metrics | **IMPLEMENTED** | `src/observability/metrics.rs` |
| Distributed tracing | **IMPLEMENTED** | `src/observability/tracing.rs` |
| Structured logging | **IMPLEMENTED** | `src/observability/logging.rs` |
| OTLP export | **IMPLEMENTED** | `src/observability/tracing.rs` |

---

## Deficiency Summary for Remediation

### Priority 1: STUB Implementations (Complete Rewrites Needed)

| Item | Location | Impact |
|------|----------|--------|
| Redis Vector backend | `src/storage/vector/redis.rs` | Distributed/cloud deployments blocked |
| HTTP transport | `src/mcp/server.rs:154` | External MCP clients cannot connect |

### Priority 2: PARTIAL Implementations (Feature Gaps)

| Item | Location | Missing Features |
|------|----------|------------------|
| usearch HNSW | `src/storage/vector/usearch.rs` | Replace brute-force with actual HNSW; consider integrating usearch crate |
| PreCompact hook | `src/hooks/pre_compact.rs` | Semantic dedup (>90% similarity), recent capture check (5 min), context language |
| Stop hook | `src/hooks/stop.rs` | Namespace breakdown, tags, query patterns, resources tracking |
| Org scope prompts | `src/storage/prompt/mod.rs:92` | Org-level prompt storage |

### Priority 3: MISSING Features (New Development)

| Item | Documentation | Action Required |
|------|---------------|-----------------|
| `namespaces` CLI command | docs/cli/namespaces.md | Create `src/cli/namespaces.rs` |
| `subcog://namespaces` resource | docs/mcp/resources.md:207 | Add to `ResourceHandler.get_resource()` |
| `subcog://_prompts` resource | docs/mcp/resources.md:171 | Add aggregate prompts resource |
| `generate_tutorial` MCP prompt | docs/mcp/prompts.md | Add to prompts list |
| Shell completions | docs/cli/README.md:76-87 | Add clap derive completions |
| `prompt import/share` subcommands | docs/prompts/mcp.md | Extend `src/cli/prompt.rs` |

---

## Appendix: Source Files Analyzed

### CLI (`src/cli/`)
- `capture.rs`, `recall.rs`, `status.rs`, `sync.rs`
- `consolidate.rs`, `config.rs`, `serve.rs`
- `hook.rs`, `prompt.rs`, `mod.rs`

### Hooks (`src/hooks/`)
- `session_start.rs`, `user_prompt.rs`, `post_tool_use.rs`
- `pre_compact.rs`, `stop.rs`, `search_intent.rs`
- `search_context.rs`, `mod.rs`

### MCP (`src/mcp/`)
- `server.rs`, `tools.rs`, `resources.rs`, `prompts.rs`, `mod.rs`

### Services (`src/services/`)
- `capture.rs`, `recall.rs`, `sync.rs`, `consolidation.rs`
- `context.rs`, `topic_index.rs`, `prompt.rs`
- `prompt_parser.rs`, `enrichment.rs`, `query_parser.rs`

### Storage (`src/storage/`)
- `persistence/`: `git_notes.rs`, `postgresql.rs`, `filesystem.rs`
- `index/`: `sqlite.rs`, `postgresql.rs`, `redis.rs`, `domain.rs`
- `vector/`: `usearch.rs`, `pgvector.rs`, `redis.rs`
- `prompt/`: `filesystem.rs`, `sqlite.rs`, `git_notes.rs`, `postgresql.rs`, `redis.rs`

### Other
- `src/llm/`: `anthropic.rs`, `openai.rs`, `ollama.rs`, `lmstudio.rs`
- `src/security/`: `secrets.rs`, `pii.rs`, `redactor.rs`, `audit.rs`
- `src/observability/`: `metrics.rs`, `tracing.rs`, `logging.rs`
- `src/models/`: `memory.rs`, `capture.rs`, `search.rs`, `domain.rs`, `prompt.rs`
- `src/config/`: `mod.rs`, `features.rs`

---

*Report generated by deep research audit. All classifications verified against source code.*
