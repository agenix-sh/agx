# AGX Agents Specification

AGX itself is an Agentic Unit (AU) designed to run in CLI environments.  
This document defines the internal agents, their responsibilities in the Phase 1 architecture (AGX planner + AGQ queue + AGW workers), and the contributor workflow expectations.

## 1. Planner Agent (AGX CLI — REPL & PLAN modes)
### Purpose
Transform natural-language user instructions into deterministic JSON workflow plans that AGQ can schedule for AGW workers.

### Modes of Operation

#### Interactive REPL Mode (AGX-042)
Run `agx` without arguments to enter an interactive session for iterative plan crafting:

**REPL Commands:**
- `add "<instruction>"` – Generate and append plan steps using Echo model
- `preview` – Show current plan
- `edit <num>` – Modify a specific step
- `remove <num>` – Delete a specific step
- `clear` – Reset the plan
- `validate` – Run Delta model validation
- `submit` – Submit plan to AGQ
- `save` – Manually save session
- `help` – Show available commands
- `quit` – Exit REPL

**REPL Features:**
- Session persistence to `~/.agx/repl-state.json` (auto-save on quit, auto-resume on launch)
- Vi mode keybindings (Ctrl-G to enter command mode)
- Full plan editing capabilities
- Command history across sessions
- Echo model integration for conversational planning

#### Non-Interactive PLAN Mode
For scripted workflows and CI/CD pipelines:
- `PLAN new` – start/reset the persisted plan buffer
- `PLAN add "<instruction>"` – capture the instruction, run the configured planner backend, and append steps
- `PLAN preview` – pretty-print / lint the in-progress plan
- `PLAN submit` – send the finalized plan to AGQ via RESP with session-key auth and emit machine-readable status
- `PLAN validate` – Run Delta model validation on current plan

#### Ops Commands (both modes)
- `JOBS list [--json]`
- `WORKERS list [--json]`
- `QUEUE stats [--json]`

### Input Context
- Natural-language instructions
- Description of the input stream gathered by AGX
- Tool registry snapshot (Section 5)

### Output Format
```json
{
  "plan": [
    {"cmd": "sort"},
    {"cmd": "uniq"}
  ]
}
```

### Model Backends
- Phase 1: Ollama (`phi3:mini`, `qwen2.5:1.5b`, or `mistral-nemo`)
- Phase 2: embedded llama.cpp fallback
- Future: additional backends via `PlannerConfig`

### AGQ Submission
- `AGQ_ADDR` (default: `127.0.0.1:6380`)
- `AGQ_SESSION_KEY` (optional) for AUTH
- `AGQ_TIMEOUT_SECS` (default: 5)
- On success, `PLAN submit` writes plan submission metadata (job ID, timestamp) next to the plan buffer for later Ops commands.

---

## 2. Ops Agent (AGX CLI — JOBS/WORKERS/QUEUE)
### Purpose
Provide deterministic management commands that inspect AGQ state without leaving the CLI.

### Capabilities
- `JOBS list [--json]` – list queued/running/completed jobs with IDs from AGQ
- `WORKERS list [--json]` – show registered workers, capabilities, and heartbeat status
- `QUEUE stats [--json]` – expose queue depth, scheduling windows, and retry counts
- Shared RESP client with PLAN submit (env vars such as `AGQ_ADDR`, `AGQ_SESSION_KEY`)
- Friendly error handling for unreachable/auth failures

---

## 3. Queue Manager Agent (AGQ)
### Purpose
Accept JSON plans from AGX, persist them in HeroDB (redb), schedule execution, and coordinate workers.

### Responsibilities
- RESP server with session-key authentication
- Plan ingestion and validation
- List/zset queue primitives for ready/scheduled work
- Job metadata persistence (status, retries, timestamps)
- Worker heartbeat tracking and dispatching

---

## 4. Worker Agent (AGW)
### Purpose
Execute plan steps deterministically on local hardware without invoking LLMs.

### Capabilities
- RESP client that blocks on AGQ assignments
- Executes Unix tools and registered AGX-* agent tools
- Captures stdout/stderr, posts results back to AGQ, and reports failures cleanly
- Sends periodic heartbeats so AGQ can reschedule stalled work

---

## 5. Registry Agent
### Purpose
Resolve natural-language intentions to tool capabilities and expose them to the planner.

### Responsibilities
- Maintain catalog of Unix-like and AGX-* tools
- Produce structured metadata:
```json
{
  "tool": "uniq",
  "description": "Remove duplicate lines",
  "patterns": ["dedupe", "unique", "remove duplicates"]
}
```

---

## 6. Future Agents
### Semantic Tool Agent
Wraps AI-based transformations (summarisation, extraction, rewriting).

### MCP Integration Agent
Discovers remote tools via MCP servers.

### Distributed AU Agent
Allows AGX to participate in a cluster of AOA units.

---

## AOA Notes
AGX is a micro-AU with a clear contract:
- PLAN mode shapes instructions into JSON plans.
- Ops mode provides visibility into AGQ state.
- AGQ + AGW guarantee deterministic execution.
- The system can be embedded into larger AOA pipelines.
- Job envelope schema is documented in `docs/JOB_SCHEMA.md` (includes job_id, plan_id, optional plan_description, and steps with input_from_step/timeout).
- Canonical terminology (Task, Plan, Job, Action, Workflow) is defined in `docs/EXECUTION-LAYERS.md`. AGX generates Plans and Actions; AGQ stores Plans and Jobs; AGW executes Tasks as part of Jobs.

---

## GitHub Issue Workflow
This workflow is used to implement GitHub issues for the `agenix-sh/agx` repository:

1. Inspect the issue  
   - Use `gh issue view <number> --repo agenix-sh/agx --json number,title,body,state,url` to read the issue details.
   - **Issue naming**: Use the `AGX-XXX: Title` format (for example, `AGX-030: Implement PLAN subcommands`). Keep the numeric ID stable across PRs/branches.

2. Create a feature branch  
   - From `main`, create a branch named `issue-<number>-<short-slug>`, for example:  
     - `git switch -c issue-1-bootstrap-agx-cli`

3. Implement the change  
   - Make focused changes in the repository to satisfy the issue description.
   - Keep the structure aligned with `ARCHITECTURE.md` and this `AGENTS.md`.

4. Commit the work  
   - Stage files and commit with a message that references the issue number, for example:  
     - `git commit -m "Bootstrap AGX Rust CLI (#1)"`

5. Push the branch and open a PR  
   - Push the branch to GitHub:  
     - `git push -u origin issue-<number>-<short-slug>`  
   - Open a pull request that references the issue:  
     - `gh pr create --repo agenix-sh/agx --head issue-<number>-<short-slug> --fill`

6. Iterate and merge  
   - Address review feedback from both Codex and Claude reviewers; suggestions must be accepted (or discussed to an explicit resolution) before merge.  
   - Merge the pull request when approved, ensuring the issue is linked or closed via the PR.

### Testing Expectations
- **Tests are required for every change** (unit and/or integration depending on scope); untested work should not merge.
- Favor deterministic tests that cover CLI parsing, planner logic, RESP client behaviour, and registry metadata.
- Add regression coverage alongside fixes to prevent repeats.

### Naming & PR Expectations
- **Issues**: `AGX-###: …` (e.g., `AGX-031: Wire PLAN submit to AGQ`). Tool-specific repos should follow the same pattern (for example, `AGX-OCR-00X` once those projects exist).
- **Branches**: `issue-<number>-<slug>` using the GitHub issue number so automation can correlate (e.g., `issue-30-plan-cli` for AGX-030).
- **Pull requests**: Title must match `AGX-###: …` exactly. The PR body should include `## Issue`, `## Security Review`, `## Testing` sections as enforced by PR Checks.

---

## 7. Shared Claude Code Skills & Agents

This repository uses shared Claude Code configuration from the agenix repo (via git submodule at `agenix-shared/.claude/`):

### Available Skills (Auto-Activated)
- **agenix-architecture** - Enforces execution layer nomenclature (Task/Plan/Job/Action/Workflow)
- **agenix-security** - OWASP Top 10, zero-trust principles, constant-time comparisons
- **agenix-testing** - TDD practices, 80% coverage minimum, 100% for security-critical code
- **rust-agenix-standards** - Rust error handling, async patterns, type safety idioms

### Available Agents (Explicit Invocation)
- **rust-engineer** - Deep Rust expertise for async, performance, safety
- **security-auditor** - Vulnerability detection and prevention
- **github-manager** - Issue/PR creation with proper templates and labels
- **multi-repo-coordinator** - Cross-repository change coordination

See `.claude/README.md` for detailed documentation on skill activation and agent usage.
