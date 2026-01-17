---
project_id: SPEC-2026-01-01-002
project_name: "Prompt Variable Context-Aware Extraction"
slug: prompt-variable-context-awareness
status: completed
created: 2026-01-01T00:00:00Z
approved: null
started: 2026-01-02T00:20:00Z
completed: 2026-01-02T01:20:00Z
final_effort: 8 hours
outcome: success
expires: 2026-04-01T00:00:00Z
superseded_by: null
tags: [bug-fix, prompt, variable-extraction, parsing, llm-enrichment]
stakeholders: []
github_issue: 29
github_url: https://github.com/zircote/subcog/issues/29
---

# Prompt Variable Context-Aware Extraction

## Summary

This specification addresses two related capabilities:

1. **Bug Fix (Issue #29)**: Fix the prompt variable extraction logic to ignore `{{variable}}` patterns inside fenced code blocks
2. **Enhancement**: Add LLM-assisted frontmatter enrichment at save time, generating descriptions, defaults, required flags, and prompt-level metadata automatically

## Quick Links

- **GitHub Issue**: [#29](https://github.com/zircote/subcog/issues/29)
- **Requirements**: [REQUIREMENTS.md](./REQUIREMENTS.md)
- **Architecture**: [ARCHITECTURE.md](./ARCHITECTURE.md)
- **Implementation Plan**: [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md)
- **Decisions**: [DECISIONS.md](./DECISIONS.md)

## Status

| Phase | Status |
|-------|--------|
| Requirements | Complete |
| Architecture | Complete |
| Implementation Plan | Complete |
| Decisions (ADRs) | Complete |
| Approval | ️ Proceeding without formal approval |
| Implementation | In Progress |
