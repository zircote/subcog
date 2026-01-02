---
document_type: architecture
project_id: SPEC-2026-01-01-002
version: 1.0.0
last_updated: 2026-01-01
status: draft
---

# Prompt Variable Context-Aware Extraction - Architecture

## System Overview

This enhancement modifies the prompt variable extraction pipeline to:

1. **Filter out code blocks** before scanning for `{{variable}}` patterns
2. **Enrich extracted variables** with LLM-generated metadata

```
┌──────────────────────────────────────────────────────────────────────┐
│                         PROMPT SAVE FLOW                             │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Input: Raw prompt content                                           │
│    │                                                                 │
│    ▼                                                                 │
│  ┌──────────────────────┐                                            │
│  │ Code Block Detector  │  ← NEW: Identify ``` regions               │
│  │ (src/models/prompt)  │                                            │
│  └──────────┬───────────┘                                            │
│             │                                                        │
│             ▼                                                        │
│  ┌──────────────────────┐                                            │
│  │ Variable Extractor   │  ← MODIFIED: Skip code block regions       │
│  │ extract_variables()  │                                            │
│  └──────────┬───────────┘                                            │
│             │                                                        │
│             ▼                                                        │
│  ┌──────────────────────┐     ┌─────────────────────┐                │
│  │ Enrichment Service   │ ──▶ │ LLM Provider        │                │
│  │ (src/services)       │ ◀── │ (Anthropic/OpenAI)  │                │
│  └──────────┬───────────┘     └─────────────────────┘                │
│             │                   NEW: Generate descriptions,          │
│             │                        defaults, tags                  │
│             ▼                                                        │
│  ┌──────────────────────┐                                            │
│  │ PromptTemplate       │  ← Enriched with full metadata             │
│  │ (with frontmatter)   │                                            │
│  └──────────┬───────────┘                                            │
│             │                                                        │
│             ▼                                                        │
│  ┌──────────────────────┐                                            │
│  │ Storage Backend      │                                            │
│  │ (Git Notes/SQLite)   │                                            │
│  └──────────────────────┘                                            │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

## Component Design

### Component 1: Code Block Detector

**Purpose**: Identify byte ranges of fenced code blocks in content

**Location**: `src/models/prompt.rs` (new function)

**Interface**:
```rust
/// Represents a fenced code block region
#[derive(Debug, Clone)]
pub struct CodeBlockRegion {
    /// Start byte position (inclusive)
    pub start: usize,
    /// End byte position (exclusive)
    pub end: usize,
    /// Optional language identifier
    pub language: Option<String>,
}

/// Detects fenced code blocks in content.
/// Returns regions sorted by start position.
pub fn detect_code_blocks(content: &str) -> Vec<CodeBlockRegion>
```

**Algorithm**:
```
1. Scan for ``` (triple backticks)
2. For each opening ```:
   a. Capture optional language identifier (alphanumeric after ```)
   b. Find matching closing ```
   c. Record (start, end, language) tuple
3. Handle edge cases:
   - Unclosed blocks (warn, treat rest as code)
   - Nested/escaped backticks (rare, defer)
4. Return sorted list of regions
```

**Regex Pattern**:
```rust
static CODE_BLOCK_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // Match: ``` followed by optional language, then content, then ```
    Regex::new(r"```([a-zA-Z0-9_-]*)\n([\s\S]*?)```")
        .expect("static regex: code block pattern")
});
```

### Component 2: Modified Variable Extractor

**Purpose**: Extract variables while excluding code block regions

**Location**: `src/models/prompt.rs` (modify existing `extract_variables`)

**New Signature**:
```rust
/// Extracts variables from prompt content, skipping fenced code blocks.
pub fn extract_variables(content: &str) -> Vec<ExtractedVariable> {
    let code_blocks = detect_code_blocks(content);
    extract_variables_with_exclusions(content, &code_blocks)
}

/// Internal: Extract variables while excluding specified regions
fn extract_variables_with_exclusions(
    content: &str,
    exclusions: &[CodeBlockRegion],
) -> Vec<ExtractedVariable>
```

**Algorithm**:
```
1. Get code block regions
2. For each regex match of {{variable}}:
   a. Check if match.start() falls within any exclusion region
   b. If inside exclusion: skip
   c. If outside: add to results (with deduplication)
3. Return deduplicated list
```

### Component 3: Prompt Enrichment Service

**Purpose**: Generate rich metadata for prompts using LLM

**Location**: `src/services/prompt_enrichment.rs` (new file)

**Interface**:
```rust
pub struct PromptEnrichmentService<P: LlmProvider> {
    llm: P,
}

/// Request for prompt enrichment
pub struct EnrichmentRequest {
    /// The prompt content
    pub content: String,
    /// Extracted variable names
    pub variables: Vec<String>,
    /// Existing metadata to preserve
    pub existing: Option<PartialMetadata>,
}

/// Result of enrichment
pub struct EnrichmentResult {
    /// Generated prompt description
    pub description: String,
    /// Generated tags
    pub tags: Vec<String>,
    /// Enriched variable definitions
    pub variables: Vec<PromptVariable>,
}

impl<P: LlmProvider> PromptEnrichmentService<P> {
    /// Enriches a prompt with LLM-generated metadata
    pub async fn enrich(&self, request: EnrichmentRequest) -> Result<EnrichmentResult>;
}
```

**LLM Prompt Template**:
```
You are analyzing a prompt template. Generate metadata in JSON format.

<prompt_content>
{content}
</prompt_content>

<detected_variables>
{variables}
</detected_variables>

Generate a JSON response with:
{
  "description": "One-sentence description of what this prompt does",
  "tags": ["tag1", "tag2", "tag3"],
  "variables": [
    {
      "name": "variable_name",
      "description": "What this variable represents",
      "required": true/false,
      "default": "default value or null",
      "validation_hint": "format guidance or null"
    }
  ]
}
```

### Component 4: Modified Prompt Save Flow

**Location**: `src/services/prompt.rs` and `src/cli/prompt.rs`

**CLI Flow**:
```
subcog prompt save --name "my-prompt" --from-file prompt.md
    │
    ▼
parse_prompt_file(path)
    │
    ▼
extract_variables(content)  ← Now context-aware
    │
    ▼
enrichment_service.enrich(content, variables)  ← NEW
    │
    ▼
build_prompt_template(enriched)
    │
    ▼
storage.save(template)
```

**MCP Flow**:
```rust
// In src/mcp/tools.rs - prompt_save tool
let variables = extract_variables(&content);

// Skip enrichment if explicitly disabled or if user provided full variables
let enriched = if args.skip_enrichment.unwrap_or(false) || has_full_variables(&args) {
    EnrichmentResult::from_user_input(&args)
} else {
    enrichment_service.enrich(EnrichmentRequest {
        content: content.clone(),
        variables: variables.iter().map(|v| v.name.clone()).collect(),
        existing: args.variables.clone(),
    }).await?
};
```

## Data Flow

### Variable Extraction with Code Block Exclusion

```
Input: "Review {{file}} for issues.\n```\n{{timestamp}}\n```"
           │
           ▼
detect_code_blocks()
    → [CodeBlockRegion { start: 27, end: 45, language: None }]
           │
           ▼
extract_variables_with_exclusions()
    → Match 1: {{file}} at position 7 → NOT in exclusion → KEEP
    → Match 2: {{timestamp}} at position 31 → IN exclusion (27..45) → SKIP
           │
           ▼
Output: [ExtractedVariable { name: "file", position: 7 }]
```

### Enrichment Data Flow

```
Input: 
  content: "Review {{file}} for {{issue_type}} issues"
  variables: ["file", "issue_type"]
           │
           ▼
LLM Request:
  System: "You are analyzing a prompt template..."
  User: "<prompt_content>Review {{file}}...</prompt_content>"
           │
           ▼
LLM Response (JSON):
  {
    "description": "Code review prompt for specific issue types",
    "tags": ["code-review", "quality", "analysis"],
    "variables": [
      {"name": "file", "description": "Path to file to review", "required": true},
      {"name": "issue_type", "description": "Category of issues", "default": "general"}
    ]
  }
           │
           ▼
Output: EnrichmentResult with full metadata
```

## Integration Points

### With Existing LLM Infrastructure

Uses `LlmProvider` trait from `src/llm/mod.rs`:
- `complete_with_system()` for enrichment calls
- Existing JSON extraction utilities
- Provider selection based on config

### With PromptService

```rust
// src/services/prompt.rs
impl PromptService {
    pub async fn save_with_enrichment(
        &self,
        content: &str,
        name: &str,
        options: SaveOptions,
    ) -> Result<PromptTemplate> {
        let variables = extract_variables(content);
        
        let enriched = if options.skip_enrichment {
            self.basic_metadata(variables)
        } else {
            self.enrichment_service.enrich(...).await?
        };
        
        let template = PromptTemplate::new(name, content)
            .with_description(enriched.description)
            .with_variables(enriched.variables)
            .with_tags(enriched.tags);
        
        self.storage.save(&template)?;
        Ok(template)
    }
}
```

### With CLI

```rust
// src/cli/prompt.rs
#[derive(Parser)]
pub struct SaveArgs {
    #[arg(long)]
    pub no_enrich: bool,
    
    #[arg(long)]
    pub dry_run: bool,
}
```

### With MCP Tools

```rust
// src/mcp/tools.rs - prompt_save schema
{
    "skip_enrichment": {
        "type": "boolean",
        "description": "Skip LLM enrichment (use basic variable detection only)"
    }
}
```

## Error Handling

### Code Block Detection Errors

| Error | Handling |
|-------|----------|
| Unclosed code block | Warn, treat rest as code block |
| Invalid UTF-8 in language | Ignore language, still detect block |

### Enrichment Errors

| Error | Handling |
|-------|----------|
| LLM unavailable | Fall back to basic extraction (name + required=true) |
| Invalid JSON response | Log, retry once, then fall back |
| Timeout (>5s) | Fall back to basic extraction |
| Rate limited | Queue and retry with backoff |

### Fallback Behavior

```rust
async fn enrich_with_fallback(...) -> EnrichmentResult {
    match self.enrichment_service.enrich(request).await {
        Ok(result) => result,
        Err(e) => {
            tracing::warn!("Enrichment failed, using fallback: {}", e);
            EnrichmentResult::basic_from_variables(variables)
        }
    }
}
```

## Testing Strategy

### Unit Tests

1. **Code block detection**:
   - Single code block
   - Multiple code blocks
   - Nested content (code inside code)
   - Empty code blocks
   - Code blocks with language identifiers

2. **Variable extraction with exclusions**:
   - Variables only outside blocks
   - Variables only inside blocks (should extract none)
   - Mixed (inside and outside)
   - Edge cases (variable at boundary)

3. **Enrichment service**:
   - Mock LLM responses
   - JSON parsing
   - Fallback on error

### Integration Tests

1. **End-to-end save flow**:
   - Save prompt with code blocks → verify correct variables
   - Save with enrichment → verify metadata populated
   - Save with `--no-enrich` → verify basic metadata

2. **MCP tool integration**:
   - `prompt_save` with enrichment
   - `prompt_save` with `skip_enrichment: true`

## Performance Considerations

### Code Block Detection

- Single-pass regex: O(n) where n = content length
- Results cached per extraction call
- No persistent caching needed (fast enough)

### LLM Enrichment

- Async call to avoid blocking
- Timeout: 5 seconds max
- Token budget: ~500 input + ~300 output
- Cost: ~$0.001 per enrichment (Claude Haiku)

### Caching Strategy

- No caching for code block detection (fast)
- No caching for enrichment (content-dependent)
- Consider caching for repeated saves of same content (future)
