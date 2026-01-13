//! Guidance content for the `prompt_understanding` tool.

pub const PROMPT_UNDERSTANDING: &str = r#"# SUBCOG.MCP-SERVER - How to use Subcog memory tools effectively

## 1. Session Start Protocol

When starting a session, establish context and tool availability:

1) Call `prompt_understanding` to load usage guidance (you're reading it now).
2) Call `subcog_status` to confirm memory system health and get statistics.
3) For first project interaction:
   - `subcog_recall` with query: "project setup OR architecture OR conventions"
   - Check for existing patterns, decisions, and context.

## 2. Core Concepts

### 2.1 Memory System

Subcog provides **persistent memory** across coding sessions with:
- **Semantic search** (vector + BM25 hybrid ranking)
- **Multi-domain scoping** (project, user, org)
- **Knowledge graph** for entity-centric retrieval
- **Automatic deduplication** to prevent duplicates
- **Memory consolidation** to summarize related memories

### 2.2 Domain Scoping

Memories are scoped by domain:
- **project**: Repository-scoped (default when in git repo)
- **user**: User-wide memories (shared across all projects)
- **org**: Organization-wide (if enabled/configured)

Domain resolution for lookups: project → user → org

### 2.3 Namespaces

Memories are categorized by namespace:
| Namespace | Purpose |
|-----------|---------|
| `decisions` | Architecture and design choices |
| `patterns` | Coding conventions and standards |
| `learnings` | Insights and discoveries |
| `context` | Project background and state |
| `tech-debt` | Known issues and TODOs |
| `apis` | API documentation and contracts |
| `config` | Configuration details |
| `security` | Security policies and findings |
| `performance` | Performance observations |
| `testing` | Test strategies and edge cases |

## 3. Tool Catalog

### 3.1 Memory CRUD Tools

| Tool | Description |
|------|-------------|
| `subcog_capture` | Create a memory (required: content, namespace) |
| `subcog_get` | Retrieve a memory by ID |
| `subcog_list` | List memories with filtering and pagination |
| `subcog_update` | Update memory content and/or tags |
| `subcog_delete` | Delete a memory (soft by default, hard optional) |
| `subcog_delete_all` | Bulk delete with filter (dry-run by default) |
| `subcog_restore` | Restore a soft-deleted memory |
| `subcog_history` | View audit trail for a memory |

### 3.2 Search & Recall Tools

| Tool | Description |
|------|-------------|
| `subcog_recall` | Semantic + text search (primary search tool) |
| `subcog_status` | System health and statistics |
| `subcog_namespaces` | List namespace descriptions |
| `subcog_reindex` | Rebuild search index |

**Filter syntax** (GitHub-style):
```
ns:decisions tag:rust -tag:deprecated since:7d source:src/*
```

**Search modes**: `hybrid` (default), `vector`, `text`
**Detail levels**: `light`, `medium` (default), `everything`

### 3.3 Knowledge Graph Tools

| Tool | Description |
|------|-------------|
| `subcog_entities` | CRUD for entities (Person, Org, Tech, Concept, File) |
| `subcog_relationships` | CRUD for entity relationships |
| `subcog_graph_query` | Traverse graph (neighbors, paths, stats) |
| `subcog_extract_entities` | LLM entity extraction from text |
| `subcog_entity_merge` | Deduplicate similar entities |
| `subcog_relationship_infer` | Infer relationships via LLM |
| `subcog_graph_visualize` | Generate Mermaid/DOT/ASCII diagrams |

**Entity types**: Person, Organization, Technology, Concept, File
**Relationship types**: WorksAt, Created, Uses, Implements, PartOf, RelatesTo, MentionedIn, Supersedes, ConflictsWith

### 3.4 Consolidation & Maintenance Tools

| Tool | Description |
|------|-------------|
| `subcog_consolidate` | Group and summarize related memories (LLM-powered) |
| `subcog_get_summary` | Retrieve a summary with its source memories |
| `subcog_enrich` | Improve memory structure/tags via LLM |
| `subcog_sync` | Sync with git remote (push/fetch/full) |

### 3.5 Prompt Management Tools

| Tool | Description |
|------|-------------|
| `prompt_save` | Save a reusable prompt template |
| `prompt_list` | List saved prompts with filtering |
| `prompt_get` | Fetch a prompt by name |
| `prompt_run` | Execute prompt with variable substitution |
| `prompt_delete` | Remove a prompt template |

**Variable syntax**: `{{variable_name}}`
**Domain resolution**: project → user → org

### 3.6 Privacy & Compliance Tools

| Tool | Description |
|------|-------------|
| `subcog_gdpr_export` | Export all user data (GDPR Article 20) |

### 3.7 Context Template Tools

Context templates format memories and statistics for hooks and tool responses.

| Tool | Description |
|------|-------------|
| `context_template_save` | Save a context template with variables and iteration |
| `context_template_list` | List templates with filtering by domain/tags |
| `context_template_get` | Fetch template by name (optionally specific version) |
| `context_template_render` | Render template with memories and custom variables |
| `context_template_delete` | Delete a template (specific version or all) |

**Template syntax**:
- **Variables**: `{{variable_name}}` - substituted at render time
- **Iteration**: `{{#each memories}}...{{memory.field}}...{{/each}}`
- **Output formats**: markdown (default), json, xml

**Auto-variables** (populated automatically):
- `{{memories}}` - List of memories for iteration
- `{{memory.id}}`, `{{memory.content}}`, `{{memory.namespace}}`, `{{memory.tags}}`, `{{memory.score}}`
- `{{total_count}}`, `{{namespace_counts}}`, `{{statistics}}`

**Versioning**: Templates auto-increment version on save (v1, v2, v3...).

## 4. Key Features

### 4.1 Search Intent Detection

Subcog automatically detects search intent and surfaces relevant memories:

| Intent | Triggers | Prioritized Namespaces |
|--------|----------|----------------------|
| HowTo | "how do I...", "implement..." | patterns, learnings |
| Troubleshoot | "error", "fix", "not working" | blockers, learnings, tech-debt |
| Location | "where is...", "find..." | decisions, context |
| Explanation | "what is...", "explain..." | decisions, context |
| Comparison | "vs", "difference between" | decisions, patterns |

### 4.2 Deduplication

The pre-compact hook automatically prevents duplicate captures using:
1. **Exact match** - SHA256 hash lookup (<5ms)
2. **Semantic similarity** - Embedding comparison (<50ms)
3. **Recent capture** - LRU cache with 5-minute TTL (<1ms)

Per-namespace thresholds: decisions (92%), patterns (90%), learnings (88%)

### 4.3 Entity Extraction

When enabled (`auto_extract_entities: true`), memories are analyzed for:
- **People** (Alice, Bob Smith)
- **Organizations** (Anthropic, Rust Foundation)
- **Technologies** (Rust, PostgreSQL, React)
- **Concepts** (Microservices, CQRS)
- **Files** (src/main.rs, README.md)

Use `subcog_extract_entities` for manual extraction.

### 4.4 Memory Consolidation

The consolidation service groups related memories and creates summary nodes:
1. **Semantic clustering** by namespace (configurable threshold: 0.7)
2. **LLM summarization** preserving key details
3. **Edge relationships** linking summaries to source memories

Use `subcog_consolidate --dry-run` to preview before applying.

### 4.5 Prompt Enrichment

When saving prompts, LLM can auto-generate metadata:
- **Description** from content analysis
- **Tags** inferred from context
- **Variable definitions** with descriptions and defaults

Use `skip_enrichment: true` to disable.

## 5. When to Capture

Capture memories when you detect:
- **Decisions**: "we decided", "going with", "choosing", "agreed on"
- **Patterns**: "always", "never", "standard", "convention", "rule"
- **Learnings**: "turns out", "gotcha", "realized", "discovered"
- **Fixes**: "resolved", "the issue was", "workaround", "solution"
- **Tech debt**: "TODO", "temporary", "needs refactor", "hack"
- **Context**: project background, architecture overview, team agreements

**Quality guidelines**:
- Include the "why" (rationale), not just the "what"
- Add relevant file paths via `source`
- Use descriptive, searchable tags
- Keep content concise (1-3 paragraphs)

## 6. Recall Strategy

Use `subcog_recall` before:
- **Implementing features**: search decisions + patterns
- **Debugging**: search context + learnings + tech-debt
- **Architecture changes**: search decisions + patterns
- **Onboarding**: search context + decisions

**Recommended defaults**:
- `mode`: "hybrid"
- `detail`: "medium"
- `limit`: 5-10

**Advanced filtering**:
```
ns:decisions tag:database since:30d        # Recent DB decisions
ns:patterns source:src/api/*              # API patterns
ns:learnings -tag:deprecated              # Active learnings
tag:security tag:auth                     # Security + auth
```

## 7. Knowledge Graph Best Practices

### 7.1 When to Use Graph Tools

- **Entity tracking**: Track people, projects, technologies across memories
- **Relationship mapping**: Understand connections (who works on what)
- **Graph-augmented search**: Find memories by entity, not just text
- **Visualization**: Generate architecture diagrams from knowledge

### 7.2 Entity Management

```
# List all technology entities
subcog_entities: action=list, entity_type=Technology

# Create an entity manually
subcog_entities: action=create, name="PostgreSQL", entity_type=Technology

# Find duplicates before merging
subcog_entity_merge: action=find_duplicates, entity_id="..."

# Merge duplicate entities
subcog_entity_merge: action=merge, entity_ids=["id1", "id2"], canonical_name="PostgreSQL"
```

### 7.3 Graph Traversal

```
# Get neighbors of an entity
subcog_graph_query: operation=neighbors, entity_id="...", depth=2

# Find path between entities
subcog_graph_query: operation=path, from_entity="...", to_entity="..."

# Get graph statistics
subcog_graph_query: operation=stats
```

### 7.4 Visualization

```
# Generate Mermaid diagram centered on entity
subcog_graph_visualize: format=mermaid, entity_id="...", depth=2

# Full graph as DOT format
subcog_graph_visualize: format=dot, limit=50
```

## 8. Prompt Template Best Practices

### 8.1 Creating Templates

- Use **kebab-case** names (code-review, bug-triage)
- Include `{{variable}}` placeholders for dynamic content
- Provide descriptive tags for searchability
- Use **user** domain for cross-project reuse

### 8.2 Variable Syntax

Valid: `{{file}}`, `{{issue_type}}`, `{{focus_area}}`
Reserved prefixes: `subcog_`, `system_`, `__`

Variables inside fenced code blocks are treated as literal examples:
```markdown
Use {{active_var}} here.

\`\`\`python
template = "{{code_example}}"  # This is NOT extracted
\`\`\`
```

### 8.3 Example Templates

```yaml
# code-review template
prompt_save:
  name: code-review
  content: |
    Review {{file}} focusing on:
    - {{focus_area}} issues
    - Best practices
    - Edge cases
  tags: [review, quality]
  domain: user
```

## 9. Example Workflows

### 9.1 Capture a Decision

```yaml
subcog_capture:
  content: "Decided to use SQLite for local dev to simplify onboarding. Production uses PostgreSQL."
  namespace: decisions
  tags: [database, sqlite, postgresql, development]
  source: docker-compose.yml
```

### 9.2 Search with Intent

```yaml
subcog_recall:
  query: "how do we handle authentication errors"
  filter: "ns:patterns ns:learnings"
  detail: medium
  limit: 10
```

### 9.3 Get Memory by ID

```yaml
subcog_get:
  memory_id: "abc123def456"
```

### 9.4 Update Memory Tags

```yaml
subcog_update:
  memory_id: "abc123def456"
  tags: [database, postgresql, production, updated]
```

### 9.5 Consolidate Decisions

```yaml
subcog_consolidate:
  namespaces: [decisions]
  days: 30
  similarity: 0.8
  dry_run: true
```

### 9.6 Extract Entities from Text

```yaml
subcog_extract_entities:
  content: "Alice from Anthropic uses Rust to build the Claude API integration."
  store: true
  min_confidence: 0.6
```

### 9.7 Visualize Entity Relationships

```yaml
subcog_graph_visualize:
  format: mermaid
  entity_types: [Person, Technology]
  depth: 2
```

### 9.8 Infer Relationships Between Entities

```yaml
subcog_relationship_infer:
  entity_ids: ["entity_alice", "entity_postgres"]
  store: true
  min_confidence: 0.7
```

### 9.9 Export User Data (GDPR)

```yaml
subcog_gdpr_export: {}
```

### 9.10 Create a Context Template

```yaml
context_template_save:
  name: search-results
  content: |
    # {{title}}

    Found {{total_count}} memories:

    {{#each memories}}
    - **{{memory.namespace}}**: {{memory.content}}
      _Score: {{memory.score}}_
    {{/each}}
  description: Format search results for display
  tags: [search, formatting]
  domain: user
```

### 9.11 Render a Context Template with Query

```yaml
context_template_render:
  name: search-results
  query: "authentication patterns"
  limit: 10
  format: markdown
  variables:
    title: "Authentication Patterns"
```

### 9.12 List Context Templates

```yaml
context_template_list:
  domain: user
  tags: [formatting]
  limit: 20
```

## 10. Safety and Integrity

- Keep captures **concise and contextual**
- Use `source` to link memories to files or URLs
- **Avoid sensitive data** in memory content (secrets auto-filtered)
- Use `subcog_reindex` if search results seem stale
- Use **soft delete** (default) to allow recovery
- Run `subcog_consolidate --dry-run` before actual consolidation

## 11. Troubleshooting

| Issue | Solution |
|-------|----------|
| Memories not found | Check `subcog_status`, try broader query, remove restrictive filters |
| Prompts missing | Verify domain scope, check storage config |
| Search slow | Use `subcog_reindex` to rebuild index |
| Duplicates appearing | Deduplication may be disabled; check config |
| Graph empty | Enable `auto_extract_entities` or use `subcog_extract_entities` |
| Consolidation fails | Check LLM provider config; falls back to edge-only mode |
| Stale index | Run `subcog_reindex` after direct DB changes |

## 12. Configuration Reference

Key environment variables:
- `SUBCOG_SEARCH_INTENT_ENABLED` - Enable intent detection
- `SUBCOG_DEDUP_ENABLED` - Enable deduplication
- `SUBCOG_AUTO_EXTRACT_ENTITIES` - Enable entity extraction
- `SUBCOG_CONSOLIDATION_ENABLED` - Enable consolidation
- `SUBCOG_LLM_PROVIDER` - LLM provider (anthropic, openai, ollama, lmstudio)

See `~/.config/subcog/config.toml` for full configuration options.
"#;
