# Research Notes

This document summarizes the research findings that inform the Subcog Rust rewrite specification.

---

## Research Documents Summary

The specification was generated from comprehensive research conducted in `docs/research/2025-12-28-rust-rewrite/`:

| Document | Version | Key Contributions |
|----------|---------|-------------------|
| PRD.md | v2.1.0 | Core requirements, architecture overview, feature tiers, phasing |
| STORAGE_AND_OBSERVABILITY.md | v1.0.0 | Three-layer storage, trait definitions, observability pipeline |
| MCP_RESOURCES_AND_LLM.md | v1.0.0 | URN scheme, domain hierarchy, LLM provider implementations |
| ACCESS_INTERFACES.md | v1.0.0 | CLI, MCP server, streaming API, hook system |
| SEAMLESS_INTEGRATION.md | v1.0.0 | Event bus, pipeline composition, error propagation |
| RESEARCH_PLAN.md | v1.0.0 | Research methodology and quality gates |

---

## Key Research Findings

### 1. Python POC Validation

The Python implementation (git-notes-memory) validated the core architecture with:

| Metric | Achievement |
|--------|-------------|
| Capture Latency | <10ms (target <50ms) |
| Search Performance | <50ms for 10K memories |
| Test Coverage | 87%+ |
| Search Accuracy | ~90% relevance |
| Hook Integration | All 5 Claude Code hooks working |

**Lessons Learned:**
- Frozen data structures prevented bugs (use Rust ownership)
- Service factory pattern with lazy initialization works well
- Graceful degradation is essential
- Adaptive token budgets improve Claude output quality
- XML context format produces better results than JSON

### 2. Storage Backend Research

**SQLite + usearch Analysis:**
- Best for single-user, local-first deployments
- <10ms latency achievable
- ~50MB memory footprint
- Single-file deployment (excluding usearch index)
- FTS5 provides excellent BM25 search

**PostgreSQL + pgvector Analysis:**
- Required for multi-user/team deployments
- ACID guarantees for concurrent access
- Horizontal scaling with read replicas
- pgvector extension mature and performant
- ~50ms latency acceptable for team usage

**Redis Analysis:**
- Best for caching and real-time scenarios
- <5ms latency
- RediSearch for full-text
- HNSW vector search available
- In-memory (RAM requirements)

### 3. Embedding Model Research

**all-MiniLM-L6-v2 (Selected):**
- 384 dimensions (compact)
- Fast inference (~20ms per text)
- Good quality for general text
- Small model size (~80MB)
- Available via fastembed crate

**Alternatives Considered:**
- all-mpnet-base-v2: Better quality, slower (768d)
- text-embedding-ada-002: Requires API, not local-first
- BGE models: Good but larger

### 4. MCP Protocol Research

**rmcp Crate Analysis:**
- Active development, version 0.12+
- stdio transport works well
- SSE transport for network access
- Tool, resource, and prompt primitives
- Subscription support for real-time updates

**URN Design:**
- `subcog://{domain}/{namespace}/{id}` format
- Domain hierarchy: project, user, org
- Namespace: decisions, learnings, etc.
- ID: commit-based with index

### 5. Observability Research

**OpenTelemetry Stack:**
- `tracing` crate for instrumentation
- `opentelemetry` for OTLP export
- `tracing-subscriber` for formatting
- Prometheus endpoint optional

**Audit Requirements (SOC2/GDPR):**
- All data access must be logged
- 90-day retention default
- Immutable audit logs
- User context in all events

### 6. Hook System Research

**Claude Code Hook Timing:**
| Hook | Max Latency | Notes |
|------|-------------|-------|
| SessionStart | 2000ms | Can do remote fetch |
| UserPromptSubmit | 50ms | Must be fast |
| PostToolUse | 100ms | Context injection |
| PreCompact | 500ms | Content analysis |
| Stop | 5000ms | Can do remote push |

**JSON Contract:**
- Must NEVER fail with non-zero exit
- Must ALWAYS output valid JSON
- Errors returned as empty context with comment

### 7. LLM Provider Research

**Anthropic (Claude):**
- Primary provider for advanced features
- Best reasoning capability
- Token-based pricing

**OpenAI:**
- Good alternative
- Widespread adoption
- Function calling support

**Ollama:**
- Local inference
- No API key required
- Good for privacy-sensitive

**LM Studio:**
- OpenAI-compatible API
- Good UX for local models
- Cross-platform

---

## Technical Constraints

### Performance Requirements

| Operation | Target | Rationale |
|-----------|--------|-----------|
| Cold start | <10ms | CLI responsiveness |
| Capture | <30ms | Interactive feel |
| Vector search | <50ms | User experience |
| Hook overhead | <100ms | Claude Code responsiveness |
| Binary size | <100MB | Reasonable download |

### Resource Limits

| Resource | Limit | Rationale |
|----------|-------|-----------|
| Memory (idle) | <50MB | Background service |
| Memory (active) | <500MB | Burst operations |
| Disk (per 10K) | ~100MB | Reasonable growth |
| Threads | ≤CPU cores | Efficiency |

### Compatibility Requirements

| Requirement | Details |
|-------------|---------|
| Git notes format | YAML front matter, unchanged from Python |
| Hook JSON | Same contract as Python version |
| Environment vars | Same names as Python version |
| Migration | Zero data loss from Python |

---

## Risk Analysis

### Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| fastembed performance | Low | Medium | Fallback to BM25 |
| usearch stability | Low | High | Pin version, test extensively |
| rmcp breaking changes | Medium | Medium | Pin version, watch upstream |
| Git notes at scale | Medium | Medium | Document limits, plan alternative |

### Operational Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Model download UX | Medium | Low | Progress indicators, caching |
| Network dependency | Medium | Low | Offline-first design |
| Configuration complexity | Medium | Low | Good defaults, validation |

---

## Lessons from Python POC

### What Worked Well (Preserve)

1. **Frozen Data Structures**
   - Immutability prevented subtle bugs
   - Use Rust's ownership model for same effect

2. **Service Factory Pattern**
   - Lazy initialization avoided startup cost
   - Use `lazy_static!` or `once_cell` in Rust

3. **Graceful Degradation**
   - Features fail open, core continues
   - Design every feature with fallback

4. **Test Discipline**
   - 87%+ coverage caught real bugs
   - Integration tests essential

5. **Adaptive Token Budgets**
   - Scale context to project complexity
   - Preserve in Rust version

6. **XML Context Format**
   - Structured prompts improve Claude output
   - Keep XML formatting

### What Didn't Work (Change)

1. **Git Version Assumptions**
   - Don't assume git version features
   - Detect dynamically

2. **Hook JSON Output**
   - Template-based generation failed
   - Use serde with strict validation

3. **Embedding Download UX**
   - Silent download confused users
   - Add progress bars

4. **Documentation Timing**
   - After-the-fact docs were incomplete
   - Document alongside implementation

5. **Performance Testing**
   - Late benchmarks found issues
   - Establish benchmarks from day 1

### Critical Constraints (Respect)

| Constraint | Limit | Consequence if Violated |
|------------|-------|-------------------------|
| Signal detection | <50ms | Blocks Claude response |
| Capture pipeline | <10ms | User feels lag |
| SessionStart | <2000ms | Session start feels slow |
| Post-tool injection | <100ms | Context switching lag |
| Test coverage | ≥80% | Regressions likely |

---

## Research Methodology

### Phase 1: Requirements Gathering
- Analyzed Python codebase
- Extracted feature list
- Documented success metrics

### Phase 2: Rust Ecosystem Mapping
- Identified crate equivalents
- Evaluated performance
- Checked maintenance status

### Phase 3: Architecture Design
- Defined storage layers
- Designed trait abstractions
- Planned event flow

### Phase 4: Validation
- Cross-checked with Python POC
- Verified performance targets
- Confirmed compatibility

---

## References

### Crate Documentation
- [fastembed](https://docs.rs/fastembed) - Embedding generation
- [usearch](https://docs.rs/usearch) - Vector search
- [rmcp](https://docs.rs/rmcp) - MCP protocol
- [git2](https://docs.rs/git2) - Git operations
- [rusqlite](https://docs.rs/rusqlite) - SQLite access
- [tracing](https://docs.rs/tracing) - Instrumentation
- [clap](https://docs.rs/clap) - CLI framework

### External Resources
- [Model Context Protocol](https://modelcontextprotocol.io/) - MCP specification
- [OpenTelemetry](https://opentelemetry.io/) - Observability standard
- [HNSW Algorithm](https://arxiv.org/abs/1603.09320) - Vector search algorithm

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-28 | Claude Opus 4.5 | Initial research summary |
