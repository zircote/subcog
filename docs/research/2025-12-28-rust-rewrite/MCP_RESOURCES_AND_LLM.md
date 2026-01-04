# MCP Resources and LLM Provider Requirements

**CRITICAL**: This document defines mandatory MCP resource URN scheme and LLM provider support.

---

## 1. Memory Resource URN Scheme

### 1.1 URN Format

All memories are exposed as MCP resources using a consistent URN scheme that includes domain:

```
subcog://{domain}/{namespace}/{memory_id}
```

**Components:**
- `subcog://` - Protocol identifier for the subconsciousness memory system
- `{domain}` - Memory domain scope (first path segment):
  - `project:{project-id}` - Project-scoped memories (e.g., `project:my-app`)
  - `user` - User-level global memories (cross-project)
  - `org:{org-id}` - Organization-level shared memories (e.g., `org:acme-corp`)
- `{namespace}` - Memory namespace (decisions, learnings, blockers, etc.)
- `{memory_id}` - Unique memory identifier (e.g., `abc1234:0`)

**Examples:**
```
subcog://project:my-app/decisions/abc1234:0
subcog://project:auth-service/learnings/def5678:1
subcog://user/patterns/ghi9012:0
subcog://user/preferences/jkl3456:2
subcog://org:acme-corp/standards/mno7890:0
```

### 1.2 Domain Hierarchy and Storage Mapping

Each domain maps to an appropriate storage backend:

| Domain | Scope | Default Storage | Vector Backend | Use Case |
|--------|-------|-----------------|----------------|----------|
| `project:{id}` | Repository | **Git Notes** | usearch (local) | Project decisions, progress, blockers |
| `user` | Personal | **SQLite** | usearch (local) | Cross-project learnings, preferences |
| `org:{id}` | Organization | **PostgreSQL** | pgvector | Shared standards, patterns, compliance |

```
┌─────────────────────────────────────────────────────────────────────┐
│                   DOMAIN HIERARCHY + STORAGE                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  org:{org-id}  ────────────────────────────► PostgreSQL + pgvector │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ Organization-wide patterns, standards, compliance           │   │
│  │ Shared across all projects and users in the org             │   │
│  │ Multi-user access, team collaboration                       │   │
│  │ Example: subcog://org:acme-corp/standards/sec-001       │   │
│  └─────────────────────────────────────────────────────────────┘   │
│       │                                                             │
│       ▼                                                             │
│  user  ─────────────────────────────────────► SQLite + usearch     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ Personal learnings, preferences, cross-project patterns     │   │
│  │ Follows user across all projects (local machine)            │   │
│  │ ~/.local/share/memory/user.db                               │   │
│  │ Example: subcog://user/learnings/rust-tips-001          │   │
│  └─────────────────────────────────────────────────────────────┘   │
│       │                                                             │
│       ▼                                                             │
│  project:{project-id}  ─────────────────────► Git Notes + usearch  │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ Project-specific decisions, blockers, progress              │   │
│  │ Scoped to single repository, syncs with git remote          │   │
│  │ refs/notes/mem/{namespace}                                  │   │
│  │ Example: subcog://project:my-app/decisions/db-choice    │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.3 Multi-Domain Configuration

```toml
# memory.toml - Domain-specific storage configuration

[domains.project]
# Git notes for project-scoped memories
persistence = "git-notes"
index = "sqlite"           # Local index in .memory/index.db
vector = "usearch"         # Local vector in .memory/index.usearch

[domains.user]
# SQLite for user-level memories
persistence = "sqlite"     # ~/.local/share/memory/user.db
index = "sqlite"           # Same database
vector = "usearch"         # ~/.local/share/memory/user.usearch

[domains.org]
# PostgreSQL for organization-level memories
persistence = "postgresql"
index = "postgresql"
vector = "pgvector"
postgres_url = "postgresql://user:pass@org-db.example.com:5432/memories"
```

### 1.2 Resource Templates

MCP resource templates enable dynamic resource discovery:

```json
{
  "resourceTemplates": [
    {
      "uriTemplate": "subcog://{domain}/{namespace}/{memory_id}",
      "name": "Memory Resource",
      "description": "Access a specific memory by namespace and ID",
      "mimeType": "application/json"
    },
    {
      "uriTemplate": "subcog://{domain}/{namespace}",
      "name": "Namespace Listing",
      "description": "List all memories in a namespace",
      "mimeType": "application/json"
    },
    {
      "uriTemplate": "subcog://{domain}",
      "name": "All Memories",
      "description": "List all memory namespaces and counts",
      "mimeType": "application/json"
    }
  ]
}
```

### 1.3 Resource Responses

**Single Memory Resource:**
```json
{
  "uri": "subcog://project/decisions/abc1234:0",
  "name": "Use PostgreSQL for data layer",
  "description": "Architecture decision for database selection",
  "mimeType": "application/json",
  "contents": {
    "id": "decisions:abc1234:0",
    "namespace": "decisions",
    "domain": "project",
    "summary": "Use PostgreSQL for data layer",
    "content": "## Context\nWe need a reliable database...",
    "timestamp": "2025-01-15T10:30:00Z",
    "spec": "my-project",
    "tags": ["database", "architecture"],
    "status": "active",
    "relates_to": ["decisions:xyz7890:0"]
  }
}
```

**Namespace Listing:**
```json
{
  "uri": "subcog://project/decisions",
  "name": "Decisions",
  "description": "Architecture Decision Records (45 memories)",
  "mimeType": "application/json",
  "contents": {
    "namespace": "decisions",
    "total": 45,
    "by_domain": {
      "project": 40,
      "user": 5
    },
    "memories": [
      {
        "uri": "subcog://project/decisions/abc1234:0",
        "summary": "Use PostgreSQL for data layer",
        "timestamp": "2025-01-15T10:30:00Z"
      }
    ]
  }
}
```

### 1.4 Integration with additionalContext

Memory URNs are embedded in Claude Code's `additionalContext` for deep integration:

```xml
<memory_context>
  <project>my-project</project>
  <spec>feature-auth</spec>
  <resources>
    <resource uri="subcog://project/decisions/abc1234:0" relevance="0.92">
      <summary>Use JWT for authentication</summary>
      <namespace>decisions</namespace>
      <timestamp>2025-01-10T14:00:00Z</timestamp>
    </resource>
    <resource uri="subcog://project/learnings/def5678:1" relevance="0.85">
      <summary>Token refresh flow reduces logout issues</summary>
      <namespace>learnings</namespace>
      <timestamp>2025-01-12T09:30:00Z</timestamp>
    </resource>
  </resources>
  <resource_template uri="subcog://{domain}/{namespace}/{id}" />
</memory_context>
```

### 1.5 Tool Integration with Resource URNs

**All memory tools MUST use resource URNs for consistency:**

#### memory.capture Response:
```json
{
  "success": true,
  "resource": {
    "uri": "subcog://project/decisions/abc1234:0",
    "name": "Use PostgreSQL for data layer"
  },
  "indexed": true
}
```

#### memory.recall Response:
```json
{
  "results": [
    {
      "uri": "subcog://project/decisions/abc1234:0",
      "namespace": "decisions",
      "summary": "Use PostgreSQL for data layer",
      "relevance": 0.92,
      "domain": "project"
    }
  ],
  "resource_template": "subcog://{domain}/{namespace}/{id}"
}
```

#### memory.status Response:
```json
{
  "total_memories": 150,
  "resource_base": "subcog://{domain}",
  "namespaces": [
    {
      "uri": "subcog://project/decisions",
      "name": "decisions",
      "count": 45
    },
    {
      "uri": "subcog://project/learnings",
      "name": "learnings",
      "count": 80
    }
  ]
}
```

### 1.6 MCP Resource Implementation

```rust
/// MCP Resource handler for memory URNs
pub struct MemoryResourceHandler {
    storage: Arc<CompositeStorage>,
}

impl MemoryResourceHandler {
    /// Parse a memory URN into components
    pub fn parse_uri(uri: &str) -> Result<MemoryResourcePath> {
        // subcog://project/decisions/abc1234:0
        let parsed = url::Url::parse(uri)?;

        if parsed.scheme() != "subcog" {
            return Err(ResourceError::InvalidScheme);
        }

        let path_segments: Vec<_> = parsed.path_segments()
            .ok_or(ResourceError::InvalidPath)?
            .collect();

        match path_segments.as_slice() {
            [domain] => Ok(MemoryResourcePath::Domain(domain.to_string())),
            [domain, namespace] => Ok(MemoryResourcePath::Namespace {
                domain: domain.to_string(),
                namespace: Namespace::from_str(namespace)?,
            }),
            [domain, namespace, memory_id] => Ok(MemoryResourcePath::Memory {
                domain: domain.to_string(),
                namespace: Namespace::from_str(namespace)?,
                id: MemoryId(memory_id.to_string()),
            }),
            _ => Err(ResourceError::InvalidPath),
        }
    }

    /// Build a memory URN from components
    pub fn build_uri(domain: &str, namespace: Namespace, id: &MemoryId) -> String {
        format!("subcog://{}/{}/{}", domain, namespace.as_str(), id.0)
    }

    /// Build a namespace URI
    pub fn namespace_uri(domain: &str, namespace: Namespace) -> String {
        format!("subcog://{}/{}", domain, namespace.as_str())
    }
}

#[derive(Debug)]
pub enum MemoryResourcePath {
    Domain(String),
    Namespace { domain: String, namespace: Namespace },
    Memory { domain: String, namespace: Namespace, id: MemoryId },
}

/// MCP resources/list handler
pub async fn handle_resources_list(
    handler: &MemoryResourceHandler,
    cursor: Option<String>,
) -> Result<ListResourcesResult> {
    let stats = handler.storage.stats().await?;
    let domain = "project";

    let resources: Vec<Resource> = stats.by_namespace.iter()
        .map(|(ns, count)| Resource {
            uri: MemoryResourceHandler::namespace_uri(domain, *ns),
            name: ns.as_str().to_string(),
            description: Some(format!("{} memories", count)),
            mime_type: Some("application/json".to_string()),
        })
        .collect();

    Ok(ListResourcesResult {
        resources,
        next_cursor: None,
    })
}

/// MCP resources/read handler
pub async fn handle_resource_read(
    handler: &MemoryResourceHandler,
    uri: &str,
) -> Result<ReadResourceResult> {
    let path = MemoryResourceHandler::parse_uri(uri)?;

    match path {
        MemoryResourcePath::Domain(domain) => {
            let stats = handler.storage.stats().await?;
            Ok(ReadResourceResult {
                contents: vec![ResourceContents::Text {
                    uri: format!("subcog://{}", domain),
                    mime_type: Some("application/json".to_string()),
                    text: serde_json::to_string(&stats)?,
                }],
            })
        }
        MemoryResourcePath::Namespace { domain, namespace: ns } => {
            let memories = handler.storage.list_namespace(ns).await?;
            Ok(ReadResourceResult {
                contents: vec![ResourceContents::Text {
                    uri: MemoryResourceHandler::namespace_uri(&domain, ns),
                    mime_type: Some("application/json".to_string()),
                    text: serde_json::to_string(&memories)?,
                }],
            })
        }
        MemoryResourcePath::Memory { namespace, id } => {
            let memory = handler.storage.get(&id).await?
                .ok_or(ResourceError::NotFound)?;
            Ok(ReadResourceResult {
                contents: vec![ResourceContents::Text {
                    uri: MemoryResourceHandler::build_uri(namespace, &id),
                    mime_type: Some("application/json".to_string()),
                    text: serde_json::to_string(&memory)?,
                }],
            })
        }
    }
}

/// MCP resources/templates handler
pub fn handle_resource_templates() -> ListResourceTemplatesResult {
    ListResourceTemplatesResult {
        resource_templates: vec![
            ResourceTemplate {
                uri_template: "subcog://{domain}/{namespace}/{memory_id}".to_string(),
                name: "Memory Resource".to_string(),
                description: Some("Access a specific memory".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceTemplate {
                uri_template: "subcog://{domain}/{namespace}".to_string(),
                name: "Namespace Listing".to_string(),
                description: Some("List memories in a namespace".to_string()),
                mime_type: Some("application/json".to_string()),
            },
        ],
    }
}
```

---

## 2. LLM Provider Requirements

### 2.1 Supported Providers

| Provider | API | Models | Use Case |
|----------|-----|--------|----------|
| **Anthropic** | REST | claude-3-5-sonnet, claude-3-opus | Cloud, production |
| **OpenAI** | REST | gpt-4o, gpt-4o-mini | Cloud, production |
| **Ollama** | HTTP | llama3, mistral, codellama | Local, offline, privacy |
| **LMStudio** | OpenAI-compatible | Any GGUF model | Local, offline, privacy |

### 2.2 Provider Trait

```rust
/// LLM provider abstraction
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Send a chat completion request
    async fn chat(&self, request: &LLMRequest) -> Result<LLMResponse, LLMError>;

    /// Provider name for logging/metrics
    fn name(&self) -> &'static str;

    /// Check if provider is available
    async fn health_check(&self) -> bool;

    /// Get provider capabilities
    fn capabilities(&self) -> ProviderCapabilities;
}

#[derive(Debug, Clone)]
pub struct ProviderCapabilities {
    /// Supports streaming responses
    pub streaming: bool,
    /// Supports function/tool calling
    pub tools: bool,
    /// Supports JSON mode
    pub json_mode: bool,
    /// Maximum context length
    pub max_context: usize,
    /// Supports vision/multimodal
    pub vision: bool,
}
```

### 2.3 Anthropic Provider

```rust
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model: model.unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string()),
            base_url: "https://api.anthropic.com".to_string(),
        }
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    async fn chat(&self, request: &LLMRequest) -> Result<LLMResponse, LLMError> {
        let response = self.client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&AnthropicRequest::from(request))
            .send()
            .await?;

        // Parse and return
        let anthropic_response: AnthropicResponse = response.json().await?;
        Ok(LLMResponse::from(anthropic_response))
    }

    fn name(&self) -> &'static str { "anthropic" }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tools: true,
            json_mode: true,
            max_context: 200_000,
            vision: true,
        }
    }
}
```

### 2.4 OpenAI Provider

```rust
pub struct OpenAIProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: Option<String>, base_url: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model: model.unwrap_or_else(|| "gpt-4o-mini".to_string()),
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com".to_string()),
        }
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn chat(&self, request: &LLMRequest) -> Result<LLMResponse, LLMError> {
        let response = self.client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&OpenAIRequest::from(request))
            .send()
            .await?;

        let openai_response: OpenAIResponse = response.json().await?;
        Ok(LLMResponse::from(openai_response))
    }

    fn name(&self) -> &'static str { "openai" }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tools: true,
            json_mode: true,
            max_context: 128_000,
            vision: true,
        }
    }
}
```

### 2.5 Ollama Provider

```rust
/// Ollama - local model server
/// Runs models locally for offline/privacy use cases
pub struct OllamaProvider {
    client: reqwest::Client,
    model: String,
    base_url: String,
}

impl OllamaProvider {
    pub fn new(model: Option<String>, base_url: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            model: model.unwrap_or_else(|| "llama3".to_string()),
            base_url: base_url.unwrap_or_else(|| "http://localhost:11434".to_string()),
        }
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    async fn chat(&self, request: &LLMRequest) -> Result<LLMResponse, LLMError> {
        let ollama_request = OllamaRequest {
            model: self.model.clone(),
            messages: request.messages.iter()
                .map(|m| OllamaMessage {
                    role: m.role.to_string(),
                    content: m.content.clone(),
                })
                .collect(),
            stream: false,
            options: Some(OllamaOptions {
                temperature: request.temperature,
                num_predict: request.max_tokens as i32,
            }),
        };

        let response = self.client
            .post(format!("{}/api/chat", self.base_url))
            .json(&ollama_request)
            .send()
            .await?;

        let ollama_response: OllamaResponse = response.json().await?;
        Ok(LLMResponse {
            content: ollama_response.message.content,
            model: self.model.clone(),
            usage: LLMUsage {
                prompt_tokens: ollama_response.prompt_eval_count.unwrap_or(0) as u32,
                completion_tokens: ollama_response.eval_count.unwrap_or(0) as u32,
                total_tokens: (ollama_response.prompt_eval_count.unwrap_or(0)
                             + ollama_response.eval_count.unwrap_or(0)) as u32,
                cost_usd: None,
            },
            latency_ms: ollama_response.total_duration.unwrap_or(0) / 1_000_000,
        })
    }

    async fn health_check(&self) -> bool {
        self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    fn name(&self) -> &'static str { "ollama" }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tools: false,  // Ollama tool support varies by model
            json_mode: true,
            max_context: 8_192,  // Varies by model
            vision: false,  // Varies by model
        }
    }
}
```

### 2.6 LMStudio Provider (OpenAI-compatible)

```rust
/// LMStudio - local model server with OpenAI-compatible API
/// Perfect for running any GGUF model locally
pub struct LMStudioProvider {
    // Reuses OpenAI provider with different base URL
    inner: OpenAIProvider,
}

impl LMStudioProvider {
    pub fn new(model: Option<String>, port: Option<u16>) -> Self {
        let port = port.unwrap_or(1234);
        let base_url = format!("http://localhost:{}", port);

        Self {
            inner: OpenAIProvider::new(
                "lm-studio".to_string(),  // LMStudio doesn't require API key
                model,
                Some(base_url),
            ),
        }
    }
}

#[async_trait]
impl LLMProvider for LMStudioProvider {
    async fn chat(&self, request: &LLMRequest) -> Result<LLMResponse, LLMError> {
        self.inner.chat(request).await
    }

    async fn health_check(&self) -> bool {
        // LMStudio provides /v1/models endpoint
        reqwest::Client::new()
            .get(format!("{}/v1/models", self.inner.base_url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    fn name(&self) -> &'static str { "lmstudio" }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tools: true,  // LMStudio supports OpenAI function calling
            json_mode: true,
            max_context: 32_768,  // Varies by model
            vision: false,  // Depends on model
        }
    }
}
```

### 2.7 Provider Factory

```rust
/// Create LLM provider from configuration
pub fn create_llm_provider(config: &LLMConfig) -> Result<Arc<dyn LLMProvider>> {
    match config.provider.as_str() {
        "anthropic" => {
            let api_key = std::env::var(&config.api_key_env)
                .map_err(|_| LLMError::MissingApiKey("anthropic"))?;
            Ok(Arc::new(AnthropicProvider::new(api_key, config.model.clone())))
        }
        "openai" => {
            let api_key = std::env::var(&config.api_key_env)
                .map_err(|_| LLMError::MissingApiKey("openai"))?;
            Ok(Arc::new(OpenAIProvider::new(api_key, config.model.clone(), None)))
        }
        "ollama" => {
            Ok(Arc::new(OllamaProvider::new(
                config.model.clone(),
                config.base_url.clone(),
            )))
        }
        "lmstudio" => {
            Ok(Arc::new(LMStudioProvider::new(
                config.model.clone(),
                config.port,
            )))
        }
        other => Err(LLMError::UnknownProvider(other.to_string())),
    }
}
```

### 2.8 LLM Configuration

```toml
# memory.toml - LLM configuration

[llm]
# Provider: anthropic | openai | ollama | lmstudio
provider = "anthropic"

# Model (provider-specific)
model = "claude-3-5-sonnet-20241022"

# API key environment variable (for cloud providers)
api_key_env = "ANTHROPIC_API_KEY"

# Base URL override (for custom endpoints)
# base_url = "https://api.anthropic.com"

# Port (for lmstudio)
# port = 1234

# Request defaults
temperature = 0.3
max_tokens = 1000
timeout_seconds = 30

# Rate limiting
requests_per_minute = 60
tokens_per_minute = 100000

# Retry configuration
max_retries = 3
retry_delay_ms = 1000
```

### 2.9 Provider Comparison Matrix

| Provider | Cloud/Local | API Key Required | Models | Best For |
|----------|-------------|------------------|--------|----------|
| Anthropic | Cloud | Yes | Claude 3.5, 3 | Production, quality |
| OpenAI | Cloud | Yes | GPT-4o, 4o-mini | Production, variety |
| Ollama | Local | No | Llama, Mistral, etc. | Offline, privacy |
| LMStudio | Local | No | Any GGUF | Offline, custom models |

---

## 3. Integration Requirements

### 3.1 Resource URNs in All Tools

Every MCP tool MUST return resource URNs:

| Tool | Input URN | Output URN |
|------|-----------|------------|
| `memory.capture` | - | `subcog://{domain}/{namespace}/{id}` |
| `memory.recall` | - | List of `subcog://{domain}/{namespace}/{id}` |
| `memory.status` | - | Namespace URIs |
| `memory.sync` | - | Synced resource URIs |
| `memory.consolidate` | - | Affected resource URIs |

### 3.2 LLM Provider in Tier 3 Features

| Feature | LLM Required | Provider Agnostic |
|---------|--------------|-------------------|
| Implicit capture | Yes | Yes - works with any provider |
| Consolidation | Yes | Yes - works with any provider |
| Temporal reasoning | Yes | Yes - works with any provider |
| Query expansion | Yes | Yes - works with any provider |

### 3.3 Fallback Behavior

If LLM provider is unavailable:
1. Tier 3 features gracefully disable
2. Error logged with provider name
3. Core functionality continues
4. User notified via warning in response
