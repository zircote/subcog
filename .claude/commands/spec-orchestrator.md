---
name: spec-orchestrator
description: Orchestrate implementation of a large specification using parallel agent teams. Reads specs via discovery subagents, synthesizes a task plan, spawns implementation teammates, and manages wave-based execution.
allowed-tools:
  - Task
  - Bash
  - Read
  - Write
  - Glob
  - Grep
  - TaskCreate
  - TaskUpdate
  - TaskList
  - TeamCreate
  - TeamDelete
  - SendMessage
  - AskUserQuestion
argument-hint: "[--auto] [spec-directory]"
---

# Spec Orchestrator Command

You are now running the **spec-orchestrator** procedure. You will execute this step-by-step in the main conversation, keeping the user informed at every phase. Heavy work (reading specs, writing code) is delegated to `Task` subagents. You orchestrate the lifecycle.

## Argument Parsing

Parse `$ARGUMENTS` for flags and the spec directory:

- **`--auto`**: Autonomous mode. Skip all `AskUserQuestion` checkpoints, accept all recommendations as-is, and perform maximum work identified. Log decisions to `/tmp/orchestrator-decisions.md` instead of asking.
- **Spec directory**: Any argument that is not a flag. Defaults to `docs/spec/`.

Examples:
- `/spec-orchestrator` → interactive mode, `docs/spec/`
- `/spec-orchestrator docs/api-spec/` → interactive mode, `docs/api-spec/`
- `/spec-orchestrator --auto` → autonomous mode, `docs/spec/`
- `/spec-orchestrator --auto docs/api-spec/` → autonomous mode, `docs/api-spec/`

Set `AUTO_MODE=true` if `--auto` is present, `false` otherwise.

---

## Phase 0: Bootstrap — Understand the Landscape

Perform lightweight reconnaissance with native tools:

1. **Read conventions**: `Read` CLAUDE.md in full — it defines the source root and all project conventions.
2. **Enumerate spec files**: `Glob` pattern `{spec_dir}/**/*` — group by type (`.md`, `.yaml`, `.json`).
3. **Enumerate source files**: `Glob` with the source root from CLAUDE.md (e.g., `crates/**/*.rs`).
4. **Assess large files**: `Read` with `limit: 5` on potentially large files to gauge scope before partitioning.

From this, build a **partition plan**: group spec files into batches of 3-5, pairing each with the most relevant source directories.

**Key principle**: Each discovery subagent should receive no more than ~40% of its context window in input material.

**Show the partition plan to the user before proceeding.**

---

## Phase 1: Distributed Discovery

### 1.1 Spawn Discovery Subagents

Spawn one `Task` subagent per partition (up to 5 concurrent), using `subagent_type: "general-purpose"`. Each produces a **structured inventory** at `/tmp/discovery/partition-{N}.json`.

**IMPORTANT**: Discovery subagents are fire-and-done `Task` calls with NO `team_name`. They cannot use `SendMessage`. Their output is returned via the Task tool result.

#### Discovery Prompt Template

Give each subagent this directive (customized with their assigned files):

```
You are a **discovery analyst**. Your job is to thoroughly read assigned spec files
and related source code, then produce a structured inventory.

## Your Assigned Files

### Spec files to read (READ EVERY LINE):
- {spec_file_1}
- {spec_file_2}
- {spec_file_3}

### Related existing source to read (for patterns and existing implementations):
- {src_dir_or_files}

### Project conventions (read first):
- CLAUDE.md

## What to Extract

Read every file completely. Do not skim. Then produce a JSON inventory written to
`/tmp/discovery/{partition_name}.json` with this structure:

{
  "partition": "{partition_name}",
  "endpoints": [
    {
      "method": "POST",
      "path": "/api/v1/things",
      "spec_file": "docs/spec/sections/things.md",
      "request_schema": "CreateThingRequest { name: String, ... }",
      "response_schema": "Thing { id: Uuid, ... }",
      "status_codes": [201, 400, 401, 409, 422],
      "error_cases": ["name already exists", "invalid field X"],
      "auth_required": true,
      "pagination": false,
      "notes": "any special behavior, business rules, edge cases"
    }
  ],
  "models": [
    {
      "name": "Thing",
      "spec_file": "docs/spec/sections/things.md",
      "fields": [
        {"name": "id", "type": "Uuid", "constraints": "primary key, auto-generated"},
        {"name": "name", "type": "String", "constraints": "unique, 1-255 chars"}
      ],
      "relationships": ["belongs_to User via user_id"],
      "existing_impl": "src/models/thing.rs (partial, missing field X)",
      "notes": ""
    }
  ],
  "enums": [
    {
      "name": "ThingStatus",
      "variants": ["Active", "Inactive", "Archived"],
      "spec_file": "docs/spec/sections/things.md",
      "existing_impl": "src/models/enums.rs (exists, correct)"
    }
  ],
  "validation_rules": [
    {
      "entity": "Thing",
      "rule": "name must be unique within a workspace",
      "spec_file": "docs/spec/sections/things.md"
    }
  ],
  "business_logic": [
    {
      "description": "When a Thing is archived, cascade soft-delete to child Widgets",
      "spec_file": "docs/spec/sections/things.md",
      "affected_entities": ["Thing", "Widget"]
    }
  ],
  "cross_cutting": [
    {
      "concern": "rate_limiting",
      "details": "POST /things limited to 100/min per user",
      "spec_file": "docs/spec/sections/things.md"
    }
  ],
  "existing_code_notes": [
    "src/models/thing.rs exists but is missing the 'archived_at' field",
    "src/handlers/things.rs has GET implemented but not POST/PUT/DELETE",
    "Error types in src/errors.rs need new variants for Thing-specific errors"
  ],
  "gaps": [
    "Spec mentions WebSocket events for Thing updates but no handler exists",
    "Migration for adding 'archived_at' column needed"
  ]
}

## Rules
- Be EXHAUSTIVE. Every endpoint, every field, every error case, every validation rule.
- Note what ALREADY EXISTS in the codebase and what is MISSING or INCOMPLETE.
- If the spec is ambiguous, note the ambiguity in `notes` — do not guess.
- Do NOT implement anything. Your only output is the inventory JSON file.
- When done, report: the path to your inventory file, a count of endpoints/models/enums
  found, and a brief summary. This is your final output — the orchestrator reads it
  from the Task result.
```

### 1.2 OpenAPI-Specific Discovery

If an OpenAPI spec exists, spawn a dedicated `Task` subagent (`subagent_type: "general-purpose"`) for it (often too large and structurally different to bundle with prose spec files):

```
You are an **OpenAPI analyst**. Read the OpenAPI spec completely and produce
a structured inventory at `/tmp/discovery/openapi.json`.

Extract:
- Every path + method combination with full request/response schemas
- All schema definitions from components/schemas
- All security schemes
- All error response schemas
- Any x-* extensions with behavioral meaning
- Parameter patterns (pagination, filtering, sorting query params)

Use the same JSON structure as other discovery agents but also add:
{
  "openapi_schemas": [...],
  "security_schemes": [...],
  "common_parameters": [...]
}

When done, report: the path to your inventory file, total paths/schemas found,
and a brief summary.
```

### 1.3 Schema-Specific Discovery

If there are standalone JSON schemas, spawn a `Task` subagent (`subagent_type: "general-purpose"`):

```
You are a **schema analyst**. Read the JSON schema completely.
Produce inventory at `/tmp/discovery/schema.json`.

Extract every type definition, property, constraint, $ref resolution, enum,
required field, and validation pattern. Map each to the Rust type it should become.

When done, report: the path to your inventory file, total types found, and a brief summary.
```

### 1.4 Collect & Validate Discovery

After all discovery `Task` subagents return their results, use `Glob` pattern `/tmp/discovery/*.json` to verify all inventory files exist. Read only the inventory JSON files — do NOT re-read the original spec files.

### 1.5 User Checkpoint

Present a summary of discovery results (endpoint count, model count, gaps).

- **Interactive mode** (`AUTO_MODE=false`): Use `AskUserQuestion` to ask if discovery looks complete or if additional areas need investigation. Do NOT proceed to Phase 2 until the user confirms.
- **Autonomous mode** (`AUTO_MODE=true`): Log the summary to `/tmp/orchestrator-decisions.md` with the heading `## Phase 1: Discovery Results — Auto-Accepted`. Proceed immediately to Phase 2.

---

## Phase 2: Synthesis — Build the Master Task Plan

With all inventories loaded, synthesize the complete task plan.
**This phase is planning only — do NOT call `TaskCreate` yet.**

### 2.1 Merge Inventories

**CRITICAL: Use `jq` to process inventory JSON files instead of reading them into context.**
Discovery inventories can be large. Reading them all into the context window risks exhaustion.
Use `jq` via `Bash` to extract, merge, and summarize without loading raw JSON into context.

#### Step 1: Get aggregate counts (zero context cost)

```bash
# Count endpoints, models, enums across all partitions
jq -s '{
  total_endpoints: [.[].endpoints // [] | length] | add,
  total_models: [.[].models // [] | length] | add,
  total_enums: [.[].enums // [] | length] | add,
  total_validation_rules: [.[].validation_rules // [] | length] | add,
  total_business_logic: [.[].business_logic // [] | length] | add,
  total_gaps: [.[].gaps // [] | length] | add,
  partitions: [.[].partition]
}' /tmp/discovery/*.json
```

#### Step 2: Extract deduplicated entity lists

```bash
# Unique model names across all partitions
jq -s '[.[].models // [] | .[].name] | unique | .[]' /tmp/discovery/*.json

# Unique endpoint paths with methods
jq -s '[.[].endpoints // [] | .[] | "\(.method) \(.path)"] | unique | sort | .[]' /tmp/discovery/*.json

# All gaps aggregated
jq -s '[.[].gaps // [] | .[]] | unique | .[]' /tmp/discovery/*.json
```

#### Step 3: Build merged inventory file (for task generation)

```bash
# Merge all partitions into a single deduplicated inventory
jq -s '{
  endpoints: [.[].endpoints // [] | .[]] | unique_by(.method + .path),
  models: [.[].models // [] | .[]] | unique_by(.name),
  enums: [.[].enums // [] | .[]] | unique_by(.name),
  validation_rules: [.[].validation_rules // [] | .[]],
  business_logic: [.[].business_logic // [] | .[]],
  cross_cutting: [.[].cross_cutting // [] | .[]],
  existing_code_notes: [.[].existing_code_notes // [] | .[]],
  gaps: [.[].gaps // [] | .[]] | unique
}' /tmp/discovery/*.json > /tmp/discovery/merged.json
```

#### Step 4: Generate task candidates per phase

```bash
# Phase A candidates: shared types, enums, error variants
jq '[.enums[] | {phase: "A", subject: "Define \(.name) enum", spec_file: .spec_file, existing: .existing_impl}]' /tmp/discovery/merged.json

# Phase B candidates: one task per model
jq '[.models[] | {phase: "B", subject: "Implement \(.name) model", fields: [.fields[].name], spec_file: .spec_file, existing: .existing_impl}]' /tmp/discovery/merged.json

# Phase C candidates: repository/data layer per model
jq '[.models[] | {phase: "C", subject: "Implement \(.name) repository and queries", spec_file: .spec_file, relationships: .relationships}]' /tmp/discovery/merged.json

# Phase D candidates: one task per endpoint
jq '[.endpoints[] | {phase: "D", subject: "\(.method) \(.path)", status_codes: .status_codes, error_cases: .error_cases, spec_file: .spec_file}]' /tmp/discovery/merged.json

# Phase E candidates: business logic and cross-entity workflows
jq '[.business_logic[] | {phase: "E", subject: "Implement: \(.description)", affected_entities: .affected_entities, spec_file: .spec_file}]' /tmp/discovery/merged.json

# Phase F candidates: auth, middleware, cross-cutting concerns
jq '[.cross_cutting[] | {phase: "F", subject: "Implement \(.concern)", details: .details, spec_file: .spec_file}]' /tmp/discovery/merged.json

# Phase G candidates: integration tests per endpoint
jq '[.endpoints[] | {phase: "G", subject: "Integration tests for \(.method) \(.path)", error_cases: .error_cases, spec_file: .spec_file}]' /tmp/discovery/merged.json

# Phase H: polish tasks are static (not derived from inventory)
# - Clippy clean, fmt, doc comments, final `just check`, missing coverage
```

#### Step 5: Read only what you need

After `jq` extracts structured summaries, read only specific sections into context:
- Read the **counts** output (Step 1) to understand scope
- Read the **gaps** output (Step 3) to identify missing coverage
- Read individual model/endpoint details from `merged.json` using targeted `jq` queries **only when writing specific task descriptions**

**NEVER read `/tmp/discovery/*.json` files directly with the `Read` tool.** Always use `jq` to extract the specific fields needed.

#### Merging rules

- **Deduplicate**: Same model referenced in multiple partitions → merge into one entry
- **Resolve cross-references**: Endpoint X references Model Y from a different partition
- **Identify gaps**: Any spec area not covered by discovery? If so, spawn a follow-up discovery subagent for just that area.
- **Catalog existing code**: What's done, what's partial, what's missing entirely?

### 2.2 Design Task Breakdown

Decompose into tasks following this phase structure. Adapt phases as needed, but maintain the dependency ordering:

#### Phase A: Foundation
- Project structure / directory scaffolding (if needed)
- Shared error types and error response formatting
- Shared types: enums, common structs, newtypes
- Configuration / environment
- Database migrations for all new/modified tables

#### Phase B: Core Models
- One task per model (struct definition, Display/Debug, serde, builder if applicable)
- Validation logic per model
- Model tests

#### Phase C: Data Layer
- Repository traits per domain area
- Database query implementations (one task per entity's CRUD)
- Query tests with test fixtures

#### Phase D: API Handlers
- One task per endpoint (or tightly coupled endpoint group like CRUD for one entity)
- Request parsing, response formatting
- Handler-level error mapping

#### Phase E: Business Logic / Services
- Service layer for complex operations
- Cross-entity workflows
- Event/notification triggers

#### Phase F: Auth & Middleware
- Authentication middleware
- Authorization / permission checks per endpoint
- Rate limiting
- CORS, logging, request ID propagation

#### Phase G: Integration Tests
- One task per endpoint or endpoint group
- Happy path + every error case from the spec
- Edge cases identified during discovery

#### Phase H: Polish
- Clippy clean, fmt, doc comments
- Final `just check` pass
- Missing test coverage

### 2.3 Task Format

Plan each task with this structure:

```
subject: "Implement Thing model with validation"
description: |
  ## What
  Implement the `Thing` struct and its validation logic per the spec.

  ## Spec Reference
  - docs/spec/sections/things.md (Thing model section)
  - OpenAPI: #/components/schemas/Thing

  ## Fields
  - id: Uuid (auto-generated)
  - name: String (1-255 chars, unique per workspace)
  - status: ThingStatus enum
  - created_at: DateTime<Utc>
  - updated_at: DateTime<Utc>
  - archived_at: Option<DateTime<Utc>>

  ## Acceptance Criteria
  - [ ] Struct defined with all fields
  - [ ] serde Serialize/Deserialize derived
  - [ ] Builder pattern per CLAUDE.md conventions
  - [ ] Validation: name length, uniqueness constraint annotation
  - [ ] Unit tests for builder and validation
  - [ ] File: src/models/thing.rs

  ## Existing Code
  - src/models/thing.rs exists but missing archived_at field — extend it

  ## Convention Reminders
  - Use thiserror for error types
  - Follow builder pattern from CLAUDE.md
  - Run `cargo clippy` and `cargo fmt` before completing
activeForm: "Implementing Thing model"
blockedBy: [Phase A task IDs]
```

**Critical**: Include enough context in each task description that the implementing teammate does NOT need to read the full spec — only the specific files referenced.

### 2.4 Write the Task Manifest

Use `Write` to save the full plan to `/tmp/task-manifest.md` as an audit trail for spec coverage.

Structure:

```markdown
# Task Manifest

## Spec Coverage Audit
| Spec Section | Endpoints Covered | Models Covered | Tasks |
|---|---|---|---|
| things.md | POST/GET/PUT/DELETE /things | Thing, ThingStatus | T-B01, T-C01, T-D01-04 |
| ... | ... | ... | ... |

## Uncovered Items
(anything from discovery inventories not yet assigned to a task — should be empty)

## Task List
### Phase A: Foundation
- T-A01: ... (blocked by: none)
- T-A02: ... (blocked by: none)
### Phase B: Core Models
- T-B01: ... (blocked by: T-A01)
...
```

### 2.5 User Checkpoint — Plan Approval

Present the task breakdown with dependency graph and spec coverage audit.

- **Interactive mode** (`AUTO_MODE=false`): Use `AskUserQuestion` to ask the user to approve the plan before any code is written. Do NOT proceed to Phase 3 until the user explicitly approves.
- **Autonomous mode** (`AUTO_MODE=true`): Log the full task manifest to `/tmp/orchestrator-decisions.md` with the heading `## Phase 2: Task Plan — Auto-Accepted`. Proceed immediately to Phase 3.

---

## Phase 3: Execution

### 3.1 Create Team

**MANDATORY FIRST STEP**: Create the team BEFORE creating any tasks. Tasks created without a team context land in the wrong task list and teammates cannot see them.

```
TeamCreate:
  team_name: "spec-impl"
  description: "Specification implementation team"
```

### 3.2 Create ALL Tasks

**After the team exists**, create every task from the manifest using `TaskCreate`. Tasks are now automatically associated with the team's task list.

1. Call `TaskCreate` for every task in the manifest (all phases A through H). Each task MUST have `subject`, `description`, and `activeForm`.
2. Call `TaskUpdate` with `addBlockedBy` to set ALL dependency relationships (e.g., all Phase B tasks blocked by relevant Phase A tasks).
3. Call `TaskList` to verify all tasks exist with correct dependencies.

**Do NOT proceed to 3.3 until every task is registered and all dependencies are set.**

### 3.3 Spawn Teammates

Spawn teammates in PARALLEL (single response turn).

Determine how many parallel teammates you need for the current wave. Spawn them ALL in one response using multiple `Task` calls simultaneously.

**CRITICAL requirements for teammate spawning:**

1. Every teammate MUST use `subagent_type: "general-purpose"`. Custom agent types (e.g., `rust-developer`) are **broken as teammates** — they do not respond to `SendMessage`, do not claim tasks from `TaskList`, and go permanently idle after spawn. This is a known limitation: custom agents defined in `.claude/agents/` have conflicting instructions that override the teammate prompt. Only `general-purpose` (Tools: `*`, no conflicting agent instructions) works reliably as a teammate.

2. Every teammate MUST be spawned with `run_in_background: true`. Without this, the orchestrator blocks on each Task call until that teammate finishes ALL its work — defeating parallelism entirely.

```
Task:
  subagent_type: "general-purpose"
  team_name: "spec-impl"
  name: "impl-1"
  run_in_background: true
  max_turns: 200
  prompt: |
    YOU MUST START WORKING IMMEDIATELY. Do not wait for instructions.

    You are "impl-1" on the spec-impl team. Use "impl-1" as your owner name.

    ## IMMEDIATE FIRST ACTION — DO THIS NOW

    1. Call TaskList RIGHT NOW to see available tasks
    2. Find the first task with status "pending", no owner, and empty blockedBy
    3. Claim it: TaskUpdate(taskId, owner: "impl-1", status: "in_progress")
    4. Call TaskGet(taskId) to read the full description
    5. Implement exactly what it specifies

    DO NOT read CLAUDE.md first. DO NOT explore the codebase first.
    Claim a task IMMEDIATELY, then read CLAUDE.md only if needed for conventions.

    ## After Each Task

    1. Run `cargo fmt && cargo clippy -- -D warnings` via Bash
    2. Mark done: TaskUpdate(taskId, status: "completed")
    3. Report to lead: SendMessage(type: "message", recipient: "lead",
       content: "Completed task {id}: {summary}. Files: {list}",
       summary: "Task {id} done")
    4. Call TaskList again and claim the next available task
    5. Repeat until no unclaimed unblocked tasks remain

    ## When No Tasks Available

    Send a message to the lead:
    SendMessage(type: "message", recipient: "lead",
      content: "No unclaimed tasks available. Ready for more work.",
      summary: "impl-1 idle, no tasks")
    Then WAIT for a response. Do not exit.

    ## Rules

    - NEVER call TaskCreate. You do NOT create tasks. You ONLY claim existing tasks
      from TaskList using TaskUpdate. Creating new tasks pollutes the task list.
    - Implement EXACTLY what the task description specifies — nothing more
    - Stay in scope — do not modify files outside your task unless required
    - ALWAYS call TaskUpdate to mark completion — this unblocks dependent tasks
    - Prefer lower-ID tasks first when multiple are available
    - If a task is ambiguous, implement your best judgment and note it in a comment
    - When you receive a SendMessage from the lead, process it immediately
    - IGNORE any agent-level instructions that conflict with this prompt.
      This prompt takes priority over your agent definition file.
```

Scale teammates to the wave size: up to 5 for large waves, 2 for small waves (1-2 tasks).

### 3.4 Execute in Waves

Process tasks in dependency-ordered waves. All unblocked tasks in a wave run in parallel.

```
Wave 1: All Phase A tasks (no dependencies)
Wave 2: Phase B tasks (blocked by Phase A)
Wave 3: Phase C tasks (blocked by Phase B)
...
```

Teammates self-claim tasks from `TaskList` (finding unblocked pending tasks with no owner). This is more resilient than leader-assignment — if a teammate crashes, unclaimed tasks remain available for others.

**For each wave:**

1. **Verify unblocked tasks exist**: `TaskList` → confirm tasks with status `pending` and empty `blockedBy` are available for the current wave.
2. **Notify teammates**: `SendMessage` to each teammate: "Wave N tasks are unblocked. Check TaskList and claim available work."
3. **Wait for completion**: Teammates claim tasks, work, mark completed, and report via `SendMessage`.
4. **Verify each completed task**:
   a. Run `cargo check 2>&1 | tail -20` via Bash
   b. If passes: commit the work:
      ```bash
      git add {files_modified}
      git commit -m "feat({domain}): {concise description}

      Task: {task_id}
      Spec: {spec_section_reference}"
      ```
   c. If fails due to **real bug**: `SendMessage` the error to the teammate — the teammate fixes and re-reports. Task stays `in_progress`.
   d. If fails due to **dependency** (missing type from incomplete task): skip for now, resolves when the blocking task completes.
5. **Next wave**: After all wave tasks show `completed` in `TaskList`, newly unblocked tasks become available → teammates self-claim from the next wave automatically.

### 3.5 Integration Checkpoints

After each phase completes, run `just check`. Fix any issues via teammate fix tasks before proceeding to the next phase.

---

## Phase 4: Final Verification

After all execution tasks complete (confirm via `TaskList` — all tasks show `completed`):

### 4.1 Full Test Suite

```bash
cargo test --all 2>&1
```

### 4.2 Spec Coverage Audit

Spawn a **verification** `Task` subagent (`subagent_type: "general-purpose"`, no `team_name` — fire-and-done audit):

```
You are an **audit analyst**. Verify the implementation fully covers the specification.

Read these via `Read` (use `Glob` to enumerate files first):
- /tmp/task-manifest.md (the task plan)
- /tmp/discovery/*.json (the spec inventories)
- Source files under the project's source root (check CLAUDE.md — may be `crates/`, not `src/`)

For every endpoint: verify handler exists, tests exist, error cases are handled.
For every model: verify struct has all fields and validation logic exists.

Produce a coverage report at /tmp/audit-report.md (covered, missing, partial items
with file references). Report the summary as your final output.
```

### 4.3 Address Gaps

If the audit reveals gaps, create new tasks via `TaskCreate`. Teammates will self-claim from `TaskList` when notified via `SendMessage`.

### 4.4 Final Commit

```bash
git add {files_modified}
git commit -m "feat: complete specification implementation

Implements all endpoints, models, validation, and tests per spec.
See /tmp/task-manifest.md for full task breakdown."
```

---

## Phase 5: Cleanup

### 5.1 Shutdown Teammates

Send `shutdown_request` to each teammate:

```
SendMessage:
  type: "shutdown_request"
  recipient: "impl-1"
  content: "All tasks complete. Shutting down."
```

Repeat for each teammate (impl-2, impl-3, etc.).

### 5.2 Wait for Shutdown Confirmations

Wait for all teammates to confirm shutdown.

### 5.3 Delete Team

Call `TeamDelete` to clean up team and task resources.

### 5.4 Report Final Summary

Present to the user:
- Total tasks completed
- Files created/modified
- Test results
- Any unresolved issues or ambiguities logged in `/tmp/issues/`

---

## Orchestrator Rules

1. **Never read raw spec files yourself** — delegate to discovery subagents.
2. **Never write code yourself** — delegate to implementation teammates.
3. **Structured data over prose** — discovery produces JSON, not summaries.
4. **Fail fast, fix fast** — resolve issues before moving to dependent tasks.
5. **Commit after every task** — atomic commits enable rollback and show progress.
6. **Audit relentlessly** — trust but verify; the final phase catches what earlier phases miss.
7. **Parallelize within waves, serialize across waves** — spawn teammates with `run_in_background: true` in a single response turn for actual parallelism.
8. **Context budget** — give each subagent or teammate only CLAUDE.md, the relevant spec files, the relevant source files, and a self-contained task description.
9. **Task lifecycle is MANDATORY** — `TaskCreate` → teammate claims via `TaskUpdate(owner + in_progress)` → work → verify → `TaskUpdate(completed)`. Never skip status updates; they unblock dependent tasks.
10. **Team before tasks, tasks before teammates** — Phase 3.1 (TeamCreate) → 3.2 (TaskCreate) → 3.3 (spawn teammates). Tasks created without a team land in the wrong list. No exceptions.
11. **Teammates MUST use `subagent_type: "general-purpose"`** — Custom agent types (e.g., `rust-developer`) do not function as teammates. They go permanently idle after spawn, ignore `SendMessage`, and don't claim tasks. Only `general-purpose` works reliably as a teammate because it has all tools and no conflicting agent-level instructions.
12. **Teammates self-claim tasks** — Teammates find unblocked unclaimed tasks via `TaskList` and claim them with `TaskUpdate(owner)`. This is more resilient than leader-assignment.
13. **Clean shutdown** — Always send `shutdown_request` to all teammates and call `TeamDelete` when done.
14. **User checkpoints are mandatory in interactive mode** — `AskUserQuestion` gates between discovery→synthesis and synthesis→execution. In `--auto` mode, checkpoints are logged to `/tmp/orchestrator-decisions.md` and auto-accepted. The user can review decisions after completion.

---

## Troubleshooting

**Context overflow in discovery** — Use `Grep` to find section boundaries (`'^##'` for Markdown, `'paths:'` for OpenAPI), split the file across multiple `Task` subagents, then merge their partial inventories.

**Teammate failure** — Reassign with narrower scope via `SendMessage`. For implementation failures, dispatch a fix task to another teammate with the error message, affected files, and original task description. Update task status via `TaskUpdate` to reflect retries.

**`cargo check` failure after a task** — If caused by an incomplete dependency, note it on the blocked task via `TaskUpdate` and continue with other unblocked work. If it is a real error in the just-completed task, dispatch a fix immediately; do not mark the task `completed` until the fix lands.

**Teammate goes idle immediately after spawn** — The teammate prompt may not be directive enough. The prompt MUST start with an imperative action ("Call TaskList RIGHT NOW") not a description of the workflow. Teammates that receive descriptive prompts ("Your workflow is...") may wait for further instructions instead of acting. Also ensure `max_turns` is set high enough (200+) so teammates don't exhaust their turn budget mid-task.

**Teammate goes idle between tasks** — This is normal. Teammates go idle between turns. Send them a new `SendMessage` to wake them up with new work. Do NOT treat idle as an error or spawn a replacement.

**Discovery subagent can't SendMessage** — This is by design. Fire-and-done `Task` subagents (no `team_name`) communicate via their Task return value, not `SendMessage`. Only team-member teammates (spawned with `team_name`) can use `SendMessage`.

**Teammates not responding / not claiming tasks** — Three possible causes:
1. **Wrong agent type**: Custom agents (e.g., `rust-developer`) do not work as teammates. Always use `subagent_type: "general-purpose"`.
2. **Tasks in wrong namespace**: Tasks created before `TeamCreate` land in the default task list, not the team's. Always create the team FIRST.
3. **Passive prompt**: The teammate prompt must command immediate action, not describe a workflow. Start with "Call TaskList RIGHT NOW" not "Your workflow is to check TaskList."

**No parallelism despite multiple teammates** — Verify every teammate `Task` call includes `run_in_background: true`. Without it, the orchestrator blocks on each spawn until that teammate finishes, making execution sequential.
