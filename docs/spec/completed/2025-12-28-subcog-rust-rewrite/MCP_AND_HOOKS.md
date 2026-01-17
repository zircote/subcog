# MCP Server and Hook Integration

This document details the integration architecture between the MCP server (long-lived process) and Claude Code hooks (short-lived processes).

---

## 1. Process Lifecycles

### 1.1 MCP Server (Long-Lived Process)

The MCP server is a persistent process that runs for the duration of a Claude Code or Claude Desktop session.

```
┌─────────────────────────────────────────────────────────────────────┐
│ MCP SERVER LIFECYCLE │
├─────────────────────────────────────────────────────────────────────┤
│ │
│ 1. STARTUP │
│ Claude Code/Desktop starts subcog as MCP server │
│ $ subcog serve --transport stdio │
│ │
│ 2. INITIALIZATION │
│ - Load configuration from config.toml │
│ - Initialize storage connections (SQLite, usearch) │
│ - Load embedding model (fastembed) │
│ - Set up event bus │
│ - Register MCP tools, resources, prompts │
│ │
│ 3. RUNNING │
│ - Listen on stdin for JSON-RPC requests │
│ - Write JSON-RPC responses to stdout │
│ - Maintain state: connections, caches, subscriptions │
│ - Process tool calls from AI agent │
│ │
│ 4. TOOL HANDLING │
│ memory.capture -> CaptureService.capture() │
│ memory.recall -> RecallService.search() │
│ memory.status -> StatusService.stats() │
│ memory.sync -> SyncService.sync() │
│ memory.consolidate -> ConsolidationService.run() │
│ memory.configure -> ConfigService.get/set() │
│ │
│ 5. SHUTDOWN │
│ Server terminates when Claude Code/Desktop exits │
│ - Flush pending operations │
│ - Close storage connections │
│ - Clean up resources │
│ │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 Hooks (Short-Lived Processes)

Hooks are ephemeral processes spawned by Claude Code at specific events.

```
┌─────────────────────────────────────────────────────────────────────┐
│ HOOK LIFECYCLE (Per Invocation) │
├─────────────────────────────────────────────────────────────────────┤
│ │
│ 1. SPAWN │
│ Claude Code triggers hook at specific event │
│ $ subcog hook session-start < input.json > output.json │
│ │
│ 2. INITIALIZATION (must be fast: <10ms) │
│ - Load configuration (cached on disk) │
│ - Open storage connections │
│ - Skip heavy initialization (no model loading) │
│ │
│ 3. EXECUTION │
│ - Read JSON from stdin │
│ - Perform operation (recall, signal detection, etc.) │
│ - Build additionalContext response │
│ │
│ 4. RESPONSE │
│ - Write JSON to stdout │
│ - Exit with code 0 (ALWAYS, even on error) │
│ │
│ 5. CLEANUP │
│ - Close connections │
│ - Process terminates │
│ │
│ IMPORTANT: Each invocation is independent │
│ - No shared memory between invocations │
│ - Must re-initialize services each time │
│ - State persists only via storage layer │
│ │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. Integration Architecture

### 2.1 Claude Code Integration Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ CLAUDE CODE INTEGRATION │
├─────────────────────────────────────────────────────────────────────────────┤
│ │
│ ┌─────────────────┐ ┌───────────────────────────────┐ │
│ │ Claude Code │ │ MCP Configuration │ │
│ │ Process │ │ ~/.config/claude/mcp.json │ │
│ │ │ │ │ │
│ │ ┌───────────┐ │ JSON-RPC/stdio │ { │ │
│ │ │ Claude │ │◄───────────────────► │ "memory": { │ │
│ │ │ LLM │ │ (persistent conn) │ "command": "subcog", │ │
│ │ └───────────┘ │ │ "args": ["serve"] │ │
│ │ │ │ │ } │ │
│ │ │ Tool │ │ } │ │
│ │ │ calls │ └───────────────────────────────┘ │
│ │ ▼ │ │
│ │ ┌───────────┐ │ ┌───────────────────────────────┐ │
│ │ │ MCP │ │ Long-lived proc │ MCP Server Process │ │
│ │ │ Client │──┼─────────────────────►│ (subcog serve) │ │
│ │ └───────────┘ │ │ - Maintains state │ │
│ │ │ │ │ - Handles tool calls │ │
│ │ │ Events │ │ - Event bus active │ │
│ │ ▼ │ └───────────────────────────────┘ │
│ │ ┌───────────┐ │ │
│ │ │ Hook │ │ ┌───────────────────────────────┐ │
│ │ │ Dispatcher│ │ Short-lived procs │ Hooks Configuration │ │
│ │ └───────────┘ │◄─────────────────────│ ~/.claude/hooks.json │ │
│ │ │ │ (spawn per event) │ │ │
│ │ │ │ │ [ │ │
│ │ ▼ │ │ { │ │
│ │ ┌───────────┐ │ │ "event": "SessionStart", │ │
│ │ │ Hook │ │ │ "command": ["subcog", │ │
│ │ │ Process │──┼─────────────────────►│ "hook", "session-start"] │
│ │ │ (subcog │ │ │ }, │ │
│ │ │ hook...) │ │ │ { │ │
│ │ └───────────┘ │ │ "event": "UserPromptSubmit",│
│ │ │ │ "command": ["subcog", │ │
│ │ │ │ "hook", "user-prompt"] │ │
│ │ │ │ }, │ │
│ │ │ │... │ │
│ │ │ │ ] │ │
│ └─────────────────┘ └───────────────────────────────┘ │
│ │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Process Comparison

| Aspect | MCP Server | Hooks |
|--------|-----------|-------|
| **Lifecycle** | Long-lived (session duration) | Short-lived (per-event) |
| **Transport** | stdio (JSON-RPC stream) | stdin/stdout (single JSON) |
| **State** | Maintains connections, caches, subscriptions | Stateless, re-initializes |
| **Invocation** | Started once at session begin | Spawned per hook event |
| **Caller** | Claude Desktop/Code MCP client | Claude Code hook dispatcher |
| **Response** | Streaming (multiple responses) | Single JSON response |
| **Event Bus** | Active (handles internal events) | Not used |
| **Model Loading** | Once at startup | Never (uses storage only) |

---

## 3. Shared Storage Layer

Both MCP server and hooks access the same underlying storage through file-based coordination.

### 3.1 Storage Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ SHARED STORAGE LAYER │
├─────────────────────────────────────────────────────────────────────────────┤
│ │
│ ~/.local/share/subcog/ │
│ ├── index.db <- SQLite (FTS5) │
│ │ - WAL mode for concurrent access │
│ │ - File locking for writes │
│ │ - Shared read access │
│ │ │
│ ├── vectors.usearch <- usearch HNSW index │
│ │ - Memory-mapped for reads │
│ │ - Exclusive lock for writes │
│ │ │
│ ├── config.toml <- Configuration (read-only after load) │
│ │ │
│ └── audit/ <- Audit logs (append-only) │
│ │
│ Project Repository: │
│ └──.git/refs/notes/mem/ │
│ ├── decisions <- Git notes (git handles concurrency) │
│ ├── learnings │
│ ├── blockers │
│ └──... │
│ │
│ User Repository (~/.local/share/subcog/user-memories): │
│ └──.git/refs/notes/mem/ │
│ ├── patterns <- Global user memories │
│ └──... │
│ │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Concurrency Model

```rust
// MCP Server: Keeps connections open
pub struct McpServerStorage {
 sqlite: Arc<Mutex<Connection>>, // Held for session
 usearch: Arc<RwLock<usearch::Index>>, // Read-heavy, occasional writes
 git: Arc<GitOps>, // Thread-safe git operations
}

// Hook: Opens connections briefly
pub struct HookStorage {
 sqlite: Connection, // Opened, used, closed
 usearch: usearch::Index, // Opened read-only
 git: GitOps, // Quick operations
}

// SQLite concurrency via WAL mode
// - Multiple readers allowed
// - Single writer with retry on SQLITE_BUSY
// - Hooks primarily read, MCP server primarily writes
```

### 3.3 Storage Access Patterns

| Operation | MCP Server | Hooks |
|-----------|-----------|-------|
| **Read memories** | Frequent, cached | Frequent, fresh each time |
| **Write memories** | Via capture tool | Rare (Stop hook sync) |
| **Search** | Via recall tool | SessionStart, PostToolUse |
| **Git operations** | Via sync tool | SessionStart fetch, Stop push |
| **Index rebuild** | On demand | Never (too slow) |

---

## 4. Event Bus (MCP Server Only)

The event bus operates **only within** the MCP server process. Hooks do not participate.

### 4.1 Event Bus Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ EVENT BUS (MCP SERVER INTERNAL) │
├─────────────────────────────────────────────────────────────────────────────┤
│ │
│ ┌─────────────────────────────────────────────────────────────────────┐ │
│ │ tokio::broadcast::channel │ │
│ │ │ │
│ │ Publisher (Services) Subscribers (Handlers) │ │
│ │ ┌─────────────────┐ ┌─────────────────────────────┐ │ │
│ │ │ CaptureService │──────────────►│ MetricsHandler │ │ │
│ │ │.capture() │ │ - Update counters │ │ │
│ │ └─────────────────┘ │ - Record latencies │ │ │
│ │ │ └─────────────────────────────┘ │ │
│ │ │ MemoryCaptured │ │
│ │ │ ┌─────────────────────────────┐ │ │
│ │ ├────────────────────────►│ McpSubscriptionHandler │ │ │
│ │ │ │ - Notify MCP clients │ │ │
│ │ │ │ - Resource change events │ │ │
│ │ │ └─────────────────────────────┘ │ │
│ │ │ │ │
│ │ │ ┌─────────────────────────────┐ │ │
│ │ └────────────────────────►│ ConsolidationTrigger │ │ │
│ │ │ - Count captures │ │ │
│ │ │ - Trigger if threshold │ │ │
│ │ └─────────────────────────────┘ │ │
│ │ │ │
│ └─────────────────────────────────────────────────────────────────────┘ │
│ │
│ NOTE: Hooks are separate processes - they don't receive these events │
│ They interact only via the shared storage layer │
│ │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Why Hooks Don't Use Event Bus

1. **Process Isolation**: Hooks are separate OS processes, can't share memory
2. **Lifetime Mismatch**: Hooks exist for milliseconds, events are ongoing
3. **Simplicity**: Hooks just need storage access, not event streaming
4. **Performance**: IPC overhead would exceed hook timing budgets

---

## 5. Hook Specifications

### 5.1 Hook Types and Timing

| Hook | Trigger | Max Latency | Purpose |
|------|---------|-------------|---------|
| **SessionStart** | Session begins | 2000ms | Context injection, remote fetch |
| **UserPromptSubmit** | User sends message | 50ms | Detect capture markers |
| **PostToolUse** | After tool execution | 100ms | Surface related memories |
| **PreCompact** | Before context compaction | 500ms | Auto-capture important content |
| **Stop** | Session ends | 5000ms | Sync, push, session analysis |

### 5.2 Hook Input/Output Contracts

#### SessionStart

```json
// INPUT (stdin)
{
 "hook": "SessionStart",
 "session_id": "sess_abc123",
 "project_path": "/Users/dev/my-project",
 "project_name": "my-project",
 "git_branch": "feature/auth",
 "timestamp": "2025-01-20T10:00:00Z"
}

// OUTPUT (stdout)
{
 "additionalContext": "<memory_context>...</memory_context>",
 "suppressSystemPrompt": false
}
```

#### UserPromptSubmit

```json
// INPUT (stdin)
{
 "hook": "UserPromptSubmit",
 "session_id": "sess_abc123",
 "prompt": "Let's use PostgreSQL [decision] because of JSONB support"
}

// OUTPUT (stdout)
{
 "additionalContext": "<capture_detected>...</capture_detected>",
 "suppressSystemPrompt": false
}
```

#### PostToolUse

```json
// INPUT (stdin)
{
 "hook": "PostToolUse",
 "session_id": "sess_abc123",
 "tool_name": "Read",
 "tool_input": { "file_path": "/src/auth.rs" },
 "tool_output": "pub fn authenticate(...) {... }"
}

// OUTPUT (stdout)
{
 "additionalContext": "<related_memories>...</related_memories>",
 "suppressSystemPrompt": false
}
```

#### PreCompact

```json
// INPUT (stdin)
{
 "hook": "PreCompact",
 "session_id": "sess_abc123",
 "conversation_summary": "User implemented authentication...",
 "key_decisions": ["Use JWT", "Store in httpOnly cookies"],
 "remaining_context_tokens": 5000
}

// OUTPUT (stdout)
{
 "additionalContext": "<auto_captured>...</auto_captured>",
 "suppressSystemPrompt": false
}
```

#### Stop

```json
// INPUT (stdin)
{
 "hook": "Stop",
 "session_id": "sess_abc123",
 "reason": "user_initiated",
 "duration_seconds": 1800,
 "tool_calls": 45,
 "tokens_used": 50000
}

// OUTPUT (stdout)
{
 "additionalContext": "",
 "suppressSystemPrompt": true
}
```

### 5.3 Error Handling

Hooks must NEVER fail with non-zero exit or malformed JSON:

```rust
fn main() {
 let result = run_hook();

 match result {
 Ok(output) => {
 // Success: output valid JSON
 println!("{}", serde_json::to_string(&output).unwrap());
 }
 Err(e) => {
 // Error: still output valid JSON with empty context
 eprintln!("Hook error: {}", e); // Goes to stderr, not captured
 println!("{}", serde_json::to_string(&HookOutput {
 additional_context: format!("<!-- hook error: {} -->", e),
 suppress_system_prompt: false,
 }).unwrap());
 }
 }

 // ALWAYS exit 0
 std::process::exit(0);
}
```

---

## 6. Cross-Process Coordination

### 6.1 How Hooks Affect MCP Server State

Hooks can modify storage that the MCP server reads:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ HOOK -> MCP SERVER DATA FLOW │
├─────────────────────────────────────────────────────────────────────────────┤
│ │
│ SessionStart Hook: │
│ ┌─────────────────────────────────────────────────────────────────────┐ │
│ │ 1. git fetch origin refs/notes/mem/* │ │
│ │ -> Updates local git notes from remote │ │
│ │ │ │
│ │ 2. Rebuild index if stale │ │
│ │ -> Updates SQLite index │ │
│ │ │ │
│ │ RESULT: MCP server's next memory.recall sees new data │ │
│ └─────────────────────────────────────────────────────────────────────┘ │
│ │
│ Stop Hook: │
│ ┌─────────────────────────────────────────────────────────────────────┐ │
│ │ 1. Sync index with git notes │ │
│ │ -> Ensures consistency │ │
│ │ │ │
│ │ 2. git push origin refs/notes/mem/* │ │
│ │ -> Pushes to remote for other machines │ │
│ │ │ │
│ │ RESULT: MCP server typically shutting down anyway │ │
│ └─────────────────────────────────────────────────────────────────────┘ │
│ │
│ PreCompact Hook (with auto-capture): │
│ ┌─────────────────────────────────────────────────────────────────────┐ │
│ │ 1. Analyze conversation for capture-worthy content │ │
│ │ │ │
│ │ 2. Write new memories to git notes + index │ │
│ │ -> Creates new memories │ │
│ │ │ │
│ │ RESULT: MCP server's next query sees new memories │ │
│ └─────────────────────────────────────────────────────────────────────┘ │
│ │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Timing Considerations

```
Session Timeline:
─────────────────────────────────────────────────────────────────────────────►

│ SessionStart │ │ User Prompts │ │ Stop │
│ Hook │ │ Hooks │ │ Hook │
│ (2000ms) │ │ (50ms ea) │ │(5000ms)│
 │ │ │
 ▼ ▼ ▼
┌──────────┐ ┌──────────┐ ┌──────────┐
│ Fetch │ │ Signal │ │ Sync │
│ Remote │ │ Detection│ │ Push │
│ Rebuild │ │ │ │ │
│ Index │ │ │ │ │
└──────────┘ └──────────┘ └──────────┘
 │ │
 │ MCP Server Running (handles tool calls) │
 ├───────────────────────────────────────────────────────────────┤
 │ │
 │ memory.capture ─► writes to storage │
 │ memory.recall ─► reads from storage │
 │ memory.sync ─► syncs storage │
 │ │
```

---

## 7. Configuration

### 7.1 MCP Server Configuration

```json
// ~/.config/claude/mcp.json
{
 "mcpServers": {
 "memory": {
 "command": "subcog",
 "args": ["serve", "--transport", "stdio"],
 "env": {
 "SUBCOG_CONFIG": "~/.config/subcog/config.toml"
 }
 }
 }
}
```

### 7.2 Hooks Configuration

```json
// ~/.claude/hooks.json
[
 {
 "event": "SessionStart",
 "command": ["subcog", "hook", "session-start"],
 "timeout": 2000
 },
 {
 "event": "UserPromptSubmit",
 "command": ["subcog", "hook", "user-prompt"],
 "timeout": 50
 },
 {
 "event": "PostToolUse",
 "command": ["subcog", "hook", "post-tool"],
 "timeout": 100
 },
 {
 "event": "PreCompact",
 "command": ["subcog", "hook", "pre-compact"],
 "timeout": 500
 },
 {
 "event": "Stop",
 "command": ["subcog", "hook", "stop"],
 "timeout": 5000
 }
]
```

### 7.3 Subcog Configuration

```toml
# ~/.config/subcog/config.toml

[hooks]
enabled = true

[hooks.session_start]
enabled = true
fetch_remote = false
context_token_budget = 2000
include_guidance = true

[hooks.user_prompt]
enabled = true
signal_patterns = ["[decision]", "[learned]", "[blocker]", "[progress]"]

[hooks.post_tool_use]
enabled = true
trigger_tools = ["Read", "Edit", "Write"]
exclude_patterns = ["*.lock", "node_modules/*"]

[hooks.pre_compact]
enabled = true
confidence_threshold = 0.8

[hooks.stop]
enabled = true
push_remote = false
session_analysis = true
```

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-28 | Claude Opus 4.5 | Initial MCP and hooks integration specification |
