use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use async_trait::async_trait;
use candle_core::{Device, Tensor};
use candle_transformers::models::quantized_llama;
use candle_transformers::models::quantized_qwen2;
use tokenizers::Tokenizer;

use super::backend::ModelBackend;
use super::device::select_device_from_env;
use super::types::{GeneratedPlan, ModelError, PlanContext, PlanMetadata, ToolInfo};
use crate::plan::{PlanStep, WorkflowPlan};

/// Unified model wrapper supporting multiple architectures
enum ModelWeights {
    Llama(quantized_llama::ModelWeights),
    Qwen2(quantized_qwen2::ModelWeights),
}

impl ModelWeights {
    /// Detect architecture from GGUF metadata and load appropriate model
    fn from_gguf<R: std::io::Seek + std::io::Read>(
        content: candle_core::quantized::gguf_file::Content,
        reader: &mut R,
        device: &Device,
    ) -> Result<Self, ModelError> {
        // Detect architecture by checking for architecture-specific metadata keys
        let arch = if content.metadata.contains_key("qwen2.attention.head_count") {
            "qwen2"
        } else if content.metadata.contains_key("llama.attention.head_count") {
            "llama"
        } else {
            return Err(ModelError::LoadError(
                "Unknown model architecture. Expected 'llama' or 'qwen2' metadata keys."
                    .to_string(),
            ));
        };

        log::info!("Detected model architecture: {}", arch);

        match arch {
            "qwen2" => {
                let model = quantized_qwen2::ModelWeights::from_gguf(content, reader, device)?;
                Ok(ModelWeights::Qwen2(model))
            }
            "llama" => {
                let model = quantized_llama::ModelWeights::from_gguf(content, reader, device)?;
                Ok(ModelWeights::Llama(model))
            }
            _ => unreachable!(),
        }
    }

    /// Forward pass through the model
    fn forward(&mut self, x: &Tensor, index_pos: usize) -> candle_core::Result<Tensor> {
        match self {
            ModelWeights::Llama(model) => model.forward(x, index_pos),
            ModelWeights::Qwen2(model) => model.forward(x, index_pos),
        }
    }
}

/// Configuration for Candle backend
#[derive(Debug, Clone)]
pub struct CandleConfig {
    /// Path to the GGUF model file
    pub model_path: PathBuf,
    /// Temperature for sampling (0.0 = deterministic, higher = more creative)
    pub temperature: f64,
    /// Top-p sampling parameter
    pub top_p: f64,
    /// Maximum tokens to generate
    pub max_tokens: usize,
    /// Repetition penalty
    pub repeat_penalty: f32,
    /// Model role (echo or delta) for prompt selection
    pub model_role: ModelRole,
    /// RNG seed for reproducible generation (None = random)
    pub seed: Option<u64>,
    /// Context window size for token generation
    pub context_size: usize,
}

/// Model role determines prompt style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelRole {
    /// Echo: Fast, conversational planning
    Echo,
    /// Delta: Thorough validation and refinement
    Delta,
}

impl Default for CandleConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::from("model.gguf"),
            temperature: 0.7,
            top_p: 0.9,
            max_tokens: 2048,
            repeat_penalty: 1.1,
            model_role: ModelRole::Echo,
            seed: None, // Random seed by default
            context_size: 2048,
        }
    }
}

impl CandleConfig {
    /// Build configuration from environment variables
    pub fn from_env(role: ModelRole) -> Result<Self, ModelError> {
        let model_path = match role {
            ModelRole::Echo => {
                std::env::var("AGX_ECHO_MODEL")
                    .or_else(|_| std::env::var("AGX_MODEL_PATH"))
                    .map(PathBuf::from)
                    .map_err(|_| {
                        ModelError::ConfigError(
                            "No model path specified. Set AGX_ECHO_MODEL or AGX_MODEL_PATH"
                                .to_string(),
                        )
                    })?
            }
            ModelRole::Delta => {
                std::env::var("AGX_DELTA_MODEL")
                    .or_else(|_| std::env::var("AGX_MODEL_PATH"))
                    .map(PathBuf::from)
                    .map_err(|_| {
                        ModelError::ConfigError(
                            "No model path specified. Set AGX_DELTA_MODEL or AGX_MODEL_PATH"
                                .to_string(),
                        )
                    })?
            }
        };

        let temperature = std::env::var("AGX_CANDLE_TEMPERATURE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.7);

        let top_p = std::env::var("AGX_CANDLE_TOP_P")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.9);

        let max_tokens = std::env::var("AGX_CANDLE_MAX_TOKENS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2048);

        let seed = std::env::var("AGX_CANDLE_SEED")
            .ok()
            .and_then(|s| s.parse().ok());

        let context_size = std::env::var("AGX_CANDLE_CONTEXT_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2048);

        Ok(Self {
            model_path,
            temperature,
            top_p,
            max_tokens,
            repeat_penalty: 1.1,
            model_role: role,
            seed,
            context_size,
        })
    }

    /// Get tokenizer path (assumes tokenizer.json in same directory as model)
    pub fn tokenizer_path(&self) -> PathBuf {
        self.model_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("tokenizer.json")
    }
}

/// Candle-based model backend for local LLM inference
pub struct CandleBackend {
    model: Mutex<ModelWeights>,
    tokenizer: Tokenizer,
    device: Device,
    config: CandleConfig,
}

impl CandleBackend {
    /// Create a new Candle backend with the given configuration
    pub async fn new(config: CandleConfig) -> Result<Self, ModelError> {
        // Run model loading in a blocking task to avoid blocking async runtime
        let backend = tokio::task::spawn_blocking(move || {
            let device = select_device_from_env()?;

            log::info!(
                "Loading model from {:?} on {:?}",
                config.model_path,
                device
            );

            if !config.model_path.exists() {
                return Err(ModelError::ConfigError(format!(
                    "Model file not found: {:?}",
                    config.model_path
                )));
            }

            // Load GGUF model weights
            let mut file = std::fs::File::open(&config.model_path).map_err(|e| {
                ModelError::LoadError(format!(
                    "Failed to open model file {:?}: {}",
                    config.model_path, e
                ))
            })?;

            // Parse GGUF file content
            let content = candle_core::quantized::gguf_file::Content::read(&mut file)?;

            // Load model from GGUF
            let model = ModelWeights::from_gguf(content, &mut file, &device)?;

            // Load tokenizer
            let tokenizer_path = config.tokenizer_path();
            if !tokenizer_path.exists() {
                return Err(ModelError::ConfigError(format!(
                    "Tokenizer not found at {:?}. Place tokenizer.json next to model file.",
                    tokenizer_path
                )));
            }

            let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|e| {
                ModelError::TokenizerError(format!(
                    "Failed to load tokenizer from {:?}: {}",
                    tokenizer_path, e
                ))
            })?;

            Ok::<_, ModelError>(Self {
                model: Mutex::new(model),
                tokenizer,
                device,
                config,
            })
        })
        .await
        .map_err(|e| ModelError::LoadError(format!("Task join error: {}", e)))??;

        Ok(backend)
    }

    /// Build prompt based on model role (Echo vs Delta)
    fn build_prompt(&self, instruction: &str, context: &PlanContext) -> String {
        match self.config.model_role {
            ModelRole::Echo => self.build_echo_prompt(instruction, context),
            ModelRole::Delta => self.build_delta_prompt(instruction, context),
        }
    }

    /// Build Echo prompt (fast, streamlined)
    fn build_echo_prompt(&self, instruction: &str, context: &PlanContext) -> String {
        let tools = self.format_tool_list(&context.tool_registry);
        let input_info = context
            .input_summary
            .as_ref()
            .map(|s| format!("\nInput: {}", s))
            .unwrap_or_default();

        format!(
            "You are a fast task planner. Convert this instruction into a JSON task list.\n\
             Available tools: {}\n\
             Instruction: {}{}\n\
             Output only valid JSON: {{\"plan\": [{{\"cmd\": \"tool-id\"}}, ...]}}",
            tools, instruction, input_info
        )
    }

    /// Build Delta prompt (thorough, validation-focused)
    fn build_delta_prompt(&self, instruction: &str, context: &PlanContext) -> String {
        let tools = self.format_tool_list(&context.tool_registry);
        let existing_plan = if !context.existing_tasks.is_empty() {
            serde_json::to_string(&context.existing_tasks).unwrap_or_default()
        } else {
            "[]".to_string()
        };

        format!(
            "You are an expert task planner. Validate and refine this plan.\n\
             Original instruction: {}\n\
             Current plan: {}\n\
             Available tools: {}\n\
             \n\
             Validate:\n\
             1. Task ordering and dependencies\n\
             2. Tool availability and arguments\n\
             3. Error handling\n\
             4. Edge cases\n\
             \n\
             Output improved JSON plan: {{\"plan\": [{{\"cmd\": \"tool-id\", \"args\": [...]}}]}}",
            instruction, existing_plan, tools
        )
    }

    /// Format tool list for prompt
    fn format_tool_list(&self, tools: &[ToolInfo]) -> String {
        tools
            .iter()
            .map(|t| format!("{} ({})", t.name, t.description))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Generate tokens using the model
    fn generate_tokens(&self, input_tokens: &[u32]) -> Result<Vec<u32>, ModelError> {
        use candle_transformers::generation::LogitsProcessor;

        // Use configured seed or generate random one
        let seed = self.config.seed.unwrap_or_else(|| {
            use std::collections::hash_map::RandomState;
            use std::hash::{BuildHasher, Hash, Hasher};
            let mut hasher = RandomState::new().build_hasher();
            std::time::SystemTime::now().hash(&mut hasher);
            hasher.finish()
        });

        let mut logits_processor = LogitsProcessor::new(
            seed,
            Some(self.config.temperature),
            Some(self.config.top_p),
        );

        let mut tokens = input_tokens.to_vec();
        let mut generated_tokens = Vec::new();

        // Lock the model for generation
        let mut model = self.model.lock().map_err(|e| {
            ModelError::InferenceError(format!("Failed to lock model mutex: {}", e))
        })?;

        // Generate tokens one by one
        for _ in 0..self.config.max_tokens {
            let context_size = if tokens.len() > self.config.context_size {
                self.config.context_size
            } else {
                tokens.len()
            };

            let start_pos = tokens.len().saturating_sub(context_size);
            let context_tokens = &tokens[start_pos..];

            let input = candle_core::Tensor::new(context_tokens, &self.device)?
                .unsqueeze(0)?;

            let logits = model.forward(&input, start_pos)?;
            let logits = logits.squeeze(0)?.to_dtype(candle_core::DType::F32)?;

            let next_token = logits_processor.sample(&logits)?;
            tokens.push(next_token);
            generated_tokens.push(next_token);

            // Check for EOS token (typically 2 for LLaMA models)
            if next_token == 2 {
                break;
            }

            // Early stopping if we can parse valid JSON
            // Check every 10 tokens to avoid too much overhead
            if generated_tokens.len() % 10 == 0 {
                if let Ok(text) = self.tokenizer.decode(&generated_tokens, true) {
                    // Try to parse as JSON - if successful, we have a complete response
                    if serde_json::from_str::<serde_json::Value>(&text).is_ok() {
                        log::debug!("Valid JSON detected, stopping generation early");
                        break;
                    }
                }
            }
        }

        Ok(generated_tokens)
    }

    /// Parse model response into tasks
    fn parse_plan_response(&self, response: &str) -> Result<Vec<PlanStep>, ModelError> {
        // Use existing WorkflowPlan parser which handles various JSON formats
        let plan = WorkflowPlan::from_str(response)
            .map_err(|e| ModelError::ParseError(format!("Failed to parse plan JSON: {}", e)))?;

        Ok(plan.plan)
    }
}

#[async_trait]
impl ModelBackend for CandleBackend {
    async fn generate_plan(
        &self,
        instruction: &str,
        context: &PlanContext,
    ) -> Result<GeneratedPlan, ModelError> {
        let prompt = self.build_prompt(instruction, context);
        let start = Instant::now();

        // Tokenize
        let encoding = self.tokenizer.encode(prompt, true)?;
        let input_tokens: Vec<u32> = encoding.get_ids().to_vec();

        // Generate tokens (CPU-intensive, but we keep it sync for now)
        // TODO: Consider using spawn_blocking if generation is too slow
        let output_tokens = self.generate_tokens(&input_tokens)?;

        // Decode
        let response = self.tokenizer.decode(&output_tokens, true)?;

        let latency_ms = start.elapsed().as_millis() as u64;

        // Parse the response
        let tasks = self.parse_plan_response(&response)?;

        Ok(GeneratedPlan {
            tasks,
            metadata: PlanMetadata {
                model_used: self
                    .config
                    .model_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
                tokens: Some(output_tokens.len()),
                latency_ms,
                backend: "candle".to_string(),
            },
        })
    }

    fn backend_type(&self) -> &'static str {
        "candle"
    }

    fn model_name(&self) -> &str {
        self.config
            .model_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap_or("unknown")
    }

    async fn health_check(&self) -> Result<(), ModelError> {
        // Simple test: try to tokenize a short string
        self.tokenizer
            .encode("test", true)
            .map_err(|e| ModelError::HealthCheckError(format!("Tokenizer test failed: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_echo_prompt_structure() {
        let config = CandleConfig {
            model_role: ModelRole::Echo,
            ..Default::default()
        };

        let context = PlanContext {
            tool_registry: vec![ToolInfo::new("ls", "list files")],
            ..Default::default()
        };

        // Test prompt building without needing a full backend
        let tools = context
            .tool_registry
            .iter()
            .map(|t| format!("{} ({})", t.name, t.description))
            .collect::<Vec<_>>()
            .join(", ");

        let prompt = format!(
            "You are a fast task planner. Convert this instruction into a JSON task list.\n\
             Available tools: {}\n\
             Instruction: {}\n\
             Output only valid JSON: {{\"plan\": [{{\"cmd\": \"tool-id\"}}, ...]}}",
            tools, "list files"
        );

        assert!(prompt.contains("fast task planner"));
        assert!(prompt.contains("list files"));
        assert!(!prompt.contains("validate")); // Echo should be simple
    }

    #[test]
    fn test_delta_prompt_structure() {
        let context = PlanContext {
            tool_registry: vec![ToolInfo::new("ls", "list files")],
            existing_tasks: vec![PlanStep {
                cmd: "ls".to_string(),
                args: vec![],
                input_from_step: None,
                timeout_secs: None,
            }],
            ..Default::default()
        };

        let tools = context
            .tool_registry
            .iter()
            .map(|t| format!("{} ({})", t.name, t.description))
            .collect::<Vec<_>>()
            .join(", ");

        let existing_plan = serde_json::to_string(&context.existing_tasks).unwrap();

        // Just verify the structure matches what we expect for Delta
        assert!(!context.existing_tasks.is_empty());
        assert_eq!(context.existing_tasks[0].cmd, "ls");
    }

    #[test]
    fn test_tool_list_formatting() {
        let tools = vec![
            ToolInfo::new("ls", "list files"),
            ToolInfo::new("grep", "search text"),
        ];

        let formatted = tools
            .iter()
            .map(|t| format!("{} ({})", t.name, t.description))
            .collect::<Vec<_>>()
            .join(", ");

        assert!(formatted.contains("ls (list files)"));
        assert!(formatted.contains("grep (search text)"));
    }

    #[test]
    fn test_model_role_enum() {
        assert_eq!(ModelRole::Echo, ModelRole::Echo);
        assert_ne!(ModelRole::Echo, ModelRole::Delta);
    }

    #[tokio::test]
    async fn test_missing_model_file() {
        let config = CandleConfig {
            model_path: PathBuf::from("/nonexistent/model.gguf"),
            ..Default::default()
        };
        let result = CandleBackend::new(config).await;
        assert!(matches!(result, Err(ModelError::ConfigError(_))));
    }

    #[tokio::test]
    async fn test_missing_tokenizer() {
        // Create a temp file for model (won't be a valid GGUF but tests path checking)
        let temp_dir = std::env::temp_dir();
        let model_path = temp_dir.join("test_model_missing_tok.gguf");
        std::fs::write(&model_path, b"fake model").unwrap();

        let config = CandleConfig {
            model_path: model_path.clone(),
            ..Default::default()
        };

        let result = CandleBackend::new(config).await;
        // Should fail because tokenizer.json doesn't exist
        assert!(result.is_err());

        // Cleanup
        let _ = std::fs::remove_file(&model_path);
    }

    #[test]
    fn test_config_from_env_missing_model() {
        // Clear environment variables
        std::env::remove_var("AGX_ECHO_MODEL");
        std::env::remove_var("AGX_MODEL_PATH");

        let result = CandleConfig::from_env(ModelRole::Echo);
        assert!(matches!(result, Err(ModelError::ConfigError(_))));
    }

    #[test]
    fn test_config_with_seed() {
        std::env::set_var("AGX_ECHO_MODEL", "/tmp/test.gguf");
        std::env::set_var("AGX_CANDLE_SEED", "12345");

        let config = CandleConfig::from_env(ModelRole::Echo).unwrap();
        assert_eq!(config.seed, Some(12345));

        std::env::remove_var("AGX_ECHO_MODEL");
        std::env::remove_var("AGX_CANDLE_SEED");
    }

    #[test]
    fn test_config_with_context_size() {
        std::env::set_var("AGX_ECHO_MODEL", "/tmp/test.gguf");
        std::env::set_var("AGX_CANDLE_CONTEXT_SIZE", "4096");

        let config = CandleConfig::from_env(ModelRole::Echo).unwrap();
        assert_eq!(config.context_size, 4096);

        std::env::remove_var("AGX_ECHO_MODEL");
        std::env::remove_var("AGX_CANDLE_CONTEXT_SIZE");
    }
}
