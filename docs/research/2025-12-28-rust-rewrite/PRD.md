# Product Requirements Document: Memory System Rust Rewrite

**Document Type**: PRD
**Version**: 2.0.0
**Date**: 2025-12-28
**Status**: Draft
**Author**: Claude Opus 4.5 with User Collaboration

---

## Executive Summary

This PRD defines requirements for a complete rewrite of the `git-notes-memory` system from Python to Rust. The rewrite aims to:

1. **Consolidate all validated features** from the Python POC (90%+ success rate)
2. **Improve performance** with native code and single-binary distribution
3. **Implement truly pluggable storage backends** - git notes is just the beginning; Redis, PostgreSQL, Pinecone, and future backends must be first-class citizens through configuration
4. **Implement MCP tools** for AI agent integration
5. **Provide industry best-of-breed observability** - full execution profiling, distributed tracing, structured logging, and audit capabilities
6. **Enable explicit opt-out for enhanced features** - LLM-powered features, consolidation, and other enhancements must be optional; core memory + semantic search must work standalone
7. **Ensure seamless feature integration** - every capability must integrate cohesively with others; no isolated features

### Critical Design Principles

| Principle | Requirement |
|-----------|-------------|
| **Pluggable Storage** | Backend selection via configuration; git notes, Redis, PostgreSQL+pgvector, Pinecone all supported |
| **Feature Tiers** | Core tier works without LLM; Enhanced and LLM tiers are opt-in |
| **Full Observability** | Every operation is traceable, measurable, and auditable |
| **Seamless Integration** | Features compose cleanly; no feature works in isolation |
| **Configuration-Driven** | All backends and features selectable through unified config |

The Python implementation served as a successful research and POC platform, validating:
- Git notes as persistent storage with YAML front matter
- SQLite + sqlite-vec for semantic search
- Hook-based integration with Claude Code
- LLM-powered implicit capture and consolidation
- Multi-domain memories (project + user scope)
- Secrets filtering and PII protection

---

## Table of Contents

1. [Background and Context](#1-background-and-context)
2. [Goals and Non-Goals](#2-goals-and-non-goals)
3. [User Stories](#3-user-stories)
4. [Feature Tiers and Opt-Out Architecture](#4-feature-tiers-and-opt-out-architecture)
5. [Functional Requirements](#5-functional-requirements)
6. [Architecture Overview](#6-architecture-overview)
7. [Data Models](#7-data-models)
8. [MCP Tools Specification](#8-mcp-tools-specification)
9. [Pluggable Storage System](#9-pluggable-storage-system)
10. [Vector Search Abstraction](#10-vector-search-abstraction)
11. [Configuration System](#11-configuration-system)
12. [Performance Requirements](#12-performance-requirements)
13. [Security Requirements](#13-security-requirements)
14. [Observability Requirements](#14-observability-requirements)
15. [Rust Ecosystem Mapping](#15-rust-ecosystem-mapping)
16. [Migration Path](#16-migration-path)
17. [Phasing and Milestones](#17-phasing-and-milestones)
18. [Success Criteria](#18-success-criteria)
19. [Appendix: Lessons Learned from Python POC](#19-appendix-lessons-learned)

---

## 1. Background and Context

### 1.1 What is git-notes-memory?

`git-notes-memory` is an AI-powered memory system that enables Claude Code (and other AI assistants) to:
- **Capture** decisions, learnings, blockers, and progress as git notes
- **Recall** relevant context using semantic (vector) search
- **Consolidate** memories over time using LLM-powered summarization
- **Surface** proactively relevant information during coding sessions

### 1.2 Why Rust?

The Python implementation achieved its goals but has inherent limitations:

| Aspect | Python | Rust |
|--------|--------|------|
| **Distribution** | Requires Python runtime, pip, virtualenv | Single static binary |
| **Startup Time** | ~500ms+ with model loading | ~10ms cold start |
| **Memory Usage** | Unpredictable (GC pauses) | Predictable, minimal |
| **Concurrency** | GIL limitations | True parallelism |
| **Type Safety** | Runtime errors | Compile-time guarantees |
| **Security** | Memory safety via runtime | Memory safety via compiler |

### 1.3 Success Metrics from Python POC

The Python implementation validated these capabilities:

- **Capture Pipeline**: <10ms latency (target <50ms)
- **Search Performance**: <50ms for 10K memories
- **Hook Integration**: All 5 Claude Code hooks working
- **Test Coverage**: 87%+ across all modules
- **Accuracy**: ~90% relevance in semantic search

---

## 2. Goals and Non-Goals

### 2.1 Goals

**G1: Full Feature Parity**
Implement all validated features from Python POC without regression.

**G2: Performance Improvement**
- Single-binary distribution (<100MB)
- <10ms cold start
- <30ms capture pipeline
- <50ms search (10K memories)

**G3: Pluggable Storage**
Trait-based storage abstraction supporting multiple backends.

**G4: MCP Tools Integration**
First-class Model Context Protocol tools for AI agent integration.

**G5: Maintainability**
Clean architecture, comprehensive tests, documentation.

### 2.2 Non-Goals

**NG1: Web Interface**
No web UI; CLI and MCP tools only.

**NG2: Multi-User/Cloud**
Single-user, local-first; no multi-tenant cloud service.

**NG3: Real-Time Sync**
Batch sync on session boundaries; no real-time streaming.

**NG4: Custom Embedding Training**
Use pre-trained models; no fine-tuning infrastructure.

---

## 3. User Stories

### 3.1 Memory Capture

**US-C1**: As a developer, I want to capture decisions during coding sessions so I can recall the rationale later.

**US-C2**: As a developer, I want inline markers like `[remember]` to capture memories without leaving my workflow.

**US-C3**: As a developer, I want global memories (preferences, patterns) that persist across projects.

**US-C4**: As a developer, I want automatic capture of high-confidence insights without manual intervention.

### 3.2 Memory Recall

**US-R1**: As a developer, I want semantic search to find relevant memories even with different phrasing.

**US-R2**: As a developer, I want automatic context injection at session start with relevant memories.

**US-R3**: As a developer, I want to filter memories by namespace, domain, or time range.

**US-R4**: As a developer, I want LLM-powered reasoning for temporal queries ("when did we decide...").

### 3.3 Memory Consolidation

**US-CO1**: As a developer, I want memories to be summarized over time to reduce noise.

**US-CO2**: As a developer, I want outdated memories to be archived automatically.

**US-CO3**: As a developer, I want to see relationships between memories (supersedes, references).

### 3.4 Integration

**US-I1**: As a developer, I want memory tools available via MCP for any AI agent.

**US-I2**: As a developer, I want to sync memories across machines via git remote.

**US-I3**: As a developer, I want secrets filtered from captured memories automatically.

---

## 4. Feature Tiers and Opt-Out Architecture

**CRITICAL REQUIREMENT**: The memory system MUST support explicit opt-out for enhanced features. Users must be able to run the system with only core memory and semantic search capabilities, without requiring LLM providers, external services, or advanced features.

### 4.1 Feature Tier Definitions

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           FEATURE TIERS                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  TIER 3: LLM-POWERED (opt-in, requires LLM provider)                        │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ • Implicit capture (auto-detect capture-worthy content)              │   │
│  │ • Memory consolidation (clustering + summarization)                  │   │
│  │ • Supersession detection (LLM determines outdated memories)          │   │
│  │ • Temporal reasoning ("when did we decide...")                       │   │
│  │ • Query expansion (LLM rewrites queries for better recall)           │   │
│  │ • Smart capture suggestions                                          │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  TIER 2: ENHANCED (opt-in, no external services required)                   │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ • Entity extraction and matching (NER-based)                         │   │
│  │ • Temporal extraction and matching                                   │   │
│  │ • Advanced filtering (tags, date ranges, specs)                      │   │
│  │ • Memory relationships (edges: references, relates_to)               │   │
│  │ • Tiered storage (HOT/WARM/COLD without LLM)                         │   │
│  │ • Hook system (Claude Code integration)                              │   │
│  │ • Secrets filtering and PII detection                                │   │
│  │ • Advanced observability (OTLP export, profiling)                    │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  TIER 1: CORE (always available, zero external dependencies)                │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ • Memory capture (namespace, summary, content, domain)               │   │
│  │ • Semantic search (vector similarity via embeddings)                 │   │
│  │ • BM25 full-text search                                              │   │
│  │ • Hybrid search (RRF fusion)                                         │   │
│  │ • Git notes persistence (authoritative storage)                      │   │
│  │ • Index synchronization (git notes ↔ search index)                   │   │
│  │ • Multi-domain memories (project + user scope)                       │   │
│  │ • Progressive hydration (SUMMARY → FULL → FILES)                     │   │
│  │ • Basic metrics and logging                                          │   │
│  │ • MCP tools (capture, recall, status, sync)                          │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Feature Dependencies Matrix

| Feature | Tier | Requires LLM | Requires External Service | Can Disable |
|---------|------|--------------|---------------------------|-------------|
| Memory capture | Core | No | No | No (core) |
| Semantic search | Core | No | No | No (core) |
| BM25 search | Core | No | No | No (core) |
| Hybrid search | Core | No | No | No (core) |
| Git notes storage | Core | No | No | No (core) |
| Index sync | Core | No | No | No (core) |
| Multi-domain | Core | No | No | No (core) |
| Basic metrics | Core | No | No | No (core) |
| MCP tools | Core | No | No | No (core) |
| Entity extraction | Enhanced | No | No | **Yes** |
| Temporal extraction | Enhanced | No | No | **Yes** |
| Hook system | Enhanced | No | No | **Yes** |
| Secrets filtering | Enhanced | No | No | **Yes** |
| OTLP export | Enhanced | No | OTLP endpoint | **Yes** |
| Implicit capture | LLM | **Yes** | LLM provider | **Yes** |
| Consolidation | LLM | **Yes** | LLM provider | **Yes** |
| Supersession detection | LLM | **Yes** | LLM provider | **Yes** |
| Temporal reasoning | LLM | **Yes** | LLM provider | **Yes** |
| Query expansion | LLM | **Yes** | LLM provider | **Yes** |

### 4.3 Configuration for Feature Tiers

```toml
# memory.toml - Feature tier configuration

[features]
# Tier 2: Enhanced features (default: enabled)
entity_extraction = true
temporal_extraction = true
hooks_enabled = true
secrets_filtering = true
advanced_observability = true

# Tier 3: LLM-powered features (default: disabled, requires LLM config)
llm_enabled = false
implicit_capture = false
consolidation_enabled = false
supersession_detection = false
temporal_reasoning = false
query_expansion = false

[llm]
# Only required if any Tier 3 feature is enabled
provider = "anthropic"  # anthropic | openai | ollama
model = "claude-3-5-sonnet"
api_key_env = "ANTHROPIC_API_KEY"  # Environment variable name
```

### 4.4 Runtime Feature Detection

```rust
/// Feature tier system for opt-out architecture
pub struct FeatureFlags {
    // Tier 2: Enhanced
    pub entity_extraction: bool,
    pub temporal_extraction: bool,
    pub hooks_enabled: bool,
    pub secrets_filtering: bool,
    pub advanced_observability: bool,

    // Tier 3: LLM-powered
    pub llm_enabled: bool,
    pub implicit_capture: bool,
    pub consolidation: bool,
    pub supersession_detection: bool,
    pub temporal_reasoning: bool,
    pub query_expansion: bool,
}

impl FeatureFlags {
    /// Load from configuration with validation
    pub fn from_config(config: &Config) -> Result<Self, ConfigError> {
        let flags = Self {
            // Tier 2 defaults to enabled
            entity_extraction: config.get_bool("features.entity_extraction").unwrap_or(true),
            temporal_extraction: config.get_bool("features.temporal_extraction").unwrap_or(true),
            hooks_enabled: config.get_bool("features.hooks_enabled").unwrap_or(true),
            secrets_filtering: config.get_bool("features.secrets_filtering").unwrap_or(true),
            advanced_observability: config.get_bool("features.advanced_observability").unwrap_or(true),

            // Tier 3 defaults to disabled
            llm_enabled: config.get_bool("features.llm_enabled").unwrap_or(false),
            implicit_capture: config.get_bool("features.implicit_capture").unwrap_or(false),
            consolidation: config.get_bool("features.consolidation_enabled").unwrap_or(false),
            supersession_detection: config.get_bool("features.supersession_detection").unwrap_or(false),
            temporal_reasoning: config.get_bool("features.temporal_reasoning").unwrap_or(false),
            query_expansion: config.get_bool("features.query_expansion").unwrap_or(false),
        };

        // Validate: Tier 3 features require LLM to be enabled
        if !flags.llm_enabled {
            if flags.implicit_capture || flags.consolidation ||
               flags.supersession_detection || flags.temporal_reasoning || flags.query_expansion {
                return Err(ConfigError::InvalidFeatureConfig(
                    "LLM features require llm_enabled = true".into()
                ));
            }
        }

        Ok(flags)
    }

    /// Check if any Tier 3 feature is active
    pub fn requires_llm(&self) -> bool {
        self.llm_enabled || self.implicit_capture || self.consolidation ||
        self.supersession_detection || self.temporal_reasoning || self.query_expansion
    }

    /// Get tier level
    pub fn tier(&self) -> FeatureTier {
        if self.requires_llm() {
            FeatureTier::LLMPowered
        } else if self.entity_extraction || self.temporal_extraction ||
                  self.hooks_enabled || self.secrets_filtering || self.advanced_observability {
            FeatureTier::Enhanced
        } else {
            FeatureTier::Core
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureTier {
    Core,
    Enhanced,
    LLMPowered,
}
```

### 4.5 Service Construction Based on Features

```rust
/// Build services based on enabled features
pub struct ServiceBuilder {
    config: Config,
    features: FeatureFlags,
}

impl ServiceBuilder {
    pub fn build_capture_service(&self) -> Result<CaptureService> {
        let storage = self.build_storage()?;
        let embedder = self.build_embedder()?;

        // Optional: Secrets filtering (Tier 2)
        let secrets_filter = if self.features.secrets_filtering {
            Some(SecretsFilter::from_config(&self.config)?)
        } else {
            None
        };

        // Optional: Implicit capture agent (Tier 3)
        let implicit_agent = if self.features.implicit_capture {
            let llm = self.build_llm_client()?;
            Some(ImplicitCaptureAgent::new(llm))
        } else {
            None
        };

        Ok(CaptureService::new(storage, embedder, secrets_filter, implicit_agent))
    }

    pub fn build_recall_service(&self) -> Result<RecallService> {
        let storage = self.build_storage()?;
        let embedder = self.build_embedder()?;

        // Optional: Entity matcher (Tier 2)
        let entity_matcher = if self.features.entity_extraction {
            Some(EntityMatcher::new()?)
        } else {
            None
        };

        // Optional: Temporal matcher (Tier 2)
        let temporal_matcher = if self.features.temporal_extraction {
            Some(TemporalMatcher::new()?)
        } else {
            None
        };

        // Optional: Query expander (Tier 3)
        let query_expander = if self.features.query_expansion {
            let llm = self.build_llm_client()?;
            Some(QueryExpander::new(llm))
        } else {
            None
        };

        // Optional: Temporal reasoning (Tier 3)
        let temporal_reasoner = if self.features.temporal_reasoning {
            let llm = self.build_llm_client()?;
            Some(TemporalReasoner::new(llm))
        } else {
            None
        };

        Ok(RecallService::new(
            storage,
            embedder,
            entity_matcher,
            temporal_matcher,
            query_expander,
            temporal_reasoner,
        ))
    }
}
```

### 4.6 Graceful Degradation Guarantees

| Scenario | Behavior | User Experience |
|----------|----------|-----------------|
| LLM provider unavailable | Tier 3 features disabled, Tier 1-2 continue | Capture/search works, no consolidation |
| Embedding model unavailable | Graceful degradation to BM25-only | Search works with keyword matching |
| OTLP endpoint unavailable | Metrics/traces buffered locally | No data loss, export when available |
| Git remote unavailable | Local operations continue | Sync deferred until available |
| Secrets filter error | Content blocked with warning | Safe default, no data leakage |

---

## 5. Functional Requirements

### 4.1 Core Memory Operations

#### FR-CAPTURE: Memory Capture

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-C01 | Capture memory with namespace, summary, content | P0 |
| FR-C02 | Store as git note with YAML front matter | P0 |
| FR-C03 | Generate embedding for semantic search | P0 |
| FR-C04 | Index in SQLite with metadata | P0 |
| FR-C05 | Support 10 namespaces (decisions, learnings, etc.) | P0 |
| FR-C06 | Validate summary ≤100 chars, content ≤100KB | P0 |
| FR-C07 | Support domain selection (project/user) | P0 |
| FR-C08 | Graceful degradation if embedding fails | P0 |
| FR-C09 | Return CaptureResult with memory ID | P0 |
| FR-C10 | Atomic file locking for concurrency | P1 |

#### FR-RECALL: Memory Recall

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-R01 | Semantic search via vector similarity | P0 |
| FR-R02 | BM25 full-text search fallback | P0 |
| FR-R03 | Hybrid search with RRF fusion | P0 |
| FR-R04 | Filter by namespace, domain, spec, tags | P0 |
| FR-R05 | Configurable result limit (default 10) | P0 |
| FR-R06 | Return MemoryResult with distance score | P0 |
| FR-R07 | Progressive hydration (summary → full → files) | P1 |
| FR-R08 | Temporal filtering (date range) | P1 |
| FR-R09 | Entity-based boosting | P2 |
| FR-R10 | LLM query expansion (opt-in) | P2 |

#### FR-SYNC: Synchronization

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-S01 | Rebuild index from git notes | P0 |
| FR-S02 | Fetch notes from remote | P0 |
| FR-S03 | Merge notes with cat_sort_uniq strategy | P0 |
| FR-S04 | Push notes to remote | P0 |
| FR-S05 | Idempotent refspec configuration | P0 |
| FR-S06 | Track sync state (last sync timestamp) | P1 |

### 4.2 Hook System

#### FR-HOOKS: Claude Code Integration

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-H01 | SessionStart: Inject context, fetch remote | P0 |
| FR-H02 | UserPromptSubmit: Detect capture markers | P0 |
| FR-H03 | PostToolUse: Surface related memories | P1 |
| FR-H04 | PreCompact: Auto-capture before compaction | P1 |
| FR-H05 | Stop: Session analysis, sync, push | P0 |
| FR-H06 | All hooks output valid JSON | P0 |
| FR-H07 | Hook timing <100ms overhead | P0 |
| FR-H08 | Adaptive token budget for context | P0 |

### 4.3 Multi-Domain Memories

#### FR-DOMAIN: Domain Management

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-D01 | PROJECT domain: repo-scoped memories | P0 |
| FR-D02 | USER domain: global cross-project memories | P0 |
| FR-D03 | User memories in separate bare git repo | P0 |
| FR-D04 | Domain markers ([global], [user]) | P0 |
| FR-D05 | Merged search across domains | P0 |
| FR-D06 | Project memories prioritized in results | P0 |

### 4.4 Subconsciousness (LLM-Powered)

#### FR-SUB: Implicit Capture & Consolidation

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-SUB01 | Provider-agnostic LLM client (Anthropic/OpenAI/Ollama) | P0 |
| FR-SUB02 | Confidence-based auto-capture (0.9+) | P1 |
| FR-SUB03 | Review queue for medium confidence (0.7-0.9) | P1 |
| FR-SUB04 | Adversarial content detection | P1 |
| FR-SUB05 | Tiered storage (HOT/WARM/COLD/ARCHIVED) | P2 |
| FR-SUB06 | Semantic clustering of related memories | P2 |
| FR-SUB07 | LLM-powered summarization of clusters | P2 |
| FR-SUB08 | Supersession detection for contradictions | P2 |
| FR-SUB09 | Memory edge relationships (SUPERSEDES, CONSOLIDATES, REFERENCES) | P2 |
| FR-SUB10 | Retention score with decay formula | P2 |

### 4.5 Security

#### FR-SEC: Secrets & PII Filtering

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-SEC01 | Detect secrets (API keys, tokens, passwords) | P0 |
| FR-SEC02 | Detect PII (SSN, credit cards, phones) | P0 |
| FR-SEC03 | Four strategies: REDACT, MASK, BLOCK, WARN | P0 |
| FR-SEC04 | Configurable strategy per secret type | P1 |
| FR-SEC05 | Allowlist for false positives | P1 |
| FR-SEC06 | SOC2/GDPR audit logging | P1 |
| FR-SEC07 | Path traversal prevention | P0 |
| FR-SEC08 | Git command injection prevention | P0 |

---

## 5. Architecture Overview

### 5.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              Memory System Architecture                         │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                 │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐               │
│  │   CLI Interface  │  │   MCP Server     │  │  Hook Handlers   │               │
│  │   (clap)         │  │   (rmcp)         │  │  (JSON output)   │               │
│  └────────┬─────────┘  └────────┬─────────┘  └────────┬─────────┘               │
│           │                     │                     │                         │
│           └─────────────────────┼─────────────────────┘                         │
│                                 │                                               │
│                                 ▼                                               │
│  ┌──────────────────────────────────────────────────────────────────────────┐   │
│  │                          Service Layer                                   │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │   │
│  │  │ Capture     │  │ Recall      │  │ Sync        │  │ Consolidation   │  │   │
│  │  │ Service     │  │ Service     │  │ Service     │  │ Service         │  │   │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └───────┬─────────┘  │   │
│  └─────────┼────────────────┼────────────────┼─────────────────┼────────────┘   │
│            │                │                │                 │                │
│            ▼                ▼                ▼                 ▼                │
│  ┌──────────────────────────────────────────────────────────────────────────┐   │
│  │                        Core Components                                   │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │   │
│  │  │ GitOps      │  │ Index       │  │ Embedding   │  │ LLM Client      │  │   │
│  │  │ (git notes) │  │ (SQLite)    │  │ (fastembed) │  │ (multi-provider)│  │   │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └───────┬─────────┘  │   │
│  └─────────┼────────────────┼────────────────┼─────────────────┼────────────┘   │
│            │                │                │                 │                │
│            ▼                ▼                ▼                 ▼                │
│  ┌──────────────────────────────────────────────────────────────────────────┐   │
│  │                      Storage Abstraction Layer                           │   │
│  │  ┌─────────────────────────────────────────────────────────────────┐     │   │
│  │  │                    trait MemoryStorage                          │     │   │
│  │  │  - insert(memory, embedding) -> Result<()>                      │     │   │
│  │  │  - search(query_embedding, k) -> Result<Vec<MemoryResult>>      │     │   │
│  │  │  - get(id) -> Result<Option<Memory>>                            │     │   │
│  │  │  - delete(id) -> Result<()>                                     │     │   │
│  │  └─────────────────────────────────────────────────────────────────┘     │   │
│  │                              │                                           │   │
│  │  ┌───────────────────────────┼───────────────────────────┐               │   │
│  │  │                           ▼                           │               │   │
│  │  │  ┌─────────────┐  ┌─────────────┐  ┌───────────────┐  │               │   │
│  │  │  │ SQLite +    │  │ LanceDB     │  │ PostgreSQL +  │  │               │   │
│  │  │  │ usearch     │  │ Backend     │  │ pgvector      │  │               │   │
│  │  │  │ (default)   │  │ (future)    │  │ (future)      │  │               │   │
│  │  │  └─────────────┘  └─────────────┘  └───────────────┘  │               │   │
│  │  └───────────────────────────────────────────────────────┘               │   │
│  └──────────────────────────────────────────────────────────────────────────┘   │
│                                                                                 │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │                        Observability Layer                              │    │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐ │    │
│  │  │ Metrics     │  │ Tracing     │  │ Logging     │  │ OTLP Export     │ │    │
│  │  │ (counters)  │  │ (spans)     │  │ (tracing)   │  │ (opentelemetry) │ │    │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────────┘ │    │
│  └─────────────────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Module Structure

```
src/
├── lib.rs                    # Library entry point
├── main.rs                   # CLI entry point
│
├── models/                   # Data structures
│   ├── mod.rs
│   ├── memory.rs            # Memory, MemoryResult, HydratedMemory
│   ├── capture.rs           # CaptureResult, CaptureAccumulator
│   ├── consolidation.rs     # MemoryTier, EdgeType, RetentionScore
│   └── llm.rs               # LLMRequest, LLMResponse, LLMError
│
├── storage/                  # Storage abstraction
│   ├── mod.rs               # MemoryStorage trait
│   ├── sqlite.rs            # SQLite + usearch backend
│   ├── lancedb.rs           # LanceDB backend (future)
│   └── postgres.rs          # PostgreSQL + pgvector (future)
│
├── services/                 # Business logic
│   ├── mod.rs
│   ├── capture.rs           # CaptureService
│   ├── recall.rs            # RecallService
│   ├── sync.rs              # SyncService
│   └── consolidation.rs     # ConsolidationService
│
├── git/                      # Git operations
│   ├── mod.rs
│   ├── notes.rs             # Git notes CRUD
│   ├── remote.rs            # Fetch/push operations
│   └── parser.rs            # YAML front matter parsing
│
├── embedding/                # Embedding generation
│   ├── mod.rs
│   ├── fastembed.rs         # FastEmbed implementation
│   └── circuit_breaker.rs   # Failure handling
│
├── llm/                      # LLM client
│   ├── mod.rs               # LLMProvider trait
│   ├── anthropic.rs         # Anthropic implementation
│   ├── openai.rs            # OpenAI implementation
│   └── ollama.rs            # Ollama implementation
│
├── hooks/                    # Claude Code hooks
│   ├── mod.rs
│   ├── session_start.rs
│   ├── user_prompt.rs
│   ├── post_tool_use.rs
│   ├── pre_compact.rs
│   └── stop.rs
│
├── mcp/                      # MCP server
│   ├── mod.rs
│   ├── server.rs            # MCP server setup
│   └── tools.rs             # Tool implementations
│
├── security/                 # Security features
│   ├── mod.rs
│   ├── secrets.rs           # Secret detection
│   ├── pii.rs               # PII detection
│   ├── redactor.rs          # Content redaction
│   └── audit.rs             # Audit logging
│
├── config/                   # Configuration
│   ├── mod.rs
│   └── env.rs               # Environment variable loading
│
└── observability/            # Telemetry
    ├── mod.rs
    ├── metrics.rs
    ├── tracing.rs
    └── otlp.rs
```

---

## 6. Data Models

### 6.1 Core Types

```rust
/// Core memory entity stored in git notes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Unique ID: {namespace}:{commit_sha}:{index} or user:{namespace}:{commit_sha}:{index}
    pub id: String,
    /// Git commit this memory is attached to
    pub commit_sha: String,
    /// Memory namespace (decisions, learnings, etc.)
    pub namespace: Namespace,
    /// One-line summary (≤100 chars)
    pub summary: String,
    /// Full markdown content (≤100KB)
    pub content: String,
    /// Capture timestamp (UTC)
    pub timestamp: DateTime<Utc>,
    /// Storage domain
    pub domain: Domain,
    /// Optional specification reference
    pub spec: Option<String>,
    /// Project phase
    pub phase: Option<String>,
    /// Categorization tags
    pub tags: Vec<String>,
    /// Memory status
    pub status: MemoryStatus,
    /// Related memory IDs
    pub relates_to: Vec<String>,
}

/// Memory search result with relevance score
#[derive(Debug, Clone)]
pub struct MemoryResult {
    pub memory: Memory,
    /// Distance from query (lower = more similar)
    pub distance: f32,
}

/// Memory namespaces
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Namespace {
    Decisions,
    Learnings,
    Blockers,
    Progress,
    Reviews,
    Patterns,
    Retrospective,
    Inception,
    Elicitation,
    Research,
}

/// Storage domain
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Domain {
    /// Repository-scoped memories
    Project,
    /// Global cross-project memories
    User,
}

/// Memory status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MemoryStatus {
    Active,
    Resolved,
    Archived,
    Tombstone,
}
```

### 6.2 Consolidation Types

```rust
/// Memory tier based on retention score
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MemoryTier {
    /// Active, included in reflexive retrieval (score ≥0.6)
    Hot,
    /// Summaries and moderate activity (score ≥0.3)
    Warm,
    /// Historical, excluded from default search
    Cold,
    /// Superseded, audit access only
    Archived,
}

/// Relationship between memories
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EdgeType {
    /// Newer memory invalidates older
    Supersedes,
    /// Summary combines multiple sources
    Consolidates,
    /// Explicit reference/link
    References,
}

/// Retention score with factor breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionScore {
    /// Combined score (0.0-1.0)
    pub overall: f32,
    /// Recency factor (exponential decay)
    pub recency: f32,
    /// Activation factor (log-scale retrieval count)
    pub activation: f32,
    /// Importance factor (namespace weight)
    pub importance: f32,
    /// Whether superseded
    pub is_superseded: bool,
    /// Penalty multiplier (1.0 or 0.2)
    pub supersession_penalty: f32,
}
```

### 6.3 LLM Types

```rust
/// LLM provider abstraction
pub trait LLMProvider: Send + Sync {
    /// Send a chat completion request
    async fn chat(&self, request: &LLMRequest) -> Result<LLMResponse, LLMError>;

    /// Provider name for logging
    fn name(&self) -> &'static str;

    /// Check if provider is available
    async fn health_check(&self) -> bool;
}

/// LLM request
#[derive(Debug, Clone)]
pub struct LLMRequest {
    pub messages: Vec<LLMMessage>,
    pub model: Option<String>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub json_mode: bool,
}

/// LLM response
#[derive(Debug, Clone)]
pub struct LLMResponse {
    pub content: String,
    pub model: String,
    pub usage: LLMUsage,
    pub latency_ms: u64,
}
```

---

## 7. MCP Tools Specification

### 7.1 Tool: memory.capture

Capture a new memory to the git-backed storage.

```json
{
  "name": "memory_capture",
  "description": "Capture a memory (decision, learning, blocker, etc.) to git-backed storage",
  "inputSchema": {
    "type": "object",
    "required": ["namespace", "summary", "content"],
    "properties": {
      "namespace": {
        "type": "string",
        "enum": ["decisions", "learnings", "blockers", "progress", "reviews", "patterns", "retrospective", "inception", "elicitation", "research"],
        "description": "Memory category/type"
      },
      "summary": {
        "type": "string",
        "maxLength": 100,
        "description": "One-line summary of the memory"
      },
      "content": {
        "type": "string",
        "maxLength": 102400,
        "description": "Full markdown content"
      },
      "domain": {
        "type": "string",
        "enum": ["project", "user"],
        "default": "project",
        "description": "Storage domain (project-scoped or global)"
      },
      "tags": {
        "type": "array",
        "items": { "type": "string" },
        "description": "Categorization tags"
      },
      "spec": {
        "type": "string",
        "description": "Optional specification reference"
      }
    }
  }
}
```

**Response**:
```json
{
  "success": true,
  "memory_id": "decisions:abc1234:0",
  "indexed": true,
  "warning": null
}
```

### 7.2 Tool: memory.recall

Search and retrieve relevant memories.

```json
{
  "name": "memory_recall",
  "description": "Search memories using semantic similarity",
  "inputSchema": {
    "type": "object",
    "required": ["query"],
    "properties": {
      "query": {
        "type": "string",
        "description": "Search query in natural language"
      },
      "limit": {
        "type": "integer",
        "default": 10,
        "minimum": 1,
        "maximum": 100,
        "description": "Maximum results to return"
      },
      "namespace": {
        "type": "string",
        "enum": ["decisions", "learnings", "blockers", "progress", "reviews", "patterns"],
        "description": "Filter by namespace"
      },
      "domain": {
        "type": "string",
        "enum": ["all", "project", "user"],
        "default": "all",
        "description": "Search scope"
      },
      "mode": {
        "type": "string",
        "enum": ["hybrid", "vector", "bm25"],
        "default": "hybrid",
        "description": "Search strategy"
      },
      "min_similarity": {
        "type": "number",
        "minimum": 0,
        "maximum": 1,
        "description": "Minimum similarity threshold"
      }
    }
  }
}
```

**Response**:
```json
{
  "results": [
    {
      "id": "decisions:abc1234:0",
      "namespace": "decisions",
      "summary": "Use PostgreSQL for main database",
      "content": "...",
      "timestamp": "2025-01-15T10:30:00Z",
      "domain": "project",
      "distance": 0.15,
      "tags": ["database", "architecture"]
    }
  ],
  "total": 1
}
```

### 7.3 Tool: memory.status

Get memory system status and statistics.

```json
{
  "name": "memory_status",
  "description": "Get memory system status and statistics",
  "inputSchema": {
    "type": "object",
    "properties": {
      "domain": {
        "type": "string",
        "enum": ["all", "project", "user"],
        "default": "all"
      }
    }
  }
}
```

**Response**:
```json
{
  "total_memories": 150,
  "by_namespace": {
    "decisions": 45,
    "learnings": 80,
    "blockers": 10,
    "progress": 15
  },
  "by_domain": {
    "project": 120,
    "user": 30
  },
  "index_size_bytes": 1048576,
  "last_sync": "2025-01-20T15:30:00Z"
}
```

### 7.4 Tool: memory.sync

Synchronize memories with git remote.

```json
{
  "name": "memory_sync",
  "description": "Synchronize memory index with git notes and optionally remote",
  "inputSchema": {
    "type": "object",
    "properties": {
      "remote": {
        "type": "boolean",
        "default": false,
        "description": "Also sync with git remote"
      },
      "domain": {
        "type": "string",
        "enum": ["all", "project", "user"],
        "default": "all"
      }
    }
  }
}
```

### 7.5 Tool: memory.consolidate

Trigger memory consolidation (requires LLM).

```json
{
  "name": "memory_consolidate",
  "description": "Run consolidation to summarize and tier memories",
  "inputSchema": {
    "type": "object",
    "properties": {
      "dry_run": {
        "type": "boolean",
        "default": true,
        "description": "Preview changes without persisting"
      },
      "full": {
        "type": "boolean",
        "default": false,
        "description": "Full consolidation (vs incremental)"
      }
    }
  }
}
```

### 7.6 Tool: memory.configure

Runtime configuration management.

```json
{
  "name": "memory_configure",
  "description": "View or update memory system configuration",
  "inputSchema": {
    "type": "object",
    "properties": {
      "action": {
        "type": "string",
        "enum": ["get", "set"],
        "default": "get"
      },
      "key": {
        "type": "string",
        "description": "Configuration key (e.g., 'hook.session_start.enabled')"
      },
      "value": {
        "type": "string",
        "description": "Value to set (for 'set' action)"
      }
    }
  }
}
```

---

## 8. Pluggable Storage System

### 8.1 Storage Trait

```rust
/// Core storage abstraction for memory persistence and search
#[async_trait]
pub trait MemoryStorage: Send + Sync {
    /// Insert a memory with its embedding
    async fn insert(&self, memory: &Memory, embedding: &[f32]) -> Result<()>;

    /// Search by vector similarity
    async fn search_vector(
        &self,
        query_embedding: &[f32],
        k: usize,
        filter: Option<&SearchFilter>,
    ) -> Result<Vec<MemoryResult>>;

    /// Search by full-text (BM25)
    async fn search_text(
        &self,
        query: &str,
        k: usize,
        filter: Option<&SearchFilter>,
    ) -> Result<Vec<MemoryResult>>;

    /// Get memory by ID
    async fn get(&self, id: &str) -> Result<Option<Memory>>;

    /// Delete memory by ID
    async fn delete(&self, id: &str) -> Result<()>;

    /// Get storage statistics
    async fn stats(&self) -> Result<StorageStats>;

    /// Initialize storage (create tables, indexes)
    async fn initialize(&self) -> Result<()>;

    /// Run schema migrations
    async fn migrate(&self) -> Result<()>;
}

/// Search filter criteria
#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    pub namespace: Option<Namespace>,
    pub domain: Option<Domain>,
    pub spec: Option<String>,
    pub tags: Option<Vec<String>>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub min_similarity: Option<f32>,
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_memories: u64,
    pub by_namespace: HashMap<Namespace, u64>,
    pub by_domain: HashMap<Domain, u64>,
    pub index_size_bytes: u64,
    pub last_sync: Option<DateTime<Utc>>,
}
```

### 8.2 Default Backend: SQLite + usearch

```rust
/// SQLite + usearch storage implementation
pub struct SqliteStorage {
    conn: Connection,
    index: usearch::Index,
    db_path: PathBuf,
}

impl SqliteStorage {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let conn = Connection::open(&db_path)?;

        // Configure SQLite
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA busy_timeout=5000;"
        )?;

        // Initialize usearch index
        let index_path = db_path.with_extension("usearch");
        let index = usearch::Index::new(&usearch::IndexOptions {
            dimensions: 384,  // MiniLM embedding size
            metric: usearch::MetricKind::Cos,
            quantization: usearch::ScalarKind::F32,
            ..Default::default()
        })?;

        Ok(Self { conn, index, db_path })
    }
}

#[async_trait]
impl MemoryStorage for SqliteStorage {
    async fn insert(&self, memory: &Memory, embedding: &[f32]) -> Result<()> {
        // Insert metadata into SQLite
        self.conn.execute(
            "INSERT INTO memories (id, namespace, summary, content, timestamp, domain, spec, tags, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                memory.id,
                memory.namespace.as_str(),
                memory.summary,
                memory.content,
                memory.timestamp.to_rfc3339(),
                memory.domain.as_str(),
                memory.spec,
                serde_json::to_string(&memory.tags)?,
                memory.status.as_str(),
            ],
        )?;

        // Insert embedding into usearch
        let key = self.memory_id_to_key(&memory.id)?;
        self.index.add(key, embedding)?;

        Ok(())
    }

    async fn search_vector(
        &self,
        query_embedding: &[f32],
        k: usize,
        filter: Option<&SearchFilter>,
    ) -> Result<Vec<MemoryResult>> {
        // Search usearch index
        let results = self.index.search(query_embedding, k * 3)?;  // Over-fetch for filtering

        // Fetch metadata and apply filters
        let mut memories = Vec::with_capacity(k);
        for (key, distance) in results {
            let id = self.key_to_memory_id(key)?;
            if let Some(memory) = self.get(&id).await? {
                if self.matches_filter(&memory, filter) {
                    memories.push(MemoryResult { memory, distance });
                    if memories.len() >= k {
                        break;
                    }
                }
            }
        }

        Ok(memories)
    }

    // ... other methods
}
```

### 8.3 Future Backends

**LanceDB Backend** (for columnar storage with versioning):
```rust
pub struct LanceStorage {
    db: lancedb::Connection,
    table: lancedb::Table,
}
```

**PostgreSQL + pgvector Backend** (for multi-user deployment):
```rust
pub struct PostgresStorage {
    pool: sqlx::PgPool,
}
```

### 8.4 Backend Selection

```rust
/// Create storage backend from configuration
pub fn create_storage(config: &Config) -> Result<Arc<dyn MemoryStorage>> {
    match config.storage_backend.as_str() {
        "sqlite" | "" => Ok(Arc::new(SqliteStorage::new(config.data_dir.join("index.db"))?)),
        "lancedb" => Ok(Arc::new(LanceStorage::new(&config.lance_uri)?)),
        "postgres" => Ok(Arc::new(PostgresStorage::new(&config.postgres_url).await?)),
        other => Err(anyhow!("Unknown storage backend: {}", other)),
    }
}
```

---

## 9. Performance Requirements

### 9.1 Latency Targets

| Operation | Target | Measurement |
|-----------|--------|-------------|
| Cold start | <10ms | Binary load to first operation |
| Capture pipeline | <30ms | Validate + git note + embed + index |
| Vector search (10K memories) | <50ms | Query embedding + KNN + hydrate |
| BM25 search (10K memories) | <20ms | FTS5 query + hydrate |
| Hybrid search (10K memories) | <80ms | Vector + BM25 + RRF fusion |
| Hook overhead | <100ms | Total hook execution time |
| SessionStart context | <2000ms | Memory fetch + context build |
| Embedding generation | <20ms | Single text → 384d vector |

### 9.2 Throughput Targets

| Metric | Target |
|--------|--------|
| Concurrent captures | 100/s |
| Concurrent searches | 500/s |
| Memory capacity | 100K+ memories |
| Embedding batch | 100 texts/s |

### 9.3 Resource Constraints

| Resource | Limit |
|----------|-------|
| Binary size | <100MB |
| Memory (idle) | <50MB |
| Memory (active) | <500MB |
| Disk (per 10K memories) | ~100MB |

---

## 10. Security Requirements

### 10.1 Secret Detection Patterns

| Type | Pattern | Action |
|------|---------|--------|
| API Keys | `sk-[a-zA-Z0-9]{32,}` | REDACT |
| AWS Keys | `AKIA[A-Z0-9]{16}` | REDACT |
| Private Keys | `-----BEGIN.*PRIVATE KEY-----` | BLOCK |
| Passwords | `password\s*[:=]\s*['"][^'"]+['"]` | REDACT |
| JWT Tokens | `eyJ[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+` | MASK |
| Database URLs | `(postgres|mysql|mongodb)://[^@]+@` | MASK |

### 10.2 PII Detection

| Type | Validation | Action |
|------|------------|--------|
| SSN | Format + checksum | REDACT |
| Credit Cards | Luhn algorithm | REDACT |
| Phone Numbers | E.164 format | MASK |
| Email | RFC 5322 | WARN |

### 10.3 Security Controls

```rust
/// Filter strategies
#[derive(Debug, Clone, Copy)]
pub enum FilterStrategy {
    /// Replace with [REDACTED:type]
    Redact,
    /// Show partial content (abc...xyz)
    Mask,
    /// Reject content entirely
    Block,
    /// Log warning but pass through
    Warn,
}

/// Security configuration
pub struct SecurityConfig {
    pub enabled: bool,
    pub default_strategy: FilterStrategy,
    pub entropy_detection: bool,
    pub pii_detection: bool,
    pub audit_logging: bool,
    pub allowlist_path: PathBuf,
}
```

---

## 11. Observability Requirements

### 11.1 Metrics

| Metric | Type | Labels |
|--------|------|--------|
| `memory_captures_total` | Counter | namespace, domain, status |
| `memory_searches_total` | Counter | mode, domain |
| `memory_search_latency_ms` | Histogram | mode |
| `memory_capture_latency_ms` | Histogram | namespace |
| `memory_embedding_latency_ms` | Histogram | - |
| `memory_hook_latency_ms` | Histogram | hook_type |
| `memory_index_size_bytes` | Gauge | domain |
| `memory_total_count` | Gauge | namespace, domain |

### 11.2 Tracing

Distributed tracing via OpenTelemetry:

```rust
#[tracing::instrument(skip(content), fields(namespace = %namespace))]
pub async fn capture(
    &self,
    namespace: Namespace,
    summary: &str,
    content: &str,
) -> Result<CaptureResult> {
    let span = tracing::info_span!("embed");
    let embedding = span.in_scope(|| self.embedder.embed(content))?;

    let span = tracing::info_span!("index");
    span.in_scope(|| self.storage.insert(&memory, &embedding))?;

    Ok(result)
}
```

### 11.3 OTLP Export

```rust
// Initialize OTLP exporter
let tracer = opentelemetry_otlp::new_pipeline()
    .tracing()
    .with_exporter(
        opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(&config.otlp_endpoint)
    )
    .install_batch(opentelemetry_sdk::runtime::Tokio)?;
```

---

## 12. Rust Ecosystem Mapping

### 12.1 Primary Dependencies

| Function | Python | Rust | Rationale |
|----------|--------|------|-----------|
| Embeddings | sentence-transformers | `fastembed` | Direct MiniLM support |
| Vector Search | sqlite-vec | `usearch` | High performance, single-file |
| Git Operations | GitPython | `git2` | Full notes API |
| LLM (OpenAI) | openai | `async-openai` | Well-maintained |
| LLM (Anthropic) | anthropic | Custom | Official SDK immature |
| Serialization | PyYAML | `serde_yml` | Active fork |
| CLI | click | `clap` | Industry standard |
| Async | asyncio | `tokio` | De facto standard |
| Database | sqlite3 | `rusqlite` | Direct SQLite |
| Observability | opentelemetry | `tracing` + `opentelemetry` | Standard combo |
| MCP | FastMCP | `rmcp` | Official-feeling SDK |

### 12.2 Cargo.toml

```toml
[package]
name = "memory"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[dependencies]
# Core
tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
thiserror = "2.0"

# Embeddings
fastembed = "5"

# Vector Search
usearch = "2"

# Git Operations
git2 = "0.20"

# LLM Clients
async-openai = "0.32"
reqwest = { version = "0.12", features = ["json"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yml = "0.0.12"

# Database
rusqlite = { version = "0.38", features = ["bundled"] }

# CLI
clap = { version = "4", features = ["derive"] }

# Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
opentelemetry = "0.31"
opentelemetry_sdk = { version = "0.31", features = ["rt-tokio"] }
opentelemetry-otlp = { version = "0.31", features = ["tonic"] }

# MCP Server
rmcp = { version = "0.12", features = ["server", "transport-stdio"] }

# Time
chrono = { version = "0.4", features = ["serde"] }

[features]
default = []
consolidation = []  # LLM-powered features
tui = ["ratatui", "crossterm"]

[dev-dependencies]
tempfile = "3"
tokio-test = "0.4"
criterion = "0.6"

[[bench]]
name = "benchmarks"
harness = false
```

---

## 13. Migration Path

### 13.1 Data Compatibility

**Git Notes**: Fully compatible. YAML front matter format unchanged.

```yaml
---
type: decisions
timestamp: 2025-01-15T10:30:00Z
summary: Use PostgreSQL for persistence
spec: my-project
tags: [database, architecture]
domain: project
---
## Context
We needed a database...
```

**SQLite Index**: Requires rebuild (schema differences). Migration tool provided.

### 13.2 Migration Steps

1. **Install Rust binary** alongside Python
2. **Run migration tool**: `memory migrate --from-python`
3. **Verify data**: `memory status --verify`
4. **Update hooks**: Point to Rust binary
5. **Remove Python**: `pip uninstall git-notes-memory`

### 13.3 Backwards Compatibility

- Git notes format unchanged (Python can read Rust-created notes)
- Environment variables unchanged
- Hook JSON output format unchanged
- MCP tools are additive (new capability)

---

## 14. Phasing and Milestones

### Phase 1: Core Foundation (MVP)

**Deliverables**:
- Memory capture and storage
- Vector search with fastembed
- Git notes integration
- CLI with capture/recall/status/sync
- 80%+ test coverage
- All performance targets met

**Definition of Done**:
- [ ] Can capture memory via CLI
- [ ] Can search memories semantically
- [ ] Git notes properly created
- [ ] Sync with local git notes works
- [ ] Performance benchmarks pass

### Phase 2: Hook Integration

**Deliverables**:
- SessionStart context injection
- UserPromptSubmit marker detection
- Stop hook sync/push
- PostToolUse memory surfacing
- PreCompact auto-capture
- Hook JSON output validation

**Definition of Done**:
- [ ] All 5 hooks functional
- [ ] Hook timing <100ms
- [ ] JSON output valid on all paths
- [ ] Integration tests pass

### Phase 3: MCP Server

**Deliverables**:
- MCP server with rmcp
- All 6 tools implemented
- stdio transport
- Tool documentation

**Definition of Done**:
- [ ] memory.capture works
- [ ] memory.recall works
- [ ] memory.status works
- [ ] memory.sync works
- [ ] memory.consolidate works
- [ ] memory.configure works

### Phase 4: Advanced Features

**Deliverables**:
- Multi-domain memories
- Hybrid search (RRF fusion)
- Secrets filtering
- Remote sync (fetch/push)
- Audit logging

**Definition of Done**:
- [ ] User domain captures work
- [ ] Hybrid search improves accuracy
- [ ] Secrets properly redacted
- [ ] Remote sync functional
- [ ] Audit logs generated

### Phase 5: Subconsciousness (Optional)

**Deliverables**:
- LLM client abstraction
- Confidence-based auto-capture
- Tiered storage
- Memory consolidation

**Definition of Done**:
- [ ] Provider-agnostic LLM client
- [ ] Auto-capture with confidence
- [ ] Tier assignment working
- [ ] Consolidation produces summaries

---

## 15. Success Criteria

### 15.1 Quantitative

| Metric | Target | Measurement |
|--------|--------|-------------|
| Test Coverage | ≥80% | cargo tarpaulin |
| Capture Latency | <30ms | p99 in benchmarks |
| Search Latency | <50ms | p99 in benchmarks |
| Binary Size | <100MB | release build |
| Cold Start | <10ms | time to first op |

### 15.2 Qualitative

- [ ] All Python features have Rust equivalents
- [ ] Documentation complete (README, CLI help, API docs)
- [ ] No data corruption in stress tests
- [ ] Graceful degradation for all failure modes
- [ ] Clean architecture (no circular dependencies)

### 15.3 User Acceptance

- [ ] Existing users can migrate without data loss
- [ ] CLI UX matches or exceeds Python version
- [ ] MCP tools work with Claude Desktop
- [ ] Performance improvement noticeable

---

## 16. Appendix: Lessons Learned from Python POC

### 16.1 What Worked Well (Preserve)

1. **Frozen Data Structures**: Immutability prevented bugs. Use Rust's ownership model.

2. **Service Factory Pattern**: Lazy initialization with singletons. Use `lazy_static!` or `once_cell`.

3. **Graceful Degradation**: Features fail open. Design every feature with fallback.

4. **Test Discipline**: 87%+ coverage. Integration tests caught real bugs.

5. **Adaptive Token Budgets**: Scale context to project complexity.

6. **XML Context Format**: Structured prompts improve Claude output.

### 16.2 What Didn't Work (Change)

1. **Git Version Assumptions**: Detect dynamically, don't assume.

2. **Hook JSON Output**: Template-based generation to ensure validity.

3. **Embedding Download UX**: Show progress bars.

4. **Documentation Timing**: Document alongside features, not after.

5. **Performance Testing**: Establish benchmarks from day 1.

### 16.3 Critical Constraints (Respect)

1. **Signal Detection**: <50ms
2. **Capture Pipeline**: <10ms (achieved <5ms)
3. **SessionStart**: <2000ms
4. **Post-Tool Injection**: <100ms
5. **Test Coverage**: ≥80%

### 16.4 Technical Debt to Avoid

1. **Model loading without progress**
2. **Lock contention at scale**
3. **Hook path coupling**
4. **Manual cache sync**

---

## Appendix A: Configuration Reference

### Environment Variables

```bash
# Core
MEMORY_DATA_DIR="$HOME/.local/share/memory"
MEMORY_GIT_NAMESPACE="refs/notes/mem"
MEMORY_EMBEDDING_MODEL="all-MiniLM-L6-v2"
MEMORY_STORAGE_BACKEND="sqlite"  # sqlite, lancedb, postgres

# Hooks
HOOK_ENABLED=true
HOOK_SESSION_START_ENABLED=true
HOOK_SESSION_START_FETCH_REMOTE=false
HOOK_USER_PROMPT_ENABLED=false
HOOK_POST_TOOL_USE_ENABLED=true
HOOK_PRE_COMPACT_ENABLED=true
HOOK_STOP_ENABLED=true
HOOK_STOP_PUSH_REMOTE=false

# Security
SECRETS_FILTER_ENABLED=true
SECRETS_FILTER_STRATEGY=redact  # redact, mask, block, warn
SECRETS_FILTER_PII_ENABLED=true
SECRETS_FILTER_AUDIT_ENABLED=true

# LLM (for subconsciousness features)
LLM_PROVIDER=anthropic  # anthropic, openai, ollama
LLM_MODEL=  # Provider default if empty
LLM_TEMPERATURE=0.3
LLM_MAX_TOKENS=1000

# Observability
MEMORY_OTLP_ENDPOINT=  # http://localhost:4318
MEMORY_LOG_LEVEL=info  # quiet, info, debug, trace
MEMORY_LOG_FORMAT=json  # json, text
MEMORY_METRICS_ENABLED=true
MEMORY_TRACING_ENABLED=true

# Consolidation (optional feature)
CONSOLIDATION_ENABLED=true
CONSOLIDATION_INTERVAL_HOURS=24
TIER_THRESHOLD_HOT=0.7
TIER_THRESHOLD_WARM=0.4
```

---

## Appendix B: CLI Reference

```bash
# Capture
memory capture decisions "Use PostgreSQL" --content "Due to JSONB support..."
memory capture learnings "TIL pytest -k filters" --domain user
memory capture blockers "API rate limiting" --spec my-project

# Recall
memory recall "database decisions"
memory recall --namespace decisions --limit 5
memory recall --domain user --mode hybrid

# Status
memory status
memory status --domain project

# Sync
memory sync
memory sync --remote --push

# Consolidate
memory consolidate --dry-run
memory consolidate --full

# Configuration
memory config get hook.session_start.enabled
memory config set hook.stop.push_remote true

# MCP Server
memory serve --transport stdio
memory serve --transport sse --port 8080
```

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-28 | Claude Opus 4.5 | Initial PRD |
| 2.0.0 | 2025-12-28 | Claude Opus 4.5 | Added feature tiers, pluggable storage, comprehensive specs |
| 2.1.0 | 2025-12-28 | Claude Opus 4.5 | Added storage/observability, MCP/LLM, access interfaces, integration |

---

**Next Steps**:
1. Review and approve PRD with user
2. Create Rust project skeleton
3. Begin Phase 1 implementation
4. Establish CI/CD pipeline with coverage gates

---

## Related Specification Documents

This PRD is part of a comprehensive specification suite. **All documents are mandatory reading before implementation.**

| Document | Purpose | Key Contents |
|----------|---------|--------------|
| **[PRD.md](./PRD.md)** (this document) | Core requirements and architecture | Goals, user stories, data models, phasing |
| **[STORAGE_AND_OBSERVABILITY.md](./STORAGE_AND_OBSERVABILITY.md)** | Storage backends and telemetry | Three-layer storage, all backends, OTLP export |
| **[MCP_RESOURCES_AND_LLM.md](./MCP_RESOURCES_AND_LLM.md)** | MCP integration and LLM providers | URN scheme, domain hierarchy, provider implementations |
| **[ACCESS_INTERFACES.md](./ACCESS_INTERFACES.md)** | All access methods | CLI, MCP server, streaming API, hooks |
| **[SEAMLESS_INTEGRATION.md](./SEAMLESS_INTEGRATION.md)** | Feature composition | Event bus, pipelines, error propagation, testing |
| **[RESEARCH_PLAN.md](./RESEARCH_PLAN.md)** | Research methodology | Research phases, subagent delegation, quality gates |

### Specification Summary

```
┌─────────────────────────────────────────────────────────────────┐
│                    SPECIFICATION SUITE                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  PRD.md ─────────────────► Core requirements, architecture      │
│     │                                                           │
│     ├── STORAGE_AND_OBSERVABILITY.md                            │
│     │   └── Git Notes, SQLite, PostgreSQL, Redis backends       │
│     │   └── Metrics, tracing, logging, OTLP export              │
│     │                                                           │
│     ├── MCP_RESOURCES_AND_LLM.md                                │
│     │   └── subcog://{domain}/{namespace}/{id} URNs         │
│     │   └── Anthropic, OpenAI, Ollama, LMStudio providers       │
│     │                                                           │
│     ├── ACCESS_INTERFACES.md                                    │
│     │   └── CLI commands and arguments                          │
│     │   └── MCP server with tools, resources, prompts           │
│     │   └── SSE/WebSocket streaming                             │
│     │   └── Hook system (all 5 Claude Code hooks)               │
│     │                                                           │
│     └── SEAMLESS_INTEGRATION.md                                 │
│         └── Event bus architecture                              │
│         └── Pipeline composition                                │
│         └── Error propagation and recovery                      │
│         └── Feature matrix testing                              │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Document Dependencies

```
RESEARCH_PLAN.md
    └── Informs all specification documents

PRD.md (v2.1.0)
    ├── STORAGE_AND_OBSERVABILITY.md
    │   └── Details for §8 (Storage) and §11 (Observability)
    ├── MCP_RESOURCES_AND_LLM.md
    │   └── Details for §7 (MCP Tools) and LLM providers
    ├── ACCESS_INTERFACES.md
    │   └── Details for CLI, MCP, hooks, streaming
    └── SEAMLESS_INTEGRATION.md
        └── Details for feature composition and testing
```
