use std::sync::Arc;

use crate::input::InputSummary;
use crate::plan::WorkflowPlan;
use crate::registry::ToolRegistry;

use super::backend::ModelBackend;
use super::candle::{CandleBackend, CandleConfig, ModelRole};
use super::ollama::{OllamaBackend, OllamaConfig};
use super::types::{ModelError, PlanContext, ToolInfo};

/// Backend selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Ollama,
    Candle,
}

impl BackendKind {
    pub fn from_env() -> Self {
        match std::env::var("AGX_BACKEND") {
            Ok(value) => {
                let normalized = value.to_lowercase();
                match normalized.as_str() {
                    "candle" => BackendKind::Candle,
                    "" | "ollama" => BackendKind::Ollama,
                    _ => {
                        log::warn!("Unknown backend '{}', defaulting to ollama", value);
                        BackendKind::Ollama
                    }
                }
            }
            Err(_) => BackendKind::Ollama,
        }
    }
}

/// Planner configuration
pub struct PlannerConfig {
    pub backend: BackendKind,
}

impl PlannerConfig {
    pub fn from_env() -> Self {
        let backend = BackendKind::from_env();
        Self { backend }
    }
}

/// Main planner that wraps backend implementations
pub struct Planner {
    backend: Arc<dyn ModelBackend>,
}

/// Output from planner (for backward compatibility)
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
    /// Create a new planner with the given configuration
    ///
    /// Note: This is a blocking constructor that may perform I/O
    /// For async construction, use `new_async`
    pub fn new(config: PlannerConfig) -> Self {
        // Use tokio runtime to block on async initialization
        let runtime = tokio::runtime::Runtime::new()
            .expect("Failed to create tokio runtime for Planner initialization");

        runtime.block_on(async { Self::new_async(config).await })
    }

    /// Create a new planner asynchronously
    pub async fn new_async(config: PlannerConfig) -> Self {
        let backend: Arc<dyn ModelBackend> = match config.backend {
            BackendKind::Ollama => {
                let ollama_config = OllamaConfig::default();
                Arc::new(OllamaBackend::from_config(ollama_config))
            }
            BackendKind::Candle => {
                // Determine model role from environment
                let role = match std::env::var("AGX_MODEL_ROLE") {
                    Ok(r) if r.eq_ignore_ascii_case("delta") => ModelRole::Delta,
                    _ => ModelRole::Echo,
                };

                let candle_config = CandleConfig::from_env(role).unwrap_or_else(|e| {
                    panic!("Failed to configure Candle backend: {}", e);
                });

                match CandleBackend::new(candle_config).await {
                    Ok(backend) => Arc::new(backend),
                    Err(e) => panic!("Failed to initialize Candle backend: {}", e),
                }
            }
        };

        Self { backend }
    }

    /// Generate a plan from an instruction (backward-compatible sync API)
    pub fn plan(
        &self,
        instruction: &str,
        input: &InputSummary,
        registry: &ToolRegistry,
    ) -> Result<PlannerOutput, String> {
        // Use tokio runtime to block on async plan generation
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;

        runtime.block_on(async { self.plan_async(instruction, input, registry).await })
    }

    /// Generate a plan asynchronously
    pub async fn plan_async(
        &self,
        instruction: &str,
        input: &InputSummary,
        registry: &ToolRegistry,
    ) -> Result<PlannerOutput, String> {
        // Build context from legacy types
        let input_summary = if input.is_empty {
            None
        } else {
            Some(format!(
                "bytes: {}, lines: {}, binary: {}",
                input.bytes, input.lines, input.is_probably_binary
            ))
        };

        let tool_registry: Vec<ToolInfo> = registry
            .list_tools()
            .iter()
            .map(|t| ToolInfo::new(t.id.clone(), t.description.clone()))
            .collect();

        let context = PlanContext {
            tool_registry,
            input_summary,
            existing_tasks: Vec::new(),
            max_tasks: 20,
        };

        // Generate plan using backend
        let generated = self
            .backend
            .generate_plan(instruction, &context)
            .await
            .map_err(|e| format!("Backend error: {}", e))?;

        // Convert back to legacy format (raw JSON)
        let plan = WorkflowPlan {
            plan: generated.tasks,
        };

        let raw_json =
            serde_json::to_string(&plan).map_err(|e| format!("JSON serialization error: {}", e))?;

        Ok(PlannerOutput { raw_json })
    }

    /// Get backend information
    pub fn backend_info(&self) -> (&'static str, &str) {
        (self.backend.backend_type(), self.backend.model_name())
    }

    /// Perform health check on the backend
    pub async fn health_check(&self) -> Result<(), ModelError> {
        self.backend.health_check().await
    }
}
