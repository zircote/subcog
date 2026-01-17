# Memory Consolidation and Enrichment

This document details the memory consolidation pipeline and enrichment process, including how enriched data flows to Claude Code hooks as `additionalContext`.

---

## Table of Contents

1. [Overview](#overview)
2. [ConsolidationService Architecture](#consolidationservice-architecture)
3. [Consolidation Pipeline](#consolidation-pipeline)
4. [Memory Tiering System](#memory-tiering-system)
5. [Retention Score Calculation](#retention-score-calculation)
6. [Enrichment to Hooks Flow](#enrichment-to-hooks-flow)
7. [SessionStart Context Building](#sessionstart-context-building)
8. [XML Output Format](#xml-output-format)
9. [Configuration](#configuration)
10. [Examples](#examples)

---

## Overview

Memory consolidation and enrichment is a **Tier 3 (LLM-powered)** feature that:

1. **Clusters** related memories for pattern recognition
2. **Summarizes** clusters into higher-level insights
3. **Tiers** memories by importance (HOT, WARM, COLD, ARCHIVED)
4. **Detects supersession** when newer memories obsolete older ones
5. **Extracts edges** to build a memory graph
6. **Persists** enrichment back to storage

The enrichment data persists to the storage layer and is subsequently read by hooks when building `additionalContext` for Claude sessions.

---

## ConsolidationService Architecture

### Service Definition

```rust
pub struct ConsolidationService {
 /// Storage layers for reading/writing memories
 storage: Arc<CompositeStorage>,

 /// LLM client for consolidation prompts
 llm: Arc<dyn LlmClient>,

 /// Event bus for publishing consolidation events
 event_bus: Arc<EventBus>,

 /// Configuration for consolidation behavior
 config: ConsolidationConfig,

 /// Metrics collector
 metrics: Arc<ConsolidationMetrics>,
}

#[derive(Debug, Clone)]
pub struct ConsolidationConfig {
 /// Similarity threshold for clustering (default: 0.85)
 pub cluster_threshold: f32,

 /// Maximum cluster size before forcing split
 pub max_cluster_size: usize,

 /// Memory count threshold to trigger auto-consolidation
 pub auto_trigger_threshold: usize,

 /// Minimum memories required for consolidation
 pub min_memories: usize,

 /// Token budget for LLM summarization
 pub summary_token_budget: usize,

 /// Whether to enable supersession detection
 pub enable_supersession: bool,

 /// Whether to extract edges for memory graph
 pub enable_edge_extraction: bool,
}
```

### Consolidation Triggers

Consolidation can be triggered in four ways:

| Trigger | Description | Typical Timing |
|---------|-------------|----------------|
| **Manual** | User runs `subcog consolidate` | On demand |
| **Threshold** | Memory count exceeds `auto_trigger_threshold` | After many captures |
| **Scheduled** | Cron-style schedule (e.g., daily) | Background |
| **Stop Hook** | During session end if significant activity | End of session |

```rust
pub enum ConsolidationTrigger {
 /// User-initiated via CLI or MCP tool
 Manual { scope: ConsolidationScope },

 /// Auto-triggered by memory count threshold
 Threshold { namespace: String, count: usize },

 /// Scheduled background consolidation
 Scheduled { schedule_id: String },

 /// Triggered by Stop hook during session end
 SessionEnd { session_id: String, activity_level: ActivityLevel },
}

pub enum ConsolidationScope {
 /// All memories in all namespaces
 All,

 /// Specific namespace only
 Namespace(String),

 /// Specific domain only
 Domain(Domain),

 /// Time-bounded consolidation
 TimeRange { since: DateTime<Utc>, until: DateTime<Utc> },
}
```

---

## Consolidation Pipeline

The consolidation pipeline consists of six sequential stages:

```
┌─────────────┐ ┌───────────────┐ ┌────────────┐
│ Cluster │───▶│ Summarize │───▶│ Tier │
└─────────────┘ └───────────────┘ └────────────┘
 │
 ▼
┌─────────────┐ ┌───────────────┐ ┌────────────┐
│ Persist │◀───│ Edge │◀───│ Supersede │
└─────────────┘ └───────────────┘ └────────────┘
```

### Stage 1: Cluster

Groups semantically similar memories using vector similarity:

```rust
pub struct ClusterStage {
 vector_backend: Arc<dyn VectorBackend>,
 threshold: f32,
 max_cluster_size: usize,
}

impl ClusterStage {
 pub async fn execute(
 &self,
 memories: Vec<Memory>,
 ) -> Result<Vec<MemoryCluster>, ConsolidationError> {
 // 1. Build similarity matrix using vector embeddings
 let embeddings = self.get_embeddings(&memories).await?;
 let similarity_matrix = self.compute_similarities(&embeddings);

 // 2. Apply hierarchical clustering with threshold
 let clusters = self.hierarchical_cluster(
 &memories,
 &similarity_matrix,
 self.threshold,
 );

 // 3. Split oversized clusters
 let bounded_clusters = self.enforce_size_limits(clusters);

 Ok(bounded_clusters)
 }
}

#[derive(Debug)]
pub struct MemoryCluster {
 pub id: ClusterId,
 pub members: Vec<MemoryId>,
 pub centroid: Vec<f32>,
 pub coherence_score: f32,
 pub dominant_namespace: String,
}
```

### Stage 2: Summarize

Uses LLM to generate cluster summaries:

```rust
pub struct SummarizeStage {
 llm: Arc<dyn LlmClient>,
 token_budget: usize,
}

impl SummarizeStage {
 pub async fn execute(
 &self,
 cluster: &MemoryCluster,
 memories: &[Memory],
 ) -> Result<ClusterSummary, ConsolidationError> {
 let prompt = self.build_summary_prompt(cluster, memories);

 let response = self.llm.complete(&prompt, self.token_budget).await?;

 Ok(ClusterSummary {
 cluster_id: cluster.id.clone(),
 title: response.title,
 summary: response.summary,
 key_insights: response.key_insights,
 temporal_span: self.compute_temporal_span(memories),
 })
 }

 fn build_summary_prompt(
 &self,
 cluster: &MemoryCluster,
 memories: &[Memory],
 ) -> String {
 format!(
 r#"Analyze these related memories and provide:
1. A concise title (max 10 words)
2. A summary paragraph (max 100 words)
3. Key insights as bullet points (max 5)

Memories:
{}

Focus on patterns, decisions, and learnings."#,
 self.format_memories(memories)
 )
 }
}

#[derive(Debug)]
pub struct ClusterSummary {
 pub cluster_id: ClusterId,
 pub title: String,
 pub summary: String,
 pub key_insights: Vec<String>,
 pub temporal_span: Option<(DateTime<Utc>, DateTime<Utc>)>,
}
```

### Stage 3: Tier

Assigns importance tiers based on retention score:

```rust
pub struct TierStage {
 retention_calculator: RetentionCalculator,
}

impl TierStage {
 pub async fn execute(
 &self,
 memories: &mut [Memory],
 ) -> Result<TieringResult, ConsolidationError> {
 let mut result = TieringResult::default();

 for memory in memories.iter_mut() {
 let score = self.retention_calculator.calculate(memory);
 let tier = MemoryTier::from_score(score);

 // Track tier changes
 if memory.tier!= tier {
 result.transitions.push(TierTransition {
 memory_id: memory.id.clone(),
 from: memory.tier,
 to: tier,
 score,
 });
 }

 memory.retention_score = score;
 memory.tier = tier;
 }

 Ok(result)
 }
}
```

### Stage 4: Supersede

Detects when newer memories obsolete older ones:

```rust
pub struct SupersedeStage {
 llm: Arc<dyn LlmClient>,
 threshold: f32,
}

impl SupersedeStage {
 pub async fn execute(
 &self,
 cluster: &MemoryCluster,
 memories: &[Memory],
 ) -> Result<Vec<SupersessionRelation>, ConsolidationError> {
 let mut relations = Vec::new();

 // Sort by timestamp (newest first)
 let sorted: Vec<_> = memories.iter()
.sorted_by_key(|m| std::cmp::Reverse(m.created_at))
.collect();

 // Compare newer memories against older ones
 for (i, newer) in sorted.iter().enumerate() {
 for older in sorted.iter().skip(i + 1) {
 if let Some(relation) = self.check_supersession(newer, older).await? {
 relations.push(relation);
 }
 }
 }

 Ok(relations)
 }

 async fn check_supersession(
 &self,
 newer: &Memory,
 older: &Memory,
 ) -> Result<Option<SupersessionRelation>, ConsolidationError> {
 let prompt = format!(
 r#"Does the newer memory supersede (make obsolete) the older memory?

Newer: {}
Older: {}

Answer with:
- "full" if newer completely supersedes older
- "partial" if newer partially supersedes older
- "none" if they are independent

Respond with just the word."#,
 newer.content,
 older.content
 );

 let response = self.llm.complete(&prompt, 10).await?;

 match response.trim() {
 "full" => Ok(Some(SupersessionRelation {
 superseder: newer.id.clone(),
 superseded: older.id.clone(),
 kind: SupersessionKind::Full,
 })),
 "partial" => Ok(Some(SupersessionRelation {
 superseder: newer.id.clone(),
 superseded: older.id.clone(),
 kind: SupersessionKind::Partial,
 })),
 _ => Ok(None),
 }
 }
}

#[derive(Debug)]
pub struct SupersessionRelation {
 pub superseder: MemoryId,
 pub superseded: MemoryId,
 pub kind: SupersessionKind,
}

#[derive(Debug, Clone, Copy)]
pub enum SupersessionKind {
 /// Newer memory completely replaces older
 Full,
 /// Newer memory partially updates older
 Partial,
}
```

### Stage 5: Edge

Extracts relationships between memories for graph building:

```rust
pub struct EdgeStage {
 llm: Arc<dyn LlmClient>,
}

impl EdgeStage {
 pub async fn execute(
 &self,
 memories: &[Memory],
 ) -> Result<Vec<MemoryEdge>, ConsolidationError> {
 let mut edges = Vec::new();

 // Extract edges from each memory
 for memory in memories {
 let extracted = self.extract_edges(memory).await?;
 edges.extend(extracted);
 }

 // Deduplicate and merge edges
 let unique_edges = self.deduplicate(edges);

 Ok(unique_edges)
 }

 async fn extract_edges(
 &self,
 memory: &Memory,
 ) -> Result<Vec<MemoryEdge>, ConsolidationError> {
 let prompt = format!(
 r#"Extract relationships from this memory as edges.
Types: RELATES_TO, DEPENDS_ON, CONTRADICTS, IMPLEMENTS, SUPERSEDES

Memory: {}

Output as JSON array:
[{{"type": "...", "target_hint": "...", "confidence": 0.X}}]"#,
 memory.content
 );

 let response = self.llm.complete(&prompt, 200).await?;
 let raw_edges: Vec<RawEdge> = serde_json::from_str(&response)?;

 Ok(raw_edges.into_iter()
.map(|e| MemoryEdge {
 source: memory.id.clone(),
 edge_type: e.edge_type,
 target_hint: e.target_hint,
 confidence: e.confidence,
 })
.collect())
 }
}

#[derive(Debug)]
pub struct MemoryEdge {
 pub source: MemoryId,
 pub edge_type: EdgeType,
 pub target_hint: String,
 pub confidence: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum EdgeType {
 RelatesTo,
 DependsOn,
 Contradicts,
 Implements,
 Supersedes,
}
```

### Stage 6: Persist

Writes all enrichment back to storage:

```rust
pub struct PersistStage {
 storage: Arc<CompositeStorage>,
 event_bus: Arc<EventBus>,
}

impl PersistStage {
 pub async fn execute(
 &self,
 result: ConsolidationResult,
 ) -> Result<(), ConsolidationError> {
 // 1. Update memory tiers and scores
 for memory in &result.enriched_memories {
 self.storage.update_memory(memory).await?;
 }

 // 2. Store cluster summaries
 for summary in &result.summaries {
 self.storage.store_summary(summary).await?;
 }

 // 3. Store supersession relations
 for relation in &result.supersessions {
 self.storage.store_supersession(relation).await?;

 // Apply penalty to superseded memory
 if let Some(mut superseded) = self.storage.get_memory(&relation.superseded).await? {
 superseded.superseded_by = Some(relation.superseder.clone());
 superseded.retention_score *= match relation.kind {
 SupersessionKind::Full => 0.3,
 SupersessionKind::Partial => 0.7,
 };
 self.storage.update_memory(&superseded).await?;
 }
 }

 // 4. Store edges
 for edge in &result.edges {
 self.storage.store_edge(edge).await?;
 }

 // 5. Publish consolidation event
 self.event_bus.publish(MemoryEvent::ConsolidationCompleted {
 memories_processed: result.enriched_memories.len(),
 clusters_formed: result.summaries.len(),
 supersessions_detected: result.supersessions.len(),
 edges_extracted: result.edges.len(),
 }).await;

 Ok(())
 }
}
```

---

## Memory Tiering System

### Tier Definitions

| Tier | Score Range | Description | Behavior |
|------|-------------|-------------|----------|
| **HOT** | ≥0.7 | Active, frequently accessed, recent | Always included in context |
| **WARM** | ≥0.4 | Moderately important, occasionally accessed | Included when relevant |
| **COLD** | ≥0.2 | Aging, rarely accessed | Included only on direct query |
| **ARCHIVED** | <0.2 | Superseded or very old | Excluded from normal search |

### Tier Implementation

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemoryTier {
 Hot,
 Warm,
 Cold,
 Archived,
}

impl MemoryTier {
 pub const fn from_score(score: f32) -> Self {
 if score >= 0.7 {
 Self::Hot
 } else if score >= 0.4 {
 Self::Warm
 } else if score >= 0.2 {
 Self::Cold
 } else {
 Self::Archived
 }
 }

 pub const fn threshold(&self) -> f32 {
 match self {
 Self::Hot => 0.7,
 Self::Warm => 0.4,
 Self::Cold => 0.2,
 Self::Archived => 0.0,
 }
 }

 /// Token budget multiplier for this tier
 pub const fn budget_multiplier(&self) -> f32 {
 match self {
 Self::Hot => 1.0,
 Self::Warm => 0.7,
 Self::Cold => 0.4,
 Self::Archived => 0.1,
 }
 }
}
```

---

## Retention Score Calculation

The retention score determines memory tier and inclusion priority.

### Formula

```
retention_score = (recency × 0.3) + (activation × 0.3) + (importance × 0.3) - (supersession_penalty × 0.1)
```

### Components

```rust
pub struct RetentionCalculator {
 /// Weight for recency component
 recency_weight: f32,

 /// Weight for activation count
 activation_weight: f32,

 /// Weight for importance signal
 importance_weight: f32,

 /// Weight for supersession penalty
 supersession_weight: f32,

 /// Half-life for recency decay (days)
 recency_half_life: f32,
}

impl RetentionCalculator {
 pub fn calculate(&self, memory: &Memory) -> f32 {
 let recency = self.calculate_recency(memory);
 let activation = self.calculate_activation(memory);
 let importance = self.calculate_importance(memory);
 let supersession = self.calculate_supersession_penalty(memory);

 let score = (recency * self.recency_weight)
 + (activation * self.activation_weight)
 + (importance * self.importance_weight)
 - (supersession * self.supersession_weight);

 score.clamp(0.0, 1.0)
 }

 fn calculate_recency(&self, memory: &Memory) -> f32 {
 let age_days = (Utc::now() - memory.created_at).num_days() as f32;

 // Exponential decay with half-life
 // score = 0.5^(age / half_life)
 (0.5_f32).powf(age_days / self.recency_half_life)
 }

 fn calculate_activation(&self, memory: &Memory) -> f32 {
 // Normalize activation count (log scale)
 let count = memory.activation_count as f32;
 (count.ln() + 1.0) / 5.0 // Normalize to ~0-1 range
 }

 fn calculate_importance(&self, memory: &Memory) -> f32 {
 // Combine signals: explicit importance + namespace weight
 let explicit = memory.importance.unwrap_or(0.5);
 let namespace_weight = self.namespace_importance(&memory.namespace);

 (explicit + namespace_weight) / 2.0
 }

 fn calculate_supersession_penalty(&self, memory: &Memory) -> f32 {
 match &memory.superseded_by {
 Some(_) => 0.7, // Heavy penalty if superseded
 None => 0.0,
 }
 }

 fn namespace_importance(&self, namespace: &str) -> f32 {
 match namespace {
 "decisions" => 0.9,
 "learnings" => 0.8,
 "patterns" => 0.8,
 "blockers" => 0.7,
 "context" => 0.5,
 "observations" => 0.4,
 _ => 0.5,
 }
 }
}
```

### Recency Decay Visualization

```
Score
1.0 │▓▓▓▓▓░
 │ ▓▓▓▓░
 │ ▓▓▓░
0.5 │───────────────▓▓▓░────────────
 │ ▓▓░
 │ ▓░
0.0 │─────────────────────────▓▓▓░──
 └─────────────────────────────── Days
 0 7 14 21 28 35 42

 Half-life = 14 days (default)
```

---

## Enrichment to Hooks Flow

Enrichment data persists to storage and is read by hooks when building context.

### Data Flow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│ ConsolidationService │
│ │
│ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ │
│ │ Cluster │─▶│Summarize │─▶│ Tier │─▶│ Persist │ │
│ └──────────┘ └──────────┘ └──────────┘ └──────────┘ │
│ │ │
└─────────────────────────────────────────────────────┼───────┘
 │
 ▼
┌─────────────────────────────────────────────────────────────┐
│ CompositeStorage │
│ │
│ ┌───────────────────────────────────────────────────────┐ │
│ │ Memory Record │ │
│ │ ─────────────────────────────────────────────────────│ │
│ │ id: "abc123:0" │ │
│ │ content: "Decided to use PostgreSQL for..." │ │
│ │ namespace: "decisions" │ │
│ │ ─────────────────────────────────────────────────────│ │
│ │ tier: HOT ◀── Enrichment │ │
│ │ retention_score: 0.85 ◀── Enrichment │ │
│ │ superseded_by: None ◀── Enrichment │ │
│ │ edges: [{type: RELATES_TO,...}] ◀── Enrichment │ │
│ │ cluster_id: "cluster_42" ◀── Enrichment │ │
│ └───────────────────────────────────────────────────────┘ │
│ │
└──────────────────────────────┬──────────────────────────────┘
 │
 │ Read at hook invocation
 ▼
┌─────────────────────────────────────────────────────────────┐
│ SessionStart Hook │
│ │
│ 1. Read enriched memories from storage │
│ 2. Filter by tier (HOT always, WARM if relevant) │
│ 3. Sort by retention_score (descending) │
│ 4. Apply token budget with tier multipliers │
│ 5. Format as XML additionalContext │
│ │
└─────────────────────────────────────────────────────────────┘
```

### Storage Schema for Enrichment

```sql
-- Memory table with enrichment columns
CREATE TABLE memories (
 id TEXT PRIMARY KEY,
 content TEXT NOT NULL,
 namespace TEXT NOT NULL,
 domain TEXT NOT NULL,
 created_at TIMESTAMP NOT NULL,

 -- Enrichment fields (updated by consolidation)
 tier TEXT DEFAULT 'warm',
 retention_score REAL DEFAULT 0.5,
 superseded_by TEXT,
 cluster_id TEXT,
 activation_count INTEGER DEFAULT 0,
 last_accessed TIMESTAMP
);

-- Cluster summaries table
CREATE TABLE cluster_summaries (
 cluster_id TEXT PRIMARY KEY,
 title TEXT NOT NULL,
 summary TEXT NOT NULL,
 key_insights TEXT, -- JSON array
 member_count INTEGER,
 created_at TIMESTAMP NOT NULL
);

-- Memory edges table
CREATE TABLE memory_edges (
 id INTEGER PRIMARY KEY,
 source_id TEXT NOT NULL,
 edge_type TEXT NOT NULL,
 target_hint TEXT,
 target_id TEXT, -- Resolved target (if found)
 confidence REAL,
 FOREIGN KEY (source_id) REFERENCES memories(id)
);

-- Supersession relations table
CREATE TABLE supersessions (
 superseder_id TEXT NOT NULL,
 superseded_id TEXT NOT NULL,
 kind TEXT NOT NULL, -- 'full' or 'partial'
 detected_at TIMESTAMP NOT NULL,
 PRIMARY KEY (superseder_id, superseded_id)
);
```

---

## SessionStart Context Building

The SessionStart hook reads enriched memories and builds `additionalContext`.

### Context Builder

```rust
pub struct SessionContextBuilder {
 storage: Arc<CompositeStorage>,
 config: ContextBuildConfig,
}

#[derive(Debug, Clone)]
pub struct ContextBuildConfig {
 /// Base token budget for context
 pub base_token_budget: usize,

 /// Minimum tier to include in context
 pub min_tier: MemoryTier,

 /// Maximum memories per namespace
 pub max_per_namespace: usize,

 /// Whether to include cluster summaries
 pub include_summaries: bool,

 /// Whether to include edge hints
 pub include_edges: bool,
}

impl SessionContextBuilder {
 pub async fn build(
 &self,
 project_path: &Path,
 ) -> Result<SessionContext, HookError> {
 // 1. Get project complexity for adaptive budget
 let complexity = self.assess_complexity(project_path).await?;
 let token_budget = self.calculate_budget(complexity);

 // 2. Load enriched memories with tier filtering
 let memories = self.load_tier_filtered_memories(
 project_path,
 self.config.min_tier,
 ).await?;

 // 3. Sort by retention score
 let sorted = self.sort_by_retention(memories);

 // 4. Apply token budget with tier multipliers
 let selected = self.apply_budget(sorted, token_budget);

 // 5. Load cluster summaries if enabled
 let summaries = if self.config.include_summaries {
 self.load_relevant_summaries(&selected).await?
 } else {
 Vec::new()
 };

 // 6. Build context structure
 Ok(SessionContext {
 memories: selected,
 summaries,
 token_budget,
 complexity,
 })
 }

 async fn load_tier_filtered_memories(
 &self,
 project_path: &Path,
 min_tier: MemoryTier,
 ) -> Result<Vec<Memory>, HookError> {
 let query = MemoryQuery {
 domain: Some(Domain::Project(project_path.to_path_buf())),
 min_tier: Some(min_tier),
 exclude_superseded: true,
..Default::default()
 };

 self.storage.query_memories(query).await
 }

 fn sort_by_retention(&self, mut memories: Vec<Memory>) -> Vec<Memory> {
 memories.sort_by(|a, b| {
 b.retention_score.partial_cmp(&a.retention_score)
.unwrap_or(std::cmp::Ordering::Equal)
 });
 memories
 }

 fn apply_budget(
 &self,
 memories: Vec<Memory>,
 budget: usize,
 ) -> Vec<Memory> {
 let mut used = 0;
 let mut selected = Vec::new();

 for memory in memories {
 // Apply tier multiplier to effective cost
 let effective_cost = memory.estimated_tokens() as f32
 / memory.tier.budget_multiplier();
 let cost = effective_cost as usize;

 if used + cost <= budget {
 used += cost;
 selected.push(memory);
 } else if memory.tier == MemoryTier::Hot {
 // Always include HOT memories even if over budget
 selected.push(memory);
 }
 }

 selected
 }

 fn calculate_budget(&self, complexity: ProjectComplexity) -> usize {
 match complexity {
 ProjectComplexity::Simple => self.config.base_token_budget,
 ProjectComplexity::Moderate => (self.config.base_token_budget as f32 * 1.5) as usize,
 ProjectComplexity::Complex => self.config.base_token_budget * 2,
 ProjectComplexity::Enterprise => (self.config.base_token_budget as f32 * 2.5) as usize,
 }
 }
}
```

### Tier-Aware Selection Algorithm

```
Input: memories sorted by retention_score (descending)
Budget: N tokens
Output: selected memories

1. Initialize selected = [], used = 0
2. For each memory in memories:
 a. Calculate effective_cost = tokens / tier_multiplier
 - HOT: tokens / 1.0 = full cost
 - WARM: tokens / 0.7 = ~1.4x cost
 - COLD: tokens / 0.4 = 2.5x cost
 - ARCHIVED: tokens / 0.1 = 10x cost
 b. If used + effective_cost <= budget:
 - Add memory to selected
 - used += effective_cost
 c. Else if tier == HOT:
 - Add memory anyway (HOT always included)
3. Return selected

Effect: HOT memories "cost less" in budget terms,
 allowing more of them to be included
```

---

## XML Output Format

The hook output uses XML format for `additionalContext`:

### Full XML Structure

```xml
<subcog-context>
 <project path="/path/to/project" complexity="moderate" />
 <token-budget used="2500" available="4000" />

 <memories count="15">
 <memory-group namespace="decisions" count="5">
 <memory id="abc123:0" tier="hot" score="0.92">
 <summary>Use PostgreSQL for primary storage</summary>
 <content>
 Decided to use PostgreSQL with pgvector extension for
 team deployment scenario. Rationale: ACID guarantees,
 horizontal scaling, mature vector search.
 </content>
 <meta>
 <created>2025-12-27T10:30:00Z</created>
 <activations>12</activations>
 <edges>
 <edge type="relates_to" target="def456:0" confidence="0.85" />
 </edges>
 </meta>
 </memory>
 <!-- More decision memories -->
 </memory-group>

 <memory-group namespace="learnings" count="4">
 <memory id="ghi789:0" tier="warm" score="0.65">
 <summary>Async Rust requires careful lifetime management</summary>
 <content>
 Learning: When using async functions with borrowed data,
 prefer owned types or Arc to avoid lifetime complexity.
 </content>
 <meta>
 <created>2025-12-26T14:20:00Z</created>
 <activations>3</activations>
 <superseded_by />
 </meta>
 </memory>
 <!-- More learning memories -->
 </memory-group>

 <memory-group namespace="patterns" count="3">
 <!-- Pattern memories -->
 </memory-group>
 </memories>

 <summaries count="2">
 <cluster-summary id="cluster_42">
 <title>Database Architecture Decisions</title>
 <summary>
 Multiple decisions converged on a three-layer storage
 architecture with PostgreSQL for persistence, SQLite for
 local indexing, and usearch for vector operations.
 </summary>
 <insights>
 <insight>ACID guarantees prioritized for team scenarios</insight>
 <insight>Local-first with optional cloud sync</insight>
 <insight>Trait-based abstraction enables future backends</insight>
 </insights>
 <members>5</members>
 </cluster-summary>
 </summaries>

 <status>
 <last-consolidation>2025-12-27T08:00:00Z</last-consolidation>
 <total-memories>127</total-memories>
 <tier-distribution hot="15" warm="42" cold="50" archived="20" />
 </status>
</subcog-context>
```

### XML Generation Code

```rust
pub struct XmlFormatter;

impl XmlFormatter {
 pub fn format_context(context: &SessionContext) -> String {
 let mut xml = String::new();

 xml.push_str("<subcog-context>\n");

 // Project metadata
 xml.push_str(&format!(
 r#" <project path="{}" complexity="{}" />"#,
 context.project_path.display(),
 context.complexity,
 ));
 xml.push('\n');

 // Token budget
 xml.push_str(&format!(
 r#" <token-budget used="{}" available="{}" />"#,
 context.tokens_used,
 context.token_budget,
 ));
 xml.push('\n');

 // Memories grouped by namespace
 xml.push_str(&format!(
 r#" <memories count="{}">"#,
 context.memories.len()
 ));
 xml.push('\n');

 for (namespace, memories) in context.memories_by_namespace() {
 xml.push_str(&Self::format_memory_group(namespace, memories));
 }

 xml.push_str(" </memories>\n");

 // Cluster summaries
 if!context.summaries.is_empty() {
 xml.push_str(&Self::format_summaries(&context.summaries));
 }

 // Status
 xml.push_str(&Self::format_status(&context.status));

 xml.push_str("</subcog-context>\n");

 xml
 }

 fn format_memory_group(namespace: &str, memories: &[Memory]) -> String {
 let mut xml = format!(
 r#" <memory-group namespace="{}" count="{}">"#,
 namespace,
 memories.len()
 );
 xml.push('\n');

 for memory in memories {
 xml.push_str(&Self::format_memory(memory));
 }

 xml.push_str(" </memory-group>\n");
 xml
 }

 fn format_memory(memory: &Memory) -> String {
 format!(
 r#" <memory id="{}" tier="{}" score="{:.2}">
 <summary>{}</summary>
 <content>{}</content>
 <meta>
 <created>{}</created>
 <activations>{}</activations>
 {}
 </meta>
 </memory>
"#,
 memory.id,
 memory.tier,
 memory.retention_score,
 Self::escape_xml(&memory.summary),
 Self::escape_xml(&memory.content),
 memory.created_at.to_rfc3339(),
 memory.activation_count,
 Self::format_edges(&memory.edges),
 )
 }

 fn escape_xml(s: &str) -> String {
 s.replace('&', "&amp;")
.replace('<', "&lt;")
.replace('>', "&gt;")
.replace('"', "&quot;")
 }
}
```

---

## Configuration

### Consolidation Configuration

```toml
# ~/.config/subcog/config.toml

[consolidation]
# Tier 3 feature - requires LLM provider
enabled = true
llm_provider = "anthropic"

# Clustering settings
cluster_threshold = 0.85
max_cluster_size = 20

# Auto-trigger settings
auto_trigger = true
auto_trigger_threshold = 100
schedule = "0 0 * * *" # Daily at midnight

# Features
enable_supersession = true
enable_edge_extraction = true

# Token budget for LLM calls
summary_token_budget = 500

[retention]
# Score weights (must sum to 1.0)
recency_weight = 0.3
activation_weight = 0.3
importance_weight = 0.3
supersession_weight = 0.1

# Decay settings
recency_half_life_days = 14

[context]
# SessionStart hook settings
base_token_budget = 4000
min_tier = "warm" # "hot", "warm", "cold", "archived"
max_per_namespace = 10
include_summaries = true
include_edges = true
```

### Environment Variables

```bash
# LLM provider for consolidation
export SUBCOG_LLM_PROVIDER="anthropic"
export ANTHROPIC_API_KEY="sk-ant-..."

# Override token budget
export SUBCOG_CONTEXT_TOKEN_BUDGET="6000"

# Disable auto-consolidation
export SUBCOG_CONSOLIDATION_AUTO="false"
```

---

## Examples

### Example 1: Manual Consolidation

```bash
# Consolidate all memories in project
$ subcog consolidate

Consolidating memories...
 Clustering: 127 memories -> 23 clusters
 Summarizing: 23 cluster summaries generated
 Tiering: 15 HOT, 42 WARM, 50 COLD, 20 ARCHIVED
 Supersession: 8 relations detected
 Edges: 45 edges extracted

Consolidation complete in 12.3s
```

### Example 2: SessionStart Hook Output

```json
{
 "hookName": "SessionStart",
 "result": {
 "continue": true,
 "additionalContext": "<subcog-context>\n <project path=\"/home/user/my-project\" complexity=\"moderate\" />\n <token-budget used=\"2847\" available=\"4000\" />\n <memories count=\"18\">\n <memory-group namespace=\"decisions\" count=\"6\">\n..."
 }
}
```

### Example 3: Tier Transitions

```
Before Consolidation:
 Memory "abc123:0" - tier: WARM, score: 0.55

After Consolidation (superseded by newer decision):
 Memory "abc123:0" - tier: COLD, score: 0.23
 Memory "def456:0" (superseder) - tier: HOT, score: 0.89
```

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-28 | Claude Opus 4.5 | Initial document |
