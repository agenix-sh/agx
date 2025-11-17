use async_trait::async_trait;

use super::types::{GeneratedPlan, ModelError, PlanContext};

/// Trait for model backends that generate plans from natural language instructions
#[async_trait]
pub trait ModelBackend: Send + Sync {
    /// Generate a plan from a natural language instruction
    ///
    /// # Arguments
    /// * `instruction` - The user's natural language instruction
    /// * `context` - Additional context for plan generation (tools, input summary, etc.)
    ///
    /// # Returns
    /// A `GeneratedPlan` containing the tasks and metadata, or a `ModelError`
    async fn generate_plan(
        &self,
        instruction: &str,
        context: &PlanContext,
    ) -> Result<GeneratedPlan, ModelError>;

    /// Get the backend type identifier (e.g., "candle", "ollama", "openai")
    fn backend_type(&self) -> &'static str;

    /// Get the model name/identifier
    fn model_name(&self) -> &str;

    /// Validate that the model is loaded and ready to generate plans
    async fn health_check(&self) -> Result<(), ModelError>;
}
