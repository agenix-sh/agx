# agx

![CI](https://github.com/agenix-sh/agx/workflows/CI/badge.svg)
![PR Checks](https://github.com/agenix-sh/agx/workflows/PR%20Checks/badge.svg)
[![codecov](https://codecov.io/gh/agenix-sh/agx/branch/main/graph/badge.svg)](https://codecov.io/gh/agenix-sh/agx)

AGX (`agx`) is the Phase 1 planner CLI in the AGX/AGQ/AGW ecosystem. It shapes natural-language instructions into deterministic JSON plans, persists them locally, and prepares them for submission to the AGQ queue where AGW workers execute each step.

**For comprehensive architecture documentation, see the [AGEniX central repository](https://github.com/agenix-sh/agenix).**

## Installing

Once DNS is configured for `agenix.sh`, you will be able to install or update AGX with:

```sh
curl https://agenix.sh/install.sh | sh
```

This script:
- Detects your OS and architecture.
- Downloads a prebuilt binary from GitHub Releases when available.
- Falls back to building from source with `cargo` if needed.
- Installs `agx` into a standard location (for example `/usr/local/bin` or `$HOME/.local/bin`).

As an alternative, you can install from source with Rust:

```sh
cargo install agx
```

(Until AGX is published on crates.io, you may instead use `cargo install --git https://github.com/agenix-sh/agx.git --locked agx`.)

## Interactive REPL Mode (AGX-042)

**New in Phase 1:** Run `agx` without arguments to enter an interactive REPL for iterative plan crafting:

```bash
agx  # Enter interactive mode
```

The REPL provides:
- **Iterative planning** — Add, edit, and refine plan steps in a conversational session
- **Full editing** — Modify or remove specific steps with `edit <num>` and `remove <num>`
- **Session persistence** — State auto-saves to `~/.agx/repl-state.json` and resumes on next launch
- **Vi mode** — Default vi keybindings (Ctrl-G to enter command mode)
- **Echo model integration** — Uses the fast, conversational Echo model for natural back-and-forth refinement

### REPL Commands

All commands support single-letter shortcuts shown in brackets:

**Plan Building:**
- `[a]dd "<instruction>"` — Generate and append plan steps using Echo model
- `[p]review` — Show current plan
- `[e]dit <num>` — Modify a specific step
- `[r]emove <num>` — Delete a specific step
- `[c]lear` — Reset the plan

**Plan Actions:**
- `[v]alidate` — Run Delta model validation (coming in AGX-045/046)
- `[s]ubmit` — Submit plan to AGQ and get plan-id
- `save` — Manually save session

**Plan Operations:**
- `plan list` — List all stored plans from AGQ
- `plan get <id>` — View details of a specific plan

**Action Operations:**
- `action <plan-id>` — Execute plan (no input)
- `action <plan-id> <json>` — Execute plan with input data

**Operational Commands:**
- `[j]obs` — List all jobs from AGQ
- `[w]orkers` — List active workers
- `stats` / `queue` — Show queue statistics

**Session:**
- `[h]elp` — Show available commands
- `[q]uit` — Exit REPL

**Tip**: Type either the full command or just the first letter (e.g., `a` or `add`)

### Keyboard Shortcuts

- **Ctrl-G** — Enter vi mode for editing
- **Ctrl-C** — Cancel current input
- **Ctrl-D** — Exit REPL

## PLAN workflow (non-interactive)

For scripted workflows, use the traditional `PLAN` subcommands:

**Building Plans:**
1. `PLAN new` — start/reset the persisted plan buffer (defaults to `$TMPDIR/agx-plan.json`, override with `AGX_PLAN_PATH`).
2. `PLAN add "<instruction>"` — capture a natural-language instruction, read STDIN when piped, run the configured planner backend, and append the generated steps to the buffer.
3. `PLAN preview` — pretty-print the current JSON plan so it can be inspected before queueing.
4. `PLAN submit [--json]` — validate the plan and send it to AGQ. Returns the plan-id needed for ACTION submit.

**Viewing Plans in AGQ:**
5. `PLAN list [--json]` — list all stored plans from AGQ.
6. `PLAN get <plan-id>` — view details of a specific plan.

`PLAN add` can be run multiple times to iteratively build a workflow. Structured logs (`--debug`) show the instruction, input summary, tool registry snapshot, and the raw planner JSON to keep the pipeline auditable.

### PLAN submit output

By default, `PLAN submit` displays a human-readable success message with the plan-id:

```bash
$ agx PLAN submit
✅ Plan submitted successfully
   Plan ID: plan_abc123def456
   Tasks: 5

Use with: agx ACTION submit --plan-id plan_abc123def456
         (optional: --input '{...}' or --inputs-file <path>)
```

For machine-readable output, use `--json`:

```bash
$ agx PLAN submit --json
{"plan_id":"plan_abc123def456","job_id":"job_xyz789","task_count":5,"status":"submitted"}
```

### PLAN list and get

After submitting plans to AGQ, you can view and retrieve them:

**List all stored plans:**
```bash
$ agx PLAN list
PLANS (3):
  plan_abc123def456 | 5 tasks | Process log files | 2025-01-19 14:30
  plan_def456ghi789 | 3 tasks | Backup database | 2025-01-19 14:25
  plan_ghi789jkl012 | 2 tasks | (no description) | 2025-01-19 14:20
```

**Machine-readable format:**
```bash
$ agx PLAN list --json
{"plans":[{"plan_id":"plan_abc123def456","description":"Process log files","task_count":5,"created_at":"2025-01-19 14:30"},...]}
```

**Get specific plan details:**
```bash
$ agx PLAN get plan_abc123def456
{
  "plan_id": "plan_abc123def456",
  "plan": {
    "tasks": [
      {"task_number": 1, "command": "grep", "args": ["ERROR", "app.log"]},
      {"task_number": 2, "command": "wc", "args": ["-l"]}
    ]
  }
}
```

## ACTION submit

After creating and storing plans in AGQ, you can execute them with input data using ACTION submit:

**Basic usage:**
```bash
# Submit action with input data
$ agx ACTION submit --plan-id plan_abc123def456 --input '{"path": "/tmp"}'
Action submitted successfully
Job ID: job_xyz789abc012
Plan: plan_abc123def456
Input: {"path":"/tmp"}
Status: queued
```

**Machine-readable format:**
```bash
$ agx ACTION submit --plan-id plan_abc123def456 --input '{"path": "/tmp"}' --json
{"job_id":"job_xyz789abc012","plan_id":"plan_abc123def456","status":"queued"}
```

**Workflow:**
1. Validates the plan-id format (alphanumeric, underscore, dash only)
2. Retrieves the plan from AGQ using PLAN.GET
3. Validates the plan exists (errors if not found)
4. Parses and validates the input JSON
5. Submits the action to AGQ, which creates jobs combining the plan with input data
6. Returns the job-id for tracking execution

**Error handling:**
- Plan not found: `Error: Plan 'plan_xyz' not found`
- Invalid JSON: `Error: Invalid input JSON: <parse error>`
- AGQ connection failure: `Error: Cannot connect to AGQ at <address>: <error>`
- Invalid plan-id: `invalid plan-id: must contain only alphanumeric characters, underscore, or dash`

## CI/CD and Contribution Guide

- The full GitHub Actions matrix (macOS + Linux, build + tests + audit + coverage) is documented in `.github/CICD_SETUP.md`.
- The migration template (`.github/TEMPLATE_FOR_AGX_AGW.md`) explains how AGX/AGW stay in lockstep.
- For workflow configuration details and rollout notes, see `.github/DEPLOYMENT_SUMMARY.md`.
- Every pull request must pass the PR Checks workflow and supply tests (see `AGENTS.md` for the engineering contract).

## AGQ submission

`PLAN submit` sends the current plan to AGQ over RESP:

- `AGQ_ADDR` — TCP address of AGQ (default: `127.0.0.1:6380`)
- `AGQ_SESSION_KEY` — optional session key for AUTH
- `AGQ_TIMEOUT_SECS` — network timeout in seconds (default: 5)

On success, the CLI displays the `plan_id` (needed for ACTION submit) and `task_count`. When using `--json`, it outputs machine-readable JSON with `plan_id`, `job_id`, `task_count`, and `status`.

## Ops mode

Use Ops commands to inspect AGQ without leaving the CLI:

- `JOBS list [--json]`
- `WORKERS list [--json]`
- `QUEUE stats [--json]`

These reuse the same AGQ configuration as PLAN submit. Add `--json` for machine-readable output; otherwise, a simple list is printed.

## Job envelope schema

PLAN submit now wraps the full plan into a job envelope so all steps run on a single worker. See `docs/JOB_SCHEMA.md` for the canonical JSON shape and validation rules (`job_id`, `plan_id`, optional `plan_description`, and `steps[...]` with `input_from_step` and `timeout_secs`).

## Examples

### Interactive REPL Session

```bash
# Enter interactive mode
agx

# In the REPL (using shortcuts):
agx (0)> a "convert PDF to text"
🤖 Generating plan steps...
✓ Added 2 task(s)

agx (2)> p
📋 Current plan (2 tasks):

  1. pdf-to-text input.pdf
  2. save-output output.txt

agx (2)> e 2
Editing task 2:
  Current: save-output output.txt

  New command> write-file output.txt
✓ Updated task 2

agx (2)> h
AGX Interactive REPL v0.1.0

Commands:
  [a]dd <instruction>    Generate and append plan steps
  [p]review              Show current plan
  ...

agx (2)> j
Jobs (3):
  - job_abc123
  - job_def456
  - job_ghi789

agx (2)> w
Active Workers (2):
  - worker-1 (idle)
  - worker-2 (busy)

agx (2)> stats
Queue Statistics:
  pending_jobs: 3
  active_jobs: 1
  completed_jobs: 15
  workers: 2

agx (2)> s
📤 Submitting plan to AGQ...
✅ Plan submitted successfully
   Plan ID: plan_abc123def456
   Tasks: 2

Use with: agx ACTION submit --plan-id plan_abc123def456
         (optional: --input '{...}' or --inputs-file <path>)

agx (2)> plan list
Plans (2):
  plan_abc123def456 (5 tasks) - Process log files
  plan_xyz789ghi012 (3 tasks) - Backup database

agx (2)> plan get plan_abc123def456
Plan: plan_abc123def456
Tasks:
  1. grep ERROR app.log
  2. wc -l
  3. sort
  4. uniq
  5. tee results.txt

agx (2)> action plan_abc123def456 {"path": "/var/log"}
✅ Action submitted successfully
Job ID: job_607bbd71ef8940d5ab53d174fcd6911a
Plan: plan_abc123def456
Input: {"path": "/var/log"}

agx (2)> q
Saving session...
Goodbye!
```

### Non-interactive Workflow

```bash
# start clean
agx PLAN new

# pipe sample data while describing steps
cat data.csv | agx PLAN add "strip header row"
cat data.csv | agx PLAN add "dedupe rows by first three columns"

# inspect the JSON plan buffer
agx PLAN preview

# submit to AGQ
agx PLAN submit
```

For more scenarios, see `EXAMPLES.md`.
