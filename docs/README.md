# Subcog Documentation

Subcog is a persistent memory system for AI coding assistants. It captures decisions, patterns, learnings, and context from coding sessions and surfaces them when relevant.

## Quick Links

| Document | Description |
|----------|-------------|
| [QUICKSTART.md](QUICKSTART.md) | Get started in 5 minutes |
| [URN-GUIDE.md](URN-GUIDE.md) | URN and URI addressing schemes |
| [QUERY_SYNTAX.md](QUERY_SYNTAX.md) | Filter syntax for searching memories |

## Documentation Sections

### [CLI Commands](cli/README.md)

Complete reference for all command-line interface commands:

- [capture](cli/capture.md) - Capture memories
- [recall](cli/recall.md) - Search and retrieve memories
- [status](cli/status.md) - System status
- [sync](cli/sync.md) - Synchronize with git remote
- [consolidate](cli/consolidate.md) - Merge similar memories
- [config](cli/config.md) - Configuration management
- [serve](cli/serve.md) - Run MCP server
- [hook](cli/hook.md) - Claude Code hook handlers
- [prompt](cli/prompt.md) - Prompt template management
- [namespaces](cli/namespaces.md) - List memory namespaces

### [MCP Integration](mcp/README.md)

Model Context Protocol server documentation:

- [Tools](mcp/tools.md) - 13 available MCP tools
- [Resources](mcp/resources.md) - 26+ URI-based resources
- [Prompts](mcp/prompts.md) - 11 built-in prompt templates
- [Protocol](mcp/protocol.md) - JSON-RPC protocol details

### [Claude Code Hooks](hooks/README.md)

Integration with Claude Code IDE:

- [session-start](hooks/session-start.md) - Context injection on session start
- [user-prompt-submit](hooks/user-prompt-submit.md) - Signal detection and memory surfacing
- [post-tool-use](hooks/post-tool-use.md) - Related memory surfacing
- [pre-compact](hooks/pre-compact.md) - Auto-capture before compaction
- [stop](hooks/stop.md) - Session analysis and sync
- [Search Intent](hooks/search-intent.md) - Intent detection system

### [Configuration](configuration/README.md)

Configuration reference:

- [Config File](configuration/config-file.md) - TOML configuration format
- [Environment Variables](configuration/environment.md) - All environment variables
- [Feature Flags](configuration/features.md) - Optional feature toggles
- [File Locations](configuration/locations.md) - OS-specific paths

### [Storage Architecture](storage/README.md)

Three-layer storage system:

- [Persistence Layer](storage/persistence.md) - SQLite, PostgreSQL, Filesystem
- [Index Layer](storage/index.md) - SQLite, PostgreSQL FTS, RediSearch
- [Vector Layer](storage/vector.md) - usearch, pgvector, Redis Vector
- [Domains](storage/domains.md) - Project, User, Org scoping

### [Prompt Templates](prompts/README.md)

User-defined prompt management:

- [Overview](prompts/overview.md) - What are prompt templates
- [Variables](prompts/variables.md) - Variable substitution syntax
- [Formats](prompts/formats.md) - YAML, JSON, Markdown, Plain text
- [Storage](prompts/storage.md) - Domain-scoped storage
- [MCP Integration](prompts/mcp.md) - Accessing prompts via MCP

### [Architecture](architecture/README.md)

System design and internals:

- [Overview](architecture/overview.md) - High-level architecture
- [Data Models](architecture/models.md) - Core data structures
- [Services](architecture/services.md) - Business logic layer
- [Search](architecture/search.md) - Hybrid search (RRF fusion)
- [Security](architecture/security.md) - Secrets and PII filtering

## Memory Namespaces

Memories are categorized into 14 namespaces:

| Namespace | Purpose | Signal Words |
|-----------|---------|--------------|
| `decisions` | Architectural and design decisions | "decided", "chose", "going with" |
| `patterns` | Discovered patterns and conventions | "always", "never", "convention" |
| `learnings` | Lessons learned from debugging | "TIL", "learned", "discovered" |
| `context` | Important background information | "because", "constraint", "requirement" |
| `tech-debt` | Technical debt tracking | "TODO", "FIXME", "temporary", "hack" |
| `blockers` | Blockers and impediments | "blocked", "waiting", "depends on" |
| `progress` | Work progress and milestones | "completed", "milestone", "shipped" |
| `apis` | API documentation and contracts | "endpoint", "request", "response" |
| `config` | Configuration details | "environment", "setting", "variable" |
| `security` | Security findings and notes | "vulnerability", "CVE", "auth" |
| `testing` | Test strategies and edge cases | "test", "edge case", "coverage" |
| `performance` | Performance insights and benchmarks | "latency", "throughput", "optimization" |
| `help` | Help and usage information | "how to", "usage", "example" |
| `prompts` | Prompt templates (reserved) | — |

## Domain Scopes

Memories and prompts can be scoped to different domains:

| Scope | Description | Use Case |
|-------|-------------|----------|
| `project` | Current repository | Repo-specific decisions |
| `user` | User-wide (global) | Personal learnings |
| `org` | Organization-level | Cross-repo patterns |

## Installation

### From Source

```bash
git clone https://github.com/zircote/subcog.git
cd subcog
cargo build --release
```

### With Claude Code

Add to your Claude Code configuration:

```json
{
  "mcpServers": {
    "subcog": {
      "command": "subcog",
      "args": ["serve"]
    }
  }
}
```

## Version Information

- **Language**: Rust (Edition 2024)
- **MSRV**: 1.85
- **Repository**: [github.com/zircote/subcog](https://github.com/zircote/subcog)

## Specifications

Active and completed feature specifications are in [docs/spec/](spec/).
