# Subcog Integrator Skill

Analyze and enhance AI prompts, skills, commands, hooks, and system configurations to leverage Subcog's persistent memory system effectively.

## Trigger Phrases

- "enhance my prompts with subcog", "integrate subcog"
- "improve memory usage", "optimize for subcog"
- "add memory support", "memory-aware prompts"
- "review my CLAUDE.md", "analyze my hooks"
- "help me use subcog better", "subcog best practices"

## Quick Reference

| Mode | What It Does | Example Trigger |
|------|--------------|-----------------|
| Analyze | Audit existing configs for integration gaps | "review my CLAUDE.md" |
| Enhance | Add memory integration to existing artifacts | "add subcog to my skill" |
| Create | Generate new memory-aware artifacts | "create a memory-aware skill" |

## Execution Strategy

<strategy>
**MCP-First Approach:**
1. Use `mcp__subcog__subcog_status` to verify memory system health
2. Use `mcp__subcog__subcog_recall` to surface existing patterns
3. Use `mcp__subcog__subcog_capture` to store new patterns discovered
4. Fall back to CLI: `subcog integrate --analyze` if MCP unavailable

**Interactive Workflow:**
1. Discover current integration state (CLAUDE.md, hooks, skills)
2. Ask user what to analyze/enhance via `AskUserQuestion`
3. Generate recommendations with code snippets
4. Offer to apply changes
</strategy>

## Workflow

<workflow>
When invoked:

1. **Discover current state**
   - Read project and user CLAUDE.md files
   - Check for hooks configuration
   - Scan skills/ and commands/ directories
   - Call `subcog_status` for health check

2. **Surface existing patterns**
   - `subcog_recall` with "subcog integration patterns"

3. **Analyze and recommend**
   - Check for Memory Protocol section in CLAUDE.md
   - Verify hooks are configured
   - Review skills for recall/capture integration

4. **Present report**
   - Current state summary
   - Missing integration points
   - Priority-ranked recommendations

5. **Apply changes** (with user approval)
</workflow>

## Analysis Checklist

<checklist>
**CLAUDE.md:**
- [ ] Subcog Memory Protocol section exists
- [ ] `subcog_init` instruction at session start
- [ ] Recall-before-implement pattern documented
- [ ] MCP-only access pattern specified

**Hooks:**
- [ ] `session_start` for context injection
- [ ] `user_prompt_submit` for intent detection

**Skills/Commands:**
- [ ] Include `subcog_recall` in workflows
- [ ] Include `subcog_capture` for decisions
</checklist>

## MCP Tool Reference

<mcp>
**Tool:** `mcp__subcog__subcog_recall`
- Surface existing integration patterns and guidance

**Tool:** `mcp__subcog__subcog_capture`
- Store new patterns discovered during analysis

**Tool:** `mcp__subcog__subcog_status`
- Verify memory system is healthy before analysis

**Other Tools:**
- `AskUserQuestion` - Interactive artifact selection
- `Read` / `Edit` / `Write` - Analyze and modify files
- `Glob` - Find configuration files
</mcp>

## Command

Use `/subcog:integrate` for the full interactive workflow with detailed analysis patterns, templates, and examples.
