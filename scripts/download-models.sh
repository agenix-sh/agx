#!/bin/bash
# AGX Model Download Script
#
# Downloads recommended GGUF models for AGX planning (Echo and Delta)
#
# Usage:
#   ./scripts/download-models.sh [models_dir]
#
# Environment:
#   AGX_MODELS_DIR - Directory to store models (default: ~/.agx/models)

set -euo pipefail

# Configuration
MODELS_DIR="${1:-${AGX_MODELS_DIR:-$HOME/.agx/models}}"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Create models directory
mkdir -p "$MODELS_DIR"
log_info "Using models directory: $MODELS_DIR"

# Echo model: VibeThinker-1.5B (fast, reasoning-optimized)
ECHO_MODEL_NAME="VibeThinker-1.5B.Q4_K_M.gguf"
ECHO_MODEL_URL="https://huggingface.co/mradermacher/VibeThinker-1.5B-GGUF/resolve/main/${ECHO_MODEL_NAME}"

# Echo model directory (model and tokenizer in same dir)
ECHO_DIR="$MODELS_DIR/echo"
mkdir -p "$ECHO_DIR"

# Echo paths
ECHO_MODEL_PATH="$ECHO_DIR/$ECHO_MODEL_NAME"
# Tokenizer comes from base VibeThinker model repo
ECHO_TOKENIZER_URL="https://huggingface.co/WeiboAI/VibeThinker-1.5B/resolve/main/tokenizer.json"
ECHO_TOKENIZER_PATH="$ECHO_DIR/tokenizer.json"

# Delta model directory
DELTA_DIR="$MODELS_DIR/delta"
mkdir -p "$DELTA_DIR"

# Delta model: Mistral-Nemo (thorough, larger)
DELTA_MODEL_NAME="Mistral-Nemo-Instruct-2407.Q4_K_M.gguf"
DELTA_MODEL_URL="https://huggingface.co/mistralai/Mistral-Nemo-Instruct-2407-GGUF/resolve/main/${DELTA_MODEL_NAME}"
DELTA_MODEL_PATH="$DELTA_DIR/$DELTA_MODEL_NAME"

# Delta tokenizer
DELTA_TOKENIZER_URL="https://huggingface.co/mistralai/Mistral-Nemo-Instruct-2407-GGUF/resolve/main/tokenizer.json"
DELTA_TOKENIZER_PATH="$DELTA_DIR/tokenizer.json"

download_file() {
    local url=$1
    local output=$2
    local name=$3

    if [ -f "$output" ]; then
        log_warn "$name already exists at $output, skipping download"
        return 0
    fi

    log_info "Downloading $name..."
    log_info "  URL: $url"
    log_info "  Output: $output"

    if command -v curl > /dev/null; then
        curl -L --progress-bar "$url" -o "$output"
    elif command -v wget > /dev/null; then
        wget --show-progress "$url" -O "$output"
    else
        log_error "Neither curl nor wget is installed. Please install one of them."
        exit 1
    fi

    if [ $? -eq 0 ]; then
        log_info "✓ Downloaded $name successfully"
    else
        log_error "Failed to download $name"
        exit 1
    fi
}

# Download Echo model and tokenizer
log_info "=== Echo Model (VibeThinker-1.5B) ==="
download_file "$ECHO_MODEL_URL" "$ECHO_MODEL_PATH" "Echo model"
download_file "$ECHO_TOKENIZER_URL" "$ECHO_TOKENIZER_PATH" "Echo tokenizer"

# Download Delta model and tokenizer
log_info ""
log_info "=== Delta Model (Mistral-Nemo) ==="
download_file "$DELTA_MODEL_URL" "$DELTA_MODEL_PATH" "Delta model"
download_file "$DELTA_TOKENIZER_URL" "$DELTA_TOKENIZER_PATH" "Delta tokenizer"

# Print configuration instructions
echo ""
log_info "=== Download Complete ==="
echo ""
echo "To use these models with AGX, set the following environment variables:"
echo ""
echo "  # For Echo model (fast planning):"
echo "  export AGX_BACKEND=candle"
echo "  export AGX_MODEL_ROLE=echo"
echo "  export AGX_ECHO_MODEL=\"$ECHO_MODEL_PATH\""
echo ""
echo "  # For Delta model (thorough validation):"
echo "  export AGX_BACKEND=candle"
echo "  export AGX_MODEL_ROLE=delta"
echo "  export AGX_DELTA_MODEL=\"$DELTA_MODEL_PATH\""
echo ""
echo "Or add to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
echo ""
echo "  export AGX_MODELS_DIR=\"$MODELS_DIR\""
echo "  export AGX_BACKEND=candle"
echo "  export AGX_ECHO_MODEL=\"\$AGX_MODELS_DIR/echo/$ECHO_MODEL_NAME\""
echo "  export AGX_DELTA_MODEL=\"\$AGX_MODELS_DIR/delta/$DELTA_MODEL_NAME\""
echo ""
echo "Note: Tokenizers are automatically loaded from tokenizer.json"
echo "      in the same directory as each model."
echo ""
log_info "Models ready for use!"
