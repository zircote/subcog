# Requirements: User Prompt Management

**Version**: 1.0
**Status**: Draft
**Last Updated**: 2025-12-30
**GitHub Issues**: #6 (parent), #8-#14 (phases)

## 1. Problem Statement

Users want to save, manage, and run reusable prompts with template variables. When a prompt is invoked, subcog should:

1. Recall the prompt by name or semantic search
2. Identify template variables in the prompt content
3. Elicit variable values from the user via MCP sampling
4. Generate the complete prompt with substituted values
5. Return the populated prompt for the LLM to execute

Prompts should be stored at three domain levels (org, user, project) for efficient reuse and sharing.

## 2. User Stories

### US-1: Save Prompt from File
**As a** developer
**I want to** save a prompt template from a markdown file with YAML frontmatter
**So that** I can version control my prompts alongside my code

**Acceptance Criteria:**
- Can save from `.md` files with YAML frontmatter
- Can save from `.yaml`, `.json`, `.txt` files
- Variables in frontmatter are respected
- Auto-detection of format from extension

### US-2: Save Prompt Inline
**As a** developer
**I want to** quickly save a simple prompt from the command line
**So that** I don't need to create a file for one-liners

**Acceptance Criteria:**
- Can save inline: `subcog prompt save --name "quick" "Generate {{thing}}"`
- Variables are auto-detected from `{{var}}` patterns
- Minimal required arguments (just name + content)

### US-3: Run Prompt with Variable Elicitation
**As a** developer
**I want to** run a saved prompt and have it ask for variable values
**So that** I can reuse templates without remembering exact syntax

**Acceptance Criteria:**
- Running a prompt prompts for each variable
- Variable descriptions (if defined) are shown
- Defaults are applied when not overridden
- Pre-filled variables via `--var key=value` are used

### US-4: MCP Integration
**As an** AI assistant
**I want to** see user prompts in `prompts/list` and run them via MCP
**So that** I can help users leverage their saved prompts

**Acceptance Criteria:**
- User prompts appear alongside built-in prompts
- `prompt.run` uses MCP sampling for variable elicitation
- All prompt tools available via MCP

### US-5: Domain Hierarchy
**As a** team member
**I want to** save prompts at project, user, or org levels
**So that** I can share prompts with my team or keep them personal

**Acceptance Criteria:**
- Prompts can be saved to project, user, or org domains
- Search prioritizes: project → user → org
- Can filter by domain in list/search

### US-6: AI Format Guidance
**As an** AI assistant
**I want to** receive format guidance when saving prompts
**So that** I create valid, well-structured prompt templates

**Acceptance Criteria:**
- Help documentation available at `subcog://help/prompts`
- Hook validation detects malformed prompts
- Guidance injected when issues detected

## 3. Functional Requirements

### FR-1: Namespace
- **FR-1.1**: Add `Namespace::Prompts` as a user-writable namespace
- **FR-1.2**: Prompts namespace appears in namespace list
- **FR-1.3**: Prompts are searchable via standard recall

### FR-2: Template Syntax
- **FR-2.1**: Use Handlebars-style double-brace syntax: `{{variable_name}}`
- **FR-2.2**: Variable names: alphanumeric + underscore, cannot start with number
- **FR-2.3**: Invalid patterns should produce clear errors

### FR-3: File Formats
| Extension | Format | Frontmatter |
|-----------|--------|-------------|
| `.md` | Markdown | Optional YAML frontmatter |
| `.txt` | Plain text | No frontmatter |
| `.yaml`/`.yml` | YAML | Full YAML structure |
| `.json` | JSON | Full JSON structure |

### FR-4: Prompt Model
```
PromptTemplate:
  - name: String (kebab-case, unique)
  - description: String
  - content: String (with {{variable}} placeholders)
  - variables: Vec<PromptVariable>
  - tags: Vec<String>
  - author: Option<String>
  - usage_count: u64
  - created_at: DateTime
  - updated_at: DateTime

PromptVariable:
  - name: String
  - description: Option<String>
  - default: Option<String>
  - required: bool (default: true)
```

### FR-5: MCP Tools
| Tool | Description |
|------|-------------|
| `prompt.save` | Save/update prompt (content OR file_path) |
| `prompt.list` | List prompts with filters |
| `prompt.get` | Get prompt template by name |
| `prompt.run` | Run prompt with variable elicitation |
| `prompt.delete` | Delete prompt by name |

### FR-6: CLI Commands
| Command | Description |
|---------|-------------|
| `subcog prompt save` | Save prompt (--from-file, --from-stdin, or inline) |
| `subcog prompt list` | List prompts |
| `subcog prompt get` | Show prompt template |
| `subcog prompt run` | Run prompt interactively |
| `subcog prompt delete` | Delete prompt |
| `subcog prompt export` | Export to file |

### FR-7: Variable Elicitation
- **FR-7.1**: CLI uses interactive prompts (dialoguer)
- **FR-7.2**: MCP uses `sampling/createMessage` for each variable
- **FR-7.3**: Variable descriptions shown as prompts
- **FR-7.4**: Defaults applied when value not provided

### FR-8: Storage
- **FR-8.1**: Prompts stored as memories in `prompts` namespace
- **FR-8.2**: Serialized as JSON with YAML frontmatter option
- **FR-8.3**: SQLite index for fast name lookup
- **FR-8.4**: Semantic search via existing recall system

### FR-9: Domain Hierarchy
- **FR-9.1**: Three domains: project, user, org
- **FR-9.2**: `get()` returns first match in priority order
- **FR-9.3**: `list()` returns all matches, sorted by domain
- **FR-9.4**: `delete()` requires explicit domain

### FR-10: Help & Hooks
- **FR-10.1**: Create `src/help/content/prompts.md` with format guide
- **FR-10.2**: Add `subcog://help/prompts` MCP resource
- **FR-10.3**: PostToolUse hook validates prompt.save calls
- **FR-10.4**: Inject guidance when format issues detected

## 4. Non-Functional Requirements

### NFR-1: Performance
| Operation | Target |
|-----------|--------|
| Save prompt | <100ms |
| List prompts | <50ms |
| Get by name | <10ms |
| Run (excluding elicitation) | <100ms |
| Variable extraction | <1ms |

### NFR-2: Reliability
- Graceful handling of malformed files
- Clear error messages with fix suggestions
- No data loss on validation failures

### NFR-3: Security
- Content filtering for secrets (like other namespaces)
- No execution of variable values
- Sanitization of file paths

### NFR-4: Usability
- Minimal required arguments
- Sensible defaults
- Clear help text for all commands
- Examples in documentation

## 5. Out of Scope (MVP)

The following are explicitly out of scope for the initial implementation:

1. **Full Handlebars syntax** - Conditionals (`{{#if}}`) and loops (`{{#each}}`)
2. **Versioned prompts** - Git notes already provide history
3. **URL input** - Loading from remote URLs/gists
4. **Prompt sharing** - Publishing to a central registry
5. **Prompt import** - Batch import from directory

## 6. Acceptance Criteria (End-to-End)

- [ ] User can save prompts via CLI: `subcog prompt save --name "my-prompt" "content with {{vars}}"`
- [ ] User can save prompts from file: `subcog prompt save --name "my-prompt" --from-file ./prompt.md`
- [ ] User can save prompts from stdin: `cat prompt.md | subcog prompt save --name "my-prompt" --from-stdin`
- [ ] Supported file formats: `.md`, `.txt`, `.yaml`, `.yml`, `.json`
- [ ] User can save prompts via MCP: `prompt.save` tool with `content` or `file_path`
- [ ] Prompts appear in `prompts/list` MCP response alongside built-in prompts
- [ ] Running a prompt via MCP elicits missing variables using `sampling/createMessage`
- [ ] CLI `subcog prompt run` interactively prompts for variables
- [ ] Prompts are searchable via `subcog recall "keyword" --namespace prompts`
- [ ] Prompts support all three domains (project, user, org)
- [ ] Variable extraction auto-detects `{{variable}}` patterns
- [ ] Prompt metadata includes: name, description, tags, author, timestamps, usage count
- [ ] Help memory exists at `subcog://help/prompts` with format documentation
- [ ] PostToolUse hook validates prompt format and injects guidance for malformed prompts
- [ ] AI assistants receive format guidance when saving prompts via hooks
- [ ] User can export prompts back to file: `subcog prompt export name --output ./file.md`

## 7. Test Plan Summary

### Unit Tests
- Variable extraction regex
- Variable substitution
- YAML frontmatter parsing
- Full YAML/JSON file parsing
- Plain text parsing
- Format validation logic

### Integration Tests
- Save → recall → run roundtrip
- Save from file → recall roundtrip
- Save from stdin → recall roundtrip
- Domain hierarchy search order
- MCP prompts/list includes user prompts
- CLI CRUD operations
- PostToolUse hook injects guidance for malformed prompts

### Functional Tests
- End-to-end prompt workflow via MCP
- Variable elicitation via sampling mock
- Help resource returns format guide
- Export to file produces valid format

## 8. Files to Create/Modify

### New Files
- `src/models/prompt.rs` - PromptTemplate, PromptVariable models
- `src/services/prompt.rs` - PromptService for CRUD + variable handling
- `src/services/prompt_parser.rs` - File format parsers
- `src/cli/prompt.rs` - CLI subcommands
- `src/help/content/prompts.md` - Format documentation

### Modified Files
- `src/models/domain.rs` - Add `Namespace::Prompts`
- `src/models/mod.rs` - Export prompt models
- `src/services/mod.rs` - Export PromptService
- `src/mcp/prompts.rs` - Load user prompts into registry
- `src/mcp/tools.rs` - Add prompt.* tools
- `src/mcp/server.rs` - Wire up tools, add help resource
- `src/main.rs` - Add prompt CLI subcommand
- `src/hooks/post_tool_use.rs` - Add format validation

## 9. Dependencies

### External Crates (Existing)
- `serde`, `serde_json`, `serde_yml` - Serialization
- `regex` - Variable extraction
- `clap` - CLI parsing
- `dialoguer` - Interactive prompts (add if not present)

### Internal Dependencies
- `CaptureService` - Storage backend
- `RecallService` - Search backend
- `HelpIndexService` - Help content indexing
- MCP server infrastructure

## 10. Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Complex frontmatter edge cases | Medium | Low | Comprehensive test suite |
| MCP sampling not available | Low | Medium | Fall back to defaults or skip elicitation |
| Performance with many prompts | Low | Low | SQLite indexing, pagination |
| Variable name conflicts | Low | Low | Validation with clear errors |
