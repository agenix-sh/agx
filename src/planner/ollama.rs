use std::time::{Duration, Instant};

use async_trait::async_trait;

use super::backend::ModelBackend;
use super::types::{GeneratedPlan, ModelError, PlanContext, PlanMetadata};
use crate::plan::{PlanStep, WorkflowPlan};

/// Ollama backend configuration
#[derive(Debug, Clone)]
pub struct OllamaConfig {
    pub model: String,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            model: std::env::var("AGX_OLLAMA_MODEL").unwrap_or_else(|_| "phi3:mini".to_string()),
        }
    }
}

/// Ollama backend using CLI invocation
pub struct OllamaBackend {
    model: String,
}

impl OllamaBackend {
    pub fn new(model: String) -> Self {
        Self { model }
    }

    pub fn from_config(config: OllamaConfig) -> Self {
        Self::new(config.model)
    }

    /// Build prompt for Ollama
    fn build_prompt(&self, instruction: &str, context: &PlanContext) -> String {
        let input_description = context
            .input_summary
            .as_ref()
            .map(|s| format!("Input description:\n{}\n\n", s))
            .unwrap_or_default();

        let tools_description = context
            .tool_registry
            .iter()
            .map(|t| format!("{}: {}", t.name, t.description))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "You are the AGX Planner.\n\
             \n\
             User instruction:\n\
             {instruction}\n\
             \n\
             {input_description}\
             Available tools:\n\
             {tools}\n\
             \n\
             Respond with a single JSON object only, no extra commentary.\n\
             Use this exact format:\n\
             {{\"tasks\": [{{\"task_number\": 1, \"command\": \"tool-id\", \"args\": [], \"timeout_secs\": 300}}]}}\n\
             \n\
             - task_number: 1-based, contiguous (1, 2, 3...)\n\
             - command: tool identifier from list above\n\
             - args: arguments for the command (empty array if none)\n\
             - timeout_secs: timeout in seconds (default 300)\n\
             \n\
             Use only the tools listed above and produce a deterministic, minimal plan.",
            instruction = instruction,
            input_description = input_description,
            tools = tools_description
        )
    }

    /// Parse model response into tasks
    fn parse_plan_response(&self, response: &str) -> Result<Vec<PlanStep>, ModelError> {
        let plan = WorkflowPlan::from_str(response)
            .map_err(|e| ModelError::ParseError(format!("Failed to parse plan JSON: {}", e)))?;

        Ok(plan.tasks)
    }
}

#[async_trait]
impl ModelBackend for OllamaBackend {
    async fn generate_plan(
        &self,
        instruction: &str,
        context: &PlanContext,
    ) -> Result<GeneratedPlan, ModelError> {
        let prompt = self.build_prompt(instruction, context);
        let model = self.model.clone();

        // Timeout for Ollama calls (default 5 minutes)
        let timeout_secs = std::env::var("AGX_OLLAMA_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(300);

        // Run ollama in a blocking task with timeout
        let (response, latency_ms) = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            tokio::task::spawn_blocking(move || {
            let start = Instant::now();

            let output = std::process::Command::new("ollama")
                .arg("run")
                .arg(&model)
                .arg(&prompt)
                .output()
                .map_err(|error| {
                    ModelError::InferenceError(format!("failed to run ollama: {}", error))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(ModelError::InferenceError(format!(
                    "ollama exited with status {}: {}",
                    output.status,
                    stderr.trim()
                )));
            }

            let text = String::from_utf8(output.stdout).map_err(|error| {
                ModelError::InferenceError(format!("ollama produced non-UTF-8 output: {}", error))
            })?;

            let latency_ms = start.elapsed().as_millis() as u64;

            Ok::<_, ModelError>((text.trim().to_string(), latency_ms))
        }),
        )
        .await
        .map_err(|_| {
            ModelError::InferenceError(format!(
                "Ollama call timed out after {} seconds",
                timeout_secs
            ))
        })?
        .map_err(|e| ModelError::InferenceError(format!("Task join error: {}", e)))??;

        // Parse the response
        let tasks = self.parse_plan_response(&response)?;

        Ok(GeneratedPlan {
            tasks,
            metadata: PlanMetadata {
                model_used: self.model.clone(),
                tokens: None, // Ollama doesn't expose token counts via CLI
                latency_ms,
                backend: "ollama".to_string(),
            },
        })
    }

    fn backend_type(&self) -> &'static str {
        "ollama"
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    async fn health_check(&self) -> Result<(), ModelError> {
        let model = self.model.clone();

        tokio::task::spawn_blocking(move || {
            // Try to list models to verify ollama is installed
            let output = std::process::Command::new("ollama")
                .arg("list")
                .output()
                .map_err(|e| {
                    ModelError::HealthCheckError(format!(
                        "Failed to run 'ollama list': {}. Is ollama installed?",
                        e
                    ))
                })?;

            if !output.status.success() {
                return Err(ModelError::HealthCheckError(
                    "ollama list command failed".to_string(),
                ));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);

            // Check if the specified model is in the list
            if !stdout.contains(&model) {
                return Err(ModelError::HealthCheckError(format!(
                    "Model '{}' not found. Run 'ollama pull {}' to download it.",
                    model, model
                )));
            }

            Ok(())
        })
        .await
        .map_err(|e| ModelError::HealthCheckError(format!("Task join error: {}", e)))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::types::ToolInfo;

    #[test]
    fn test_ollama_prompt_generation() {
        let backend = OllamaBackend::new("phi3:mini".to_string());
        let context = PlanContext {
            tool_registry: vec![ToolInfo::new("ls", "list files")],
            input_summary: Some("test input".to_string()),
            ..Default::default()
        };

        let prompt = backend.build_prompt("list files", &context);

        assert!(prompt.contains("list files"));
        assert!(prompt.contains("test input"));
        assert!(prompt.contains("ls: list files"));
    }
}
