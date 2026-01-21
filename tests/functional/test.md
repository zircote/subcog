# Subcog Manual Validation Test Script

This document provides comprehensive manual tests for ALL documented Subcog functionality.
Copy and paste each test into Claude to validate. Mark results in the checkboxes.

---

## Table of Contents

1. [Session Initialization](#1-session-initialization)
2. [Slash Commands](#2-slash-commands)
3. [Memory CRUD Tools](#3-memory-crud-tools)
4. [Search & Recall](#4-search--recall)
5. [Filter Syntax](#5-filter-syntax)
6. [URN Queries](#6-urn-queries)
7. [Knowledge Graph - Entities](#7-knowledge-graph---entities)
8. [Knowledge Graph - Relationships](#8-knowledge-graph---relationships)
9. [Knowledge Graph - Graph Operations](#9-knowledge-graph---graph-operations)
10. [Prompt Management](#10-prompt-management)
11. [Context Templates](#11-context-templates)
12. [Consolidation & Maintenance](#12-consolidation--maintenance)
13. [CLI Commands](#13-cli-commands)
14. [Privacy & Compliance](#14-privacy--compliance)

---

## 1. Session Initialization

### 1.1 subcog_init (Recommended)

```
Call subcog_init with include_recall: true, recall_limit: 5
```

- [ ] Returns guidance content
- [ ] Returns status information
- [ ] Returns recalled memories (if any exist)

### 1.2 subcog_init with Custom Query

```
Call subcog_init with recall_query: "database OR storage", recall_limit: 3
```

- [ ] Uses custom query for recall
- [ ] Respects recall_limit parameter

### 1.3 subcog_status

```
Call subcog_status
```

- [ ] Returns system health status
- [ ] Shows total memory count
- [ ] Shows namespace breakdown
- [ ] Shows storage backend info

### 1.4 prompt_understanding

```
Call prompt_understanding
```

- [ ] Returns full usage guidance document
- [ ] Documents all tools
- [ ] Documents all namespaces

---

## 2. Slash Commands

### 2.1 /subcog:status

```
/subcog:status
```

- [ ] Command recognized
- [ ] Returns system status

### 2.2 /subcog:namespaces

```
/subcog:namespaces
```

- [ ] Command recognized
- [ ] Lists all 10 namespaces: decisions, patterns, learnings, context, tech-debt, apis, config, security, performance, testing

### 2.3 /subcog:capture (Quick)

```
/subcog:capture "Test memory for validation - can be deleted"
```

- [ ] Command recognized
- [ ] Auto-detects namespace or prompts
- [ ] Memory captured successfully
- [ ] Returns memory ID

### 2.4 /subcog:capture (Explicit Namespace)

```
/subcog:capture --namespace testing "Test memory with explicit namespace - can be deleted"
```

- [ ] Command recognized
- [ ] Uses specified namespace
- [ ] Memory captured successfully

### 2.5 /subcog:capture (Interactive)

```
/subcog:capture --interactive
```

- [ ] Command recognized
- [ ] Prompts for namespace selection
- [ ] Prompts for content
- [ ] Prompts for tags

### 2.6 /subcog:recall (Basic)

```
/subcog:recall "test"
```

- [ ] Command recognized
- [ ] Returns search results
- [ ] Shows relevance scores

### 2.7 /subcog:recall (With Namespace)

```
/subcog:recall --namespace testing "test"
```

- [ ] Filters by namespace

### 2.8 /subcog:recall (With Mode)

```
/subcog:recall --mode vector "test"
```

- [ ] Uses specified search mode

### 2.9 /subcog:recall (With Limit)

```
/subcog:recall --limit 3 "test"
```

- [ ] Respects limit parameter

### 2.10 /subcog:prompts (List)

```
/subcog:prompts --list
```

- [ ] Lists available prompt templates

### 2.11 /subcog:sync (Deprecated)

```
/subcog:sync
```

- [ ] Command recognized
- [ ] Returns no-op/deprecated message

### 2.12 /subcog:integrate

```
/subcog:integrate --analyze
```

- [ ] Command recognized
- [ ] Analyzes project for Subcog integration

---

## 3. Memory CRUD Tools

### 3.1 subcog_capture (All Namespaces)

Test each documented namespace:

```
Call subcog_capture with:
  content: "Test decision memory - DELETE ME"
  namespace: decisions
  tags: ["test", "validation"]
```

- [ ] decisions namespace works

```
Call subcog_capture with:
  content: "Test pattern memory - DELETE ME"
  namespace: patterns
  tags: ["test", "validation"]
```

- [ ] patterns namespace works

```
Call subcog_capture with:
  content: "Test learning memory - DELETE ME"
  namespace: learnings
  tags: ["test", "validation"]
```

- [ ] learnings namespace works

```
Call subcog_capture with:
  content: "Test context memory - DELETE ME"
  namespace: context
  tags: ["test", "validation"]
```

- [ ] context namespace works

```
Call subcog_capture with:
  content: "Test tech-debt memory - DELETE ME"
  namespace: tech-debt
  tags: ["test", "validation"]
```

- [ ] tech-debt namespace works

```
Call subcog_capture with:
  content: "Test apis memory - DELETE ME"
  namespace: apis
  tags: ["test", "validation"]
```

- [ ] apis namespace works

```
Call subcog_capture with:
  content: "Test config memory - DELETE ME"
  namespace: config
  tags: ["test", "validation"]
```

- [ ] config namespace works

```
Call subcog_capture with:
  content: "Test security memory - DELETE ME"
  namespace: security
  tags: ["test", "validation"]
```

- [ ] security namespace works

```
Call subcog_capture with:
  content: "Test performance memory - DELETE ME"
  namespace: performance
  tags: ["test", "validation"]
```

- [ ] performance namespace works

```
Call subcog_capture with:
  content: "Test testing memory - DELETE ME"
  namespace: testing
  tags: ["test", "validation"]
```

- [ ] testing namespace works

### 3.2 subcog_capture (With Source)

```
Call subcog_capture with:
  content: "Test memory with source - DELETE ME"
  namespace: testing
  source: "test.md"
  tags: ["test", "with-source"]
```

- [ ] source parameter accepted
- [ ] source stored with memory

### 3.3 subcog_capture (With Domain)

```
Call subcog_capture with:
  content: "Test user-scoped memory - DELETE ME"
  namespace: testing
  domain: user
  tags: ["test", "user-scope"]
```

- [ ] domain: user works

```
Call subcog_capture with:
  content: "Test project-scoped memory - DELETE ME"
  namespace: testing
  domain: project
  tags: ["test", "project-scope"]
```

- [ ] domain: project works

### 3.4 subcog_capture (With TTL)

```
Call subcog_capture with:
  content: "Test memory with TTL - DELETE ME"
  namespace: testing
  ttl: "1d"
  tags: ["test", "ttl"]
```

- [ ] ttl parameter accepted (format: "7d", "24h", "60m", "3600s")

### 3.5 subcog_get

```
Call subcog_get with memory_id: "<INSERT_MEMORY_ID_FROM_CAPTURE>"
```

- [ ] Returns full memory content
- [ ] Returns metadata (namespace, tags, created_at)

### 3.6 subcog_update

```
Call subcog_update with:
  memory_id: "<INSERT_MEMORY_ID>"
  tags: ["test", "updated", "validation"]
```

- [ ] Tags updated successfully

```
Call subcog_update with:
  memory_id: "<INSERT_MEMORY_ID>"
  content: "Updated content - DELETE ME"
```

- [ ] Content updated successfully

### 3.7 subcog_delete (Soft Delete)

```
Call subcog_delete with:
  memory_id: "<INSERT_MEMORY_ID>"
  hard: false
```

- [ ] Memory soft-deleted (tombstoned)
- [ ] Returns success

### 3.8 subcog_restore

```
Call subcog_restore with:
  memory_id: "<INSERT_SOFT_DELETED_ID>"
```

- [ ] Memory restored from tombstone
- [ ] Memory accessible again

### 3.9 subcog_delete (Hard Delete)

```
Call subcog_delete with:
  memory_id: "<INSERT_MEMORY_ID>"
  hard: true
```

- [ ] Memory permanently deleted
- [ ] Cannot be restored

### 3.10 subcog_history

```
Call subcog_history with:
  memory_id: "<INSERT_MEMORY_ID>"
  limit: 10
```

- [ ] Returns audit trail
- [ ] Shows creation event
- [ ] Shows update events (if any)

### 3.11 subcog_delete_all (Dry Run)

```
Call subcog_delete_all with:
  filter: "tag:test tag:validation"
  dry_run: true
```

- [ ] Returns list of memories that WOULD be deleted
- [ ] Does NOT actually delete

### 3.12 subcog_list (via subcog_recall without query)

```
Call subcog_recall without query parameter, with:
  limit: 10
  offset: 0
```

- [ ] Lists all memories (no search)
- [ ] Pagination works

---

## 4. Search & Recall

### 4.1 Search Modes

```
Call subcog_recall with:
  query: "test"
  mode: hybrid
```

- [ ] hybrid mode works (default, combines semantic + keyword)

```
Call subcog_recall with:
  query: "test"
  mode: vector
```

- [ ] vector mode works (pure semantic similarity)

```
Call subcog_recall with:
  query: "test"
  mode: text
```

- [ ] text mode works (BM25 keyword matching)

### 4.2 Detail Levels

```
Call subcog_recall with:
  query: "test"
  detail: light
```

- [ ] light detail returns frontmatter only

```
Call subcog_recall with:
  query: "test"
  detail: medium
```

- [ ] medium detail returns frontmatter + summary (default)

```
Call subcog_recall with:
  query: "test"
  detail: everything
```

- [ ] everything detail returns full content

### 4.3 Pagination

```
Call subcog_recall with:
  query: "test"
  limit: 5
  offset: 0
```

- [ ] Returns first 5 results

```
Call subcog_recall with:
  query: "test"
  limit: 5
  offset: 5
```

- [ ] Returns next 5 results (pagination works)

### 4.4 subcog_namespaces

```
Call subcog_namespaces
```

- [ ] Returns all namespace descriptions:
  - [ ] decisions
  - [ ] patterns
  - [ ] learnings
  - [ ] context
  - [ ] tech-debt
  - [ ] apis
  - [ ] config
  - [ ] security
  - [ ] performance
  - [ ] testing

### 4.5 subcog_reindex

```
Call subcog_reindex
```

- [ ] Rebuilds search index
- [ ] Returns success/stats

---

## 5. Filter Syntax

All filters use GitHub-style syntax in the `filter` parameter.

### 5.1 Namespace Filter (ns:)

```
Call subcog_recall with:
  query: "test"
  filter: "ns:testing"
```

- [ ] Filters to testing namespace only

```
Call subcog_recall with:
  query: "test"
  filter: "ns:decisions ns:patterns"
```

- [ ] Multiple namespaces (OR logic)

### 5.2 Tag Filter (tag: / -tag:)

```
Call subcog_recall with:
  query: "test"
  filter: "tag:validation"
```

- [ ] Includes memories with tag

```
Call subcog_recall with:
  query: "test"
  filter: "-tag:deprecated"
```

- [ ] Excludes memories with tag

```
Call subcog_recall with:
  query: "test"
  filter: "tag:test -tag:deprecated"
```

- [ ] Combined include/exclude

### 5.3 Time Filter (since:)

```
Call subcog_recall with:
  query: "test"
  filter: "since:1d"
```

- [ ] Memories from last 1 day

```
Call subcog_recall with:
  query: "test"
  filter: "since:7d"
```

- [ ] Memories from last 7 days

```
Call subcog_recall with:
  query: "test"
  filter: "since:30d"
```

- [ ] Memories from last 30 days

### 5.4 Source Filter (source:)

```
Call subcog_recall with:
  query: "test"
  filter: "source:src/*"
```

- [ ] Filters by source path glob

```
Call subcog_recall with:
  query: "test"
  filter: "source:*.rs"
```

- [ ] Filters by file extension

### 5.5 Status Filter (status:)

```
Call subcog_recall with filter: "status:active"
```

- [ ] Active memories only

```
Call subcog_recall with filter: "status:deleted"
```

- [ ] Tombstoned memories only

### 5.6 Entity Filter (entity:)

```
Call subcog_recall with:
  query: "test"
  filter: "entity:PostgreSQL"
```

- [ ] Filters by entity name

### 5.7 Combined Filters

```
Call subcog_recall with:
  query: "database"
  filter: "ns:decisions tag:postgresql -tag:deprecated since:30d"
```

- [ ] All filters combined work together

---

## 6. URN Queries

URN format: `subcog://{domain}/{namespace}/{memory_id}`

### 6.1 Full URN (Specific Memory)

```
Call subcog_get with:
  memory_id: "subcog://project/testing/<INSERT_ID>"
```

- [ ] URN resolves to specific memory
- [ ] Returns memory content

### 6.2 URN Filter (Wildcard Domain)

```
Call subcog_recall with:
  filter from URN: subcog://_/testing
```

- [ ] Matches any domain, specific namespace

### 6.3 URN Filter (Wildcard Namespace)

```
Call subcog_recall with:
  filter from URN: subcog://project/_
```

- [ ] Matches specific domain, any namespace

### 6.4 URN Filter (All Wildcards)

```
Call subcog_recall with:
  filter from URN: subcog://_/_
```

- [ ] Matches all memories

### 6.5 URN Domains

Test each documented domain:

- [ ] `subcog://project/...` - Project-scoped
- [ ] `subcog://user/...` - User-scoped
- [ ] `subcog://org/...` - Organization-scoped
- [ ] `subcog://_/...` - Wildcard (any domain)

### 6.6 URN Namespaces

Test URNs with each namespace:

- [ ] `subcog://project/decisions/...`
- [ ] `subcog://project/patterns/...`
- [ ] `subcog://project/learnings/...`
- [ ] `subcog://project/context/...`
- [ ] `subcog://project/tech-debt/...`
- [ ] `subcog://project/apis/...`
- [ ] `subcog://project/config/...`
- [ ] `subcog://project/security/...`
- [ ] `subcog://project/performance/...`
- [ ] `subcog://project/testing/...`

---

## 7. Knowledge Graph - Entities

### 7.1 Entity Types

Documented types: Person, Organization, Technology, Concept, File

### 7.2 subcog_entities - Create

```
Call subcog_entities with:
  action: create
  name: "TestEntity"
  entity_type: Technology
```

- [ ] Entity created
- [ ] Returns entity_id

```
Call subcog_entities with:
  action: create
  name: "TestPerson"
  entity_type: Person
  aliases: ["Test User", "Tester"]
```

- [ ] Person entity with aliases created

### 7.3 subcog_entities - Get

```
Call subcog_entities with:
  action: get
  entity_id: "<INSERT_ENTITY_ID>"
```

- [ ] Returns entity details
- [ ] Shows name, type, aliases

### 7.4 subcog_entities - List

```
Call subcog_entities with:
  action: list
  entity_type: Technology
  limit: 10
```

- [ ] Lists entities of specified type

```
Call subcog_entities with:
  action: list
  limit: 20
```

- [ ] Lists all entities (no type filter)

### 7.5 subcog_entities - Delete

```
Call subcog_entities with:
  action: delete
  entity_id: "<INSERT_ENTITY_ID>"
```

- [ ] Entity deleted

### 7.6 subcog_extract_entities (LLM-powered)

```
Call subcog_extract_entities with:
  content: "Alice from Anthropic uses Rust to build the Claude API integration."
  store: false
  min_confidence: 0.5
```

- [ ] Extracts entities from text
- [ ] Returns: Person (Alice), Organization (Anthropic), Technology (Rust)
- [ ] Does not store when store: false

```
Call subcog_extract_entities with:
  content: "PostgreSQL and Redis are used for storage and caching."
  store: true
  min_confidence: 0.6
```

- [ ] Stores extracted entities when store: true

### 7.7 subcog_entity_merge - Find Duplicates

```
Call subcog_entity_merge with:
  action: find_duplicates
  threshold: 0.7
```

- [ ] Finds potential duplicate entities

```
Call subcog_entity_merge with:
  action: find_duplicates
  entity_id: "<INSERT_ENTITY_ID>"
  threshold: 0.7
```

- [ ] Finds duplicates for specific entity

### 7.8 subcog_entity_merge - Merge

```
Call subcog_entity_merge with:
  action: merge
  entity_ids: ["<ID1>", "<ID2>"]
  canonical_name: "MergedEntity"
```

- [ ] Entities merged into one
- [ ] Relationships transferred

---

## 8. Knowledge Graph - Relationships

### 8.1 Relationship Types

Documented types:
- WorksAt
- Created
- Uses
- Implements
- PartOf
- RelatesTo
- MentionedIn
- Supersedes
- ConflictsWith

### 8.2 subcog_relationships - Create

```
Call subcog_relationships with:
  action: create
  from_entity: "<PERSON_ID>"
  to_entity: "<TECHNOLOGY_ID>"
  relationship_type: Uses
```

- [ ] Relationship created
- [ ] Returns relationship details

### 8.3 subcog_relationships - Get/List

```
Call subcog_relationships with:
  action: list
  entity_id: "<INSERT_ENTITY_ID>"
  direction: both
  limit: 20
```

- [ ] Lists relationships for entity

```
Call subcog_relationships with:
  action: list
  entity_id: "<INSERT_ENTITY_ID>"
  direction: outgoing
```

- [ ] Shows only outgoing relationships

```
Call subcog_relationships with:
  action: list
  entity_id: "<INSERT_ENTITY_ID>"
  direction: incoming
```

- [ ] Shows only incoming relationships

### 8.4 subcog_relationships - Delete

```
Call subcog_relationships with:
  action: delete
  from_entity: "<ID1>"
  to_entity: "<ID2>"
  relationship_type: Uses
```

- [ ] Relationship deleted

### 8.5 subcog_relationship_infer (LLM-powered)

```
Call subcog_relationship_infer with:
  limit: 50
  min_confidence: 0.6
  store: false
```

- [ ] Infers relationships between entities
- [ ] Returns inferred relationships
- [ ] Does not store when store: false

---

## 9. Knowledge Graph - Graph Operations

### 9.1 subcog_graph - Stats

```
Call subcog_graph with:
  operation: stats
```

- [ ] Returns graph statistics
- [ ] Shows entity counts by type
- [ ] Shows relationship counts

### 9.2 subcog_graph - Neighbors

```
Call subcog_graph with:
  operation: neighbors
  entity_id: "<INSERT_ENTITY_ID>"
  depth: 2
```

- [ ] Returns neighboring entities
- [ ] Respects depth parameter

### 9.3 subcog_graph - Path

```
Call subcog_graph with:
  operation: path
  from_entity: "<ID1>"
  to_entity: "<ID2>"
  depth: 3
```

- [ ] Finds path between entities
- [ ] Returns path if exists

### 9.4 subcog_graph - Visualize (Mermaid)

```
Call subcog_graph with:
  operation: visualize
  format: mermaid
  limit: 50
```

- [ ] Returns Mermaid diagram syntax
- [ ] Valid Mermaid graph

### 9.5 subcog_graph - Visualize (DOT)

```
Call subcog_graph with:
  operation: visualize
  format: dot
  limit: 50
```

- [ ] Returns DOT/Graphviz format

### 9.6 subcog_graph - Visualize (ASCII)

```
Call subcog_graph with:
  operation: visualize
  format: ascii
  limit: 50
```

- [ ] Returns ASCII representation

### 9.7 subcog_graph - Visualize (Filtered)

```
Call subcog_graph with:
  operation: visualize
  format: mermaid
  entity_id: "<INSERT_ENTITY_ID>"
  entity_types: ["Person", "Technology"]
  relationship_types: ["Uses", "Created"]
  depth: 2
  limit: 30
```

- [ ] Filters by entity types
- [ ] Filters by relationship types
- [ ] Centers on specific entity

---

## 10. Prompt Management

### 10.1 subcog_prompts - Save

```
Call subcog_prompts with:
  action: save
  name: "test-prompt"
  content: "Review {{file}} focusing on {{focus_area}}"
  description: "Test prompt for validation"
  tags: ["test", "validation"]
  domain: project
```

- [ ] Prompt saved
- [ ] Variables extracted: file, focus_area

```
Call subcog_prompts with:
  action: save
  name: "test-prompt-with-vars"
  content: "Analyze {{input}} for {{purpose}}"
  variables: [
    {"name": "input", "description": "The input to analyze", "required": true},
    {"name": "purpose", "description": "Analysis purpose", "required": false, "default": "quality"}
  ]
  domain: user
```

- [ ] Explicit variable definitions work
- [ ] domain: user works

### 10.2 subcog_prompts - List

```
Call subcog_prompts with:
  action: list
  domain: project
  limit: 20
```

- [ ] Lists prompts in domain

```
Call subcog_prompts with:
  action: list
  tags: ["test"]
  name_pattern: "test-*"
```

- [ ] Filters by tags
- [ ] Filters by name pattern

### 10.3 subcog_prompts - Get

```
Call subcog_prompts with:
  action: get
  name: "test-prompt"
```

- [ ] Returns prompt content
- [ ] Returns variable definitions
- [ ] Returns metadata

### 10.4 subcog_prompts - Run

```
Call subcog_prompts with:
  action: run
  name: "test-prompt"
  variables: {
    "file": "src/main.rs",
    "focus_area": "error handling"
  }
```

- [ ] Substitutes variables
- [ ] Returns rendered prompt

### 10.5 subcog_prompts - Delete

```
Call subcog_prompts with:
  action: delete
  name: "test-prompt"
  domain: project
```

- [ ] Prompt deleted

### 10.6 Variable Syntax

- [ ] `{{variable}}` syntax recognized
- [ ] Reserved prefixes rejected: `subcog_`, `system_`, `__`
- [ ] Variables in code blocks treated as literals (not extracted)

---

## 11. Context Templates

### 11.1 subcog_templates - Save

```
Call subcog_templates with:
  action: save
  name: "test-template"
  content: |
    # {{title}}

    Found {{total_count}} memories:

    {{#each memories}}
    - **{{memory.namespace}}**: {{memory.content}}
    {{/each}}
  description: "Test template for validation"
  output_format: markdown
  tags: ["test"]
  domain: project
```

- [ ] Template saved
- [ ] Versioning works (v1 created)

### 11.2 subcog_templates - List

```
Call subcog_templates with:
  action: list
  domain: project
  limit: 20
```

- [ ] Lists templates

```
Call subcog_templates with:
  action: list
  tags: ["test"]
```

- [ ] Filters by tags

### 11.3 subcog_templates - Get

```
Call subcog_templates with:
  action: get
  name: "test-template"
```

- [ ] Returns template content
- [ ] Returns version info

```
Call subcog_templates with:
  action: get
  name: "test-template"
  version: 1
```

- [ ] Gets specific version

### 11.4 subcog_templates - Render

```
Call subcog_templates with:
  action: render
  name: "test-template"
  query: "test"
  limit: 5
  variables: {
    "title": "Test Results"
  }
```

- [ ] Renders template with memories
- [ ] Substitutes custom variables
- [ ] Populates auto-variables (memories, total_count)

```
Call subcog_templates with:
  action: render
  name: "test-template"
  format: json
  query: "test"
```

- [ ] format: json works

```
Call subcog_templates with:
  action: render
  name: "test-template"
  format: xml
  query: "test"
```

- [ ] format: xml works

### 11.5 subcog_templates - Delete

```
Call subcog_templates with:
  action: delete
  name: "test-template"
  domain: project
```

- [ ] Template deleted

### 11.6 Auto-Variables

Documented auto-variables that should work in templates:
- [ ] `{{memories}}` - List for iteration
- [ ] `{{memory.id}}`
- [ ] `{{memory.content}}`
- [ ] `{{memory.namespace}}`
- [ ] `{{memory.tags}}`
- [ ] `{{memory.score}}`
- [ ] `{{total_count}}`
- [ ] `{{namespace_counts}}`
- [ ] `{{statistics}}`

---

## 12. Consolidation & Maintenance

### 12.1 subcog_consolidate (Dry Run)

```
Call subcog_consolidate with:
  namespaces: ["testing"]
  similarity: 0.7
  min_memories: 3
  dry_run: true
```

- [ ] Shows what would be consolidated
- [ ] Does not make changes

### 12.2 subcog_consolidate (Execute)

```
Call subcog_consolidate with:
  namespaces: ["testing"]
  days: 30
  similarity: 0.8
  min_memories: 3
  dry_run: false
```

- [ ] Creates summary nodes
- [ ] Links sources via edges

### 12.3 subcog_get_summary

```
Call subcog_get_summary with:
  memory_id: "<INSERT_SUMMARY_ID>"
```

- [ ] Returns summary content
- [ ] Returns linked source memories

### 12.4 subcog_enrich (LLM-powered)

```
Call subcog_enrich with:
  memory_id: "<INSERT_MEMORY_ID>"
  enrich_structure: true
  enrich_tags: true
  add_context: true
```

- [ ] Restructures content for clarity
- [ ] Generates/improves tags
- [ ] Adds context and rationale

---

## 13. CLI Commands

These test the CLI binary, not MCP tools.

### 13.1 subcog status

```bash
subcog status
```

- [ ] Shows system status
- [ ] Shows database path
- [ ] Shows memory counts

### 13.2 subcog namespaces

```bash
subcog namespaces
```

- [ ] Lists all namespaces

### 13.3 subcog capture

```bash
subcog capture --namespace testing "CLI test memory - DELETE ME"
```

- [ ] Memory captured via CLI

### 13.4 subcog recall

```bash
subcog recall "test" --mode hybrid --limit 5
```

- [ ] Search works via CLI

### 13.5 subcog import

```bash
# First create a test file
echo '{"content": "Import test", "namespace": "testing", "tags": ["test"]}' > /tmp/test-import.json
subcog import /tmp/test-import.json --dry-run
```

- [ ] Validates import file
- [ ] dry-run shows what would be imported

### 13.6 subcog export

```bash
subcog export /tmp/test-export.json --filter "ns:testing" --limit 10
```

- [ ] Exports memories to JSON

```bash
subcog export /tmp/test-export.yaml --filter "ns:testing"
```

- [ ] YAML export works

```bash
subcog export /tmp/test-export.csv --filter "ns:testing"
```

- [ ] CSV export works

### 13.7 subcog hook (for Claude Code hooks)

```bash
subcog hook session-start
```

- [ ] Session start hook output

```bash
subcog hook user-prompt-submit "test query"
```

- [ ] User prompt submit hook output

---

## 14. Privacy & Compliance

### 14.1 subcog_gdpr_export

```
Call subcog_gdpr_export
```

- [ ] Exports all user data
- [ ] Returns portable JSON format
- [ ] GDPR Article 20 compliant

---

## Cleanup

After testing, clean up test memories:

```
Call subcog_delete_all with:
  filter: "tag:validation tag:test"
  dry_run: false
  hard: true
```

- [ ] Test memories cleaned up

---

## Test Summary

| Category | Tests | Passed | Failed | Not Tested |
|----------|-------|--------|--------|------------|
| Session Init | 4 | | | |
| Slash Commands | 12 | | | |
| Memory CRUD | 12 | | | |
| Search & Recall | 5 | | | |
| Filter Syntax | 7 | | | |
| URN Queries | 6 | | | |
| Entities | 8 | | | |
| Relationships | 5 | | | |
| Graph Ops | 7 | | | |
| Prompts | 6 | | | |
| Templates | 6 | | | |
| Consolidation | 4 | | | |
| CLI | 7 | | | |
| Privacy | 1 | | | |
| **TOTAL** | **90** | | | |

---

## Issues Found

Document any issues discovered during testing:

1.
2.
3.

---

## Notes

- This test script is based on documentation in `src/mcp/prompt_understanding.rs`
- Slash commands defined in `commands/*.md`
- URN format from `src/models/urn.rs`
- All 10 namespaces documented: decisions, patterns, learnings, context, tech-debt, apis, config, security, performance, testing
- All 5 entity types: Person, Organization, Technology, Concept, File
- All 9 relationship types: WorksAt, Created, Uses, Implements, PartOf, RelatesTo, MentionedIn, Supersedes, ConflictsWith
- 3 search modes: hybrid, vector, text
- 3 detail levels: light, medium, everything
- 3 domains: project, user, org (+ wildcard _)
