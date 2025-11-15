use std::process::Command;

use crate::input::InputSummary;
use crate::plan::WorkflowPlan;
use crate::registry::ToolRegistry;

const DEFAULT_OLLAMA_MODEL: &str = "phi3:mini";

pub enum BackendKind {
    Ollama,
    #[cfg(feature = "embedded-backend")]
    Embedded,
}

impl BackendKind {
    pub fn from_env() -> Self {
        match std::env::var("AGX_BACKEND") {
            Ok(value) => {
                let normalized = value.to_lowercase();

                match normalized.as_str() {
                    "" | "ollama" => BackendKind::Ollama,
                    #[cfg(feature = "embedded-backend")]
                    "embedded" | "llm" => BackendKind::Embedded,
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
            #[cfg(feature = "embedded-backend")]
            BackendKind::Embedded => {
                std::env::var("AGX_MODEL_PATH").unwrap_or_else(|_| "model.bin".to_string())
            }
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

#[cfg(feature = "embedded-backend")]
struct EmbeddedBackend {
    model: Box<dyn llm::Model>,
}

#[cfg(feature = "embedded-backend")]
impl EmbeddedBackend {
    fn new(model_path: String) -> Result<Self, String> {
        use std::path::Path;

        let arch = match std::env::var("AGX_MODEL_ARCH") {
            Ok(value) => value
                .parse::<llm::ModelArchitecture>()
                .unwrap_or(llm::ModelArchitecture::Llama),
            Err(_) => llm::ModelArchitecture::Llama,
        };

        let tokenizer_source = llm::TokenizerSource::Embedded;

        let model = llm::load_dynamic(
            Some(arch),
            Path::new(&model_path),
            tokenizer_source,
            Default::default(),
            llm::load_progress_callback_stdout,
        )
        .map_err(|error| format!("failed to load embedded model from {}: {error}", model_path))?;

        Ok(Self { model })
    }
}

#[cfg(feature = "embedded-backend")]
impl ModelBackend for EmbeddedBackend {
    fn generate_plan(&self, prompt: &str) -> Result<String, String> {
        use std::convert::Infallible;

        use llm::{InferenceFeedback, InferenceParameters, InferenceRequest, InferenceResponse};
        use rand::thread_rng;

        let mut session = self.model.start_session(Default::default());
        let mut output = String::new();

        let result = session.infer::<Infallible>(
            self.model.as_ref(),
            &mut thread_rng(),
            &InferenceRequest {
                prompt: prompt.into(),
                parameters: &InferenceParameters::default(),
                play_back_previous_tokens: false,
                maximum_token_count: None,
            },
            &mut Default::default(),
            |response| {
                if let InferenceResponse::PromptToken(token)
                | InferenceResponse::InferredToken(token) = response
                {
                    output.push_str(&token);
                }

                Ok(InferenceFeedback::Continue)
            },
        );

        match result {
            Ok(_) => Ok(output.trim().to_string()),
            Err(error) => Err(format!("embedded model inference error: {error}")),
        }
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
        let backend: Result<Box<dyn ModelBackend>, String> = match config.backend {
            BackendKind::Ollama => Ok(Box::new(OllamaBackend::new(config.model))),
            #[cfg(feature = "embedded-backend")]
            BackendKind::Embedded => EmbeddedBackend::new(config.model).map(|b| Box::new(b) as _),
        };

        Self {
            backend: backend
                .unwrap_or_else(|error| panic!("planner backend initialization failed: {error}")),
        }
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
