# Research Report: System Prompts for Non-Claude CLI Assistants

## Executive Summary

This research investigated how to enable non-Claude CLI assistants (OpenAI Codex, OpenCode, and similar tools) to leverage Subcog's persistent memory capabilities. Since these tools lack Claude Code's hooks system, we developed system prompt templates that encode hook behaviors as explicit LLM instructions.

**Key Deliverables:**
1. `docs/integrations/AGENTS.md` - System prompt for OpenAI Codex CLI
2. `docs/integrations/opencode-subcog-agent.md` - Agent definition for OpenCode
3. `docs/integrations/opencode.json` - Example OpenCode configuration
4. `docs/integrations/README.md` - Comprehensive integration guide

**Key Finding:** Most hook functionality can be replicated via well-crafted system prompts with HIGH fidelity for capture/recall operations, but automatic lifecycle events and server-side deduplication cannot be replicated.

---

## Research Scope

- **Subject:** System prompts enabling Subcog memory for non-Claude CLIs
- **Research Type:** HYBRID (CODEBASE + TECHNICAL)
- **Methodology:**
  - Codebase exploration of Subcog hooks implementation
  - Web research on OpenAI Codex and OpenCode documentation
  - Synthesis of hook behaviors into prompt instructions
- **LSP Available:** Yes
- **Sources:**
  - Subcog source code (`src/hooks/`, `src/mcp/tools/`)
  - [OpenAI Codex CLI Documentation](https://developers.openai.com/codex/cli/)
  - [OpenCode Documentation](https://opencode.ai/docs/)
- **Limitations:**
  - OpenCode MCP schema not fully documented
  - Cannot test integration end-to-end without actual CLI tools

---

## Key Findings

### Finding 1: Hook Behaviors Are Well-Documented and Encodable

**Evidence:** Direct codebase exploration of `src/hooks/*.rs` revealed:
- SessionStart: Context injection, memory statistics, tutorial invitation
- UserPromptSubmit: 5 capture signal patterns, 6 search intent types
- PostToolUse: Tool-specific memory queries
- PreCompact: Auto-capture with 3-tier deduplication
- Stop: Session summary, auto-sync

**Analysis:** All hook behaviors except deduplication and timing enforcement can be expressed as LLM instructions. The capture signals (decision, pattern, learning, blocker, tech-debt) are regex patterns that translate directly to instruction text.

**Confidence:** HIGH - Direct code analysis

### Finding 2: OpenAI Codex Uses AGENTS.md for Instructions

**Evidence:** [Official documentation](https://developers.openai.com/codex/guides/agents-md/) confirms:
- Discovery order: `~/.codex/AGENTS.override.md` → `~/.codex/AGENTS.md` → project root
- Max 32KB combined content
- Markdown format with no special syntax required
- MCP servers configured in `~/.codex/config.toml`

**Analysis:** Codex's AGENTS.md is ideal for Subcog integration - we can provide comprehensive memory protocols up to 32KB.

**Confidence:** HIGH - Official documentation

### Finding 3: OpenCode Supports Multiple Agent Definition Formats

**Evidence:** [OpenCode docs](https://opencode.ai/docs/agents/) show:
- JSON config in `opencode.json`
- Markdown agents in `.opencode/agent/`
- YAML frontmatter for metadata
- `prompt` field references external files
- MCP configured in `mcp` section

**Analysis:** OpenCode's flexibility allows both inline prompts and file references. The agent markdown format is similar to AGENTS.md.

**Confidence:** HIGH - Official documentation

### Finding 4: Capture Signal Detection Is Replicable

**Evidence:** From `src/hooks/user_prompt.rs`:
- Decision signals: `"we('re| are|'ll| will) (going to |gonna )?use"`, `"decided"`, `"choosing"`
- Pattern signals: `"pattern"`, `"convention"`, `"always"`, `"never"`
- Learning signals: `"TIL"`, `"turns out"`, `"discovered"`, `"gotcha"`
- Blocker signals: `"fixed"`, `"solved"`, `"the issue was"`
- Tech-debt signals: `"TODO"`, `"FIXME"`, `"temporary"`, `"refactor"`

**Analysis:** These patterns are easily encoded in system prompts as trigger phrases. LLMs can match them with similar accuracy to regex patterns.

**Confidence:** HIGH - Direct code analysis

### Finding 5: Search Intent Detection Requires Heuristics

**Evidence:** From `src/hooks/search_intent.rs`:
- 6 intent types: HowTo, Location, Explanation, Comparison, Troubleshoot, General
- Keyword patterns per intent type
- Optional LLM classification with 200ms timeout

**Analysis:** Without server-side classification, we rely on prompt-based heuristics. This works for common patterns but lacks the confidence scoring of the hybrid detection system.

**Confidence:** MEDIUM - Heuristic-based replication

### Finding 6: Deduplication Cannot Be Replicated

**Evidence:** From `src/services/deduplication/`:
- 3-tier system: exact hash, semantic similarity, recent cache
- Requires server-side embeddings and hash storage
- Short-circuit evaluation for performance

**Analysis:** LLM cannot efficiently check for duplicate content before capture. Users must manually avoid duplicates or use `subcog_consolidate` periodically.

**Confidence:** HIGH - Architectural limitation

---

## Recommendations

### 1. Use the Provided AGENTS.md for Codex
Copy `docs/integrations/AGENTS.md` to `~/.codex/AGENTS.md` for global memory integration.

### 2. Configure OpenCode with Agent + MCP
Use the provided `opencode.json` and `opencode-subcog-agent.md` templates, verifying against current OpenCode schema.

### 3. Start Sessions with Status Check
Without SessionStart hook, users should train the LLM to call `subcog_status` at session start.

### 4. Rely on Explicit Capture Signals
The prompt includes comprehensive signal patterns. For best results, users should use explicit commands like "capture this decision" when implicit detection fails.

### 5. Use Consolidation for Deduplication
Periodically run `subcog_consolidate --strategy dedupe` to clean up duplicate memories created without server-side deduplication.

### 6. Document Tool-Specific Quirks
Each CLI tool has unique behaviors. The integration guide should be updated as users discover quirks.

---

## Open Questions

1. **How do Codex/OpenCode handle token limits for system prompts?**
   - The AGENTS.md file is ~6KB, well under Codex's 32KB limit
   - OpenCode limits unclear; may need testing

2. **Do these CLIs support MCP sampling for consolidation/enrichment?**
   - Subcog's `subcog_consolidate` and `subcog_enrich` use MCP sampling
   - Unclear if Codex/OpenCode support this feature

3. **What's the best session-end workflow without Stop hook?**
   - Current approach: manual checklist in prompt
   - Could potentially use CLI tool's exit detection if available

---

## Appendix: Files Created

| File | Size | Purpose |
|------|------|---------|
| `docs/integrations/README.md` | ~12KB | Comprehensive integration guide |
| `docs/integrations/AGENTS.md` | ~6KB | OpenAI Codex system prompt |
| `docs/integrations/opencode-subcog-agent.md` | ~3KB | OpenCode agent definition |
| `docs/integrations/opencode.json` | ~1KB | Example OpenCode config |
| `RESEARCH_PLAN.md` | ~2KB | Research planning (internal) |
| `RESEARCH_NOTES.md` | ~4KB | Evidence synthesis (internal) |

---

## Sources Consulted

### Codebase (LSP-assisted)
- `src/hooks/mod.rs` - HookHandler trait
- `src/hooks/session_start.rs` - SessionStart implementation
- `src/hooks/user_prompt.rs` - UserPromptSubmit implementation
- `src/hooks/post_tool_use.rs` - PostToolUse implementation
- `src/hooks/pre_compact/mod.rs` - PreCompact implementation
- `src/hooks/stop.rs` - Stop implementation
- `src/mcp/tools/definitions.rs` - MCP tool schemas

### External Documentation
- [OpenAI Codex CLI](https://developers.openai.com/codex/cli/)
- [Codex AGENTS.md Guide](https://developers.openai.com/codex/guides/agents-md/)
- [Codex Basic Configuration](https://developers.openai.com/codex/config-basic/)
- [Codex CLI Features](https://developers.openai.com/codex/cli/features/)
- [OpenCode Documentation](https://opencode.ai/docs/)
- [OpenCode Agents](https://opencode.ai/docs/agents/)
- [OpenCode Config](https://opencode.ai/docs/config/)
- [OpenCode MCP Servers](https://opencode.ai/docs/mcp-servers/)
