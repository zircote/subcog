---
description: Analyze and enhance AI artifacts to leverage Subcog memory effectively
allowed-tools: mcp__subcog__subcog_recall, mcp__subcog__subcog_capture, mcp__subcog__subcog_status, mcp__subcog__subcog_namespaces, AskUserQuestion, Read, Edit, Write, Glob, Bash
argument-hint: "[--analyze | --enhance | --create] [artifact-path]"
---

# /subcog:integrate

Analyze, enhance, or create AI prompts, skills, commands, and configurations to leverage Subcog's persistent memory system effectively.

## Usage

```
/subcog:integrate                           # Interactive mode (recommended)
/subcog:integrate --analyze                 # Analyze current project for integration opportunities
/subcog:integrate --enhance CLAUDE.md       # Enhance specific file
/subcog:integrate --create skill            # Create new memory-aware artifact
```

## Interactive Workflow

<workflow>
When invoked without arguments, guide the user through these steps:

### Step 1: Discover Current State
1. Check for existing Subcog integration:
   - Read project `CLAUDE.md` if exists
   - Read `~/.claude/CLAUDE.md` if exists
   - Check for `hooks/hooks.json` or `.claude/hooks.json`
   - Scan `skills/` and `commands/` directories
2. Call `subcog_status` to verify memory system is healthy
3. Call `subcog_recall` with query "subcog integration patterns" to surface existing guidance

### Step 2: Ask What to Analyze/Enhance
Use `AskUserQuestion` to present options:
- CLAUDE.md / System Prompts
- Skills & Commands
- Hooks Configuration
- MCP Tool Usage Patterns
- All of the above (comprehensive audit)

### Step 3: Perform Analysis
Based on selection, analyze artifacts and generate recommendations.

### Step 4: Present Recommendations
Show a structured report with:
- Current state summary
- Missing integration points
- Specific recommendations with code snippets
- Priority ranking (high/medium/low impact)

### Step 5: Offer to Apply Changes
Ask user which recommendations to apply, then make the changes.
</workflow>

## Analysis Patterns

<analysis>
### CLAUDE.md / System Prompts

**Check for:**
- [ ] Subcog Memory Protocol section exists
- [ ] `subcog_init` call instruction at session start
- [ ] Recall-before-implement pattern documented
- [ ] Capture-immediately guidance present
- [ ] MCP-only access pattern specified (no shell commands)
- [ ] Namespace guidance for the project domain

**Missing Protocol - Add This:**
```markdown
## Subcog Memory Protocol (MANDATORY)

At the start of EVERY session, call `subcog_init` to load memory context and best practices.

During the session:
- **Before implementing**: Recall relevant decisions and patterns
- **Capture immediately**: When decisions, patterns, learnings, or fixes are identified
- Access Subcog only via MCP tools, never shell commands
```

**Enhanced Protocol - For Power Users:**
```markdown
## Subcog Memory Protocol (MANDATORY)

### Session Start
Call `subcog_init` at the start of EVERY session to:
- Load usage guidance and best practices
- Check memory system health
- Surface relevant project context

### Before Implementing Features
1. `subcog_recall` with query matching the task domain
2. Check for existing decisions, patterns, and learnings
3. Reference memory IDs when building on previous work

### During Development
Capture immediately when you identify:
- **Decisions** (`ns:decisions`): Architecture choices, technology selections, trade-offs
- **Patterns** (`ns:patterns`): Coding conventions, recurring practices, "when X do Y"
- **Learnings** (`ns:learnings`): Gotchas, TILs, discovered behaviors
- **Tech-debt** (`ns:tech-debt`): TODOs, temporary solutions, future refactoring needs

### Capture Quality
- Include the "why" (rationale), not just the "what"
- Add relevant file paths via `source` parameter
- Use descriptive, searchable tags
- Keep content concise (1-3 paragraphs)

### Access Pattern
- Use MCP tools exclusively (`mcp__subcog__*`)
- Never use shell commands for Subcog operations
```

### Skills & Commands

**Check for:**
- [ ] MCP tool references include Subcog tools where relevant
- [ ] Workflow includes recall step before major operations
- [ ] Trigger phrases detect memory-related intent
- [ ] Capture guidance for decisions made during skill execution

**Skill Enhancement Pattern:**
```markdown
## Execution Strategy

<strategy>
**Memory-Aware Workflow:**
1. Call `subcog_recall` to surface relevant existing memories
2. [Perform the skill's primary function]
3. If significant decisions are made, suggest capturing via `subcog_capture`

**MCP Tools:**
- `mcp__subcog__subcog_recall` - Check for existing patterns before suggesting
- `mcp__subcog__subcog_capture` - Store new patterns discovered
</strategy>
```

**Command Enhancement - Add to frontmatter:**
```yaml
allowed-tools: [...existing tools..., mcp__subcog__subcog_recall, mcp__subcog__subcog_capture]
```

### Hooks Configuration

**Check for:**
- [ ] `session_start` hook configured for context injection
- [ ] `user_prompt_submit` hook for intent detection
- [ ] `pre_compact` hook for auto-capture before context loss
- [ ] `stop` hook for session summary/sync

**Minimal hooks.json:**
```json
{
  "hooks": [
    {
      "matcher": { "event": "session_start" },
      "hooks": [{
        "type": "command",
        "command": "subcog hook session-start"
      }]
    },
    {
      "matcher": { "event": "user_prompt_submit" },
      "hooks": [{
        "type": "command",
        "command": "sh -c 'subcog hook user-prompt-submit \"$PROMPT\"'"
      }]
    }
  ]
}
```

**Full hooks.json:**
```json
{
  "hooks": [
    {
      "matcher": { "event": "session_start" },
      "hooks": [{
        "type": "command",
        "command": "subcog hook session-start"
      }]
    },
    {
      "matcher": { "event": "user_prompt_submit" },
      "hooks": [{
        "type": "command",
        "command": "sh -c 'subcog hook user-prompt-submit \"$PROMPT\"'"
      }]
    },
    {
      "matcher": { "event": "post_tool_use" },
      "hooks": [{
        "type": "command",
        "command": "subcog hook post-tool-use"
      }]
    },
    {
      "matcher": { "event": "pre_compact" },
      "hooks": [{
        "type": "command",
        "command": "subcog hook pre-compact"
      }]
    },
    {
      "matcher": { "event": "stop" },
      "hooks": [{
        "type": "command",
        "command": "subcog hook stop"
      }]
    }
  ]
}
```

### MCP Tool Usage Patterns

**Check for:**
- [ ] `subcog_init` called at session start
- [ ] `subcog_recall` used before implementing features
- [ ] `subcog_capture` used after significant decisions
- [ ] Appropriate namespace selection
- [ ] Effective search queries (hybrid mode, namespace filters)

**Effective Recall Patterns:**
```yaml
# Before implementing a feature
subcog_recall: query="authentication patterns", mode="hybrid", limit=10

# When debugging
subcog_recall: query="error handling gotchas", filter="ns:learnings ns:tech-debt"

# Finding specific decisions
subcog_recall: query="database choice", filter="ns:decisions", mode="text"
```

**Effective Capture Patterns:**
```yaml
# Architectural decision
subcog_capture:
  namespace: decisions
  content: "Chose PostgreSQL over SQLite for production due to concurrent write requirements."
  tags: [database, postgresql, sqlite, architecture]
  source: docker-compose.yml

# Discovered gotcha
subcog_capture:
  namespace: learnings
  content: "TIL: OAuth2 refresh tokens must be stored securely - localStorage is vulnerable to XSS."
  tags: [oauth2, security, tokens, frontend]

# Coding pattern
subcog_capture:
  namespace: patterns
  content: "Always validate redirect URIs server-side before OAuth2 authorization."
  tags: [oauth2, security, validation, patterns]
```
</analysis>

## Namespace Selection Guide

<namespaces>
| Namespace | Use When | Signal Words | Example |
|-----------|----------|--------------|---------|
| `decisions` | Making choices between alternatives | "decided", "chose", "going with" | "Chose React over Vue" |
| `patterns` | Establishing recurring practices | "always", "never", "convention" | "Always use repository pattern" |
| `learnings` | Discovering something new | "TIL", "gotcha", "discovered" | "TIL: SQLite locks on writes" |
| `context` | Recording project background | "because", "constraint", "requirement" | "Project requires HIPAA compliance" |
| `tech-debt` | Noting future work | "TODO", "FIXME", "temporary" | "TODO: Replace polling with WebSockets" |
| `apis` | Documenting interfaces | "endpoint", "API", "schema" | "POST /auth/token returns JWT" |
| `config` | Recording settings | "config", "environment", "setting" | "Redis runs on port 6380 in dev" |
| `security` | Security information | "auth", "vulnerability", "permission" | "API keys must never be logged" |
| `performance` | Performance insights | "optimization", "benchmark", "latency" | "DB queries over 100ms trigger alerts" |
| `testing` | Test strategies | "edge case", "fixture", "coverage" | "Always test with empty arrays" |
</namespaces>

## Report Template

<report>
When presenting analysis results, use this format:

```markdown
# Subcog Integration Report

## Current State
- **Project CLAUDE.md**: [Found/Missing] - [Integration status]
- **User CLAUDE.md**: [Found/Missing] - [Integration status]
- **Hooks**: [Configured/Partial/Missing]
- **Skills/Commands**: [X] analyzed, [Y] need enhancement
- **Memory System**: [Healthy/Issues detected]

## Recommendations

### High Priority
1. **[Title]** - [Artifact]
   - Issue: [What's missing]
   - Impact: [Why it matters]
   - Fix: [Specific action]

### Medium Priority
...

### Low Priority
...

## Quick Wins
- [ ] Add Subcog Memory Protocol to CLAUDE.md
- [ ] Configure session_start hook
- [ ] Add subcog_recall to [skill name]

## Next Steps
Would you like me to apply any of these recommendations?
```
</report>

## Creation Templates

<create>
### New Memory-Aware Skill

```markdown
# [Skill Name]

[Brief description]

## Trigger Phrases

- [trigger 1]
- [trigger 2]

## Execution Strategy

<strategy>
**Memory-Aware Workflow:**
1. Call `subcog_recall` with query relevant to this skill's domain
2. Review existing patterns and decisions
3. [Primary skill action]
4. If decisions made, capture via `subcog_capture`

**MCP-First Approach:**
- Use `mcp__subcog__subcog_recall` before suggesting implementations
- Use `mcp__subcog__subcog_capture` to store significant decisions
</strategy>

## MCP Tool Reference

<mcp>
**Memory Tools:**
- `mcp__subcog__subcog_recall` - Surface existing context
- `mcp__subcog__subcog_capture` - Store new knowledge
</mcp>
```

### New Memory-Aware Command

```markdown
---
description: [Command description]
allowed-tools: mcp__subcog__subcog_recall, mcp__subcog__subcog_capture, [other tools]
argument-hint: "[arguments]"
---

# /[command-name]

[Command description]

## Workflow

<workflow>
1. **Recall context**: Search for relevant existing memories
2. **[Command-specific steps]**
3. **Capture outcomes**: Store significant decisions or learnings
</workflow>
```
</create>

## Examples

<examples>
### Example: Analyze Project

```
User: /subcog:integrate
Assistant: I will analyze your project for Subcog integration opportunities.

[Reads CLAUDE.md, checks for hooks.json, scans skills/ directory]
[Calls subcog_status]

# Subcog Integration Report

## Current State
- **Project CLAUDE.md**: Found - Missing memory protocol
- **User CLAUDE.md**: Found - Has basic protocol
- **Hooks**: Missing - No hooks.json found
- **Skills/Commands**: 3 analyzed, 2 need enhancement
- **Memory System**: Healthy (100 memories)

## Recommendations

### High Priority
1. **Add Memory Protocol** - CLAUDE.md
   - Issue: No Subcog protocol section
   - Impact: AI assistants will not recall/capture memories
   - Fix: Add mandatory protocol section

Would you like me to apply any of these recommendations?
```

### Example: Enhance Specific File

```
User: /subcog:integrate --enhance skills/code-review/SKILL.md
Assistant: I will enhance skills/code-review/SKILL.md with Subcog memory integration.

[Reads the skill file]
[Calls subcog_recall with query "code review patterns"]

## Analysis

The skill is missing memory integration. I recommend:

1. **Add recall step** - Surface existing code review patterns before reviewing
2. **Add capture guidance** - Store significant findings as learnings
3. **Update MCP tools** - Include subcog_recall and subcog_capture

## Suggested Enhancement

Add to Execution Strategy:
```markdown
**Memory-Aware Workflow:**
1. Call `subcog_recall` with "code review patterns" to surface existing guidelines
2. Perform the code review
3. Capture significant findings via `subcog_capture` in learnings namespace
```

Would you like me to apply this enhancement?
```

### Example: Create New Artifact

```
User: /subcog:integrate --create skill
Assistant: I will help you create a new memory-aware skill. Let me ask a few questions.

[Uses AskUserQuestion]

Questions:
1. What should this skill be named?
2. What is its primary purpose?
3. What MCP tools does it need besides Subcog?

User: [Answers: "api-designer", "Help design REST APIs", "Read, Write"]
Assistant: I will create skills/api-designer/SKILL.md with Subcog integration.

[Calls subcog_recall with "API design patterns"]
[Creates the skill file]

Created: skills/api-designer/SKILL.md

The skill includes:
- Trigger phrases for API design requests
- Memory-aware workflow (recall before designing, capture decisions)
- MCP tool references including Subcog tools

Would you like to review the generated skill?
```
</examples>

## See Also

- [memory-capture](../skills/memory-capture/SKILL.md) - Capture skill reference
- [memory-recall](../skills/memory-recall/SKILL.md) - Recall skill reference
- [Hooks Documentation](../docs/hooks/README.md) - Hook configuration details
- [MCP Resources](../docs/mcp/resources.md) - Available MCP resources
