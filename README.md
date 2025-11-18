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

- `add "<instruction>"` — Generate and append plan steps using Echo model
- `preview` — Show current plan
- `edit <num>` — Modify a specific step
- `remove <num>` — Delete a specific step
- `clear` — Reset the plan
- `validate` — Run Delta model validation (coming in AGX-045/046)
- `submit` — Submit plan to AGQ (use `agx PLAN submit` for now)
- `save` — Manually save session
- `help` — Show available commands
- `quit` — Exit REPL

### Keyboard Shortcuts

- **Ctrl-G** — Enter vi mode for editing
- **Ctrl-C** — Cancel current input
- **Ctrl-D** — Exit REPL

## PLAN workflow (non-interactive)

For scripted workflows, use the traditional `PLAN` subcommands:

1. `PLAN new` — start/reset the persisted plan buffer (defaults to `$TMPDIR/agx-plan.json`, override with `AGX_PLAN_PATH`).
2. `PLAN add "<instruction>"` — capture a natural-language instruction, read STDIN when piped, run the configured planner backend, and append the generated steps to the buffer.
3. `PLAN preview` — pretty-print the current JSON plan so it can be inspected before queueing.
4. `PLAN submit` — validate the plan and send it to AGQ.

`PLAN add` can be run multiple times to iteratively build a workflow. Structured logs (`--debug`) show the instruction, input summary, tool registry snapshot, and the raw planner JSON to keep the pipeline auditable.

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

On success, the CLI prints JSON including the `job_id` and writes metadata alongside the plan buffer for future Ops commands.

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

# In the REPL:
agx (0)> add "convert PDF to text"
🤖 Generating plan steps...
✓ Added 2 task(s)

agx (2)> preview
📋 Current plan (2 tasks):

  1. pdf-to-text input.pdf
  2. save-output output.txt

agx (2)> edit 2
Editing task 2:
  Current: save-output output.txt

  New command> write-file output.txt
✓ Updated task 2

agx (2)> submit
📤 Submitting plan to AGQ...
⚠️  Submit via REPL not yet fully integrated
   Use 'agx PLAN submit' for now

agx (2)> quit
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
