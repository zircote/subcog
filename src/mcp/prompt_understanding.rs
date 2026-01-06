//! Guidance content for the `prompt_understanding` tool.

pub const PROMPT_UNDERSTANDING: &str = r#"# SUBCOG.MCP-SERVER - How to use Subcog memory tools effectively

## 1. Session Start Protocol

When starting a session, establish context and tool availability:

1) Call `prompt_understanding` to load usage guidance.
2) Call `subcog_status` to confirm memory system health.
3) If this is the first interaction in a project, call:
   - `subcog_recall` with query: "project setup OR architecture OR conventions"

## 2. Core Concepts

Subcog provides persistent memory across sessions. Memories are scoped by domain:
- **project**: repository-scoped memories
- **user**: user-wide memories (shared across projects)
- **org**: organization-wide memories (if enabled/configured)

Use domain-aware defaults: if in a git repo, default scope is project; otherwise user.

## 3. Tool Catalog

### 3.1 Memory Tools

- **subcog_capture**: Save a memory
  - Required: `content`, `namespace`
  - Optional: `tags`, `source`
- **subcog_recall**: Search memories
  - Required: `query`
  - Optional: `filter`, `mode`, `detail`, `limit`
- **subcog_status**: Health and backend status
- **subcog_namespaces**: List available namespaces
- **subcog_consolidate**: Merge/summarize/dedupe memories (LLM-backed)
- **subcog_enrich**: Improve memory structure/tags (LLM-backed)
- **subcog_reindex**: Rebuild search index

### 3.2 Prompt Tools

- **prompt_save**: Save a reusable prompt template (set `merge: true` to preserve existing metadata; content optional)
- **prompt_list**: List saved prompts
- **prompt_get**: Fetch a prompt by name
- **prompt_run**: Render a prompt with variables
- **prompt_delete**: Remove a prompt

Domain resolution order for `prompt_get` and `prompt_run`: project -> user -> org.

## 4. When to Capture

Capture memories when you detect:
- Decisions ("we decided", "going with", "choosing")
- Patterns ("always", "never", "standard", "convention")
- Learnings ("turns out", "gotcha", "realized")
- Fixes ("resolved", "the issue was", "workaround")
- Tech debt ("TODO", "temporary", "needs refactor")

## 5. Recall Strategy

Use `subcog_recall` before:
- Implementing features (search decisions/patterns)
- Debugging (search context/learnings)
- Architecture changes (search decisions)

Recommended defaults:
- `mode`: "hybrid"
- `detail`: "medium"
- `limit`: 5-10

Filter examples:
- `ns:decisions tag:database since:30d`
- `ns:context source:src/* -tag:deprecated`

## 6. Prompt Template Best Practices

When saving prompts:
- Use descriptive names (kebab-case).
- Include variable placeholders like `{{var}}`.
- Provide tags for discoverability.
- Prefer user-domain for cross-project reuse.

## 7. Example Workflows

### 7.1 Capture a Decision
```
subcog_capture:
  content: "Decided to use SQLite for local dev to simplify onboarding."
  namespace: "decisions"
  tags: ["database", "sqlite", "dev"]
  source: "docker-compose.yml"
```

### 7.2 Recall Architecture Conventions
```
subcog_recall:
  query: "architecture conventions"
  filter: "ns:patterns"
  detail: "medium"
  limit: 8
```

### 7.3 Save a Prompt Template
```
prompt_save:
  name: "code-review"
  content: "Review {{file}} for {{focus_area}} issues."
  tags: ["review", "quality"]
```

### 7.4 Update a Prompt While Preserving Metadata
```
prompt_save:
  name: "code-review"
  content: "Review {{file}} for {{focus_area}} issues and edge cases."
  merge: true
```

### 7.5 Update Metadata Only
```
prompt_save:
  name: "code-review"
  description: "Updated description only"
  merge: true
```

## 8. Safety and Integrity

- Keep captures concise and contextual.
- Use `source` to link memories to files or URLs.
- Avoid sensitive data in memory content.
- Use `subcog_reindex` if search results look stale.

## 9. Troubleshooting

- If memories are missing, check `subcog_status`.
- Use broader queries or remove restrictive filters.
- If prompts are missing, verify the domain and storage config.
"#;
