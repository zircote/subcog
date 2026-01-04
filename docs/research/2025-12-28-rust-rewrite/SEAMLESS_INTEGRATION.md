# Seamless Feature Integration Design

**CRITICAL**: No feature works in isolation. Every capability MUST integrate cohesively with every other capability. This document defines how all features compose, interact, and propagate through the system.

---

## 1. Integration Philosophy

### 1.1 Core Principles

| Principle | Description | Enforcement |
|-----------|-------------|-------------|
| **Single Data Flow** | All operations flow through the same pipeline | Service layer architecture |
| **Event Propagation** | State changes propagate to all interested components | Event bus pattern |
| **Graceful Composition** | Disabled features don't break others | Feature flags at service level |
| **Unified Observability** | Every operation is traced end-to-end | Span context propagation |
| **Consistent URNs** | Every memory is addressable via URN | Resource handler pattern |

### 1.2 Integration Diagram

```
┌───────────────────────────────────────────────────────────────────────────────────────┐
│                           SEAMLESS INTEGRATION ARCHITECTURE                            │
├───────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                       │
│                              ┌─────────────────────┐                                  │
│                              │    ACCESS LAYER     │                                  │
│                              │ CLI│MCP│Hook│Stream │                                  │
│                              └──────────┬──────────┘                                  │
│                                         │                                             │
│                                         ▼                                             │
│  ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│  │                              EVENT BUS (tokio broadcast)                        │  │
│  │  MemoryCaptured │ MemoryUpdated │ SyncCompleted │ ConsolidationRan │ ...        │  │
│  └────────┬────────────────┬────────────────┬───────────────────┬──────────────────┘  │
│           │                │                │                   │                     │
│           ▼                ▼                ▼                   ▼                     │
│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌────────────────┐          │
│  │ Capture        │ │ Recall         │ │ Sync           │ │ Consolidate    │          │
│  │ Pipeline       │ │ Pipeline       │ │ Pipeline       │ │ Pipeline       │          │
│  │                │ │                │ │                │ │                │          │
│  │ ┌────────────┐ │ │ ┌────────────┐ │ │ ┌────────────┐ │ │ ┌────────────┐ │          │
│  │ │Secrets Flt │ │ │ │Query Exp.  │ │ │ │Git Fetch   │ │ │ │Clustering  │ │          │
│  │ └─────┬──────┘ │ │ └─────┬──────┘ │ │ └─────┬──────┘ │ │ └─────┬──────┘ │          │
│  │       ▼        │ │       ▼        │ │       ▼        │ │       ▼        │          │
│  │ ┌────────────┐ │ │ ┌────────────┐ │ │ ┌────────────┐ │ │ ┌────────────┐ │          │
│  │ │Embedding   │ │ │ │Hybrid Srch │ │ │ │Merge Notes │ │ │ │Summarize   │ │          │
│  │ └─────┬──────┘ │ │ └─────┬──────┘ │ │ └─────┬──────┘ │ │ └─────┬──────┘ │          │
│  │       ▼        │ │       ▼        │ │       ▼        │ │       ▼        │          │
│  │ ┌────────────┐ │ │ ┌────────────┐ │ │ ┌────────────┐ │ │ ┌────────────┐ │          │
│  │ │Index Write │ │ │ │Tier Filter │ │ │ │Index Rebld │ │ │ │Edge Create │ │          │
│  │ └─────┬──────┘ │ │ └─────┬──────┘ │ │ └─────┬──────┘ │ │ └─────┬──────┘ │          │
│  │       ▼        │ │       ▼        │ │       ▼        │ │       ▼        │          │
│  │ ┌────────────┐ │ │ ┌────────────┐ │ │ ┌────────────┐ │ │ ┌────────────┐ │          │
│  │ │Git Write   │ │ │ │Hydrate     │ │ │ │Git Push    │ │ │ │Tier Assign │ │          │
│  │ └────────────┘ │ │ └────────────┘ │ │ └────────────┘ │ │ └────────────┘ │          │
│  └────────────────┘ └────────────────┘ └────────────────┘ └────────────────┘          │
│           │                │                │                   │                     │
│           └────────────────┴────────────────┴───────────────────┘                     │
│                                         │                                             │
│                                         ▼                                             │
│  ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│  │                           UNIFIED STORAGE LAYER                                 │  │
│  │  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐               │  │
│  │  │ Persistence      │  │ Index            │  │ Vector           │               │  │
│  │  │ (Git/SQLite/PG)  │  │ (SQLite/PG)      │  │ (usearch/pgvec)  │               │  │
│  │  └──────────────────┘  └──────────────────┘  └──────────────────┘               │  │
│  └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                         │                                             │
│                                         ▼                                             │
│  ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│  │                        OBSERVABILITY LAYER                                      │  │
│  │  Traces ──────► Metrics ──────► Logs ──────► Audit ──────► OTLP                │  │
│  └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                       │
└───────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Feature Interaction Matrix

Every feature pair has a defined interaction. Empty cells indicate no direct interaction (but indirect through events).

| Feature → | Capture | Recall | Sync | Consolidate | Secrets | Hooks | Embedding | Storage | Observability |
|-----------|---------|--------|------|-------------|---------|-------|-----------|---------|---------------|
| **Capture** | - | triggers recall | updates git | triggers background | filters content | emits signals | generates vectors | writes all layers | traces + metrics |
| **Recall** | uses captured | - | syncs before | tier filtering | redacts output | context injection | uses for search | reads all layers | traces + metrics |
| **Sync** | imports memories | rebuilds index | - | updates tiers | re-validates | triggers on stop | re-embeds if needed | syncs git↔index | traces + metrics |
| **Consolidate** | creates summaries | tier-aware results | syncs after | - | excludes secrets | background run | clusters by vector | updates tiers | traces + metrics |
| **Secrets** | blocks/redacts | redacts results | filters sync | excludes from | - | filters in hooks | skips secret text | audit logging | security events |
| **Hooks** | marker detection | injects context | fetch/push | triggers | applies filter | - | uses for context | hook-specific ops | hook timing |
| **Embedding** | generates on capture | query embedding | regenerates | clusters use | skips secrets | context building | - | vector storage | latency metrics |
| **Storage** | multi-layer write | multi-source read | bidirectional sync | tier updates | audit log | read/write | vector index | - | size metrics |
| **Observability** | capture spans | search spans | sync spans | consolidation spans | security spans | hook spans | embedding spans | storage spans | - |

---

## 3. Event Bus Architecture

### 3.1 Event Types

```rust
/// All events that propagate through the system
#[derive(Debug, Clone)]
pub enum MemoryEvent {
    // Capture events
    MemoryCaptured {
        memory_id: MemoryId,
        namespace: Namespace,
        domain: Domain,
        uri: String,
    },
    CaptureBlocked {
        reason: String,
        detection: SecretDetection,
    },

    // Recall events
    SearchCompleted {
        query: String,
        result_count: usize,
        latency_ms: u64,
    },

    // Sync events
    SyncStarted {
        direction: SyncDirection,
        domain: Domain,
    },
    SyncCompleted {
        direction: SyncDirection,
        memories_synced: usize,
        conflicts_resolved: usize,
    },

    // Consolidation events
    ConsolidationStarted {
        full: bool,
    },
    ClusterCreated {
        cluster_id: String,
        memory_count: usize,
    },
    SummaryGenerated {
        cluster_id: String,
        summary_id: MemoryId,
    },
    TierAssigned {
        memory_id: MemoryId,
        old_tier: MemoryTier,
        new_tier: MemoryTier,
    },
    ConsolidationCompleted {
        clusters_created: usize,
        summaries_generated: usize,
        tiers_updated: usize,
    },

    // Security events
    SecretDetected {
        detection: SecretDetection,
        action: FilterAction,
    },
    AuditLogWritten {
        event_type: String,
        memory_id: Option<MemoryId>,
    },

    // Hook events
    HookTriggered {
        hook_type: HookType,
        session_id: String,
    },
    HookCompleted {
        hook_type: HookType,
        latency_ms: u64,
    },

    // Storage events
    IndexRebuilt {
        memories_indexed: usize,
        duration_ms: u64,
    },
    StorageError {
        backend: String,
        error: String,
    },

    // LLM events
    LlmRequestSent {
        provider: String,
        purpose: String,
    },
    LlmResponseReceived {
        provider: String,
        tokens_used: u32,
        latency_ms: u64,
    },
}
```

### 3.2 Event Bus Implementation

```rust
use tokio::sync::broadcast;

/// Central event bus for cross-component communication
pub struct EventBus {
    sender: broadcast::Sender<MemoryEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn publish(&self, event: MemoryEvent) {
        // Ignore send errors (no receivers)
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<MemoryEvent> {
        self.sender.subscribe()
    }
}

/// Trait for components that handle events
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an event
    async fn handle(&self, event: &MemoryEvent);

    /// Filter for events this handler cares about
    fn filter(&self, event: &MemoryEvent) -> bool;
}
```

### 3.3 Event Handlers

```rust
/// MCP subscription handler - notifies clients of resource changes
pub struct McpSubscriptionHandler {
    subscriptions: Arc<RwLock<HashMap<String, broadcast::Sender<ResourceChange>>>>,
}

#[async_trait]
impl EventHandler for McpSubscriptionHandler {
    async fn handle(&self, event: &MemoryEvent) {
        if let MemoryEvent::MemoryCaptured { uri, .. } = event {
            let subs = self.subscriptions.read().await;
            if let Some(tx) = subs.get(uri) {
                let _ = tx.send(ResourceChange { uri: uri.clone() });
            }
        }
    }

    fn filter(&self, event: &MemoryEvent) -> bool {
        matches!(event, MemoryEvent::MemoryCaptured { .. } | MemoryEvent::TierAssigned { .. })
    }
}

/// Metrics handler - updates Prometheus counters
pub struct MetricsHandler {
    metrics: Arc<Metrics>,
}

#[async_trait]
impl EventHandler for MetricsHandler {
    async fn handle(&self, event: &MemoryEvent) {
        match event {
            MemoryEvent::MemoryCaptured { namespace, domain, .. } => {
                self.metrics.captures_total
                    .with_label_values(&[namespace.as_str(), domain.as_str()])
                    .inc();
            }
            MemoryEvent::SearchCompleted { latency_ms, .. } => {
                self.metrics.search_latency.observe(*latency_ms as f64);
            }
            MemoryEvent::SecretDetected { action, .. } => {
                self.metrics.secrets_detected
                    .with_label_values(&[action.as_str()])
                    .inc();
            }
            _ => {}
        }
    }

    fn filter(&self, _: &MemoryEvent) -> bool {
        true // Metrics handler cares about all events
    }
}

/// Background consolidation trigger
pub struct ConsolidationTrigger {
    consolidation: Arc<ConsolidationService>,
    config: ConsolidationConfig,
    capture_count: AtomicUsize,
}

#[async_trait]
impl EventHandler for ConsolidationTrigger {
    async fn handle(&self, event: &MemoryEvent) {
        if let MemoryEvent::MemoryCaptured { .. } = event {
            let count = self.capture_count.fetch_add(1, Ordering::SeqCst);
            if count >= self.config.memory_threshold {
                self.capture_count.store(0, Ordering::SeqCst);
                // Trigger background consolidation
                tokio::spawn(self.consolidation.clone().run_background());
            }
        }
    }

    fn filter(&self, event: &MemoryEvent) -> bool {
        matches!(event, MemoryEvent::MemoryCaptured { .. })
    }
}
```

---

## 4. Pipeline Composition

### 4.1 Capture Pipeline

The capture pipeline shows how features compose:

```rust
impl CaptureService {
    pub async fn capture(&self, request: CaptureRequest) -> Result<CaptureResult> {
        let span = tracing::info_span!("capture", namespace = %request.namespace);
        let _guard = span.enter();

        // 1. Validate input (Core - always runs)
        self.validate(&request)?;

        // 2. Secrets filtering (Enhanced - if enabled)
        let filtered_content = if self.features.secrets_filtering {
            let filter_result = self.secrets_filter.filter(&request.content).await?;
            if filter_result.blocked {
                self.event_bus.publish(MemoryEvent::CaptureBlocked {
                    reason: "secrets_detected".into(),
                    detection: filter_result.detections.first().cloned().unwrap(),
                });
                return Err(MemoryError::ContentBlocked);
            }
            filter_result.filtered_content
        } else {
            request.content.clone()
        };

        // 3. Generate embedding (Core - always runs)
        let embedding = self.embedder.embed(&filtered_content).await?;

        // 4. Create memory object
        let memory = Memory {
            id: self.generate_id(&request.namespace),
            namespace: request.namespace,
            domain: request.domain,
            summary: request.summary.clone(),
            content: filtered_content,
            timestamp: Utc::now(),
            spec: request.spec,
            tags: request.tags,
            status: MemoryStatus::Active,
            relates_to: request.relates_to,
        };

        // 5. Write to all storage layers
        self.storage.write(&memory, &embedding).await?;

        // 6. Build URN
        let uri = MemoryResourceHandler::build_uri(
            memory.domain.clone(),
            memory.namespace,
            &MemoryId(memory.id.clone()),
        );

        // 7. Publish event (triggers all subscribers)
        self.event_bus.publish(MemoryEvent::MemoryCaptured {
            memory_id: MemoryId(memory.id.clone()),
            namespace: memory.namespace,
            domain: memory.domain,
            uri: uri.clone(),
        });

        // 8. Record metrics (Observability - always runs)
        self.metrics.capture_latency.observe(span.elapsed().as_millis() as f64);

        Ok(CaptureResult {
            success: true,
            memory_id: memory.id,
            uri,
            indexed: true,
            warning: None,
        })
    }
}
```

### 4.2 Recall Pipeline

```rust
impl RecallService {
    pub async fn search(&self, request: SearchRequest) -> Result<SearchResult> {
        let span = tracing::info_span!("recall", query = %request.query);
        let _guard = span.enter();

        // 1. Query expansion (LLM Tier - if enabled)
        let expanded_query = if self.features.query_expansion {
            self.query_expander.expand(&request.query).await?
        } else {
            request.query.clone()
        };

        // 2. Generate query embedding (Core - always runs)
        let query_embedding = self.embedder.embed(&expanded_query).await?;

        // 3. Hybrid search (Core - always runs)
        let raw_results = match request.mode {
            SearchMode::Hybrid => {
                let vector_results = self.storage.search_vector(&query_embedding, request.limit * 2).await?;
                let text_results = self.storage.search_text(&expanded_query, request.limit * 2).await?;
                self.rrf_fusion(vector_results, text_results, request.limit)
            }
            SearchMode::Vector => {
                self.storage.search_vector(&query_embedding, request.limit).await?
            }
            SearchMode::Bm25 => {
                self.storage.search_text(&expanded_query, request.limit).await?
            }
        };

        // 4. Tier filtering (Enhanced - if enabled)
        let filtered_results = if self.features.tier_filtering {
            raw_results.into_iter()
                .filter(|r| self.should_include_tier(r.memory.tier, &request.tier_filter))
                .collect()
        } else {
            raw_results
        };

        // 5. Entity boosting (Enhanced - if enabled)
        let boosted_results = if self.features.entity_extraction {
            self.entity_matcher.boost_results(filtered_results, &request.query).await?
        } else {
            filtered_results
        };

        // 6. Temporal filtering (Enhanced - if enabled)
        let temporal_filtered = if self.features.temporal_extraction {
            self.temporal_matcher.filter_results(boosted_results, &request).await?
        } else {
            boosted_results
        };

        // 7. Secrets redaction (Enhanced - if enabled)
        let redacted_results = if self.features.secrets_filtering {
            temporal_filtered.into_iter()
                .map(|mut r| {
                    r.memory.content = self.secrets_filter.redact(&r.memory.content);
                    r
                })
                .collect()
        } else {
            temporal_filtered
        };

        // 8. Hydrate results with URNs
        let final_results: Vec<_> = redacted_results.into_iter()
            .map(|r| MemoryResult {
                uri: MemoryResourceHandler::build_uri(
                    r.memory.domain.clone(),
                    r.memory.namespace,
                    &MemoryId(r.memory.id.clone()),
                ),
                ..r
            })
            .collect();

        // 9. Publish event
        self.event_bus.publish(MemoryEvent::SearchCompleted {
            query: request.query,
            result_count: final_results.len(),
            latency_ms: span.elapsed().as_millis() as u64,
        });

        Ok(SearchResult {
            results: final_results,
            total: final_results.len(),
            resource_template: "subcog://{domain}/{namespace}/{id}".into(),
        })
    }
}
```

---

## 5. Configuration Composition

### 5.1 Unified Configuration Structure

```toml
# memory.toml - Complete configuration with feature integration

# ===== CORE (always active) =====
[storage]
# Domain-specific storage backends
[storage.project]
persistence = "git-notes"
index = "sqlite"
vector = "usearch"

[storage.user]
persistence = "sqlite"
index = "sqlite"
vector = "usearch"
path = "~/.local/share/memory/user.db"

[storage.org]
persistence = "postgresql"
index = "postgresql"
vector = "pgvector"
url = "postgresql://user:pass@host:5432/memories"

[embedding]
model = "all-MiniLM-L6-v2"
# Fallback if model unavailable
fallback_to_bm25 = true

[search]
# Default search mode
default_mode = "hybrid"
# RRF fusion constant
rrf_k = 60
# Default result limit
default_limit = 10

# ===== ENHANCED (opt-in, no external deps) =====
[features.enhanced]
# Entity extraction for smart matching
entity_extraction = true
# Temporal extraction for date filtering
temporal_extraction = true
# Secrets filtering
secrets_filtering = true
# Advanced observability
advanced_observability = true

[secrets]
# See STORAGE_AND_OBSERVABILITY.md for full config
enabled = true
strategy = "redact"
pii_enabled = true
audit_enabled = true

[observability]
# See STORAGE_AND_OBSERVABILITY.md for full config
otlp_endpoint = "http://localhost:4318"
log_level = "info"
metrics_enabled = true
tracing_enabled = true

# ===== LLM (opt-in, requires provider) =====
[features.llm]
enabled = false
# If enabled, all below become available
implicit_capture = false
consolidation = false
supersession_detection = false
temporal_reasoning = false
query_expansion = false

[llm]
# Provider: anthropic | openai | ollama | lmstudio
provider = "anthropic"
model = "claude-3-5-sonnet-20241022"
api_key_env = "ANTHROPIC_API_KEY"
temperature = 0.3
max_tokens = 1000
timeout_seconds = 30

[consolidation]
# Automatic triggers
interval_hours = 24
memory_threshold = 50
# Tier thresholds
tier_hot = 0.7
tier_warm = 0.4
tier_cold = 0.2
# Clustering
cluster_min_size = 3
cluster_max_size = 10
similarity_threshold = 0.8

# ===== HOOKS (Enhanced tier) =====
[hooks]
enabled = true

[hooks.session_start]
enabled = true
fetch_remote = false
context_token_budget = 2000
include_guidance = true
# Integration: Uses recall service with tier filtering

[hooks.user_prompt]
enabled = true
signal_patterns = ["[decision]", "[learned]", "[blocker]"]
# Integration: Triggers capture service

[hooks.post_tool_use]
enabled = true
trigger_tools = ["Read", "Edit", "Write"]
# Integration: Uses recall service for context

[hooks.pre_compact]
enabled = true
confidence_threshold = 0.8
# Integration: Uses LLM for analysis if enabled

[hooks.stop]
enabled = true
push_remote = false
# Integration: Uses sync service

# ===== ACCESS INTERFACES =====
[cli]
default_format = "text"
color = true

[mcp]
server_name = "memory"
subscriptions = true
# Integration: All services exposed as tools

[streaming]
sse_keepalive_seconds = 15
ws_ping_seconds = 30
```

### 5.2 Configuration Validation

```rust
impl Config {
    /// Validate configuration for internal consistency
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Rule: LLM features require llm.enabled
        if !self.features.llm.enabled {
            if self.features.llm.implicit_capture {
                return Err(ConfigError::InvalidFeature(
                    "implicit_capture requires llm.enabled = true".into()
                ));
            }
            if self.features.llm.consolidation {
                return Err(ConfigError::InvalidFeature(
                    "consolidation requires llm.enabled = true".into()
                ));
            }
            // ... check all LLM features
        }

        // Rule: Consolidation requires storage to support tiers
        if self.features.llm.consolidation {
            for (domain, storage) in &self.storage.domains {
                if storage.persistence == "git-notes" && !storage.index.supports_tiers() {
                    return Err(ConfigError::InvalidStorage(
                        format!("consolidation requires tier support, {} doesn't support it", domain)
                    ));
                }
            }
        }

        // Rule: OTLP endpoint required if advanced observability enabled
        if self.features.enhanced.advanced_observability {
            if self.observability.otlp_endpoint.is_empty() {
                return Err(ConfigError::MissingRequired(
                    "observability.otlp_endpoint required when advanced_observability = true".into()
                ));
            }
        }

        // Rule: Hooks require their dependencies
        if self.hooks.session_start.fetch_remote {
            // Validate git remote is configured
            if !self.has_git_remote() {
                return Err(ConfigError::MissingRequired(
                    "hooks.session_start.fetch_remote requires git remote".into()
                ));
            }
        }

        Ok(())
    }
}
```

---

## 6. Error Propagation

### 6.1 Error Chain Design

Errors propagate through the system with full context:

```rust
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("Capture failed: {0}")]
    Capture(#[source] CaptureError),

    #[error("Recall failed: {0}")]
    Recall(#[source] RecallError),

    #[error("Sync failed: {0}")]
    Sync(#[source] SyncError),

    #[error("Consolidation failed: {0}")]
    Consolidation(#[source] ConsolidationError),
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Content blocked by security filter")]
    SecurityBlocked {
        #[source]
        detection: SecretDetection,
    },

    #[error("Embedding generation failed")]
    Embedding(#[source] EmbeddingError),

    #[error("Storage write failed")]
    Storage(#[source] StorageError),

    #[error("Git operation failed")]
    Git(#[source] git2::Error),
}

impl CaptureError {
    /// Determine if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Validation(_) => false,  // Fix input
            Self::SecurityBlocked { .. } => false,  // Remove secrets
            Self::Embedding(_) => true,  // Retry or fallback
            Self::Storage(_) => true,  // Retry
            Self::Git(_) => true,  // Retry
        }
    }

    /// Get recovery suggestion
    pub fn recovery_hint(&self) -> &'static str {
        match self {
            Self::Validation(_) => "Check input constraints",
            Self::SecurityBlocked { .. } => "Remove detected secrets and retry",
            Self::Embedding(_) => "Will fallback to BM25-only indexing",
            Self::Storage(_) => "Retrying automatically",
            Self::Git(_) => "Check git repository state",
        }
    }
}
```

### 6.2 Graceful Degradation Matrix

| Error | Affected Features | Degraded Behavior | User Experience |
|-------|-------------------|-------------------|-----------------|
| Embedding fails | Vector search | BM25-only search | "Search may be less accurate" |
| LLM unavailable | Tier 3 features | Disabled | "Enhanced features unavailable" |
| Git remote unreachable | Remote sync | Local only | "Changes not synced to remote" |
| OTLP endpoint down | Telemetry export | Buffer locally | No user impact |
| Secrets filter crash | Capture | Block with warning | "Security check failed, content blocked" |
| Storage full | All writes | Queue for retry | "Storage full, queuing..." |

### 6.3 Error Event Propagation

```rust
impl CaptureService {
    async fn handle_error(&self, error: &CaptureError, context: &CaptureContext) {
        // 1. Log with full context
        tracing::error!(
            error = %error,
            memory_id = ?context.memory_id,
            namespace = %context.namespace,
            "Capture failed"
        );

        // 2. Emit metric
        self.metrics.capture_errors.with_label_values(&[error.code()]).inc();

        // 3. Publish event for interested handlers
        self.event_bus.publish(MemoryEvent::CaptureError {
            error: error.to_string(),
            context: context.clone(),
            recoverable: error.is_recoverable(),
        });

        // 4. Trigger recovery if possible
        if error.is_recoverable() {
            self.retry_queue.push(context.clone());
        }
    }
}
```

---

## 7. Observability Integration

### 7.1 Trace Context Propagation

Every operation propagates trace context through all layers:

```rust
/// Trace context that flows through entire request
#[derive(Clone)]
pub struct RequestContext {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub interface: InterfaceType,
    pub session_id: Option<String>,
    pub user_id: Option<String>,
}

impl RequestContext {
    /// Create child span for sub-operation
    pub fn child_span(&self, name: &str) -> tracing::Span {
        tracing::info_span!(
            parent: None,
            name,
            trace_id = %self.trace_id,
            parent_span_id = %self.span_id,
            interface = %self.interface,
        )
    }
}

// Example: Full trace through capture
// [trace: abc123]
// └── [span: capture] (CLI)
//     ├── [span: validate] (2ms)
//     ├── [span: secrets_filter] (5ms)
//     │   └── [span: pii_detect] (2ms)
//     ├── [span: embed] (15ms)
//     │   └── [span: model_inference] (12ms)
//     └── [span: storage_write] (8ms)
//         ├── [span: git_notes_write] (3ms)
//         ├── [span: sqlite_insert] (2ms)
//         └── [span: usearch_add] (3ms)
```

### 7.2 Metrics Integration

```rust
/// Metrics that integrate across all features
pub struct IntegratedMetrics {
    // Capture metrics
    pub captures_total: IntCounterVec,
    pub capture_latency: Histogram,
    pub capture_errors: IntCounterVec,

    // Recall metrics
    pub searches_total: IntCounterVec,
    pub search_latency: Histogram,
    pub search_result_count: Histogram,

    // Storage metrics
    pub storage_operations: IntCounterVec,
    pub storage_latency: HistogramVec,
    pub storage_size_bytes: IntGaugeVec,

    // LLM metrics (Tier 3)
    pub llm_requests: IntCounterVec,
    pub llm_tokens: IntCounterVec,
    pub llm_latency: Histogram,
    pub llm_errors: IntCounterVec,

    // Security metrics
    pub secrets_detected: IntCounterVec,
    pub secrets_blocked: IntCounter,

    // Hook metrics
    pub hook_calls: IntCounterVec,
    pub hook_latency: HistogramVec,
    pub hook_errors: IntCounterVec,

    // Consolidation metrics
    pub consolidation_runs: IntCounter,
    pub clusters_created: IntCounter,
    pub summaries_generated: IntCounter,
    pub tier_changes: IntCounterVec,
}

impl IntegratedMetrics {
    pub fn new() -> Self {
        Self {
            captures_total: register_int_counter_vec!(
                "memory_captures_total",
                "Total memory captures",
                &["namespace", "domain", "status"]
            ).unwrap(),

            // ... register all metrics

            // Cross-cutting metric example
            storage_operations: register_int_counter_vec!(
                "memory_storage_operations_total",
                "Total storage operations",
                &["operation", "backend", "domain", "status"]
            ).unwrap(),
        }
    }
}
```

---

## 8. URN Integration

### 8.1 URN Generation

Every memory operation produces a URN:

```rust
/// Generate URN for any memory reference
pub fn generate_urn(memory: &Memory) -> String {
    let domain_prefix = match &memory.domain {
        Domain::Project(id) => format!("project:{}", id),
        Domain::User => "user".to_string(),
        Domain::Org(id) => format!("org:{}", id),
    };

    format!(
        "subcog://project/{}/{}/{}",
        domain_prefix,
        memory.namespace.as_str(),
        memory.id
    )
}

/// Parse URN into components
pub fn parse_urn(urn: &str) -> Result<MemoryReference> {
    let url = url::Url::parse(urn)?;

    if url.scheme() != "subcog" {
        return Err(UrnError::InvalidScheme);
    }

    let segments: Vec<_> = url.path_segments()
        .ok_or(UrnError::InvalidPath)?
        .collect();

    match segments.as_slice() {
        ["mem", domain, namespace, id] => {
            Ok(MemoryReference {
                domain: Domain::parse(domain)?,
                namespace: Namespace::from_str(namespace)?,
                id: MemoryId(id.to_string()),
            })
        }
        _ => Err(UrnError::InvalidPath),
    }
}
```

### 8.2 URN in All Responses

```rust
/// Ensure URN is included in all responses
trait WithUrn {
    fn with_urn(&mut self, urn: String);
}

impl WithUrn for CaptureResult {
    fn with_urn(&mut self, urn: String) {
        self.uri = urn;
    }
}

impl WithUrn for MemoryResult {
    fn with_urn(&mut self, urn: String) {
        self.uri = urn;
    }
}

// Applied in all services
impl CaptureService {
    pub async fn capture(&self, request: CaptureRequest) -> Result<CaptureResult> {
        let memory = self.create_memory(&request)?;
        let urn = generate_urn(&memory);

        // ... perform capture ...

        let mut result = CaptureResult { ... };
        result.with_urn(urn);
        Ok(result)
    }
}
```

---

## 9. Testing Integration

### 9.1 Integration Test Pattern

```rust
/// Integration test for full feature composition
#[tokio::test]
async fn test_capture_recall_consolidate_integration() {
    let services = TestServices::new_with_features(FeatureFlags {
        secrets_filtering: true,
        entity_extraction: true,
        llm_enabled: true,
        consolidation: true,
        ..Default::default()
    });

    // 1. Capture with secrets filtering
    let content = "API key is sk-1234567890abcdef. Decision: use JWT.";
    let result = services.capture()
        .capture(CaptureRequest {
            namespace: Namespace::Decisions,
            summary: "Use JWT tokens".into(),
            content: content.into(),
            ..Default::default()
        })
        .await
        .unwrap();

    // Verify secrets were redacted
    let memory = services.storage().get(&result.memory_id).await.unwrap();
    assert!(memory.content.contains("[REDACTED:api_key]"));
    assert!(!memory.content.contains("sk-1234567890"));

    // Verify URN format
    assert!(result.uri.starts_with("subcog://project/"));

    // 2. Recall should find the memory
    let search_result = services.recall()
        .search(SearchRequest {
            query: "JWT authentication".into(),
            mode: SearchMode::Hybrid,
            limit: 10,
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(!search_result.results.is_empty());
    assert_eq!(search_result.results[0].uri, result.uri);

    // 3. Consolidation should create clusters
    let consol_result = services.consolidation()
        .run(ConsolidationRequest { dry_run: false, full: false })
        .await
        .unwrap();

    assert!(consol_result.clusters_created >= 0);

    // 4. Verify events were published
    let events = services.event_bus().events();
    assert!(events.iter().any(|e| matches!(e, MemoryEvent::MemoryCaptured { .. })));
    assert!(events.iter().any(|e| matches!(e, MemoryEvent::SearchCompleted { .. })));
}
```

### 9.2 Feature Matrix Test

```rust
/// Test all feature combinations
#[tokio::test]
async fn test_feature_matrix() {
    let feature_combinations = vec![
        FeatureFlags::core_only(),
        FeatureFlags::enhanced_only(),
        FeatureFlags::all_enabled(),
        FeatureFlags {
            secrets_filtering: true,
            llm_enabled: false,
            ..Default::default()
        },
    ];

    for features in feature_combinations {
        let services = TestServices::new_with_features(features.clone());

        // Core operations should always work
        let result = services.capture()
            .capture(basic_capture_request())
            .await;
        assert!(result.is_ok(), "Core capture failed with {:?}", features);

        let search = services.recall()
            .search(basic_search_request())
            .await;
        assert!(search.is_ok(), "Core search failed with {:?}", features);

        // LLM operations should gracefully handle disabled state
        if !features.llm_enabled {
            let consol = services.consolidation()
                .run(ConsolidationRequest::default())
                .await;
            assert!(matches!(consol, Err(MemoryError::FeatureNotEnabled(_))));
        }
    }
}
```

---

## 10. Document Summary

This document ensures that:

1. **All features flow through the same service layer** - no bypass paths
2. **Events propagate to all interested components** - via the event bus
3. **Errors propagate with full context** - enabling debugging and recovery
4. **Observability spans the entire system** - traces, metrics, logs integrated
5. **URNs are consistent everywhere** - `subcog://{domain}/{namespace}/{id}`
6. **Configuration validates feature dependencies** - no orphaned features
7. **Tests verify feature composition** - matrix testing all combinations

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-28 | Claude Opus 4.5 | Initial specification |
