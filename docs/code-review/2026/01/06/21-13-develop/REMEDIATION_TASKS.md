# Remediation Tasks

## Critical (Do Immediately)

- None

## High Priority (This Sprint)

- [x] [src/llm/mod.rs:357] Remove raw LLM responses from parse errors (also `src/services/prompt_enrichment.rs:405`, `src/services/enrichment.rs:212`, `src/llm/lmstudio.rs:211`, `src/llm/ollama.rs:267`)

## Medium Priority (Next 2-3 Sprints)

- [x] [src/services/prompt_enrichment.rs:389] Escape user content before inserting into XML tags
- [x] [src/services/recall.rs:330] Cache branch existence checks within a recall call to avoid repeated git scans
- [ ] [src/services/path_manager.rs:71] Remove implicit fallback to repo path or require explicit opt-in

## Low Priority (Backlog)

- [ ] [src/llm/mod.rs:337] Preserve timeouts in LLM HTTP client fallback path
- [ ] [src/mcp/server.rs:272] Add tests for JWT auth failures and rate-limit exceeded responses
