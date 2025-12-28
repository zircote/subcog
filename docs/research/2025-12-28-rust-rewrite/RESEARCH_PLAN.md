# Research Plan: git-notes-memory Rust Rewrite PRD

## Research Classification
- **Type**: CODEBASE + DOMAIN + COMPARATIVE
- **Scope**: Full system rewrite with architectural evolution
- **Target Language**: Rust (user is new to Rust)
- **LSP Available**: Yes

## Research Objectives

### Primary Questions to Answer
1. What are all validated features from the Python POC that must be preserved?
2. What architectural patterns worked well and should be retained?
3. What were the pain points and lessons learned from the Python implementation?
4. How should the pluggable storage system be architected in Rust?
5. What MCP tools are needed and how should they integrate?
6. What Rust ecosystem libraries map to our current Python dependencies?
7. What Rust-specific patterns will improve upon the Python implementation?

### Expected Deliverables
1. Comprehensive PRD document
2. Rust ecosystem mapping (Python → Rust libraries)
3. Architecture decision records for key Rust-specific choices
4. MCP tools specification

## Research Phases

### Phase 1: Current System Analysis
**Sources to investigate:**
- All completed spec retrospectives (lessons learned)
- Current CLAUDE.md (validated features)
- Core module implementations (capture, recall, index, embedding)
- Hook system implementations
- Subconsciousness module (LLM integration)
- Consolidation module (memory lifecycle)
- Security subsystem (secrets filtering)
- Observability subsystem (metrics, tracing)

### Phase 2: Feature Inventory
**Catalog all features by category:**
- Core Memory Operations (capture, recall, search, sync)
- Storage Backend (git notes, SQLite, sqlite-vec)
- Embedding & Search (sentence-transformers, vector KNN)
- Hook System (5 hooks with various handlers)
- Multi-Domain Memories (project vs user scope)
- Subconsciousness (LLM-powered implicit capture)
- Consolidation (tiered storage, clustering, summarization)
- Security (secrets filtering, PII detection, audit logging)
- Observability (OTLP export, metrics, tracing)

### Phase 3: Rust Ecosystem Mapping
**Find Rust equivalents for:**
- sentence-transformers → rust-bert, candle, ort
- sqlite-vec → sqlite-vss bindings, or custom
- detect-secrets → custom or secrets-detect crate
- GitPython → git2-rs
- pydantic-style validation → serde, validator
- OpenTelemetry SDK → opentelemetry-rust
- LLM clients (anthropic, openai) → async-anthropic, async-openai

### Phase 4: Architectural Evolution
**New requirements for Rust version:**
- Pluggable storage backends (trait-based abstraction)
- MCP tools server implementation
- CLI and library dual-mode distribution
- Cross-platform binary distribution
- Performance optimization opportunities
- Memory safety guarantees

### Phase 5: MCP Tools Specification
**Define MCP tools for:**
- memory.capture - Capture new memories
- memory.recall - Search and retrieve memories
- memory.status - System status and statistics
- memory.sync - Synchronize with remotes
- memory.consolidate - Trigger memory consolidation
- memory.configure - Runtime configuration

## Subagent Delegation Plan

### Parallel Investigation Set 1: Current System
1. **Explore Agent**: Deep dive into Python codebase architecture
2. **Research Agent**: Analyze all retrospectives for lessons learned
3. **Research Agent**: Rust ecosystem for AI/ML memory systems

### Parallel Investigation Set 2: Design
1. **Rust Engineer Agent**: Best practices for trait-based storage abstraction
2. **MCP Developer Agent**: MCP tools implementation patterns
3. **Architecture Reviewer**: Validate proposed Rust architecture

## Quality Gates
- [ ] All existing features cataloged
- [ ] All retrospective lessons extracted
- [ ] Rust library mapping complete
- [ ] Pluggable storage design validated
- [ ] MCP tools specification complete
- [ ] Performance expectations defined
- [ ] Security requirements preserved
- [ ] Cross-platform distribution plan

## Risk Assessment
- **Rust learning curve**: User is new to Rust; PRD must be detailed
- **Embedding model portability**: Need to verify Rust ML inference options
- **MCP integration complexity**: Ensure proper JSON-RPC implementation
- **SQLite-vec in Rust**: May need custom bindings or alternative

## Timeline Structure
No time estimates per user guidelines. Focus on dependencies:
1. Research must complete before PRD drafting
2. PRD must be validated before implementation planning
3. Rust ecosystem validation must inform architecture decisions
