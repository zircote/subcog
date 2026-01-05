# Access Interface Layer Specification

**CRITICAL**: This document defines ALL access methods to the memory system. Every interface shares the same core services and data models.

---

## 1. Interface Overview

The memory system exposes its functionality through multiple access interfaces, all built on the same core service layer:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         ACCESS INTERFACE LAYER                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐ │
│  │    CLI        │  │  MCP Server   │  │  Streaming    │  │    Hooks      │ │
│  │   (stdio)     │  │  (JSON-RPC)   │  │     API       │  │  (JSON out)   │ │
│  ├───────────────┤  ├───────────────┤  ├───────────────┤  ├───────────────┤ │
│  │ • Interactive │  │ • Tools       │  │ • SSE         │  │ • SessionStart│ │
│  │ • Scripts     │  │ • Resources   │  │ • WebSocket   │  │ • UserPrompt  │ │
│  │ • Pipes       │  │ • Prompts     │  │ • Long-poll   │  │ • PostToolUse │ │
│  │ • One-shot    │  │ • Sampling    │  │               │  │ • PreCompact  │ │
│  │               │  │               │  │               │  │ • Stop        │ │
│  └───────┬───────┘  └───────┬───────┘  └───────┬───────┘  └───────┬───────┘ │
│          │                  │                  │                  │         │
│          └──────────────────┼──────────────────┼──────────────────┘         │
│                             │                  │                            │
│                             ▼                  ▼                            │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                      UNIFIED SERVICE LAYER                           │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │   │
│  │  │ Capture     │  │ Recall      │  │ Sync        │  │ Consolidate │  │   │
│  │  │ Service     │  │ Service     │  │ Service     │  │ Service     │  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘  │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.1 Interface Comparison Matrix

| Interface | Transport | Use Case | Latency | State |
|-----------|-----------|----------|---------|-------|
| **CLI** | stdio (stdin/stdout) | Interactive, scripts, automation | <50ms | Stateless |
| **MCP Server** | stdio / SSE | AI agent integration | <100ms | Session |
| **Streaming API** | HTTP/SSE/WS | Long operations, real-time | N/A | Streaming |
| **Hooks** | Process spawn | Claude Code integration | <100ms | Request-scoped |

### 1.2 Shared Principles

1. **Single Source of Truth**: All interfaces call the same service layer
2. **Consistent Data Models**: Same Memory, MemoryResult types across all interfaces
3. **Resource URNs**: All interfaces return URNs in the `subcog://{domain}/{namespace}/{id}` format
4. **Error Handling**: Consistent error codes and messages
5. **Observability**: All interfaces emit metrics and traces

---

## 2. CLI Interface (stdio)

### 2.1 CLI Architecture

```rust
use clap::{Parser, Subcommand};

/// Git-backed memory system with semantic search
#[derive(Parser)]
#[command(name = "memory")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Configuration file path
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Output format
    #[arg(long, global = true, default_value = "text")]
    format: OutputFormat,

    /// Verbosity level
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Quiet mode (suppress non-essential output)
    #[arg(short, long)]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Capture a new memory
    Capture(CaptureArgs),
    /// Search and recall memories
    Recall(RecallArgs),
    /// Show memory system status
    Status(StatusArgs),
    /// Synchronize memories
    Sync(SyncArgs),
    /// Run memory consolidation
    Consolidate(ConsolidateArgs),
    /// Manage configuration
    Config(ConfigArgs),
    /// Start MCP server
    Serve(ServeArgs),
    /// Run as hook handler
    Hook(HookArgs),
    /// Migrate from Python version
    Migrate(MigrateArgs),
}

#[derive(Clone, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Yaml,
    Quiet,
}
```

### 2.2 CLI Commands

#### 2.2.1 memory capture

```bash
# Basic capture
memory capture decisions "Use PostgreSQL for data layer"

# With content from stdin
echo "## Context\nWe need JSONB..." | memory capture decisions "Use PostgreSQL" --stdin

# With inline content
memory capture learnings "Rust lifetimes" --content "## Key Insight\n..."

# To user domain (global)
memory capture patterns "Error handling pattern" --domain user

# With metadata
memory capture decisions "API versioning" \
  --spec feature-api \
  --tags api,versioning,breaking-changes \
  --relates-to "decisions:abc1234:0"
```

```rust
#[derive(Args)]
pub struct CaptureArgs {
    /// Memory namespace
    #[arg(value_enum)]
    namespace: Namespace,

    /// One-line summary (≤100 chars)
    summary: String,

    /// Memory content (markdown)
    #[arg(long)]
    content: Option<String>,

    /// Read content from stdin
    #[arg(long, conflicts_with = "content")]
    stdin: bool,

    /// Storage domain
    #[arg(long, default_value = "project")]
    domain: Domain,

    /// Specification reference
    #[arg(long)]
    spec: Option<String>,

    /// Tags (comma-separated or repeated)
    #[arg(long, value_delimiter = ',')]
    tags: Vec<String>,

    /// Related memory IDs
    #[arg(long, value_delimiter = ',')]
    relates_to: Vec<String>,
}
```

**Output (text)**:
```
✓ Captured memory: decisions:abc1234:0
  Domain: project
  URI: subcog://project:my-app/decisions/abc1234:0
```

**Output (JSON)**:
```json
{
  "success": true,
  "memory_id": "decisions:abc1234:0",
  "uri": "subcog://project:my-app/decisions/abc1234:0",
  "indexed": true,
  "warning": null
}
```

#### 2.2.2 memory recall

```bash
# Semantic search
memory recall "database architecture decisions"

# With filters
memory recall "auth" --namespace decisions --domain project

# Hybrid search
memory recall "caching strategy" --mode hybrid --limit 5

# JSON output for scripting
memory recall "API design" --format json | jq '.results[].uri'

# With reasoning (Tier 3, requires LLM)
memory recall --reason "when did we decide to use PostgreSQL"
```

```rust
#[derive(Args)]
pub struct RecallArgs {
    /// Search query (natural language)
    query: Option<String>,

    /// Search mode
    #[arg(long, default_value = "hybrid")]
    mode: SearchMode,

    /// Maximum results
    #[arg(long, short, default_value = "10")]
    limit: usize,

    /// Filter by namespace
    #[arg(long)]
    namespace: Option<Namespace>,

    /// Filter by domain
    #[arg(long, default_value = "all")]
    domain: DomainFilter,

    /// Minimum similarity threshold (0.0-1.0)
    #[arg(long)]
    min_similarity: Option<f32>,

    /// Filter by spec
    #[arg(long)]
    spec: Option<String>,

    /// Filter by tags
    #[arg(long, value_delimiter = ',')]
    tags: Vec<String>,

    /// Use LLM reasoning for temporal/complex queries
    #[arg(long)]
    reason: Option<String>,

    /// Include archived memories
    #[arg(long)]
    include_archived: bool,

    /// Output detail level
    #[arg(long, default_value = "summary")]
    detail: DetailLevel,
}

#[derive(Clone, ValueEnum)]
pub enum SearchMode {
    Hybrid,
    Vector,
    Bm25,
}

#[derive(Clone, ValueEnum)]
pub enum DomainFilter {
    All,
    Project,
    User,
    Org,
}

#[derive(Clone, ValueEnum)]
pub enum DetailLevel {
    /// ID and summary only
    Minimal,
    /// Summary and metadata
    Summary,
    /// Full content
    Full,
}
```

**Output (text)**:
```
Found 3 memories:

[1] decisions:abc1234:0 (0.92 similarity)
    Use PostgreSQL for data layer
    Domain: project | Tags: database, architecture
    URI: subcog://project:my-app/decisions/abc1234:0

[2] learnings:def5678:1 (0.85 similarity)
    PostgreSQL JSONB performs well for our use case
    Domain: user | Tags: database, performance
    URI: subcog://user/learnings/def5678:1

[3] decisions:ghi9012:0 (0.78 similarity)
    Use SQLite for local development
    Domain: project | Tags: database, development
    URI: subcog://project:my-app/decisions/ghi9012:0
```

#### 2.2.3 memory status

```bash
# Overview
memory status

# Specific domain
memory status --domain project

# Detailed breakdown
memory status --verbose
```

```rust
#[derive(Args)]
pub struct StatusArgs {
    /// Filter by domain
    #[arg(long, default_value = "all")]
    domain: DomainFilter,

    /// Show detailed breakdown
    #[arg(long, short)]
    verbose: bool,

    /// Verify data integrity
    #[arg(long)]
    verify: bool,
}
```

**Output (text)**:
```
Memory System Status
════════════════════

Total: 150 memories (45 project, 105 user)

By Namespace:
  decisions     45 ████████████░░░░
  learnings     80 ██████████████████████████░
  blockers      10 ███░░░░░░░░░░░░░
  progress      15 ████░░░░░░░░░░░░

By Tier:
  hot          60 ████████████████░░░░
  warm         50 █████████████░░░░░░░
  cold         30 ████████░░░░░░░░░░░░
  archived     10 ███░░░░░░░░░░░░░░░░░

Index: 15.2 MB | Last Sync: 2025-01-20 15:30:00 UTC
Feature Tier: Enhanced (LLM disabled)
```

#### 2.2.4 memory sync

```bash
# Local sync (git notes → index)
memory sync

# Fetch from remote
memory sync --fetch

# Push to remote
memory sync --push

# Full remote sync
memory sync --remote

# Specific domain
memory sync --domain user --remote
```

```rust
#[derive(Args)]
pub struct SyncArgs {
    /// Sync with git remote
    #[arg(long)]
    remote: bool,

    /// Fetch from remote
    #[arg(long, conflicts_with = "remote")]
    fetch: bool,

    /// Push to remote
    #[arg(long, conflicts_with = "remote")]
    push: bool,

    /// Domain to sync
    #[arg(long, default_value = "all")]
    domain: DomainFilter,

    /// Force sync even if no changes detected
    #[arg(long)]
    force: bool,
}
```

#### 2.2.5 memory serve

```bash
# Start MCP server (stdio transport)
memory serve

# With SSE transport
memory serve --transport sse --port 8080

# With WebSocket transport
memory serve --transport websocket --port 8081
```

```rust
#[derive(Args)]
pub struct ServeArgs {
    /// Transport type
    #[arg(long, default_value = "stdio")]
    transport: Transport,

    /// Port for network transports
    #[arg(long, default_value = "8080")]
    port: u16,

    /// Host for network transports
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Enable CORS for browser access
    #[arg(long)]
    cors: bool,
}

#[derive(Clone, ValueEnum)]
pub enum Transport {
    Stdio,
    Sse,
    Websocket,
}
```

### 2.3 CLI Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid arguments |
| 3 | Configuration error |
| 4 | Storage error |
| 5 | Network error (remote sync) |
| 6 | LLM error (Tier 3 features) |
| 10 | Memory not found |
| 11 | Validation error (summary too long, etc.) |
| 12 | Security filter blocked content |

### 2.4 Piping and Scripting

```bash
# Pipe content from file
cat DECISIONS.md | memory capture decisions "Architecture decisions" --stdin

# Extract all decision URIs
memory recall --namespace decisions --format json | jq -r '.results[].uri'

# Batch capture from YAML
cat memories.yaml | memory import --format yaml

# Export to markdown
memory recall "all" --limit 100 --format markdown > export.md

# Integration with other tools
git log --oneline -10 | memory capture progress "Recent commits" --stdin
```

---

## 3. MCP Server Interface

### 3.1 MCP Server Architecture

```rust
use rmcp::{Server, ServerConfig, Tool, Resource};

/// MCP server for memory system
pub struct MemoryMcpServer {
    services: Arc<ServiceContainer>,
    config: McpConfig,
}

impl MemoryMcpServer {
    pub async fn serve_stdio(self) -> Result<()> {
        let server = Server::builder()
            .config(ServerConfig {
                name: "memory".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                capabilities: self.capabilities(),
            })
            .tools(self.tools())
            .resources(self.resources())
            .resource_templates(self.resource_templates())
            .build()?;

        server.serve_stdio().await
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            tools: Some(ToolsCapability::default()),
            resources: Some(ResourcesCapability {
                subscribe: true,
                list_changed: true,
            }),
            prompts: Some(PromptsCapability::default()),
            sampling: None, // We don't need to sample from LLMs
        }
    }
}
```

### 3.2 MCP Tools

All MCP tools are defined in `MCP_RESOURCES_AND_LLM.md` Section 1.5. The server implements:

| Tool | Description | Tier |
|------|-------------|------|
| `memory.capture` | Capture new memories | Core |
| `memory.recall` | Search and retrieve | Core |
| `memory.status` | System statistics | Core |
| `memory.sync` | Synchronization | Core |
| `memory.consolidate` | Run consolidation | LLM |
| `memory.configure` | Runtime config | Core |

### 3.3 MCP Resources

Memory resources use URNs as defined in `MCP_RESOURCES_AND_LLM.md` Section 1.

```rust
impl MemoryMcpServer {
    fn resources(&self) -> Vec<Resource> {
        // Dynamic listing based on actual memories
        vec![]  // Populated via resources/list
    }

    fn resource_templates(&self) -> Vec<ResourceTemplate> {
        vec![
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
            ResourceTemplate {
                uri_template: "subcog://{domain}".to_string(),
                name: "Domain Listing".to_string(),
                description: Some("List namespaces in a domain".to_string()),
                mime_type: Some("application/json".to_string()),
            },
        ]
    }
}
```

### 3.4 MCP Prompts

Pre-defined prompts for common memory operations:

```rust
fn prompts(&self) -> Vec<Prompt> {
    vec![
        Prompt {
            name: "capture-decision".to_string(),
            description: Some("Capture an architecture decision".to_string()),
            arguments: Some(vec![
                PromptArgument {
                    name: "summary".to_string(),
                    description: Some("One-line summary".to_string()),
                    required: true,
                },
                PromptArgument {
                    name: "context".to_string(),
                    description: Some("Decision context and rationale".to_string()),
                    required: true,
                },
            ]),
        },
        Prompt {
            name: "recall-context".to_string(),
            description: Some("Recall relevant context for a topic".to_string()),
            arguments: Some(vec![
                PromptArgument {
                    name: "topic".to_string(),
                    description: Some("Topic to search for".to_string()),
                    required: true,
                },
            ]),
        },
    ]
}
```

### 3.5 Resource Subscriptions

```rust
/// Handle resource subscription for real-time updates
async fn handle_subscribe(
    &self,
    uri: &str,
) -> Result<Subscription> {
    let path = MemoryResourceHandler::parse_uri(uri)?;

    // Create subscription channel
    let (tx, rx) = tokio::sync::mpsc::channel(100);

    // Register for updates
    self.subscriptions.register(path, tx);

    Ok(Subscription { receiver: rx })
}

/// Notify subscribers when memories change
async fn notify_change(&self, memory: &Memory) {
    let uri = MemoryResourceHandler::build_uri(
        memory.domain.clone(),
        memory.namespace,
        &MemoryId(memory.id.clone()),
    );

    self.subscriptions.notify(&uri, ResourceChanged {
        uri: uri.clone(),
        changed_at: Utc::now(),
    }).await;
}
```

---

## 4. Streaming API

### 4.1 Streaming Architecture

For long-running operations (consolidation, bulk sync), the streaming API provides real-time progress:

```rust
/// Streaming API for long-running operations
pub struct StreamingApi {
    services: Arc<ServiceContainer>,
}

impl StreamingApi {
    /// Start SSE endpoint
    pub async fn serve_sse(&self, port: u16) -> Result<()> {
        let app = Router::new()
            .route("/stream/consolidate", post(Self::stream_consolidate))
            .route("/stream/sync", post(Self::stream_sync))
            .route("/stream/recall", post(Self::stream_recall))
            .route("/health", get(Self::health))
            .layer(CorsLayer::permissive());

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}
```

### 4.2 SSE Streaming

```rust
/// Stream consolidation progress via SSE
async fn stream_consolidate(
    State(services): State<Arc<ServiceContainer>>,
    Json(request): Json<ConsolidateRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = async_stream::stream! {
        let consolidation = services.consolidation();

        // Start event
        yield Ok(Event::default()
            .event("start")
            .json_data(&json!({
                "operation": "consolidate",
                "dry_run": request.dry_run,
            })).unwrap());

        // Progress events
        let mut progress_rx = consolidation.subscribe_progress();
        while let Some(progress) = progress_rx.recv().await {
            yield Ok(Event::default()
                .event("progress")
                .json_data(&json!({
                    "phase": progress.phase,
                    "current": progress.current,
                    "total": progress.total,
                    "message": progress.message,
                })).unwrap());
        }

        // Complete event
        let result = consolidation.run(request.dry_run).await;
        yield Ok(Event::default()
            .event("complete")
            .json_data(&json!({
                "success": result.is_ok(),
                "result": result.ok(),
                "error": result.err().map(|e| e.to_string()),
            })).unwrap());
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}
```

### 4.3 WebSocket Streaming

```rust
/// WebSocket handler for bidirectional streaming
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(services): State<Arc<ServiceContainer>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_websocket(socket, services))
}

async fn handle_websocket(
    socket: WebSocket,
    services: Arc<ServiceContainer>,
) {
    let (mut sender, mut receiver) = socket.split();

    // Handle incoming messages
    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            let request: WsRequest = serde_json::from_str(&text).unwrap();

            match request.operation.as_str() {
                "recall" => {
                    // Stream search results as they're found
                    let mut results = services.recall().stream_search(&request.query).await;
                    while let Some(result) = results.next().await {
                        let response = json!({
                            "type": "result",
                            "data": result,
                        });
                        sender.send(Message::Text(response.to_string())).await.ok();
                    }
                }
                "subscribe" => {
                    // Subscribe to memory changes
                    let uri = request.uri.unwrap();
                    let mut changes = services.subscribe(&uri).await;
                    while let Some(change) = changes.next().await {
                        let response = json!({
                            "type": "change",
                            "uri": change.uri,
                            "memory": change.memory,
                        });
                        sender.send(Message::Text(response.to_string())).await.ok();
                    }
                }
                _ => {}
            }
        }
    }
}
```

### 4.4 Streaming Event Types

| Event | Data | When |
|-------|------|------|
| `start` | `{operation, params}` | Operation begins |
| `progress` | `{phase, current, total, message}` | Progress update |
| `result` | `{memory, score}` | Incremental result |
| `change` | `{uri, memory}` | Resource changed |
| `error` | `{code, message}` | Error occurred |
| `complete` | `{success, summary}` | Operation complete |

---

## 5. Hook System

### 5.1 Hook Architecture

Hooks are spawned by Claude Code as separate processes. Each hook receives input on stdin and writes output to stdout as JSON.

```
┌─────────────────────────────────────────────────────────────────────┐
│                      CLAUDE CODE HOOKS                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  SessionStart ──► memory hook session-start                        │
│  └── Context injection, fetch remote, memory recall                 │
│                                                                     │
│  UserPromptSubmit ──► memory hook user-prompt                       │
│  └── Detect [decision], [learned], etc. markers                    │
│                                                                     │
│  PostToolUse ──► memory hook post-tool                              │
│  └── Surface related memories after file operations                 │
│                                                                     │
│  PreCompact ──► memory hook pre-compact                             │
│  └── Auto-capture important content before context loss             │
│                                                                     │
│  Stop ──► memory hook stop                                          │
│  └── Session analysis, sync, push remote                            │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.2 Hook CLI Subcommand

```rust
#[derive(Args)]
pub struct HookArgs {
    /// Hook type to handle
    #[arg(value_enum)]
    hook_type: HookType,

    /// Read input from file instead of stdin
    #[arg(long)]
    input: Option<PathBuf>,

    /// Write output to file instead of stdout
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Clone, ValueEnum)]
pub enum HookType {
    SessionStart,
    UserPrompt,
    PostTool,
    PreCompact,
    Stop,
}
```

### 5.3 Hook Input/Output Contracts

#### 5.3.1 SessionStart Hook

**Input (stdin)**:
```json
{
  "hook": "SessionStart",
  "session_id": "sess_abc123",
  "project_path": "/Users/dev/my-project",
  "project_name": "my-project",
  "git_branch": "feature/auth",
  "timestamp": "2025-01-20T10:00:00Z"
}
```

**Output (stdout)**:
```json
{
  "additionalContext": "<memory_context>\n  <project>my-project</project>\n  <branch>feature/auth</branch>\n  <resources>\n    <resource uri=\"subcog://project:my-project/decisions/abc1234:0\" relevance=\"0.95\">\n      <summary>Use JWT for authentication</summary>\n    </resource>\n  </resources>\n</memory_context>",
  "suppressSystemPrompt": false
}
```

**Implementation**:
```rust
pub async fn handle_session_start(input: SessionStartInput) -> Result<SessionStartOutput> {
    let services = get_services()?;

    // Fetch remote if configured
    if config.fetch_remote_on_start {
        services.sync().fetch_remote().await?;
    }

    // Build context
    let context = services.context_builder()
        .with_project(&input.project_name)
        .with_branch(&input.git_branch)
        .with_recent_memories(10)
        .with_token_budget(2000)
        .build()
        .await?;

    Ok(SessionStartOutput {
        additional_context: context.to_xml(),
        suppress_system_prompt: false,
    })
}
```

#### 5.3.2 UserPromptSubmit Hook

**Input (stdin)**:
```json
{
  "hook": "UserPromptSubmit",
  "session_id": "sess_abc123",
  "prompt": "Let's use PostgreSQL for the database [decision] because of JSONB support"
}
```

**Output (stdout)**:
```json
{
  "additionalContext": "<capture_detected>\n  <signal type=\"decision\">\n    <summary>Use PostgreSQL for database</summary>\n    <reason>JSONB support</reason>\n    <action>Confirm to capture this decision</action>\n  </signal>\n</capture_detected>",
  "suppressSystemPrompt": false
}
```

**Implementation**:
```rust
pub async fn handle_user_prompt(input: UserPromptInput) -> Result<UserPromptOutput> {
    let detector = SignalDetector::new();
    let signals = detector.detect(&input.prompt);

    if signals.is_empty() {
        return Ok(UserPromptOutput::empty());
    }

    // Build capture suggestions
    let mut suggestions = Vec::new();
    for signal in signals {
        suggestions.push(CaptureSuggestion {
            namespace: signal.namespace,
            summary: signal.extract_summary(&input.prompt),
            confidence: signal.confidence,
        });
    }

    Ok(UserPromptOutput {
        additional_context: format_suggestions_xml(&suggestions),
        suppress_system_prompt: false,
    })
}
```

#### 5.3.3 PostToolUse Hook

**Input (stdin)**:
```json
{
  "hook": "PostToolUse",
  "session_id": "sess_abc123",
  "tool_name": "Read",
  "tool_input": {
    "file_path": "/Users/dev/my-project/src/auth.rs"
  },
  "tool_output": "pub fn authenticate(...) { ... }"
}
```

**Output (stdout)**:
```json
{
  "additionalContext": "<related_memories>\n  <memory uri=\"subcog://project:my-project/decisions/auth-jwt:0\" relevance=\"0.88\">\n    <summary>Use JWT tokens for authentication</summary>\n  </memory>\n  <memory uri=\"subcog://user/learnings/rust-auth:0\" relevance=\"0.75\">\n    <summary>Rust authentication patterns</summary>\n  </memory>\n</related_memories>",
  "suppressSystemPrompt": false
}
```

#### 5.3.4 PreCompact Hook

**Input (stdin)**:
```json
{
  "hook": "PreCompact",
  "session_id": "sess_abc123",
  "conversation_summary": "User implemented authentication using JWT...",
  "key_decisions": ["Use JWT", "Store tokens in httpOnly cookies"],
  "remaining_context_tokens": 5000
}
```

**Output (stdout)**:
```json
{
  "additionalContext": "<auto_captured>\n  <memory uri=\"subcog://project:my-project/progress/session-abc:0\">\n    <summary>Authentication implementation progress</summary>\n    <captured>2 decisions auto-saved before context compaction</captured>\n  </memory>\n</auto_captured>",
  "suppressSystemPrompt": false
}
```

#### 5.3.5 Stop Hook

**Input (stdin)**:
```json
{
  "hook": "Stop",
  "session_id": "sess_abc123",
  "reason": "user_initiated",
  "duration_seconds": 1800,
  "tool_calls": 45,
  "tokens_used": 50000
}
```

**Output (stdout)**:
```json
{
  "additionalContext": "",
  "suppressSystemPrompt": true
}
```

**Side Effects**:
- Sync index with git notes
- Push to remote if configured
- Session analysis and potential auto-capture

### 5.4 Hook Configuration

```toml
# memory.toml

[hooks]
enabled = true

[hooks.session_start]
enabled = true
fetch_remote = false
context_token_budget = 2000
include_guidance = true
guidance_detail = "standard"  # minimal, standard, detailed

[hooks.user_prompt]
enabled = true
signal_patterns = ["[decision]", "[learned]", "[blocker]", "[progress]"]

[hooks.post_tool_use]
enabled = true
# Only trigger for these tools
trigger_tools = ["Read", "Edit", "Write"]
# Don't trigger for these file patterns
exclude_patterns = ["*.lock", "node_modules/*"]

[hooks.pre_compact]
enabled = true
# Auto-capture threshold
confidence_threshold = 0.8

[hooks.stop]
enabled = true
push_remote = false
session_analysis = true
```

### 5.5 Hook Performance Requirements

| Hook | Max Latency | Measurement |
|------|-------------|-------------|
| SessionStart | <2000ms | Total execution |
| UserPromptSubmit | <50ms | Signal detection |
| PostToolUse | <100ms | Memory retrieval |
| PreCompact | <500ms | Content analysis |
| Stop | <5000ms | Sync + push |

### 5.6 Hook Error Handling

Hooks must NEVER fail with non-zero exit or malformed JSON. On error, return valid JSON with empty context:

```rust
fn handle_hook_error(error: anyhow::Error) -> HookOutput {
    // Log error to stderr (not captured by Claude Code)
    eprintln!("Hook error: {}", error);

    // Return valid empty output
    HookOutput {
        additional_context: format!("<!-- hook error: {} -->", error),
        suppress_system_prompt: false,
    }
}
```

---

## 6. Interface Integration

### 6.1 Shared Service Container

```rust
/// Container for all services, shared across interfaces
pub struct ServiceContainer {
    capture: Arc<CaptureService>,
    recall: Arc<RecallService>,
    sync: Arc<SyncService>,
    consolidation: Option<Arc<ConsolidationService>>,
    config: Arc<Config>,
    features: FeatureFlags,
}

impl ServiceContainer {
    pub fn new(config: Config) -> Result<Self> {
        let features = FeatureFlags::from_config(&config)?;
        let storage = create_storage(&config)?;
        let embedder = create_embedder(&config)?;

        let capture = Arc::new(CaptureService::new(
            storage.clone(),
            embedder.clone(),
            &features,
        )?);

        let recall = Arc::new(RecallService::new(
            storage.clone(),
            embedder.clone(),
            &features,
        )?);

        let sync = Arc::new(SyncService::new(storage.clone())?);

        let consolidation = if features.consolidation {
            let llm = create_llm_provider(&config)?;
            Some(Arc::new(ConsolidationService::new(storage, llm)?))
        } else {
            None
        };

        Ok(Self {
            capture,
            recall,
            sync,
            consolidation,
            config: Arc::new(config),
            features,
        })
    }
}
```

### 6.2 Unified Error Types

```rust
/// Unified error type across all interfaces
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("Memory not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("Embedding error: {0}")]
    Embedding(#[from] EmbeddingError),

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("LLM error: {0}")]
    Llm(#[from] LlmError),

    #[error("Security: content blocked")]
    ContentBlocked,

    #[error("Feature not enabled: {0}")]
    FeatureNotEnabled(String),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
}

impl MemoryError {
    /// Get exit code for CLI
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NotFound(_) => 10,
            Self::Validation(_) => 11,
            Self::ContentBlocked => 12,
            Self::Storage(_) => 4,
            Self::Git(_) => 5,
            Self::Llm(_) => 6,
            Self::FeatureNotEnabled(_) => 3,
            Self::Config(_) => 3,
            _ => 1,
        }
    }

    /// Get MCP error code
    pub fn mcp_code(&self) -> i32 {
        match self {
            Self::NotFound(_) => -32001,
            Self::Validation(_) => -32002,
            Self::ContentBlocked => -32003,
            _ => -32000,
        }
    }
}
```

### 6.3 Cross-Interface Observability

```rust
/// Trace context propagation across interfaces
pub struct TraceContext {
    trace_id: TraceId,
    span_id: SpanId,
    interface: InterfaceType,
}

#[derive(Debug, Clone, Copy)]
pub enum InterfaceType {
    Cli,
    Mcp,
    Streaming,
    Hook,
}

impl TraceContext {
    /// Create from MCP tool call
    pub fn from_mcp(tool_call_id: &str) -> Self {
        Self {
            trace_id: TraceId::from_hex(tool_call_id).unwrap_or_default(),
            span_id: SpanId::random(),
            interface: InterfaceType::Mcp,
        }
    }

    /// Create from hook session
    pub fn from_hook(session_id: &str) -> Self {
        Self {
            trace_id: TraceId::from_hex(session_id).unwrap_or_default(),
            span_id: SpanId::random(),
            interface: InterfaceType::Hook,
        }
    }
}
```

---

## 7. Configuration Summary

### 7.1 Interface-Specific Settings

```toml
# memory.toml - Complete interface configuration

[cli]
# Default output format
default_format = "text"
# Enable color output
color = true
# Progress indicators
progress = true

[mcp]
# Server name advertised to clients
server_name = "memory"
# Enable resource subscriptions
subscriptions = true
# Prompt templates
prompts_enabled = true

[streaming]
# SSE keep-alive interval
sse_keepalive_seconds = 15
# WebSocket ping interval
ws_ping_seconds = 30
# Maximum concurrent connections
max_connections = 100

[hooks]
# See Section 5.4 for full hook configuration
enabled = true
```

### 7.2 Entry Points

| Interface | Entry Point | Command |
|-----------|-------------|---------|
| CLI | `memory <command>` | Direct invocation |
| MCP (stdio) | `memory serve` | Claude Code config |
| MCP (SSE) | `memory serve --transport sse` | HTTP client |
| Streaming | `memory serve --transport sse` | HTTP SSE client |
| Hooks | `memory hook <type>` | Claude Code hooks.json |

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-28 | Claude Opus 4.5 | Initial specification |
