# CLAUDE.md - Claude AI Integration Guide for AGX

**Version:** 1.0
**Last Updated:** 2025-11-16
**Purpose:** Comprehensive guide for Claude AI to understand, navigate, and contribute to the AGX codebase

---

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Architecture & Design Philosophy](#2-architecture--design-philosophy)
3. [Codebase Structure](#3-codebase-structure)
4. [Key Workflows](#4-key-workflows)
5. [Core Modules Deep Dive](#5-core-modules-deep-dive)
6. [Data Structures & Schemas](#6-data-structures--schemas)
7. [Integration Points](#7-integration-points)
8. [Development Workflow](#8-development-workflow)
9. [Testing & Quality Standards](#9-testing--quality-standards)
10. [Common Tasks](#10-common-tasks)
11. [Troubleshooting Guide](#11-troubleshooting-guide)
12. [References](#12-references)

---

## 1. Project Overview

### What is AGX?

AGX (`agx`) is a **Phase 1 Planner CLI** in the AGX/AGQ/AGW ecosystem. It transforms natural-language instructions into deterministic JSON workflow plans that can be executed by workers.

**Core Purpose:**
- Accept natural-language user instructions
- Generate deterministic, executable JSON plans via LLM backends
- Persist plans locally for inspection and iteration
- Submit plans to AGQ (queue manager) for worker execution
- Provide operational visibility into job/worker status

**Ecosystem Position:**
```
┌─────────┐     ┌─────────┐     ┌─────────┐
│   AGX   │────▶│   AGQ   │────▶│   AGW   │
│ Planner │     │  Queue  │     │ Worker  │
└─────────┘     └─────────┘     └─────────┘
```

- **AGX** (this repo): Plans creation and orchestration
- **AGQ**: Queue management, job scheduling, worker coordination
- **AGW**: Deterministic step execution without LLM dependencies
- **AGX-* tools**: Single-purpose agent tools (Phase 2+)

### Project Metadata

- **Language:** Rust (edition 2021, minimum version 1.82)
- **Repository:** `agenix-sh/agx`
- **License:** (Check repository)
- **Status:** Phase 1 implementation (MVP execution pipeline)
- **Version:** 0.1.0

### Key Features

1. **REPL-style Plan Building:** Iterative plan construction via `PLAN new`, `PLAN add`, `PLAN preview`, `PLAN submit`
2. **Zero External Dependencies:** Pure Rust with no cloud requirements
3. **LLM-Agnostic:** Pluggable backends (Ollama default, embedded llama.cpp optional)
4. **STDIN Integration:** Pipe data for context-aware planning
5. **RESP Protocol:** Redis-compatible protocol for AGQ communication
6. **Job Envelope:** Structured job submission with validation
7. **Ops Mode:** Query jobs, workers, and queue stats without leaving CLI

---

## 2. Architecture & Design Philosophy

### Design Principles

1. **Unix Philosophy**
   - Do one thing well: AGX only plans, doesn't execute
   - Compose via stdin/stdout
   - Text-based, inspectable JSON plans
   - Single-responsibility modules

2. **Local-First**
   - All state in local files (`$TMPDIR/agx-plan.json`)
   - No cloud dependencies
   - Portable across macOS and Linux
   - Zero-install execution post-build

3. **Separation of Concerns**
   - Planning separated from execution
   - LLM interaction isolated in planner module
   - RESP client decoupled from plan logic
   - Clear module boundaries

4. **Zero Trust LLM Output**
   - Multiple JSON parsing strategies
   - Automatic quote repair
   - Markdown fence stripping
   - Plan normalization (e.g., auto-insert `sort` before `uniq`)

5. **Explicit Error Handling**
   - No panics in normal operation
   - Result types throughout
   - Human-readable error messages with context
   - Graceful degradation

6. **Deterministic Execution**
   - Workers cannot call LLMs
   - Plans are fully specified JSON
   - Reproducible execution
   - Timeout and dependency management

### Core Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      AGX CLI                            │
├─────────────────────────────────────────────────────────┤
│  lib.rs: Command Router                                │
│    ├─ PLAN mode ──▶ handle_plan_command()              │
│    └─ OPS mode  ──▶ handle_ops_command()               │
├─────────────────────────────────────────────────────────┤
│  PLAN Mode Components                                   │
│    ├─ cli.rs          : Argument parsing                │
│    ├─ input.rs        : STDIN collection & analysis     │
│    ├─ registry.rs     : Tool catalog                    │
│    ├─ planner.rs      : LLM backend (Ollama/Embedded)   │
│    ├─ plan.rs         : Plan parsing & normalization    │
│    ├─ plan_buffer.rs  : Local persistence               │
│    ├─ job.rs          : Job envelope creation           │
│    └─ agq_client.rs   : RESP client for AGQ             │
├─────────────────────────────────────────────────────────┤
│  OPS Mode Components                                    │
│    ├─ cli.rs          : Command parsing                 │
│    └─ agq_client.rs   : AGQ queries (jobs/workers/stats)│
├─────────────────────────────────────────────────────────┤
│  Supporting Modules                                     │
│    ├─ executor.rs     : Local plan execution (testing)  │
│    └─ logging.rs      : Debug output                    │
└─────────────────────────────────────────────────────────┘
```

---

## 3. Codebase Structure

### Directory Layout

```
/home/user/agx/
├── src/                          # Core Rust source (12 modules)
│   ├── main.rs                   # Entry point
│   ├── lib.rs                    # Command router & orchestration
│   ├── cli.rs                    # CLI parsing (manual, no clap)
│   ├── plan.rs                   # Plan parsing & normalization
│   ├── planner.rs                # LLM backend integration
│   ├── plan_buffer.rs            # Local file persistence
│   ├── registry.rs               # Tool catalog
│   ├── agq_client.rs             # RESP client
│   ├── job.rs                    # Job envelope schema
│   ├── executor.rs               # Local execution (testing)
│   ├── input.rs                  # STDIN collection
│   └── logging.rs                # Debug logging
│
├── docs/                         # Architecture documentation
│   ├── ARCHITECTURE.md           # System design
│   ├── JOB_SCHEMA.md             # Job envelope specification
│   └── ROADMAP.md                # Phase planning
│
├── scripts/                      # Installation & utilities
│   └── install.sh                # Cross-platform installer
│
├── .github/                      # CI/CD & automation
│   ├── workflows/
│   │   ├── ci.yml                # Multi-platform build & test
│   │   ├── pr-checks.yml         # PR validation (coverage, format)
│   │   ├── release.yml           # Binary release automation
│   │   ├── claude.yml            # Claude integration
│   │   └── claude-code-review.yml
│   ├── CICD_SETUP.md             # CI documentation
│   ├── DEPLOYMENT_SUMMARY.md     # Rollout notes
│   └── TEMPLATE_FOR_AGX_AGW.md   # Ecosystem migration guide
│
├── Cargo.toml                    # Dependencies & metadata
├── Cargo.lock                    # Locked dependency versions
├── rust-toolchain.toml           # Rust version (1.82)
├── README.md                     # User documentation
├── AGENTS.md                     # Agent specs & workflows
├── EXAMPLES.md                   # Usage examples
└── CLAUDE.md                     # This file
```

### File Organization Principles

- **One concern per module:** Each `.rs` file has a single responsibility
- **Tests colocated:** `#[cfg(test)] mod tests` at bottom of each module
- **No deep nesting:** Flat src/ directory for discoverability
- **Documentation proximity:** Technical docs in `docs/`, user docs at root

---

## 4. Key Workflows

### Workflow 1: Plan Creation & Submission

```bash
# 1. Start fresh plan
agx PLAN new

# 2. Add steps with context (piped data)
cat data.csv | agx PLAN add "strip header row"
cat data.csv | agx PLAN add "dedupe rows by first three columns"

# 3. Inspect before submission
agx PLAN preview

# 4. Submit to AGQ
agx PLAN submit
```

**Internal Flow:**
```
PLAN new
  └─▶ plan_buffer::reset()
      └─▶ Write empty plan to $TMPDIR/agx-plan.json

PLAN add "instruction"
  ├─▶ input::collect() [if stdin is piped]
  ├─▶ registry::describe_for_planner()
  ├─▶ planner::plan(instruction, input, registry)
  │   └─▶ OllamaBackend::generate_plan(prompt)
  ├─▶ plan::parse() [with quote repair & fence stripping]
  ├─▶ plan::normalize_for_execution() [e.g., insert sort before uniq]
  ├─▶ plan_buffer::load() + append + save()
  └─▶ Output JSON status

PLAN preview
  └─▶ plan_buffer::load()
      └─▶ Output plan as JSON

PLAN submit
  ├─▶ plan_buffer::load()
  ├─▶ job::from_plan(uuid, uuid, description)
  ├─▶ job::validate(max_steps=100)
  ├─▶ agq_client::submit_plan(job_json)
  │   └─▶ RESP: PLAN.SUBMIT <json>
  ├─▶ plan_buffer::save_submission_metadata({job_id, timestamp})
  └─▶ Output job_id
```

### Workflow 2: Operational Monitoring

```bash
# List queued/running jobs
agx JOBS list [--json]

# Check registered workers
agx WORKERS list [--json]

# View queue statistics
agx QUEUE stats [--json]
```

**Internal Flow:**
```
JOBS list
  └─▶ agq_client::list_jobs()
      └─▶ RESP: JOBS.LIST
          └─▶ Parse response array

WORKERS list
  └─▶ agq_client::list_workers()
      └─▶ RESP: WORKERS.LIST

QUEUE stats
  └─▶ agq_client::queue_stats()
      └─▶ RESP: QUEUE.STATS
```

### Workflow 3: Development Contribution

See section [8. Development Workflow](#8-development-workflow) for detailed GitHub issue workflow.

---

## 5. Core Modules Deep Dive

### 5.1 `lib.rs` - Command Router

**Purpose:** Orchestrates CLI execution, routes commands to handlers

**Key Functions:**
- `run() -> Result<(), String>`: Main entry point
  - Parses CLI config
  - Enables debug logging
  - Routes to `handle_plan_command()` or `handle_ops_command()`

- `handle_plan_command(PlanCommand)`: Handles PLAN new/add/preview/submit
- `handle_ops_command(OpsCommand)`: Handles JOBS/WORKERS/QUEUE
- `build_job_envelope(plan)`: Creates validated job envelope
- `enforce_instruction_limit()`: Validates instruction ≤ 8KB

**Important Patterns:**
```rust
// Error propagation with context
let plan = storage.load()
    .map_err(|e| format!("failed to load plan: {e}"))?;

// JSON output convention
print_json(json!({
    "status": "ok",
    "field": value
}));
```

**Location:** `/home/user/agx/src/lib.rs:14-241`

---

### 5.2 `cli.rs` - Command Line Interface

**Purpose:** Manual CLI parser (no external framework)

**Key Types:**
```rust
pub struct CliConfig {
    pub show_help: bool,
    pub show_version: bool,
    pub debug: bool,
    pub command: Option<Command>,
}

pub enum Command {
    Plan(PlanCommand),
    Ops(OpsCommand),
}

pub enum PlanCommand {
    New,
    Add { instruction: String },
    Preview,
    Submit,
}

pub enum OpsCommand {
    Jobs { json: bool },
    Workers { json: bool },
    Queue { json: bool },
}
```

**Parsing Strategy:**
1. Collect flags first (`-h`, `-v`, `-d`)
2. Uppercase remaining tokens for case-insensitive matching
3. Match command (PLAN, JOBS, WORKERS, QUEUE)
4. Parse subcommand and arguments
5. Multi-word instructions joined with spaces

**Example:**
```bash
agx --debug PLAN add "remove duplicates"
# Parsed as:
# CliConfig { debug: true, command: Plan(Add { instruction: "remove duplicates" }) }
```

**Location:** `/home/user/agx/src/cli.rs`

---

### 5.3 `planner.rs` - LLM Backend Integration

**Purpose:** Abstract LLM interaction, support multiple backends

**Architecture:**
```rust
pub trait ModelBackend {
    fn generate_plan(&self, prompt: &str) -> Result<String, String>;
}

pub struct Planner {
    backend: Box<dyn ModelBackend>,
}

// Default: Ollama
struct OllamaBackend {
    model: String,
}

// Optional: Embedded llama.cpp
#[cfg(feature = "embedded-backend")]
struct EmbeddedBackend {
    model_path: String,
    arch: String,
}
```

**Prompt Construction:**
```rust
pub fn plan(
    &self,
    instruction: &str,
    input: &InputSummary,
    registry: &ToolRegistry,
) -> Result<PlanOutput, String>
```

**Prompt Template:**
```
You are the AGX Planner.

User instruction:
{instruction}

Input description:
bytes: {bytes}, lines: {lines}, is_empty: {empty}, is_probably_binary: {binary}

Available tools:
{registry.describe_for_planner()}

Respond with a single JSON object only, no extra commentary, in one of these exact shapes:
{"plan": [{"cmd": "tool-id"}, {"cmd": "tool-id", "args": ["arg1", "arg2"]}]}
or
{"plan": ["tool-id", "another-tool-id"]}

Use only the tools listed above and produce a deterministic, minimal plan.
```

**Configuration (Environment Variables):**
- `AGX_BACKEND`: "ollama" (default) or "embedded"
- `AGX_OLLAMA_MODEL`: Model name (default: "phi3:mini")
- `AGX_MODEL_PATH`: Local model file for embedded backend
- `AGX_MODEL_ARCH`: Architecture (default: "llama")

**Ollama Execution:**
```rust
let output = Command::new("ollama")
    .arg("run")
    .arg(&self.model)
    .arg(prompt)
    .output()
    .map_err(|error| format!("failed to run ollama: {error}"))?;
```

**Location:** `/home/user/agx/src/planner.rs`

---

### 5.4 `plan.rs` - Plan Parsing & Normalization

**Purpose:** Parse LLM output into structured plans, normalize for execution

**Key Types:**
```rust
pub struct WorkflowPlan {
    pub plan: Vec<PlanStep>,
}

pub struct PlanStep {
    pub cmd: String,
    pub args: Vec<String>,
    pub input_from_step: Option<u32>,
    pub timeout_secs: Option<u32>,
}

pub struct PlanOutput {
    pub raw_json: String,
}
```

**Parsing Strategies (tried in order):**

1. **Direct deserialize:** `serde_json::from_str::<WorkflowPlan>`
2. **Nested plan object:** `{"plan": {...}}` unwrapped
3. **String array:** `["sort", "uniq"]` converted to steps
4. **Quote repair:** Fix unescaped quotes in args
5. **Markdown fence strip:** Remove ` ```json` fences

**Quote Repair Example:**
```rust
// LLM output: {"cmd": "grep", "args": ["foo"bar"]}
// Repaired:   {"cmd": "grep", "args": ["foo\"bar"]}
repair_unescaped_quotes_in_args(json_text)
```

**Normalization Rules:**
```rust
pub fn normalize_for_execution(&self) -> WorkflowPlan {
    // Rule: Always sort before uniq
    if has_uniq_without_prior_sort() {
        insert_sort_before_first_uniq();
    }
    self
}
```

**Location:** `/home/user/agx/src/plan.rs`

---

### 5.5 `plan_buffer.rs` - Plan Persistence

**Purpose:** Manage local plan storage and submission metadata

**Storage Layout:**
```
$TMPDIR/agx-plan.json       # Main plan buffer
$TMPDIR/agx-plan.json.meta  # Submission metadata
```

**Key Types:**
```rust
pub struct PlanStorage {
    path: PathBuf,
}

pub struct PlanMetadata {
    pub job_id: String,
    pub submitted_at: String, // RFC3339 timestamp
}
```

**API:**
```rust
impl PlanStorage {
    pub fn from_env() -> Self;
    pub fn reset(&self) -> Result<WorkflowPlan, String>;
    pub fn load(&self) -> Result<WorkflowPlan, String>;
    pub fn save(&self, plan: &WorkflowPlan) -> Result<(), String>;
    pub fn save_submission_metadata(&self, meta: &PlanMetadata) -> Result<(), String>;
}
```

**Configuration:**
- `AGX_PLAN_PATH`: Override default location

**Important Behavior:**
- Missing plan files treated as empty (not error)
- Pretty-printed JSON for human readability
- Atomic write (temp file + rename pattern can be added)

**Location:** `/home/user/agx/src/plan_buffer.rs`

---

### 5.6 `registry.rs` - Tool Catalog

**Purpose:** Provide static catalog of available tools for planner

**Data Structure:**
```rust
pub struct Tool {
    pub id: &'static str,
    pub command: &'static str,
    pub description: &'static str,
    pub patterns: &'static [&'static str],
    pub ok_exit_codes: &'static [i32],
}

pub struct ToolRegistry {
    tools: Vec<Tool>,
}
```

**Current Tools (Phase 1):**
1. **sort** - Sort lines alphanumerically
2. **uniq** - Remove duplicate adjacent lines
3. **grep** - Filter lines matching pattern
4. **cut** - Extract column ranges
5. **tr** - Translate or delete characters
6. **jq** - Process JSON

**Pattern Matching:**
```rust
Tool {
    id: "uniq",
    patterns: &["dedupe", "unique", "remove duplicates"],
    ...
}
```

**Planner Description Format:**
```rust
pub fn describe_for_planner(&self) -> String {
    // Output: "- sort: Sort lines alphanumerically\n- uniq: ..."
}
```

**Future:** Phase 2+ will add `agx-ocr`, `agx-summarise`, etc.

**Location:** `/home/user/agx/src/registry.rs`

---

### 5.7 `agq_client.rs` - RESP Client

**Purpose:** Communicate with AGQ via Redis-compatible RESP protocol

**Configuration:**
```rust
pub struct AgqConfig {
    pub addr: String,              // Default: 127.0.0.1:6380
    pub session_key: Option<String>,
    pub timeout: Duration,          // Default: 5s
}
```

**Environment Variables:**
- `AGQ_ADDR`: TCP address
- `AGQ_SESSION_KEY`: Optional auth
- `AGQ_TIMEOUT_SECS`: Network timeout

**RESP Protocol Implementation:**

**Value Types:**
```rust
enum RespValue {
    SimpleString(String),   // +OK\r\n
    BulkString(String),     // $6\r\njob-42\r\n
    Error(String),          // -ERR message\r\n
    Integer(i64),           // :42\r\n
    Array(Vec<RespValue>),  // *2\r\n...
    Null,                   // $-1\r\n
}
```

**Encoding Example:**
```rust
fn resp_array(items: &[&str]) -> Vec<u8> {
    // *2\r\n$4\r\nPLAN\r\n$6\r\nSUBMIT\r\n
}
```

**Commands:**

1. **PLAN.SUBMIT <json>**
   ```rust
   pub fn submit_plan(&self, plan_json: &str) -> Result<SubmissionResult, String>
   // Returns: SubmissionResult { job_id, submitted_at }
   ```

2. **JOBS.LIST**
   ```rust
   pub fn list_jobs(&self) -> Result<OpsResponse, String>
   // Returns: OpsResponse::Jobs(Vec<String>)
   ```

3. **WORKERS.LIST / QUEUE.STATS**
   - Similar pattern to JOBS.LIST

**Connection Lifecycle:**
1. Open TCP stream
2. Set read/write timeouts
3. Send AUTH if session_key provided
4. Send command
5. Parse response
6. Close connection

**Location:** `/home/user/agx/src/agq_client.rs`

---

### 5.8 `job.rs` - Job Envelope Schema

**Purpose:** Define and validate job submission format

**Schema:**
```rust
pub struct JobEnvelope {
    pub job_id: String,           // UUID per submission
    pub plan_id: String,          // Stable plan ID
    pub plan_description: Option<String>,
    pub steps: Vec<JobStep>,
}

pub struct JobStep {
    pub step_number: u32,         // 1-based, contiguous
    pub command: String,
    pub args: Vec<String>,
    pub input_from_step: Option<u32>,
    pub timeout_secs: Option<u32>,
}
```

**Construction:**
```rust
impl JobEnvelope {
    pub fn from_plan(
        plan: WorkflowPlan,
        job_id: String,
        plan_id: String,
        plan_description: Option<String>,
    ) -> Self;
}
```

**Validation Rules:**
```rust
pub enum EnvelopeValidationError {
    EmptySteps,
    TooManySteps(usize),
    NonMonotonicSteps,
    BadInputReference(u32),  // References non-existent step
    FirstStepNotOne(u32),
}

impl JobEnvelope {
    pub fn validate(&self, max_steps: usize) -> Result<(), EnvelopeValidationError>;
}
```

**Validation Checks:**
1. Steps non-empty
2. Steps count ≤ max_steps (default: 100)
3. First step number is 1
4. Step numbers are monotonic (1, 2, 3, ...)
5. `input_from_step` references only prior steps

**JSON Example:**
```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "plan_id": "123e4567-e89b-12d3-a456-426614174000",
  "plan_description": "Dedupe CSV file",
  "steps": [
    {
      "step_number": 1,
      "command": "sort",
      "args": [],
      "timeout_secs": 30
    },
    {
      "step_number": 2,
      "command": "uniq",
      "args": [],
      "input_from_step": 1,
      "timeout_secs": 30
    }
  ]
}
```

**See Also:** `docs/JOB_SCHEMA.md`

**Location:** `/home/user/agx/src/job.rs`

---

### 5.9 `input.rs` - STDIN Collection

**Purpose:** Collect and analyze piped input for planner context

**Key Types:**
```rust
pub struct InputSummary {
    pub bytes: usize,
    pub lines: usize,
    pub is_empty: bool,
    pub is_probably_binary: bool,
    pub content: Vec<u8>,
}

pub struct InputCollector;
```

**API:**
```rust
impl InputCollector {
    pub fn stdin_is_terminal() -> bool;
    pub fn collect() -> std::io::Result<InputSummary>;
}

impl InputSummary {
    pub fn empty() -> Self;
}
```

**Binary Detection:**
```rust
// Presence of null byte indicates binary
is_probably_binary = content.contains(&0);
```

**Usage Pattern:**
```rust
let input = if InputCollector::stdin_is_terminal() {
    InputSummary::empty()
} else {
    InputCollector::collect()?
};
```

**Location:** `/home/user/agx/src/input.rs`

---

### 5.10 `executor.rs` - Local Execution (Testing)

**Purpose:** Execute plans locally for testing (not production execution)

**Key Function:**
```rust
pub fn execute_plan_local(
    plan: &WorkflowPlan,
    registry: &ToolRegistry,
) -> Result<String, String>
```

**Execution Model:**
- Sequential step execution
- Pipe stdout of step N to stdin of step N+1
- Exit code validation against `ok_exit_codes`
- Returns final stdout

**Used For:**
- Integration testing
- Local plan validation
- Development debugging

**Production Note:** AGW workers handle production execution, not this module.

**Location:** `/home/user/agx/src/executor.rs`

---

### 5.11 `logging.rs` - Debug Output

**Purpose:** Simple debug logging to stderr

**Implementation:**
```rust
static DEBUG: AtomicBool = AtomicBool::new(false);

pub fn set_debug(enabled: bool);
pub fn info(message: &str);
```

**Usage:**
```rust
logging::set_debug(config.debug);
logging::info(&format!("instruction: {}", instruction));
```

**Output Format:**
```
[agx] instruction: remove duplicates
[agx] available tools: sort, uniq, grep, cut, tr, jq
[agx] planner raw output: {"plan": [...]}
```

**Location:** `/home/user/agx/src/logging.rs`

---

## 6. Data Structures & Schemas

### 6.1 WorkflowPlan

**Definition:** `src/plan.rs`

```rust
#[derive(Serialize, Deserialize)]
pub struct WorkflowPlan {
    pub plan: Vec<PlanStep>,
}

#[derive(Serialize, Deserialize)]
pub struct PlanStep {
    pub cmd: String,
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_from_step: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u32>,
}
```

**JSON Example:**
```json
{
  "plan": [
    {"cmd": "sort"},
    {"cmd": "uniq", "input_from_step": 1}
  ]
}
```

---

### 6.2 JobEnvelope

**Definition:** `src/job.rs`

```rust
#[derive(Serialize, Deserialize)]
pub struct JobEnvelope {
    pub job_id: String,
    pub plan_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_description: Option<String>,
    pub steps: Vec<JobStep>,
}

#[derive(Serialize, Deserialize)]
pub struct JobStep {
    pub step_number: u32,
    pub command: String,
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_from_step: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u32>,
}
```

**See:** `docs/JOB_SCHEMA.md` for full specification

---

### 6.3 Tool Registry Entry

**Definition:** `src/registry.rs`

```rust
pub struct Tool {
    pub id: &'static str,
    pub command: &'static str,
    pub description: &'static str,
    pub patterns: &'static [&'static str],
    pub ok_exit_codes: &'static [i32],
}
```

**Example:**
```rust
Tool {
    id: "grep",
    command: "grep",
    description: "Filter lines matching a pattern",
    patterns: &["filter", "search", "match", "find lines"],
    ok_exit_codes: &[0, 1], // 1 = no matches
}
```

---

### 6.4 RESP Protocol Values

**Definition:** `src/agq_client.rs`

```rust
enum RespValue {
    SimpleString(String),   // +OK\r\n
    BulkString(String),     // $6\r\nvalue\r\n
    Error(String),          // -ERR message\r\n
    Integer(i64),           // :42\r\n
    Array(Vec<RespValue>),  // *2\r\n...
    Null,                   // $-1\r\n
}
```

---

## 7. Integration Points

### 7.1 Ollama Integration

**Backend:** Default LLM provider

**Requirements:**
- Ollama CLI installed and in PATH
- Model pulled: `ollama pull phi3:mini` (or configured model)

**Execution:**
```rust
Command::new("ollama")
    .arg("run")
    .arg(&self.model)
    .arg(prompt)
    .output()
```

**Configuration:**
- `AGX_OLLAMA_MODEL`: Model name (default: "phi3:mini")

**Recommended Models:**
- `phi3:mini` (default, good balance)
- `qwen2.5:1.5b` (faster, smaller)
- `mistral-nemo` (higher quality)

**Output Handling:**
- Strips markdown fences
- Repairs malformed JSON
- Supports multiple response formats

---

### 7.2 AGQ (Queue Manager) Integration

**Protocol:** RESP (Redis Serialization Protocol)

**Connection:**
- TCP to `AGQ_ADDR` (default: `127.0.0.1:6380`)
- Optional session-key authentication
- Timeout-controlled operations

**Commands Sent by AGX:**

1. **AUTH** (if session_key configured)
   ```
   *2\r\n$4\r\nAUTH\r\n$6\r\nsecret\r\n
   ```

2. **PLAN.SUBMIT**
   ```
   *2\r\n$11\r\nPLAN.SUBMIT\r\n$<len>\r\n<job_json>\r\n
   ```
   Expected response: Bulk string with job_id

3. **JOBS.LIST**
   ```
   *1\r\n$9\r\nJOBS.LIST\r\n
   ```
   Expected response: Array of strings

4. **WORKERS.LIST / QUEUE.STATS**
   - Similar to JOBS.LIST

**Error Handling:**
- Connection failures: Descriptive error messages
- AUTH errors: Propagated to user
- Unexpected responses: Type mismatch detection

**See Also:** AGQ repository documentation

---

### 7.3 Embedded Backend (Optional)

**Feature Flag:** `embedded-backend`

**Dependencies:**
```toml
llm = { version = "0.1", default-features = false, optional = true }
rand = { version = "0.8", optional = true }
```

**Configuration:**
- `AGX_BACKEND=embedded`
- `AGX_MODEL_PATH`: Path to GGML model file
- `AGX_MODEL_ARCH`: "llama" (default) or other

**Build:**
```bash
cargo build --features embedded-backend
```

**Use Case:** Offline operation without Ollama dependency

---

### 7.4 Plan Buffer File Format

**Location:** `$TMPDIR/agx-plan.json` (or `$AGX_PLAN_PATH`)

**Format:** Pretty-printed JSON

```json
{
  "plan": [
    {
      "cmd": "sort",
      "args": []
    },
    {
      "cmd": "uniq",
      "args": [],
      "input_from_step": 1
    }
  ]
}
```

**Metadata Sidecar:** `<plan_path>.meta`
```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "submitted_at": "2025-11-16T10:30:00Z"
}
```

---

## 8. Development Workflow

### 8.1 GitHub Issue Workflow

**Defined in:** `AGENTS.md`

**Process:**

1. **Inspect Issue**
   ```bash
   gh issue view <number> --repo agenix-sh/agx --json number,title,body,state,url
   ```

2. **Issue Naming Convention**
   - Format: `AGX-XXX: Description`
   - Example: `AGX-030: Implement PLAN subcommands`
   - Keep numeric ID stable across PRs/branches

3. **Create Feature Branch**
   ```bash
   git switch -c issue-<number>-<short-slug>
   # Example: issue-30-plan-cli
   ```

4. **Implement Changes**
   - Align with `ARCHITECTURE.md` and `AGENTS.md`
   - Write tests (required for every change)
   - Follow code style (rustfmt + clippy)

5. **Commit Work**
   ```bash
   git add .
   git commit -m "AGX-030: Implement PLAN subcommands (#30)"
   ```

6. **Push and Create PR**
   ```bash
   git push -u origin issue-<number>-<short-slug>
   gh pr create --repo agenix-sh/agx --head issue-<number>-<short-slug> --fill
   ```

7. **PR Requirements**
   - Title: Must match `AGX-XXX: ...` exactly
   - Body sections: `## Issue`, `## Security Review`, `## Testing`
   - Coverage: ≥80%
   - All checks passing
   - Dual AI review (Codex + Claude)

8. **Iterate and Merge**
   - Address review feedback
   - Accept or discuss suggestions
   - Merge when approved

---

### 8.2 Branch Naming

**Pattern:** `issue-<number>-<slug>`

**Examples:**
- `issue-1-bootstrap-agx-cli`
- `issue-30-plan-cli`
- `issue-31-agq-submit`

---

### 8.3 Commit Message Format

**Pattern:**
```
AGX-XXX: Brief description (#issue-number)
```

**Examples:**
```
AGX-030: Implement PLAN subcommands (#30)
AGX-031: Wire PLAN submit to AGQ (#31)
AGX-040: Wrap PLAN submit in job envelope (#40)
```

---

## 9. Testing & Quality Standards

### 9.1 Coverage Requirements

**Minimum:** 80% line coverage

**Enforcement:**
- PR Checks workflow (`pr-checks.yml`)
- Uses `cargo-llvm-cov`
- Blocks merge if below threshold

**Running Coverage Locally:**
```bash
# Generate HTML report
cargo llvm-cov --workspace --html

# View in browser
open target/llvm-cov/html/index.html
```

---

### 9.2 Testing Patterns

**Unit Tests:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_describes_scenario() {
        // Arrange
        let input = create_test_input();

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected);
    }
}
```

**Integration Tests (Mock TCP Servers):**
```rust
#[test]
fn submits_plan_and_parses_job_id() {
    // Start mock AGQ server
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn server thread
    thread::spawn(move || {
        // Handle request, send response
    });

    // Test client
    let client = AgqClient::new(config_with_addr(addr));
    let result = client.submit_plan(plan_json).unwrap();

    assert_eq!(result.job_id, "expected-id");
}
```

---

### 9.3 Quality Gates (CI)

**1. Format Check**
```bash
cargo fmt --all -- --check
```

**2. Clippy Linting**
```bash
cargo clippy --all-targets -- -D warnings -W clippy::all -W clippy::pedantic
```

**3. Build**
```bash
cargo build --verbose
cargo build --release --verbose
```

**4. Tests**
```bash
cargo test --verbose
```

**5. Security Audit**
```bash
cargo audit
```

**6. Coverage**
```bash
cargo llvm-cov --workspace --lcov --output-path coverage.lcov
# Upload to Codecov
```

---

### 9.4 Test Organization

**Location:** End of each module file

```rust
// src/plan.rs
pub struct WorkflowPlan { ... }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repairs_unescaped_quotes_in_args() { ... }
}
```

**Coverage by Module:**
- `cli.rs`: Argument parsing variations
- `plan.rs`: JSON parsing robustness
- `plan_buffer.rs`: Persistence roundtrips
- `agq_client.rs`: RESP protocol handling
- `job.rs`: Envelope validation
- `lib.rs`: Instruction limits, envelope building
- `registry.rs`: Tool metadata
- `input.rs`: STDIN analysis

---

## 10. Common Tasks

### 10.1 Add a New Tool to Registry

**File:** `src/registry.rs`

**Steps:**

1. Add tool definition:
```rust
Tool {
    id: "new-tool",
    command: "new-tool",
    description: "What the tool does",
    patterns: &["intent1", "intent2", "intent3"],
    ok_exit_codes: &[0],
}
```

2. Append to `ToolRegistry::new()`:
```rust
pub fn new() -> Self {
    Self {
        tools: vec![
            Tool { id: "sort", ... },
            // ... existing tools
            Tool { id: "new-tool", ... },
        ],
    }
}
```

3. Write tests for pattern matching

4. Update docs if needed

---

### 10.2 Add a New PLAN Subcommand

**Files:** `src/cli.rs`, `src/lib.rs`

**Steps:**

1. Add variant to `PlanCommand` enum:
```rust
pub enum PlanCommand {
    New,
    Add { instruction: String },
    Preview,
    Submit,
    YourNewCommand { arg: String },
}
```

2. Update parser in `cli.rs`:
```rust
"YOURNEWCOMMAND" => {
    let arg = args.join(" ");
    Ok(Command::Plan(PlanCommand::YourNewCommand { arg }))
}
```

3. Add handler in `lib.rs`:
```rust
fn handle_plan_command(command: PlanCommand) -> Result<(), String> {
    match command {
        // ... existing handlers
        PlanCommand::YourNewCommand { arg } => {
            // Implementation
            print_json(json!({"status": "ok"}));
        }
    }
}
```

4. Write tests for parsing and execution

5. Update `cli::print_help()` with new command

---

### 10.3 Add a New Ops Command

**Similar to PLAN subcommand:**

1. Update `OpsCommand` enum
2. Update parser
3. Implement AGQ client method if needed
4. Add handler in `handle_ops_command()`

---

### 10.4 Modify Job Envelope Schema

**Files:** `src/job.rs`, `docs/JOB_SCHEMA.md`

**Steps:**

1. Update `JobEnvelope` or `JobStep` struct
2. Update `from_plan()` construction
3. Update `validate()` if new validation needed
4. Update tests
5. Update `docs/JOB_SCHEMA.md`
6. Coordinate with AGQ/AGW repos if breaking change

---

### 10.5 Add a New Planner Backend

**File:** `src/planner.rs`

**Steps:**

1. Implement `ModelBackend` trait:
```rust
struct MyBackend {
    config: String,
}

impl ModelBackend for MyBackend {
    fn generate_plan(&self, prompt: &str) -> Result<String, String> {
        // Call your LLM API
        Ok(json_response)
    }
}
```

2. Update `PlannerConfig::from_env()`:
```rust
let backend_name = env::var("AGX_BACKEND").unwrap_or_else(|_| "ollama".into());
match backend_name.as_str() {
    "ollama" => Box::new(OllamaBackend::new(...)),
    "mybackend" => Box::new(MyBackend::new(...)),
    _ => return Err(...),
}
```

3. Add configuration environment variables
4. Write integration tests
5. Update README.md with new backend docs

---

### 10.6 Run Local Development Build

```bash
# Debug build
cargo build

# Run with debug logging
./target/debug/agx --debug PLAN new

# Test with sample data
cat test.txt | ./target/debug/agx PLAN add "remove duplicates"

# Preview plan
./target/debug/agx PLAN preview
```

---

### 10.7 Test RESP Client Against Mock AGQ

**File:** `src/agq_client.rs`

**Pattern:**
```rust
#[test]
fn test_submit_success() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        // Read PLAN.SUBMIT command
        // Send mock response: $6\r\njob-42\r\n
    });

    let config = AgqConfig {
        addr: addr.to_string(),
        session_key: None,
        timeout: Duration::from_secs(5),
    };

    let client = AgqClient::new(config);
    let result = client.submit_plan(r#"{"plan":[]}"#).unwrap();
    assert_eq!(result.job_id, "job-42");
}
```

---

## 11. Troubleshooting Guide

### 11.1 Common Issues

**Issue:** `ollama: command not found`

**Cause:** Ollama not installed or not in PATH

**Solution:**
```bash
# Install Ollama: https://ollama.ai
curl -fsSL https://ollama.ai/install.sh | sh

# Pull model
ollama pull phi3:mini

# Verify
ollama run phi3:mini "test"
```

---

**Issue:** `failed to connect to AGQ: Connection refused`

**Cause:** AGQ not running

**Solution:**
```bash
# Start AGQ (in separate terminal)
agq

# Verify listening on 6380
netstat -an | grep 6380

# Or configure different address
export AGQ_ADDR=127.0.0.1:6380
```

---

**Issue:** `AUTH failed: invalid session key`

**Cause:** Mismatched session keys between AGX and AGQ

**Solution:**
```bash
# Ensure same key on both sides
export AGQ_SESSION_KEY=your-secret-key

# Or omit for no auth (local dev only)
unset AGQ_SESSION_KEY
```

---

**Issue:** `plan contains no steps`

**Cause:** LLM returned empty plan or parsing failed

**Solution:**
```bash
# Enable debug logging to see raw LLM output
agx --debug PLAN add "your instruction"

# Check planner raw output in logs
# Adjust instruction or try different model
export AGX_OLLAMA_MODEL=qwen2.5:1.5b
```

---

**Issue:** `job envelope validation failed: BadInputReference(5)`

**Cause:** Step references non-existent prior step

**Solution:**
- Check plan with `agx PLAN preview`
- Ensure `input_from_step` only references earlier steps
- File bug if planner generated invalid reference

---

**Issue:** `instruction is too long (9000 bytes > 8192 allowed)`

**Cause:** Instruction exceeds 8KB limit

**Solution:**
- Shorten instruction
- Or increase `MAX_INSTRUCTION_BYTES` in `src/lib.rs:159` (requires rebuild)

---

**Issue:** Coverage below 80% in PR checks

**Cause:** New code lacks sufficient tests

**Solution:**
```bash
# Generate coverage report
cargo llvm-cov --workspace --html

# Identify untested lines
open target/llvm-cov/html/index.html

# Add tests for uncovered code paths
```

---

### 11.2 Debug Logging

**Enable:**
```bash
agx --debug PLAN add "instruction"
```

**Output Includes:**
- Full instruction text
- Input summary (bytes, lines, binary flag)
- Available tools list
- Raw planner JSON output
- Step counts after append

**Location:** stderr

**Format:**
```
[agx] instruction: remove duplicates
[agx] available tools: sort, uniq, grep, cut, tr, jq
[agx] planner raw output: {"plan": [{"cmd": "sort"}, {"cmd": "uniq"}]}
[agx] PLAN add appended 2 step(s); buffer now has 2 step(s)
```

---

### 11.3 Inspecting Plan Buffer

**Location:** `$TMPDIR/agx-plan.json`

```bash
# View raw plan file
cat $TMPDIR/agx-plan.json | jq .

# Or use PLAN preview
agx PLAN preview

# View submission metadata
cat $TMPDIR/agx-plan.json.meta | jq .
```

---

### 11.4 Testing RESP Communication

**Manual RESP Testing:**
```bash
# Connect to AGQ with netcat
nc localhost 6380

# Send AUTH (if needed)
*2\r\n$4\r\nAUTH\r\n$6\r\nsecret\r\n

# Send JOBS.LIST
*1\r\n$9\r\nJOBS.LIST\r\n
```

**Or use redis-cli:**
```bash
redis-cli -h 127.0.0.1 -p 6380
> AUTH secret
> JOBS.LIST
```

---

## 12. References

### Internal Documentation

- **AGENTS.md** - Agent specifications, GitHub workflow, testing requirements
- **ARCHITECTURE.md** - System design, component responsibilities
- **ROADMAP.md** - Phased delivery plan
- **JOB_SCHEMA.md** - Job envelope specification
- **EXAMPLES.md** - Usage examples
- **README.md** - User-facing documentation

### External Resources

- **Rust Documentation:** https://doc.rust-lang.org/
- **Ollama:** https://ollama.ai/
- **RESP Protocol:** https://redis.io/docs/reference/protocol-spec/
- **Codecov:** https://codecov.io/gh/agenix-sh/agx

### Related Repositories

- **agenix-sh/agq** - Queue manager
- **agenix-sh/agw** - Worker executor
- **agenix-sh/agx-ocr** - OCR agent tool (Phase 2)

---

## Appendix A: Environment Variables Reference

| Variable | Default | Purpose |
|----------|---------|---------|
| `AGX_PLAN_PATH` | `$TMPDIR/agx-plan.json` | Plan buffer location |
| `AGX_PLAN_DESCRIPTION` | (none) | Optional plan description for job envelope |
| `AGX_BACKEND` | `ollama` | Planner backend: "ollama" or "embedded" |
| `AGX_OLLAMA_MODEL` | `phi3:mini` | Ollama model name |
| `AGX_MODEL_PATH` | (none) | Local model file for embedded backend |
| `AGX_MODEL_ARCH` | `llama` | Model architecture for embedded backend |
| `AGQ_ADDR` | `127.0.0.1:6380` | AGQ TCP address |
| `AGQ_SESSION_KEY` | (none) | Session key for AGQ AUTH |
| `AGQ_TIMEOUT_SECS` | `5` | Network timeout in seconds |

---

## Appendix B: File Locations Quick Reference

| Path | Purpose |
|------|---------|
| `src/main.rs` | Entry point |
| `src/lib.rs` | Command router |
| `src/cli.rs` | CLI parsing |
| `src/planner.rs` | LLM backends |
| `src/plan.rs` | Plan parsing |
| `src/plan_buffer.rs` | Persistence |
| `src/registry.rs` | Tool catalog |
| `src/agq_client.rs` | RESP client |
| `src/job.rs` | Job envelope |
| `src/input.rs` | STDIN handling |
| `src/executor.rs` | Local execution |
| `src/logging.rs` | Debug logging |
| `docs/ARCHITECTURE.md` | System design |
| `docs/JOB_SCHEMA.md` | Job format |
| `docs/ROADMAP.md` | Development plan |
| `AGENTS.md` | Workflow guide |
| `README.md` | User docs |
| `EXAMPLES.md` | Usage examples |

---

## Appendix C: Testing Checklist

When contributing new code, ensure:

- [ ] Unit tests cover new functions
- [ ] Integration tests for external interactions (RESP, Ollama)
- [ ] Error cases tested (invalid input, connection failures)
- [ ] Tests run on all platforms (macOS, Linux)
- [ ] Coverage ≥ 80%
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo audit` passes
- [ ] PR includes `## Testing` section describing test strategy

---

## Appendix D: Code Style Guidelines

**Enforced by:** `rustfmt` and `clippy`

**Key Conventions:**

1. **Error Handling:**
   - Use `Result<T, String>` for fallible operations
   - Add context at each layer: `.map_err(|e| format!("context: {e}"))`
   - No unwrap/expect in production code

2. **Naming:**
   - Snake_case for functions, variables
   - PascalCase for types
   - SCREAMING_SNAKE_CASE for constants

3. **Documentation:**
   - Public items must have doc comments
   - Use `///` for item docs
   - Use `//!` for module-level docs

4. **Testing:**
   - Test module at bottom of file: `#[cfg(test)] mod tests`
   - Descriptive test names: `fn test_scenario_expected_behavior()`

5. **Imports:**
   - Group: std, external crates, internal modules
   - Use `use crate::module::item` for clarity

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2025-11-16 | Initial CLAUDE.md creation |

---

**End of CLAUDE.md**

For questions or improvements to this guide, please open an issue at: https://github.com/agenix-sh/agx/issues
