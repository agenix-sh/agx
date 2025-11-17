// Core backend abstraction
pub mod backend;
pub mod types;

// Device selection
pub mod device;

// Backend implementations
pub mod candle;
pub mod ollama;

// High-level wrapper (backward compatible API)
pub mod wrapper;

// Re-exports for backend abstraction
pub use backend::ModelBackend;
pub use candle::{CandleBackend, CandleConfig, ModelRole};
pub use device::{select_device_from_env, DeviceSelector};
pub use ollama::OllamaBackend;
pub use types::{GeneratedPlan, ModelError, PlanContext, PlanMetadata, ToolInfo};

// Re-exports for backward compatibility
pub use wrapper::{BackendKind, Planner, PlannerConfig, PlannerOutput};
