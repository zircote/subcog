# Implementation Plan: User Prompt Management

**Version**: 1.0
**Status**: Draft
**Last Updated**: 2025-12-30

## Overview

This plan implements user prompt management across 7 phases, corresponding to GitHub issues #8-#14. Each phase builds on the previous, with clear deliverables and acceptance criteria.

## Phase Summary

| Phase | Issue | Description | Est. LOC | Dependencies |
|-------|-------|-------------|----------|--------------|
| 1 | #8 | Foundation - Models & Variable Extraction | ~250 | None |
| 2 | #9 | File Parsing - Format Support | ~350 | Phase 1 |
| 3 | #10 | Storage - PromptService & Indexing | ~450 | Phase 1, 2 |
| 4 | #11 | MCP Integration - Tools & Sampling | ~550 | Phase 1, 2, 3 |
| 5 | #12 | CLI - Subcommands | ~450 | Phase 2, 3 |
| 6 | #13 | Help & Hooks - AI Guidance | ~350 | Phase 1, 4 |
| 7 | #14 | Polish - Validation, Docs & Testing | ~350 | All phases |

**Total**: ~2,750 lines of code + tests

---

## Phase 1: Foundation - Models & Variable Extraction

**GitHub Issue**: #8
**Objective**: Establish core data models and variable extraction logic.

### Tasks

- [ ] **P1-T1**: Add `Namespace::Prompts` variant to `src/models/domain.rs`
  - Add to enum with doc comment
  - Implement `as_str()` mapping
  - Update `Namespace::parse()` to handle "prompts"
  - Update namespace list in help content

- [ ] **P1-T2**: Create `src/models/prompt.rs`
  - Define `PromptTemplate` struct
  - Define `PromptVariable` struct
  - Define `ExtractedVariable` struct
  - Define `ValidationResult` and `ValidationIssue` structs
  - Implement `Default` traits
  - Add serde derive macros

- [ ] **P1-T3**: Implement variable extraction
  - Create `extract_variables(content: &str) -> Vec<ExtractedVariable>`
  - Regex pattern: `\{\{(\w+)\}\}`
  - Return variables in order of first appearance
  - Deduplicate variable names

- [ ] **P1-T4**: Implement variable substitution
  - Create `substitute_variables(content, values, variables) -> Result<String>`
  - Check required variables have values or defaults
  - Apply defaults for missing optional variables
  - Replace all `{{var}}` patterns
  - Error on missing required variables

- [ ] **P1-T5**: Export models from `src/models/mod.rs`
  - Add `mod prompt;`
  - Add `pub use prompt::*;`

- [ ] **P1-T6**: Add unit tests
  - Test variable extraction (valid patterns)
  - Test variable extraction (edge cases: escaped, nested)
  - Test variable substitution (complete values)
  - Test variable substitution (with defaults)
  - Test variable substitution (missing required)
  - Test model serialization roundtrip

### Deliverables

- `src/models/prompt.rs` (~150 lines)
- Modified `src/models/domain.rs` (~10 lines)
- Modified `src/models/mod.rs` (~5 lines)
- Tests (~100 lines)

### Acceptance Criteria

- [x] `Namespace::Prompts` exists and serializes to "prompts"
- [x] `PromptTemplate` can be serialized to JSON and back
- [x] `extract_variables("Hello {{name}}, your {{item}} is ready")` returns `["name", "item"]`
- [x] `substitute_variables` correctly replaces all `{{var}}` patterns
- [x] Missing required variable produces error with variable name
- [x] All tests pass

---

## Phase 2: File Parsing - Format Support

**GitHub Issue**: #9
**Objective**: Implement parsers for loading prompts from various file formats.

### Tasks

- [ ] **P2-T1**: Create `src/services/prompt_parser.rs`
  - Define `PromptParser` trait
  - Define `PromptFormat` enum
  - Create `PromptFileParser` struct

- [ ] **P2-T2**: Implement `MarkdownParser`
  - Detect `---` frontmatter delimiters
  - Parse YAML frontmatter
  - Extract body after frontmatter
  - Handle missing frontmatter (content only)

- [ ] **P2-T3**: Implement `YamlParser`
  - Parse full YAML structure
  - Map fields to `PromptTemplate`
  - Validate required fields

- [ ] **P2-T4**: Implement `JsonParser`
  - Parse full JSON structure
  - Map fields to `PromptTemplate`
  - Validate required fields

- [ ] **P2-T5**: Implement `PlainTextParser`
  - Content becomes prompt content
  - Auto-detect variables from content
  - Generate default name from first line

- [ ] **P2-T6**: Implement format detection
  - `detect_format(path: &Path) -> PromptFormat`
  - Map extensions: `.md`, `.yaml`, `.yml`, `.json`, `.txt`
  - Default to PlainText for unknown

- [ ] **P2-T7**: Implement file loading
  - `parse_file(path: &Path) -> Result<PromptTemplate>`
  - Read file content
  - Handle file not found, permission errors
  - Validate UTF-8 encoding

- [ ] **P2-T8**: Implement stdin loading
  - `parse_stdin() -> Result<String>`
  - Read until EOF

- [ ] **P2-T9**: Export from `src/services/mod.rs`

- [ ] **P2-T10**: Add unit tests
  - Test markdown with frontmatter
  - Test markdown without frontmatter
  - Test YAML parsing
  - Test JSON parsing
  - Test plain text parsing
  - Test format detection
  - Test error cases

### Deliverables

- `src/services/prompt_parser.rs` (~300 lines)
- Modified `src/services/mod.rs` (~5 lines)
- Tests (~150 lines)

### Acceptance Criteria

- [x] Can parse markdown file with YAML frontmatter
- [x] Can parse pure YAML file
- [x] Can parse JSON file
- [x] Can parse plain text (variables auto-detected)
- [x] File extension correctly routes to parser
- [x] Graceful errors for malformed files
- [x] All tests pass

---

## Phase 3: Storage - PromptService & Indexing

**GitHub Issue**: #10
**Objective**: Implement service layer for storing, retrieving, and searching prompts.

### Tasks

- [ ] **P3-T1**: Create `src/services/prompt.rs`
  - Define `PromptService` struct
  - Add `CaptureService` and `RecallService` dependencies

- [ ] **P3-T2**: Implement `save()`
  - Serialize `PromptTemplate` to JSON
  - Create memory with namespace=prompts
  - Store via `CaptureService`
  - Return URN

- [ ] **P3-T3**: Implement `get()`
  - Search by exact name first (indexed)
  - Fall back to recall search
  - Implement domain hierarchy: project → user → org
  - Deserialize `PromptTemplate` from content

- [ ] **P3-T4**: Implement `list()`
  - Query with `PromptFilter`
  - Filter by domain, tags, name pattern
  - Return all matches sorted by domain priority

- [ ] **P3-T5**: Implement `delete()`
  - Require explicit domain
  - Delete from specified domain only
  - Return success boolean

- [ ] **P3-T6**: Implement `search()`
  - Semantic search via `RecallService`
  - Filter to namespace=prompts
  - Deserialize results

- [ ] **P3-T7**: Implement `increment_usage()`
  - Load prompt
  - Increment usage_count
  - Save back

- [ ] **P3-T8**: Define `PromptFilter` struct
  - domain: Option<DomainScope>
  - tags: Vec<String>
  - name_pattern: Option<String>
  - limit: Option<usize>

- [ ] **P3-T9**: Add SQLite index for prompt names
  - Index on JSON-extracted name field
  - Filter to prompts namespace
  - Update index on save

- [ ] **P3-T10**: Export from `src/services/mod.rs`

- [ ] **P3-T11**: Add integration tests
  - Save → get roundtrip
  - Save → list
  - Domain hierarchy search
  - Search by name pattern
  - Delete and verify

### Deliverables

- `src/services/prompt.rs` (~400 lines)
- Modified `src/services/mod.rs` (~5 lines)
- Modified `src/storage/index/sqlite.rs` (~50 lines)
- Tests (~150 lines)

### Acceptance Criteria

- [x] Can save prompt and retrieve by name
- [x] Domain hierarchy search works (project → user → org)
- [x] Semantic search finds relevant prompts
- [x] Exact name lookup is fast (indexed)
- [x] Usage count increments on run
- [x] Delete removes prompt from specified domain
- [x] All tests pass

---

## Phase 4: MCP Integration - Tools & Sampling

**GitHub Issue**: #11
**Objective**: Add MCP tools for prompt management and variable elicitation via sampling.

### Tasks

- [ ] **P4-T1**: Add tool schemas to `src/mcp/tools.rs`
  - `prompt.save` schema
  - `prompt.list` schema
  - `prompt.get` schema
  - `prompt.run` schema
  - `prompt.delete` schema

- [ ] **P4-T2**: Implement `prompt.save` handler
  - Parse input parameters
  - Handle `content` OR `file_path`
  - Call `PromptFileParser` for file input
  - Call `PromptService.save()`
  - Return success with URN

- [ ] **P4-T3**: Implement `prompt.list` handler
  - Parse filter parameters
  - Call `PromptService.list()`
  - Format results

- [ ] **P4-T4**: Implement `prompt.get` handler
  - Parse name and domain
  - Call `PromptService.get()`
  - Return template or not found

- [ ] **P4-T5**: Implement `prompt.run` handler
  - Get prompt by name
  - Extract missing variables
  - For each missing variable:
    - Call `sampling/createMessage`
    - Collect response
  - Substitute variables
  - Return populated content
  - Increment usage count

- [ ] **P4-T6**: Implement `prompt.delete` handler
  - Parse name and domain
  - Call `PromptService.delete()`
  - Return success boolean

- [ ] **P4-T7**: Wire up tools in `src/mcp/server.rs`
  - Register tool schemas
  - Route `call_tool` to handlers

- [ ] **P4-T8**: Extend `PromptRegistry` for user prompts
  - Inject `PromptService` dependency
  - Merge built-in + user prompts in `list_prompts()`
  - Support `prompts/get` for user prompts

- [ ] **P4-T9**: Add integration tests
  - Tool schema validation
  - Save via MCP
  - List includes user prompts
  - Run with elicitation mock
  - Delete via MCP

### Deliverables

- Modified `src/mcp/tools.rs` (~400 lines)
- Modified `src/mcp/server.rs` (~100 lines)
- Modified `src/mcp/prompts.rs` (~50 lines)
- Tests (~150 lines)

### Acceptance Criteria

- [x] All 5 tools registered and callable via MCP
- [x] `prompt.save` accepts content OR file_path
- [x] `prompts/list` includes user prompts
- [x] `prompt.run` elicits missing variables via sampling
- [x] Variable substitution produces correct output
- [x] Domain parameter respected in all tools
- [x] All tests pass

---

## Phase 5: CLI - Subcommands

**GitHub Issue**: #12
**Objective**: Implement CLI subcommands for prompt management.

### Tasks

- [ ] **P5-T1**: Create `src/cli/prompt.rs`
  - Define `PromptCommand` enum with subcommands
  - Implement clap derive macros

- [ ] **P5-T2**: Implement `save` subcommand
  - `--name` (required)
  - `--description`
  - `--tags`
  - `--domain`
  - `--from-file`
  - `--from-stdin`
  - `[CONTENT]` positional

- [ ] **P5-T3**: Implement `list` subcommand
  - `--domain`
  - `--tags`
  - `--format <table|json>`
  - Pretty table output

- [ ] **P5-T4**: Implement `get` subcommand
  - `<NAME>` positional
  - `--domain`
  - `--format <template|json>`

- [ ] **P5-T5**: Implement `run` subcommand
  - `<NAME>` positional
  - `--var <KEY=VALUE>` repeatable
  - `--domain`
  - Interactive prompts with dialoguer

- [ ] **P5-T6**: Implement `delete` subcommand
  - `<NAME>` positional
  - `--domain` (required)
  - `--force`
  - Confirmation prompt

- [ ] **P5-T7**: Implement `export` subcommand
  - `<NAME>` positional
  - `--output`
  - `--format <markdown|yaml|json>`
  - Write to file

- [ ] **P5-T8**: Wire up in `src/main.rs`
  - Add to subcommand enum
  - Route to handler

- [ ] **P5-T9**: Add functional tests
  - CLI save with --from-file
  - CLI save with --from-stdin
  - CLI run with --var
  - CLI export

### Deliverables

- `src/cli/prompt.rs` (~400 lines)
- Modified `src/main.rs` (~20 lines)
- Tests (~100 lines)

### Acceptance Criteria

- [x] All 6 subcommands implemented and working
- [x] `--from-file` loads from file path
- [x] `--from-stdin` and `< file` both work
- [x] Interactive variable prompts work
- [x] `--var` pre-fills variables
- [x] Export produces valid file format
- [x] Help text is clear and complete
- [x] All tests pass

---

## Phase 6: Help & Hooks - AI Guidance

**GitHub Issue**: #13
**Objective**: Provide AI assistants with format guidance when saving prompts.

### Tasks

- [ ] **P6-T1**: Create `src/help/content/prompts.md`
  - Quick reference for formats
  - Variable syntax rules
  - Frontmatter field documentation
  - Variable definition fields
  - Best practices
  - Complete examples

- [ ] **P6-T2**: Add `Prompts` to `HelpCategory` enum
  - Update `src/help/content.rs`
  - Add category description

- [ ] **P6-T3**: Update `HelpIndexService`
  - Index prompts help content at startup
  - Generate stable IDs

- [ ] **P6-T4**: Add `subcog://help/prompts` resource
  - Handle in `read_resource()`
  - Return prompts.md content
  - Set mime type to text/markdown

- [ ] **P6-T5**: Create format validation function
  - `validate_prompt(content: &str) -> ValidationResult`
  - Check for unclosed braces
  - Check for invalid variable names
  - Validate YAML frontmatter if present
  - Return issues with positions

- [ ] **P6-T6**: Update `PostToolUse` hook
  - Detect `prompt.save` tool calls
  - Extract content from tool input
  - Call `validate_prompt()`
  - Inject guidance if issues found
  - Reference `subcog://help/prompts`

- [ ] **P6-T7**: Add unit tests
  - Validation: valid content
  - Validation: unclosed braces
  - Validation: invalid variable names
  - Validation: invalid YAML
  - Hook: guidance injected

### Deliverables

- `src/help/content/prompts.md` (~200 lines)
- Modified `src/help/content.rs` (~10 lines)
- Modified `src/help/indexer.rs` (~20 lines)
- Modified `src/mcp/server.rs` (~20 lines)
- Modified `src/hooks/post_tool_use.rs` (~80 lines)
- Tests (~100 lines)

### Acceptance Criteria

- [x] `subcog://help/prompts` returns format documentation
- [x] Help is indexed and searchable via recall
- [x] PostToolUse hook detects `prompt.save` calls
- [x] Malformed prompts trigger guidance injection
- [x] Validation catches: unclosed braces, invalid YAML, bad variable names
- [x] Guidance includes specific issue locations
- [x] All tests pass

---

## Phase 7: Polish - Validation, Docs & Testing

**GitHub Issue**: #14
**Objective**: Final polish with comprehensive validation, documentation, and test coverage.

### Tasks

- [ ] **P7-T1**: Add comprehensive prompt validation
  - Valid variable names (alphanumeric + underscore)
  - No unclosed braces
  - Valid YAML frontmatter
  - Unique variable names
  - Reserved names check
  - Name format (kebab-case)

- [ ] **P7-T2**: Implement usage count tracking
  - Increment on `prompt.run`
  - Store in prompt metadata
  - Sort by popularity in list

- [ ] **P7-T3**: Verify domain hierarchy search
  - Search order: project → user → org
  - First match wins for get()
  - All matches for list()

- [ ] **P7-T4**: Add performance optimization
  - LRU cache for frequently used prompts
  - Lazy load prompt content
  - Cache variable extraction results

- [ ] **P7-T5**: Improve error messages
  - Clear, actionable text
  - Include fix suggestions
  - Show relevant examples

- [ ] **P7-T6**: Update documentation
  - Update CLAUDE.md with prompt commands
  - Add CLI examples
  - Document MCP tools

- [ ] **P7-T7**: Write comprehensive tests
  - Unit tests for all functions
  - Integration tests for roundtrips
  - Functional tests for CLI
  - Edge case tests

- [ ] **P7-T8**: Performance testing
  - Measure save/get/list times
  - Verify <100ms for common operations

### Deliverables

- Modified various source files (~200 lines)
- Updated documentation (~100 lines)
- Additional tests (~200 lines)

### Acceptance Criteria

- [x] All validation rules enforced
- [x] Usage tracking works correctly
- [x] Domain hierarchy search is correct
- [x] Performance targets met (<100ms common operations)
- [x] Error messages are clear and helpful
- [x] Documentation is complete
- [x] All tests pass (unit, integration, functional)
- [x] `make ci` passes

---

## Execution Strategy

### Recommended Order

1. **Phase 1** (Foundation) - Must be first
2. **Phase 2** (Parsing) - Enables file input
3. **Phase 3** (Storage) - Core functionality
4. **Phase 4** (MCP) and **Phase 5** (CLI) - Can be parallel
5. **Phase 6** (Help & Hooks) - After MCP integration
6. **Phase 7** (Polish) - Final phase

### Parallel Opportunities

- Phase 4 (MCP) and Phase 5 (CLI) are independent after Phase 3
- Tests can be written alongside implementation
- Documentation can be updated incrementally

### Risk Mitigation

1. **Frontmatter parsing edge cases**: Use proven YAML library (serde_yml)
2. **MCP sampling availability**: Graceful fallback to defaults
3. **Performance with many prompts**: SQLite indexing, LRU cache

---

## Progress Tracking

Use the `/claude-spec:implement` command to track progress via PROGRESS.md.

```bash
# Start implementation
/claude-spec:implement 2025-12-30-prompt-management

# Check status
/claude-spec:status 2025-12-30-prompt-management
```
