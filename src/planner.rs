use std::process::Command;

use crate::input::InputSummary;
use crate::plan::WorkflowPlan;
use crate::registry::ToolRegistry;

const DEFAULT_OLLAMA_MODEL: &str = "phi3:mini";

pub struct PlannerConfig {
    pub model: String,
}

impl PlannerConfig {
    pub fn from_env() -> Self {
        let model = std::env::var("AGX_OLLAMA_MODEL")
            .unwrap_or_else(|_| DEFAULT_OLLAMA_MODEL.to_string());

        Self { model }
    }
}

pub struct Planner {
    config: PlannerConfig,
}

pub struct PlannerOutput {
    pub raw_json: String,
}

impl PlannerOutput {
    pub fn parse(&self) -> Result<WorkflowPlan, String> {
        WorkflowPlan::from_str(&self.raw_json)
            .map_err(|error| format!("failed to parse planner JSON: {error}"))
    }
}

impl Planner {
    pub fn new(config: PlannerConfig) -> Self {
        Self { config }
    }

    pub fn plan(
        &self,
        instruction: &str,
        input: &InputSummary,
        registry: &ToolRegistry,
    ) -> Result<PlannerOutput, String> {
        let input_description = format!(
            "bytes: {bytes}, lines: {lines}, is_empty: {is_empty}, is_probably_binary: {binary}",
            bytes = input.bytes,
            lines = input.lines,
            is_empty = input.is_empty,
            binary = input.is_probably_binary
        );

        let tools_description = registry.describe_for_planner();

        let prompt = format!(
            "You are the AGX Planner.\n\
             \n\
             User instruction:\n\
             {instruction}\n\
             \n\
             Input description:\n\
             {input_description}\n\
             \n\
             Available tools:\n\
             {tools}\n\
             \n\
             Respond with a single JSON object only, no extra commentary, in one of these exact shapes:\n\
             {{\"plan\": [{{\"cmd\": \"tool-id\"}}, {{\"cmd\": \"tool-id\", \"args\": [\"arg1\", \"arg2\"]}}]}}\n\
             or\n\
             {{\"plan\": [\"tool-id\", \"another-tool-id\"]}}\n\
             \n\
             Use only the tools listed above and produce a deterministic, minimal plan.",
            instruction = instruction,
            input_description = input_description,
            tools = tools_description
        );

        let output = Command::new("ollama")
            .arg("run")
            .arg(&self.config.model)
            .arg(&prompt)
            .output()
            .map_err(|error| format!("failed to run ollama: {error}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            return Err(format!(
                "ollama exited with status {}: {}",
                output.status,
                stderr.trim()
            ));
        }

        let text = String::from_utf8(output.stdout)
            .map_err(|error| format!("ollama produced non-UTF-8 output: {error}"))?;

        Ok(PlannerOutput {
            raw_json: text.trim().to_string(),
        })
    }
}
