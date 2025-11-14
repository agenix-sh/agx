use std::process::Command;

use crate::input::InputSummary;
use crate::plan::WorkflowPlan;
use crate::registry::ToolRegistry;

const DEFAULT_OLLAMA_MODEL: &str = "phi3:mini";

pub enum BackendKind {
    Ollama,
}

impl BackendKind {
    pub fn from_env() -> Self {
        match std::env::var("AGX_BACKEND") {
            Ok(value) => {
                let normalized = value.to_lowercase();

                match normalized.as_str() {
                    "" | "ollama" => BackendKind::Ollama,
                    _ => BackendKind::Ollama,
                }
            }
            Err(_) => BackendKind::Ollama,
        }
    }
}

pub struct PlannerConfig {
    pub model: String,
    pub backend: BackendKind,
}

impl PlannerConfig {
    pub fn from_env() -> Self {
        let backend = BackendKind::from_env();

        let model = match backend {
            BackendKind::Ollama => std::env::var("AGX_OLLAMA_MODEL")
                .unwrap_or_else(|_| DEFAULT_OLLAMA_MODEL.to_string()),
        };

        Self { model, backend }
    }
}

pub trait ModelBackend {
    fn generate_plan(&self, prompt: &str) -> Result<String, String>;
}

struct OllamaBackend {
    model: String,
}

impl OllamaBackend {
    fn new(model: String) -> Self {
        Self { model }
    }
}

impl ModelBackend for OllamaBackend {
    fn generate_plan(&self, prompt: &str) -> Result<String, String> {
        let output = Command::new("ollama")
            .arg("run")
            .arg(&self.model)
            .arg(prompt)
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

        Ok(text.trim().to_string())
    }
}

pub struct Planner {
    backend: Box<dyn ModelBackend>,
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
        let backend: Box<dyn ModelBackend> = match config.backend {
            BackendKind::Ollama => Box::new(OllamaBackend::new(config.model)),
        };

        Self { backend }
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

        let text = self.backend.generate_plan(&prompt)?;

        Ok(PlannerOutput { raw_json: text })
    }
}
