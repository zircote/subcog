# Prompt Management Feature

**Project ID**: `2025-12-30-prompt-management`
**Status**: Completed
**Created**: 2025-12-30
**Completed**: 2025-12-30
**Issues**: #6, #8, #9, #10, #11, #12, #13, #14 (all closed)
**PR**: #26
**Outcome**: Success

## Overview

Implement user prompt management for subcog - allowing users to save, manage, and run reusable prompt templates with variable substitution and MCP sampling integration.

## Key Deliverables

1. **Namespace**: `Namespace::Prompts` for storing prompt templates
2. **Models**: `PromptTemplate`, `PromptVariable` data structures
3. **Parsers**: Support for `.md`, `.yaml`, `.json`, `.txt` file formats
4. **Service**: `PromptService` for CRUD operations with domain hierarchy search
5. **MCP Tools**: `prompt.save`, `prompt.list`, `prompt.get`, `prompt.run`, `prompt.delete`
6. **CLI**: `subcog prompt` subcommand group with file input support
7. **Help Integration**: Format documentation and hook-based validation

## Phase Summary

| Phase | Issue | Description | Est. LOC |
|-------|-------|-------------|----------|
| 1 | #8 | Foundation - Models & Variable Extraction | ~200-300 |
| 2 | #9 | File Parsing - Format Support | ~300-400 |
| 3 | #10 | Storage - PromptService & Indexing | ~400-500 |
| 4 | #11 | MCP Integration - Tools & Sampling | ~500-600 |
| 5 | #12 | CLI - Subcommands | ~400-500 |
| 6 | #13 | Help & Hooks - AI Guidance | ~300-400 |
| 7 | #14 | Polish - Validation, Docs & Testing | ~300-400 |

**Total**: ~2,400-3,100 lines of code + tests

## Documents

- [REQUIREMENTS.md](./REQUIREMENTS.md) - Product requirements
- [ARCHITECTURE.md](./ARCHITECTURE.md) - Technical architecture
- [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md) - Phased implementation plan
- [PROGRESS.md](./PROGRESS.md) - Implementation tracking (created during implementation)

## Dependency Graph

```
Phase 1 (Foundation) ───────┬───────────────────────────────────────┐
                            │                                       │
Phase 2 (File Parsing) ─────┴──┬────────────────────────────────────┤
                               │                                    │
Phase 3 (Storage) ─────────────┴──┬─────────────────────────────────┤
                                  │                                 │
Phase 4 (MCP) ────────────────────┴──┬──────────────────────────────┤
                                     │                              │
Phase 5 (CLI) ───────────────────────┤                              │
                                     │                              │
Phase 6 (Help & Hooks) ──────────────┴──────────────────────────────┤
                                                                    │
Phase 7 (Polish) ───────────────────────────────────────────────────┘
```

## Quick Start

After implementation:

```bash
# Save a prompt from file
subcog prompt save --name "code-review" --from-file ./prompts/review.md

# List prompts
subcog prompt list

# Run prompt interactively
subcog prompt run code-review

# Run with pre-filled variables
subcog prompt run code-review --var language=rust --var code="fn main() {}"
```
