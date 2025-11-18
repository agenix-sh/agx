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

/// REPL session state that persists across invocations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplState {
    /// Current plan buffer
    pub plan: WorkflowPlan,
    /// Command history
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

        Ok(path)
    }
}

/// REPL command parsed from user input
#[derive(Debug, Clone, PartialEq)]
pub enum ReplCommand {
    Add(String),
    Preview,
    Edit(usize),
    Remove(usize),
    Clear,
    Validate,
    Submit,
    Save,
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
            "add" => {
                let instruction = parts.get(1)
                    .ok_or_else(|| "add requires an instruction".to_string())?
                    .trim();

                if instruction.is_empty() {
                    return Err("add requires a non-empty instruction".to_string());
                }

                Ok(ReplCommand::Add(instruction.to_string()))
            }
            "preview" | "show" | "list" => Ok(ReplCommand::Preview),
            "edit" => {
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
            "remove" | "delete" | "rm" => {
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
            "clear" | "reset" | "new" => Ok(ReplCommand::Clear),
            "validate" => Ok(ReplCommand::Validate),
            "submit" => Ok(ReplCommand::Submit),
            "save" => Ok(ReplCommand::Save),
            "help" | "?" => Ok(ReplCommand::Help),
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

        Ok(Self {
            state,
            editor,
            backend,
            plan_storage,
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

        // Generate plan using Echo model
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("failed to create runtime: {}", e))?;

        let generated = rt.block_on(async {
            self.backend.generate_plan(instruction, &context).await
        }).map_err(|e| format!("plan generation failed: {}", e))?;

        if generated.tasks.is_empty() {
            return Err("no tasks generated".to_string());
        }

        // Append to existing plan
        let start_num = (self.state.plan.tasks.len() + 1) as u32;

        for (i, mut task) in generated.tasks.into_iter().enumerate() {
            task.task_number = start_num + i as u32;
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
        if self.state.plan.tasks.is_empty() {
            return Err("plan is empty, nothing to submit".to_string());
        }

        println!("📤 Submitting plan to AGQ...");
        println!("⚠️  Submit via REPL not yet fully integrated");
        println!("   Use 'agx PLAN submit' for now");

        Ok(())
    }

    /// Show help text
    fn cmd_help(&self) {
        println!("AGX REPL Commands:");
        println!();
        println!("  add <instruction>     Generate and append plan steps");
        println!("  preview               Show current plan");
        println!("  edit <num>            Modify a specific step");
        println!("  remove <num>          Delete a specific step");
        println!("  clear                 Reset the plan");
        println!("  validate              Run Delta model validation");
        println!("  submit                Submit plan to AGQ");
        println!("  save                  Manually save session");
        println!("  help                  Show this help");
        println!("  quit                  Exit REPL");
        println!();
        println!("Keyboard shortcuts:");
        println!("  Ctrl-G                Enter vi mode for editing");
        println!("  Ctrl-C                Cancel current input");
        println!("  Ctrl-D                Exit REPL");
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
}
