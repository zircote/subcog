# Code Review Report

## Metadata
- **Project**: subcog
- **Review Date**: 2026-01-06
- **Reviewer**: Codex Deep Clean
- **Scope**: Full repo (focused on `src/` plus key config/docs)
- **Commit**: (not captured)
- **LSP Available**: Env reported `ENABLE_LSP_TOOL=1`, but LSP tools unavailable; used `rg`/manual reads
- **Methodology**: Targeted file reads + static inspection; no speculative findings

## Executive Summary

### Overall Health Score: 7.6/10

| Dimension | Score | Critical | High | Medium | Low |
|-----------|-------|----------|------|--------|-----|
| Security | 7/10 | 0 | 1 | 1 | 0 |
| Performance | 7/10 | 0 | 0 | 1 | 0 |
| Architecture | 7/10 | 0 | 0 | 1 | 0 |
| Code Quality | 8/10 | 0 | 0 | 0 | 1 |
| Test Coverage | 7/10 | 0 | 0 | 0 | 1 |
| Documentation | 9/10 | 0 | 0 | 0 | 0 |

### Key Findings
1. LLM parse errors embed full responses in error messages, which can leak user content into logs/CLI output.
2. Prompt enrichment inserts unescaped user content inside XML tags, enabling tag injection.
3. Recall path performs per-hit branch lookups that re-scan git branches, creating avoidable overhead on hot paths.

### Recommended Action Plan
1. **Immediate**: Redact/trim LLM responses in parse errors before logging or surfacing.
2. **This Sprint**: Escape prompt enrichment XML input and cache branch existence checks in recall.
3. **Next Sprint**: Align storage path fallback with user-scope-only architecture; add auth/rate-limit tests.
4. **Backlog**: Harden LLM HTTP client fallback behavior to keep timeouts even on builder failure.

---

## High Priority Findings (🟠)

### [HIGH] [SECURITY] LLM parse errors leak raw response content into logs/CLI

**Location**:
- `src/services/prompt_enrichment.rs:405`
- `src/services/enrichment.rs:212`
- `src/llm/mod.rs:357`
- `src/llm/lmstudio.rs:211`
- `src/llm/ollama.rs:267`

**Description**:
LLM parse failures include the full `response` string in the error message. These errors are logged or printed (e.g., prompt enrichment fallback warns), so user content (potentially containing secrets/PII) can leak into logs or CLI output.

**Impact**:
Sensitive data can appear in logs/terminal output during parse failures, undermining the secret/PII redaction pipeline and audit guarantees.

**Evidence**:
`src/services/prompt_enrichment.rs`:
```rust
serde_json::from_str(json_str).map_err(|e| Error::OperationFailed {
    operation: "parse_enrichment_response".to_string(),
    cause: format!("Failed to parse LLM response: {e}. Response was: {response}"),
})?
```

**Remediation**:
- Strip or truncate response text in error messages.
- Optionally include only `response.len()` and a short prefix.
- Use existing redaction utilities (e.g., `ContentRedactor`) before logging.

---

## Medium Priority Findings (🟡)

### [MEDIUM] [SECURITY] Prompt enrichment inserts unescaped content into XML tags

**Location**: `src/services/prompt_enrichment.rs:389`

**Description**:
`build_user_message` wraps user content inside `<prompt_content>` tags without escaping. If prompt content includes `</prompt_content>` or other XML-like sequences, it can break the intended structure and alter LLM instructions.

**Impact**:
Allows prompt-injection via tag termination and instruction smuggling, reducing safety guarantees of the system prompt structure.

**Evidence**:
```rust
format!(
    "<prompt_content>\n{}\n</prompt_content>\n\n<detected_variables>\n{}\n</detected_variables>",
    request.content, variables_str
)
```

**Remediation**:
- Escape XML special characters before embedding user content.
- Reuse the existing `escape_xml` helper used by LLM clients or encode content as JSON.

---

### [MEDIUM] [PERFORMANCE] Per-hit git branch scanning in recall hot path

**Location**: `src/services/recall.rs:330` (calls `branch_exists`) + `src/gc/branch.rs:520`

**Description**:
`lazy_tombstone_stale_branches` calls `branch_exists` for each hit. `branch_exists` performs repository discovery and enumerates branches each time. For larger result sets, this creates repeated IO and iteration on a hot path.

**Impact**:
Recall latency grows with result count; repeated git scans can become a bottleneck for large repositories or frequent recall operations.

**Evidence**:
```rust
for hit in hits.iter_mut() {
    ...
    if branch_exists(branch) {
        continue;
    }
    ...
}
```

**Remediation**:
- Cache branch existence per unique branch name within the recall call.
- Reuse a single `Repository` handle and precompute branch sets.

---

### [MEDIUM] [ARCHITECTURE] Repo-path fallback reintroduces repo-local storage

**Location**: `src/services/path_manager.rs:71`

**Description**:
`PathManager::for_repo` falls back to the repo root if `get_user_data_dir()` fails. This conflicts with the user-scope-only storage model and can create repo-local `.subcog` storage unexpectedly.

**Impact**:
Storage may split across locations, and repo-local writes can reappear in environments where user data dirs are unavailable, undermining the "user database only" guarantee.

**Evidence**:
```rust
let base_dir = get_user_data_dir().unwrap_or_else(|_| repo_root.as_ref().to_path_buf());
```

**Remediation**:
- Prefer failing with a clear error and guidance for configuring storage.
- If a fallback is needed, require an explicit config opt-in.

---

## Low Priority Findings (🟢)

### [LOW] [QUALITY] LLM HTTP client fallback drops configured timeouts

**Location**: `src/llm/mod.rs:337`

**Description**:
If `reqwest::blocking::Client::builder()` fails, the fallback uses `Client::new()` without reapplying timeouts. This silently removes timeouts and can hang on network stalls.

**Impact**:
Reduced resilience in rare builder-failure scenarios; unexpected hangs if the client is constructed in the fallback path.

**Evidence**:
```rust
builder.build().unwrap_or_else(|err| {
    tracing::warn!("Failed to build LLM HTTP client: {err}");
    reqwest::blocking::Client::new()
})
```

**Remediation**:
- Propagate the error, or build a fallback client with the same timeouts.

---

### [LOW] [TEST COVERAGE] No tests for MCP HTTP auth/rate limit middleware

**Location**: `src/mcp/server.rs` (only CORS tests present)

**Description**:
The HTTP auth middleware contains JWT validation and per-client rate limiting logic, but there are no tests covering error paths or rate-limit behavior.

**Impact**:
Auth and rate limit regressions could ship without detection, especially as config options evolve.

**Remediation**:
- Add tests for missing auth header, invalid JWT, and rate limit exceeded responses.
- Verify rate limit window reset behavior.

---

## Appendix

### Files Reviewed
- `src/main.rs`
- `src/services/recall.rs`
- `src/gc/branch.rs`
- `src/services/path_manager.rs`
- `src/services/enrichment.rs`
- `src/services/prompt_enrichment.rs`
- `src/llm/mod.rs`
- `src/llm/lmstudio.rs`
- `src/llm/ollama.rs`
- `src/llm/openai.rs`
- `src/mcp/server.rs`
- `src/security/secrets.rs`
- `src/security/pii.rs`
- `src/security/redactor.rs`
- `src/hooks/user_prompt.rs`
- `src/embedding/fastembed.rs`

### Tools & Methods
- Ripgrep (`rg`) for targeted search
- Manual review of key modules

### Notes
- Tree/scan excluded `target/` and ignored files per `.gitignore` for signal-to-noise.
