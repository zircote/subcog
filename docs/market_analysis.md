# Subcog Competitive Analysis: Feature Parity with Industry AI Memory Tools

**Date:** 2026-01-22
**Version:** 1.2
**Status:** Updated

---

## Executive Summary

Subcog is a **mature, feature-rich memory system** that exceeds many competitors in key areas (observability, security, CLI tooling, graph memory). Recent development has closed critical gaps in knowledge graphs, entity extraction, memory expiration, and context templates. Remaining gaps are primarily in multimodal support and webhooks.

### Key Findings

| Category | Status |
|----------|--------|
| Core CRUD Operations | ✅ Full parity |
| Search & Retrieval | ✅ Exceeds (hybrid search) |
| Graph Memory | ✅ Full implementation |
| Entity Extraction | ✅ Auto-extraction enabled |
| Context Templates | ✅ Full implementation |
| Memory Expiration | ✅ TTL support |
| Multimodal Support | ❌ Critical gap |
| Webhooks | ❌ Missing |
| Security & Compliance | ✅ Industry-leading |
| Observability | ✅ Industry-leading |

---

## Competitive Landscape

### Industry Players Analyzed

| Tool | Focus | Deployment | Primary Users | Funding |
|------|-------|------------|---------------|---------|
| **Mem0** | General AI memory layer | Cloud (SaaS) | Startups, Enterprise | $24M Series A (Oct 2025) |
| **Zep** | Knowledge graphs + RAG | Cloud (SaaS) | Enterprise | - |
| **Letta** | Self-editing memory (MemGPT) | Hybrid | Developers, Enterprise | $10M Seed (2024) |
| **Cognee** | ECL pipeline + CodeGraph | Self-hosted | Developers | - |
| **LangMem** | LangChain ecosystem | Self-hosted | Developers | - |
| **Graphlit** | Managed connectors | Cloud (SaaS) | Enterprise | - |
| **Supermemory** | Multimodal memory | Cloud (SaaS) | Consumers, Developers | $2.6M Seed (Oct 2025) |
| **Subcog** | Code assistant memory | Self-hosted | Developers, Teams | Open Source |

### Key Benchmarks

| Provider | DMR Score | Latency | Notable |
|----------|-----------|---------|---------|
| **Zep** | 94.8% | 200ms | Best accuracy |
| **Mem0** | 66.9% | 150ms p95 | Best latency |
| **Letta** | 74% LoCoMo | - | Open source leader |

---

## Feature Comparison Matrix

| Feature Category | Subcog | Mem0 | Zep | Letta | Cognee | LangMem |
|-----------------|--------|------|-----|-------|--------|---------|
| **Core CRUD** | ✅ Full | ✅ Full | ✅ Full | ✅ Full | ✅ Full | ✅ Full |
| **Semantic Search** | ✅ Hybrid | ✅ Advanced | ✅ Graph RAG | ✅ Archival | ✅ ECL | ✅ Basic |
| **Graph Memory** | ✅ Temporal KG | ✅ Native | ✅ Temporal KG | ⚠️ Basic | ✅ Full | ❌ None |
| **Self-Editing Memory** | ❌ Missing | ❌ Missing | ❌ Missing | ✅ Native | ❌ Missing | ❌ Missing |
| **Multimodal** | ❌ Missing | ✅ Images/Docs | ❌ None | ❌ None | ⚠️ Partial | ❌ None |
| **Memory Expiration** | ✅ TTL | ✅ TTL | ❌ None | ❌ None | ❌ None | ❌ None |
| **Conflict Resolution** | ❌ Missing | ✅ Graph-based | ✅ Temporal | ✅ LLM-powered | ❌ Missing | ❌ Missing |
| **Multi-Agent Coord** | ❌ Missing | ⚠️ Partial | ⚠️ Partial | ✅ Full | ⚠️ Partial | ⚠️ Partial |
| **Bi-Temporal Model** | ❌ Missing | ❌ Missing | ✅ Full | ❌ Missing | ✅ Full | ❌ Missing |
| **CodeGraph Layer** | ❌ Missing | ❌ Missing | ❌ Missing | ❌ Missing | ✅ Full | ❌ Missing |
| **Agent Dev Environment** | ❌ Missing | ❌ Missing | ⚠️ Basic | ✅ ADE | ❌ Missing | ❌ Missing |
| **Managed Connectors** | ❌ Missing | ⚠️ Limited | ⚠️ Limited | ❌ Missing | ⚠️ Limited | ❌ Missing |
| **Entity Extraction** | ✅ Auto | ✅ Auto | ✅ Auto | ✅ Auto | ✅ Auto | ✅ Auto |
| **Context Templates** | ✅ Full | ⚠️ Partial | ✅ Full | ✅ Blocks | ⚠️ Partial | ❌ None |
| **MCP Integration** | ✅ Native | ✅ Native | ✅ Native | ✅ Native | ✅ Native | ❌ None |
| **Local/Offline** | ✅ Full | ❌ Cloud | ❌ Cloud | ✅ Full | ✅ Full | ✅ InMemory |
| **CLI Tools** | ✅ Rich | ⚠️ Basic | ✅ zepctl | ✅ letta | ⚠️ Basic | ❌ None |
| **Observability** | ✅ Full OTLP | ⚠️ Basic | ⚠️ Audit | ⚠️ Logs | ⚠️ Basic | ❌ None |

---

## Gap Analysis

### 🔴 Critical Gaps (High Priority)

#### 1. Self-Editing Memory ([#75](https://github.com/zircote/subcog/issues/75))

**Industry Standard (Letta/MemGPT):**
- LLM autonomously decides when to store/retrieve/update memories
- Memory tools: `memory_replace`, `memory_insert`, `memory_rethink`
- Context window "compiled" from existing DB state on each request

**Subcog Current State:**
- Requires explicit `subcog_capture` commands
- Hook-based automation available but not autonomous

**Recommendation:**
- Priority: **CRITICAL**
- Impact: Major friction reduction, Letta differentiator

---

#### 2. Memory Conflict Detection/Resolution ([#76](https://github.com/zircote/subcog/issues/76))

**Industry Standard (Mem0g, Zep):**
- LLM-powered resolution for contradictory memories
- Strategies: Add, Merge, Invalidate, Skip
- 2% accuracy improvement reported by Mem0g

**Subcog Current State:**
- No conflict detection mechanism
- Contradictory memories can coexist

**Recommendation:**
- Priority: **CRITICAL**
- Impact: Essential for production reliability

---

#### 3. Multi-Agent Memory Coordination ([#77](https://github.com/zircote/subcog/issues/77))

**Industry Standard (Letta, Research):**
- Memory layers: agent-private, team-shared, blackboard
- Consensus mechanisms for multi-agent decisions
- Provenance tracking per agent

**Subcog Current State:**
- No multi-agent coordination patterns
- Basic domain scoping only

**Recommendation:**
- Priority: **CRITICAL**
- Impact: Growing multi-agent market segment

---

#### 4. Collaborative Memory with ACLs ([#78](https://github.com/zircote/subcog/issues/78))

**Industry Standard (Enterprise):**
- Fine-grained sharing of specific memories
- Time-bounded access permissions
- Compliance audit trails

**Subcog Current State:**
- Static project/user/org scoping
- No per-memory access controls

**Recommendation:**
- Priority: **CRITICAL**
- Impact: Enterprise compliance requirements

---

<!-- #### 1. Graph Memory / Knowledge Graphs

**Industry Standard (Mem0, Zep):**
- Temporal knowledge graphs with entity-relationship modeling
- Graph RAG for context-aware retrieval
- Entity extraction (people, orgs, concepts, events)
- Relationship tracking ("who did what, when, and with whom")

**Subcog Current State:**
- Only edge relationships for consolidation (`SummarizedBy`, `SourceOf`, `RelatedTo`)
- No entity extraction or knowledge graph construction
- No graph-based querying

**Recommendation:**
- Priority: **HIGH**
- Effort: Large (4-6 weeks)
- Impact: Major competitive differentiator

**Implementation Plan:**
1. Add EntityExtraction service using LLM
2. Build knowledge graph layer with Neo4j/SQLite graph extensions
3. Add temporal tracking (when relationships formed/changed)
4. Implement Graph RAG retrieval alongside hybrid search
5. New MCP tools: `subcog_entities`, `subcog_relationships`, `subcog_graph_query` -->

---

#### 1. Multimodal Memory Support

**Industry Standard (Mem0):**
- Store images and documents alongside text
- Visual context in memory retrieval
- Document parsing and indexing

**Subcog Current State:**
- Text-only memory content
- No image/document support

**Recommendation:**
- Priority: **HIGH**
- Effort: Medium (2-3 weeks)
- Impact: Essential for modern AI agents

**Implementation Plan:**
1. Add MIME type field to Memory model
2. Integrate vision models for image captioning (Claude, GPT-4V)
3. Add document parsing (PDF, DOCX) with content extraction
4. Store embeddings for multimodal content
5. New tools: `subcog_capture_image`, `subcog_capture_document`

---

<!-- #### 3. Memory Expiration / TTL

**Industry Standard (Mem0):**
- Set expiration dates on memories
- Auto-cleanup of expired entries
- Temporal information management

**Subcog Current State:**
- No TTL support
- Manual deletion only
- Tombstone pattern for soft delete (but no auto-purge)

**Recommendation:**
- Priority: **HIGH**
- Effort: Small (1 week)
- Impact: Critical for production deployments

**Implementation Plan:**
1. Add `expires_at: Option<DateTime>` to Memory model
2. Add background job for expired memory cleanup
3. Add `--ttl` flag to capture command
4. New tool parameter: `subcog_capture --expires "7d"`
5. Add expiration filters to recall 

---

 #### 2. Webhooks / Event Notifications

**Industry Standard (Mem0, Zep):**
- Real-time notifications for memory events
- Integration with external systems
- Custom triggers on capture/update/delete

**Subcog Current State:**
- Event logging only (observability)
- No webhook support
- No external notification system

**Recommendation:**
- Priority: **HIGH**
- Effort: Medium (2 weeks)
- Impact: Essential for enterprise integrations

**Implementation Plan:**
1. Add webhook configuration (URL, events, auth)
2. Implement async webhook delivery with retry
3. Support events: captured, updated, deleted, consolidated
4. Add webhook management CLI: `subcog webhook add/list/delete`
5. New MCP resource: `subcog://webhooks` 

---

### 🟡 Important Gaps (Medium Priority)

#### 5. Agent Development Environment ([#79](https://github.com/zircote/subcog/issues/79))

**Industry Standard (Letta ADE):**
- Real-time event history visualization
- Memory block editing in browser
- "White-box memory" - see exactly what LLM receives

**Subcog Current State:**
- CLI-only memory inspection
- No real-time visualization

**Recommendation:**
- Priority: **HIGH**
- Impact: Major developer experience differentiator

---

#### 6. Bi-Temporal Model ([#80](https://github.com/zircote/subcog/issues/80))

**Industry Standard (Zep Graphiti):**
- Track valid time (t_valid) and ingestion time (t_ingested)
- Historical queries: "What did we know at time T?"
- 18.5% accuracy improvement on temporal queries

**Subcog Current State:**
- Only created_at and updated_at timestamps
- No point-in-time queries

**Recommendation:**
- Priority: **HIGH**
- Impact: Compliance and audit requirements

---

#### 7. CodeGraph Layer ([#81](https://github.com/zircote/subcog/issues/81))

**Industry Standard (Cognee):**
- AST-aware entity extraction
- Dependency graph construction
- Symbol-to-memory linking

**Subcog Current State:**
- Text-only memory, no code structure awareness
- File renames break references

**Recommendation:**
- Priority: **HIGH**
- Impact: Aligns with "developer's AI memory" positioning

---

#### 8. Memory Portability Standard ([#82](https://github.com/zircote/subcog/issues/82))

**Industry Standard:**
- No current interoperability standard exists
- Vendor lock-in concerns emerging

**Subcog Opportunity:**
- Define open Memory Interchange Format (MIF)
- Lead standardization effort
- Differentiate from cloud-locked competitors

**Recommendation:**
- Priority: **HIGH**
- Impact: Strategic market positioning opportunity

---

<!-- #### 5. Automatic Entity Extraction

**Industry Standard (Mem0, Zep, LangMem):**
- Auto-extract entities (people, organizations, concepts)
- Build entity-to-memory relationships
- Entity-scoped retrieval

**Subcog Current State:**
- Manual tagging only
- No automatic entity detection
- No entity-memory linking

**Recommendation:**
- Priority: **MEDIUM**
- Effort: Medium (2 weeks)
- Impact: Improves memory organization and retrieval

**Implementation Plan:**
1. Add EntityExtractor using LLM/NER models
2. Auto-tag memories with detected entities
3. Build entity index for fast lookup
4. Add entity filter to recall: `subcog recall --entity "John Doe"`
5. New tool: `subcog_entities --memory_id` or `--query` -->

---

<!-- #### 6. Custom Memory Categories / Ontologies

**Industry Standard (Mem0, Zep):**
- User-defined memory types
- Custom ontologies for domain-specific use
- Dynamic category creation

**Subcog Current State:**
- 11 fixed namespaces
- No custom namespace creation
- Limited extensibility

**Recommendation:**
- Priority: **MEDIUM**
- Effort: Small (1 week)
- Impact: Flexibility for diverse use cases

**Implementation Plan:**
1. Allow custom namespace creation via config
2. Add namespace metadata (description, icon, color)
3. New CLI: `subcog namespace create/delete/describe`
4. Update schema to support user-defined namespaces
5. Add namespace validation rules -->

---

#### 3. Group/Shared Memory Graphs for an Organization Domain

**Industry Standard (Zep):**
- Shared graphs across users 
- Group-level context distribution
- Multi-tenant memory sharing

**Subcog Current State:**
- Individual user/project scopes only
- No group-level sharing
- Org scope is feature-gated and minimal

**Recommendation:**
- Priority: **MEDIUM**
- Effort: Medium (2-3 weeks)
- Impact: Team collaboration support

**Implementation Plan:**
1. Add Group model with member management
2. Implement group-scoped memory storage
3. Add sharing permissions (read/write/admin)
4. New tools: `subcog_group_create`, `subcog_share_memory`
5. Access control for group resources

These features enable team collaboration and shared knowledge bases and currently should be CLI+MCP only with an API for future UI integration.
---

<!-- #### 8. Context Templates

**Industry Standard (Zep):**
- Reusable templates for context formatting
- Customizable context block construction
- Template versioning

**Subcog Current State:**
- Prompt templates exist but not context templates
- No structured context formatting
- Context building is code-only

**Recommendation:**
- Priority: **MEDIUM**
- Effort: Small (1 week)
- Impact: Improved context engineering

**Implementation Plan:**
1. Add ContextTemplate model (similar to PromptTemplate)
2. Template variables for memory insertion
3. Formatting options (markdown, JSON, structured)
4. New tools: `subcog_context_template_save/run`
5. Use in hooks for consistent context injection -->

<!-- ---

#### 4. Memory Import / Direct Injection

**Industry Standard (Mem0):**
- Bypass deduction phases for pre-defined memories
- Bulk import from external sources
- Migration from other memory systems

**Subcog Current State:**
- Only `subcog capture` command
- No bulk import
- No migration tools

**Recommendation:**
- Priority: **MEDIUM**
- Effort: Small (1 week)
- Impact: Easier onboarding and migration

**Implementation Plan:**
1. Add `subcog import` command (JSON, YAML, CSV)
2. Support Mem0/Zep export format parsing
3. Batch capture with deduplication
4. Progress reporting for large imports
5. New tool: `subcog_import --file --format`

---

#### 5. Structured Memory Export

**Industry Standard (Mem0):**
- Export with customizable Pydantic schemas
- Multiple format support
- Filtered exports

**Subcog Current State:**
- GDPR export is unstructured JSON
- No schema customization
- Limited export filtering

**Recommendation:**
- Priority: **MEDIUM**
- Effort: Small (1 week)
- Impact: Better data portability

**Implementation Plan:**
1. Add export schema definitions
2. Support multiple output formats (JSON, YAML, CSV, Parquet)
3. Add filter parameters to export
4. Custom field selection
5. Enhance `subcog_gdpr_export` with options

--- 
-->

### 🟢 Nice-to-Have Features (Lower Priority)

#### 9. ECL Pipeline ([#83](https://github.com/zircote/subcog/issues/83))

**Industry Standard (Cognee):**
- Extract-Cognify-Load replacing traditional RAG
- Multi-stage pipeline with pluggable stages

---

#### 10. Memory Freshness System ([#84](https://github.com/zircote/subcog/issues/84))

**Industry Standard (Cognee Memify):**
- Automated stale node cleanup
- Association strengthening based on access patterns

---

#### 11. Managed Connectors ([#85](https://github.com/zircote/subcog/issues/85))

**Industry Standard (Graphlit):**
- Auto-import from Slack, GitHub, Jira, Notion
- OAuth-based with polling/webhooks

---

#### 12. Cognitive Memory Types ([#86](https://github.com/zircote/subcog/issues/86))

**Industry Standard (LangMem):**
- Procedural (skills), Episodic (experiences), Semantic (facts)

---

#### 13. Background Memory Manager ([#87](https://github.com/zircote/subcog/issues/87))

**Industry Standard (LangMem):**
- Automatic extraction/consolidation based on triggers

---

#### 14. Conversations API ([#88](https://github.com/zircote/subcog/issues/88))

**Industry Standard (Letta):**
- Shared memory across parallel sessions

---

#### 15. Compressed Memory Snippets ([#89](https://github.com/zircote/subcog/issues/89))

**Industry Standard (Mem0):**
- 80% token reduction through intelligent compression

---

#### 16. Community Subgraph ([#90](https://github.com/zircote/subcog/issues/90))

**Industry Standard (Zep Graphiti):**
- Hierarchical graph tiers for community-level aggregations

---

#### 17. Framework Integrations

**Industry Standard (Mem0, Zep):**
- Native LangChain, CrewAI, LlamaIndex integrations
- Vercel AI SDK support
- AutoGen memory persistence

**Subcog Current State:**
- MCP-first architecture
- No specific framework adapters
- Claude Code hooks only

**Recommendation:**
- Priority: **LOW**
- Effort: Medium (2-3 weeks)
- Impact: Broader ecosystem adoption

**Implementation Plan:**
1. LangChain BaseChatMemory adapter
2. CrewAI tool wrapper
3. LlamaIndex retriever plugin
4. Python SDK for non-Rust consumers
5. JavaScript/TypeScript SDK

---

#### 7. Voice/LiveKit Integration

**Industry Standard (Zep):**
- Memory persistence for voice agents
- Real-time transcription storage
- Voice context continuity

**Subcog Current State:**
- No voice support
- Text-only capture

**Recommendation:**
- Priority: **LOW**
- Effort: Medium (2 weeks)
- Impact: Growing voice agent market

**Implementation Plan:**
1. Audio transcription storage
2. Speaker diarization metadata
3. Voice context namespace
4. LiveKit integration adapter
5. Streaming capture API

---

#### 8. Bring Your Own LLM (BYOM) Configuration

**Industry Standard (Zep):**
- Customer-managed LLM configuration
- Swap default models for custom fine-tuned
- Model routing based on task

**Subcog Current State:**
- Already supports multiple providers (Anthropic, OpenAI, Ollama, LM Studio)
- Config-based model selection
- Could be more dynamic

**Recommendation:**
- Priority: **LOW**
- Effort: Small (1 week)
- Impact: Enterprise flexibility

**Implementation Plan:**
1. Per-operation LLM configuration
2. Model routing rules
3. Fallback chains
4. Dynamic model selection based on task type

---

## Subcog Competitive Advantages

Areas where Subcog **already leads** or **equals** industry standards:

| Feature | Subcog Advantage | vs. Competition |
|---------|------------------|-----------------|
| **Local-First** | Full offline support | Only major tool with this capability |
| **Observability** | Full OTLP, tracing, metrics | Exceeds all competitors |
| **CLI Tooling** | 20+ commands | Richest CLI available |
| **Security** | Secrets, PII, encryption, RBAC, audit | Most comprehensive |
| **Deduplication** | 3-tier (exact/semantic/recent) | Exceeds industry standard |
| **Memory Consolidation** | LLM-powered summarization | Unique feature |
| **Search Flexibility** | Hybrid/Vector/Text + RRF fusion | Best-in-class |
| **Code Assistant Integration** | Deep Claude Code hooks | Unique integration |
| **Prompt Management** | Full template system with enrichment | Advanced capability |
| **GDPR Compliance** | Full Article 17/20 support | Production-ready |
| **Graph Memory** | Temporal KG with bitemporal tracking | Matches Zep, exceeds Mem0 |
| **Entity Extraction** | Auto-extraction with LLM + pattern fallback | Full parity |
| **Context Templates** | Versioned templates with variable substitution | Exceeds Mem0 |
| **Memory TTL** | Configurable expiration with auto-cleanup | Matches Mem0, exceeds Zep |

---

## Implementation Roadmap

### ✅ Completed (Q1 2026)

| Feature | Status | Completed |
|---------|--------|-----------|
| Memory Expiration/TTL | ✅ Done | Jan 2026 |
| Entity Extraction | ✅ Done | Jan 2026 |
| Knowledge Graphs | ✅ Done | Jan 2026 |
| Context Templates | ✅ Done | Jan 2026 |

**Outcome:** Major competitive gaps closed. Subcog now has feature parity with Mem0/Zep on core graph and entity capabilities.

### Phase 1: Critical Market Differentiators (Q1-Q2 2026)

| Feature | Issue | Effort | Priority | Target |
|---------|-------|--------|----------|--------|
| Self-Editing Memory | [#75](https://github.com/zircote/subcog/issues/75) | 3-4 weeks | 🔴 Critical | Feb 2026 |
| Conflict Detection | [#76](https://github.com/zircote/subcog/issues/76) | 2 weeks | 🔴 Critical | Feb 2026 |
| Multi-Agent Coordination | [#77](https://github.com/zircote/subcog/issues/77) | 3 weeks | 🔴 Critical | Mar 2026 |
| Webhooks | - | 2 weeks | 🔴 Critical | Mar 2026 |
| Multimodal Support | - | 2-3 weeks | 🔴 Critical | Apr 2026 |

**Phase 1 Outcome:** Address competitive necessity features and enterprise integrations.

### Phase 2: Enterprise Differentiators (Q2 2026)

| Feature | Issue | Effort | Priority | Target |
|---------|-------|--------|----------|--------|
| Collaborative ACLs | [#78](https://github.com/zircote/subcog/issues/78) | 2-3 weeks | 🟡 Important | Apr 2026 |
| Agent Dev Environment | [#79](https://github.com/zircote/subcog/issues/79) | 4 weeks | 🟡 Important | May 2026 |
| Bi-Temporal Model | [#80](https://github.com/zircote/subcog/issues/80) | 2 weeks | 🟡 Important | May 2026 |
| CodeGraph Layer | [#81](https://github.com/zircote/subcog/issues/81) | 3-4 weeks | 🟡 Important | Jun 2026 |
| Memory Portability | [#82](https://github.com/zircote/subcog/issues/82) | 2 weeks | 🟡 Important | Jun 2026 |

**Phase 2 Outcome:** Establish enterprise differentiators and strategic positioning.

### Phase 3: Feature Expansion (Q3 2026)

| Feature | Issue | Effort | Priority | Target |
|---------|-------|--------|----------|--------|
| ECL Pipeline | [#83](https://github.com/zircote/subcog/issues/83) | 3 weeks | 🟢 Nice-to-Have | Jul 2026 |
| Memory Freshness | [#84](https://github.com/zircote/subcog/issues/84) | 2 weeks | 🟢 Nice-to-Have | Jul 2026 |
| Managed Connectors | [#85](https://github.com/zircote/subcog/issues/85) | 4 weeks | 🟢 Nice-to-Have | Aug 2026 |
| Framework SDKs | - | 3 weeks | 🟢 Nice-to-Have | Sep 2026 |

**Phase 3 Outcome:** Expand ecosystem reach and automation capabilities.

---

## Recommendations Summary

### Immediate Actions (Next 30 Days)

1. **Design Webhook System** - Spec architecture for event notifications
2. **Prototype Multimodal Support** - Test image/document capture workflows
3. **Enhance Bulk Import/Export** - Migration tooling for enterprise adoption

### Strategic Priorities

1. **Webhooks** unlock enterprise integration scenarios
2. **Multimodal** support is essential for modern AI agent use cases
3. **Group Memory** enables team collaboration and knowledge sharing

### Positioning

Subcog should position as:
- **"The developer's AI memory system"** - Local-first, CLI-rich, deeply integrated
- **"Production-ready from day one"** - Security, observability, compliance built-in
- **"Full knowledge graph capabilities"** - Temporal KG, entity extraction, graph traversal
- **"Hybrid search that just works"** - Best-in-class retrieval without configuration

---

## Appendix: Feature Sources

### Mem0 Documentation
- Overview: https://docs.mem0.ai/overview
- Memory API: https://docs.mem0.ai/features/memory-api
- Graph Memory: https://docs.mem0.ai/open-source/features/graph-memory

### Zep Documentation
- Help Center: https://help.getzep.com/
- Knowledge Graphs: https://help.getzep.com/knowledge-graphs
- Temporal Knowledge Graph Architecture: https://arxiv.org/abs/2501.13956

### Letta Documentation
- Memory Blocks: https://www.letta.com/blog/memory-blocks
- Agent Development Environment: https://www.letta.com/blog/introducing-the-agent-development-environment
- MemGPT Concepts: https://docs.letta.com/concepts/memgpt/

### Cognee Documentation
- ECL Pipeline (LanceDB Case Study): https://lancedb.com/blog/case-study-cognee/
- Temporal Cognification: https://www.cognee.ai/blog/cognee-news/unlock-your-llm-s-time-awareness-introducing-temporal-cognification
- CodeGraph: https://memgraph.com/blog/from-rag-to-graphs-cognee-ai-memory

### LangMem Documentation
- GitHub: https://langchain-ai.github.io/langmem/
- Memory API: https://langchain-ai.github.io/langmem/concepts/

### Research Papers
- Collaborative Memory: https://arxiv.org/abs/2505.18279
- AI Memory Survey (Dec 2025): https://arxiv.org/abs/2512.13564
- Multi-Agent Memory Engineering: https://www.mongodb.com/company/blog/technical/why-multi-agent-systems-need-memory-engineering

### Other Providers
- Graphlit (Managed Connectors): https://www.graphlit.com/
- Graphiti Knowledge Graph: https://neo4j.com/blog/developer/graphiti-knowledge-graph-memory/

---

*Report generated: 2026-01-12*
*Last updated: 2026-01-22*
*Next review: 2026-02-22*
