# Data Models

Core data structures used throughout Subcog.

> **Note**: Code examples show the target design from the specification.
> Actual implementation may differ in field types or structure.

## Memory

The fundamental data unit.

```rust
pub struct Memory {
    pub id: MemoryId,
    pub namespace: Namespace,
    pub domain: Domain,
    pub content: String,
    pub tags: Vec<String>,
    pub source: Option<String>,
    pub status: MemoryStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### MemoryId

Unique identifier for memories.

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MemoryId(String);

impl MemoryId {
    pub fn new() -> Self {
        // Git SHA1-style hash or UUID
        Self(generate_id())
    }

    pub fn from_string(s: String) -> Self {
        Self(s)
    }
}
```

### Namespace

Memory categorization.

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Namespace {
    Decisions,
    Patterns,
    Learnings,
    Context,
    TechDebt,
    Blockers,
    Progress,
    Apis,
    Config,
    Security,
    Testing,
    Performance,
    Help,
    Prompts,  // Reserved for prompt templates
}

impl Namespace {
    pub fn signal_words(&self) -> &[&str] {
        match self {
            Namespace::Decisions => &["decided", "chose", "going with"],
            Namespace::Patterns => &["always", "never", "convention"],
            Namespace::Learnings => &["TIL", "learned", "discovered"],
            // ...
        }
    }
}
```

### Domain

Scope for memories.

**Target Design (Spec):**
```rust
pub enum Domain {
    Project(String),  // "zircote/subcog"
    User,             // Global user scope
    Org(String),      // "zircote"
}
```

**Current Implementation:**
```rust
pub struct Domain {
    pub organization: Option<String>,
    pub project: Option<String>,
    pub repository: Option<String>,
}

impl Domain {
    pub fn from_git_remote(url: &str) -> Self {
        // Parse "origin" remote to extract org/repo
    }
}
```

### MemoryStatus

Lifecycle status.

```rust
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum MemoryStatus {
    #[default]
    Active,
    Archived,
    Superseded,
    Pending,
}
```

## Search Models

### SearchQuery

Search request parameters.

```rust
pub struct SearchQuery {
    pub text: String,
    pub namespace: Option<Namespace>,
    pub tags_include: Vec<String>,
    pub tags_exclude: Vec<String>,
    pub since: Option<Duration>,
    pub source_pattern: Option<String>,
    pub mode: SearchMode,
    pub limit: usize,
    pub offset: usize,
}
```

### SearchMode

Search algorithm selection.

```rust
#[derive(Clone, Debug, Default)]
pub enum SearchMode {
    #[default]
    Hybrid,   // RRF fusion of vector + text
    Vector,   // Semantic similarity only
    Text,     // BM25 keyword only
}
```

### SearchResult

Search response.

```rust
pub struct SearchResult {
    pub id: MemoryId,
    pub score: f32,
    pub namespace: Namespace,
    pub content: Option<String>,  // Based on detail level
    pub tags: Vec<String>,
    pub source: Option<String>,
}
```

### DetailLevel

Content disclosure level.

```rust
#[derive(Clone, Debug, Default)]
pub enum DetailLevel {
    Light,      // Frontmatter only
    #[default]
    Medium,     // Truncated content
    Everything, // Full content
}
```

## Capture Models

### CaptureRequest

Capture input.

```rust
pub struct CaptureRequest {
    pub content: String,
    pub namespace: Namespace,
    pub tags: Vec<String>,
    pub source: Option<String>,
    pub domain: Option<Domain>,
}
```

### CaptureResult

Capture output.

```rust
pub struct CaptureResult {
    pub id: MemoryId,
    pub urn: String,
}
```

## Prompt Models

### PromptTemplate

User-defined prompt template.

```rust
pub struct PromptTemplate {
    pub name: String,
    pub description: Option<String>,
    pub content: String,
    pub variables: Vec<PromptVariable>,
    pub tags: Vec<String>,
    pub domain: PromptDomain,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### PromptVariable

Variable definition.

```rust
pub struct PromptVariable {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
    pub default: Option<String>,
}
```

### PromptDomain

Prompt scoping.

```rust
#[derive(Clone, Debug, Default)]
pub enum PromptDomain {
    #[default]
    Project,
    User,
    Org,
}
```

## Search Intent Models

### SearchIntentType

Detected user intent.

```rust
#[derive(Clone, Debug)]
pub enum SearchIntentType {
    HowTo,
    Location,
    Explanation,
    Comparison,
    Troubleshoot,
    General,
}
```

### SearchIntent

Detection result.

```rust
pub struct SearchIntent {
    pub intent_type: SearchIntentType,
    pub confidence: f32,
    pub keywords: Vec<String>,
    pub namespace_weights: HashMap<Namespace, f32>,
}
```

## Event Models

### MemoryEvent

Events for observability.

```rust
#[derive(Clone, Debug)]
pub enum MemoryEvent {
    Captured { id: MemoryId, namespace: Namespace },
    Retrieved { id: MemoryId },
    Searched { query: String, results: usize },
    Deleted { id: MemoryId },
    Synced { pushed: usize, fetched: usize },
}
```

## Consolidation Models

### ConsolidationStrategy

Memory consolidation approach.

```rust
#[derive(Clone, Debug, Default)]
pub enum ConsolidationStrategy {
    #[default]
    Merge,      // Combine similar
    Summarize,  // Create summary
    Dedupe,     // Remove duplicates
}
```

### ConsolidationCandidate

Proposed merge.

```rust
pub struct ConsolidationCandidate {
    pub memories: Vec<MemoryId>,
    pub similarity: f32,
    pub proposed_content: Option<String>,
}
```

## Serialization

All models use serde for serialization:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Memory {
    // ...
}
```

### YAML Format (Git Notes)

```yaml
---
id: dc58d23a35876f5a59426e81aaa81d796efa7fc1
namespace: decisions
domain: zircote/subcog
tags: [database, postgresql]
source: ARCHITECTURE.md
status: active
created_at: 2024-01-15T10:30:00Z
updated_at: 2024-01-15T10:30:00Z
---
Use PostgreSQL for primary storage because of JSONB support.
```

### JSON Format (MCP)

```json
{
  "id": "dc58d23a35876f5a59426e81aaa81d796efa7fc1",
  "namespace": "decisions",
  "domain": "zircote/subcog",
  "content": "Use PostgreSQL...",
  "tags": ["database", "postgresql"],
  "source": "ARCHITECTURE.md",
  "status": "active",
  "created_at": 1705314600,
  "updated_at": 1705314600
}
```

## See Also

- [Services](services.md) - How models are used
- [Storage](../storage/README.md) - How models are stored
- [MCP Tools](../mcp/tools.md) - Model in MCP context
