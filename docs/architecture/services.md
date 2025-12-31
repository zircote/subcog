# Services

Business logic layer implementing core functionality.

> **Note**: This documentation describes the target architecture from the specification.
> Current implementation uses simpler, non-generic patterns. Code examples show the spec
> design; actual implementation may differ. See source code for current state.

## ServiceContainer

Dependency injection container managing all services.

```rust
pub struct ServiceContainer<P, I, V>
where
    P: PersistenceBackend + Clone,
    I: IndexBackend + Clone,
    V: VectorBackend + Clone,
{
    pub capture: CaptureService<P, I, V>,
    pub recall: RecallService<I, V>,
    pub prompt: PromptService<P>,
    pub sync: SyncService<P>,
    pub consolidation: ConsolidationService<P, I, V>,
    pub context: ContextBuilderService<I, V>,
    pub topic_index: TopicIndexService<I>,
}

impl<P, I, V> ServiceContainer<P, I, V> {
    pub fn new(storage: CompositeStorage<P, I, V>, config: Config) -> Self {
        Self {
            capture: CaptureService::new(storage.clone(), config.clone()),
            recall: RecallService::new(storage.index.clone(), storage.vector.clone()),
            // ...
        }
    }
}
```

## CaptureService

Handles memory capture with validation and indexing.

```rust
pub struct CaptureService<P, I, V> {
    storage: CompositeStorage<P, I, V>,
    embedder: Embedder,
    security: SecurityService,
    config: Config,
}

impl<P, I, V> CaptureService<P, I, V>
where
    P: PersistenceBackend,
    I: IndexBackend,
    V: VectorBackend,
{
    pub async fn capture(&self, request: CaptureRequest) -> Result<CaptureResult> {
        // 1. Security check
        self.security.check_content(&request.content)?;

        // 2. Create memory
        let memory = Memory {
            id: MemoryId::new(),
            namespace: request.namespace,
            content: request.content,
            tags: request.tags,
            // ...
        };

        // 3. Generate embedding
        let embedding = self.embedder.embed(&memory.content)?;

        // 4. Store in persistence layer
        self.storage.persistence.store(&memory).await?;

        // 5. Index for search
        self.storage.index.index(&memory).await?;

        // 6. Store vector
        self.storage.vector.store(&memory.id, &embedding).await?;

        Ok(CaptureResult {
            id: memory.id,
            urn: memory.to_urn(),
        })
    }
}
```

### Capture Pipeline

```
Request → Security → Memory → Embedding → Persistence → Index → Vector → Result
```

## RecallService

Handles memory search with hybrid ranking.

```rust
pub struct RecallService<I, V> {
    index: I,
    vector: V,
    embedder: Embedder,
}

impl<I, V> RecallService<I, V>
where
    I: IndexBackend,
    V: VectorBackend,
{
    pub async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>> {
        match query.mode {
            SearchMode::Hybrid => self.hybrid_search(&query).await,
            SearchMode::Vector => self.vector_search(&query).await,
            SearchMode::Text => self.text_search(&query).await,
        }
    }

    async fn hybrid_search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        // 1. Generate query embedding
        let query_vector = self.embedder.embed(&query.text)?;

        // 2. Parallel search
        let (vector_results, text_results) = tokio::join!(
            self.vector.search(&query_vector, query.limit * 2),
            self.index.search(&IndexQuery::from(query))
        );

        // 3. RRF fusion
        let fused = rrf_fusion(vector_results?, text_results?, 60);

        // 4. Apply limit
        Ok(fused.into_iter().take(query.limit).collect())
    }
}
```

### Search Modes

| Mode | Algorithm | Latency |
|------|-----------|---------|
| Hybrid | RRF(vector + BM25) | ~50ms |
| Vector | Semantic only | ~30ms |
| Text | BM25 only | ~20ms |

## PromptService

Manages prompt template CRUD and execution.

```rust
pub struct PromptService<P> {
    storage: P,
    parser: PromptParser,
}

impl<P: PersistenceBackend> PromptService<P> {
    pub async fn save(&self, template: PromptTemplate) -> Result<()> {
        // Validate template
        self.parser.validate(&template)?;

        // Store in persistence
        self.storage.store_prompt(&template).await
    }

    pub async fn run(
        &self,
        name: &str,
        variables: HashMap<String, String>
    ) -> Result<String> {
        // Get template
        let template = self.get(name).await?;

        // Validate required variables
        for var in &template.variables {
            if var.required && !variables.contains_key(&var.name) {
                if var.default.is_none() {
                    return Err(Error::MissingVariable(var.name.clone()));
                }
            }
        }

        // Substitute variables
        Ok(self.parser.substitute(&template.content, &variables))
    }
}
```

## SyncService

Handles Git remote synchronization.

```rust
pub struct SyncService<P> {
    persistence: P,
    git: GitOperations,
}

impl<P: PersistenceBackend> SyncService<P> {
    pub async fn sync(&self, direction: SyncDirection) -> Result<SyncResult> {
        match direction {
            SyncDirection::Push => self.push().await,
            SyncDirection::Fetch => self.fetch().await,
            SyncDirection::Full => {
                let fetched = self.fetch().await?;
                let pushed = self.push().await?;
                Ok(SyncResult {
                    fetched: fetched.count,
                    pushed: pushed.count,
                })
            }
        }
    }

    async fn push(&self) -> Result<PushResult> {
        self.git.push_notes("refs/notes/subcog").await?;
        self.git.push_notes("refs/notes/_prompts").await
    }

    async fn fetch(&self) -> Result<FetchResult> {
        self.git.fetch_notes("refs/notes/subcog").await?;
        self.git.fetch_notes("refs/notes/_prompts").await
    }
}
```

## ConsolidationService

LLM-powered memory consolidation.

```rust
pub struct ConsolidationService<P, I, V> {
    storage: CompositeStorage<P, I, V>,
    llm: LlmClient,
}

impl<P, I, V> ConsolidationService<P, I, V> {
    pub async fn consolidate(
        &self,
        namespace: Namespace,
        strategy: ConsolidationStrategy,
    ) -> Result<ConsolidationResult> {
        // 1. Find similar memories
        let candidates = self.find_candidates(namespace).await?;

        // 2. Apply strategy
        match strategy {
            ConsolidationStrategy::Merge => {
                self.merge_candidates(&candidates).await
            }
            ConsolidationStrategy::Summarize => {
                self.summarize_candidates(&candidates).await
            }
            ConsolidationStrategy::Dedupe => {
                self.dedupe_candidates(&candidates).await
            }
        }
    }

    async fn merge_candidates(
        &self,
        candidates: &[ConsolidationCandidate]
    ) -> Result<ConsolidationResult> {
        for candidate in candidates {
            // Use LLM to merge similar memories
            let merged_content = self.llm.merge_memories(
                &candidate.memories
            ).await?;

            // Create new memory with merged content
            // Archive original memories
        }
    }
}
```

## ContextBuilderService

Builds adaptive context for hooks.

```rust
pub struct ContextBuilderService<I, V> {
    recall: RecallService<I, V>,
    topic_index: TopicIndexService<I>,
}

impl<I, V> ContextBuilderService<I, V>
where
    I: IndexBackend,
    V: VectorBackend,
{
    pub async fn build_context(
        &self,
        intent: &SearchIntent,
        max_memories: usize,
    ) -> Result<String> {
        // Determine memory count based on confidence
        let count = match intent.confidence {
            c if c >= 0.8 => max_memories,
            c if c >= 0.5 => max_memories * 2 / 3,
            _ => max_memories / 3,
        };

        // Search with weighted namespaces
        let results = self.recall.search_weighted(
            &intent.keywords.join(" "),
            &intent.namespace_weights,
            count,
        ).await?;

        // Format as markdown
        self.format_context(&results)
    }
}
```

## TopicIndexService

Maintains topic → memory mappings.

```rust
pub struct TopicIndexService<I> {
    index: I,
}

impl<I: IndexBackend> TopicIndexService<I> {
    pub async fn get_topics(&self) -> Result<Vec<TopicInfo>> {
        self.index.get_topics().await
    }

    pub async fn get_memories_for_topic(
        &self,
        topic: &str
    ) -> Result<Vec<MemoryId>> {
        self.index.search(&IndexQuery {
            text: topic.to_string(),
            ..Default::default()
        }).await
    }
}
```

## Service Interaction

```
┌─────────────────────────────────────────────────────────┐
│                    ServiceContainer                      │
│                                                          │
│  ┌─────────┐    ┌─────────┐    ┌─────────┐             │
│  │ Capture │◄───│ Storage │───►│ Recall  │             │
│  │ Service │    │ Layers  │    │ Service │             │
│  └────┬────┘    └────┬────┘    └────┬────┘             │
│       │              │              │                    │
│       │         ┌────┴────┐         │                    │
│       │         │Embedder │         │                    │
│       │         └─────────┘         │                    │
│       │                             │                    │
│       ▼                             ▼                    │
│  ┌─────────┐                   ┌─────────┐              │
│  │Security │                   │ Context │              │
│  │ Service │                   │ Builder │              │
│  └─────────┘                   └─────────┘              │
└─────────────────────────────────────────────────────────┘
```

## See Also

- [Models](models.md) - Data structures
- [Search](search.md) - Search implementation
- [Storage](../storage/README.md) - Storage backends
