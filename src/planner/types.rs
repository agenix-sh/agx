use crate::plan::PlanStep;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Context provided to the model for plan generation
#[derive(Debug, Clone)]
pub struct PlanContext {
    /// Available tools/commands with descriptions
    pub tool_registry: Vec<ToolInfo>,
    /// Summary of input data (optional)
    pub input_summary: Option<String>,
    /// Existing tasks for refinement (used by Delta model)
    pub existing_tasks: Vec<PlanStep>,
    /// Maximum number of tasks to generate
    pub max_tasks: usize,
}

impl Default for PlanContext {
    fn default() -> Self {
        Self {
            tool_registry: Vec::new(),
            input_summary: None,
            existing_tasks: Vec::new(),
            max_tasks: 20,
        }
    }
}

/// Information about an available tool/command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
}

impl ToolInfo {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
        }
    }
}

/// Output from model plan generation
#[derive(Debug, Clone)]
pub struct GeneratedPlan {
    /// The generated tasks
    pub tasks: Vec<PlanStep>,
    /// Metadata about the generation process
    pub metadata: PlanMetadata,
}

/// Metadata about plan generation
#[derive(Debug, Clone)]
pub struct PlanMetadata {
    /// Model identifier used for generation
    pub model_used: String,
    /// Token count (if available)
    pub tokens: Option<usize>,
    /// Latency in milliseconds
    pub latency_ms: u64,
    /// Backend type (e.g., "candle", "ollama", "openai")
    pub backend: String,
}

/// Errors that can occur during model operations
#[derive(Error, Debug)]
pub enum ModelError {
    #[error("Failed to load model: {0}")]
    LoadError(String),

    #[error("Model inference failed: {0}")]
    InferenceError(String),

    #[error("Failed to parse model output: {0}")]
    ParseError(String),

    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    #[error("Model health check failed: {0}")]
    HealthCheckError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Candle error: {0}")]
    CandleError(String),

    #[error("Tokenizer error: {0}")]
    TokenizerError(String),
}

// Implement From for Candle errors
impl From<candle_core::Error> for ModelError {
    fn from(err: candle_core::Error) -> Self {
        ModelError::CandleError(err.to_string())
    }
}

// Implement From for tokenizers errors
impl From<tokenizers::Error> for ModelError {
    fn from(err: tokenizers::Error) -> Self {
        ModelError::TokenizerError(err.to_string())
    }
}
