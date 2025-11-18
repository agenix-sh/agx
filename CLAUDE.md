# AGX Development Guidelines

**Repository:** AGX (Planner CLI)
**Role:** Transform natural-language instructions into deterministic JSON plans using local LLMs

## Quick Context

AGX is the **planner** in the AGEniX ecosystem:
- **AGX** (this repo) → Plans creation and orchestration
- **AGQ** → Queue management, job scheduling
- **AGW** → Deterministic task execution (no LLM)

## Key Principles

1. **Planner Only** - AGX generates plans, never executes them (that's AGW's job)
2. **Local-First** - No cloud dependencies, pure Rust binaries
3. **Zero-Trust LLM Output** - Validate and sanitize all LLM-generated content
4. **Type-Safe Plans** - Use execution layer types: Task → Plan → Job → Action → Workflow

## Architecture Layers

```
User Input → Echo (fast planning) → Delta (validation) → Plan (JSON) → AGQ (queue)
```

**Echo Model:** Fast, conversational planning (VibeThinker-1.5B)
**Delta Model:** Thorough validation and tool verification (VibeThinker-1.5B)

## Critical Files

- `src/planner/` - LLM integration and plan generation
- `src/protocol/` - RESP client for AGQ communication
- `src/cli/` - REPL interface and command parsing
- `specs/job-envelope.md` - Plan → Job transformation

## Execution Layer Nomenclature

**Always use these terms consistently:**
- **Task** - Atomic unit of work (user-facing description)
- **Plan** - Collection of tasks (LLM-generated)
- **Job** - Executable plan submitted to AGQ (with metadata)
- **Action** - Low-level worker instruction (AGW executes these)
- **Workflow** - Multi-job orchestration (Phase 2+)

**Never say:** "step", "instruction", "command" (these are ambiguous)

## Shared Configuration

This repo uses shared `.claude/` configuration via git submodule:

**Skills (auto-activated):**
- `agenix-architecture` - Execution layers, zero-trust principles
- `agenix-security` - OWASP Top 10, input validation
- `agenix-testing` - TDD, 80% coverage requirement
- `rust-agenix-standards` - Error handling, async patterns

**Reference central docs when needed:**
- Architecture: `agenix-shared/docs/architecture/`
- Security: `agenix-shared/docs/development/security-guidelines.md`
- Testing: `agenix-shared/docs/development/testing-strategy.md`

## Development Workflow

### Before Writing Code

1. Check if a skill applies (architecture, security, testing, rust-standards)
2. Read relevant issue for acceptance criteria
3. Write tests first (TDD)

### Code Standards

```rust
// ✅ Use anyhow for application code
use anyhow::{Context, Result};

fn load_model(path: &Path) -> Result<Model> {
    std::fs::read(path)
        .context(format!("Failed to load model from {:?}", path))?
}

// ✅ Use thiserror for library errors
#[derive(Error, Debug)]
pub enum PlannerError {
    #[error("Invalid plan: {0}")]
    InvalidPlan(String),
}

// ✅ Never panic in production
// ❌ Don't use: unwrap(), expect(), panic!(), unreachable!()

// ✅ Use async/await with Tokio
#[tokio::main]
async fn main() -> Result<()> {
    // ...
}
```

### Testing Requirements

- **80% minimum coverage** (100% for security-critical code)
- **TDD approach** - Write tests before implementation
- **Unit tests** - `#[cfg(test)]` modules in same file
- **Integration tests** - `tests/` directory for end-to-end flows

### Security Checklist

- [ ] No command injection (validate all shell commands)
- [ ] Input validation (sanitize user input before LLM prompts)
- [ ] No secrets in logs or error messages
- [ ] Constant-time comparison for session keys
- [ ] Path traversal prevention (validate file paths)

## Current Phase

**Phase 1 (In Progress):**
- ✅ Basic REPL interface
- ✅ RESP protocol client
- ✅ Model backend abstraction (AGX-022)
- ✅ Qwen2/VibeThinker architecture support (AGX-049)
- 🚧 Echo model prompts (AGX-045)
- 🚧 Delta model prompts (AGX-046)

**Blocked until Phase 1 complete:**
- Agentic Units (AU) integration
- Multi-job workflows
- Distributed planning

## Common Tasks

### Adding a New LLM Backend

1. Implement `ModelBackend` trait in `src/planner/backend/`
2. Add backend selection logic in config
3. Write contract tests for trait compliance
4. Document in `AGENTS.md`

### Modifying Plan Schema

1. **STOP** - Schemas are canonical in `agenix/specs/` directory
2. Read `agenix/specs/README.md` for Plan vs Job distinction
3. Changes require cross-repo coordination (agx, agq, agw)
4. Use multi-repo-coordinator agent for planning

### Adding RESP Commands

1. Check AGQ spec in `agenix/docs/api/resp-protocol.md`
2. Implement in `src/protocol/client.rs`
3. Add integration test with mock AGQ
4. Update `AGENTS.md` command reference

## Git Workflow

```bash
# Create feature branch
git checkout -b feat/echo-model-integration

# Make changes with TDD
# Write test → Make it pass → Refactor

# Commit with conventional commits
git commit -m "feat(planner): add Echo model integration

Implement conversational planning with VibeThinker-1.5B.
Uses temperature=0.7 for creative, human-readable plans.

Refs: AGX-045"

# Push and create PR
git push -u origin feat/echo-model-integration
gh pr create --title "feat: Echo model integration" --body "..."
```

## Troubleshooting

### "Skill not loading"
- Check `.claude/` symlink exists: `ls -la .claude`
- Verify submodule: `git submodule status`
- Update submodule: `git submodule update --remote`

### "Tests failing in CI but pass locally"
- Check for hardcoded paths (use `tempfile` crate)
- Verify no race conditions in async tests
- Ensure `#[tokio::test]` for async tests

### "Plan schema mismatch with AGQ"
- Canonical schemas: `agenix/specs/` directory
- Job schema spec: `agenix/docs/architecture/job-schema.md`
- Run cross-repo validation: `cargo test --test integration`

## References

- **Central docs:** `agenix/docs/`
- **Architecture:** `agenix/docs/architecture/` (execution-layers.md, job-schema.md, system-overview.md)
- **Schemas:** `agenix/specs/` (job.schema.json, README.md)
- **Security:** `agenix/docs/development/security-guidelines.md` (when created)
- **Testing:** `agenix/docs/development/testing-strategy.md` (when created)
- **RESP Protocol:** `agenix/docs/api/resp-protocol.md` (when created)

## When in Doubt

1. Check if a `.claude/skills/` applies
2. Read the central docs in `agenix-shared/docs/`
3. Ask for clarification in the issue
4. Use `Read` tool to examine existing code patterns

---

**Remember:** This instance works on **AGX-specific features**. For multi-repo coordination, architecture decisions, or documentation updates, those happen in the planning instance at `/Users/lewis/work/agenix-sh/`.
