# AGX Agents Specification

AGX itself is an Agentic Unit (AU) designed to run in CLI environments.  
This document defines the internal agents and their behaviours.

## 1. Planner Agent
### Purpose
Convert a natural-language instruction into a deterministic workflow plan.

### Input
- Natural-language task
- Description of input stream
- List of available tools

### Output (JSON)
```json
{
  "plan": [
    {"cmd": "sort"},
    {"cmd": "uniq"}
  ]
}
```

### Model
- Phase 1: Ollama (`phi3:mini`, `qwen2.5:1.5b`, or `mistral-nemo`)
- Phase 2: embedded llama.cpp

---

## 2. Executor Agent
### Purpose
Execute the planned workflow safely.

### Capabilities
- Pipe STDIN through each command
- Capture stdout/stderr
- Fail gracefully
- Provide deterministic output

---

## 3. Registry Agent
### Purpose
Resolve natural-language intentions to tool capabilities.

### Responsibilities
- Maintain catalog of Unix-like tools
- Produce structured metadata:
```json
{
  "tool": "uniq",
  "description": "Remove duplicate lines",
  "patterns": ["dedupe", "unique", "remove duplicates"]
}
```

---

## 4. Future Agents
### Semantic Tool Agent
Wraps AI-based transformations (summarisation, extraction, rewriting).

### MCP Integration Agent
Discovers remote tools via MCP servers.

### Distributed AU Agent
Allows AGX to participate in a cluster of AOA units.

---

## AOA Notes
AGX is a perfect micro-AU:
- Clear contract (stdin+instruction → stdout)
- Deterministic execution layer
- Semantic planning
- Can be embedded into larger pipelines

---

## GitHub Issue Workflow
This workflow is used to implement GitHub issues for the `agenix-sh/agx` repository:

1. Inspect the issue  
   - Use `gh issue view <number> --repo agenix-sh/agx --json number,title,body,state,url` to read the issue details.

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
   - Address review feedback as needed.  
   - Merge the pull request when approved, ensuring the issue is linked or closed via the PR.
