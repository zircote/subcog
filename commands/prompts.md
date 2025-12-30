---
description: Work with MCP prompt templates for memory operations
allowed-tools: mcp__subcog__subcog_capture, mcp__subcog__subcog_recall, AskUserQuestion, Bash
argument-hint: "<prompt-name> [arguments...] | --list"
---

# /subcog:prompts

Access and use pre-defined prompt templates for common memory operations.

## Usage

```
/subcog:prompts --list
/subcog:prompts tutorial [--focus capture|recall|namespaces|workflows]
/subcog:prompts capture-assistant
/subcog:prompts document-decision "Use PostgreSQL for storage"
/subcog:prompts search-help "find authentication patterns"
/subcog:prompts review [--namespace decisions] [--action summarize]
```

## Available Prompts

<prompts>
| Prompt | Description | Arguments |
|--------|-------------|-----------|
| `tutorial` | Interactive Subcog tutorial | `--focus`, `--familiarity` |
| `capture-assistant` | Help decide what to capture | `context` (from conversation) |
| `document-decision` | Structure a decision properly | `decision`, `alternatives` |
| `search-help` | Craft effective search queries | `goal` |
| `review` | Review and analyze memories | `--namespace`, `--action` |
</prompts>

## Prompt Details

### tutorial

<tutorial>
Interactive tutorial for learning Subcog.

**Arguments:**
- `--focus`: Topic to focus on (overview, capture, recall, namespaces, workflows, best-practices)
- `--familiarity`: Your experience level (beginner, intermediate, advanced)

**Example:**
```
/subcog:prompts tutorial --focus capture --familiarity intermediate
```
</tutorial>

### capture-assistant

<capture-assistant>
Analyzes current conversation to suggest what memories to capture.

**Process:**
1. Reviews recent conversation context
2. Identifies potential memories (decisions, learnings, patterns)
3. Suggests namespace and content for each
4. Uses AskUserQuestion to confirm captures
</capture-assistant>

### document-decision

<document-decision>
Helps document architectural or design decisions properly.

**Arguments:**
- `decision` (required): Brief description of the decision
- `alternatives` (optional): Other options considered

**Example:**
```
/subcog:prompts document-decision "Use PostgreSQL" --alternatives "MySQL, SQLite"
```
</document-decision>

### search-help

<search-help>
Helps craft effective search queries.

**Arguments:**
- `goal` (required): What you're trying to find

**Example:**
```
/subcog:prompts search-help "find how we handle authentication errors"
```
</search-help>

### review

<review>
Review and analyze existing memories.

**Arguments:**
- `--namespace`: Focus on specific namespace
- `--action`: summarize, consolidate, archive, or cleanup

**Example:**
```
/subcog:prompts review --namespace decisions --action summarize
```
</review>
