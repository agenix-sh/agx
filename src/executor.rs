use std::io::{self, Write};
use std::process::{Command, Stdio};

use crate::input::InputSummary;
use crate::plan::WorkflowPlan;
use crate::registry::ToolRegistry;

pub struct Executor;

impl Executor {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(
        &self,
        plan: &WorkflowPlan,
        input: &InputSummary,
        registry: &ToolRegistry,
    ) -> Result<(), String> {
        if plan.tasks.is_empty() {
            return io::stdout()
                .write_all(&input.content)
                .map_err(|error| format!("failed to write to STDOUT: {error}"));
        }

        let mut data = input.content.clone();

        for task in &plan.tasks {
            let tool = registry
                .find_by_id(&task.command)
                .ok_or_else(|| format!("unknown tool in plan: {}", task.command))?;

            let mut child = Command::new(tool.command);
            child.args(&task.args);

            let mut child = child
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .map_err(|error| format!("failed to start command '{}': {error}", tool.command))?;

            if let Some(stdin) = child.stdin.as_mut() {
                stdin
                    .write_all(&data)
                    .map_err(|error| format!("failed to write to '{}': {error}", tool.command))?;
            }

            let output = child
                .wait_with_output()
                .map_err(|error| format!("failed to wait for '{}': {error}", tool.command))?;

            let status = output.status;
            let code = status.code();
            let is_ok = match code {
                Some(value) => tool.ok_exit_codes.contains(&value),
                None => status.success(),
            };

            if !is_ok {
                let stderr = String::from_utf8_lossy(&output.stderr);

                return Err(format!(
                    "command '{}' failed with status {}: {}",
                    tool.command,
                    status,
                    stderr.trim()
                ));
            }

            data = output.stdout;
        }

        io::stdout()
            .write_all(&data)
            .map_err(|error| format!("failed to write final output to STDOUT: {error}"))
    }
}
