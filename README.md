# agx

![CI](https://github.com/agenix-sh/agx/workflows/CI/badge.svg)
![PR Checks](https://github.com/agenix-sh/agx/workflows/PR%20Checks/badge.svg)
[![codecov](https://codecov.io/gh/agenix-sh/agx/branch/main/graph/badge.svg)](https://codecov.io/gh/agenix-sh/agx)

AGX (`agx`) is the Phase 1 planner CLI in the AGX/AGQ/AGW ecosystem. It shapes natural-language instructions into deterministic JSON plans, persists them locally, and prepares them for submission to the AGQ queue where AGW workers execute each step.

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

## PLAN workflow

Phase 1 introduces a `PLAN` REPL-style workflow:

1. `PLAN new` — start/reset the persisted plan buffer (defaults to `$TMPDIR/agx-plan.json`, override with `AGX_PLAN_PATH`).
2. `PLAN add "<instruction>"` — capture a natural-language instruction, read STDIN when piped, run the configured planner backend (Ollama today), and append the generated steps to the buffer.
3. `PLAN preview` — pretty-print the current JSON plan so it can be inspected before queueing.
4. `PLAN submit` — validate the plan and (in upcoming work) send it to AGQ. For now, it emits the plan JSON and a placeholder status message.

`PLAN add` can be run multiple times to iteratively build a workflow. Structured logs (`--debug`) show the instruction, input summary, tool registry snapshot, and the raw planner JSON to keep the pipeline auditable.

## CI/CD and Contribution Guide

- The full GitHub Actions matrix (macOS + Linux, build + tests + audit + coverage) is documented in `.github/CICD_SETUP.md`.
- The migration template (`.github/TEMPLATE_FOR_AGX_AGW.md`) explains how AGX/AGW stay in lockstep.
- For workflow configuration details and rollout notes, see `.github/DEPLOYMENT_SUMMARY.md`.
- Every pull request must pass the PR Checks workflow and supply tests (see `AGENTS.md` for the engineering contract).

## Examples

```bash
# start clean
agx PLAN new

# pipe sample data while describing steps
cat data.csv | agx PLAN add "strip header row"
cat data.csv | agx PLAN add "dedupe rows by first three columns"

# inspect the JSON plan buffer
agx PLAN preview

# placeholder submission (AGQ wiring tracked in issue #31)
agx PLAN submit
```

For more scenarios, see `EXAMPLES.md`.
