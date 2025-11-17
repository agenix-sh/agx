# AGX Model Backend Architecture

This document describes the model backend abstraction in AGX, supporting multiple inference engines for plan generation.

## Overview

AGX uses a **ModelBackend** trait to abstract LLM inference, enabling:
- Local inference with Candle (GPU-accelerated GGUF models)
- Fallback to Ollama for compatibility
- Future support for enterprise backends (OpenAI, vLLM, etc.)

## Architecture

### Core Abstraction

```rust
#[async_trait]
pub trait ModelBackend: Send + Sync {
    async fn generate_plan(
        &self,
        instruction: &str,
        context: &PlanContext,
    ) -> Result<GeneratedPlan, ModelError>;

    fn backend_type(&self) -> &'static str;
    fn model_name(&self) -> &str;
    async fn health_check(&self) -> Result<(), ModelError>;
}
```

### Key Types

**PlanContext:**
```rust
pub struct PlanContext {
    pub tool_registry: Vec<ToolInfo>,      // Available tools
    pub input_summary: Option<String>,      // Input data summary
    pub existing_tasks: Vec<PlanStep>,      // For Delta refinement
    pub max_tasks: usize,                   // Generation limit
}
```

**GeneratedPlan:**
```rust
pub struct GeneratedPlan {
    pub tasks: Vec<PlanStep>,      // The generated plan
    pub metadata: PlanMetadata,    // Timing, tokens, model info
}
```

## Backends

### 1. Candle Backend (Recommended)

**Features:**
- Native Rust, no external processes
- GPU acceleration (Metal on macOS, CUDA on Linux)
- Small binary size (~7MB release builds)
- GGUF format support (qwen2.5, mistral, llama, etc.)

**Configuration:**
```bash
export AGX_BACKEND=candle
export AGX_ECHO_MODEL="/path/to/VibeThinker-1.5B.Q4_K_M.gguf"
export AGX_DELTA_MODEL="/path/to/Mistral-Nemo-Instruct-2407.Q4_K_M.gguf"
export AGX_MODEL_ROLE=echo  # or "delta"

# Optional GPU selection
export AGX_DEVICE=cuda      # Force CUDA
export AGX_DEVICE=metal     # Force Metal
export AGX_DEVICE=cpu       # Force CPU

# Optional generation parameters
export AGX_CANDLE_TEMPERATURE=0.7
export AGX_CANDLE_TOP_P=0.9
export AGX_CANDLE_MAX_TOKENS=2048
```

**Model Download:**
```bash
./scripts/download-models.sh
```

**Supported Model Architectures:**
- **Qwen2/Qwen2.5**: Automatic detection via `qwen2.attention.head_count` metadata
- **LLaMA/Mistral**: Automatic detection via `llama.attention.head_count` metadata
- Architecture is detected automatically from GGUF metadata

**GPU Support:**
- **macOS**: ⚠️ Metal currently unsupported for quantized models (use CPU mode)
- **Linux**: CUDA (requires CUDA 12.0+, compute capability 7.0+)
- **Blackwell GPUs**: Supported (compute capability 10.x), verify with CUDA 12.0+
- **Fallback**: CPU (slower but works everywhere)

**Device Selection Priority:**
1. `AGX_DEVICE` environment variable
2. Auto-detection: CUDA > Metal > CPU

### 2. Ollama Backend

**Features:**
- Easy model management (`ollama pull`)
- Compatible with AGX without Rust recompilation
- Good for prototyping

**Configuration:**
```bash
export AGX_BACKEND=ollama
export AGX_OLLAMA_MODEL=phi3:mini
```

**Prerequisites:**
```bash
# Install Ollama
curl -fsSL https://ollama.com/install.sh | sh

# Pull a model
ollama pull phi3:mini
```

### 3. Future Backends

**Planned:**
- OpenAI API (GPT-4, Claude via proxy)
- vLLM (high-throughput serving)
- Custom HTTP endpoints

## Model Roles

AGX uses two model roles for Echo-Delta planning:

### Echo (Fast Planning)
- **Purpose**: Generate initial plans quickly
- **Model**: qwen2.5-1.5b-instruct-q4_k_m.gguf (1.5B params)
- **Latency**:
  - GPU: < 2 seconds for 5-10 task plans
  - CPU: ~30-60 seconds for 5-10 task plans
- **Memory**: < 2GB VRAM (GPU) or < 4GB RAM (CPU)
- **Prompt**: Streamlined, focuses on speed

```bash
export AGX_MODEL_ROLE=echo
export AGX_ECHO_MODEL="$HOME/.agx/models/qwen2.5-1.5b-instruct-q4_k_m.gguf"
```

### Delta (Validation)
- **Purpose**: Validate and refine plans
- **Model**: Mistral-Nemo-Instruct-2407.Q4_K_M.gguf (12B params)
- **Latency**:
  - GPU: < 10 seconds for validation
  - CPU: ~60-120 seconds for validation
- **Memory**: < 8GB VRAM (GPU) or < 16GB RAM (CPU)
- **Prompt**: Comprehensive, validates dependencies, error handling, edge cases

```bash
export AGX_MODEL_ROLE=delta
export AGX_DELTA_MODEL="$HOME/.agx/models/Mistral-Nemo-Instruct-2407.Q4_K_M.gguf"
```

## Prompt Engineering

### Echo Prompt
```
You are a fast task planner. Convert this instruction into a JSON task list.
Available tools: ls (list files), grep (search text), ...
Instruction: {user_instruction}
Output only valid JSON: {"plan": [{"cmd": "tool-id"}, ...]}
```

### Delta Prompt
```
You are an expert task planner. Validate and refine this plan.
Original instruction: {user_instruction}
Current plan: {existing_tasks}
Available tools: ls (list files), grep (search text), ...

Validate:
1. Task ordering and dependencies
2. Tool availability and arguments
3. Error handling
4. Edge cases

Output improved JSON plan: {"plan": [{"cmd": "tool-id", "args": [...]}]}
```

## Performance Targets

### Echo Model (qwen2.5:1.5b)
- **Latency**:
  - GPU: < 2 seconds
  - CPU: ~30-60 seconds
- **Throughput**: 50+ tokens/second (GPU)
- **Memory**: < 2GB VRAM (GPU) or < 4GB RAM (CPU)

### Delta Model (mistral-nemo)
- **Latency**:
  - GPU: < 10 seconds
  - CPU: ~60-120 seconds
- **Throughput**: 30+ tokens/second (GPU)
- **Memory**: < 8GB VRAM (GPU) or < 16GB RAM (CPU)

## Testing

### Unit Tests
```bash
cargo test planner::
```

### Integration Tests (requires models)
```bash
# Download models first
./scripts/download-models.sh

# Set environment
export AGX_BACKEND=candle
export AGX_MODEL_ROLE=echo
export AGX_ECHO_MODEL="$HOME/.agx/models/qwen2.5-1.5b-instruct-q4_k_m.gguf"

# Run integration tests
cargo test --test integration
```

### Health Check
```rust
let planner = Planner::new_async(config).await;
planner.health_check().await?;
```

## Troubleshooting

### "Model file not found"
- Verify model path exists: `ls -lh $AGX_ECHO_MODEL`
- Run download script: `./scripts/download-models.sh`

### "Tokenizer not found"
- Tokenizer must be named `tokenizer.json` in same directory as model
- Download script handles this automatically

### "Failed to initialize CUDA"
- Check CUDA version: `nvcc --version` (requires 12.0+)
- Verify GPU: `nvidia-smi`
- Fallback to CPU: `export AGX_DEVICE=cpu`

### "Metal error: no metal implementation for rms-norm"
- **Known Issue**: Metal backend lacks quantized RMS-norm support in Candle 0.9
- **Workaround**: Use CPU mode on macOS: `export AGX_DEVICE=cpu`
- **Alternative**: Use CUDA on Linux/Windows with NVIDIA GPU
- **Tracking**: This is a Candle framework limitation, not AGX-specific

### Slow inference
- Check device: CPU is 10-50x slower than GPU
- Reduce max_tokens: `export AGX_CANDLE_MAX_TOKENS=1024`
- Use smaller model (Echo instead of Delta)

## Examples

### Basic Usage
```rust
use agx::planner::{Planner, PlannerConfig, BackendKind};

// Create planner
let config = PlannerConfig { backend: BackendKind::Candle };
let planner = Planner::new(config);

// Generate plan
let output = planner.plan(
    "list all rust files and count lines",
    &input_summary,
    &tool_registry,
)?;

let plan = output.parse()?;
```

### Async Usage
```rust
use agx::planner::{CandleBackend, CandleConfig, ModelRole, PlanContext};

// Create backend
let config = CandleConfig::from_env(ModelRole::Echo)?;
let backend = CandleBackend::new(config).await?;

// Generate plan
let context = PlanContext {
    tool_registry: vec![/* ... */],
    input_summary: None,
    existing_tasks: Vec::new(),
    max_tasks: 20,
};

let plan = backend.generate_plan("list files", &context).await?;
println!("Generated {} tasks in {}ms",
         plan.tasks.len(),
         plan.metadata.latency_ms);
```

## References

- **Candle**: https://github.com/huggingface/candle
- **GGUF Format**: https://github.com/ggerganov/ggml/blob/master/docs/gguf.md
- **VibeThinker**: https://huggingface.co/mradermacher/VibeThinker-1.5B-GGUF
- **Mistral-Nemo**: https://huggingface.co/mistralai/Mistral-Nemo-Instruct-2407-GGUF
- **Ollama**: https://ollama.com

## Related Issues

- AGX-022: Model backend abstraction (this implementation)
- AGX-045: Echo model integration
- AGX-046: Delta model integration
- AGX-042: Interactive REPL
