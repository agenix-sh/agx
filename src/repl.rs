//! Interactive REPL for AGX plan crafting
//!
//! This module provides an interactive Read-Eval-Print-Loop (REPL) interface
//! for iteratively building and refining plans using the Echo model.

use std::fs;
use std::path::PathBuf;

use rustyline::error::ReadlineError;
use rustyline::{Config, DefaultEditor, EditMode};
use serde::{Deserialize, Serialize};

use crate::plan::WorkflowPlan;
use crate::plan_buffer::PlanStorage;
use crate::planner::{ModelBackend, PlanContext, ToolInfo};
use crate::registry;

/// Maximum number of history entries to persist
const MAX_HISTORY_SIZE: usize = 1000;

/// REPL session state that persists across invocations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplState {
    /// Current plan buffer
    pub plan: WorkflowPlan,
    /// Command history (limited to MAX_HISTORY_SIZE entries)
    pub history: Vec<String>,
    /// Last save timestamp
    pub last_saved: Option<String>,
}

impl Default for ReplState {
    fn default() -> Self {
        Self {
            plan: WorkflowPlan::default(),
            history: Vec::new(),
            last_saved: None,
        }
    }
}

impl ReplState {
    /// Load state from disk
    pub fn load() -> Result<Self, String> {
        let path = Self::state_path()?;

        match fs::read_to_string(&path) {
            Ok(contents) => {
                if contents.trim().is_empty() {
                    return Ok(Self::default());
                }

                serde_json::from_str(&contents)
                    .map_err(|e| format!("failed to parse REPL state: {}", e))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(Self::default())
            }
            Err(e) => Err(format!("failed to read REPL state: {}", e)),
        }
    }

    /// Save state to disk
    pub fn save(&mut self) -> Result<(), String> {
        let path = Self::state_path()?;

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create state directory: {}", e))?;
        }

        // Trim history to prevent unbounded growth
        if self.history.len() > MAX_HISTORY_SIZE {
            let start = self.history.len() - MAX_HISTORY_SIZE;
            self.history = self.history[start..].to_vec();
        }

        // Update save timestamp
        self.last_saved = Some(chrono::Utc::now().to_rfc3339());

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("failed to serialize REPL state: {}", e))?;

        fs::write(&path, json)
            .map_err(|e| format!("failed to write REPL state: {}", e))
    }

    /// Get the path to the state file
    fn state_path() -> Result<PathBuf, String> {
        let home = dirs::home_dir()
            .ok_or_else(|| "could not determine home directory".to_string())?;

        let mut path = home;
        path.push(".agx");
        path.push("repl-state.json");

        // Validate path doesn't contain parent directory references
        for component in path.components() {
            if matches!(component, std::path::Component::ParentDir) {
                return Err("invalid state path: contains parent directory reference".to_string());
            }
        }

        Ok(path)
    }
}

/// REPL command parsed from user input
#[derive(Debug, Clone, PartialEq)]
pub enum ReplCommand {
    // Plan building commands
    Add(String),
    Preview,
    Edit(usize),
    Remove(usize),
    Clear,
    Validate,
    Submit,
    Save,

    // Plan operations (AGX-073)
    PlanList,
    PlanGet(String),  // plan-id

    // Action operations (AGX-073)
    ActionSubmit { plan_id: String, input: Option<String> },

    // Operational commands (AGX-058)
    JobList,
    WorkerList,
    QueueStats,

    // Session commands
    Help,
    Quit,
}

impl ReplCommand {
    /// Parse a line of user input into a REPL command
    pub fn parse(line: &str) -> Result<Self, String> {
        let line = line.trim();

        if line.is_empty() {
            return Err("empty command".to_string());
        }

        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        let cmd = parts[0].to_lowercase();

        match cmd.as_str() {
            "add" | "a" => {
                let instruction = parts.get(1)
                    .ok_or_else(|| "add requires an instruction".to_string())?
                    .trim();

                if instruction.is_empty() {
                    return Err("add requires a non-empty instruction".to_string());
                }

                Ok(ReplCommand::Add(instruction.to_string()))
            }
            "preview" | "show" | "list" | "p" => Ok(ReplCommand::Preview),
            "edit" | "e" => {
                let num_str = parts.get(1)
                    .ok_or_else(|| "edit requires a step number".to_string())?
                    .trim();

                let num: usize = num_str.parse()
                    .map_err(|_| format!("invalid step number: {}", num_str))?;

                if num == 0 {
                    return Err("step numbers start at 1".to_string());
                }

                Ok(ReplCommand::Edit(num))
            }
            "remove" | "delete" | "rm" | "r" => {
                let num_str = parts.get(1)
                    .ok_or_else(|| "remove requires a step number".to_string())?
                    .trim();

                let num: usize = num_str.parse()
                    .map_err(|_| format!("invalid step number: {}", num_str))?;

                if num == 0 {
                    return Err("step numbers start at 1".to_string());
                }

                Ok(ReplCommand::Remove(num))
            }
            "clear" | "reset" | "new" | "c" => Ok(ReplCommand::Clear),
            "validate" | "v" => Ok(ReplCommand::Validate),
            "submit" | "s" => Ok(ReplCommand::Submit),
            "save" => Ok(ReplCommand::Save),

            // Plan operations (AGX-073)
            "plan" => {
                let subparts = parts.get(1)
                    .ok_or_else(|| "plan requires subcommand: list, get <id>".to_string())?
                    .trim();

                let subparts: Vec<&str> = subparts.splitn(2, ' ').collect();
                match subparts[0] {
                    "list" => Ok(ReplCommand::PlanList),
                    "get" => {
                        let plan_id = subparts.get(1)
                            .ok_or_else(|| "plan get requires plan-id".to_string())?
                            .trim()
                            .to_string();
                        Ok(ReplCommand::PlanGet(plan_id))
                    }
                    _ => Err(format!("unknown plan subcommand: {}. Use: plan list, plan get <id>", subparts[0]))
                }
            }

            // Action operations (AGX-073)
            "action" => {
                let args = parts.get(1)
                    .ok_or_else(|| "action requires plan-id".to_string())?
                    .trim();

                let args: Vec<&str> = args.splitn(2, ' ').collect();
                let plan_id = args[0].to_string();
                let input = args.get(1).map(|s| s.to_string());

                Ok(ReplCommand::ActionSubmit { plan_id, input })
            }

            // Operational commands (AGX-058)
            "jobs" | "j" => Ok(ReplCommand::JobList),
            "workers" | "w" => Ok(ReplCommand::WorkerList),
            "stats" | "queue" => Ok(ReplCommand::QueueStats),

            "help" | "?" | "h" => Ok(ReplCommand::Help),
            "quit" | "exit" | "q" => Ok(ReplCommand::Quit),
            _ => Err(format!("unknown command: {}. Type 'help' for available commands", cmd)),
        }
    }
}

/// Interactive REPL session
pub struct Repl {
    state: ReplState,
    editor: DefaultEditor,
    backend: Box<dyn ModelBackend>,
    plan_storage: PlanStorage,
    runtime: tokio::runtime::Runtime,
}

impl Repl {
    /// Create a new REPL session
    pub fn new(backend: Box<dyn ModelBackend>) -> Result<Self, String> {
        // Load persisted state
        let state = ReplState::load()?;

        // Configure rustyline editor
        let config = Config::builder()
            .edit_mode(EditMode::Vi)  // Default to vi mode
            .auto_add_history(true)
            .build();

        let mut editor = DefaultEditor::with_config(config)
            .map_err(|e| format!("failed to create editor: {}", e))?;

        // Restore command history
        for cmd in &state.history {
            editor.add_history_entry(cmd)
                .map_err(|e| format!("failed to restore history: {}", e))?;
        }

        // Use existing plan buffer storage
        let plan_storage = PlanStorage::from_env();

        // Create Tokio runtime once for all async operations
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| format!("failed to create runtime: {}", e))?;

        Ok(Self {
            state,
            editor,
            backend,
            plan_storage,
            runtime,
        })
    }

    /// Run the REPL loop
    pub fn run(&mut self) -> Result<(), String> {
        println!("AGX Interactive REPL");
        println!("Type 'help' for available commands, 'quit' to exit");
        println!();

        // Show plan summary if resuming session
        if !self.state.plan.tasks.is_empty() {
            println!("📋 Resumed session with {} task(s)", self.state.plan.tasks.len());
            println!();
        }

        loop {
            let prompt = format!("agx ({})> ", self.state.plan.tasks.len());

            match self.editor.readline(&prompt) {
                Ok(line) => {
                    let line = line.trim();

                    if line.is_empty() {
                        continue;
                    }

                    // Parse command
                    let cmd = match ReplCommand::parse(line) {
                        Ok(cmd) => cmd,
                        Err(e) => {
                            eprintln!("❌ {}", e);
                            continue;
                        }
                    };

                    // Execute command
                    match self.execute(cmd) {
                        Ok(should_quit) => {
                            if should_quit {
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ {}", e);
                        }
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("^C (use 'quit' to exit)");
                }
                Err(ReadlineError::Eof) => {
                    println!("^D");
                    break;
                }
                Err(e) => {
                    return Err(format!("readline error: {}", e));
                }
            }
        }

        // Save state on exit
        println!("Saving session...");
        self.save_state()?;
        println!("Goodbye!");

        Ok(())
    }

    /// Execute a REPL command
    /// Returns Ok(true) if the REPL should quit
    fn execute(&mut self, cmd: ReplCommand) -> Result<bool, String> {
        match cmd {
            ReplCommand::Add(instruction) => {
                self.cmd_add(&instruction)?;
                Ok(false)
            }
            ReplCommand::Preview => {
                self.cmd_preview()?;
                Ok(false)
            }
            ReplCommand::Edit(num) => {
                self.cmd_edit(num)?;
                Ok(false)
            }
            ReplCommand::Remove(num) => {
                self.cmd_remove(num)?;
                Ok(false)
            }
            ReplCommand::Clear => {
                self.cmd_clear()?;
                Ok(false)
            }
            ReplCommand::Validate => {
                self.cmd_validate()?;
                Ok(false)
            }
            ReplCommand::Submit => {
                self.cmd_submit()?;
                Ok(false)
            }
            ReplCommand::Save => {
                self.save_state()?;
                println!("✓ Session saved");
                Ok(false)
            }

            // Plan operations (AGX-073)
            ReplCommand::PlanList => {
                self.cmd_plan_list()?;
                Ok(false)
            }
            ReplCommand::PlanGet(plan_id) => {
                self.cmd_plan_get(&plan_id)?;
                Ok(false)
            }

            // Action operations (AGX-073)
            ReplCommand::ActionSubmit { plan_id, input } => {
                self.cmd_action_submit(&plan_id, input.as_deref())?;
                Ok(false)
            }

            // Operational commands (AGX-058)
            ReplCommand::JobList => {
                self.cmd_job_list()?;
                Ok(false)
            }
            ReplCommand::WorkerList => {
                self.cmd_worker_list()?;
                Ok(false)
            }
            ReplCommand::QueueStats => {
                self.cmd_queue_stats()?;
                Ok(false)
            }

            ReplCommand::Help => {
                self.cmd_help();
                Ok(false)
            }
            ReplCommand::Quit => {
                Ok(true)
            }
        }
    }

    /// Add instruction and generate plan steps
    fn cmd_add(&mut self, instruction: &str) -> Result<(), String> {
        println!("🤖 Generating plan steps...");

        // Build context for planner
        let reg = registry::ToolRegistry::new();
        let tool_registry: Vec<ToolInfo> = reg.tools()
            .iter()
            .map(|t| ToolInfo::new(t.id, t.description))
            .collect();

        let context = PlanContext {
            tool_registry,
            input_summary: Some(format!("Interactive instruction: {}", instruction)),
            ..Default::default()
        };

        // Generate plan using Echo model (reuse existing runtime)
        let generated = self.runtime.block_on(async {
            self.backend.generate_plan(instruction, &context).await
        }).map_err(|e| format!("plan generation failed: {}", e))?;

        if generated.tasks.is_empty() {
            return Err("no tasks generated".to_string());
        }

        // Append to existing plan with overflow check
        let start_num = u32::try_from(self.state.plan.tasks.len() + 1)
            .map_err(|_| "plan exceeds maximum task count (2^32)".to_string())?;

        for (i, mut task) in generated.tasks.into_iter().enumerate() {
            let task_num = start_num.checked_add(i as u32)
                .ok_or_else(|| "task number overflow".to_string())?;
            task.task_number = task_num;
            self.state.plan.tasks.push(task);
        }

        let added_count = self.state.plan.tasks.len() - start_num as usize + 1;
        println!("✓ Added {} task(s)", added_count);

        // Auto-save to plan buffer
        self.plan_storage.save(&self.state.plan)?;

        Ok(())
    }

    /// Preview current plan
    fn cmd_preview(&self) -> Result<(), String> {
        if self.state.plan.tasks.is_empty() {
            println!("📋 Plan is empty");
            return Ok(());
        }

        println!("📋 Current plan ({} tasks):", self.state.plan.tasks.len());
        println!();

        for task in &self.state.plan.tasks {
            println!("  {}. {} {}",
                task.task_number,
                task.command,
                task.args.join(" "));

            if let Some(input_from) = task.input_from_task {
                println!("     ← input from task {}", input_from);
            }
        }

        println!();

        Ok(())
    }

    /// Edit a specific task
    fn cmd_edit(&mut self, num: usize) -> Result<(), String> {
        if num > self.state.plan.tasks.len() {
            return Err(format!("task {} does not exist (plan has {} tasks)",
                num, self.state.plan.tasks.len()));
        }

        let task = &self.state.plan.tasks[num - 1];

        println!("Editing task {}:", num);
        println!("  Current: {} {}", task.command, task.args.join(" "));
        println!();

        // Read new command
        let prompt = "  New command> ";
        let new_cmd = self.editor.readline(prompt)
            .map_err(|e| format!("failed to read input: {}", e))?;

        let new_cmd = new_cmd.trim();
        if new_cmd.is_empty() {
            return Err("command cannot be empty".to_string());
        }

        // Parse command and args
        let parts: Vec<String> = new_cmd.split_whitespace()
            .map(|s| s.to_string())
            .collect();

        if parts.is_empty() {
            return Err("command cannot be empty".to_string());
        }

        // Validate command doesn't contain shell metacharacters
        let command = &parts[0];
        if command.contains(';') || command.contains('|') || command.contains('&')
            || command.contains('`') || command.contains('$') {
            return Err("invalid characters in command (shell metacharacters not allowed)".to_string());
        }

        // Update task
        let task = &mut self.state.plan.tasks[num - 1];
        task.command = parts[0].clone();
        task.args = parts[1..].to_vec();

        println!("✓ Updated task {}", num);

        // Auto-save
        self.plan_storage.save(&self.state.plan)?;

        Ok(())
    }

    /// Remove a task
    fn cmd_remove(&mut self, num: usize) -> Result<(), String> {
        if num > self.state.plan.tasks.len() {
            return Err(format!("task {} does not exist (plan has {} tasks)",
                num, self.state.plan.tasks.len()));
        }

        self.state.plan.tasks.remove(num - 1);

        // Renumber remaining tasks
        for (i, task) in self.state.plan.tasks.iter_mut().enumerate() {
            task.task_number = (i + 1) as u32;
        }

        println!("✓ Removed task {}", num);

        // Auto-save
        self.plan_storage.save(&self.state.plan)?;

        Ok(())
    }

    /// Clear the plan
    fn cmd_clear(&mut self) -> Result<(), String> {
        self.state.plan = WorkflowPlan::default();
        self.plan_storage.save(&self.state.plan)?;

        println!("✓ Plan cleared");

        Ok(())
    }

    /// Validate plan with Delta model
    fn cmd_validate(&self) -> Result<(), String> {
        if self.state.plan.tasks.is_empty() {
            return Err("plan is empty, nothing to validate".to_string());
        }

        println!("🤖 Validating with Delta model...");
        println!("⚠️  Delta validation not yet implemented (AGX-045, AGX-046)");

        Ok(())
    }

    /// Submit plan to AGQ
    fn cmd_submit(&self) -> Result<(), String> {
        use crate::agq_client::{AgqClient, AgqConfig};
        use crate::build_job_envelope;

        if self.state.plan.tasks.is_empty() {
            return Err("plan is empty, nothing to submit".to_string());
        }

        println!("📤 Submitting plan to AGQ...");

        // Build job envelope from current plan
        let job = build_job_envelope(self.state.plan.clone())
            .map_err(|e| format!("failed to build job envelope: {}", e))?;

        let plan_id = job.plan_id.clone();
        let task_count = job.tasks.len();

        let job_json = serde_json::to_string(&job)
            .map_err(|e| format!("failed to serialize job: {}", e))?;

        // Submit to AGQ
        let config = AgqConfig::from_env();
        let agq_addr = config.addr.clone(); // Store for error messages
        let client = AgqClient::new(config);

        match client.submit_plan(&job_json) {
            Ok(submission) => {
                // Save submission metadata (AGX-075)
                let metadata = crate::plan_buffer::PlanMetadata {
                    job_id: submission.job_id.clone(),
                    submitted_at: chrono::DateTime::<chrono::Utc>::from(
                        submission.submitted_at,
                    )
                    .to_rfc3339(),
                };

                // Non-fatal: warn but don't fail the submission
                if let Err(e) = self.plan_storage.save_submission_metadata(&metadata) {
                    eprintln!("⚠️  Warning: Failed to save submission metadata: {}", e);
                }

                println!("✅ Plan submitted successfully");
                println!("   Plan ID: {}", plan_id);
                println!("   Tasks: {}", task_count);
                println!();
                println!("Use with: agx ACTION submit --plan-id {}", plan_id);
                println!("         (optional: --input '{{...}}' or --inputs-file <path>)");
                Ok(())
            }
            Err(e) => {
                // Provide helpful context for connection errors
                if e.contains("connect error") {
                    Err(format!(
                        "Failed to connect to AGQ at {}\n\
                         Error: {}\n\
                         \n\
                         Troubleshooting:\n\
                         - Ensure AGQ is running\n\
                         - Check AGQ_ADDR environment variable (current: {})\n\
                         - Verify network connectivity",
                        agq_addr, e, agq_addr
                    ))
                } else {
                    Err(format!("failed to submit plan: {}", e))
                }
            }
        }
    }

    /// List jobs from AGQ (AGX-058)
    fn cmd_job_list(&self) -> Result<(), String> {
        use crate::agq_client::{AgqClient, AgqConfig, OpsResponse};

        let config = AgqConfig::from_env();
        let client = AgqClient::new(config);

        match client.list_jobs() {
            Ok(OpsResponse::Jobs(jobs)) => {
                if jobs.is_empty() {
                    println!("No jobs found");
                } else {
                    println!("\nJobs ({}):", jobs.len());
                    for job in jobs {
                        println!("  - {}", job);
                    }
                    println!();
                }
                Ok(())
            }
            Ok(_) => Err("unexpected response from AGQ".to_string()),
            Err(e) => Err(format!("failed to list jobs: {}", e)),
        }
    }

    /// List workers from AGQ (AGX-058)
    fn cmd_worker_list(&self) -> Result<(), String> {
        use crate::agq_client::{AgqClient, AgqConfig, OpsResponse};

        let config = AgqConfig::from_env();
        let client = AgqClient::new(config);

        match client.list_workers() {
            Ok(OpsResponse::Workers(workers)) => {
                if workers.is_empty() {
                    println!("No active workers");
                } else {
                    println!("\nActive Workers ({}):", workers.len());
                    for worker in workers {
                        println!("  - {}", worker);
                    }
                    println!();
                }
                Ok(())
            }
            Ok(_) => Err("unexpected response from AGQ".to_string()),
            Err(e) => Err(format!("failed to list workers: {}", e)),
        }
    }

    /// Show queue statistics from AGQ (AGX-058)
    fn cmd_queue_stats(&self) -> Result<(), String> {
        use crate::agq_client::{AgqClient, AgqConfig, OpsResponse};

        let config = AgqConfig::from_env();
        let client = AgqClient::new(config);

        match client.queue_stats() {
            Ok(OpsResponse::QueueStats(stats)) => {
                println!("\nQueue Statistics:");
                for stat in stats {
                    println!("  {}", stat);
                }
                println!();
                Ok(())
            }
            Ok(_) => Err("unexpected response from AGQ".to_string()),
            Err(e) => Err(format!("failed to get queue stats: {}", e)),
        }
    }

    /// List all plans from AGQ (AGX-073)
    fn cmd_plan_list(&self) -> Result<(), String> {
        use crate::agq_client::{AgqClient, AgqConfig};

        let config = AgqConfig::from_env();
        let client = AgqClient::new(config);

        match client.list_plans() {
            Ok(plans) => {
                if plans.is_empty() {
                    println!("No plans found");
                } else {
                    println!("\nPlans ({}):", plans.len());
                    for plan in plans {
                        let desc = plan.description.unwrap_or_else(|| "(no description)".to_string());
                        println!("  {} ({} tasks) - {}", plan.plan_id, plan.task_count, desc);
                    }
                    println!();
                }
                Ok(())
            }
            Err(e) => Err(format!("failed to list plans: {}", e)),
        }
    }

    /// Get a specific plan from AGQ (AGX-073)
    fn cmd_plan_get(&self, plan_id: &str) -> Result<(), String> {
        use crate::agq_client::{AgqClient, AgqConfig};

        let config = AgqConfig::from_env();
        let client = AgqClient::new(config);

        match client.get_plan(plan_id) {
            Ok(plan) => {
                println!("\nPlan: {}", plan_id);
                if plan.tasks.is_empty() {
                    println!("  (no tasks)");
                } else {
                    println!("Tasks:");
                    for task in &plan.tasks {
                        let args_str = if task.args.is_empty() {
                            String::new()
                        } else {
                            format!(" {}", task.args.join(" "))
                        };
                        println!("  {}. {}{}", task.task_number, task.command, args_str);
                    }
                }
                println!();
                Ok(())
            }
            Err(e) => Err(format!("failed to get plan: {}", e)),
        }
    }

    /// Submit action to execute plan with input data (AGX-073)
    fn cmd_action_submit(&self, plan_id: &str, input: Option<&str>) -> Result<(), String> {
        use crate::agq_client::{AgqClient, AgqConfig};

        // Validate plan-id format (same as CLI)
        if !plan_id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(
                "invalid plan-id: must contain only alphanumeric characters, underscore, or dash"
                    .to_string(),
            );
        }

        if plan_id.len() > 128 {
            return Err("plan-id too long (max 128 characters)".to_string());
        }

        let config = AgqConfig::from_env();
        let client = AgqClient::new(config);

        // Step 1: Retrieve plan to validate it exists
        match client.get_plan(plan_id) {
            Ok(_) => {}, // Plan exists, continue
            Err(e) => {
                if e.contains("AGQ error") {
                    return Err(format!("Error: Plan '{}' not found", plan_id));
                } else {
                    return Err(format!("Error: Cannot connect to AGQ: {}", e));
                }
            }
        }

        // Step 2: Parse input JSON
        let inputs_array = if let Some(input_str) = input {
            let single_input = serde_json::from_str::<serde_json::Value>(input_str)
                .map_err(|e| format!("Error: Invalid input JSON: {}", e))?;
            serde_json::json!([single_input])
        } else {
            serde_json::json!([])
        };

        // Step 3: Generate action_id
        let action_id = format!("action_{}", uuid::Uuid::new_v4().simple());

        // Step 4: Build ACTION.SUBMIT payload
        let action_request = serde_json::json!({
            "action_id": action_id,
            "plan_id": plan_id,
            "inputs": inputs_array,
        });

        let action_json = serde_json::to_string(&action_request)
            .map_err(|e| format!("failed to serialize action request: {}", e))?;

        // Step 5: Submit to AGQ
        match client.submit_action(&action_json) {
            Ok(response) => {
                println!("✅ Action submitted successfully");
                if let Some(job_id) = response.job_ids.first() {
                    println!("Job ID: {}", job_id);
                }
                println!("Plan: {}", response.plan_id);
                if let Some(input_val) = action_request.get("inputs").and_then(|v| v.get(0)) {
                    if !input_val.is_array() || input_val.as_array().map_or(false, |a| !a.is_empty()) {
                        println!("Input: {}", input_val);
                    }
                }
                Ok(())
            }
            Err(e) => Err(format!("ACTION submit failed: {}", e)),
        }
    }

    /// Show help text
    fn cmd_help(&self) {
        println!("AGX Interactive REPL v{}", env!("CARGO_PKG_VERSION"));
        println!();
        println!("Commands:");
        println!("  [a]dd <instruction>    Generate and append plan steps");
        println!("  [p]review              Show current plan");
        println!("  [e]dit <num>           Modify a specific step");
        println!("  [r]emove <num>         Delete a specific step");
        println!("  [c]lear                Reset the plan");
        println!();
        println!("Plan Actions:");
        println!("  [v]alidate             Run Delta model validation");
        println!("  [s]ubmit               Submit plan to AGQ");
        println!("  save                   Manually save session");
        println!();
        println!("Plan Operations:");
        println!("  plan list              List all stored plans from AGQ");
        println!("  plan get <id>          View details of a specific plan");
        println!();
        println!("Action Operations:");
        println!("  action <plan-id>       Execute plan (no input)");
        println!("  action <plan-id> <json> Execute plan with input data");
        println!();
        println!("Operational Commands:");
        println!("  [j]obs                 List all jobs from AGQ");
        println!("  [w]orkers              List active workers");
        println!("  stats / queue          Show queue statistics");
        println!();
        println!("Session:");
        println!("  [h]elp                 Show this help");
        println!("  [q]uit                 Exit REPL");
        println!();
        println!("Keyboard Shortcuts:");
        println!("  Ctrl-G                 Enter vi mode for editing");
        println!("  Ctrl-C                 Cancel current input");
        println!("  Ctrl-D                 Exit REPL");
        println!();
        println!("Tip: Type the full command or just the first letter (e.g., 'a' or 'add')");
        println!();
    }

    /// Save REPL state to disk
    fn save_state(&mut self) -> Result<(), String> {
        // Update history from editor
        self.state.history.clear();

        for entry in self.editor.history().iter() {
            self.state.history.push(entry.to_string());
        }

        // Save state
        self.state.save()?;

        // Also save to plan buffer
        self.plan_storage.save(&self.state.plan)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_add_command() {
        let cmd = ReplCommand::parse("add convert PDF to text").unwrap();
        assert_eq!(cmd, ReplCommand::Add("convert PDF to text".to_string()));
    }

    #[test]
    fn parse_add_requires_instruction() {
        let result = ReplCommand::parse("add");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires an instruction"));
    }

    #[test]
    fn parse_preview_command() {
        assert_eq!(ReplCommand::parse("preview").unwrap(), ReplCommand::Preview);
        assert_eq!(ReplCommand::parse("show").unwrap(), ReplCommand::Preview);
        assert_eq!(ReplCommand::parse("list").unwrap(), ReplCommand::Preview);
    }

    #[test]
    fn parse_edit_command() {
        let cmd = ReplCommand::parse("edit 3").unwrap();
        assert_eq!(cmd, ReplCommand::Edit(3));
    }

    #[test]
    fn parse_edit_requires_step_number() {
        let result = ReplCommand::parse("edit");
        assert!(result.is_err());
    }

    #[test]
    fn parse_edit_rejects_zero() {
        let result = ReplCommand::parse("edit 0");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("start at 1"));
    }

    #[test]
    fn parse_remove_command() {
        assert_eq!(ReplCommand::parse("remove 2").unwrap(), ReplCommand::Remove(2));
        assert_eq!(ReplCommand::parse("delete 2").unwrap(), ReplCommand::Remove(2));
        assert_eq!(ReplCommand::parse("rm 2").unwrap(), ReplCommand::Remove(2));
    }

    #[test]
    fn parse_clear_command() {
        assert_eq!(ReplCommand::parse("clear").unwrap(), ReplCommand::Clear);
        assert_eq!(ReplCommand::parse("reset").unwrap(), ReplCommand::Clear);
        assert_eq!(ReplCommand::parse("new").unwrap(), ReplCommand::Clear);
    }

    #[test]
    fn parse_validate_command() {
        assert_eq!(ReplCommand::parse("validate").unwrap(), ReplCommand::Validate);
    }

    #[test]
    fn parse_submit_command() {
        assert_eq!(ReplCommand::parse("submit").unwrap(), ReplCommand::Submit);
    }

    #[test]
    fn parse_help_command() {
        assert_eq!(ReplCommand::parse("help").unwrap(), ReplCommand::Help);
        assert_eq!(ReplCommand::parse("?").unwrap(), ReplCommand::Help);
    }

    #[test]
    fn parse_quit_command() {
        assert_eq!(ReplCommand::parse("quit").unwrap(), ReplCommand::Quit);
        assert_eq!(ReplCommand::parse("exit").unwrap(), ReplCommand::Quit);
        assert_eq!(ReplCommand::parse("q").unwrap(), ReplCommand::Quit);
    }

    #[test]
    fn parse_unknown_command() {
        let result = ReplCommand::parse("foobar");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown command"));
    }

    #[test]
    fn parse_empty_command() {
        let result = ReplCommand::parse("");
        assert!(result.is_err());
    }

    // Shortcut tests (AGX-057)
    #[test]
    fn parse_add_shortcut() {
        let cmd = ReplCommand::parse("a sort numbers").unwrap();
        assert_eq!(cmd, ReplCommand::Add("sort numbers".to_string()));
    }

    #[test]
    fn parse_preview_shortcut() {
        assert_eq!(ReplCommand::parse("p").unwrap(), ReplCommand::Preview);
    }

    #[test]
    fn parse_edit_shortcut() {
        let cmd = ReplCommand::parse("e 2").unwrap();
        assert_eq!(cmd, ReplCommand::Edit(2));
    }

    #[test]
    fn parse_remove_shortcut() {
        let cmd = ReplCommand::parse("r 1").unwrap();
        assert_eq!(cmd, ReplCommand::Remove(1));
    }

    #[test]
    fn parse_clear_shortcut() {
        assert_eq!(ReplCommand::parse("c").unwrap(), ReplCommand::Clear);
    }

    #[test]
    fn parse_validate_shortcut() {
        assert_eq!(ReplCommand::parse("v").unwrap(), ReplCommand::Validate);
    }

    #[test]
    fn parse_submit_shortcut() {
        assert_eq!(ReplCommand::parse("s").unwrap(), ReplCommand::Submit);
    }

    #[test]
    fn parse_help_shortcut() {
        assert_eq!(ReplCommand::parse("h").unwrap(), ReplCommand::Help);
    }

    #[test]
    fn parse_quit_shortcut() {
        assert_eq!(ReplCommand::parse("q").unwrap(), ReplCommand::Quit);
    }

    // Operational command tests (AGX-058)
    #[test]
    fn parse_jobs_command() {
        assert_eq!(ReplCommand::parse("jobs").unwrap(), ReplCommand::JobList);
    }

    #[test]
    fn parse_jobs_shortcut() {
        assert_eq!(ReplCommand::parse("j").unwrap(), ReplCommand::JobList);
    }

    #[test]
    fn parse_workers_command() {
        assert_eq!(ReplCommand::parse("workers").unwrap(), ReplCommand::WorkerList);
    }

    #[test]
    fn parse_workers_shortcut() {
        assert_eq!(ReplCommand::parse("w").unwrap(), ReplCommand::WorkerList);
    }

    #[test]
    fn parse_stats_command() {
        assert_eq!(ReplCommand::parse("stats").unwrap(), ReplCommand::QueueStats);
    }

    #[test]
    fn parse_queue_command() {
        assert_eq!(ReplCommand::parse("queue").unwrap(), ReplCommand::QueueStats);
    }

    // Integration tests for state persistence
    #[test]
    fn test_state_save_and_load() {
        use tempfile::TempDir;

        // Create temp directory for test state
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("test-repl-state.json");

        // Create a state with data
        let mut state = ReplState {
            plan: WorkflowPlan {
                plan_id: Some("test-plan".to_string()),
                plan_description: Some("Test plan".to_string()),
                tasks: vec![],
            },
            history: vec!["add test".to_string(), "preview".to_string()],
            last_saved: None,
        };

        // Save state
        let json = serde_json::to_string_pretty(&state).unwrap();
        std::fs::write(&state_path, json).unwrap();

        // Load state
        let loaded_json = std::fs::read_to_string(&state_path).unwrap();
        let loaded_state: ReplState = serde_json::from_str(&loaded_json).unwrap();

        // Verify
        assert_eq!(loaded_state.plan.plan_id, Some("test-plan".to_string()));
        assert_eq!(loaded_state.history.len(), 2);
        assert_eq!(loaded_state.history[0], "add test");
    }

    #[test]
    fn test_state_history_limit() {
        let mut state = ReplState::default();

        // Add more than MAX_HISTORY_SIZE entries
        for i in 0..1500 {
            state.history.push(format!("command {}", i));
        }

        // Simulate save which should trim history
        if state.history.len() > MAX_HISTORY_SIZE {
            let start = state.history.len() - MAX_HISTORY_SIZE;
            state.history = state.history[start..].to_vec();
        }

        // Verify history is limited
        assert_eq!(state.history.len(), MAX_HISTORY_SIZE);
        assert_eq!(state.history[0], "command 500");  // First 500 should be trimmed
        assert_eq!(state.history[999], "command 1499");
    }

    #[test]
    fn test_overflow_protection() {
        // Test u32 overflow check
        let result = u32::try_from(usize::MAX);
        assert!(result.is_err());

        // Test checked_add
        let max_u32 = u32::MAX;
        assert!(max_u32.checked_add(1).is_none());
    }

    // Plan operations tests (AGX-073)
    #[test]
    fn parse_plan_list_command() {
        let cmd = ReplCommand::parse("plan list").unwrap();
        assert_eq!(cmd, ReplCommand::PlanList);
    }

    #[test]
    fn parse_plan_get_command() {
        let cmd = ReplCommand::parse("plan get plan_abc123").unwrap();
        assert_eq!(cmd, ReplCommand::PlanGet("plan_abc123".to_string()));
    }

    #[test]
    fn plan_get_requires_plan_id() {
        let result = ReplCommand::parse("plan get");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires plan-id"));
    }

    #[test]
    fn plan_requires_subcommand() {
        let result = ReplCommand::parse("plan");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires subcommand"));
    }

    #[test]
    fn plan_rejects_unknown_subcommand() {
        let result = ReplCommand::parse("plan invalid");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown plan subcommand"));
    }

    // Action operations tests (AGX-073)
    #[test]
    fn parse_action_command_no_input() {
        let cmd = ReplCommand::parse("action plan_abc123").unwrap();
        assert_eq!(cmd, ReplCommand::ActionSubmit {
            plan_id: "plan_abc123".to_string(),
            input: None
        });
    }

    #[test]
    fn parse_action_command_with_input() {
        let cmd = ReplCommand::parse("action plan_abc123 {\"path\":\"/tmp\"}").unwrap();
        assert_eq!(cmd, ReplCommand::ActionSubmit {
            plan_id: "plan_abc123".to_string(),
            input: Some("{\"path\":\"/tmp\"}".to_string())
        });
    }

    #[test]
    fn action_requires_plan_id() {
        let result = ReplCommand::parse("action");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires plan-id"));
    }

    // Submit command tests (AGX-075)
    #[test]
    fn submit_rejects_empty_plan() {
        // Create a REPL with empty plan
        use crate::planner::backend::ModelBackend;
        use crate::planner::types::{GeneratedPlan, ModelError, PlanContext};
        use async_trait::async_trait;

        // Mock backend for testing
        struct MockBackend;
        #[async_trait]
        impl ModelBackend for MockBackend {
            async fn generate_plan(
                &self,
                _instruction: &str,
                _ctx: &PlanContext,
            ) -> Result<GeneratedPlan, ModelError> {
                unreachable!("generate_plan should not be called in this test")
            }

            fn backend_type(&self) -> &'static str {
                "mock"
            }

            fn model_name(&self) -> &str {
                "mock-model"
            }

            async fn health_check(&self) -> Result<(), ModelError> {
                Ok(())
            }
        }

        let backend = Box::new(MockBackend);
        let mut repl = Repl::new(backend).unwrap();

        // Clear any persisted plan to ensure empty state
        repl.state.plan.tasks.clear();

        // Ensure plan is now empty
        assert!(repl.state.plan.tasks.is_empty());

        // Try to submit - should fail with empty plan error
        let result = repl.cmd_submit();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("plan is empty"));
    }
}
