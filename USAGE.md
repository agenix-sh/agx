# AGX Usage Guide

## Quick Start

### 1. Build AGX

```bash
# Build release binary
cargo build --release

# Binary location
./target/release/agx
```

### 2. Using with Ollama Backend (Recommended for Testing)

**Prerequisites:**
```bash
# Install Ollama
curl -fsSL https://ollama.com/install.sh | sh

# Pull a model
ollama pull phi3:mini
```

**Basic Usage:**

```bash
# Create a new plan
./target/release/agx plan new

# Add tasks to plan with input data
echo "sample.txt data.csv report.pdf" | ./target/release/agx plan add "find all text files"

# Preview the plan
./target/release/agx plan preview

# Submit plan to AGQ (requires AGQ running)
./target/release/agx plan submit
```

**With Different Models:**

```bash
# Use a specific Ollama model
export AGX_OLLAMA_MODEL=llama3:8b
echo "data" | ./target/release/agx plan add "analyze this data"

# Reset for next run
unset AGX_OLLAMA_MODEL
```

### 3. Using with Candle Backend (Local GPU Inference)

**Status:** ✅ Supports both LLaMA and Qwen2 architectures via automatic detection

**Known Limitation:** ⚠️ Metal backend (macOS GPU) has incomplete quantized RMS-norm support. Use CPU mode or CUDA until resolved.

```bash
# Download models
./scripts/download-models.sh

# Use Echo model (fast planning) - CPU mode
export AGX_BACKEND=candle
export AGX_MODEL_ROLE=echo
export AGX_ECHO_MODEL="$HOME/.agx/models/echo/VibeThinker-1.5B.Q4_K_M.gguf"
export AGX_DEVICE=cpu  # Required on macOS until Metal issue resolved

echo "data" | ./target/release/agx plan add "process this"

# Use Delta model (validation) - CPU mode
export AGX_MODEL_ROLE=delta
export AGX_DELTA_MODEL="$HOME/.agx/models/delta/Mistral-Nemo-Instruct-2407.Q4_K_M.gguf"
export AGX_DEVICE=cpu  # Required on macOS until Metal issue resolved

echo "data" | ./target/release/agx plan add "validate and refine"

# CUDA (Linux/Windows with NVIDIA GPU) works without AGX_DEVICE override
```

## Common Workflows

### Planning Workflow

```bash
# 1. Create new plan
./target/release/agx plan new

# 2. Add tasks (can be done multiple times)
echo "file1.txt file2.txt" | ./target/release/agx plan add "sort these files"
echo "data.csv" | ./target/release/agx plan add "count lines"

# 3. Preview the complete plan
./target/release/agx plan preview

# 4. Submit to AGQ for execution
./target/release/agx plan submit
```

### Operations (Requires AGQ)

```bash
# View queue stats
./target/release/agx ops queue

# List jobs
./target/release/agx ops jobs

# List workers
./target/release/agx ops workers
```

## Configuration

### Environment Variables

**Backend Selection:**
```bash
AGX_BACKEND=ollama        # Use Ollama (default)
AGX_BACKEND=candle        # Use Candle (local GPU)
```

**Ollama Configuration:**
```bash
AGX_OLLAMA_MODEL=phi3:mini           # Model to use (default: phi3:mini)
AGX_OLLAMA_TIMEOUT_SECS=300          # Timeout in seconds (default: 300)
```

**Candle Configuration:**
```bash
AGX_MODEL_ROLE=echo                  # echo (fast) or delta (thorough)
AGX_ECHO_MODEL=/path/to/model.gguf   # Echo model path
AGX_DELTA_MODEL=/path/to/model.gguf  # Delta model path

# Optional GPU settings
AGX_DEVICE=cuda                      # Force CUDA
AGX_DEVICE=metal                     # Force Metal (macOS)
AGX_DEVICE=cpu                       # Force CPU

# Optional generation settings
AGX_CANDLE_TEMPERATURE=0.7           # Temperature (default: 0.7)
AGX_CANDLE_TOP_P=0.9                 # Top-p sampling (default: 0.9)
AGX_CANDLE_MAX_TOKENS=2048           # Max tokens (default: 2048)
AGX_CANDLE_CONTEXT_SIZE=2048         # Context window (default: 2048)
AGX_CANDLE_SEED=12345                # Random seed (optional, for reproducibility)
```

**AGQ Configuration:**
```bash
AGX_AGQ_HOST=localhost               # AGQ host (default: localhost)
AGX_AGQ_PORT=6379                    # AGQ port (default: 6379)
```

**Debug:**
```bash
AGX_DEBUG=1                          # Enable debug logging
```

## Examples

### Example 1: Text Processing

```bash
./target/release/agx plan new

# Plan to process text files
cat << EOF | ./target/release/agx plan add "count unique words"
The quick brown fox
jumps over the lazy dog
The quick brown fox
EOF

./target/release/agx plan preview
```

### Example 2: File Operations

```bash
./target/release/agx plan new

# Generate plan for file operations
echo "*.txt" | ./target/release/agx plan add "find all text files and sort by name"

./target/release/agx plan preview
```

### Example 3: Using Different Backends

```bash
# With Ollama (default)
echo "data" | ./target/release/agx plan add "analyze"

# With Ollama + specific model
AGX_OLLAMA_MODEL=llama3:8b \
  echo "data" | ./target/release/agx plan add "analyze"

# With Candle (when Qwen support added)
AGX_BACKEND=candle \
  AGX_MODEL_ROLE=echo \
  AGX_ECHO_MODEL="$HOME/.agx/models/echo/vibethinker-1.5b.gguf" \
  echo "data" | ./target/release/agx plan add "analyze"
```

## Troubleshooting

### "Backend error: Failed to parse model output"

The LLM didn't generate valid JSON. Try:
1. Use a different model: `export AGX_OLLAMA_MODEL=llama3:8b`
2. Simplify your instruction
3. Enable debug: `export AGX_DEBUG=1`

### "Failed to initialize planner backend: ConfigError"

Check your configuration:
```bash
# For Ollama
ollama list  # Verify model exists

# For Candle
ls -lh "$AGX_ECHO_MODEL"  # Verify model file exists
```

### "Ollama call timed out"

Increase timeout:
```bash
export AGX_OLLAMA_TIMEOUT_SECS=600  # 10 minutes
```

### "No GPU detected, falling back to CPU"

For Candle backend on macOS:
```bash
# Verify Metal support
system_profiler SPDisplaysDataType | grep Metal

# Force Metal
export AGX_DEVICE=metal
```

## Performance Tips

### Ollama
- Use smaller models for faster planning: `phi3:mini` (2.2GB)
- Use larger models for better accuracy: `llama3:8b` (4.7GB)

### Candle
- **Echo (fast)**: VibeThinker-1.5B-Q4_K_M - <2s latency (CPU ~30-60s), <2GB VRAM
- **Delta (thorough)**: Mistral-Nemo-Q4_K_M - <10s latency (CPU ~60-120s), <8GB VRAM
- GPU highly recommended (50x+ faster than CPU)
- **Note:** Metal (macOS GPU) currently unsupported for quantized models. Use CPU or CUDA.

## Documentation

- [BACKENDS.md](./BACKENDS.md) - Detailed backend architecture
- [CLAUDE.md](./CLAUDE.md) - Development guidelines

## Quick Reference

```bash
# Planning commands
agx plan new                    # Create new plan
agx plan add "instruction"      # Add tasks to plan
agx plan preview                # View current plan
agx plan submit                 # Submit to AGQ

# Operations commands (requires AGQ)
agx ops queue                   # View queue stats
agx ops jobs                    # List jobs
agx ops workers                 # List workers

# Help
agx --help                      # Show help
agx --version                   # Show version
```
